use std::io;
use std::borrow::Cow;

use freezer::{Freeze, CryptoHash, Backend};

use collection::Collection;

use meta::{Meta, SubMeta};
use meta::cardinality::Cardinality;

use tree::node::Node;
use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End};
use tree::weight::Weight;

/// Vector Operations on a Collection
pub trait VectorOps<T, M, H, B>
    where Self: Sized,
          T: Weight + Freeze<H>,
          M: Meta<T> + Clone,
          B: Backend<Node<T, M, H>, H>,
          H: CryptoHash
{
    /// Insert element at index i
    fn insert(&mut self, i: usize, t: T) -> io::Result<()>;
    /// Remove element from index i
    fn remove(&mut self, i: usize) -> io::Result<Option<T>>;
    /// Get a reference to element at index i
    fn get(&self, i: usize) -> io::Result<Option<Cow<T>>>;
    /// Push element to end of vector
    fn push(&mut self, t: T) -> io::Result<()>;
    /// Pop from the end of the vector
    fn pop(&mut self) -> io::Result<Option<T>>;
    /// Split the vector in two at index i
    fn split(&mut self, i: usize) -> io::Result<(Self, Self)>;
    /// Concatenate two vectors
    fn concat(&mut self, b: &mut Self) -> io::Result<Self>;
    /// Splice in a vector at index i
    fn splice(&mut self, i: usize, from: &mut Self) -> io::Result<Self>;
}

