use std::fmt;
use std::mem;
use std::marker::PhantomData;
use std::borrow::Cow;

use Val;
use stash::{Stash, RelStash, Location};
use tree::node::{Node, Child, InsertResult, RemoveResult};
use meta::{Meta, SubMeta, Select, Selection, Found};
use html::Html;

pub trait Relative {
    fn at(i: usize, len: usize) -> usize;
    fn insert(i: usize, len: usize) -> usize;
    fn after(i: usize, len: usize) -> usize;
    fn order<T>(&mut T, &mut T);
    fn from_end() -> bool;
}

pub trait Opposite<R: Relative> {}

pub struct Beginning;
pub struct End;

impl Opposite<End> for Beginning {}
impl Opposite<Beginning> for End {}

impl Relative for Beginning {
    fn order<T>(_: &mut T, _: &mut T) {}
    fn at(i: usize, _: usize) -> usize {
        i
    }
    fn insert(i: usize, _: usize) -> usize {
        i
    }
    fn after(i: usize, _: usize) -> usize {
        i + 1
    }
    fn from_end() -> bool {
        false
    }
}

impl Relative for End {
    fn order<T>(a: &mut T, b: &mut T) {
        mem::swap(a, b)
    }
    fn at(i: usize, len: usize) -> usize {
        (len - i).saturating_sub(1)
    }
    fn insert(i: usize, len: usize) -> usize {
        len - i
    }
    fn after(i: usize, len: usize) -> usize {
        len - i - 1
    }
    fn from_end() -> bool {
        true
    }
}

pub struct Level<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    ofs: usize,
    location: Location<T, M>,
    _r: PhantomData<R>,
}

impl<T, M, R> Clone for Level<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    fn clone(&self) -> Self {
        Level {
            ofs: self.ofs,
            location: self.location.clone(),
            _r: PhantomData,
        }
    }
}

