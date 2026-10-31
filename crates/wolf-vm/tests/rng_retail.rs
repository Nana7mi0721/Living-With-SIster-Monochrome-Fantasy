//! Determinism proofs for the retail PRNG ports (M7 W5 / M8 W5-a, H-PRNG-1).
//!
//! These run against the public API only. There are two retail generators:
//!
//! * the **event** generator behind opcode 121's 0x6000 operator: Park-Miller
//!   MINSTD, `state = state*16807 % 0x7FFFFFFF`, with the retail rejection loop
//!   (`> 2,000,000,000` multiply again) and the pinned modulo range law;
//! * the **static-table** MSVC C-runtime LCG: `state = state*0x343FD + 0x269EC3
//!   (mod 2^32)`, draw = `(state >> 16) & 0x7fff`.
//!
//! See crates/wolf-vm/src/rng.rs, docs/research/M8_PRNG_RANGE.md and
//! docs/research/M7_PRNG.md.

use wolf_vm::WolfRng;

/// The canonical VC++ srand(1) sequence; a well-known external anchor that the
/// static-table port is the real MSVC LCG and not a lookalike.
const MSVC_SEED1: [u16; 8] = [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358];

/// The textbook MINSTD sequence from seed 1 (Park & Miller 1988; also what the
/// retail RVA 0x35810 computes). The first 16807 is the anchor everyone quotes.
const PARK_MILLER_SEED1: [u32; 8] = [
    16_807,
    282_475_249,
    1_622_650_073,
    984_943_658,
    1_144_108_930,
    470_211_272,
    101_027_544,
    1_457_850_878,
];

/// from_seed(22227) static-table draws, matching the module's own unit vectors.
const GAME_SEED: u32 = 22_227;

/// Default event seed for this build (rng::EVENT_SEED).
const EVENT_SEED: u32 = 1;

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

    let mut e = WolfRng::from_event_seed(EVENT_SEED);
    let mut f = WolfRng::from_event_seed(EVENT_SEED);
    let re: Vec<u32> = (0..64).map(|_| e.next_event_draw()).collect();
    let rf: Vec<u32> = (0..64).map(|_| f.next_event_draw()).collect();
    assert_eq!(re, rf, "identical event seeds must replay identically");
}

