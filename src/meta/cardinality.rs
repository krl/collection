use std::marker::PhantomData;
use std::borrow::Cow;

use Val;
use meta::{Meta, Select, Selection};

#[derive(Clone, PartialEq)]
pub struct Cardinality<T>(T);

impl<T> Cardinality<T>
    where T: Val
{
    pub fn new(t: &T) -> Self {
        Cardinality(t.clone())
    }

    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> Meta<T> for Cardinality<usize>
    where T: Val
{
    fn from_t(_: &T) -> Self {
        Cardinality(1)
    }

    fn merge(&mut self, other: &Self, _p: PhantomData<T>) {
        self.0 += other.0;
    }
}

impl<T> Select<T> for Cardinality<usize>
    where T: Val
{
    fn select(&mut self, other: Cow<Self>) -> Selection {
        if self.0 < other.0 {
            Selection::Hit
        } else {
            self.0 -= other.0;
            Selection::Miss
        }
    }
}
