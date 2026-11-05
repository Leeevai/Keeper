//! Random number generation, ported from `@keybr/rand`.

use std::time::{SystemTime, UNIX_EPOCH};

/// A source of random numbers in the range `0 <= value < 1`.
pub trait Rng {
    fn next_f64(&mut self) -> f64;
}

/// A linear congruential generator with the same constants as keybr.com.
#[derive(Debug, Clone)]
pub struct Lcg {
    x: u64,
}

impl Lcg {
    const A: u64 = 0x41c6_4e6d;
    const C: u64 = 0x0000_3039;
    const M: u64 = 0x8000_0000;

    pub fn new(seed: u64) -> Self {
        Self {
            x: (seed ^ 0x3fc2_8cf6) & (Self::M - 1),
        }
    }

    /// Seeds the generator from the current time.
    pub fn from_time() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Self::new(nanos ^ (nanos >> 32))
    }
}

impl Rng for Lcg {
    fn next_f64(&mut self) -> f64 {
        self.x = (Self::A.wrapping_mul(self.x).wrapping_add(Self::C)) & (Self::M - 1);
        self.x as f64 / Self::M as f64
    }
}

/// Selects a random element from a list of uniform elements.
pub fn random_sample<'a, T>(list: &'a [T], rng: &mut dyn Rng) -> &'a T {
    assert!(!list.is_empty(), "cannot sample from an empty list");
    let index = (rng.next_f64() * list.len() as f64) as usize;
    &list[index.min(list.len() - 1)]
}

/// Selects a random element from a list of weighted elements.
pub fn weighted_random_sample<'a, T>(
    list: &'a [T],
    weight: impl Fn(&T) -> f64,
    rng: &mut dyn Rng,
) -> &'a T {
    assert!(!list.is_empty(), "cannot sample from an empty list");
    let sum: f64 = list.iter().map(&weight).sum();
    let mut r = rng.next_f64() * sum;
    for v in list {
        let w = weight(v);
        if r <= w {
            return v;
        }
        r -= w;
    }
    &list[list.len() - 1]
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A fake generator that cycles through the given values.
    pub struct FakeRng {
        values: Vec<f64>,
        index: usize,
    }

    impl FakeRng {
        pub fn new(values: &[f64]) -> Self {
            Self {
                values: values.to_vec(),
                index: 0,
            }
        }
    }

    impl Rng for FakeRng {
        fn next_f64(&mut self) -> f64 {
            let v = self.values[self.index % self.values.len()];
            self.index += 1;
            v
        }
    }

    #[test]
    fn lcg_is_deterministic_and_in_range() {
        let mut a = Lcg::new(42);
        let mut b = Lcg::new(42);
        for _ in 0..10_000 {
            let v = a.next_f64();
            assert_eq!(v, b.next_f64());
            assert!((0.0..1.0).contains(&v));
        }
        assert_ne!(Lcg::new(1).next_f64(), Lcg::new(2).next_f64());
    }

    #[test]
    fn lcg_is_roughly_uniform() {
        let mut rng = Lcg::new(7);
        let mut buckets = [0u32; 10];
        for _ in 0..100_000 {
            buckets[(rng.next_f64() * 10.0) as usize] += 1;
        }
        for count in buckets {
            assert!((9_000..11_000).contains(&count), "{buckets:?}");
        }
    }

    #[test]
    fn samples() {
        let list = ["a", "b", "c", "d"];
        assert_eq!(*random_sample(&list, &mut FakeRng::new(&[0.0])), "a");
        assert_eq!(*random_sample(&list, &mut FakeRng::new(&[0.5])), "c");
        assert_eq!(*random_sample(&list, &mut FakeRng::new(&[0.99])), "d");
        let weighted = [("a", 1.0), ("b", 0.0), ("c", 3.0)];
        let w = |v: &(&str, f64)| v.1;
        assert_eq!(
            weighted_random_sample(&weighted, w, &mut FakeRng::new(&[0.0])).0,
            "a"
        );
        assert_eq!(
            weighted_random_sample(&weighted, w, &mut FakeRng::new(&[0.3])).0,
            "c"
        );
        assert_eq!(
            weighted_random_sample(&weighted, w, &mut FakeRng::new(&[0.99])).0,
            "c"
        );
    }
}
