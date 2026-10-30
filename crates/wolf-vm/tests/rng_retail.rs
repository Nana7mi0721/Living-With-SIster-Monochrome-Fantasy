//! Determinism proofs for the retail PRNG port (W5 H-PRNG-1).
//!
//! These run against the public API only (from_seed / next_raw / range / draws).
//! The exact algorithm is the Microsoft C runtime LCG documented in
//! _wolfref/game_dat_random.c: state = state*0x343FD + 0x269EC3 (mod 2^32),
//! draw = (state >> 16) & 0x7fff. See crates/wolf-vm/src/rng.rs and
//! docs/research/M7_PRNG.md.

use wolf_vm::WolfRng;

/// The canonical VC++ srand(1) sequence; a well-known external anchor that the
/// port is the real MSVC LCG and not a lookalike.
const MSVC_SEED1: [u16; 8] = [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358];

/// from_seed(22227) draws, matching the module's own unit vectors.
const GAME_SEED: u32 = 22_227;

#[test]
fn same_seed_produces_the_same_sequence() {
    let mut a = WolfRng::from_seed(GAME_SEED);
    let mut b = WolfRng::from_seed(GAME_SEED);
    let ra: Vec<u16> = (0..64).map(|_| a.next_raw()).collect();
    let rb: Vec<u16> = (0..64).map(|_| b.next_raw()).collect();
    assert_eq!(ra, rb, "identical seeds must replay identically");

    let mut c = WolfRng::new();
    let mut d = WolfRng::new();
    let rc: Vec<i32> = (0..64).map(|_| c.range(0, 1_000_000)).collect();
    let rd: Vec<i32> = (0..64).map(|_| d.range(0, 1_000_000)).collect();
    assert_eq!(rc, rd, "range() draws must also replay identically");
}

#[test]
fn different_seeds_produce_different_sequences() {
    let mut a = WolfRng::from_seed(22_227);
    let mut b = WolfRng::from_seed(25_684);
    let ra: Vec<u16> = (0..64).map(|_| a.next_raw()).collect();
    let rb: Vec<u16> = (0..64).map(|_| b.next_raw()).collect();
    assert_ne!(ra, rb, "different seeds must diverge");
}

#[test]
fn matches_the_msvc_rand_vectors() {
    let mut rng = WolfRng::from_seed(1);
    let got: Vec<u16> = (0..8).map(|_| rng.next_raw()).collect();
    assert_eq!(got, MSVC_SEED1);
}

#[test]
fn matches_the_game_seed_vectors() {
    let mut rng = WolfRng::from_seed(GAME_SEED);
    let got: Vec<u16> = (0..5).map(|_| rng.next_raw()).collect();
    assert_eq!(got, vec![7086, 1060, 5553, 18415, 25972]);
    // new() is seeded with the same default word.
    let mut a = WolfRng::new();
    let mut b = WolfRng::from_seed(GAME_SEED);
    assert_eq!(a.next_raw(), b.next_raw());
}

#[test]
fn from_game_seeds_from_the_first_static_word() {
    let mut direct = WolfRng::from_seed(GAME_SEED);
    let mut via_game = WolfRng::from_game(GAME_SEED, 25_684, (0..=127u8).collect());
    assert_eq!(direct.next_raw(), via_game.next_raw());
}

#[test]
fn range_is_inclusive_and_stays_inside_the_bounds() {
    let mut rng = WolfRng::new();
    for _ in 0..2000 {
        let value = rng.range(1, 10);
        assert!((1..=10).contains(&value), "1..=10 out of range: {value}");
    }
    let mut rng = WolfRng::new();
    for _ in 0..2000 {
        let value = rng.range(-20_000, 20_000);
        assert!(
            (-20_000..=20_000).contains(&value),
            "-20000..=20000 out of range: {value}"
        );
    }
}

#[test]
fn a_wide_range_uses_both_halves() {
    // Multiply-high scaling must reach the full signed span, not just the low
    // 15-bit slice (a plain draw % (span+1) would cap at +32767 for a 40001-wide
    // range). 3000 deterministic draws from the default seed cover both tails.
    let mut rng = WolfRng::new();
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for _ in 0..3000 {
        let value = rng.range(-20_000, 20_000);
        min = min.min(value);
        max = max.max(value);
    }
    assert!(min < -19_000, "lower tail reached: min={min}");
    assert!(max > 19_000, "upper tail reached: max={max}");
}

#[test]
fn range_draws_are_exact_vectors() {
    // Pinned so any drift in the mapping is caught. First 8 draws of range(1,10)
    // from the default seed.
    let mut rng = WolfRng::new();
    let got: Vec<i32> = (0..8).map(|_| rng.range(1, 10)).collect();
    assert_eq!(got, vec![3, 1, 2, 6, 8, 10, 5, 4]);
}

#[test]
fn reversed_bounds_are_swapped() {
    let mut rng = WolfRng::new();
    let mut a = WolfRng::new();
    let mut b = WolfRng::new();
    for _ in 0..200 {
        assert!((3..=7).contains(&rng.range(7, 3)));
    }
    // range(7,3) must equal range(3,7) draw for draw.
    for _ in 0..32 {
        assert_eq!(a.range(7, 3), b.range(3, 7));
    }
}

#[test]
fn zero_width_ranges_do_not_panic() {
    let mut rng = WolfRng::new();
    for _ in 0..10 {
        assert_eq!(rng.range(5, 5), 5);
        assert_eq!(rng.range(-3, -3), -3);
        assert_eq!(rng.range(0, 0), 0);
    }
    // Extreme-but-valid bounds must also not panic.
    let _ = rng.range(i32::MIN, i32::MIN);
    let _ = rng.range(i32::MAX, i32::MAX);
    let _ = rng.range(i32::MIN, i32::MAX);
}

#[test]
fn draws_counts_range_calls_not_raw_steps() {
    let mut rng = WolfRng::new();
    assert_eq!(rng.draws(), 0);
    for _ in 0..7 {
        let _ = rng.range(0, 10);
    }
    assert_eq!(rng.draws(), 7);
    // A zero-width range still counts as a draw call.
    let _ = rng.range(4, 4);
    assert_eq!(rng.draws(), 8);
}
