use Val;

use std::fmt;
use std::ops::{Deref, DerefMut};

use html::{Html, CSS};
use meta::{Meta, SubMeta, Select};
use stash::{Location, Stash};
use tree::node::Node;
use tree::branch::Branch;
use tree::level::{Beginning, End, Relative};


/// A collection of elements of type T,
/// with metadata of type M.
///
/// This is the base type, that all the collection operations
/// are implemented over.
pub struct Collection<T, M>
    where T: Val,
          M: Meta<T>
{
    /// The location on the stash that constitutes the root for this collection.
    pub root: Location<T, M>,
    /// The store for values of type Node<T, M>
    pub stash: Stash<T, M>,
    /// The branching factor, currently hard-coded to 2, which means on average
    /// every fourth element will have weight > 0.
    pub divisor: usize,
}

/// A view into a Collection, being able to act as a &mut T wrapper.
///
/// When this type dhrops, the collection will be re-balanced as neccesary.
pub struct MutContext<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    /// The branch into the Collection, pointing at a value T
    branch: Branch<T, M, R>,
    /// A mutable reference to the root of the Collection this branch was
    root: &'a mut Location<T, M>,
    /// The stash of the parent Collection
    stash: &'a mut Stash<T, M>,
    /// The divisor from parent.
    divisor: usize,
    /// The weight of the element pointed at, pre-mutation.
    weight: usize,
}

impl<'a, T, M, R> Deref for MutContext<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.branch.leaf(self.stash).expect("Invalid context")
    }
}

impl<'a, T, M, R> DerefMut for MutContext<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.branch.leaf_mut(self.stash).expect("Invalid context")
    }
}

impl<'a, T, M, R> Drop for MutContext<'a, T, M, R>
    where T: 'a + Val,
          M: 'a + Meta<T>,
          R: Relative
{
    fn drop(&mut self) {
        let new_weight = self.branch
            .leaf(self.stash)
            .expect("Invalid context")
            .weight();

        self.branch.rebalance(self.weight / self.divisor,
                              new_weight / self.divisor,
                              self.stash);
        *self.root = self.branch.root();
    }
}

