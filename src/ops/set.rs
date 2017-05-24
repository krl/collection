use std::hash::Hash;
use std::io;

use freezer::{Freeze, CryptoHash, Backend};

use collection::Collection;

use meta::{Meta, SubMeta};
use meta::max::Max;
use meta::checksum::CheckSum;

use tree::branch::{Branch, BranchResult};
use tree::node::Node;
use tree::level::{Beginning, End};
use tree::weight::Weight;

/// Set operations on a Collection
pub trait SetOps<T>
    where Self: Sized
{
    /// Insert element into set
    fn insert(&mut self, t: T) -> io::Result<()>;
    /// Remove element from set
    fn remove(&mut self, t: &T) -> io::Result<Option<T>>;
    /// Is element a member of this set?
    fn member(&self, t: &T) -> io::Result<bool>;
}

/// Set operations on Checksummed sets
pub trait SetOpsCheckSum<T>
    where Self: SetOps<T>
{
    /// Return a new Collection, that is the union of `self` and `b`
    fn union(&mut self, b: &mut Self) -> io::Result<Self>;
}

impl<T, M, H, B> SetOps<T> for Collection<T, M, H, B>
    where H: CryptoHash,
          T: Weight + Clone + Ord + Freeze<H>,
          M: Meta<T> + SubMeta<Max<T>> + Freeze<H>,
          B: Backend<Node<T, M, H>, H>
{
    fn member(&self, t: &T) -> io::Result<bool> {
        let mut search = Max::from_t(t);
        let branch =
            Branch::<_, _, Beginning, _, _>::new_full(self.root.clone(),
                                                      &mut search,
                                                      &self.freezer)?;
        match branch {
            BranchResult::Hit(_) => Ok(true),
            _ => Ok(false),
        }
    }

    fn insert(&mut self, t: T) -> io::Result<()> {
        let mut search = Max::from_t(&t);
        let branch =
            Branch::<_, _, Beginning, _, _>::new_full(self.root.clone(),
                                                      &mut search,
                                                      &self.freezer)?;
        match branch {
            BranchResult::Between(mut b) => {
                b.insert(t, self.divisor, &mut self.freezer)?;
                self.new_root(b.into_root())?;
            }
            // Already there
            BranchResult::Hit(_) => (),
            // At the very end
            BranchResult::Miss => {
                let mut branch: Branch<_, _, End, _, _> =
                    Branch::first(self.root.clone(), &self.freezer)?;
                branch.insert(t, self.divisor, &mut self.freezer)?;
                self.new_root(branch.into_root())?;
            }
        }
        Ok(())
    }

    fn remove(&mut self, t: &T) -> io::Result<Option<T>> {
        let mut search = Max::from_t(t);
        let branch =
            Branch::<_, _, Beginning, _, _>::new_full(self.root.clone(),
                                                      &mut search,
                                                      &self.freezer)?;
        match branch {
            BranchResult::Between(_) |
            BranchResult::Miss => Ok(None),
            BranchResult::Hit(mut b) => {
                let res = b.remove(self.divisor, &mut self.freezer)?;
                self.new_root(b.into_root())?;
                Ok(res)
            }
        }
    }
}

