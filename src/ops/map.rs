use std::hash::Hash;
use std::cmp::Ord;
use std::io;
use std::borrow::Cow;

use freezer::{CryptoHash, Sink, Source, Backend, Freeze};

use collection::Collection;

use meta::{Meta, SubMeta};
use meta::key::{Key, KeySum, Keyed};

use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End};
use tree::weight::Weight;
use tree::node::{Node, Child};

/// A Key-Value pair
#[derive(Clone, Debug)]
pub struct KV<K, V> {
    k: K,
    v: V,
}

impl<K, V> Keyed for KV<K, V>
    where K: Ord + Clone + Hash
{
    type Key = K;
    type Value = V;

    fn new(k: Self::Key, v: Self::Value) -> Self {
        KV { k, v }
    }

    fn key(&self) -> &Self::Key {
        &self.k
    }

    fn val(&self) -> &Self::Value {
        &self.v
    }

    fn val_mut(&mut self) -> &mut Self::Value {
        &mut self.v
    }

    fn into_val(self) -> Self::Value {
        self.v
    }
}

impl<K, V> Weight for KV<K, V>
    where K: Weight + Ord + PartialEq,
          V: Clone
{
    fn weight_hash(&self) -> u64 {
        self.k.weight_hash()
    }
}

impl<K, V, H> Freeze<H> for KV<K, V>
    where K: Ord + Freeze<H>,
          V: Freeze<H>,
          H: CryptoHash
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.k.freeze(into)?;
        self.v.freeze(into)
    }

    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(KV {
               k: K::thaw(from)?,
               v: V::thaw(from)?,
           })
    }
}

/// Map operations on a Collection
pub trait MapOps<K, V, M>
    where Self: Sized,
          M: Meta<KV<K, V>>,
          KV<K, V>: Keyed,
          <KV<K, V> as Keyed>::Key: Weight + Clone + Ord,
          <KV<K, V> as Keyed>::Value: Clone
{
    /// Insert a value `V` at key `K`
    fn insert(&mut self,
              key: <KV<K, V> as Keyed>::Key,
              val: <KV<K, V> as Keyed>::Value)
              -> io::Result<()>;
    /// Remove value at key `K`
    fn remove(&mut self,
              key: <KV<K, V> as Keyed>::Key)
              -> io::Result<Option<<KV<K, V> as Keyed>::Value>>;
    /// Get a reference to the value at key `K`
    fn get(&self,
           key: <KV<K, V> as Keyed>::Key)
           -> io::Result<Option<Cow<<KV<K, V> as Keyed>::Value>>>;
    /// Mutate the value at key `K` with function F
    fn mutate<F>(&mut self,
                 key: <KV<K, V> as Keyed>::Key,
                 f: F)
                 -> io::Result<Option<()>>
        where F: FnOnce(&mut <KV<K, V> as Keyed>::Value);
}

/// Operations on a map with `KeySum` metadata
pub trait MapOpsKeySum<K, V, M>
    where Self: MapOps<K, V, M>,
          M: Meta<KV<K, V>>,
          KV<K, V>: Keyed,
          <KV<K, V> as Keyed>::Key: Weight + Clone + Ord,
          <KV<K, V> as Keyed>::Value: Clone
{
    /// Merge two maps, overwriting values from `self` with `b`
    fn merge(&mut self, b: &mut Self) -> io::Result<Self>;
}

impl<K, V, M, H, B> MapOps<K, V, M> for Collection<KV<K, V>, M, H, B>
    where H: CryptoHash,
          M: Meta<KV<K, V>> + SubMeta<Key<<KV<K, V> as Keyed>::Key>>,
          M: Freeze<H>,
          KV<K, V>: Keyed + Weight + Freeze<H>,
          <KV<K, V> as Keyed>::Key: Weight + Hash + Ord + Clone + Freeze<H>,
          <KV<K, V> as Keyed>::Value: Clone + Freeze<H>,
          B: Backend<Node<KV<K, V>, M, H>, H>
{
    fn insert(&mut self,
              key: <KV<K, V> as Keyed>::Key,
              val: <KV<K, V> as Keyed>::Value)
              -> io::Result<()> {
        let mut search = Key::new(key.clone());

        let branch =
            Branch::<_, _, Beginning, _, _>::new_full(self.root.clone(),
                                                      &mut search,
                                                      &self.freezer)?;
        match branch {
            BranchResult::Between(mut b) => {
                b.insert(KV::new(key, val), self.divisor, &mut self.freezer)?;
                self.new_root(b.into_root())?;
            }
            // Already there, overwrite
            BranchResult::Hit(mut b) => {
                b.update(KV::new(key, val), &mut self.freezer)?;
                self.new_root(b.into_root())?;
            }
            // At the very end
            BranchResult::Miss => {
                let mut branch: Branch<_, _, End, _, _> =
                    Branch::first(self.root.clone(), &self.freezer)?;
                branch.insert(KV::new(key, val),
                              self.divisor,
                              &mut self.freezer)?;
                self.new_root(branch.into_root())?;
            }
        }
        Ok(())
    }

    fn remove(&mut self,
              key: <KV<K, V> as Keyed>::Key)
              -> io::Result<Option<<KV<K, V> as Keyed>::Value>> {
        let mut key = Key::new(key);

        let branch =
            Branch::<_, _, Beginning, _, _>::new_full(self.root.clone(),
                                                      &mut key,
                                                      &self.freezer)?;
        match branch {
            BranchResult::Between(_) |
            BranchResult::Miss => Ok(None),
            BranchResult::Hit(mut b) => {
                let res = b.remove(self.divisor, &mut self.freezer);
                self.new_root(b.into_root())?;
                Ok(res?.map(|kv| kv.into_val()))
            }
        }
    }

    fn get(&self,
           key: <KV<K, V> as Keyed>::Key)
           -> io::Result<Option<Cow<<KV<K, V> as Keyed>::Value>>> {
        let mut key = Key::new(key);
        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut key, &self.freezer)?;

        match res {
            BranchResult::Hit(branch) => {
                Ok(branch.leaf(&self.freezer)?.map(|l| match l {
                    Cow::Owned(leaf) => Cow::Owned(leaf.into_val()),
                    Cow::Borrowed(leaf) => Cow::Borrowed(leaf.val()),
                }))
            }
            _ => Ok(None),
        }
    }

    fn mutate<F>(&mut self,
                 key: <KV<K, V> as Keyed>::Key,
                 f: F)
                 -> io::Result<Option<()>>
        where F: FnOnce(&mut <KV<K, V> as Keyed>::Value)
    {
        let mut key = Key::new(key);
        let res: BranchResult<_, _, Beginning, _, _> =
            Branch::new_full(self.root.clone(), &mut key, &self.freezer)?;

        if let BranchResult::Hit(mut branch) = res {
            {
                let mut kv = branch.leaf_mut(&mut self.freezer)?;
                match *kv {
                    Child::Leaf(ref mut t) => {
                        f(t.val_mut());
                    }
                    _ => panic!("not a leaf"),
                }
            }
            branch.propagate(&mut self.freezer)?;
            self.new_root(branch.into_root())?;
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }
}

