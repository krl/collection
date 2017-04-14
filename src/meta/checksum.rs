use seahash::SeaHasher;

use std::marker::PhantomData;
use std::hash::{Hash, Hasher};

use Val;
use meta::{Meta, SubMeta};

use collection::Collection;

/// `T` is able to be checksummed.
#[derive(Clone, PartialEq)]
pub struct CheckSum<T>(T);

impl<T> CheckSum<T> {
    /// Construct a new CheckSum
    pub fn new(t: T) -> Self {
        CheckSum(t)
    }
}

impl<T> Meta<T> for CheckSum<u64>
    where T: Val + Hash
{
    fn from_t(t: &T) -> Self {
        let mut hasher = SeaHasher::new();
        t.hash(&mut hasher);
        CheckSum(hasher.finish())
    }

    fn merge(&mut self, other: &Self, _p: PhantomData<T>) {
        // `(a ^ b) + a` does not commute! Which means checksum
        // is order-dependant
        let a = self.0;
        let b = other.0;
        self.0 = (a ^ b).wrapping_add(a);
    }
}

impl<T, M> PartialEq for Collection<T, M>
    where T: Val,
          M: Meta<T> + SubMeta<CheckSum<u64>>
{
    fn eq(&self, other: &Self) -> bool {
        self.stash.get(self.root) == other.stash.get(other.root)
    }
}
