//! Persistent datastructure
#![cfg_attr(nightly, feature(test))]
#![deny(missing_docs)]

#[cfg(not(test))]
mod collection;

#[cfg(test)]
#[macro_use]
mod collection;

extern crate freezer;
extern crate seahash;

mod tree;
pub mod meta;
mod ops;

#[cfg(test)]
mod test_common;

#[cfg(nightly)]
mod bench;

pub use collection::Collection;

pub use ops::vector::VectorOps;
pub use ops::map::{MapOps, MapOpsKeySum};
pub use ops::set::{SetOps, SetOpsCheckSum};

pub use meta::Meta;
pub use meta::{SubMeta, Max, CheckSum, Key, Cardinality};

pub use tree::weight::Weight;

// re-exports
pub use freezer::{BlakeWrap, CryptoHash, Freeze, Sink, Source};
