//! The retail pseudo-random sources behind 121's 0x6000 operator.
//!
//! There are **two** different generators in the retail binary, and this module
//! now carries both because M8 W5-a decompiled the operator itself.
//!
//! 1. The **event generator** (the one 0x6000 actually calls).
//!
//!    `docs/research/M8_PRNG_RANGE.md` decompiles the opcode-121 handler: the
//!    random case at RVA 0xaf7c0 calls RVA 0x35810 and then reduces the result
//!    modulo `abs(arg2-arg1)+1`. RVA 0x35810 is the Park-Miller "minimal
//!    standard" LCG (Lehmer / MINSTD):
//!
//! ```text
//! state = state * 16807 % 0x7FFFFFFF      // 16807 = 0x41A7, Mersenne 2^31-1
//! ```
//!
//!    It self-seeds from the Windows `GetTickCount()` clock on first use, then
//!    a rejection loop multiplies again while the state exceeds 2,000,000,000
//!    and reseeds if it becomes 0; the returned draw is the state, roughly
//!    `1..=2_000_000_000`. `next_event_draw()` runs exactly that loop. The
//!    retail seed is the raw 32-bit tick count: RVA 0x35847 calls
//!    KERNEL32!GetTickCount and RVA 0x35852 stores EAX into the Park-Miller
//!    state with no folding (proven in docs/research/M11_PRNG_CLOCK.md).
//!    The library keeps `EVENT_SEED` as its default so tests and diagnostic
//!    commands are deterministic. The interactive front end (`wolf-cli play`)
//!    seeds the event generator from the local clock at startup, matching
//!    retail; `WOLF_EVENT_SEED` overrides that for a captured-tick replay. A
//!    zero state is still normalised to `EVENT_RESEED` because a captured tick
//!    of zero cannot be re-read from a live clock.
//!
//! 2. The **static-table / crypto stream** (the MSVC C-runtime LCG).
//!
//!    The Game.dat "static random" block ships a pre-generated stream next to
//!    the values it was drawn from, and _wolfref/game_dat_random.c documents
//!    exactly how it is built:
//!
//! ```text
//! #define RAND(a) ((a)*0x343fd+0x269ec3)
//! getNextRandom() {
//!     _seed = RAND(_seed)             // _seed = _seed*0x343fd + 0x269ec3 (mod 2^32)
//!     return ((_seed >> 16) & 0x7fff) // 15-bit output, 0..=32767
//! }
//! ```
//!
//!    This is the Microsoft C runtime LCG (rand()/srand()): multiplier
//!    0x343FD = 214013, increment 0x269EC3 = 2531011, output the middle 15 bits.
//!    Sanity anchor: seeding with 1 must yield 41, 18467, 6334, 26500, 19169, ...
//!    the canonical MSVC rand() sequence, and it does (see the unit tests).
//!    `next_raw()` keeps this stream for the Game.dat static table; it is NOT
//!    the event RNG.
//!
//! Range reduction (pinned, not inferred)
//! ```text
//! low     = min(a, b)
//! modulus = abs(b - a) + 1        // 32-bit wrapping, never zero
//! value   = low + (draw % modulus) // unsigned division
//! ```
//!
//! One event draw per call, including a zero-width range. `WolfRng::range`
//! implements this exactly; `M8_PRNG_RANGE.md` §1-2 is the decompiled source.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

/// Default state for the MSVC static-table stream: the first static-random word,
/// Game.dat random_a (22227, see docs/BASELINE.json). Fixed so the interpreter is
/// deterministic before wolf-game wires the real file in.
pub const DEFAULT_SEED: u32 = 22_227;

/// Default seed for the Park-Miller event generator. The retail stream self-seeds
/// from the clock, so there is no retail default; this build uses a fixed seed so
/// every run replays identically. It is arbitrary but stable — pin the vectors if
/// you change it.
pub const EVENT_SEED: u32 = 1;

/// What a zero Park-Miller state reseeds to. The retail routine calls the clock
/// again; this build uses a fixed nonzero constant so replays stay deterministic.
/// (State 0 is a fixed point of the LCG, so a zero seed must never be kept.)
pub const EVENT_RESEED: u32 = 1;

/// MSVC C-runtime LCG multiplier (0x343FD).
const MULTIPLIER: u32 = 0x0003_43FD;

/// MSVC C-runtime LCG increment (0x269EC3).
const INCREMENT: u32 = 0x0026_9EC3;

/// Park-Miller multiplier (0x41A7).
const PARK_MILLER_MULTIPLIER: u64 = 16_807;

/// Park-Miller modulus (2^31 - 1).
const PARK_MILLER_MODULUS: u64 = 0x7FFF_FFFF;

