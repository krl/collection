use seahash::SeaHasher;

use std::marker::PhantomData;
use std::hash::{Hash, Hasher};

use std::borrow::Cow;

use Val;
use meta::{Meta, SubMeta, Select, Selection};

use meta::checksum::CheckSum;

pub trait Keyed {
    type Key: Val + Ord;
    type Value: Clone;

    fn key(&self) -> &Self::Key;
    fn value(&self) -> &Self::Value;
    fn value_mut(&mut self) -> &mut Self::Value;
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key<K>(K);
#[derive(Clone, PartialEq)]
pub struct KeySum<T>(T);
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
    pub fn new(key: K) -> Self {
        Key(key)
    }
}

impl<T> Meta<T> for Key<T::Key>
    where T: Val + Keyed
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
    where T: Val + Keyed
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
    where T: Val + Keyed,
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
    where T: Val + Keyed,
          T::Value: Hash
{
    fn from_t(t: &T) -> Self {
        let mut hasher = SeaHasher::new();
        t.value().hash(&mut hasher);
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
