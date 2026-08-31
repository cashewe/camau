use std::sync::{Arc, Mutex};

use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

pub type SharedRng = Arc<Mutex<ChaCha12Rng>>;

pub fn seeded_rng(seed: Option<u64>) -> SharedRng {
    let rng = match seed {
        Some(seed) => ChaCha12Rng::seed_from_u64(seed),
        None => ChaCha12Rng::from_rng(&mut rand::rng()),
    };
    Arc::new(Mutex::new(rng))
}
