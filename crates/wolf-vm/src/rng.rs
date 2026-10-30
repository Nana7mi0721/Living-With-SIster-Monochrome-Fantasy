//! The retail pseudo-random source behind 121's 0x6000 operator.
//!
//! Provenance
//!
//! The Game.dat "static random" block ships a pre-generated stream next to the
//! values it was drawn from, and _wolfref/game_dat_random.c documents exactly
//! how it is built. Its top comment is the spec:
//!
//!   #define RAND(a) ((a)*0x343fd+0x269ec3)
//!   getNextRandom() {
//!       _seed = RAND(_seed)             // _seed = _seed*0x343fd + 0x269ec3  (mod 2^32)
//!       return ((_seed >> 16) & 0x7fff) // 15-bit output, 0..=32767
//!   }
//!
//! This is the Microsoft C runtime LCG (rand()/srand()): multiplier
//! 0x343FD = 214013, increment 0x269EC3 = 2531011, output the middle 15 bits.
//! Sanity anchor: seeding with 1 must yield 41, 18467, 6334, 26500, 19169, ...
//! the canonical MSVC rand() sequence, and it does (see the unit tests).
//!
//! Game.dat stores random_a = 22227 and random_b = 25684 as the first two
//! 32-bit words of that block, followed by 28,936 bytes in 0..=127; the seed
//! recovery tool proves each of those bytes is (state >> 24) & 0x7f, i.e. the
//! top seven bits of the same LCG state. So the whole block is one LCG stream.
//!
//! What this module does
//!
//! It runs that LCG live and exposes the 15-bit getNextRandom() draw through
//! next_raw(), and maps a draw onto an inclusive integer range in range()
//! (the 0x6000 operator). The previous xorshift32 placeholder, which could not
//! reproduce a retail stream, is gone.
//!
//! Residual uncertainty (inferred, not proven)
//!
//! game_dat_random.c proves the state, the multiplier, the addition, and the
//! 15-bit output width. It does NOT decompile the engine's range operator, so
//! how a 15-bit draw becomes [min, max] is inferred: this module scales the
//! draw across the whole span (draw * (span+1) / 32768, "multiply-high"). That
//! reaches both endpoints for spans up to 32,768 and never leaves the range. A
//! plain draw % (span+1) would truncate any span wider than 32,767 (notably the
//! -20000..20000 damage float), so scaling is the safer reconstruction. The exact
//! rounding/scale factor is the one open question, see docs/research/M7_PRNG.md.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

/// Default state for this build: the first static-random word, Game.dat
/// random_a (22227, see docs/BASELINE.json). Fixed so the interpreter is
/// deterministic before wolf-game wires the real file in.
pub const DEFAULT_SEED: u32 = 22_227;

/// MSVC C-runtime LCG multiplier (0x343FD).
const MULTIPLIER: u32 = 0x0003_43FD;

/// MSVC C-runtime LCG increment (0x269EC3).
const INCREMENT: u32 = 0x0026_9EC3;

/// The retail LCG. State is the single 32-bit _seed; each draw advances it by
/// state*0x343FD + 0x269EC3 (mod 2^32) and returns (state >> 16) & 0x7fff.
#[derive(Debug, Clone)]
pub struct WolfRng {
    /// The LCG state (_seed in _wolfref/game_dat_random.c).
    state: u32,
    /// How many range() draws happened. Saved so a replay can be diffed
    /// against the original's draw count instead of guessing.
    draws: u64,
}

impl Default for WolfRng {
    fn default() -> Self {
        Self::new()
    }
}

impl WolfRng {
    /// A generator seeded the way this build's Game.dat is.
    #[must_use]
    pub fn new() -> Self {
        Self::from_seed(DEFAULT_SEED)
    }

    /// A generator from an explicit 32-bit LCG seed. Two generators built from
    /// the same seed produce the same sequence; different seeds diverge.
    #[must_use]
    pub fn from_seed(seed: u32) -> Self {
        Self {
            state: seed,
            draws: 0,
        }
    }

    /// Same, seeded from a parsed Game.dat (random_a, random_b, static_randoms).
    /// The retail PRNG is a single-state LCG: the first static word random_a
    /// seeds it, and random_b / the byte table are the same stream's next
    /// outputs, so they are accepted for source-of-truth but need not be
    /// replayed separately. Kept for API stability with the Game.dat loader.
    #[must_use]
    pub fn from_game(seed_a: u32, _seed_b: u32, _table: Vec<u8>) -> Self {
        Self::from_seed(seed_a)
    }

    /// Draws taken so far (counts range() calls).
    #[must_use]
    pub const fn draws(&self) -> u64 {
        self.draws
    }