impl<T, M, R> Level<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    pub fn new(location: Location<T, M>) -> Self {
        Level {
            ofs: 0,
            location: location,
            _r: PhantomData,
        }
    }

    pub fn update_child(&mut self,
                        with: Location<T, M>,
                        stash: &mut Stash<T, M>) {
        let new_meta = stash.get(with).meta().map(|cow| cow.into_owned());
        match new_meta {
            Some(meta) => {
                let child = self.child_mut(stash).expect("valid");
                *child = Child::new_node(with, meta);
            }
            None => {
                self.remove(stash);
            }
        }
    }

    pub fn left(&self, stash: &mut Stash<T, M>) -> Option<Location<T, M>> {
        let mut left;
        {
            let node = stash.get(self.location);
            let len = node.len();
            let at = R::at(self.ofs, len);
            if at == 0 {
                return None;
            } else if at == len {
                return Some(self.location);
            } else {
                left = node.left(at);
                left.relativize(self.location.depth);
            }
        }
        Some(stash.put(left))
    }

    // Right always has at least one location.
    pub fn right(&self, stash: &mut Stash<T, M>) -> Location<T, M> {
        let mut right;
        {
            let node = stash.get(self.location);
            let len = node.len();
            let at = R::at(self.ofs, len);
            if at == 0 {
                return self.location;
            }
            if at == len {
                right = Node::new();
            } else {
                right = node.right(at);
                right.relativize(self.location.depth);
            }
        }
        stash.put(right)
    }

    pub fn child<'a>(&self, stash: &'a Stash<T, M>) -> Option<&'a Child<T, M>> {
        let node = stash.get(self.location);
        let len = node.len();
        node.child(R::at(self.ofs, len))
    }

    pub fn child_mut<'a>(&mut self,
                         stash: &'a mut Stash<T, M>)
                         -> Option<&'a mut Child<T, M>> {
        let node = stash.get_mut(&mut self.location);
        let len = node.len();
        node.child_mut(R::at(self.ofs, len))
    }

    pub fn location(&self) -> Location<T, M> {
        self.location
    }

    pub fn empty(&self, stash: &Stash<T, M>) -> bool {
        stash.get(self.location).len() == 0
    }

    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.ofs
    }

    pub fn location_mut(&mut self) -> &mut Location<T, M> {
        &mut self.location
    }

    pub fn step(&mut self, stash: &Stash<T, M>) -> Option<()> {
        let node = stash.get(self.location);

        match node.child(self.ofs + 1) {
            Some(_) => {
                self.ofs = self.ofs + 1;
                return Some(());
            }
            None => return None,
        }
    }

    pub fn force_step(&mut self) {
        self.ofs += 1;
    }

    pub fn steppable(&mut self, stash: &Stash<T, M>) -> bool {
        match stash.get(self.location).child(self.ofs + 1) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn insert_loc(&mut self,
                      loc: Location<T, M>,
                      stash: &mut Stash<T, M>) {
        stash.get(loc)
            .meta()
            .map(|meta| Child::new_node(loc, meta.into_owned()))
            .map(|child| self.insert(child, stash));
    }

    pub fn insert_after(&mut self,
                        child: Child<T, M>,
                        stash: &mut Stash<T, M>) {
        let node = stash.get_mut(&mut self.location);
        let len = node.len();
        node.insert(R::after(self.ofs, len), child);
    }

    pub fn insert(&mut self, child: Child<T, M>, stash: &mut Stash<T, M>) {
        let node = stash.get_mut(&mut self.location);
        let len = node.len();
        node.insert(R::insert(self.ofs, len), child);
    }

    pub fn insert_t(&mut self,
                    t: T,
                    divisor: usize,
                    stash: &mut Stash<T, M>)
                    -> InsertResult {
        let weight = t.weight() / divisor;
        let node = stash.get_mut(&mut self.location);
        let len = node.len();
        let self_weight = node.insert_t(R::insert(self.ofs, len), t, divisor);

        if len == 0 {
            InsertResult::Ok
        } else {
            match (self_weight, weight, R::from_end()) {
                // A
                (self_w, _, true) if self_w > 0 => InsertResult::Split(self_w),
                // B
                (_, weight, false) if weight > 0 => InsertResult::Split(weight),
                _ => InsertResult::Ok,
            }
        }
    }

    fn remove(&mut self, stash: &mut Stash<T, M>) -> Option<Node<T, M>> {
        let mut child_node_loc = None;
        {
            let node = stash.get_mut(&mut self.location);
            match node.remove(self.ofs) {
                Some(Child::Node { location, .. }) => {
                    child_node_loc = Some(location);
                    self.ofs = self.ofs.saturating_sub(1);
                }
                _ => (),
            }
        }
        child_node_loc.map(|loc| stash.remove(loc))
    }

    pub fn remove_next(&mut self,
                       stash: &mut Stash<T, M>)
                       -> Option<Node<T, M>> {
        self.ofs += 1;
        match self.remove(stash) {
            Some(removed) => Some(removed),
            // undo
            None => {
                self.ofs = self.ofs.saturating_sub(1);
                None
            }
        }
    }

    pub fn remove_t(&mut self,
                    divisor: usize,
                    stash: &mut Stash<T, M>)
                    -> RemoveResult<T> {
        stash.get_mut(&mut self.location).remove_t(self.ofs, divisor)
    }

    pub fn split(&mut self, stash: &mut Stash<T, M>) -> Child<T, M> {
        let mut new;
        {
            let node = stash.get_mut(&mut self.location);
            let len = node.len();
            new = node.split(R::after(self.ofs, len));
            R::order(node, &mut new);
        }
        let meta =
            new.meta().expect("split cannot produce empty nodes").into_owned();
        Child::new_node(stash.put(new), meta)
    }

    pub fn merge(&mut self, from: Node<T, M>, stash: &mut Stash<T, M>) {
        stash.get_mut(&mut self.location).merge(from)
    }

    pub fn first<'a>(&self, stash: &'a Stash<T, M>) -> Option<&'a Child<T, M>> {
        let node = stash.get(self.location);
        node.child(R::at(0, node.len()))
    }

    pub fn find<S>(&mut self,
                   search: &mut S,
                   stash: &Stash<T, M>)
                   -> Found<T, M>
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        let node = stash.get(self.location);
        let len = node.len();

        if len == 0 {
            return Found::Between;
        }

        loop {
            let child = node.child(R::at(self.ofs, len));
            match child {
                Some(&Child::Node { location, ref meta }) => {
                    match search.select(meta.submeta()) {
                        Selection::Hit | Selection::Between => {
                            return Found::Node(location)
                        }
                        Selection::Miss => {
                            self.ofs += 1;
                        }
                    }
                }
                Some(&Child::Leaf(ref t)) => {
                    match search.select(Cow::Owned(S::from_t(t))) {
                        Selection::Hit => {
                            return Found::Hit;
                        }
                        Selection::Between => {
                            return Found::Between;
                        }
                        Selection::Miss => {
                            self.ofs += 1;
                        }
                    }
                }
                None => return Found::Miss,
            }
        }
    }

    pub fn concat(left: Location<T, M>,
                  right: Location<T, M>,
                  stash: &mut Stash<T, M>)
                  -> Level<T, M, R> {
        let ofs;
        let new;
        {
            let lnode = stash.get_clone(left);
            let rnode = stash.get_clone(right);

            if rnode.len() == 0 {
                new = lnode;
                ofs = 0;
            } else {
                if R::from_end() {
                    ofs = rnode.len() - 1
                } else {
                    ofs = lnode.len()
                }
                if !lnode.bottom() {
                    new = Node::concat_middle(&lnode, &rnode);
                } else {
                    new = Node::concat(&lnode, &rnode);
                }
            }
        }
        Level {
            ofs: ofs,
            location: stash.put(new),
            _r: PhantomData,
        }
    }

    pub fn weight(&self, divisor: usize, stash: &Stash<T, M>) -> usize {
        stash.get(self.location).weight(divisor)
    }
}

