use Val;

use std::hash::Hash;

use collection::Collection;

use meta::{Meta, SubMeta};
use meta::max::Max;
use meta::checksum::CheckSum;

use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End};


/// Set operations on a Collection
pub trait SetOps<T>
    where Self: Sized
{
    /// Insert element into set
    fn insert(&mut self, t: T);
    /// Remove element from set
    fn remove(&mut self, t: &T) -> Option<T>;
    /// Is element a member of this set?
    fn member(&self, t: &T) -> bool;
}

/// Set operations on Checksummed sets
pub trait SetOpsCheckSum<T>
    where Self: SetOps<T>
{
    /// Return a new Collection, that is the union of `self` and `b`
    fn union(&mut self, b: &mut Self) -> Self;
}

impl<T, M> SetOps<T> for Collection<T, M>
    where T: Val + Ord,
          M: Meta<T> + SubMeta<Max<T>>
{
    fn member(&self, t: &T) -> bool {
        let mut search = Max::from_t(t);
        let branch = Branch::<_, _, Beginning>::new_full(self.root,
                                                         &mut search,
                                                         &self.stash);
        match branch {
            BranchResult::Hit(_) => true,
            _ => false,
        }
    }

    fn insert(&mut self, t: T) {
        let mut search = Max::from_t(&t);
        let branch = Branch::<_, _, Beginning>::new_full(self.root,
                                                         &mut search,
                                                         &self.stash);
        match branch {
            BranchResult::Between(mut b) => {
                b.insert(t, self.divisor, &mut self.stash);
                self.root = b.root();
            }
            // Already there
            BranchResult::Hit(_) => return,
            // At the very end
            BranchResult::Miss => {
                let mut branch: Branch<_, _, End> = Branch::first(self.root,
                                                                  &self.stash);
                branch.insert(t, self.divisor, &mut self.stash);
                self.root = branch.root();
            }
        }
    }

    fn remove(&mut self, t: &T) -> Option<T> {
        let mut search = Max::from_t(t);
        let branch = Branch::<_, _, Beginning>::new_full(self.root,
                                                         &mut search,
                                                         &self.stash);
        match branch {
            BranchResult::Between(_) |
            BranchResult::Miss => None,
            BranchResult::Hit(mut b) => {
                let res = b.remove(self.divisor, &mut self.stash);
                self.root = b.root();
                res
            }
        }
    }
}