    /// The current LCG state.
    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    /// One getNextRandom() step: advance the state and return the 15-bit draw
    /// ((_seed >> 16) & 0x7fff, 0..=32767).
    pub fn next_raw(&mut self) -> u16 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        ((self.state >> 16) & 0x7fff) as u16
    }

    /// Uniform integer in [min, max], inclusive on both ends.
    ///
    /// Out-of-order bounds are swapped rather than treated as an error: the
    /// editor lets the author pick the range with two magic values, and at least
    /// one call site (ev119 pc128) passes a variable as the upper bound, so the
    /// order is only known at run time.
    ///
    /// A zero-width range is a constant (the lower bound) and consumes no draw,
    /// so it never panics.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        self.draws += 1;
        let (low, high) = if min <= max { (min, max) } else { (max, min) };
        let span = i64::from(high) - i64::from(low);
        if span <= 0 {
            return low;
        }
        // Map the 0..=32767 draw onto 0..=span ("multiply-high"): reaches both
        // endpoints for span < 32768 and stays inside for any span.
        let draw = u64::from(self.next_raw());
        let value = (draw * (span as u64 + 1)) >> 15;
        (i64::from(low) + value as i64) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::WolfRng;

    /// (state, getNextRandom) for the first draws after seeding.
    fn raw_vectors(seed: u32, n: usize) -> (Vec<u32>, Vec<u16>) {
        let mut rng = WolfRng::from_seed(seed);
        let mut states = Vec::with_capacity(n);
        let mut raw = Vec::with_capacity(n);
        for _ in 0..n {
            raw.push(rng.next_raw());
            states.push(rng.state());
        }
        (states, raw)
    }

    #[test]
    fn matches_the_msvc_rand_sequence_for_seed_1() {
        // The canonical VC++ srand(1) output: 41, 18467, 6334, 26500, 19169, ...
        let (states, raw) = raw_vectors(1, 8);
        assert_eq!(
            raw,
            vec![41, 18467, 6334, 26500, 19169, 15724, 11478, 29358]
        );
        assert_eq!(
            states,
            vec![
                2_745_024,
                3_357_800_067,
                415_139_642,
                3_884_216_597,
                3_403_800_452,
                1_030_492_215,
                752_224_798,
                1_924_036_713,
            ]
        );
    }

    #[test]
    fn matches_the_game_seed_vectors() {
        // from_seed(22227) -> the sequence used by WolfRng::new() in this build.
        let (states, raw) = raw_vectors(22_227, 5);
        assert_eq!(raw, vec![7086, 1060, 5553, 18415, 25972]);
        assert_eq!(
            states,
            vec![464_430_666, 69_489_637, 2_511_435_540, 3_354_364_295, 3_849_641_518]
        );
        // new() == from_seed(DEFAULT_SEED).
        let mut a = WolfRng::new();
        let mut b = WolfRng::from_seed(22_227);
        assert_eq!(a.next_raw(), b.next_raw());
    }

    #[test]
    fn range_stays_inside_the_bounds() {
        let mut rng = WolfRng::new();
        for _ in 0..1000 {
            let value = rng.range(1, 10);
            assert!((1..=10).contains(&value), "out of range: {value}");
        }
        for _ in 0..1000 {
            let value = rng.range(-20_000, 20_000);
            assert!((-20_000..=20_000).contains(&value), "out of range: {value}");
        }
    }

    #[test]
    fn reversed_bounds_are_swapped() {
        let mut rng = WolfRng::new();
        for _ in 0..100 {
            assert!((3..=7).contains(&rng.range(7, 3)));
        }
    }

    #[test]
    fn a_degenerate_range_is_a_constant() {
        let mut rng = WolfRng::new();
        for _ in 0..10 {
            assert_eq!(rng.range(5, 5), 5);
            assert_eq!(rng.range(-3, -3), -3);
        }
    }

    #[test]
    fn the_sequence_is_deterministic_and_not_constant() {
        let mut a = WolfRng::from_seed(22_227);
        let mut b = WolfRng::from_seed(22_227);
        let first: Vec<i32> = (0..64).map(|_| a.range(0, 1_000_000)).collect();
        let second: Vec<i32> = (0..64).map(|_| b.range(0, 1_000_000)).collect();
        assert_eq!(first, second, "same seed must replay identically");
        assert!(
            first.iter().any(|value| *value != first[0]),
            "generator is stuck"
        );
        assert_eq!(a.draws(), 64);
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = WolfRng::from_seed(22_227);
        let mut b = WolfRng::from_seed(25_684);
        let first: Vec<u16> = (0..32).map(|_| a.next_raw()).collect();
        let second: Vec<u16> = (0..32).map(|_| b.next_raw()).collect();
        assert_ne!(first, second, "different seeds must produce different streams");
    }
}
