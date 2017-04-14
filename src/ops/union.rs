use std::hash::Hash;

use Val;

use collection::Collection;
use meta::Meta;
use meta::max::Max;
use meta::checksum::CheckSum;

use tree::branch::{Branch, BranchResult};
use tree::level::{Beginning, End};
use stash::Stash;

impl<T, M> Collection<T, M>
    where T: Val + Ord + Hash,
          M: Meta<T>
{
    pub fn union(&mut self, b: &mut Self) -> Self
        where M: SubMeta<CheckSum<u64>> + SubMeta<Max<T>>
    {
        let a = self.clone_mut();

        let mut stash =
            self.stash.merge(&mut self.root, &mut b.root, &mut b.stash);

        let mut branch_a: Branch<_, _, Beginning> = Branch::first(a.root,
                                                                  &stash);
        let mut branch_b: Branch<_, _, Beginning> = Branch::first(b.root,
                                                                  &stash);
        let mut branch_c: Option<Branch<_, _, End>> = None;

        fn a_b<T, M>(from: &mut Branch<T, M, Beginning>,
                     into: &mut Option<Branch<T, M, End>>,
                     divisor: usize,
                     mut key: Max<T>,
                     stash: &mut Stash<T, M>)
            where T: Val + Ord + Hash,
                  M: Meta<T> + SubMeta<CheckSum<u64>> + SubMeta<Max<T>>
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
            let keys = (branch_a.leaf(&stash).map(|t| Max::from_t(t)),
                        branch_b.leaf(&stash).map(|t| Max::from_t(t)));
            match keys {
                (Some(a), Some(b)) => {
                    if a == b {
                        branch_a.find_differing(&mut branch_b, &stash);
                        a_b(&mut branch_a,
                            &mut branch_c,
                            self.divisor,
                            a,
                            &mut stash);
                        branch_b = branch_b.right(&mut stash);
                    } else if a > b {
                        a_b(&mut branch_b,
                            &mut branch_c,
                            self.divisor,
                            a,
                            &mut stash);
                    } else {
                        a_b(&mut branch_a,
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
}
