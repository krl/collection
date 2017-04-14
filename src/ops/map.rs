use std::hash::Hash;
use std::cmp::Ord;
use std::ops::{Deref, DerefMut};

use Val;

use collection::{Collection, MutContext};

use meta::{Meta, SubMeta};
use meta::key::{Key, KeySum, Keyed};

use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End, Relative};
use tree::weight::Weight;

/// A Key-Value pair
#[derive(Clone, Debug)]
pub struct KV<K, V>
    where K: Val + Ord + PartialEq,
          V: Clone
{
    k: K,
    v: V,
}

impl<K, V> KV<K, V>
    where K: Val + Ord + PartialEq,
          V: Clone
{
    fn new(k: K, v: V) -> Self {
        KV { k: k, v: v }
    }
    fn val(&self) -> &V {
        &self.v
    }
    fn into_val(self) -> V {
        self.v
    }
}

impl<K, V> Keyed for KV<K, V>
    where K: Val + Ord + PartialEq,
          V: Clone
{
    type Key = K;
    type Value = V;
    fn key(&self) -> &K {
        &self.k
    }
    fn value(&self) -> &V {
        &self.v
    }
    fn value_mut(&mut self) -> &mut V {
        &mut self.v
    }
}

impl<K, V> Weight for KV<K, V>
    where K: Val + Ord + PartialEq,
          V: Clone
{
    fn weight_hash(&self) -> u64 {
        self.k.weight_hash()
    }
}

pub struct ValContext<'a, T, M, R>
    where T: 'a + Val + Keyed,
          M: 'a + Meta<T>,
          R: Relative
{
    context: MutContext<'a, T, M, R>,
}

impl<'a, T, M, R> ValContext<'a, T, M, R>
    where T: 'a + Val + Keyed,
          M: 'a + Meta<T>,
          R: Relative
{
    pub fn new(context: MutContext<'a, T, M, R>) -> Self {
        ValContext { context: context }
    }
}

impl<'a, T, M, R> Deref for ValContext<'a, T, M, R>
    where T: 'a + Val + Keyed,
          M: 'a + Meta<T>,
          R: Relative
{
    type Target = T::Value;
    fn deref(&self) -> &Self::Target {
        (*self.context).value()
    }
}

impl<'a, T, M, R> DerefMut for ValContext<'a, T, M, R>
    where T: 'a + Val + Keyed,
          M: 'a + Meta<T>,
          R: Relative
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.context.deref_mut().value_mut()
    }
}

/// Map operations on a Collection
pub trait MapOps<K, V, M>
    where Self: Sized,
          M: Meta<KV<K, V>>,
          K: Val + Ord,
          V: Clone
{
    /// Insert a value `V` at key `K`
    fn insert(&mut self, key: K, V);
    /// Remove value at key `K`
    fn remove(&mut self, key: K) -> Option<V>;
    /// Get a reference to the value at key `K`
    fn get(&self, key: K) -> Option<&V>;
    /// Get a mutable reference to the value at key `K`
    fn get_mut(&mut self,
               key: K)
               -> Option<ValContext<KV<K, V>, M, Beginning>>;
}

/// Operations on a map with `KeySum` metadata
pub trait MapOpsKeySum<K, V, M>
    where Self: MapOps<K, V, M>,
          M: Meta<KV<K, V>>,
          K: Val + Ord,
          V: Clone
{
    /// Merge two maps, overwriting values from `self` with `b`
    fn merge(&mut self, b: &mut Self) -> Self;
}