/// The retail rejection ceiling: the loop keeps multiplying while the state
/// exceeds this (0x77359400 = 2,000,000,000).
const EVENT_CEILING: u64 = 2_000_000_000;

/// The two retail generators.
#[derive(Debug, Clone)]
pub struct WolfRng {
    /// MSVC LCG state (_seed in _wolfref/game_dat_random.c), for the static table.
    state: u32,
    /// Park-Miller state, for opcode 121's 0x6000 operator (RVA 0x35810).
    event_state: u32,
    /// The seed the event generator was last (re)seeded with, after the zero
    /// normalisation. Recorded so a run can report the seed it actually used
    /// (the production hook in resolve_event_seed) and a replay can be
    /// reproduced from that number.
    event_seed: u32,
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

    /// A generator from an explicit 32-bit MSVC seed, with the event generator
    /// left at its default [`EVENT_SEED`]. Two generators built from the same
    /// seed produce the same sequence; different seeds diverge.
    #[must_use]
    pub fn from_seed(seed: u32) -> Self {
        Self::from_seeds(seed, EVENT_SEED)
    }

    /// A generator with both streams seeded explicitly.
    #[must_use]
    pub fn from_seeds(seed: u32, event_seed: u32) -> Self {
        let event_seed = normalise_event_seed(event_seed);
        Self {
            state: seed,
            event_state: event_seed,
            event_seed,
            draws: 0,
        }
    }

    /// Only the Park-Miller event generator is reseeded; the MSVC static-table
    /// stream keeps its default seed. For tests and same-save replay.
    #[must_use]
    pub fn from_event_seed(event_seed: u32) -> Self {
        Self::from_seeds(DEFAULT_SEED, event_seed)
    }

    /// The retail initial seed: feed a raw Windows `GetTickCount()` DWORD into
    /// the event generator, exactly as RVA 0x35810 does.
    ///
    /// Retail provenance (docs/research/M11_PRNG_CLOCK.md):
    /// - RVA 0x35847: `FF 15 44 20 80 00  call dword ptr [0x802044]`, and
    ///   0x802044 is KERNEL32!GetTickCount in the import table;
    /// - RVA 0x35852: `A3 F0 17 1B 01  mov dword ptr [0x11b17f0], eax`.
    ///
    /// The raw tick is stored directly: there is no add/xor/shift/multiply
    /// folding before the first Park-Miller step. The MSVC static-table stream
    /// keeps `DEFAULT_SEED`. A zero tick is normalised by
    /// `from_event_seed` to `EVENT_RESEED`; retail would instead re-read the
    /// clock on a zero state, which cannot be replayed from one captured tick.
    #[must_use]
    pub fn from_clock_ticks(ticks: u32) -> Self {
        Self::from_event_seed(seed_from_clock_ticks(ticks))
    }

    /// Production hook: seed the event generator from the WOLF_EVENT_SEED
    /// environment variable, leaving the MSVC static-table stream at its
    /// default. Absent or invalid means the fixed EVENT_SEED, so behaviour is
    /// unchanged unless a seed is supplied explicitly.
    #[must_use]
    pub fn from_event_seed_env() -> Self {
        Self::from_event_seed(event_seed_from_env())
    }

    /// Reseed the event generator in place, keeping the static-table stream.
    /// The recorded active seed (see WolfRng::event_seed) is updated too.
    pub fn set_event_seed(&mut self, event_seed: u32) {
        let event_seed = normalise_event_seed(event_seed);
        self.event_state = event_seed;
        self.event_seed = event_seed;
    }

    /// The seed the event generator was last seeded with (after zero
    /// normalisation). This is the number a replay must be given to reproduce
    /// the same stream; the production hook records it here.
    #[must_use]
    pub const fn event_seed(&self) -> u32 {
        self.event_seed
    }

    /// Same, seeded from a parsed Game.dat (random_a, random_b, static_randoms).
    /// The MSVC stream is a single-state LCG: the first static word random_a
    /// seeds it, and random_b / the byte table are the same stream's next
    /// outputs, so they are accepted for source-of-truth but need not be
    /// replayed separately. (Note: M8_PRNG_RANGE §3 shows random_a/random_b are
    /// encoded *draws* of the static stream, not its seed; this signature is
    /// kept for API stability with the Game.dat loader.)
    #[must_use]
    pub fn from_game(seed_a: u32, _seed_b: u32, _table: Vec<u8>) -> Self {
        Self::from_seed(seed_a)
    }

    /// Draws taken so far (counts range() calls).
    #[must_use]
    pub const fn draws(&self) -> u64 {
        self.draws
    }

    /// The current MSVC static-table LCG state.
    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    /// The current Park-Miller event state.
    #[must_use]
    pub const fn event_state(&self) -> u32 {
        self.event_state
    }

