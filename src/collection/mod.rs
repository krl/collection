use std::io;

use meta::{Meta, SubMeta, Select};
use freezer::{Location, Backend, CryptoHash};
use freezer::{Freezer, Freeze, Sink, Source};
use tree::weight::Weight;
use tree::node::Node;
use tree::branch::Branch;
use tree::level::{Beginning, End};

/// A collection of elements of type T,
/// with metadata of type M.
///
/// This is the base type, that all the collection operations
/// are implemented over.
pub struct Collection<T, M, H, B>
    where H: CryptoHash
{
    /// The location that constitutes the root for this collection.
    pub root: Location<H>,
    /// Top level metadata
    pub meta: Option<M>,
    /// The store for nodes
    pub freezer: Freezer<Node<T, M, H>, H, B>,
    /// The branching factor, currently hard-coded to 2, which means on average
    /// every fourth element will have weight > 0.
    pub divisor: usize,
}

impl<T, M, H, B> Collection<T, M, H, B>
    where T: Weight + Freeze<H>,
          M: Meta<T> + Freeze<H>,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>
{
    /// Returns a new, empty Collection.
    pub fn new(backend: B) -> Self {
        let freezer = Freezer::new(backend);
        let root = freezer.put(Node::new());
        Collection {
            root,
            freezer,
            meta: None,
            // hard-coded for now
            divisor: 2,
        }
    }

    /// Constructs a Collection given a root, and a freezer
    pub fn new_from(root: Location<H>,
                    freezer: Freezer<Node<T, M, H>, H, B>)
                    -> io::Result<Self> {
        Ok(Collection {
               meta: freezer.get(&root)?
                   .meta()
                   .map(|m| m.into_owned()),
               root,
               freezer,
               divisor: 2,
           })
    }

    /// Constructs a Collection given a freezer
    pub fn with_freezer(freezer: Freezer<Node<T, M, H>, H, B>) -> Self {
        let root = freezer.put(Node::new());
        Collection {
            root,
            freezer,
            meta: None,
            divisor: 2,
        }
    }

    /// Returns the metadata of the collection, if non-empty
    pub fn meta(&self) -> &Option<M> {
        &self.meta
    }

    /// Sets the root of the collection, updating metadata
    pub fn new_root(&mut self, loc: Location<H>) -> io::Result<()> {
        self.meta = self.freezer
            .get(&loc)?
            .meta()
            .map(|m| m.into_owned());
        self.root = loc;
        Ok(())
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
    pub fn union_using<F, E>(&mut self, b: &mut Self) -> io::Result<Self>
        where F: Meta<T> + Select<T> + PartialEq + Ord,
              E: Meta<T> + PartialEq,
              M: SubMeta<F> + SubMeta<E>,
              B: Backend<Node<T, M, H>, H> + Clone
    {
        let a = self.clone();

        self.freezer.merge(&mut b.freezer);

        let mut branch_a: Branch<_, _, Beginning, _, _> =
            Branch::first(a.root.clone(), &self.freezer)?;
        let mut branch_b: Branch<_, _, Beginning, _, _> =
            Branch::first(b.root.clone(), &self.freezer)?;
        // Branch into union, being constructed as we go
        let mut branch_c: Option<Branch<_, _, End, _, _>> = None;

        fn a_b<T, M, F, E, H, B>(from: &mut Branch<T, M, Beginning, H, B>,
                                 into: &mut Option<Branch<T, M, End, H, B>>,
                                 divisor: usize,
                                 mut key: F,
                                 freezer: &mut Freezer<Node<T, M, H>, H, B>)
                                 -> io::Result<()>
            where T: Weight + Freeze<H> + Clone,
                  F: Meta<T> + Select<T> + PartialEq + Ord,
                  E: Meta<T> + PartialEq,
                  M: Meta<T> + Freeze<H> + SubMeta<F> + SubMeta<E>,
                  B: Backend<Node<T, M, H>, H>,
                  H: CryptoHash
        {
            from.find_full(&mut key, freezer)?;

            let left = from.left(freezer)?;
            *from = from.right(freezer)?;

            if into.is_some() {
                *into = Some(into.as_ref()
                                 .expect("is some")
                                 .concat(&left.reverse(&freezer)?,
                                         divisor,
                                         freezer)?);
                Ok(())
            } else {
                *into = Some(left);
                Ok(())
            }
        }

        loop {
            let keys = (branch_a.leaf(&self.freezer)?.map(|t| F::from_t(&*t)),
                        branch_b.leaf(&self.freezer)?.map(|t| F::from_t(&*t)));
            match keys {
                (Some(a), Some(b)) => {
                    if a == b {
                        branch_a.skip_equal::<E>(&mut branch_b, &self.freezer)?;
                        a_b::<_, _, F, E, _, _>(&mut branch_b,
                                                &mut branch_c,
                                                self.divisor,
                                                a,
                                                &mut self.freezer)?;
                        branch_a = branch_a.right(&mut self.freezer)?;
                    } else if a > b {
                        a_b::<_, _, F, E, _, _>(&mut branch_b,
                                                &mut branch_c,
                                                self.divisor,
                                                a,
                                                &mut self.freezer)?;
                    } else {
                        a_b::<_, _, F, E, _, _>(&mut branch_a,
                                                &mut branch_c,
                                                self.divisor,
                                                b,
                                                &mut self.freezer)?;
                    }
                }
                (None, Some(_)) => {
                    // concat full b
                    if branch_c.is_some() {
                        branch_c = Some(branch_c.as_ref()
                                            .expect("is some")
                                            .concat(&branch_b,
                                                    self.divisor,
                                                    &mut self.freezer)?);
                    } else {
                        branch_c = Some(branch_b.reverse(&self.freezer)?)
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
                                                    &mut self.freezer)?);
                    } else {
                        branch_c = Some(branch_a.reverse(&self.freezer)?)
                    }
                    break;
                }
                (None, None) => break,
            }
        }
        match branch_c {
            None => {
                Ok(Collection {
                       root: self.freezer.put(Node::new()),
                       freezer: self.freezer.clone(),
                       meta: None,
                       divisor: self.divisor,
                   })
            }
            Some(branch) => {
                let root = branch.into_root();
                Ok(Collection {
                       meta: self.freezer
                           .get(&root)?
                           .meta()
                           .map(|m| m.into_owned()),
                       root: root,
                       freezer: self.freezer.clone(),
                       divisor: self.divisor,
                   })
            }
        }
    }

    /// Persists the collection to the underlying Backend
    pub fn persist(&self) -> io::Result<H::Digest> {
        self.freezer.freeze(&self.root)
    }

    /// Restores a collection from the Backend given a hash
    pub fn restore(hash: H::Digest, backend: B) -> io::Result<Self> {
        let freezer = Freezer::new(backend);
        let root = Location::from_hash(hash);
        Ok(Collection {
               meta: freezer.get(&root)?
                   .meta()
                   .map(|m| m.into_owned()),
               freezer: freezer.clone(),
               root: root,
               divisor: 2,
           })
    }
}


