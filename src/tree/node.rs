use std::collections::VecDeque;
use std::io;
use std::marker::PhantomData;

use std::borrow::Cow;

use tree::weight::Weight;
use freezer::{Freeze, Location, CryptoHash, Sink, Source};
use meta::{Meta, SubMeta};

use meta::checksum::CheckSum;

pub enum Child<T, M, H>
    where H: CryptoHash
{
    Node { location: Location<H>, meta: M },
    Leaf(T),
}

impl<T, M, H> Child<T, M, H>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T>,
          H: CryptoHash
{
    pub fn new_leaf(t: T) -> Self {
        Child::Leaf(t)
    }

    pub fn new_node(location: Location<H>, meta: M) -> Self {
        Child::Node {
            location: location,
            meta: meta,
        }
    }

    pub fn meta(&self) -> Cow<M> {
        match *self {
            Child::Leaf(ref t) => Cow::Owned(M::from_t(t)),
            Child::Node { ref meta, .. } => Cow::Borrowed(meta),
        }
    }

    pub fn into_meta(self) -> M {
        match self {
            Child::Leaf(t) => M::from_t(&t),
            Child::Node { meta, .. } => meta,
        }
    }
}

pub enum RemoveResult<T, H> {
    Ok(T),
    Final(T),
    Merge {
        t: T,
        depth: usize,
        _p: PhantomData<H>,
    },
    Void,
}

pub enum InsertResult {
    Ok,
    Split(usize),
}

pub struct Node<T, M, H>
    where H: CryptoHash
{
    pub children: VecDeque<Child<T, M, H>>,
}

impl<T, M, H> Clone for Node<T, M, H>
    where H: CryptoHash,
          T: Clone,
          M: Clone
{
    fn clone(&self) -> Self {
        Node { children: self.children.clone() }
    }
}

impl<T, M, H> Clone for Child<T, M, H>
    where T: Clone,
          M: Clone,
          H: CryptoHash
{
    fn clone(&self) -> Self {
        match *self {
            Child::Node {
                ref location,
                ref meta,
            } => {
                Child::Node {
                    location: location.clone(),
                    meta: meta.clone(),
                }
            }
            Child::Leaf(ref t) => Child::Leaf(t.clone()),
        }
    }
}

