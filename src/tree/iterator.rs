use std::mem;
use std::io;
use std::borrow::Cow;

use freezer::{Freeze, CryptoHash, Backend};
use freezer::freezer::{Freezer, Location};

use tree::weight::Weight;
use tree::branch::Branch;
use tree::node::Node;
use tree::level::{Relative, Beginning, End};
use meta::Meta;
use collection::Collection;

pub struct Iter<'a, T, M, R, H, B>
    where H: 'a + CryptoHash,
          T: 'a,
          M: 'a,
          B: 'a
{
    freezer: &'a Freezer<Node<T, M, H>, H, B>,
    state: State<T, M, R, H, B>,
}

/// An iterator over a Collection
enum State<T, M, R, H, B>
    where H: CryptoHash
{
    Initial(Location<H>),
    Branch(Branch<T, M, R, H, B>),
    Placeholder,
}

impl<'a, T, M, R, H, B> Iter<'a, T, M, R, H, B>
    where H: CryptoHash
{
    /// Constructs a new iterator over the provided root and freezer-ref.
    pub fn new(root: Location<H>,
               freezer: &'a Freezer<Node<T, M, H>, H, B>)
               -> Self {
        Iter {
            freezer,
            state: State::Initial(root),
        }
    }
}

impl<'a, T, M, R, H, B> Iterator for Iter<'a, T, M, R, H, B>
    where H: 'a + CryptoHash,
          T: 'a + Weight + Freeze<H>,
          M: 'a + Meta<T>,
          B: 'a + Backend<Node<T, M, H>, H>,
          R: Relative
{
    type Item = io::Result<Cow<'a, T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let oldstate = mem::replace(&mut self.state, State::Placeholder);
        match oldstate {
            State::Initial(root) => {
                let branch: io::Result<Branch<_, _, R, _, _>> =
                    Branch::first(root.clone(), &self.freezer);
                match branch {
                    Ok(branch) => self.state = State::Branch(branch),
                    Err(e) => {
                        self.state = State::Initial(root);
                        return Some(Err(e));
                    }
                }
            }
            State::Branch(mut branch) => {
                match branch.step(&self.freezer) {
                    Ok(Some(_)) => (),
                    Ok(None) => return None,
                    Err(e) => return Some(Err(e)),
                }
                self.state = State::Branch(branch)
            }
            State::Placeholder => unreachable!(),
        }

        if let State::Branch(ref branch) = self.state {
            match branch.leaf(&self.freezer) {
                Ok(Some(t)) => Some(Ok(t)),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            }
        } else {
            unreachable!()
        }
    }
}

impl<T, M, H, B> Collection<T, M, H, B>
    where H: CryptoHash,
          T: Weight + Freeze<H>,
          M: Meta<T>,
          B: Backend<Node<T, M, H>, H>
{
    /// Returns an iterator over Collection
    pub fn iter<'a>(&'a self) -> Iter<'a, T, M, Beginning, H, B> {
        Iter::new(self.root.clone(), &self.freezer)
    }

    /// Returns a reverse iterator over Collection
    pub fn iter_rev<'a>(&'a self) -> Iter<'a, T, M, End, H, B> {
        Iter::new(self.root.clone(), &self.freezer)
    }
}
