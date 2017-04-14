use Val;
use std::marker::PhantomData;
use std::borrow::Cow;
use meta::{Meta, Select, Selection};

/// Metadata for the maximum `T` in subtree.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Max<T>(T);

impl<T> Meta<T> for Max<T>
    where T: Val + Ord + PartialEq
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
    where T: Val + Ord + PartialEq
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