impl<K, V, M, H, B> MapOpsKeySum<K, V, M> for Collection<KV<K, V>, M, H, B>
    where H: CryptoHash,
          M: Meta<KV<K, V>> + SubMeta<Key<<KV<K, V> as Keyed>::Key>>,
          M: SubMeta<KeySum<u64>> + Freeze<H>,
          KV<K, V>: Weight + Keyed + Freeze<H>,
          <KV<K, V> as Keyed>::Key: Hash + Ord + Clone + Freeze<H>,
          <KV<K, V> as Keyed>::Value: Clone + Freeze<H>,
          B: Backend<Node<KV<K, V>, M, H>, H>
{
    fn merge(&mut self, b: &mut Self) -> io::Result<Self> {
        self.union_using::<Key<<KV<K, V> as Keyed>::Key>, KeySum<u64>>(b)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use test_common::LOTS;

    use std::hash::Hash;

    use meta::key::{Key, Keyed, KeySum, ValSum};
    use freezer::BlakeWrap;

    use collection::Collection;

    use super::MapOps;
    use super::MapOpsKeySum;

    collection!(Map<T, BlakeWrap> {
        key: Key<T::Key>,
        keysum: KeySum<u64>,
        valsum: ValSum<u64>,
    } where T: Keyed,
            T::Key: Hash,
            T::Value: Hash);

    #[test]
    fn insert() {
        let mut map = Map::new(());
        map.insert(1, 1).unwrap();
        assert_eq!(*map.get(1).unwrap().unwrap(), 1);
    }

    #[test]
    fn partial_eq() {
        let mut a = Map::new(());
        let mut b = Map::new(());

        for i in 0..LOTS {
            a.insert(i, i + 1).unwrap();
            b.insert(LOTS - i - 1, LOTS - i - 1).unwrap();
        }

        // mutate in a
        for i in 0..LOTS {
            a.mutate(i, |val| *val -= 1).unwrap();
        }

        assert!(a == b);
    }

    #[test]
    fn overwrite() {
        let mut map = Map::new(());

        map.insert(1, 1).unwrap();
        assert_eq!(*map.get(1).unwrap().unwrap(), 1);
        map.insert(1, 2).unwrap();
        assert_eq!(*map.get(1).unwrap().unwrap(), 2);
    }

    #[test]
    fn clone() {
        let mut a = Map::new(());

        a.insert(1, 1).unwrap();

        let mut b = a.clone();

        b.insert(1, 2).unwrap();

        assert_eq!(*a.get(1).unwrap().unwrap(), 1);
        assert_eq!(*b.get(1).unwrap().unwrap(), 2);
    }

    #[test]
    fn merge() {
        let mut a = Map::new(());
        let mut b = Map::new(());

        a.insert(1, 1).unwrap();
        b.insert(1, 1).unwrap();

        a.insert(2, 2).unwrap();
        b.insert(2, 3).unwrap();

        b.insert(3, 4).unwrap();

        let am = a.merge(&mut b).unwrap();
        let bm = b.merge(&mut a).unwrap();

        assert_eq!(*am.get(1).unwrap().unwrap(), 1);
        assert_eq!(*am.get(2).unwrap().unwrap(), 3);
        assert_eq!(*am.get(3).unwrap().unwrap(), 4);

        assert_eq!(*bm.get(1).unwrap().unwrap(), 1);
        assert_eq!(*bm.get(2).unwrap().unwrap(), 2);
        assert_eq!(*bm.get(3).unwrap().unwrap(), 4);
    }

    #[test]
    fn nesting() {
        let mut a = Map::new(());
        let mut b = Map::new(());

        b.insert(0, 0).unwrap();
        a.insert(0, b).unwrap();

        assert_eq!(*a.get(0)
                   .unwrap()
                   .unwrap()
                   .get(0)
                   .unwrap()
                   .unwrap(),
                   0)
    }
}
