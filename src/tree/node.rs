use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;

use std::borrow::Cow;

use Val;
use stash::{RelStash, Location};
use meta::{Meta, SubMeta};
use html::Html;

use meta::checksum::CheckSum;

pub enum Child<T, M>
    where T: Val,
          M: Meta<T>
{
    Node { location: Location<T, M>, meta: M },
    Leaf(T),
}

impl<T, M> Child<T, M>
    where T: Val,
          M: Meta<T>
{
    pub fn new_leaf(t: T) -> Self {
        Child::Leaf(t)
    }

    pub fn new_node(location: Location<T, M>, meta: M) -> Self {
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

    fn relativize(&mut self, depth: usize) {
        match *self {
            Child::Node { ref mut location, .. } => {
                *location = location.relative(depth)
            }
            _ => return,
        }
    }
}

pub enum RemoveResult<T>
    where T: Val
{
    Ok(T),
    Final(T),
    Merge { t: T, depth: usize },
    Void,
}

pub enum InsertResult {
    Ok,
    Split(usize),
}

pub struct Node<T, M>
    where T: Val,
          M: Meta<T>
{
    pub children: VecDeque<Child<T, M>>,
}

impl<T, M> Clone for Node<T, M>
    where T: Val,
          M: Meta<T>
{
    fn clone(&self) -> Self {
        Node { children: self.children.clone() }
    }
}

impl<T, M> Clone for Child<T, M>
    where T: Val,
          M: Meta<T>
{
    fn clone(&self) -> Self {
        match *self {
            Child::Node { location, ref meta } => {
                Child::Node {
                    location: location,
                    meta: meta.clone(),
                }
            }
            Child::Leaf(ref t) => Child::Leaf(t.clone()),
        }
    }
}

impl<T, M> Node<T, M>
    where T: Val,
          M: Meta<T>
{
    pub fn new() -> Self {
        Node { children: VecDeque::new() }
    }

    pub fn concat(a: &Node<T, M>, b: &Node<T, M>) -> Self {
        let mut node = Node::new();
        for child in &a.children {
            node.children.push_back(child.clone());
        }
        for child in &b.children {
            node.children.push_back(child.clone());
        }
        node
    }

    pub fn concat_middle(a: &Node<T, M>, b: &Node<T, M>) -> Self {
        let mut node = Node::new();
        for child in &a.children {
            node.children.push_back(child.clone());
        }
        for child in b.children.iter().skip(1) {
            node.children.push_back(child.clone());
        }
        node
    }

    pub fn single(child: Child<T, M>) -> Self {
        Node { children: vec![child].into() }
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn bottom(&self) -> bool {
        if self.children.len() == 0 {
            true
        } else {
            if let Child::Leaf(_) = self.children[0] {
                true
            } else {
                false
            }
        }
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

    pub fn relativize(&mut self, depth: usize) {
        for child in &mut self.children {
            child.relativize(depth)
        }
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

    pub fn insert(&mut self, ofs: usize, child: Child<T, M>) {
        self.children.insert(ofs, child)
    }

    pub fn insert_t(&mut self, ofs: usize, t: T, divisor: usize) -> usize {
        let self_weight = self.weight(divisor);
        self.insert(ofs, Child::new_leaf(t));
        self_weight
    }

    pub fn remove(&mut self, ofs: usize) -> Option<Child<T, M>> {
        self.children.remove(ofs)
    }

    pub fn remove_t(&mut self, ofs: usize, divisor: usize) -> RemoveResult<T> {
        if self.children.len() == 0 {
            RemoveResult::Void
        } else {
            match self.remove(ofs) {
                Some(Child::Leaf(t)) => {
                    let w = t.weight() / divisor;
                    if w > 0 {
                        RemoveResult::Merge { t: t, depth: w }
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

    pub fn update(&mut self, ofs: usize, child: Child<T, M>) {
        self.children[ofs] = child
    }

    pub fn child<'a: 'b, 'b>(&'a self, ofs: usize) -> Option<&'b Child<T, M>> {
        self.children.get(ofs)
    }

    pub fn rightmost_child(&self) -> Option<&Child<T, M>> {
        let l = self.children.len();
        self.children.get(l - 1)
    }

    pub fn child_mut(&mut self, ofs: usize) -> Option<&mut Child<T, M>> {
        self.children.get_mut(ofs)
    }

    pub fn child_node_location(&self, ofs: usize) -> Option<Location<T, M>> {
        match self.child(ofs) {
            Some(&Child::Node { location, .. }) => Some(location),
            _ => None,
        }
    }

    pub fn merge(&mut self, Node { ref mut children, .. }: Node<T, M>) {
        self.children.append(children);
    }

    pub fn splice(&mut self, ofs: usize, mut from: Node<T, M>) {
        while let Some(child) = from.children.pop_back() {
            self.insert(ofs, child);
        }
    }
}

impl<'a, T, M> PartialEq for Node<T, M>
    where T: Val,
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

impl<T, M> Html<T, M> for Node<T, M>
    where T: Val + fmt::Debug,
          M: Meta<T>
{
    fn _html(&self, stash: RelStash<T, M>) -> String {
        let mut s = String::new();
        for child in &self.children {
            s += &child._html(stash);
        }
        format!("<div class=\"node\">
                   {}
                 </div>",
                s)
    }
}

impl<T, M> Html<T, M> for Child<T, M>
    where T: Val + fmt::Debug,
          M: Meta<T>
{
    fn _html(&self, stash: RelStash<T, M>) -> String {
        match *self {
            Child::Leaf(ref t) => {
                format!("<div class=\"leaf weight-{}\">{:?}</div>",
                        t.weight() / 2,
                        t)
            }
            Child::Node { location, .. } => location._html(stash),
        }
    }
}
