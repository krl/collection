//! Persistent datastructure

//#![deny(missing_docs)]

#[cfg(not(test))]
mod collection;

#[cfg(test)]
#[macro_use]
mod collection;

extern crate freezer;
extern crate seahash;

mod tree;
mod meta;
mod ops;

#[cfg(test)]
mod test_common;

pub use collection::Collection;

pub use ops::vector::VectorOps;
pub use ops::map::{MapOps, MapOpsKeySum};
pub use ops::set::{SetOps, SetOpsCheckSum};

pub use meta::Meta;
pub use meta::Max;
pub use meta::CheckSum;
pub use meta::Key;
