use std::sync::Arc;
use std::marker::PhantomData;
use std::fmt;
use std::mem;

use Val;
use tree::node::{Node, Child};
use meta::Meta;
use html::Html;

pub struct Stash<T, M>
    where T: Val,
          M: Meta<T>
{
    uniq: Vec<Node<T, M>>,
    shared: Vec<Arc<Vec<Node<T, M>>>>,
}

pub struct RelStash<'a, T, M>
    where T: 'a + Val,
          M: 'a + Meta<T>
{
    depth: usize,
    stash: &'a Stash<T, M>,
}

impl<'a, T, M> RelStash<'a, T, M>
    where T: 'a + Val,
          M: 'a + Meta<T>
{
    pub fn get(&self, location: Location<T, M>) -> &'a Node<T, M> {
        self.stash.get(location.relative(self.depth))
    }

    pub fn depth_mut(&mut self) -> &mut usize {
        &mut self.depth
    }

    pub fn relative_n(&self, to: usize) -> RelStash<T, M> {
        RelStash {
            stash: self.stash,
            depth: self.depth + to,
        }
    }

    pub fn relative(&self, to: Location<T, M>) -> RelStash<T, M> {
        RelStash {
            stash: self.stash,
            depth: self.depth + to.depth,
        }
    }

    pub fn absolute(&self, location: Location<T, M>) -> Location<T, M> {
        Location {
            ofs: location.ofs,
            depth: location.depth + self.depth,
            _t: PhantomData,
            _m: PhantomData,
        }
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn top(&self) -> RelStash<T, M> {
        RelStash {
            depth: 0,
            stash: self.stash,
        }
    }
}

impl<'a, T, M> RelStash<'a, T, M>
    where T: 'a + Val + fmt::Debug,
          M: 'a + Meta<T>
{
    pub fn _html(&self) -> String {
        self.stash._html(self.depth)
    }
}

impl<'a, T, M> Clone for RelStash<'a, T, M>
    where T: 'a + Val,
          M: 'a + Meta<T>
{
    fn clone(&self) -> Self {
        RelStash {
            depth: self.depth,
            stash: self.stash,
        }
    }
}

impl<'a, T, M> Copy for RelStash<'a, T, M>
    where T: 'a + Val,
          M: 'a + Meta<T>
{
}

pub struct Location<T, M>
    where T: Val,
          M: Meta<T>
{
    pub ofs: usize,
    pub depth: usize,
    _t: PhantomData<T>,
    _m: PhantomData<M>,
}

impl<T, M> Location<T, M>
    where T: Val,
          M: Meta<T>
{
    pub fn new(ofs: usize, depth: usize) -> Self {
        Location {
            ofs: ofs,
            depth: depth,
            _t: PhantomData,
            _m: PhantomData,
        }
    }

    pub fn relative(&self, depth: usize) -> Self {
        Location {
            ofs: self.ofs,
            depth: self.depth + depth,
            _t: PhantomData,
            _m: PhantomData,
        }
    }
}

impl<T, M> Clone for Location<T, M>
    where T: Val,
          M: Meta<T>
{
    fn clone(&self) -> Self {
        Location {
            ofs: self.ofs,
            depth: self.depth,
            _t: PhantomData,
            _m: PhantomData,
        }
    }
}

impl<T, M> Copy for Location<T, M>
    where T: Val,
          M: Meta<T>
{
}

