use std::fmt;
use std::cmp;

use std::borrow::Cow;

use Val;

use tree::node::{Node, Child, RemoveResult, InsertResult};
use stash::{Stash, RelStash, Location};
use tree::level::{Level, Relative, Opposite, Beginning, End};
use meta::{Meta, Select, Selection, Found, SubMeta};

use html::Html;

pub struct Branch<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    levels: Vec<Level<T, M, R>>,
}

pub enum BranchResult<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    Miss,
    Hit(Branch<T, M, R>),
    Between(Branch<T, M, R>),
}

impl<T, M, R> Clone for Branch<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
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

impl<T, M, R> Branch<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    pub fn new(root: Location<T, M>) -> Branch<T, M, R> {
        Branch { levels: vec![Level::new(root)] }
    }

    pub fn first(root: Location<T, M>, stash: &Stash<T, M>) -> Self {
        let mut branch = Branch::new(root);
        branch.extend(stash);
        branch
    }

    fn empty(&self, stash: &Stash<T, M>) -> bool {
        stash.get(self.root()).len() == 0
    }

    fn from_levels(levels: Vec<Level<T, M, R>>) -> Branch<T, M, R> {
        Branch { levels: levels }
    }

    fn level_meta<'a>(&self,
                      from_bottom: usize,
                      stash: &'a Stash<T, M>)
                      -> Option<Cow<'a, M>> {
        // TODO: re-use cached meta when available
        if from_bottom >= self.depth() {
            None
        } else {
            let at = self.depth() - from_bottom - 1;
            let node = stash.get(self.levels[at].location());
            node.meta()
        }
    }

    pub fn new_full<S>(root: Location<T, M>,
                       search: &mut S,
                       stash: &Stash<T, M>)
                       -> BranchResult<T, M, R>
        where S: Select<T> + Meta<T>,
              M: SubMeta<S>
    {
        let mut branch = Self::new(root);
        match branch.find_full(search, stash) {
            Selection::Miss => BranchResult::Miss,
            Selection::Hit => BranchResult::Hit(branch),
            Selection::Between => BranchResult::Between(branch),
        }

        //panic!()
    }

    fn find<S>(&mut self, search: &mut S, stash: &Stash<T, M>) -> Found<T, M>
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        self.bottom_mut().find(search, stash)
    }

    pub fn find_full<S>(&mut self,
                        search: &mut S,
                        stash: &Stash<T, M>)
                        -> Selection
        where S: Meta<T> + Select<T>,
              M: SubMeta<S>
    {
        loop {
            match self.find(search, stash) {
                Found::Hit => return Selection::Hit,
                Found::Between => return Selection::Between,
                Found::Node(location) => {
                    self.push(location);
                }
                Found::Miss => {
                    match self.steppable_depth(stash) {
                        Some(depth) => {
                            let trim = self.depth() - depth - 1;
                            self.trim(trim);
                        }
                        None => {
                            return Selection::Miss;
                        }
                    }
                }
            }
        }
    }

    pub fn extend(&mut self, stash: &Stash<T, M>) {
        loop {
            if let Some(&Child::Node { location, .. }) =
                self.bottom().child(stash) {
                self.push(location);
            } else {
                break;
            }
        }
    }

    fn depth(&self) -> usize {
        self.levels.len()
    }

    pub fn leaf<'a>(&self, stash: &'a Stash<T, M>) -> Option<&'a T> {
        if let Some(&Child::Leaf(ref t)) = self.bottom().child(stash) {
            Some(t)
        } else {
            None
        }
    }

    pub fn leaf_mut<'a>(&mut self,
                        stash: &'a mut Stash<T, M>)
                        -> Option<&'a mut T> {
        if let Some(&mut Child::Leaf(ref mut t)) =
            self.bottom_mut().child_mut(stash) {
            Some(t)
        } else {
            None
        }
    }

    fn push(&mut self, loc: Location<T, M>) {
        let depth = self.bottom().location().depth;
        self.levels.push(Level::new(loc.relative(depth)));
    }

    pub fn root(&self) -> Location<T, M> {
        self.levels[0].location()
    }

    fn bottom(&self) -> &Level<T, M, R> {
        self.levels.last().expect("branch len always > 0")
    }

    pub fn bottom_mut(&mut self) -> &mut Level<T, M, R> {
        self.levels.last_mut().expect("branch len always > 0")
    }

    fn top(&mut self) -> &Level<T, M, R> {
        self.levels.first().expect("branch len always > 0")
    }

    fn find_first_root(&mut self, stash: &mut Stash<T, M>) {
        loop {
            let root = self.root();
            let node = stash.get(root);
            match (node.len(), node.child(0)) {
                (1, Some(&Child::Node { location, .. })) => {
                    self.levels.truncate(1);
                    *self.levels[0].location_mut() = location;
                }
                _ => return,
            }
        }
    }

    fn trim(&mut self, amount: usize) {
        for _ in 0..amount {
            self.levels.pop();
        }
    }

    pub fn propagate(&mut self, stash: &mut Stash<T, M>) {
        for i in 0..self.depth() - 1 {
            let at = self.depth() - i - 2;
            let below = self.levels[at + 1].location();
            self.levels[at].update_child(below, stash);
        }
    }

    fn propagate_insert(&mut self, stash: &mut Stash<T, M>) {
        for i in 0..self.depth() - 1 {
            let at = self.depth() - i - 2;
            let below = self.levels[at + 1].location();
            self.levels[at].insert_loc(below, stash);
        }
    }

    pub fn insert(&mut self, t: T, divisor: usize, stash: &mut Stash<T, M>) {
        match self.bottom_mut().insert_t(t, divisor, stash) {
            InsertResult::Ok => (),
            InsertResult::Split(depth) => {
                self.ensure_depth(depth + 1, stash);
                self.split(depth, stash);
            }
        }
        self.propagate(stash);
    }

    // Gotcha: Updates value in place, without re-balancing
    // used for maps, which are only balanced on key.
    pub fn update(&mut self, t: T, stash: &mut Stash<T, M>) {
        self.leaf_mut(stash).map(|l| *l = t);
    }

    pub fn rebalance(&mut self,
                     old_weight: usize,
                     new_weight: usize,
                     stash: &mut Stash<T, M>) {
        self.propagate(stash);
        if new_weight != old_weight {
            if old_weight > 0 {
                self.merge(old_weight, stash);
                self.propagate(stash);
            }

            if new_weight > 0 {
                self.ensure_depth(new_weight + 1, stash);
                self.split(new_weight, stash);
                self.propagate(stash);
            }
            self.find_first_root(stash);
        }
    }

    pub fn remove(&mut self,
                  divisor: usize,
                  stash: &mut Stash<T, M>)
                  -> Option<T> {
        match self.bottom_mut().remove_t(divisor, stash) {
            RemoveResult::Void => None,
            RemoveResult::Ok(t) => {
                self.propagate(stash);
                Some(t)
            }
            RemoveResult::Final(t) => {
                self.propagate(stash);
                self.find_first_root(stash);
                Some(t)
            }
            RemoveResult::Merge { t, depth } => {
                self.merge(depth, stash);
                self.propagate(stash);
                self.find_first_root(stash);
                Some(t)
            }
        }
    }

    fn merge(&mut self, depth: usize, stash: &mut Stash<T, M>) {
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
            if let Some(removed) = self.levels[merge_top].remove_next(stash) {
                self.levels[merge_top + 1].merge(removed, stash);
            }
        }
    }

    fn ensure_depth(&mut self, depth: usize, stash: &mut Stash<T, M>) {
        while self.depth() < depth {
            let top_loc = self.top().location().clone();
            // singleton node has the same meta as its child
            let meta = stash.get(top_loc)
                .meta()
                .expect("root never empty here")
                .into_owned();
            let new_root = Node::single(Child::new_node(top_loc, meta));
            let root_loc = stash.put(new_root);
            self.levels.insert(0, Level::new(root_loc));
        }
    }

    fn split(&mut self, depth: usize, stash: &mut Stash<T, M>) {
        debug_assert!(depth > 0);
        let len = self.levels.len();
        for i in 0..depth {
            let child = self.levels[len - i - 1].split(stash);
            self.levels[len - i - 2].insert_after(child, stash);
        }
    }

    fn steppable_depth(&mut self, stash: &Stash<T, M>) -> Option<usize> {
        for i in 1..self.levels.len() {
            let depth = self.levels.len() - i - 1;
            if self.levels[depth].steppable(stash) {
                return Some(depth);
            }
        }
        None
    }

    pub fn step(&mut self, stash: &Stash<T, M>) -> Option<()> {
        self.bottom_mut().step(stash).or_else(|| {
            if self.depth() < 2 {
                self.bottom_mut().force_step();
                return None;
            }
            let mut depth = self.depth() - 2;
            loop {
                match self.levels[depth].step(stash) {
                    Some(_) => {
                        let trim = self.depth() - depth - 1;
                        self.trim(trim);
                        self.extend(stash);
                        return Some(());
                    }
                    None => {
                        if depth == 0 {
                            // nothing steppable, move to end-condition
                            self.bottom_mut().force_step();
                            return None;
                        } else {
                            depth -= 1;
                        }
                    }
                }
            }
        })
    }

    pub fn left(&self, stash: &mut Stash<T, M>) -> Branch<T, M, End> {
        let mut levels: Vec<Level<T, M, End>> = vec![];

        for i in 0..self.levels.len() {
            if let Some(loc) = self.levels[i].left(stash) {
                levels.push(Level::new(loc));
            } else {
                if levels.len() > 0 {
                    levels.push(Level::new(stash.put(Node::new())));
                }
            }
        }

        if levels.len() > 0 {
            let mut branch = Branch::from_levels(levels);
            branch.propagate_insert(stash);
            Branch::first(branch.root(), stash)
        } else {
            Branch::new(stash.put(Node::new()))
        }
    }

    pub fn right(&self, stash: &mut Stash<T, M>) -> Branch<T, M, Beginning> {
        let mut levels: Vec<Level<T, M, Beginning>> = vec![];

        for i in 0..self.levels.len() {
            let i = self.levels.len() - i - 1;
            levels.insert(0, Level::new(self.levels[i].right(stash)));
        }

        let mut branch = Branch::from_levels(levels);
        branch.propagate(stash);
        branch.find_first_root(stash);
        branch.extend(stash);
        branch
    }
}

