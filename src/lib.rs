//! This library implements persistent datastructures for Rust, with
//! efficient, with the complexity of the union operation can be sublinear
//! in practical cases.

//! # Deterministically balanced search trees
//! This is achieved by using deterministically balanced trees, where the
//! split-points of the tree are determined by the `Weight` of the elements
//! in the collection.
//!
//! That T implements Weight, means that `T`, or parts of `T` can be Hashed
//! this hash is then used to determine the weight, essentialy by counting
//! leading zeroes in the hash.
//!
//! # Copy on write, and structural sharing
//! Cloning a collection is a constant time operation. This is achieved by
//! the Stash abstraction, which keeps a collection of Nodes, referenced by
//! a `Location` indirection.
//!
//! # Pluggable metadata
//! Each collection can carry different kinds of metadata, metadata is defined
//! as the operations `&T -> Meta<T>`, and a binary operation combining
//! two `Meta<T>` with each other. So, to implement a Set, you would use
//! `Max<T>`, and if you also want constant-time equality checking, you would
//! add `CheckSum<T>

#![deny(missing_docs)]
#[macro_use]
mod collection;

extern crate seahash;
#[macro_use]
extern crate trait_group;

mod stash;
mod tree;
mod html;
mod meta;
mod ops;

use tree::weight::Weight;

trait_group! {
/// A value that can be put into a Collection.
/// Has to be clonable, and have impl the `tree::weight::Weight` trait
    pub trait Val: Weight + Clone
}

pub use collection::Collection;
