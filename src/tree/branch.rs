use std::cmp;
use std::io;

use std::borrow::Cow;

use freezer::{Freeze, Location, CryptoHash, Backend};
use freezer::freezer::Freezer;

use tree::weight::Weight;
use tree::node::{Node, Child, RemoveResult, InsertResult};
use tree::level::{Level, Relative, Opposite, Beginning, End};
use meta::{Meta, Select, Selection, Found, SubMeta};

pub struct Branch<T, M, R, H, B>
    where H: CryptoHash
{
    levels: Vec<Level<T, M, R, H, B>>,
}

pub enum BranchResult<T, M, R, H, B>
    where H: CryptoHash
{
    Miss,
    Hit(Branch<T, M, R, H, B>),
    Between(Branch<T, M, R, H, B>),
}

impl<T, M, R, H, B> Clone for Branch<T, M, R, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T>,
          R: Relative,
          H: CryptoHash
{
    fn clone(&self) -> Self {
        Branch {
            levels: self.levels
                .iter()
                .map(|r| r.clone())
                .collect(),
        }
    }
}

impl<T, M, R, H, B> Branch<T, M, R, H, B>
    where T: Weight + Freeze<H>,
          H: CryptoHash,
          M: Meta<T> + Clone,
          R: Relative,
          B: Backend<Node<T, M, H>, H>
{
    pub fn new(root: Location<H>) -> Branch<T, M, R, H, B> {
        Branch { levels: vec![Level::new(root)] }
    }

    pub fn first(root: Location<H>,
                 freezer: &Freezer<Node<T, M, H>, H, B>)
                 -> io::Result<Self> {
        let mut branch = Branch::new(root);
        branch.extend(freezer)?;
        Ok(branch)
    }

    fn empty(&self,
             freezer: &Freezer<Node<T, M, H>, H, B>)
             -> io::Result<bool> {
        Ok(freezer.get(self.root())?.len() == 0)
    }

    fn from_levels(levels: Vec<Level<T, M, R, H, B>>) -> Branch<T, M, R, H, B> {
        Branch { levels: levels }
    }

    fn level_meta<'a>(&self,
                      from_bottom: usize,
                      freezer: &'a Freezer<Node<T, M, H>, H, B>)
                      -> io::Result<Option<Cow<'a, M>>> {
        // TODO: re-use cached meta when available
        if from_bottom >= self.depth() {
            Ok(None)
        } else {
            let at = self.depth() - from_bottom - 1;
            let node = freezer.get(self.levels[at].location())?;
            match node {
                Cow::Owned(node) => {
                    Ok(node.into_meta().map(|meta| Cow::Owned(meta)))
                }
                Cow::Borrowed(node) => Ok(node.meta()),
            }
        }
    }

    pub fn new_full<S>(root: Location<H>,
                       search: &mut S,
                       freezer: &Freezer<Node<T, M, H>, H, B>)
                       -> io::Result<BranchResult<T, M, R, H, B>>
        where S: Select<T> + Meta<T>,
              M: SubMeta<S>
    {
        let mut branch = Self::new(root);
        Ok(match branch.find_full(search, freezer)? {
               Selection::Miss => BranchResult::Miss,
               Selection::Hit => BranchResult::Hit(branch),
               Selection::Between => BranchResult::Between(branch),
           })
    }

    fn find<S>(&mut self,
               search: &mut S,
               freezer: &Freezer<Node<T, M, H>, H, B>)
               -> io::Result<Found<H>>
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        self.bottom_mut().find(search, freezer)
    }

    pub fn find_full<S>(&mut self,
                        search: &mut S,
                        freezer: &Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<Selection>
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        loop {
            match self.find(search, freezer)? {
                Found::Hit => return Ok(Selection::Hit),
                Found::Between => return Ok(Selection::Between),
                Found::Node(location) => {
                    self.push(location);
                }
                Found::Miss => {
                    match self.steppable_depth(freezer)? {
                        Some(depth) => {
                            let trim = self.depth() - depth - 1;
                            self.trim(trim);
                        }
                        None => {
                            return Ok(Selection::Miss);
                        }
                    }
                }
            }
        }
    }

    pub fn extend(&mut self,
                  freezer: &Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<()> {
        loop {
            match self.bottom().child(freezer)? {
                Some(Cow::Owned(Child::Node { location, .. })) => {
                    self.push(location)
                }
                Some(Cow::Borrowed(&Child::Node { ref location, .. })) => {
                    self.push(location.clone())
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn depth(&self) -> usize {
        self.levels.len()
    }

    pub fn leaf<'a>(&self,
                    freezer: &'a Freezer<Node<T, M, H>, H, B>)
                    -> io::Result<Option<Cow<'a, T>>> {
        match self.bottom().child(freezer)? {
            Some(Cow::Owned(Child::Leaf(t))) => Ok(Some(Cow::Owned(t))),
            Some(Cow::Borrowed(&Child::Leaf(ref t))) => {
                Ok(Some(Cow::Borrowed(t)))
            }
            _ => Ok(None),
        }
    }

    pub fn leaf_mut<'a>(&mut self,
                        freezer: &'a mut Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<Option<&'a mut T>> {
        if let Some(&mut Child::Leaf(ref mut t)) =
            self.bottom_mut().child_mut(freezer)? {
            Ok(Some(t))
        } else {
            Ok(None)
        }
    }

    fn push(&mut self, loc: Location<H>) {
        self.levels.push(Level::new(loc));
    }

    pub fn root(&self) -> &Location<H> {
        self.levels[0].location()
    }

    pub fn into_root(self) -> Location<H> {
        let Branch { mut levels, .. } = self;
        levels.remove(0).into_location()
    }

    fn bottom(&self) -> &Level<T, M, R, H, B> {
        self.levels.last().expect("branch len always > 0")
    }

    pub fn bottom_mut(&mut self) -> &mut Level<T, M, R, H, B> {
        self.levels.last_mut().expect("branch len always > 0")
    }

    fn top(&mut self) -> &Level<T, M, R, H, B> {
        self.levels.first().expect("branch len always > 0")
    }

    fn find_first_root(&mut self,
                       freezer: &mut Freezer<Node<T, M, H>, H, B>)
                       -> io::Result<()> {
        loop {
            let root = self.root().clone();
            let node = freezer.get(&root)?;
            match (node.len(), node.child(0)) {
                (1, Some(&Child::Node { ref location, .. })) => {
                    self.levels.truncate(1);
                    *self.levels[0].location_mut() = location.clone();
                }
                _ => return Ok(()),
            }
        }
    }

    fn trim(&mut self, amount: usize) {
        for _ in 0..amount {
            self.levels.pop();
        }
    }

    pub fn propagate(&mut self,
                     freezer: &mut Freezer<Node<T, M, H>, H, B>)
                     -> io::Result<()> {
        for i in 0..self.depth() - 1 {
            let at = self.depth() - i - 2;
            let below = self.levels[at + 1].location().clone();
            self.levels[at].update_child(&below, freezer)?;
        }
        Ok(())
    }

    fn propagate_insert(&mut self,
                        freezer: &mut Freezer<Node<T, M, H>, H, B>)
                        -> io::Result<()> {
        for i in 0..self.depth() - 1 {
            let at = self.depth() - i - 2;
            let below = self.levels[at + 1].location().clone();
            self.levels[at].insert_loc(below, freezer)?;
        }
        Ok(())
    }

    pub fn insert(&mut self,
                  t: T,
                  divisor: usize,
                  freezer: &mut Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<()> {
        match self.bottom_mut().insert_t(t, divisor, freezer)? {
            InsertResult::Ok => (),
            InsertResult::Split(depth) => {
                self.ensure_depth(depth + 1, freezer)?;
                self.split(depth, freezer)?;
            }
        }
        self.propagate(freezer)
    }

    // Gotcha: Updates value in place, without re-balancing
    // used for maps, who are only balanced on key.
    pub fn update(&mut self,
                  t: T,
                  freezer: &mut Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<()> {
        self.leaf_mut(freezer)?.map(|l| *l = t);
        self.propagate(freezer)?;
        Ok(())
    }

    // pub fn rebalance(&mut self,
    //                  old_weight: usize,
    //                  new_weight: usize,
    //                  freezer: &mut Freezer<Node<T, M, H>, H, B>) {
    //     self.propagate(freezer)?;
    //     if new_weight != old_weight {
    //         if old_weight > 0 {
    //             self.merge(old_weight, freezer)?;
    //             self.propagate(freezer)?;
    //         }

    //         if new_weight > 0 {
    //             self.ensure_depth(new_weight + 1, freezer)?;
    //             self.split(new_weight, freezer)?;
    //             self.propagate(freezer)?;
    //         }
    //         self.find_first_root(freezer)?;
    //     }
    // }

    pub fn remove(&mut self,
                  divisor: usize,
                  freezer: &mut Freezer<Node<T, M, H>, H, B>)
                  -> io::Result<Option<T>> {
        match self.bottom_mut().remove_t(divisor, freezer)? {
            RemoveResult::Void => Ok(None),
            RemoveResult::Ok(t) => {
                self.propagate(freezer)?;
                Ok(Some(t))
            }
            RemoveResult::Final(t) => {
                self.propagate(freezer)?;
                self.find_first_root(freezer)?;
                Ok(Some(t))
            }
            RemoveResult::Merge { t, depth, .. } => {
                self.merge(depth, freezer)?;
                self.propagate(freezer)?;
                self.find_first_root(freezer)?;
                Ok(Some(t))
            }
        }
    }

    fn merge(&mut self,
             depth: usize,
             freezer: &mut Freezer<Node<T, M, H>, H, B>)
             -> io::Result<()> {
        // levels: [a b c d] merge depth 2
        //            |/|/
        //            1 2

        // levels: [a b c d] merge depth 3 or greater
        //          |/|/|/
        //          1 2 3
        let mergers = cmp::min(depth, self.depth() - 1);
        let offset = self.depth() - mergers - 1;

        for i in 0..mergers {
            let merge_top = offset + i;
            if let Some(removed) =
                self.levels[merge_top].remove_next(freezer)? {
                self.levels[merge_top + 1].merge(removed, freezer)?;
            }
        }
        Ok(())
    }

    fn ensure_depth(&mut self,
                    depth: usize,
                    freezer: &mut Freezer<Node<T, M, H>, H, B>)
                    -> io::Result<()> {
        while self.depth() < depth {
            let top_loc = self.top().location().clone();
            // singleton node has the same meta as its child
            let meta = freezer.get(&top_loc)?
                .meta()
                .expect("root never empty here")
                .into_owned();
            let new_root = Node::single(Child::new_node(top_loc, meta));
            let root_loc = freezer.put(new_root);
            self.levels.insert(0, Level::new(root_loc));
        }
        Ok(())
    }

    fn split(&mut self,
             depth: usize,
             freezer: &mut Freezer<Node<T, M, H>, H, B>)
             -> io::Result<()> {
        debug_assert!(depth > 0);
        let len = self.levels.len();
        for i in 0..depth {
            let child = self.levels[len - i - 1].split(freezer)?;
            self.levels[len - i - 2].insert_after(child, freezer)?;
        }
        Ok(())
    }

    fn steppable_depth(&mut self,
                       freezer: &Freezer<Node<T, M, H>, H, B>)
                       -> io::Result<Option<usize>> {
        for i in 1..self.levels.len() {
            let depth = self.levels.len() - i - 1;
            if self.levels[depth].steppable(freezer)? {
                return Ok(Some(depth));
            }
        }
        Ok(None)
    }

    pub fn step(&mut self,
                freezer: &Freezer<Node<T, M, H>, H, B>)
                -> io::Result<Option<()>> {
        if self.bottom_mut()
               .step(freezer)?
               .is_some() {
            return Ok(Some(()));
        } else {
            if self.depth() < 2 {
                self.bottom_mut().force_step();
                return Ok(None);
            }
            let mut depth = self.depth() - 2;
            loop {
                match self.levels[depth].step(freezer)? {
                    Some(_) => {
                        let trim = self.depth() - depth - 1;
                        self.trim(trim);
                        self.extend(freezer)?;
                        return Ok(Some(()));
                    }
                    None => {
                        if depth == 0 {
                            // nothing steppable, move to end-condition
                            self.bottom_mut().force_step();
                            return Ok(None);
                        } else {
                            depth -= 1;
                        }
                    }
                }
            }
        }
    }

    pub fn left(&self,
                freezer: &mut Freezer<Node<T, M, H>, H, B>)
                -> io::Result<Branch<T, M, End, H, B>> {
        let mut levels: Vec<Level<T, M, End, H, B>> = vec![];

        for i in 0..self.levels.len() {
            if let Some(loc) = self.levels[i].left(freezer)? {
                levels.push(Level::new(loc));
            } else {
                if levels.len() > 0 {
                    levels.push(Level::new(freezer.put(Node::new())));
                }
            }
        }

        if levels.len() > 0 {
            let mut branch = Branch::from_levels(levels);
            branch.propagate_insert(freezer)?;
            Branch::first(branch.into_root(), freezer)
        } else {
            Ok(Branch::new(freezer.put(Node::new())))
        }
    }

    pub fn right(&self,
                 freezer: &mut Freezer<Node<T, M, H>, H, B>)
                 -> io::Result<Branch<T, M, Beginning, H, B>> {
        let mut levels: Vec<Level<T, M, Beginning, H, B>> = vec![];

        for i in 0..self.levels.len() {
            let i = self.levels.len() - i - 1;
            levels.insert(0, Level::new(self.levels[i].right(freezer)?));
        }

        let mut branch = Branch::from_levels(levels);
        branch.propagate(freezer)?;
        branch.find_first_root(freezer)?;
        branch.extend(freezer)?;
        Ok(branch)
    }
}

impl<'a, T, M, R, H, B> Branch<T, M, R, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T>,
          R: Relative,
          H: CryptoHash
{
    pub fn skip_equal<E>(&mut self,
                         other: &mut Self,
                         freezer: &'a Freezer<Node<T, M, H>, H, B>)
                         -> io::Result<()>
        where M: SubMeta<E>,
              E: Meta<T> + PartialEq,
              B: Backend<Node<T, M, H>, H>
    {
        let mut depth = 0;

        while {
                  match (self.level_meta(depth, freezer)?,
                         other.level_meta(depth, freezer)?) {
                      (Some(a), Some(b)) => a.submeta() == b.submeta(),
                      _ => false,
                  }
              } {
            depth += 1;
        }

        if depth > 0 {
            // how many layers can we skip over given the depth?
            let depth = cmp::min(depth, self.depth() - 1);
            let depth = cmp::min(depth, other.depth() - 1);

            self.trim(depth);
            other.trim(depth);

            self.step(freezer)?;
            other.step(freezer)?;

            self.extend(freezer)?;
            other.extend(freezer)?;
        }

        // at leaf level
        match (self.leaf(freezer)?.map(|t| E::from_t(&*t)),
               other.leaf(freezer)?.map(|t| E::from_t(&*t))) {
            (Some(ref a), Some(ref b)) if a == b => {
                self.step(freezer)?;
                other.step(freezer)?;
                Ok(())
            }
            _ => return Ok(()),
        }
    }
}

impl<T, M, R, H, B> Branch<T, M, R, H, B>
    where T: Weight + Freeze<H> + Clone,
          M: Meta<T>,
          R: Relative,
          H: CryptoHash,
          B: Backend<Node<T, M, H>, H>
{
    pub fn reverse<O>(&self,
                      freezer: &Freezer<Node<T, M, H>, H, B>)
                      -> io::Result<Branch<T, M, O, H, B>>
        where O: Relative + Opposite<R>,
              R: Relative + Opposite<O>
    {
        Branch::first(self.root().clone(), freezer)
    }

    pub fn concat<O>(&self,
                     right: &Branch<T, M, O, H, B>,
                     divisor: usize,
                     freezer: &mut Freezer<Node<T, M, H>, H, B>)
                     -> io::Result<Branch<T, M, R, H, B>>
        where O: Relative + Opposite<R>,
              R: Opposite<O>
    {
        if self.empty(freezer)? {
            return right.reverse(freezer);
        } else if right.empty(freezer)? {
            return Ok(self.clone());
        }

        let self_weight = self.leaf(freezer)?.map(|l| l.weight() / divisor);

        let mut l_self = self.levels.iter().rev();
        let mut l_right = right.levels.iter().rev();

        let mut levels = vec![];

        loop {
            match (l_self.next(), l_right.next()) {
                (Some(s), Some(r)) => {
                    levels.insert(0,
                                  Level::concat(s.location(),
                                                r.location(),
                                                freezer)?)
                }
                (Some(s), None) => levels.insert(0, s.clone()),
                (None, Some(r)) => {
                    levels.insert(0, r.reverse(freezer)?);
                }
                (None, None) => break,
            }
        }

        let mut branch = Branch::from_levels(levels);
        if let Some(weight) = self_weight {
            if weight > 0 {
                branch.ensure_depth(weight + 1, freezer)?;
                branch.split(weight, freezer)?;
            }
        }
        branch.propagate(freezer)?;
        let branch = Branch::first(branch.root().clone(), freezer)?;
        Ok(branch)
    }
}
