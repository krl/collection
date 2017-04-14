use collection::{Collection, MutContext};

use meta::{Meta, SubMeta};
use meta::cardinality::Cardinality;

use Val;

use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End};

pub trait VectorOps<T, M>
    where Self: Sized,
          T: Val,
          M: Meta<T>
{
    fn insert(&mut self, i: usize, t: T);
    fn remove(&mut self, i: usize) -> Option<T>;
    fn get(&self, i: usize) -> Option<&T>;
    fn get_mut(&mut self, i: usize) -> Option<MutContext<T, M, Beginning>>;
    fn push(&mut self, t: T);
    fn pop(&mut self) -> Option<T>;
    fn split(&mut self, i: usize) -> (Self, Self);
    fn concat(&mut self, b: &mut Self) -> Self;
    fn splice(&mut self, i: usize, from: &mut Self) -> Self;
}

impl<T, M> VectorOps<T, M> for Collection<T, M>
    where T: Val,
          M: Meta<T> + SubMeta<Cardinality<usize>>
{
    fn get(&self, i: usize) -> Option<&T> {
        let mut state = Cardinality::new(&i);

        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut state, &self.stash);

        match res {
            BranchResult::Hit(branch) => branch.leaf(&self.stash),
            _ => None,
        }
    }

    fn get_mut(&mut self, i: usize) -> Option<MutContext<T, M, Beginning>> {
        let mut state = Cardinality::new(&i);

        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut state, &self.stash);

        match res {
            BranchResult::Hit(branch) => Some(self.mut_context(branch)),
            _ => None,
        }
    }

    fn insert(&mut self, i: usize, t: T) {
        let mut key = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut key, &self.stash);
        match res {
            // empty case
            BranchResult::Between(mut branch) => {
                if i == 0 {
                    branch.insert(t, self.divisor, &mut self.stash);
                    self.root = branch.root();
                } else {
                    panic!("Insert past length of collection");
                }
            }
            BranchResult::Hit(mut branch) => {
                branch.insert(t, self.divisor, &mut self.stash);
                self.root = branch.root();
            }
            // non-empty case
            BranchResult::Miss => {
                if *key.inner() == 0 {
                    let mut branch: Branch<_, _, End> =
                        Branch::first(self.root, &self.stash);
                    branch.insert(t, self.divisor, &mut self.stash);
                    self.root = branch.root();
                } else {
                    panic!("Insert past length of collection");
                }
            }
        }
    }

    fn remove(&mut self, i: usize) -> Option<T> {
        let mut key = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut key, &self.stash);
        match res {
            BranchResult::Hit(mut branch) => {
                let ret = branch.remove(self.divisor, &mut self.stash);
                self.root = branch.root();
                ret
            }
            BranchResult::Miss |
            BranchResult::Between(_) => None,

        }
    }

    fn push(&mut self, t: T) {
        let mut branch: Branch<_, _, End> = Branch::first(self.root,
                                                          &self.stash);
        branch.insert(t, self.divisor, &mut self.stash);
        self.root = branch.root();
    }

    fn pop(&mut self) -> Option<T> {
        let mut branch: Branch<_, _, End> = Branch::first(self.root,
                                                          &self.stash);
        let ret = branch.remove(self.divisor, &mut self.stash);
        self.root = branch.root();
        ret
    }

    fn split(&mut self, i: usize) -> (Self, Self)
        where Self: Sized
    {
        if i == 0 {
            return (Collection::new(), self.clone_mut());
        }

        let (mut stash_a, mut stash_b) = self.stash.split(&mut self.root);

        let mut state = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut state, &self.stash);

        match res {
            BranchResult::Hit(branch) => {
                let a = branch.left(&mut stash_a);
                let b = branch.right(&mut stash_b);
                (Collection::new_from(a.root(), stash_a),
                 Collection::new_from(b.root(), stash_b))
            }
            BranchResult::Miss => (self.clone_mut(), Collection::new()),
            _ => unreachable!(),
        }
    }

    fn concat(&mut self, b: &mut Self) -> Self {
        let mut stash =
            self.stash.merge(&mut self.root, &mut b.root, &mut b.stash);

        let a_branch: Branch<_, _, End> = Branch::first(self.root, &stash);
        let b_branch: Branch<_, _, Beginning> = Branch::first(b.root, &stash);

        let branch = a_branch.concat(&b_branch, self.divisor, &mut stash);

        Collection::new_from(branch.root(), stash)
    }

    fn splice(&mut self, i: usize, from: &mut Self) -> Self {
        let (mut first, mut second) = self.split(i);
        first.concat(&mut from.clone_mut()).concat(&mut second)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    const LOTS: usize = 100_000;
    const QUADRATIC: usize = 100;
    const SPLITS: usize = 100;

    use meta::cardinality::Cardinality;
    use meta::checksum::CheckSum;
    use collection::Collection;
    use super::VectorOps;

    use std::hash::Hash;

    collection!(Vector<T> {
        cardinality: Cardinality<usize>,
        checksum: CheckSum<u64>,
    } where T: Hash);

    #[test]
    fn insert() {
        let mut a = Vector::new();
        let mut b = Vector::new();

        for i in 0..LOTS {
            a.push(i);
        }

        for i in 0..LOTS {
            b.insert(0, LOTS - i - 1);
        }

        assert!(a == b);
    }

    #[test]
    fn indexing() {
        let mut vec = Vector::new();

        for i in 0..LOTS {
            vec.push(i);
        }

        for i in 0..LOTS {
            let got = vec.get(i);
            assert_eq!(got, Some(&i));
        }
        assert_eq!(vec.get(LOTS), None);
    }

    #[test]
    fn remove() {
        debug_assert!(LOTS % 2 == 0);

        let mut vec = Vector::new();
        let mut even = Vector::new();
        let empty = Vector::new();

        for i in 0..LOTS {
            vec.push(i);
            if i % 2 == 0 {
                even.push(i);
            }
        }

        for i in 0..LOTS / 2 {
            assert_eq!(vec.remove(i + 1), Some(i * 2 + 1));
        }

        assert!(vec == even);

        for i in 0..LOTS / 2 {
            assert_eq!(vec.remove(0), Some(i * 2));
        }

        assert!(vec == empty);

        assert_eq!(vec.remove(0), None)
    }

    #[test]
    fn remove_from_back() {
        let mut vec = Vector::new();
        let empty = Vector::new();

        for i in 0..LOTS {
            vec.push(i);
        }

        for i in 0..LOTS {
            let i = LOTS - i - 1;
            assert_eq!(vec.remove(i), Some(i));
        }

        assert!(vec == empty);

        assert_eq!(vec.remove(0), None)
    }

    #[test]
    #[should_panic]
    fn insert_panic() {
        let mut vec = Vector::new();
        vec.insert(1, 1);
    }

    #[test]
    fn partial_equal() {
        let mut vec_a = Vector::new();
        let mut vec_b = Vector::new();

        assert!(vec_a == vec_b);

        vec_a.push("a");

        assert!(vec_a != vec_b);

        vec_b.push("a");

        assert!(vec_a == vec_b);
    }

    #[test]
    fn partial_equal_ordering() {
        let mut vec_a = Vector::new();
        let mut vec_b = Vector::new();

        assert!(vec_a == vec_b);

        vec_a.push("a");
        vec_a.push("b");

        vec_b.push("b");
        vec_b.push("a");

        assert!(vec_a != vec_b);
    }

    #[test]
    fn split() {
        let mut vec = Vector::new();

        for i in 0..LOTS {
            vec.push(i)
        }

        let mut split_points = vec![1];

        for i in 0..SPLITS {
            split_points.push((i * LOTS) / SPLITS);
        }
        split_points.push(LOTS - 1);
        split_points.push(LOTS);

        for i in split_points {
            let (a, b) = vec.split(i);

            let mut iter_a = a.iter();
            let mut iter_b = b.iter();

            for o in 0..i {
                assert_eq!(iter_a.next(), Some(&o))
            }
            assert_eq!(iter_a.next(), None);

            for o in i..LOTS {
                assert_eq!(iter_b.next(), Some(&o))
            }
            assert_eq!(iter_b.next(), None);
        }
    }

    #[test]
    fn concat() {
        let mut vec = Vector::new();

        for i in 0..LOTS {
            vec.push(i)
        }

        let mut split_points = vec![1];

        for i in 0..100 {
            split_points.push((LOTS / 100) * i);
        }
        split_points.push(LOTS - 1);
        split_points.push(LOTS);

        for i in split_points {
            let (mut a, mut b) = vec.split(i);
            let c = a.concat(&mut b);
            assert!(c == vec);
        }
    }

    #[test]
    fn concat_comprehensive() {
        for lots in 0..QUADRATIC {
            let mut vec = Vector::new();
            for i in 0..lots {
                vec.push(i)
            }
            for i in 0..lots {
                let (mut a, mut b) = vec.split(i);
                let c = a.concat(&mut b);
                assert!(c == vec);
            }
        }
    }

    #[test]
    fn splice_middle() {
        let mut into = Vector::new();
        let mut splice_in = Vector::new();
        let mut reference = Vector::new();

        for i in 0..LOTS {
            if i < LOTS / 3 || i > (LOTS / 3 * 2) {
                into.push(i);
            } else {
                splice_in.push(i);
            }
            reference.push(i);
        }

        let spliced = into.splice(LOTS / 3, &mut splice_in);

        assert!(spliced == reference);
    }

    #[test]
    fn mutate() {
        let mut a = Vector::new();
        let mut b = Vector::new();

        for i in 0..LOTS {
            a.push(i);
            b.push(i + 1);
        }

        for i in 0..LOTS {
            b.get_mut(i).map(|mut v| *v -= 1);
        }

        assert!(a == b);
    }
}