impl<T, M, H, B> VectorOps<T, M, H, B> for Collection<T, M, H, B>
    where T: Weight + Freeze<H>,
          M: Meta<T> + SubMeta<Cardinality<usize>> + Clone,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>
{
    fn get(&self, i: usize) -> io::Result<Option<Cow<T>>> {
        let mut state = Cardinality::new(&i);

        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut state, &self.freezer)?;

        match res {
            BranchResult::Hit(branch) => branch.leaf(&self.freezer),
            _ => Ok(None),
        }
    }

    fn insert(&mut self, i: usize, t: T) -> io::Result<()> {
        let mut key = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut key, &self.freezer)?;
        match res {
            // empty case
            BranchResult::Between(mut branch) => {
                if i == 0 {
                    branch.insert(t, self.divisor, &mut self.freezer)?;
                    self.new_root(branch.into_root())?;
                } else {
                    panic!("Insert past length of collection");
                }
            }
            BranchResult::Hit(mut branch) => {
                branch.insert(t, self.divisor, &mut self.freezer)?;
                self.new_root(branch.into_root())?;
            }
            // non-empty case
            BranchResult::Miss => {
                if *key.inner() == 0 {
                    let mut branch: Branch<_, _, End, _, _> =
                        Branch::first(self.root.clone(), &self.freezer)?;
                    branch.insert(t, self.divisor, &mut self.freezer)?;
                    self.new_root(branch.into_root())?;
                } else {
                    panic!("Insert past length of collection");
                }
            }
        }
        Ok(())
    }

    fn remove(&mut self, i: usize) -> io::Result<Option<T>> {
        let mut key = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut key, &self.freezer)?;
        match res {
            BranchResult::Hit(mut branch) => {
                let ret = branch.remove(self.divisor, &mut self.freezer);
                self.new_root(branch.into_root())?;
                ret
            }
            BranchResult::Miss |
            BranchResult::Between(_) => Ok(None),

        }
    }

    fn push(&mut self, t: T) -> io::Result<()> {
        let mut branch: Branch<_, _, End, _, _> =
            Branch::first(self.root.clone(), &self.freezer)?;
        branch.insert(t, self.divisor, &mut self.freezer)?;
        self.new_root(branch.into_root())?;
        Ok(())
    }

    fn pop(&mut self) -> io::Result<Option<T>> {
        let mut branch: Branch<_, _, End, _, _> =
            Branch::first(self.root.clone(), &self.freezer)?;
        let ret = branch.remove(self.divisor, &mut self.freezer);
        self.new_root(branch.into_root())?;
        ret
    }

    fn split(&mut self, i: usize) -> io::Result<(Self, Self)>
        where Self: Sized
    {
        // first clone the freezer, so that all changes will
        // be kept out of the original.
        let mut freezer = self.freezer.clone();

        if i == 0 {
            return Ok((Collection::with_freezer(freezer), self.clone()));
        }

        let mut state = Cardinality::new(&i);
        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut state, &freezer)?;

        match res {
            BranchResult::Hit(branch) => {
                let a = branch.left(&mut freezer)?;
                let b = branch.right(&mut freezer)?;
                Ok((Collection::new_from(a.into_root(), freezer.clone())?,
                    Collection::new_from(b.into_root(), freezer)?))
            }
            BranchResult::Miss => {
                Ok((self.clone(), Collection::with_freezer(freezer)))
            }
            _ => unreachable!(),
        }
    }

    fn concat(&mut self, b: &mut Self) -> io::Result<Self> {
        self.freezer.merge(&mut b.freezer);

        let a_branch: Branch<_, _, End, _, _> =
            Branch::first(self.root.clone(), &self.freezer)?;
        let b_branch: Branch<_, _, Beginning, _, _> =
            Branch::first(b.root.clone(), &self.freezer)?;

        let branch =
            a_branch.concat(&b_branch, self.divisor, &mut self.freezer)?;

        Ok(Collection::new_from(branch.into_root(), self.freezer.clone())?)
    }

    fn splice(&mut self, i: usize, from: &mut Self) -> io::Result<Self> {
        let (mut first, mut second) = self.split(i)?;
        first.concat(&mut from.clone())?.concat(&mut second)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use test_common::LOTS;
    use test_common::LESS;

    const SPLITS: usize = 40;

    use meta::cardinality::Cardinality;
    use meta::checksum::CheckSum;
    use collection::Collection;
    use super::VectorOps;
    use freezer::VoidHash;

    use std::hash::Hash;

    collection!(Vector<T, VoidHash> {
        cardinality: Cardinality<usize>,
        checksum: CheckSum<u64>,
    } where T: Hash);

    #[test]
    fn insert() {
        let mut a = Vector::new(());
        let mut b = Vector::new(());

        for i in 0..LOTS {
            a.push(i).unwrap();
        }

        for i in 0..LOTS {
            b.insert(0, LOTS - i - 1).unwrap();
        }

        assert!(a == b);
    }

    #[test]
    fn indexing() {
        let mut vec = Vector::new(());

        for i in 0..LOTS {
            vec.push(i).unwrap();
        }

        for i in 0..LOTS {
            let got = *vec.get(i).unwrap().unwrap();
            assert_eq!(got, i);
        }
        assert!(vec.get(LOTS).unwrap().is_none());
    }

    #[test]
    fn remove() {
        debug_assert!(LOTS % 2 == 0);

        let mut vec = Vector::new(());
        let mut even = Vector::new(());
        let empty = Vector::new(());

        for i in 0..LOTS {
            vec.push(i).unwrap();
            if i % 2 == 0 {
                even.push(i).unwrap();
            }
        }

        for i in 0..LOTS / 2 {
            assert_eq!(vec.remove(i + 1).unwrap(), Some(i * 2 + 1));
        }

        assert!(vec == even);

        for i in 0..LOTS / 2 {
            assert_eq!(vec.remove(0).unwrap(), Some(i * 2));
        }

        assert!(vec == empty);

        assert_eq!(vec.remove(0).unwrap(), None)
    }

    #[test]
    fn remove_from_back() {
        let mut vec = Vector::new(());
        let empty = Vector::new(());

        for i in 0..LOTS {
            vec.push(i).unwrap();
        }

        for i in 0..LOTS {
            let i = LOTS - i - 1;
            assert_eq!(vec.remove(i).unwrap(), Some(i));
        }

        assert!(vec == empty);

        assert_eq!(vec.remove(0).unwrap(), None)
    }

    #[test]
    #[should_panic]
    fn insert_panic() {
        let mut vec = Vector::new(());
        vec.insert(1, 1).unwrap();
    }

    #[test]
    fn partial_equal() {
        let mut vec_a = Vector::new(());
        let mut vec_b = Vector::new(());

        assert!(vec_a == vec_b);

        vec_a.push(1).unwrap();

        assert!(vec_a != vec_b);

        vec_b.push(1).unwrap();

        assert!(vec_a == vec_b);
    }

    #[test]
    fn partial_equal_ordering() {
        let mut vec_a = Vector::new(());
        let mut vec_b = Vector::new(());

        assert!(vec_a == vec_b);

        vec_a.push(1).unwrap();
        vec_a.push(2).unwrap();

        vec_b.push(2).unwrap();
        vec_b.push(1).unwrap();

        assert!(vec_a != vec_b);
    }

    #[test]
    fn split() {

        let mut split_points = vec![1];

        for i in 0..SPLITS {
            split_points.push((i * LOTS) / SPLITS);
        }
        split_points.push(LOTS - 1);
        split_points.push(LOTS);

        for i in split_points {
            // to avoid unnecessary cloning, we re-initialize vec each round
            let mut vec = Vector::new(());
            for i in 0..LOTS {
                vec.push(i).unwrap()
            }

            println!("i: {}", i);

            let (a, b) = vec.split(i).unwrap();

            let mut iter_a = a.iter();
            let mut iter_b = b.iter();

            for o in 0..i {
                assert_eq!(*iter_a.next().unwrap().unwrap(), o)
            }
            assert!(iter_a.next().is_none());

            for o in i..LOTS {
                assert_eq!(*iter_b.next().unwrap().unwrap(), o)
            }
            assert!(iter_b.next().is_none());
        }
    }

    #[test]
    fn concat() {
        let mut vec = Vector::new(());

        for i in 0..LOTS {
            vec.push(i).unwrap()
        }

        let mut split_points = vec![1];

        for i in 0..100 {
            split_points.push((LOTS / 100) * i);
        }

        split_points.push(LOTS - 1);
        split_points.push(LOTS);

        for i in split_points {
            let (mut a, mut b) = vec.split(i).unwrap();

            let c = a.concat(&mut b).unwrap();
            assert!(c == vec);
        }
    }

    #[test]
    fn concat_comprehensive() {
        for less in 0..LESS {
            let mut vec = Vector::new(());
            for i in 0..less {
                vec.push(i).unwrap()
            }
            for i in 0..less {
                if i == 1 {
                    println!("i: {}", i);
                }
                let (mut a, mut b) = vec.split(i).unwrap();
                let c = a.concat(&mut b).unwrap();
                assert!(c == vec);
            }
        }
    }

    #[test]
    fn splice_middle() {
        let mut into = Vector::new(());
        let mut splice_in = Vector::new(());
        let mut reference = Vector::new(());

        for i in 0..LOTS {
            if i < LOTS / 3 || i > (LOTS / 3 * 2) {
                into.push(i).unwrap();
            } else {
                splice_in.push(i).unwrap();
            }
            reference.push(i).unwrap();
        }

        let spliced = into.splice(LOTS / 3, &mut splice_in).unwrap();

        assert!(spliced == reference);
    }
}