impl<'a, T, M, R> Branch<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    pub fn skip_equal<E>(&mut self, other: &mut Self, stash: &'a Stash<T, M>)
        where M: SubMeta<E>,
              E: Meta<T> + PartialEq
    {
        let mut depth = 0;

        while self.level_meta(depth, stash).map(|m| {
                                                    (*(*m).submeta()).clone() as
                                                    E
                                                }) ==
              other.level_meta(depth, stash).map(|m| {
                                                     (*(*m).submeta())
                                                         .clone() as
                                                     E
                                                 }) {
            depth += 1;
        }

        if depth > 0 {
            // how many layers can we skip over given the depth?
            let depth = cmp::min(depth, self.depth() - 1);
            let depth = cmp::min(depth, other.depth() - 1);

            self.trim(depth);
            other.trim(depth);

            self.step(stash);
            other.step(stash);

            self.extend(stash);
            other.extend(stash);
        }

        // at leaf level
        match (self.leaf(stash).map(|t| E::from_t(t)),
               other.leaf(stash).map(|t| E::from_t(t))) {
            (Some(ref a), Some(ref b)) if a == b => {
                self.step(stash);
                other.step(stash);
            }
            _ => return,
        }
    }
}

impl<T, M, R> Branch<T, M, R>
    where T: Val,
          M: Meta<T>,
          R: Relative
{
    pub fn reverse<O>(&self, stash: &Stash<T, M>) -> Branch<T, M, O>
        where O: Relative + Opposite<R>,
              R: Opposite<O>
    {
        Branch::first(self.root(), stash)
    }

    pub fn concat<O>(&self,
                     right: &Branch<T, M, O>,
                     divisor: usize,
                     stash: &mut Stash<T, M>)
                     -> Branch<T, M, R>
        where O: Relative + Opposite<R>,
              R: Opposite<O>
    {
        if self.empty(stash) {
            return right.reverse(stash);
        } else if right.empty(stash) {
            return self.clone();
        }

        let self_weight = self.leaf(stash).map(|l| l.weight() / divisor);

        let mut l_self = self.levels.iter().rev();
        let mut l_right = right.levels.iter().rev();

        let mut levels = vec![];

        loop {
            match (l_self.next(), l_right.next()) {
                (Some(s), Some(r)) => {
                    levels.insert(0,
                                  Level::concat(s.location(),
                                                r.location(),
                                                stash))
                }
                (Some(s), None) => levels.insert(0, s.clone()),
                (None, Some(r)) => {
                    levels.insert(0, r.reverse(stash));
                }
                (None, None) => break,
            }
        }

        let mut branch = Branch::from_levels(levels);
        if let Some(weight) = self_weight {
            if weight > 0 {
                branch.ensure_depth(weight + 1, stash);
                branch.split(weight, stash);
            }
        }
        branch.propagate(stash);
        let branch = Branch::first(branch.root(), stash);
        branch
    }

    pub fn weight(&self, divisor: usize, stash: &Stash<T, M>) -> usize {
        cmp::max(self.depth() - 1, self.bottom().weight(divisor, stash))
    }
}

// impl<T, M, R> Branch<T, M, R>
//     where T: Val,
//           M: Meta<T> + Into<&KeyMeta<T>>,
//           R: Relative
// {
//     pub fn key(&self, stash: &Stash<T, M>) -> Option<T::Key> {
//         self.leaf(stash).map(|leaf| )
//     }
// }

impl<T, M, R> Html<T, M> for Branch<T, M, R>
    where T: Val + fmt::Debug,
          M: Meta<T>,
          R: Relative
{
    fn _html(&self, stash: RelStash<T, M>) -> String {
        let mut s = String::from("<table>");
        for level in &self.levels {
            s += &format!("<tr>{}</tr>", level._html(stash))
        }
        s.push_str("</table>");
        s
    }
}
