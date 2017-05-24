//! Metadata keeping track of the number of elements in the collection.
//!
//! Implements `Select` to find the n:th element.

use std::marker::PhantomData;
use std::borrow::Cow;
use std::io;

use meta::{Meta, Select, Selection};
use freezer::{Freeze, CryptoHash, Sink, Source};

/// Wrapper type for the value storing the cardinality of subtrees
#[derive(Clone, PartialEq)]
pub struct Cardinality<T>(T);

impl<T> Cardinality<T>
    where T: Clone
{
    /// Construct a new `Cardinality` from `T`
    pub fn new(t: &T) -> Self {
        Cardinality(t.clone())
    }

    /// Get the inner value of the wrapper type.
    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> Meta<T> for Cardinality<usize> {
    fn from_t(_: &T) -> Self {
        Cardinality(1)
    }

    fn merge(&mut self, other: &Self, _p: PhantomData<T>) {
        self.0 += other.0;
    }
}

impl<T> Select<T> for Cardinality<usize> {
    fn select(&mut self, other: Cow<Self>) -> Selection {
        if self.0 < other.0 {
            Selection::Hit
        } else {
            self.0 -= other.0;
            Selection::Miss
        }
    }
}

impl<T, H> Freeze<H> for Cardinality<T>
    where H: CryptoHash,
          T: Freeze<H>
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        self.0.freeze(into)
    }
    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        Ok(Cardinality(T::thaw(from)?))
    }
}
