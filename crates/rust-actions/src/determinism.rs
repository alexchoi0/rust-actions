use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

#[derive(Debug)]
pub struct SeededRng {
    rng: ChaCha8Rng,
    seed: u64,
}

impl SeededRng {
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
        }
    }

    pub fn from_scenario_name(name: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        let seed = hasher.finish();
        Self::with_seed(seed)
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn next_uuid(&mut self) -> Uuid {
        let bytes: [u8; 16] = self.rng.gen();
        Uuid::from_bytes(bytes)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.rng.gen()
    }

    pub fn next_u32(&mut self) -> u32 {
        self.rng.gen()
    }

    pub fn next_i64(&mut self) -> i64 {
        self.rng.gen()
    }

    pub fn next_f64(&mut self) -> f64 {
        self.rng.gen()
    }

    pub fn next_bool(&mut self) -> bool {
        self.rng.gen()
    }

    pub fn next_string(&mut self, len: usize) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        (0..len)
            .map(|_| {
                let idx = self.rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    pub fn next_alphanumeric(&mut self, len: usize) -> String {
        self.next_string(len)
    }

    pub fn next_hex(&mut self, len: usize) -> String {
        const CHARSET: &[u8] = b"0123456789abcdef";
        (0..len)
            .map(|_| {
                let idx = self.rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    pub fn next_range(&mut self, min: u64, max: u64) -> u64 {
        self.rng.gen_range(min..max)
    }

    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            let idx = self.rng.gen_range(0..items.len());
            Some(&items[idx])
        }
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        use rand::seq::SliceRandom;
        items.shuffle(&mut self.rng);
    }
}

impl Default for SeededRng {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SeededRng {
    fn clone(&self) -> Self {
        Self::with_seed(self.seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_uuid() {
        let mut rng1 = SeededRng::with_seed(42);
        let mut rng2 = SeededRng::with_seed(42);

        let uuid1 = rng1.next_uuid();
        let uuid2 = rng2.next_uuid();

        assert_eq!(uuid1, uuid2);
    }

    #[test]
    fn test_deterministic_string() {
        let mut rng1 = SeededRng::with_seed(123);
        let mut rng2 = SeededRng::with_seed(123);

        let s1 = rng1.next_string(32);
        let s2 = rng2.next_string(32);

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_from_scenario_name() {
        let rng1 = SeededRng::from_scenario_name("test scenario");
        let rng2 = SeededRng::from_scenario_name("test scenario");
        let rng3 = SeededRng::from_scenario_name("different scenario");

        assert_eq!(rng1.seed(), rng2.seed());
        assert_ne!(rng1.seed(), rng3.seed());
    }

    #[test]
    fn test_sequence_determinism() {
        let mut rng1 = SeededRng::with_seed(999);
        let mut rng2 = SeededRng::with_seed(999);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }
}
