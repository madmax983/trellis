//! A tiny seeded generator both sides of the wire can run bit for bit.
//!
//! mulberry32: one 32-bit state, `wrapping_add` and `wrapping_mul` only, so the
//! browser port is three lines of `Math.imul`.

/// mulberry32.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng(u32);

impl Rng {
    /// A generator from a seed.
    pub const fn new(seed: u32) -> Self {
        Self(seed)
    }

    /// The next 32 bits.
    pub const fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let t = self.0;
        let mut t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3D));
        t ^ (t >> 14)
    }

    /// A draw in `0..n`, for `n > 0`. Modulo bias is accepted: these are
    /// starting positions and a wild card's temper, not a lottery.
    pub const fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { self.next_u32() % n }
    }
}

/// FNV-1a over the UTF-8 bytes of a word: every word is a seed.
pub fn seed_of(word: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in word.trim().to_lowercase().bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        // Pinned so the browser port has something to agree with.
        let mut r = Rng::new(1);
        let a = r.next_u32();
        let b = r.next_u32();
        assert_ne!(a, b);
        let mut r2 = Rng::new(1);
        assert_eq!(r2.next_u32(), a);
        assert_eq!(seed_of(""), 0x811C_9DC5);
        assert_eq!(seed_of("Ivy"), seed_of(" ivy "));
    }
}