impl<T, M, R> Level<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    pub fn reverse<O>(&self, stash: &Stash<T, M>) -> Level<T, M, O>
        where O: Relative + Opposite<R>,
              R: Relative + Opposite<O>
    {
        let len = stash.get(self.location).len();
        Level {
            ofs: len - self.ofs - 1,
            location: self.location,
            _r: PhantomData,
        }
    }
}

impl<T, M, R> Html<T, M> for Level<T, M, R>
    where T: Val + fmt::Debug,
          M: Meta<T>,
          R: Relative
{
    fn _html(&self, stash: RelStash<T, M>) -> String {
        let mut left = String::new();
        let mut right = String::new();
        let mut marker = String::new();

        let node = stash.get(self.location());
        // re-set stash
        let stash = stash.relative(self.location());
        let len = node.len() as i16;

        let pivot;

        match R::from_end() {
            false => pivot = self.ofs as i16,
            true => pivot = len - self.ofs as i16 - 1,
        }

        if pivot < 0 {
            marker += &format!("({})", pivot);
        } else {
            for i in 0..pivot {
                left += &node.children[i as usize]._html(stash)
            }

            if pivot >= len as i16 {
                marker += &format!("({})", pivot);
            } else {
                marker = node.children[pivot as usize]._html(stash);
                for i in pivot as usize + 1..len as usize {
                    right += &node.children[i]._html(stash)
                }
            }
        }

        format!("<td><div class=\"lbranch\">{}</div></td>
                 <td class=\"mid\">{}</td>
                 <td><div class=\"rbranch\">{}</div></td><td>{:?}</td>",
                left,
                marker,
                right,
                self.ofs,
        )
    }
}