    /// One getNextRandom() step on the MSVC static-table stream: advance the
    /// state and return the 15-bit draw ((_seed >> 16) & 0x7fff, 0..=32767).
    /// This is NOT the event RNG; opcode 0x6000 uses `next_event_draw`.
    pub fn next_raw(&mut self) -> u16 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        ((self.state >> 16) & 0x7fff) as u16
    }

    /// One Park-Miller step: `state = state * 16807 % 0x7FFFFFFF`, multiplied
    /// again while the state exceeds 2,000,000,000, reseeded if it becomes 0.
    /// Returns the state (roughly `1..=2_000_000_000`), exactly as RVA 0x35810.
    pub fn next_event_draw(&mut self) -> u32 {
        loop {
            let next = u64::from(self.event_state) * PARK_MILLER_MULTIPLIER % PARK_MILLER_MODULUS;
            let next = next as u32;
            self.event_state = if next == 0 { EVENT_RESEED } else { next };
            if u64::from(self.event_state) <= EVENT_CEILING {
                return self.event_state;
            }
            // Above the ceiling: multiply again (the retail rejection loop).
        }
    }

    /// Uniform integer in [min, max], inclusive on both ends, reduced with the
    /// retail law at RVA 0xaf7c0:
    ///
    /// ```text
    /// low = min(a, b); modulus = abs(b - a) + 1; value = low + draw % modulus
    /// ```
    ///
    /// `abs` is 32-bit wrapping (`unsigned_abs` of a wrapping subtraction), so
    /// `i32::MIN..=i32::MAX` has modulus 2 just like the retail `div`. One event
    /// draw is consumed per call, including a zero-width range, matching the
    /// retail call order (the generator is called before the span is computed).
    ///
    /// Out-of-order bounds are handled by `min` + the absolute difference, which
    /// is equivalent to swapping. A zero-width range always returns the lower
    /// bound but still advances the stream.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        self.draws += 1;
        let draw = self.next_event_draw();
        let low = min.min(max);
        let diff = max.wrapping_sub(min);
        let modulus = diff.unsigned_abs().wrapping_add(1);
        low.wrapping_add((draw % modulus) as i32)
    }
}

/// The retail folding of a raw `GetTickCount()` DWORD into the Park-Miller
/// seed. The disassembly proves the folding is the identity: RVA 0x35847 calls
/// KERNEL32!GetTickCount through IAT slot 0x802044 and RVA 0x35852 writes its
/// EAX return straight into the state at 0x11b17f0. Kept as a named helper so
/// the zero-folding fact is explicit and testable.
#[must_use]
pub const fn seed_from_clock_ticks(ticks: u32) -> u32 {
    ticks
}

/// A zero event seed is a fixed point of the Park-Miller LCG; the retail routine
/// reseeds it. This build maps 0 to the fixed [`EVENT_RESEED`].
const fn normalise_event_seed(seed: u32) -> u32 {
    if seed == 0 {
        EVENT_RESEED
    } else {
        seed
    }
}

/// Environment variable the production event-RNG hook reads. No retail binary
/// is involved: a caller that captures the retail clock seed can pass it here to
/// reproduce a retail run bit for bit (see docs/research/M9_PRNG_SEED.md).
pub const EVENT_SEED_ENV: &str = "WOLF_EVENT_SEED";

/// Resolve an event seed from a raw environment value. None (variable absent)
/// or an unparsable value returns the fixed EVENT_SEED, so the default build is
/// unchanged; a valid u32 is zero-normalised exactly like from_event_seed.
#[must_use]
pub fn resolve_event_seed(raw: Option<&str>) -> u32 {
    match raw {
        Some(text) => text
            .trim()
            .parse::<u32>()
            .map_or(EVENT_SEED, normalise_event_seed),
        None => EVENT_SEED,
    }
}

/// Read WOLF_EVENT_SEED and resolve it through resolve_event_seed. This is the
/// only place the environment is consulted; WolfRng::new/from_seed never look at
/// it, which is why the default sequence stays deterministic.
#[must_use]
pub fn event_seed_from_env() -> u32 {
    resolve_event_seed(std::env::var(EVENT_SEED_ENV).ok().as_deref())
}

/// Resolve the event seed for an interactive session: a supplied
/// `WOLF_EVENT_SEED` wins so a captured retail tick can be replayed, otherwise
/// the local clock tick is used exactly as the retail `GetTickCount` seed would
/// be (identity folding, zero normalised to `EVENT_RESEED`).
///
/// This is kept pure so the policy is testable without touching the process
/// environment or sleeping for a real clock tick. `wolf-cli play` computes
/// `clock_ticks` from `SystemTime` and installs the result with
/// `WolfRng::set_event_seed`; the library constructors stay deterministic.
#[must_use]
pub fn resolve_session_event_seed(raw: Option<&str>, clock_ticks: u32) -> u32 {
    match raw {
        Some(_) => resolve_event_seed(raw),
        None => normalise_event_seed(seed_from_clock_ticks(clock_ticks)),
    }
}

