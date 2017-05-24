//! Metadata keeping track of the checksum of elements in the collection.

use seahash::SeaHasher;

use std::marker::PhantomData;
use std::hash::{Hash, Hasher};
use std::io;

use meta::{Meta, SubMeta};

use freezer::{Backend, Freeze, CryptoHash, Sink, Source};
use tree::node::Node;
use tree::weight::Weight;

use collection::Collection;

/// `T` is able to be checksummed.
#[derive(Clone, PartialEq, Hash)]
pub struct CheckSum<T>(T);

impl<T> CheckSum<T> {
    /// Construct a new CheckSum
    pub fn new(t: T) -> Self {
        CheckSum(t)
    }
}

impl<T> Meta<T> for CheckSum<u64>
    where T: Hash
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

impl<T, M, H, B> PartialEq for Collection<T, M, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T> + SubMeta<CheckSum<u64>> + Freeze<H>,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>
{
    fn eq(&self, other: &Self) -> bool {
        let ma = self.meta();
        let mb = other.meta();
        ma.as_ref().map(|m| m.submeta()) == mb.as_ref().map(|m| m.submeta())
    }
}

impl<T, M, H, B> Hash for Collection<T, M, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T> + SubMeta<CheckSum<u64>> + Freeze<H>,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>
{
    fn hash<I: Hasher>(&self, state: &mut I) {
        self.meta.as_ref().map(|m| m.submeta().hash(state));
    }
}

impl<T, H> Freeze<H> for CheckSum<T>
    where H: CryptoHash,
          T: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(CheckSum(T::thaw(from)?))
    }
}