impl<T, M> Stash<T, M>
    where T: Val,
          M: Meta<T>
{
    pub fn new() -> Self {
        Stash {
            uniq: vec![],
            shared: vec![],
        }
    }

    pub fn top(&self) -> RelStash<T, M> {
        RelStash {
            stash: self,
            depth: 0,
        }
    }

    pub fn relative(&self, to: Location<T, M>) -> RelStash<T, M> {
        RelStash {
            stash: self,
            depth: to.depth,
        }
    }

    pub fn relative_n(&self, to: usize) -> RelStash<T, M> {
        RelStash {
            stash: self,
            depth: to,
        }
    }

    pub fn put(&mut self, node: Node<T, M>) -> Location<T, M> {
        let idx = self.uniq.len();
        self.uniq.push(node);
        Location::new(idx, 0)
    }

    pub fn get(&self, loc: Location<T, M>) -> &Node<T, M> {
        let Location { ofs, depth, .. } = loc;
        if depth == 0 {
            &self.uniq[ofs]
        } else {
            &self.shared[depth - 1][ofs]
        }
    }

    pub fn get_clone(&self, loc: Location<T, M>) -> Node<T, M> {
        let Location { ofs, depth, .. } = loc;
        if depth == 0 {
            self.uniq[ofs].clone()
        } else {
            let mut clone = self.shared[depth - 1][ofs].clone();
            clone.relativize(depth);
            clone
        }
    }

    pub fn get_mut(&mut self, loc: &mut Location<T, M>) -> &mut Node<T, M> {
        let Location {
            ref mut ofs,
            ref mut depth,
            ..
        } = *loc;
        if *depth == 0 {
            &mut self.uniq[*ofs]
        } else {
            let mut node = self.shared[*depth - 1][*ofs].clone();
            node.relativize(*depth);

            let idx = self.uniq.len();
            self.uniq.push(node);

            *depth = 0;
            *ofs = idx;

            &mut self.uniq[idx]
        }
    }

    pub fn remove(&mut self, loc: Location<T, M>) -> Node<T, M> {
        let Location { ofs, depth, .. } = loc;
        if depth == 0 {
            mem::replace(&mut self.uniq[ofs], Node::new())
        } else {
            self.shared[depth - 1][ofs].clone()
        }
    }

    pub fn split(&mut self, root: &mut Location<T, M>) -> (Self, Self) {
        let _ = self.clone_mut(root);
        let a = Stash {
            uniq: vec![],
            shared: self.shared.clone(),
        };
        let b = Stash {
            uniq: vec![],
            shared: self.shared.clone(),
        };
        (a, b)
    }

    // Merges the two stashes
    pub fn merge(&mut self,
                 root: &mut Location<T, M>,
                 other_root: &mut Location<T, M>,
                 other: &mut Self)
                 -> Self {
        // clear uniq
        let _ = self.clone_mut(root);
        let _ = other.clone_mut(other_root);

        let ofs = self.shared.len();
        self.shared.append(&mut other.shared);

        // offset root in other
        *other_root = other_root.relative(ofs);
        // clone shared arcs
        other.shared = self.shared.clone();

        Stash {
            uniq: vec![],
            shared: self.shared.clone(),
        }
    }

    pub fn clone_mut(&mut self, root: &mut Location<T, M>) -> Self {
        let Location { ref mut depth, .. } = *root;
        if *depth == 0 {
            let uniq = mem::replace(&mut self.uniq, vec![]);
            self.shared.insert(0, Arc::new(uniq));
            *depth += 1;
        }
        Stash {
            uniq: vec![],
            shared: self.shared.clone(),
        }
    }
}

impl<T, M> Stash<T, M>
    where T: Val + fmt::Debug,
          M: Meta<T>
{
    pub fn _html(&self, hilight_depth: usize) -> String {
        let mut s = String::new();
        s += &format!("<div class=\"line hilight-{}\">", hilight_depth == 0);
        for node in &self.uniq {
            for child in &node.children {
                match *child {
                    Child::Node { location, .. } => {
                        s += &format!("<div class=\"rel col-{}\">{} </div>",
                                      location.depth,
                                      location.ofs)
                    }
                    Child::Leaf(ref t) => {
                        s += &format!("<div class=\"leaf\">{:?}</div>", t)
                    }
                }
            }
            s += "<br/>"
        }
        s += "</div>";

        let mut linecount = 1;
        for line in &self.shared {
            s += &format!("<div class=\"line hilight-{}\">",
                          hilight_depth == linecount);
            linecount += 1;
            for node in &**line {
                for child in &node.children {
                    match *child {
                        Child::Node { location, .. } => {
                            s += &format!("<div class=\"loc col-{}\">{} \
                                           </div>",
                                          location.depth,
                                          location.ofs)
                        }
                        Child::Leaf(ref t) => {
                            s += &format!("<div class=\"leaf \
                                           weight-{}\">{:?}</div>",
                                          t.weight() / 2,
                                          t)
                        }
                    }
                }
                s += "<br/>"
            }
            s += "</div>";
        }
        s
    }
}

impl<T, M> fmt::Debug for Location<T, M>
    where T: Val + fmt::Debug,
          M: Meta<T>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "loc {} ({})", self.ofs, self.depth)
    }
}

impl<T, M> Html<T, M> for Location<T, M>
    where T: Val + fmt::Debug,
          M: Meta<T>
{
    fn _html(&self, stash: RelStash<T, M>) -> String {
        format!("<div class=\"rel col-{}\">{}</div>",
                stash.depth() + self.depth,
                stash.get(*self)._html(stash.relative(*self)))
    }
}
