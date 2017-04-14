use std::fmt;

use Val;
use tree::branch::Branch;
use tree::level::{Relative, Beginning, End};
use stash::Stash;
use meta::Meta;
use html::Html;
use collection::Collection;

/// An iterator over a Collection
pub struct Iter<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    stash: &'a Stash<T, M>,
    branch: Branch<T, M, R>,
    first: bool,
}

impl<'a, T, M, R> Iter<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    /// Constructs a new iterator over the provided branch and stash-ref.
    pub fn new(branch: Branch<T, M, R>, stash: &'a Stash<T, M>) -> Self {
        Iter {
            stash: stash,
            branch: branch,
            first: true,
        }
    }
}

impl<'a, T, M, R> Iterator for Iter<'a, T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.first == true {
            self.first = false;
            self.branch.leaf(&self.stash)
        } else {
            match self.branch.step(&self.stash) {
                Some(_) => self.branch.leaf(&self.stash),
                None => None,
            }
        }
    }
}

impl<T, M> Collection<T, M>
    where T: Val,
          M: Meta<T>
{
    /// Returns an iterator over Collection
    pub fn iter<'a>(&'a self) -> Iter<'a, T, M, Beginning> {
        let branch: Branch<_, _, Beginning> = Branch::first(self.root,
                                                            &self.stash);
        Iter::new(branch, &self.stash)
    }

    /// Returns a reverse iterator over Collection
    pub fn iter_rev<'a>(&'a self) -> Iter<'a, T, M, End> {
        let branch: Branch<_, _, End> = Branch::first(self.root, &self.stash);
        Iter::new(branch, &self.stash)
    }
}

impl<'a, T, M, R> Iter<'a, T, M, R>
    where T: 'a + Val + fmt::Debug,
          M: 'a + Meta<T>,
          R: Relative
{
    /// Debug HTML output
    pub fn _html(&self) -> String {
        self.branch._html(self.stash.top())
    }
}
