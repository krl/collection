//! Metadata keeping track of the maximum key of elements in the collection.
//!
//! Implements `Select` over sorted collections, to find where a key would sort.

use seahash::SeaHasher;
use freezer::{Freeze, CryptoHash, Sink, Source};

use std::marker::PhantomData;
use std::hash::{Hash, Hasher};
use std::io;

use std::borrow::Cow;

use meta::{Meta, SubMeta, Select, Selection};
use tree::weight::Weight;

use meta::checksum::CheckSum;

/// This `T` can be viewed as a Key-Value pair.
pub trait Keyed {
    /// The key type of `T`
    type Key: Weight + Ord + Clone;
    /// The value type of `T`
    type Value;

    /// Create a new `T` from a key value pair.
    fn new(Self::Key, Self::Value) -> Self;
    /// Get a reference to the key of `T`
    fn key(&self) -> &Self::Key;
    /// Get a reference to the value of `T`
    fn val(&self) -> &Self::Value;
    /// Get a mutable reference to the value of `T`
    fn val_mut(&mut self) -> &mut Self::Value;
    /// Throw away the key and return the value.
    fn into_val(self) -> Self::Value;
}

/// A key, K is `T::Key` where `T: Keyed`
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key<K>(K);

/// The checksum of the hashes of all keys in collection
#[derive(Clone, PartialEq)]
pub struct KeySum<T>(T);

/// The checksum of the hashes of all values in collection
#[derive(Clone, PartialEq)]
pub struct ValSum<T>(T);

impl<T> KeySum<T> {
    fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> ValSum<T> {
    fn inner(&self) -> &T {
        &self.0
    }
}

impl<K> Key<K> {
    /// Construct a new Key
    pub fn new(key: K) -> Self {
        Key(key)
    }
}

impl<T> Meta<T> for Key<T::Key>
    where T: Keyed + Clone
{
    fn from_t(t: &T) -> Self {
        Key(t.key().clone())
    }
    fn merge(&mut self, other: &Self, _t: PhantomData<T>) {
        if self.0 < other.0 {
            self.0 = other.0.clone()
        }
    }
}

impl<T> Select<T> for Key<T::Key>
    where T: Keyed + Clone,
          T::Key: Clone + Ord + PartialEq
{
    fn select(&mut self, other: Cow<Self>) -> Selection {
        if self.0 == other.0 {
            Selection::Hit
        } else if self.0 < other.0 {
            Selection::Between
        } else {
            Selection::Miss
        }
    }
}

impl<T> Meta<T> for KeySum<u64>
    where T: Keyed,
          T::Key: Hash
{
    fn from_t(t: &T) -> Self {
        let mut hasher = SeaHasher::new();
        t.key().hash(&mut hasher);
        KeySum(hasher.finish())
    }
    fn merge(&mut self, other: &Self, _t: PhantomData<T>) {
        let a = self.0;
        let b = other.0;
        self.0 = (a ^ b).wrapping_add(a);
    }
}

impl<T> Meta<T> for ValSum<u64>
    where T: Keyed,
          T::Value: Hash
{
    fn from_t(t: &T) -> Self {
        let mut hasher = SeaHasher::new();
        t.val().hash(&mut hasher);
        ValSum(hasher.finish())
    }

    fn merge(&mut self, other: &Self, _t: PhantomData<T>) {
        let a = self.0;
        let b = other.0;
        self.0 = (a ^ b).wrapping_add(a);
    }
}

impl<M> SubMeta<CheckSum<u64>> for M
    where M: SubMeta<KeySum<u64>> + SubMeta<ValSum<u64>>
{
    fn submeta(&self) -> Cow<CheckSum<u64>> {
        let k: Cow<KeySum<u64>> = self.submeta();
        let v: Cow<ValSum<u64>> = self.submeta();

        let check = CheckSum::new(k.inner() ^ v.inner());
        Cow::Owned(check)
    }
}

impl<T, H> Freeze<H> for KeySum<T>
    where H: CryptoHash,
          T: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(KeySum(T::thaw(from)?))
    }
}

impl<T, H> Freeze<H> for ValSum<T>
    where H: CryptoHash,
          T: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(ValSum(T::thaw(from)?))
    }
}

impl<K, H> Freeze<H> for Key<K>
    where H: CryptoHash,
          K: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(Key(K::thaw(from)?))
    }
}