impl<K, V, M> MapOps<K, V, M> for Collection<KV<K, V>, M>
    where M: Meta<KV<K, V>> + SubMeta<Key<K>>,
          K: Val + Ord,
          V: Clone
{
    fn insert(&mut self, key: K, val: V) {
        let mut search = Key::new(key.clone());
        let branch = Branch::<_, _, Beginning>::new_full(self.root,
                                                         &mut search,
                                                         &self.stash);
        match branch {
            BranchResult::Between(mut b) => {
                b.insert(KV::new(key, val), self.divisor, &mut self.stash);
                self.root = b.root();
            }
            // Already there, overwrite
            BranchResult::Hit(mut b) => {
                b.update(KV::new(key, val), &mut self.stash);
                self.root = b.root();
            }
            // At the very end
            BranchResult::Miss => {
                let mut branch: Branch<_, _, End> = Branch::first(self.root,
                                                                  &self.stash);
                branch.insert(KV::new(key, val), self.divisor, &mut self.stash);
                self.root = branch.root();
            }
        }
    }

    fn remove(&mut self, key: K) -> Option<V> {
        let mut key = Key::new(key);

        let branch = Branch::<_, _, Beginning>::new_full(self.root,
                                                         &mut key,
                                                         &self.stash);
        match branch {
            BranchResult::Between(_) |
            BranchResult::Miss => None,
            BranchResult::Hit(mut b) => {
                let res = b.remove(self.divisor, &mut self.stash);
                self.root = b.root();
                res.map(|kv| kv.into_val())
            }
        }
    }

    fn get(&self, key: K) -> Option<&V> {
        let mut key = Key::new(key);
        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut key, &self.stash);

        match res {
            BranchResult::Hit(branch) => {
                branch.leaf(&self.stash).map(|l| l.val())
            }
            _ => None,
        }
    }

    fn get_mut(&mut self,
               key: K)
               -> Option<ValContext<KV<K, V>, M, Beginning>> {
        let mut key = Key::new(key);
        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut key, &self.stash);

        if let BranchResult::Hit(branch) = res {
            Some(ValContext::new(self.mut_context(branch)))
        } else {
            None
        }
    }
}

impl<K, V, M> MapOpsKeySum<K, V, M> for Collection<KV<K, V>, M>
    where M: Meta<KV<K, V>> + SubMeta<Key<K>> + SubMeta<KeySum<u64>>,
          K: Val + Ord + Hash,
          V: Clone
{
    fn merge(&mut self, b: &mut Self) -> Self {
        self.union_using::<Key<K>, KeySum<u64>>(b)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    const LOTS: usize = 100_000;

    use std::hash::Hash;

    use meta::key::{Key, Keyed, KeySum, ValSum};

    use collection::Collection;

    use super::MapOps;
    use super::MapOpsKeySum;

    collection!(Map<T> {
        key: Key<T::Key>,
        keysum: KeySum<u64>,
        valsum: ValSum<u64>,
    } where T: Keyed, T::Key: Hash, T::Value: Hash);

    #[test]
    fn insert() {
        let mut map = Map::new();
        map.insert("a", 1);
        assert_eq!(map.get("a"), Some(&1));
    }

    #[test]
    fn partial_eq() {
        let mut a = Map::new();
        let mut b = Map::new();

        for i in 0..LOTS {
            a.insert(i, i + 1);
            b.insert(LOTS - i - 1, LOTS - i - 1);
        }

        // mutate in a
        for i in 0..LOTS {
            a.get_mut(i).map(|mut v| *v -= 1);
        }

        assert!(a == b);
    }

    #[test]
    fn overwrite() {
        let mut map = Map::new();

        map.insert("a", 1);
        assert_eq!(map.get("a"), Some(&1));
        map.insert("a", 2);
        assert_eq!(map.get("a"), Some(&2));
    }

    #[test]
    fn clone() {
        let mut a = Map::new();

        a.insert("a", 1);

        let mut b = a.clone_mut();

        b.insert("a", 2);

        assert_eq!(a.get("a"), Some(&1));
        assert_eq!(b.get("a"), Some(&2));
    }

    #[test]
    fn merge() {
        let mut a = Map::new();
        let mut b = Map::new();

        a.insert("a", 1);
        b.insert("a", 1);

        a.insert("b", 2);
        b.insert("b", 3);

        b.insert("c", 4);

        let am = a.merge(&mut b);
        let bm = b.merge(&mut a);

        assert_eq!(am.get("a"), Some(&1));
        assert_eq!(am.get("b"), Some(&3));
        assert_eq!(am.get("c"), Some(&4));

        assert_eq!(bm.get("a"), Some(&1));
        assert_eq!(bm.get("b"), Some(&2));
        assert_eq!(bm.get("c"), Some(&4));
    }
}
