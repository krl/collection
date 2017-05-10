use std::mem;
use std::io;
use std::marker::PhantomData;
use std::borrow::Cow;

use freezer::{Freezer, Freeze, Location, CryptoHash, Backend};
use tree::weight::Weight;
use tree::node::{Node, Child, InsertResult, RemoveResult};
use meta::{Meta, SubMeta, Select, Selection, Found};

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

pub struct Level<T, M, R, H, B>
    where H: CryptoHash
{
    ofs: usize,
    location: Location<H>,
    _r: PhantomData<(T, M, R, B)>,
}

impl<T, M, R, H, B> Clone for Level<T, M, R, H, B>
    where H: CryptoHash
{
    fn clone(&self) -> Self {
        Level {
            ofs: self.ofs,
            location: self.location.clone(),
            _r: PhantomData,
        }
    }
}

impl<T, M, R, H, B> Level<T, M, R, H, B>
    where T: Weight + Freeze<H>,
          M: Meta<T> + Freeze<H>,
          R: Relative,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>,
          Node<T, M, H>: Clone
{
    pub fn new(location: Location<H>) -> Self {
        Level {
            ofs: 0,
            location: location,
            _r: PhantomData,
        }
    }

    pub fn into_location(self) -> Location<H> {
        self.location
    }

    pub fn update_child(&mut self,
                        with: &Location<H>,
                        freezer: &mut Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<()> {
        let new_meta =
            (*freezer.get(&with)?).meta().map(|cow| cow.into_owned());
        match new_meta {
            Some(meta) => {
                let child = self.child_mut(freezer)?.expect("valid");
                *child = Child::new_node(with.clone(), meta);
                Ok(())
            }
            None => {
                self.remove(freezer)?;
                Ok(())
            }
        }
    }

    pub fn left(&self,
                freezer: &mut Freezer<Node<T, M, H>, H, B>)
                -> io::Result<Option<Location<H>>> {
        let left;
        {
            let node = freezer.get(&self.location)?;
            let len = node.len();
            let at = R::at(self.ofs, len);
            if at == 0 {
                return Ok(None);
            } else if at == len {
                return Ok(Some(self.location.clone()));
            } else {
                left = node.left(at);
            }
        }
        Ok(Some(freezer.put(left)))
    }

    // Right always has at least one location.
    pub fn right(&self,
                 freezer: &mut Freezer<Node<T, M, H>, H, B>)
                 -> io::Result<Location<H>> {
        let right;
        {
            let node = freezer.get(&self.location)?;
            let len = node.len();
            let at = R::at(self.ofs, len);
            if at == 0 {
                return Ok(self.location.clone());
            }
            if at == len {
                right = Node::new();
            } else {
                right = node.right(at);
            }
        }
        Ok(freezer.put(right))
    }

    pub fn child_at<'a>(&self,
                        ofs: usize,
                        freezer: &'a Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<Option<Cow<'a, Child<T, M, H>>>> {
        let node = freezer.get(&self.location)?;
        let len = node.len();
        let at = R::at(ofs, len);
        Ok(match node {
               Cow::Owned(node) => {
                   node.into_child(at).map(|child| Cow::Owned(child))
               }
               Cow::Borrowed(ref node) => {
                   node.child(at).map(|child| Cow::Borrowed(child))
               }
           })
    }

    pub fn child<'a>(&self,
                     freezer: &'a Freezer<Node<T, M, H>, H, B>)
                     -> io::Result<Option<Cow<'a, Child<T, M, H>>>> {
        self.child_at(self.ofs, freezer)
    }

    // pub fn first<'a>(&self,
    //                  freezer: &'a Freezer<Node<T, M, H>, H, B>)
    //                  -> io::Result<Option<Cow<'a, Child<T, M, H>>>> {
    //     self.child_at(0, freezer)
    // }

    pub fn child_mut<'a>(&mut self,
                         freezer: &'a mut Freezer<Node<T, M, H>, H, B>)
                         -> io::Result<Option<&'a mut Child<T, M, H>>> {
        let node = freezer.get_mut(&mut self.location)?;
        let len = node.len();
        Ok(node.child_mut(R::at(self.ofs, len)))
    }

    pub fn location(&self) -> &Location<H> {
        &self.location
    }

    // pub fn empty(&self,
    //              freezer: &Freezer<Node<T, M, H>, H, B>)
    //              -> io::Result<bool> {
    //     Ok(freezer.get(&self.location)?.len() == 0)
    // }

    // pub fn offset_mut(&mut self) -> &mut usize {
    //     &mut self.ofs
    // }

    pub fn location_mut(&mut self) -> &mut Location<H> {
        &mut self.location
    }

    pub fn step(&mut self,
                freezer: &Freezer<Node<T, M, H>, H, B>)
                -> io::Result<Option<()>> {
        let node = freezer.get(&self.location)?;

        match node.child(self.ofs + 1) {
            Some(_) => {
                self.ofs = self.ofs + 1;
                return Ok(Some(()));
            }
            None => return Ok(None),
        }
    }

    pub fn force_step(&mut self) {
        self.ofs += 1;
    }

    pub fn steppable(&mut self,
                     freezer: &Freezer<Node<T, M, H>, H, B>)
                     -> io::Result<bool> {
        Ok(match freezer.get(&self.location)?.child(self.ofs + 1) {
               Some(_) => true,
               None => false,
           })
    }

    pub fn insert_loc(&mut self,
                      loc: Location<H>,
                      freezer: &mut Freezer<Node<T, M, H>, H, B>)
                      -> io::Result<()> {
        freezer.get(&loc)?
            .meta()
            .map(|meta| Child::new_node(loc, meta.into_owned()))
            .map(|child| self.insert(child, freezer));
        Ok(())
    }

    pub fn insert_after(&mut self,
                        child: Child<T, M, H>,
                        freezer: &mut Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<()> {
        let node = freezer.get_mut(&mut self.location)?;
        let len = node.len();
        node.insert(R::after(self.ofs, len), child);
        Ok(())
    }

    pub fn insert(&mut self,
                  child: Child<T, M, H>,
                  freezer: &mut Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<()> {
        let node = freezer.get_mut(&mut self.location)?;
        let len = node.len();
        node.insert(R::insert(self.ofs, len), child);
        Ok(())
    }

    pub fn insert_t(&mut self,
                    t: T,
                    divisor: usize,
                    freezer: &mut Freezer<Node<T, M, H>, H, B>)
                    -> io::Result<InsertResult> {
        let weight = t.weight() / divisor;
        let node = freezer.get_mut(&mut self.location)?;
        let len = node.len();
        let self_weight = node.insert_t(R::insert(self.ofs, len), t, divisor);

        if len == 0 {
            Ok(InsertResult::Ok)
        } else {
            match (self_weight, weight, R::from_end()) {
                // A
                (self_w, _, true) if self_w > 0 => {
                    Ok(InsertResult::Split(self_w))
                }
                // B
                (_, weight, false) if weight > 0 => {
                    Ok(InsertResult::Split(weight))
                }
                _ => Ok(InsertResult::Ok),
            }
        }
    }

    fn remove(&mut self,
              freezer: &mut Freezer<Node<T, M, H>, H, B>)
              -> io::Result<Option<Node<T, M, H>>> {
        {
            let node = freezer.get_mut(&mut self.location)?;
            match node.remove(self.ofs) {
                Some(Child::Node { location, .. }) => {
                    self.ofs = self.ofs.saturating_sub(1);
                    Ok(Some((freezer.get(&location)?.into_owned())))
                }
                _ => Ok(None),
            }
        }
    }

    pub fn remove_next(&mut self,
                       freezer: &mut Freezer<Node<T, M, H>, H, B>)
                       -> io::Result<Option<Node<T, M, H>>> {
        self.ofs += 1;
        match self.remove(freezer)? {
            None => {
                self.ofs = self.ofs.saturating_sub(1);
                Ok(None)
            }
            Some(removed) => Ok(Some(removed)),
        }
    }

    pub fn remove_t(&mut self,
                    divisor: usize,
                    freezer: &mut Freezer<Node<T, M, H>, H, B>)
                    -> io::Result<RemoveResult<T, H>> {
        Ok(freezer.get_mut(&mut self.location)?.remove_t(self.ofs, divisor))
    }

    pub fn split(&mut self,
                 freezer: &mut Freezer<Node<T, M, H>, H, B>)
                 -> io::Result<Child<T, M, H>> {
        let mut new;
        {
            let node = freezer.get_mut(&mut self.location)?;
            let len = node.len();
            new = node.split(R::after(self.ofs, len));
            R::order(node, &mut new);
        }
        let meta =
            new.meta().expect("split cannot produce empty nodes").into_owned();
        Ok(Child::new_node(freezer.put(new), meta))
    }

    pub fn merge(&mut self,
                 from: Node<T, M, H>,
                 freezer: &mut Freezer<Node<T, M, H>, H, B>)
                 -> io::Result<()> {
        Ok(freezer.get_mut(&mut self.location)?.merge(from))
    }

    pub fn find<S>(&mut self,
                   search: &mut S,
                   freezer: &Freezer<Node<T, M, H>, H, B>)
                   -> io::Result<Found<H>>
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        let node = freezer.get(&self.location)?;
        let len = node.len();

        if len == 0 {
            return Ok(Found::Between);
        }

        loop {
            let child = node.child(R::at(self.ofs, len));
            match child {
                Some(&Child::Node {
                          ref location,
                          ref meta,
                      }) => {
                    match search.select(meta.submeta()) {
                        Selection::Hit | Selection::Between => {
                            return Ok(Found::Node(location.clone()))
                        }
                        Selection::Miss => {
                            self.ofs += 1;
                        }
                    }
                }
                Some(&Child::Leaf(ref t)) => {
                    match search.select(Cow::Owned(S::from_t(t))) {
                        Selection::Hit => {
                            return Ok(Found::Hit);
                        }
                        Selection::Between => {
                            return Ok(Found::Between);
                        }
                        Selection::Miss => {
                            self.ofs += 1;
                        }
                    }
                }
                None => return Ok(Found::Miss),
            }
        }
    }

    pub fn concat(left: &Location<H>,
                  right: &Location<H>,
                  freezer: &mut Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<Level<T, M, R, H, B>> {
        let ofs;
        let new;
        {
            let lnode = freezer.get(left)?.into_owned();
            let rnode = freezer.get(right)?.into_owned();

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
        Ok(Level {
               ofs: ofs,
               location: freezer.put(new),
               _r: PhantomData,
           })
    }
}

impl<T, M, R, H, B> Level<T, M, R, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T> + Clone + Freeze<H>,
          H: CryptoHash,
          Node<T, M, H>: Clone,
          B: Backend<Node<T, M, H>, H> + Clone
{
    pub fn reverse<O>(&self,
                      freezer: &Freezer<Node<T, M, H>, H, B>)
                      -> io::Result<Level<T, M, O, H, B>>
        where O: Relative + Opposite<R>,
              R: Relative + Opposite<O>,
              H: CryptoHash
    {
        let len = freezer.get(&self.location)?.len();
        Ok(Level {
               ofs: len - self.ofs - 1,
               location: self.location.clone(),
               _r: PhantomData,
           })
    }
}