#[cfg(test)]
mod tests {
    use super::{WolfRng, EVENT_RESEED};

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

    /// The Park-Miller draw sequence, states included.
    fn event_vectors(seed: u32, n: usize) -> Vec<u32> {
        let mut rng = WolfRng::from_event_seed(seed);
        (0..n).map(|_| rng.next_event_draw()).collect()
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
            vec![
                464_430_666,
                69_489_637,
                2_511_435_540,
                3_354_364_295,
                3_849_641_518
            ]
        );
        // new() == from_seed(DEFAULT_SEED).
        let mut a = WolfRng::new();
        let mut b = WolfRng::from_seed(22_227);
        assert_eq!(a.next_raw(), b.next_raw());
    }

    #[test]
    fn matches_the_park_miller_event_vectors() {
        // The textbook MINSTD sequence: seed 1 -> 16807, 282475249, 1622650073,
        // 984943658, 1144108930, 470211272, 101027544, 1457850878.
        assert_eq!(
            event_vectors(1, 8),
            vec![
                16_807,
                282_475_249,
                1_622_650_073,
                984_943_658,
                1_144_108_930,
                470_211_272,
                101_027_544,
                1_457_850_878,
            ]
        );
        // Every draw stays inside the retail ceiling.
        for draw in event_vectors(1, 200) {
            assert!(
                (1..=2_000_000_000).contains(&draw),
                "out of ceiling: {draw}"
            );
        }
        // The event stream is independent of the MSVC stream.
        let mut rng = WolfRng::from_seeds(22_227, 1);
        assert_eq!(rng.next_event_draw(), 16_807);
        assert_eq!(rng.next_raw(), 7086, "MSVC stream unaffected");
    }

    #[test]
    fn the_rejection_loop_multiplies_again_above_the_ceiling() {
        // A seed whose first product exceeds 2e9 must not be returned as-is.
        // 2_000_000_001 * 16807 % (2^31-1) is the first Park-Miller step; assert
        // the draw is below the ceiling and that it is the *next* product, not
        // the seed.
        let mut rng = WolfRng::from_event_seed(2_000_000_001);
        let first = rng.next_event_draw();
        assert!(
            first <= 2_000_000_000,
            "rejected the oversized state: {first}"
        );
        // Reproduce the loop by hand for the same seed.
        let mut state = u64::from(2_000_000_001u32);
        loop {
            state = state * 16_807 % 0x7FFF_FFFF;
            if state == 0 {
                state = u64::from(EVENT_RESEED);
            }
            if state <= 2_000_000_000 {
                break;
            }
        }
        assert_eq!(u64::from(first), state);
    }

    #[test]
    fn a_zero_seed_reseeds_instead_of_sticking_at_zero() {
        let mut rng = WolfRng::from_event_seed(0);
        assert_eq!(rng.event_state(), EVENT_RESEED, "zero seed is normalised");
        // The same draw sequence as seed EVENT_RESEED.
        let mut reference = WolfRng::from_event_seed(EVENT_RESEED);
        for _ in 0..8 {
            assert_eq!(rng.next_event_draw(), reference.next_event_draw());
        }
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
    fn a_degenerate_range_is_a_constant_but_consumes_a_draw() {
        let mut rng = WolfRng::new();
        let before = rng.event_state();
        assert_eq!(rng.range(5, 5), 5);
        assert_eq!(rng.range(-3, -3), -3);
        assert_ne!(
            rng.event_state(),
            before,
            "retail calls the generator before computing the span"
        );
        assert_eq!(rng.draws(), 2, "each zero-width range counts as a draw");
    }

    #[test]
    fn range_matches_the_pinned_modulo_law() {
        // Default event seed 1: draw % 10 mapped into 1..=10.
        let mut rng = WolfRng::new();
        let got: Vec<i32> = (0..8).map(|_| rng.range(1, 10)).collect();
        assert_eq!(got, vec![8, 10, 4, 9, 1, 3, 5, 9]);
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
        assert_ne!(
            first, second,
            "different seeds must produce different streams"
        );

        let mut c = WolfRng::from_event_seed(1);
        let mut d = WolfRng::from_event_seed(2);
        let first: Vec<u32> = (0..16).map(|_| c.next_event_draw()).collect();
        let second: Vec<u32> = (0..16).map(|_| d.next_event_draw()).collect();
        assert_ne!(first, second, "different event seeds must diverge");
    }
}
