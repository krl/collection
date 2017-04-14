pub mod cardinality;
pub mod max;
pub mod key;
pub mod checksum;

use std::marker::PhantomData;
use std::borrow::Cow;

use Val;
use stash::Location;

pub trait Meta<T>
    where Self: Clone,
          T: Val
{
    fn from_t(t: &T) -> Self;
    // The PhantomData argument seems to be neccesary
    // to have T be in scope in this method.
    fn merge(&mut self, other: &Self, _t: PhantomData<T>);
}

pub trait SubMeta<T>
    where T: Clone
{
    fn submeta(&self) -> Cow<T>;
}

#[derive(Debug)]
pub enum Selection {
    // We found the element
    Hit,
    // We found where the element would have been
    // if it existed (by Ord, or otherwise)
    Between,
    // Not here
    Miss,
}

pub trait Select<T>
    where Self: Sized + Clone
{
    fn select(&mut self, other: Cow<Self>) -> Selection;
}

pub enum Found<T, M>
    where T: Val,
          M: Meta<T>
{
    /// It might be there, but deeper!
    Node(Location<T, M>),
    /// No such thing!
    Miss,
    /// We found it!
    Hit,
    /// If it existed, it would have been here.
    Between,
}
