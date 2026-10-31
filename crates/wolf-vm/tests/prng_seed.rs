//! Deterministic replay proof for the production event-RNG seed hook.
//!
//! The event RNG (opcode 0x6000) is a Park-Miller MINSTD generator seeded by
//! EVENT_SEED=1 in a default build. Retail self-seeds it from the clock; this
//! port keeps a fixed default and adds an opt-in WOLF_EVENT_SEED hook so a
//! captured clock seed can be supplied at runtime without changing the default
//! sequence. These tests pin that contract:
//!   1. absent/invalid seed -> fixed EVENT_SEED (default behaviour unchanged),
//!   2. a supplied seed is recorded and reproduces the same stream every time,
//!   3. the event stream is independent of the MSVC static-table stream.

use std::sync::Mutex;

use wolf_vm::rng::{
    event_seed_from_env, resolve_event_seed, WolfRng, EVENT_RESEED, EVENT_SEED, EVENT_SEED_ENV,
};

/// Serialises the one test that mutates the process environment.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn absent_or_invalid_seed_keeps_the_fixed_default() {
    // No variable at all: the production hook must resolve to EVENT_SEED.
    assert_eq!(resolve_event_seed(None), EVENT_SEED);

    // Empty, non-numeric, negative and overflow values all fall back too.
    for raw in ["", "   ", "abc", "-5", "4294967296", "0x10"] {
        assert_eq!(
            resolve_event_seed(Some(raw)),
            EVENT_SEED,
            "invalid seed {raw:?} must fall back to the fixed default"
        );
    }

    // The default constructors never read the hook and keep EVENT_SEED.
    assert_eq!(WolfRng::new().event_seed(), EVENT_SEED);
    assert_eq!(
        WolfRng::from_event_seed(EVENT_SEED).event_seed(),
        EVENT_SEED
    );
}

#[test]
fn supplied_seed_is_normalised_and_recorded() {
    assert_eq!(resolve_event_seed(Some("12345")), 12_345);
    assert_eq!(resolve_event_seed(Some("  678  ")), 678);
    // A zero seed is reseeded exactly like from_event_seed(0).
    assert_eq!(resolve_event_seed(Some("0")), EVENT_RESEED);

    // The generator records the seed it was actually built with.
    assert_eq!(WolfRng::from_event_seed(98_765).event_seed(), 98_765);
    assert_eq!(WolfRng::from_event_seed(0).event_seed(), EVENT_RESEED);

    // Re-seeding in place updates the recorded active seed.
    let mut rng = WolfRng::new();
    assert_eq!(rng.event_seed(), EVENT_SEED);
    rng.set_event_seed(4_242);
    assert_eq!(rng.event_seed(), 4_242);
}

#[test]
fn same_seed_replays_the_identical_event_stream() {
    let seed = 987_654_321;
    let mut a = WolfRng::from_event_seed(seed);
    let mut b = WolfRng::from_event_seed(seed);

    for step in 0..1_000 {
        assert_eq!(
            a.next_event_draw(),
            b.next_event_draw(),
            "event draw diverged at step {step}"
        );
        assert_eq!(a.range(1, 100), b.range(1, 100), "range(1,100) diverged");
        assert_eq!(a.range(-50, 50), b.range(-50, 50), "range(-50,50) diverged");
    }
    assert_eq!(a.draws(), b.draws());
    assert_eq!(a.event_state(), b.event_state());
}

#[test]
fn different_seeds_diverge() {
    let mut a = WolfRng::from_event_seed(1);
    let mut b = WolfRng::from_event_seed(2);

    let seq_a: Vec<u32> = (0..16).map(|_| a.next_event_draw()).collect();
    let seq_b: Vec<u32> = (0..16).map(|_| b.next_event_draw()).collect();
    assert_ne!(seq_a, seq_b, "distinct seeds must produce distinct streams");

    let ranges_a: Vec<i32> = (0..64).map(|_| a.range(1, 1000)).collect();
    let ranges_b: Vec<i32> = (0..64).map(|_| b.range(1, 1000)).collect();
    assert_ne!(ranges_a, ranges_b);
}

#[test]
fn park_miller_seed_one_matches_the_retail_vector() {
    // Known MINSTD stream from the existing unit vectors (RVA 0x35810).
    let expected = [
        16_807u32,
        282_475_249,
        1_622_650_073,
        984_943_658,
        1_144_108_930,
        470_211_272,
        101_027_544,
        1_457_850_878,
    ];
    let mut rng = WolfRng::from_event_seed(1);
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(rng.next_event_draw(), *want, "event draw {i} mismatch");
    }
}

#[test]
fn msvc_static_stream_is_independent_of_the_event_seed() {
    // Same MSVC seed, different event seeds: next_raw must be identical.
    let mut a = WolfRng::from_seeds(22_227, 1);
    let mut b = WolfRng::from_seeds(22_227, 999_999);

    for step in 0..32 {
        assert_eq!(
            a.next_raw(),
            b.next_raw(),
            "MSVC static stream changed at step {step}"
        );
    }
    assert_eq!(a.state(), b.state());

    // And the event streams from those two seeds genuinely differ.
    let ev_a: Vec<u32> = (0..8).map(|_| a.next_event_draw()).collect();
    let ev_b: Vec<u32> = (0..8).map(|_| b.next_event_draw()).collect();
    assert_ne!(ev_a, ev_b);
}

#[test]
fn environment_hook_reads_and_records_a_seed() {
    // Serialise with any other test that could touch the environment.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    std::env::remove_var(EVENT_SEED_ENV);
    assert_eq!(event_seed_from_env(), EVENT_SEED);
    // Default constructor ignores the hook.
    assert_eq!(WolfRng::new().event_seed(), EVENT_SEED);

    std::env::set_var(EVENT_SEED_ENV, "24680");
    assert_eq!(event_seed_from_env(), 24_680);
    let hooked = WolfRng::from_event_seed_env();
    assert_eq!(hooked.event_seed(), 24_680);
    // The default constructor still keeps the fixed event seed and stream.
    assert_eq!(WolfRng::new().event_seed(), EVENT_SEED);
    let mut default_a = WolfRng::new();
    let mut default_b = WolfRng::from_event_seed(EVENT_SEED);
    for _ in 0..16 {
        assert_eq!(default_a.next_event_draw(), default_b.next_event_draw());
    }

    std::env::set_var(EVENT_SEED_ENV, "0");
    assert_eq!(event_seed_from_env(), EVENT_RESEED);

    std::env::set_var(EVENT_SEED_ENV, "not-a-number");
    assert_eq!(event_seed_from_env(), EVENT_SEED);

    std::env::remove_var(EVENT_SEED_ENV);
}

#[test]
fn zero_width_range_still_consumes_one_draw() {
    let mut rng = WolfRng::from_event_seed(12_345);
    let before = rng.draws();

    // min == max: modulus 1, the value is min, but the draw counter advances.
    assert_eq!(rng.range(7, 7), 7);
    assert_eq!(rng.draws(), before + 1);

    // Reversed bounds still normalise to low = min(a, b).
    assert_eq!(rng.range(9, 2), 2);
    assert_eq!(rng.draws(), before + 2);
}