impl<T, M, H, B> Clone for Collection<T, M, H, B>
    where H: CryptoHash,
          T: Weight + Freeze<H>,
          M: Clone + Freeze<H> + Meta<T>,
          B: Backend<Node<T, M, H>, H> + Clone
{
    fn clone(&self) -> Self {
        Collection {
            freezer: self.freezer.clone(),
            root: self.root.clone(),
            meta: self.meta.clone(),
            divisor: self.divisor,
        }
    }
}

#[macro_export]
macro_rules! collection {
    ($collection:ident<$t:ident, $h:ty>
     {
         $( $slot:ident: $submeta:ident<$subtype:ty>, )*
     } where $($restraints:tt)*) => (
        mod col {
            use {Weight, Freeze, CryptoHash, Sink, Source};
            use {Meta, SubMeta};
            use std::marker::PhantomData;
            use std::borrow::Cow;
            use std::io;

            use super::*;

            #[derive(Clone)]
            pub struct CollectionMeta<$t, H>
                where H: CryptoHash,
                      $t: Clone,
                      $($restraints)*
            {
                _t: PhantomData<($t, H)>,
                $(
                    $slot: $submeta<$subtype>,
                )*
            }

            impl<T, H> Freeze<H> for CollectionMeta<T, H>
                where T: Weight + Freeze<H>,
                      H: CryptoHash,
                      $($submeta<$subtype>: Freeze<H>,)*
                      $($restraints)*
            {
                fn freeze(&self, into: &mut Sink<H>)
                          -> io::Result<()> {
                    $(
                        self.$slot.freeze(into)?;
                    )*
                        Ok(())
                }

                fn thaw(from: &mut Source<H>) -> io::Result<Self> {
                    Ok(CollectionMeta {
                        _t: PhantomData,
                        $( $slot: $submeta::thaw(from)?, ) *
                    })
                }
            }

            impl<$t, H> Meta<$t> for CollectionMeta<$t, H>
                where H: CryptoHash,
                      $t: Clone + Freeze<H>,
                      $($restraints)*
            {
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

            macro_rules! submeta {
                ($_submeta:ident, $_subtype:ty, $_slot:ident) => (
                    impl<'a, $t, H> SubMeta<$_submeta<$_subtype>>
                        for CollectionMeta<T, H>
                        where H: CryptoHash,
                              $t: Freeze<H>,
                              $($restraints)*
                    {
                        fn submeta(&self) -> Cow<$_submeta<$_subtype>> {
                            Cow::Borrowed(&self.$_slot)
                        }
                    }
                )
            }

            $(
                submeta!($submeta, $subtype, $slot);
            )*
        }

        pub type $collection<T, B> =
            Collection<T, self::col::CollectionMeta<T, $h>, $h, B>;
    )
}

impl<T, M, H, B> Freeze<H> for Collection<T, M, H, B>
    where T: Weight + Freeze<H>,
          M: Meta<T> + Freeze<H>,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H> + Clone
{
    fn freeze(&self, _: &mut Sink<H>) -> io::Result<()> {
        Ok(())
    }

    fn thaw(_: &mut Source<H>) -> io::Result<Self> {
        panic!()
    }
}