impl<T, M, H> Node<T, M, H>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T> + Clone,
          H: CryptoHash
{
    pub fn new() -> Self {
        Node { children: VecDeque::new() }
    }

    pub fn concat(a: &Node<T, M, H>, b: &Node<T, M, H>) -> Self {
        let mut node = Node::new();
        for child in &a.children {
            node.children.push_back(child.clone());
        }
        for child in &b.children {
            node.children.push_back(child.clone());
        }
        node
    }

    pub fn concat_middle(a: &Node<T, M, H>, b: &Node<T, M, H>) -> Self {
        let mut node = Node::new();
        for child in &a.children {
            node.children.push_back(child.clone());
        }
        for child in b.children.iter().skip(1) {
            node.children.push_back(child.clone());
        }
        node
    }

    pub fn single(child: Child<T, M, H>) -> Self {
        Node { children: vec![child].into() }
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn weight(&self, divisor: usize) -> usize {
        self.children
            .back()
            .map(|last| match *last {
                     Child::Leaf(ref t) => t.weight() / divisor,
                     _ => panic!("Weight on node without leaves"),
                 })
            .unwrap_or(0)
    }

    pub fn empty(&self) -> bool {
        self.children.len() == 0
    }

    pub fn meta(&self) -> Option<Cow<M>> {
        let mut m = None;
        for c in &self.children {
            match m {
                None => m = Some(c.meta().into_owned()),
                Some(ref mut meta) => meta.merge(&c.meta(), PhantomData),
            }
        }
        m.map(|inner| Cow::Owned(inner))
    }

    pub fn into_meta(self) -> Option<M> {
        let mut m = None;
        let Node { mut children } = self;
        for c in children.drain(..) {
            match m {
                None => m = Some(c.into_meta()),
                Some(ref mut meta) => meta.merge(&c.into_meta(), PhantomData),
            }
        }
        m.map(|meta| meta)
    }

    pub fn split(&mut self, ofs: usize) -> Self {
        let new = self.children.split_off(ofs);
        Node { children: new }
    }

    pub fn left(&self, ofs: usize) -> Self {
        debug_assert!(ofs > 0);
        debug_assert!(ofs < self.len());
        let mut new = Node::new();
        for i in 0..ofs {
            new.children.push_back(self.children[i].clone())
        }
        new
    }

    pub fn right(&self, ofs: usize) -> Self {
        debug_assert!(ofs > 0);
        debug_assert!(ofs < self.len());
        let mut new = Node::new();
        for i in ofs..self.len() {
            new.children.push_back(self.children[i].clone())
        }
        new
    }

    pub fn insert(&mut self, ofs: usize, child: Child<T, M, H>) {
        self.children.insert(ofs, child)
    }

    pub fn insert_t(&mut self, ofs: usize, t: T, divisor: usize) -> usize {
        let self_weight = self.weight(divisor);
        self.insert(ofs, Child::new_leaf(t));
        self_weight
    }

    pub fn remove(&mut self, ofs: usize) -> Option<Child<T, M, H>> {
        self.children.remove(ofs)
    }

    pub fn remove_t(&mut self,
                    ofs: usize,
                    divisor: usize)
                    -> RemoveResult<T, H> {
        if self.children.len() == 0 {
            RemoveResult::Void
        } else {
            match self.remove(ofs) {
                Some(Child::Leaf(t)) => {
                    let w = t.weight() / divisor;
                    if w > 0 {
                        RemoveResult::Merge {
                            t: t,
                            depth: w,
                            _p: PhantomData,
                        }
                    } else if self.empty() {
                        RemoveResult::Final(t)
                    } else {
                        RemoveResult::Ok(t)
                    }
                }
                _ => panic!("remove_t on non-leaf"),
            }
        }
    }

    pub fn update(&mut self, ofs: usize, child: Child<T, M, H>) {
        self.children[ofs] = child
    }

    pub fn child(&self, ofs: usize) -> Option<&Child<T, M, H>> {
        self.children.get(ofs)
    }

    pub fn into_child(self, ofs: usize) -> Option<Child<T, M, H>> {
        let Node { mut children, .. } = self;
        children.remove(ofs)
    }

    pub fn child_mut(&mut self, ofs: usize) -> Option<&mut Child<T, M, H>> {
        self.children.get_mut(ofs)
    }

    pub fn child_node_location(&self, ofs: usize) -> Option<&Location<H>> {
        match self.child(ofs) {
            Some(&Child::Node { ref location, .. }) => Some(location),
            _ => None,
        }
    }

    pub fn merge(&mut self, Node { ref mut children, .. }: Node<T, M, H>) {
        self.children.append(children);
    }

    pub fn splice(&mut self, ofs: usize, mut from: Node<T, M, H>) {
        while let Some(child) = from.children.pop_back() {
            self.insert(ofs, child);
        }
    }

    pub fn bottom(&self) -> bool {
        match self.children.get(0) {
            Some(&Child::Node { .. }) => false,
            _ => true,
        }
    }
}

impl<'a, T, M, H> PartialEq for Node<T, M, H>
    where T: Weight + Freeze<H> + Clone,
          H: CryptoHash,
          M: Meta<T> + SubMeta<CheckSum<u64>>
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for i in 0..self.len() {
            let ma = self.children[i].meta();
            let mb = other.children[i].meta();
            let a: Cow<CheckSum<u64>> = (*ma).submeta();
            let b: Cow<CheckSum<u64>> = (*mb).submeta();
            if a != b {
                return false;
            }
        }
        true
    }
}

impl<T, M, H> Freeze<H> for Node<T, M, H>
    where T: Weight + Freeze<H>,
          <H as CryptoHash>::Digest: Freeze<H>,
          M: Meta<T> + Freeze<H>,
          H: CryptoHash
{
    fn freeze(&self, into: &mut Sink<H>) -> io::Result<()> {
        let len = self.len();
        let bottom = self.bottom();
        len.freeze(into)?;
        bottom.freeze(into)?;
        for i in 0..len {
            match self.children[i] {
                Child::Leaf(ref t) => {
                    t.freeze(into)?;
                }
                Child::Node {
                    ref location,
                    ref meta,
                } => {
                    location.freeze(into)?;
                    meta.freeze(&mut into.bypass_hashing())?;
                }
            }
        }
        Ok(())
    }

    fn thaw(from: &mut Source<H>) -> io::Result<Self> {
        let len = usize::thaw(from)?;
        let bottom = bool::thaw(from)?;
        let mut node = Node::new();

        if bottom {
            for _ in 0..len {
                node.children.push_back(Child::new_leaf(T::thaw(from)?))
            }
        } else {
            for _ in 0..len {
                node.children.push_back(Child::new_node(Location::thaw(from)?,
                                                        M::thaw(from)?))
            }
        }
        Ok(node)
    }
}