impl<T, M> SetOpsCheckSum<T> for Collection<T, M>
    where T: Val + Ord + Hash,
          M: Meta<T> + SubMeta<Max<T>> + SubMeta<CheckSum<u64>>
{
    fn union(&mut self, b: &mut Self) -> Self
        where M: SubMeta<CheckSum<u64>> + SubMeta<Max<T>>
    {
        self.union_using::<Max<T>, CheckSum<u64>>(b)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use self::rand::Rng;
    use super::SetOps;
    const LOTS: usize = 100_000;

    use std::cmp::Ord;
    use std::hash::Hash;

    use meta::max::Max;
    use meta::checksum::CheckSum;

    use collection::Collection;

    use super::SetOpsCheckSum;

    collection!(Set<T> {
        max: Max<T>,
        checksum: CheckSum<u64>,
    } where T: Ord + Hash);

    #[test]
    fn insert_one() {
        let mut set = Set::new();
        set.insert(42);
    }

    #[test]
    fn member() {
        let mut set = Set::new();

        for i in 0..LOTS / 2 {
            set.insert(i * 2);
        }

        for i in 0..LOTS {
            if i % 2 == 0 {
                assert!(set.member(&i))
            } else {
                assert!(!set.member(&i))
            }
        }
    }

    #[test]
    fn set_insert() {

        type T = i32;

        let mut set = Set::<T>::new();

        let mut values = vec![];

        for i in 0..LOTS {
            values.push(i as T);
        }

        let mut r = rand::thread_rng();
        r.shuffle(&mut values);

        for i in 0..LOTS {
            set.insert(values[i]);
        }

        let mut iter = set.iter();

        for i in 0..LOTS {
            let next = iter.next();
            assert_eq!(next, Some(&(i as T)));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn set_remove() {
        debug_assert!(LOTS % 2 == 0);

        let mut set = Set::new();
        let mut even = Set::new();
        let empty = Set::new();

        for i in 0..LOTS {
            set.insert(i);
            if i % 2 == 0 {
                even.insert(i);
            }
        }

        for i in 0..LOTS / 2 {
            assert_eq!(set.remove(&(i * 2 + 1)), Some(i * 2 + 1));
        }

        assert!(set == even);

        for i in 0..LOTS / 2 {
            assert_eq!(set.remove(&(i * 2)), Some(i * 2));
        }

        assert!(set == empty);
    }

    #[test]
    fn set_equal() {
        let mut vec_a = vec![];

        for i in 0..LOTS {
            vec_a.push(i);
        }

        let mut vec_b = vec_a.clone();
        vec_b.reverse();

        let mut vec_c = vec_a.clone();
        rand::thread_rng().shuffle(&mut vec_c);

        let mut a = Set::new();
        let mut b = Set::new();
        let mut c = Set::new();

        for i in 0..vec_a.len() {
            a.insert(vec_a[i]);
            b.insert(vec_b[i]);
            c.insert(vec_c[i]);
        }

        assert!(a == b);
        assert!(a == c);
    }

    #[test]
    fn set_clone_noninterleaving() {
        assert!(LOTS % 4 == 0);

        let mut count: usize = 0;

        let mut set_a = Set::new();

        for _ in 0..LOTS / 4 {
            set_a.insert(count);
            count += 1;
        }

        let mut set_b = set_a.clone_mut();

        for _ in 0..LOTS / 4 {
            set_b.insert(count);
            count += 1;
        }

        let mut set_c = set_b.clone_mut();

        for _ in 0..LOTS / 4 {
            set_c.insert(count);
            count += 1;
        }

        let mut set_d = set_c.clone_mut();

        for _ in 0..LOTS / 4 {
            set_d.insert(count);
            count += 1;
        }

        let mut iter_a = set_a.iter();
        let mut iter_b = set_b.iter();
        let mut iter_c = set_c.iter();
        let mut iter_d = set_d.iter();
        let mut itercount = 0;

        for _ in 0..LOTS / 4 {
            assert_eq!(iter_a.next(), Some(&itercount));
            assert_eq!(iter_b.next(), Some(&itercount));
            assert_eq!(iter_c.next(), Some(&itercount));
            assert_eq!(iter_d.next(), Some(&itercount));
            itercount += 1;
        }
        assert_eq!(iter_a.next(), None);

        for _ in 0..LOTS / 4 {
            assert_eq!(iter_b.next(), Some(&itercount));
            assert_eq!(iter_c.next(), Some(&itercount));
            assert_eq!(iter_d.next(), Some(&itercount));
            itercount += 1;
        }
        assert_eq!(iter_b.next(), None);

        for _ in 0..LOTS / 4 {
            assert_eq!(iter_c.next(), Some(&itercount));
            assert_eq!(iter_d.next(), Some(&itercount));
            itercount += 1;
        }
        assert_eq!(iter_c.next(), None);

        for _ in 0..LOTS / 4 {
            assert_eq!(iter_d.next(), Some(&itercount));
            itercount += 1;
        }
        assert_eq!(iter_d.next(), None);
    }

    #[test]
    fn set_clone_interleaving() {
        assert!(LOTS % 4 == 0);

        let mut set_a = Set::new();

        for i in 0..LOTS / 4 {
            set_a.insert(i * 4);
        }

        let mut set_b = set_a.clone_mut();

        for i in 0..LOTS / 4 {
            set_b.insert(i * 4 + 1);
        }

        let mut set_c = set_b.clone_mut();

        for i in 0..LOTS / 4 {
            set_c.insert(i * 4 + 2);
        }

        let mut set_d = set_c.clone_mut();

        for i in 0..LOTS / 4 {
            set_d.insert(i * 4 + 3);
        }

        let mut iter_a = set_a.iter();
        let mut iter_b = set_b.iter();
        let mut iter_c = set_c.iter();
        let mut iter_d = set_d.iter();

        for i in 0..LOTS {
            if i % 4 == 0 {
                assert_eq!(iter_a.next(), Some(&i));
                assert_eq!(iter_b.next(), Some(&i));
                assert_eq!(iter_c.next(), Some(&i));
                assert_eq!(iter_d.next(), Some(&i));
            } else if i % 4 == 1 {
                assert_eq!(iter_b.next(), Some(&i));
                assert_eq!(iter_c.next(), Some(&i));
                assert_eq!(iter_d.next(), Some(&i));
            } else if i % 4 == 2 {
                assert_eq!(iter_c.next(), Some(&i));
                assert_eq!(iter_d.next(), Some(&i));
            } else if i % 4 == 3 {
                assert_eq!(iter_d.next(), Some(&i));
            }
        }
        assert_eq!(iter_a.next(), None);
        assert_eq!(iter_b.next(), None);
        assert_eq!(iter_c.next(), None);
        assert_eq!(iter_d.next(), None);
    }

    #[test]
    fn partial_equal() {
        let mut set_a = Set::new();
        let mut set_b = Set::new();

        assert!(set_a == set_b);

        set_a.insert("a");

        assert!(set_a != set_b);

        set_b.insert("a");

        assert!(set_a == set_b);
    }

    #[test]
    fn union() {
        let mut a = Set::new();
        let mut b = Set::new();
        let mut r = Set::new();

        for i in 0..LOTS {
            if (i % 10) < 5 {
                a.insert(i);
            } else {
                b.insert(i);
            }
            r.insert(i);
        }

        let u = a.union(&mut b);
        assert!(r == u)
    }

    #[test]
    fn union_overlapping() {
        let mut a = Set::new();
        let mut b = Set::new();
        let mut r = Set::new();

        for i in 0..LOTS {
            if i < LOTS / 3 {
                a.insert(i);
            } else if i < (LOTS * 2) / 3 {
                a.insert(i);
                b.insert(i);
            } else {
                b.insert(i);
            }
            r.insert(i);
        }

        let u = a.union(&mut b);
        assert!(r == u)
    }
}