#[test]
fn different_seeds_produce_different_sequences() {
    let mut a = WolfRng::from_seed(22_227);
    let mut b = WolfRng::from_seed(25_684);
    let ra: Vec<u16> = (0..64).map(|_| a.next_raw()).collect();
    let rb: Vec<u16> = (0..64).map(|_| b.next_raw()).collect();
    assert_ne!(ra, rb, "different static-table seeds must diverge");

    let mut c = WolfRng::from_event_seed(1);
    let mut d = WolfRng::from_event_seed(2);
    let rc: Vec<u32> = (0..64).map(|_| c.next_event_draw()).collect();
    let rd: Vec<u32> = (0..64).map(|_| d.next_event_draw()).collect();
    assert_ne!(rc, rd, "different event seeds must diverge");
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
fn matches_the_park_miller_event_vectors() {
    // The retail event generator is MINSTD, so seed 1 is the published sequence.
    let mut rng = WolfRng::from_event_seed(1);
    let got: Vec<u32> = (0..8).map(|_| rng.next_event_draw()).collect();
    assert_eq!(got, PARK_MILLER_SEED1);
    // The default event seed is EVENT_SEED, and from_event_seed is the only knob.
    let mut a = WolfRng::new();
    assert_eq!(a.next_event_draw(), PARK_MILLER_SEED1[0]);
    // Both streams are independent: advancing one does not move the other.
    let mut mixed = WolfRng::from_seeds(22_227, 1);
    assert_eq!(mixed.next_event_draw(), 16_807);
    assert_eq!(mixed.next_raw(), 7086);
    assert_eq!(mixed.next_event_draw(), 282_475_249);
    assert_eq!(mixed.next_raw(), 1060);
}

#[test]
fn the_event_draw_is_the_full_retail_domain_not_15_bits() {
    // M8's decompile says the draw is roughly 1..=2e9. A 15-bit stream would
    // never exceed 32767; assert both a large value and the ceiling.
    let mut rng = WolfRng::from_event_seed(1);
    let draws: Vec<u32> = (0..64).map(|_| rng.next_event_draw()).collect();
    assert!(
        draws.iter().any(|draw| *draw > 32_767),
        "the event stream is wider than the old 15-bit MSVC draw: {draws:?}"
    );
    assert!(
        draws.iter().any(|draw| *draw > 1_000_000_000),
        "the event stream reaches the retail 31-bit scale: {draws:?}"
    );
    assert!(
        draws.iter().all(|draw| (1..=2_000_000_000).contains(draw)),
        "every draw respects the rejection ceiling: {draws:?}"
    );
}

#[test]
fn from_game_seeds_the_static_table_from_the_first_static_word() {
    let mut direct = WolfRng::from_seed(GAME_SEED);
    let mut via_game = WolfRng::from_game(GAME_SEED, 25_684, (0..=127u8).collect());
    assert_eq!(direct.next_raw(), via_game.next_raw());
    // from_game does not touch the event stream: it still uses EVENT_SEED.
    assert_eq!(via_game.next_event_draw(), PARK_MILLER_SEED1[0]);
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
    // Both endpoints are reachable: a 0..=1 range must produce both 0 and 1.
    let mut rng = WolfRng::from_event_seed(1);
    let mut seen = [false; 2];
    for _ in 0..32 {
        seen[rng.range(0, 1) as usize] = true;
    }
    assert!(seen[0] && seen[1], "range(0,1) must reach both endpoints");
}

#[test]
fn a_wide_range_reaches_both_tails_of_the_signed_span() {
    // Under the old multiply-high mapping a 15-bit draw could never reach past
    // -20000+12767. The retail 31-bit draw + modulo reaches both tails, which is
    // the player-visible fix M8 W5-a was about.
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
fn range_draws_are_exact_vectors_under_the_pinned_modulo_law() {
    // Pinned so any drift in the reduction is caught. Default event seed 1.
    // range(1,10): draw % 10 + 1.
    let mut rng = WolfRng::new();
    let got: Vec<i32> = (0..8).map(|_| rng.range(1, 10)).collect();
    assert_eq!(got, vec![8, 10, 4, 9, 1, 3, 5, 9]);

    // A zero-based range is the raw reduction: draw % 10.
    let mut rng = WolfRng::from_event_seed(1);
    let got: Vec<i32> = (0..8).map(|_| rng.range(0, 10)).collect();
    assert_eq!(got, vec![10, 1, 0, 6, 8, 3, 2, 0]);

    // Wide range: low = -20000, modulus 40001.
    let mut rng = WolfRng::from_event_seed(1);
    let got: Vec<i32> = (0..8).map(|_| rng.range(-20_000, 20_000)).collect();
    assert_eq!(
        got,
        vec![-3193, 8188, -10492, 19036, -19672, 19518, 5019, -5567]
    );

    // The default *event* seed can be moved without touching the static table.
    let mut rng = WolfRng::from_event_seed(22_227);
    let got: Vec<i32> = (0..8).map(|_| rng.range(-20_000, 20_000)).collect();
    assert_eq!(
        got,
        vec![19851, 2277, 19785, -13511, 19102, 13181, -6096, 8956]
    );
}

#[test]
fn range_agrees_with_the_law_formula_draw_for_draw() {
    // Independent re-implementation of the decompiled law (RVA 0xaf7c0), fed the
    // same event stream, must agree for plain, reversed, zero-width, wide and
    // extreme ranges.
    fn law(draw: u32, min: i32, max: i32) -> i32 {
        let low = min.min(max);
        let modulus = max.wrapping_sub(min).unsigned_abs().wrapping_add(1);
        low.wrapping_add((draw % modulus) as i32)
    }
    let cases: [(i32, i32); 5] = [
        (1, 10),
        (7, 3),
        (5, 5),
        (-20_000, 20_000),
        (i32::MIN, i32::MAX),
    ];
    for (min, max) in cases {
        let mut a = WolfRng::from_event_seed(1);
        let mut b = WolfRng::from_event_seed(1);
        for _ in 0..64 {
            assert_eq!(
                a.range(min, max),
                law(b.next_event_draw(), min, max),
                "range({min},{max}) disagrees with the pinned law"
            );
        }
    }
}

#[test]
fn reversed_bounds_are_swapped_and_consume_the_same_stream() {
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
    assert_eq!(a.draws(), b.draws());
}

#[test]
fn zero_width_ranges_consume_exactly_one_draw() {
    // Retail calls the generator before it computes the span, so a constant range
    // still advances the stream. Prove it by comparing the following draw.
    let mut constant = WolfRng::from_event_seed(1);
    let value = constant.range(5, 5);
    assert_eq!(value, 5);
    let after = constant.next_event_draw();

    let mut reference = WolfRng::from_event_seed(1);
    assert_eq!(reference.next_event_draw(), PARK_MILLER_SEED1[0]);
    assert_eq!(reference.next_event_draw(), after);

    // The constant value is the lower bound for negative bounds too.
    let mut rng = WolfRng::from_event_seed(1);
    for _ in 0..10 {
        assert_eq!(rng.range(-3, -3), -3);
        assert_eq!(rng.range(0, 0), 0);
    }
    // Extreme-but-valid bounds must not panic.
    let _ = rng.range(i32::MIN, i32::MIN);
    let _ = rng.range(i32::MAX, i32::MAX);
    let _ = rng.range(i32::MIN, i32::MAX);
}

#[test]
fn an_extreme_range_uses_the_retail_32_bit_wrapping_modulus() {
    // max - min wraps to -1, so abs+1 = 2: only the lower two values are reachable.
    // The retail `div ecx` does exactly this; reproducing it is the point.
    let mut rng = WolfRng::from_event_seed(1);
    let mut seen = [false; 2];
    for _ in 0..64 {
        let value = rng.range(i32::MIN, i32::MAX);
        assert!(
            value == i32::MIN || value == i32::MIN + 1,
            "wrapping modulus 2 expected, got {value}"
        );
        seen[(value - i32::MIN) as usize] = true;
    }
    assert!(seen[0] && seen[1], "both residues of modulus 2 appear");
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
    // The static-table stream is untouched by range().
    assert_eq!(rng.state(), 22_227, "next_raw was not used by range()");
}