impl<T, M, H, B> SetOpsCheckSum<T> for Collection<T, M, H, B>
    where T: Hash + Weight + Clone + Ord + Freeze<H>,
          H: CryptoHash,
          M: SubMeta<CheckSum<u64>> + SubMeta<Max<T>> + Meta<T> + Freeze<H>,
          B: Backend<Node<T, M, H>, H>
{
    fn union(&mut self, b: &mut Self) -> io::Result<Self> {
        self.union_using::<Max<T>, CheckSum<u64>>(b)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use self::rand::Rng;
    use super::SetOps;
    use test_common::LOTS;

    use std::cmp::Ord;
    use std::hash::Hash;

    use meta::max::Max;
    use meta::checksum::CheckSum;
    use freezer::BlakeWrap;

    use collection::Collection;

    use super::SetOpsCheckSum;

    collection!(Set<T, BlakeWrap> {
        max: Max<T>,
        checksum: CheckSum<u64>,
    } where T: Ord + Hash);

    #[test]
    fn insert_one() {
        let mut set = Set::new(());
        set.insert(42).unwrap();
    }

    #[test]
    fn member() {
        let mut set = Set::new(());

        for i in 0..LOTS / 2 {
            set.insert(i * 2).unwrap();
        }

        for i in 0..LOTS {
            if i % 2 == 0 {
                assert!(set.member(&i).unwrap())
            } else {
                assert!(!set.member(&i).unwrap())
            }
        }
    }

    #[test]
    fn set_insert() {
        let mut set = Set::new(());

        let mut values = vec![];

        for i in 0..LOTS {
            values.push(i);
        }

        let mut r = rand::thread_rng();
        r.shuffle(&mut values);

        for i in 0..LOTS {
            set.insert(values[i]).unwrap();
        }

        let mut iter = set.iter();

        for i in 0..LOTS {
            let next = *iter.next().unwrap().unwrap();
            assert_eq!(next, i);
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn set_remove() {
        debug_assert!(LOTS % 2 == 0);

        let mut set = Set::new(());
        let mut even = Set::new(());
        let empty = Set::new(());

        for i in 0..LOTS {
            set.insert(i).unwrap();
            if i % 2 == 0 {
                even.insert(i).unwrap();
            }
        }

        for i in 0..LOTS / 2 {
            assert_eq!(set.remove(&(i * 2 + 1)).unwrap(), Some(i * 2 + 1));
        }

        assert!(set == even);

        for i in 0..LOTS / 2 {
            assert_eq!(set.remove(&(i * 2)).unwrap(), Some(i * 2));
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

        let mut a = Set::new(());
        let mut b = Set::new(());
        let mut c = Set::new(());

        for i in 0..vec_a.len() {
            a.insert(vec_a[i]).unwrap();
            b.insert(vec_b[i]).unwrap();
            c.insert(vec_c[i]).unwrap();
        }

        assert!(a == b);
        assert!(a == c);
    }

    #[test]
    fn set_clone_noninterleaving() {
        assert!(LOTS % 4 == 0);

        let mut count: usize = 0;

        let mut set_a = Set::new(());

        for _ in 0..LOTS / 4 {
            set_a.insert(count).unwrap();
            count += 1;
        }

        let mut set_b = set_a.clone();

        for _ in 0..LOTS / 4 {
            set_b.insert(count).unwrap();
            count += 1;
        }

        let mut set_c = set_b.clone();

        for _ in 0..LOTS / 4 {
            set_c.insert(count).unwrap();
            count += 1;
        }

        let mut set_d = set_c.clone();

        for _ in 0..LOTS / 4 {
            set_d.insert(count).unwrap();
            count += 1;
        }

        let mut iter_a = set_a.iter();
        let mut iter_b = set_b.iter();
        let mut iter_c = set_c.iter();
        let mut iter_d = set_d.iter();
        let mut itercount = 0;

        for _ in 0..LOTS / 4 {
            assert_eq!(*iter_a.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_b.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_c.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_d.next().unwrap().unwrap(), itercount);
            itercount += 1;
        }
        assert!(iter_a.next().is_none());

        for _ in 0..LOTS / 4 {
            assert_eq!(*iter_b.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_c.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_d.next().unwrap().unwrap(), itercount);
            itercount += 1;
        }
        assert!(iter_b.next().is_none());

        for _ in 0..LOTS / 4 {
            assert_eq!(*iter_c.next().unwrap().unwrap(), itercount);
            assert_eq!(*iter_d.next().unwrap().unwrap(), itercount);
            itercount += 1;
        }
        assert!(iter_c.next().is_none());

        for _ in 0..LOTS / 4 {
            assert_eq!(*iter_d.next().unwrap().unwrap(), itercount);
            itercount += 1;
        }
        assert!(iter_d.next().is_none());
    }

    #[test]
    fn set_clone_interleaving() {
        assert!(LOTS % 4 == 0);

        let mut set_a = Set::new(());

        for i in 0..LOTS / 4 {
            set_a.insert(i * 4).unwrap();
        }

        let mut set_b = set_a.clone();

        for i in 0..LOTS / 4 {
            set_b.insert(i * 4 + 1).unwrap();
        }

        let mut set_c = set_b.clone();

        for i in 0..LOTS / 4 {
            set_c.insert(i * 4 + 2).unwrap();
        }

        let mut set_d = set_c.clone();

        for i in 0..LOTS / 4 {
            set_d.insert(i * 4 + 3).unwrap();
        }

        let mut iter_a = set_a.iter();
        let mut iter_b = set_b.iter();
        let mut iter_c = set_c.iter();
        let mut iter_d = set_d.iter();

        for i in 0..LOTS {
            if i % 4 == 0 {
                assert_eq!(*iter_a.next().unwrap().unwrap(), i);
                assert_eq!(*iter_b.next().unwrap().unwrap(), i);
                assert_eq!(*iter_c.next().unwrap().unwrap(), i);
                assert_eq!(*iter_d.next().unwrap().unwrap(), i);
            } else if i % 4 == 1 {
                assert_eq!(*iter_b.next().unwrap().unwrap(), i);
                assert_eq!(*iter_c.next().unwrap().unwrap(), i);
                assert_eq!(*iter_d.next().unwrap().unwrap(), i);
            } else if i % 4 == 2 {
                assert_eq!(*iter_c.next().unwrap().unwrap(), i);
                assert_eq!(*iter_d.next().unwrap().unwrap(), i);
            } else if i % 4 == 3 {
                assert_eq!(*iter_d.next().unwrap().unwrap(), i);
            }
        }
        assert!(iter_a.next().is_none());
        assert!(iter_b.next().is_none());
        assert!(iter_c.next().is_none());
        assert!(iter_d.next().is_none());
    }

    #[test]
    fn partial_equal() {
        let mut set_a = Set::new(());
        let mut set_b = Set::new(());

        assert!(set_a == set_b);

        set_a.insert(1).unwrap();

        assert!(set_a != set_b);

        set_b.insert(1).unwrap();

        assert!(set_a == set_b);
    }

    #[test]
    fn union() {
        let mut a = Set::new(());
        let mut b = Set::new(());
        let mut r = Set::new(());

        for i in 0..LOTS {
            if (i % 10) < 5 {
                a.insert(i).unwrap();
            } else {
                b.insert(i).unwrap();
            }
            r.insert(i).unwrap();
        }

        let u = a.union(&mut b).unwrap();
        assert!(r == u);
    }

    #[test]
    fn union_overlapping() {
        let mut a = Set::new(());
        let mut b = Set::new(());
        let mut r = Set::new(());

        for i in 0..LOTS {
            if i < LOTS / 3 {
                a.insert(i).unwrap();
            } else if i < (LOTS * 2) / 3 {
                a.insert(i).unwrap();
                b.insert(i).unwrap();
            } else {
                b.insert(i).unwrap();
            }
            r.insert(i).unwrap();
        }

        let u = a.union(&mut b).unwrap();
        assert!(r == u);
    }

}

#[cfg(test)]
mod tests_disk {

    extern crate tempdir;
    use self::tempdir::TempDir;

    use super::SetOps;
    use test_common::LOTS;

    use std::cmp::Ord;
    use std::hash::Hash;

    use meta::max::Max;
    use meta::checksum::CheckSum;

    use collection::*;
    use freezer::BlakeWrap;
    use std::path::PathBuf;

    collection!(Set<T, BlakeWrap> {
        max: Max<T>,
        checksum: CheckSum<u64>,
    } where T: Ord + Hash);

    #[test]
    fn persist() {

        let tmp = TempDir::new("freezer_test").unwrap();
        let path = PathBuf::from(&tmp.path());

        let mut set = Set::<usize, PathBuf>::new(path.clone());
        let mut values = vec![];

        for i in 0..LOTS {
            values.push(i);
        }

        for i in 0..LOTS {
            set.insert(i).unwrap();
        }

        let hash = set.persist().unwrap();
        let restored = Set::<usize, PathBuf>::restore(hash, path).unwrap();

        assert!(set == restored);

        let mut iter = restored.iter();

        for i in 0..LOTS {
            let next = *iter.next().unwrap().unwrap();
            assert_eq!(next, i);
        }
        assert!(iter.next().is_none());
    }
}
