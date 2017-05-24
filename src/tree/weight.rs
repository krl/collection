use std::hash::{Hash, Hasher};

use seahash::SeaHasher;

/// The "weight" of an element, should produce a uniform distribution from the
/// possible values of the type implementing this trait.
pub trait Weight {
    /// Hash the element
    fn weight_hash(&self) -> u64;

    /// Weight is the amount of leading zeroes in the `weight_hash`
    fn weight(&self) -> usize {
        // count leading zeroes
        // TODO: use extended instructions when supported
        let mut w = 0;
        let a = self.weight_hash();
        loop {
            if a >> (63 - w) == 1 {
                return w;
            }
            w += 1;
        }
    }
}

impl<T> Weight for T
    where T: Hash
{
    fn weight_hash(&self) -> u64 {
        let mut hasher = SeaHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}