impl<T, M> Collection<T, M>
    where T: Val,
          M: Meta<T>
{
    /// Returns a new, empty Collection.
    pub fn new() -> Self {
        let mut stash = Stash::new();
        let root = stash.put(Node::new());
        Collection {
            root: root,
            stash: stash,
            divisor: 2,
        }
    }

    /// Constructs a Collection given a root and a stash
    pub fn new_from(root: Location<T, M>, stash: Stash<T, M>) -> Self {
        Collection {
            root: root,
            stash: stash,
            divisor: 2,
        }
    }

    /// Produces a html representation of this Collection. For debug use only.
    pub fn _html(&self) -> String
        where T: fmt::Debug
    {
        format!("<style>{}</style>{}",
                CSS,
                self.root._html(self.stash.top()))
    }

    /// Clones the collection, mutating self
    pub fn clone_mut(&mut self) -> Self {
        let new_stash = self.stash.clone_mut(&mut self.root);
        Collection {
            stash: new_stash,
            root: self.root,
            divisor: self.divisor,
        }
    }

    /// Returns a new, cloned collection that is the result of a union operation
    /// given two Meta implementations `F` and `E`
    ///
    /// `F` is used to select which T goes first in the union.
    ///
    /// `E` is used to find overlapping common subtrees.
    ///
    /// For Sets: `F: Max<T>`, `E: CheckSum<T>`, and for Maps:
    /// `F: Key<T::Key>`, `E: KeySum<T>`.
    ///
    /// When the equality testing succeeds, elements will be picked from
    /// the Collection `b`.
    pub fn union_using<F, E>(&mut self, b: &mut Self) -> Self
        where F: Meta<T> + Select<T> + PartialEq + Ord,
              E: Meta<T> + PartialEq,
              M: SubMeta<F> + SubMeta<E>
    {
        let a = self.clone_mut();

        let mut stash =
            self.stash.merge(&mut self.root, &mut b.root, &mut b.stash);

        let mut branch_a: Branch<_, _, Beginning> = Branch::first(a.root,
                                                                  &stash);
        let mut branch_b: Branch<_, _, Beginning> = Branch::first(b.root,
                                                                  &stash);
        // Branch into union, being constructed as we go
        let mut branch_c: Option<Branch<_, _, End>> = None;

        fn a_b<T, M, F, E>(from: &mut Branch<T, M, Beginning>,
                           into: &mut Option<Branch<T, M, End>>,
                           divisor: usize,
                           mut key: F,
                           stash: &mut Stash<T, M>)
            where T: Val,
                  F: Meta<T> + Select<T> + PartialEq + Ord,
                  E: Meta<T> + PartialEq,
                  M: Meta<T> + SubMeta<F> + SubMeta<E>
        {
            from.find_full(&mut key, stash);

            let left = from.left(stash);
            *from = from.right(stash);

            if into.is_some() {
                *into = Some(into.as_ref()
                                 .expect("is some")
                                 .concat(&left.reverse(&stash),
                                         divisor,
                                         stash));
            } else {
                *into = Some(left)
            }
        }

        loop {
            let keys = (branch_a.leaf(&stash).map(|t| F::from_t(t)),
                        branch_b.leaf(&stash).map(|t| F::from_t(t)));
            match keys {
                (Some(a), Some(b)) => {
                    if a == b {
                        branch_a.skip_equal::<E>(&mut branch_b, &stash);
                        a_b::<_, _, F, E>(&mut branch_b,
                                          &mut branch_c,
                                          self.divisor,
                                          a,
                                          &mut stash);
                        branch_a = branch_a.right(&mut stash);
                    } else if a > b {
                        a_b::<_, _, F, E>(&mut branch_b,
                                          &mut branch_c,
                                          self.divisor,
                                          a,
                                          &mut stash);
                    } else {
                        a_b::<_, _, F, E>(&mut branch_a,
                                          &mut branch_c,
                                          self.divisor,
                                          b,
                                          &mut stash);
                    }
                }
                (None, Some(_)) => {
                    // concat full b
                    if branch_c.is_some() {
                        branch_c = Some(branch_c.as_ref()
                                            .expect("is some")
                                            .concat(&branch_b,
                                                    self.divisor,
                                                    &mut stash));
                    } else {
                        branch_c = Some(branch_b.reverse(&stash))
                    }
                    break;
                }
                (Some(_), None) => {
                    // concat full a
                    if branch_c.is_some() {
                        branch_c = Some(branch_c.as_ref()
                                            .expect("is some")
                                            .concat(&branch_a,
                                                    self.divisor,
                                                    &mut stash));
                    } else {
                        branch_c = Some(branch_a.reverse(&stash))
                    }
                    break;
                }
                (None, None) => break,
            }
        }
        match branch_c {
            None => Self::new(),
            Some(branch) => {
                Collection {
                    root: branch.root(),
                    stash: stash,
                    divisor: self.divisor,
                }
            }
        }
    }

    /// Constructs a MutContext context, given a branch into the Collection.
    pub fn mut_context<R: Relative>(&mut self,
                                    branch: Branch<T, M, R>)
                                    -> MutContext<T, M, R> {
        MutContext {
            weight: branch.leaf(&self.stash).expect("Invalid context").weight(),
            branch: branch,
            root: &mut self.root,
            stash: &mut self.stash,
            divisor: self.divisor,
        }
    }
}

#[macro_export]
macro_rules! collection {
    ($collection:ident<$t:ident>
     {
         $( $slot:ident: $submeta:ident<$subtype:ty>, )*
     } where $($restraints:tt)*) => (
        mod col {
            use std::marker::PhantomData;
            use std::borrow::Cow;
            use Val;
            use meta::{Meta, SubMeta};

            use super::*;

            #[derive(Clone)]
                pub struct CollectionMeta<$t> where $t: Val, $($restraints)*
            {
                _t: PhantomData<$t>,
                $(
                    $slot: $submeta<$subtype>,
                )*
            }

            impl<$t> Meta<$t> for CollectionMeta<$t> where $t: Val, $($restraints)* {
                    fn from_t(t: &$t) -> Self {
                        CollectionMeta {
                            _t: PhantomData,
                            $(
                                $slot: $submeta::from_t(t),
                            )*
                        }
                    }
                    fn merge(&mut self, other: &Self, t: PhantomData<$t>) {
                        $(
                            self.$slot.merge(&other.$slot, t);
                        )*
                    }
                }

            macro_rules! as_ref {
                ($_submeta:ident, $_subtype:ty, $_slot:ident) => (
                    impl<'a, $t> SubMeta<$_submeta<$_subtype>>
                        for CollectionMeta<T> where $t: Val, $($restraints)*
                    {
                        fn submeta(&self) -> Cow<$_submeta<$_subtype>> {
                            Cow::Borrowed(&self.$_slot)
                        }
                    }
                )
            }

            $(
                as_ref!($submeta, $subtype, $slot);
            )*
        }

        pub type $collection<T> = Collection<T, self::col::CollectionMeta<T>>;
    )
}
