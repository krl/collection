//! Metadata keeping track of the maximum element in the collection.
//!
//! Implements `Select` over sorted collections, to find where `T` would sort.

use std::marker::PhantomData;
use std::borrow::Cow;
use std::io;

use meta::{Meta, Select, Selection};

use freezer::{Freeze, CryptoHash, Sink, Source};

/// Metadata for the maximum `T` in subtree.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Max<T>(T);

impl<T> Meta<T> for Max<T>
    where T: Clone + Ord
{
    fn from_t(t: &T) -> Self {
        Max(t.clone())
    }

    fn merge(&mut self, other: &Self, _t: PhantomData<T>) {
        if self.0 < other.0 {
            self.0 = other.0.clone()
        }
    }
}

impl<T> Select<T> for Max<T>
    where T: Clone + Ord + PartialEq
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

impl<T, H> Freeze<H> for Max<T>
    where H: CryptoHash,
          T: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(Max(T::thaw(from)?))
    }
}
