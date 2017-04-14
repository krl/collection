use collection::Collection;

use meta::Meta;
use meta::cardinality::Cardinality;

use Val;

use tree::branch::{Branch, BranchResult};
use tree::level::Beginning;

impl<T, M> Collection<T, M>
    where T: Val,
          M: Meta<T> + SubMeta<Cardinality<usize>>
{
    pub fn get(&mut self, i: usize) -> Option<&T> {
        let mut state = Cardinality::new(&i);

        let res: BranchResult<_, _, Beginning> =
            Branch::new_full(self.root, &mut state, &self.stash);

        match res {
            BranchResult::Hit(branch) => branch.leaf(&self.stash),
            _ => None,

        }
    }
}
