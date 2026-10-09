//! Random numbers for things that only need to look random (a server to
//! hop to, random extra waits in macros, a browser tracker ID): SplitMix64
//! seeded from the system's random source (through `uuid`, which Pious uses
//! anyway). Not for secrets: tokens use `uuid` directly.

use std::sync::atomic::{AtomicU64, Ordering};

static STATE: AtomicU64 = AtomicU64::new(0);

/// A random 64-bit number.
pub fn u64() -> u64 {
    if STATE.load(Ordering::Relaxed) == 0 {
        let seed = uuid::Uuid::new_v4().as_u64_pair().0 | 1;
        let _ = STATE.compare_exchange(0, seed, Ordering::Relaxed, Ordering::Relaxed);
    }
    let mut z = STATE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A random number in `[0, 1)`.
pub fn unit() -> f64 {
    (u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// A random item of a list.
pub fn pick<T>(items: &[T]) -> Option<&T> {
    if items.is_empty() { None } else { items.get((u64() % items.len() as u64) as usize) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_random_and_stays_in_range() {
        let values: Vec<f64> = (0..2000).map(|_| unit()).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        assert!((0.4..0.6).contains(&mean), "{mean}");
        let items = [1, 2, 3];
        let picked: std::collections::BTreeSet<_> = (0..200).filter_map(|_| pick(&items)).collect();
        assert_eq!(picked.len(), 3);
        assert!(pick::<u8>(&[]).is_none());
    }
}
