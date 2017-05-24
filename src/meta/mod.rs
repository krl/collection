//! Metadata for collections

pub mod cardinality;
pub mod max;
pub mod key;
pub mod checksum;

use std::marker::PhantomData;
use std::borrow::Cow;

use freezer::{Location, CryptoHash};

pub use meta::cardinality::Cardinality;
pub use meta::checksum::CheckSum;
pub use meta::max::Max;
pub use meta::key::Key;

/// Metadata for `T`
pub trait Meta<T>
    where Self: Clone
{
    /// Construct a metadata value from `&T`
    fn from_t(t: &T) -> Self;
    /// Merge two metadata values, `(M, M) -> M`
    fn merge(&mut self, other: &Self, _t: PhantomData<T>);
}

/// Implemented for compound-Metadata, for each of the sub-metadatas.
pub trait SubMeta<T>
    where T: Clone
{
    /// Turns the compound `Meta` into a specific `Meta`
    fn submeta(&self) -> Cow<T>;
}

/// This enum is returned from select methods, to signal if something was found.
#[derive(Debug)]
pub enum Selection {
    /// We found the element
    Hit,
    /// We found where the element would sort if it existed.
    Between,
    /// Not here
    Miss,
}

/// A trait that metadata can implement to allow searching through
/// the tree.
pub trait Select<T>
    where Self: Sized + Clone
{
    /// Compare the metadata to an element, and return a `Selection`
    fn select(&mut self, other: Cow<Self>) -> Selection;
}

/// Like `Selection` but used on the branch level.
pub enum Found<H>
    where H: CryptoHash
{
    /// It might be there, but deeper!
    Node(Location<H>),
    /// No such thing!
    Miss,
    /// We found it!
    Hit,
    /// If it existed, it would have been here.
    Between,
}
