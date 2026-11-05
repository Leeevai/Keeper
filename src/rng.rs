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
