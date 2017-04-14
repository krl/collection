//! Persistent datastructure

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
