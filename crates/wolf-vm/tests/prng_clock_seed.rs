//! Retail clock-seed provenance for the Park-Miller event generator.
//!
//! RVA 0x35810 is the event RNG behind opcode 121's 0x6000 operator. Its first
//! seed is the raw DWORD returned by KERNEL32!GetTickCount:
//!
//!   0x35847  FF 15 44 20 80 00   call dword ptr [0x802044]   ; KERNEL32!GetTickCount
//!   0x35852  A3 F0 17 1B 01      mov  dword ptr [0x11b17f0], eax
//!
//! There is no add/xor/shift/multiply folding before the first Park-Miller
//! step: the tick is written straight into the state. These tests pin that
//! identity and the resulting stream against vectors computed independently in
//! Python from the same rule. See docs/research/M11_PRNG_CLOCK.md for the full
//! disassembly, the IAT proof, and the residual zero-tick case.

use wolf_vm::rng::{seed_from_clock_ticks, WolfRng, EVENT_RESEED, EVENT_SEED};

/// Independent implementation of the retail Park-Miller step used to generate
/// the expected vectors (state = state * 16807 mod 0x7FFFFFFF, multiply again
/// while the state exceeds 2,000,000,000, and map zero to the build's fixed
/// EVENT_RESEED).
fn independent_stream(seed: u32, count: usize) -> Vec<u32> {
    let mut state = seed;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        loop {
            let next = (u64::from(state) * 16_807 % 0x7FFF_FFFF) as u32;
            state = if next == 0 { EVENT_RESEED } else { next };
            if u64::from(state) <= 2_000_000_000 {
                break;
            }
        }
        out.push(state);
    }
    out
}

fn draws(rng: &mut WolfRng, count: usize) -> Vec<u32> {
    (0..count).map(|_| rng.next_event_draw()).collect()
}

#[test]
fn clock_ticks_are_stored_without_folding() {
    // The disassembly at 0x35847/0x35852 is an identity transformation.
    assert_eq!(seed_from_clock_ticks(0), 0);
    assert_eq!(seed_from_clock_ticks(1), 1);
    assert_eq!(seed_from_clock_ticks(1000), 1000);
    assert_eq!(seed_from_clock_ticks(0x1234_5678), 0x1234_5678);
    assert_eq!(seed_from_clock_ticks(u32::MAX), u32::MAX);

    // Non-zero ticks are recorded verbatim as the active event seed.
    assert_eq!(
        WolfRng::from_clock_ticks(123_456_789).event_seed(),
        123_456_789
    );
    assert_eq!(
        WolfRng::from_clock_ticks(0x7FFF_FFFE).event_seed(),
        0x7FFF_FFFE
    );
}

#[test]
fn retail_tick_stream_matches_independently_computed_vectors() {
    // Python-generated vectors: seed = tick, then the retail Park-Miller loop.
    let cases: [(u32, [u32; 8]); 7] = [
        (
            1,
            [
                16_807,
                282_475_249,
                1_622_650_073,
                984_943_658,
                1_144_108_930,
                470_211_272,
                101_027_544,
                1_457_850_878,
            ],
        ),
        (
            1_000,
            [
                16_807_000,
                1_154_891_243,
                1_299_919_515,
                1_396_147_674,
                1_647_629_796,
                95_812_591,
                1_856_965_334,
                636_526_687,
            ],
        ),
        (
            1_000_000,
            [
                1_774_614_471,
                1_692_524_561,
                691_908_565,
                283_303_450,
                509_838_751,
                400_136_527,
                1_323_310_532,
                1_539_462_992,
            ],
        ),
        (
            3_600_000,
            [
                375_657_884,
                80_134_208,
                343_387_187,
                1_019_892_420,
                117_432_586,
                152_001_309,
                1_327_944_080,
                131_021_851,
            ],
        ),
        (
            0x1234_5678,
            [
                706_275_742,
                1_234_278_825,
                1_979_665_402,
                1_272_268_443,
                521_048_322,
                1_968_319_035,
                1_699_922_857,
                481_017_911,
            ],
        ),
        (
            123_456_789,
            [
                469_049_721,
                1_781_357_515,
                1_206_231_778,
                891_865_166,
                141_988_902,
                553_144_097,
                236_130_416,
                94_122_056,
            ],
        ),
        (
            0x7FFF_FFFE,
            [
                1_865_008_398,
                524_833_574,
                1_162_539_989,
                1_003_374_717,
                1_677_272_375,
                689_632_769,
                688_705_724,
                140_245_938,
            ],
        ),
    ];

    for (tick, expected) in cases {
        let actual = draws(&mut WolfRng::from_clock_ticks(tick), expected.len());
        assert_eq!(actual, expected.to_vec(), "tick {tick} stream mismatch");

        // Cross-check the independent formula too, so a typo in the table is
        // caught by a second derivation rather than trusted blindly.
        assert_eq!(
            actual,
            independent_stream(tick, expected.len()),
            "tick {tick} disagrees with the independent Park-Miller formula"
        );
    }
}

#[test]
fn same_tick_replays_identically_and_different_ticks_diverge() {
    let tick = 1_000_000_u32;
    let first = draws(&mut WolfRng::from_clock_ticks(tick), 64);
    let replay = draws(&mut WolfRng::from_clock_ticks(tick), 64);
    assert_eq!(first, replay, "same captured tick must replay identically");

    let other = draws(&mut WolfRng::from_clock_ticks(tick + 1), 64);
    assert_ne!(
        first, other,
        "different ticks must produce different streams"
    );

    // Also pin the range reduction observed from the same two ticks.
    let mut a = WolfRng::from_clock_ticks(tick);
    let mut b = WolfRng::from_clock_ticks(tick + 1);
    let ra: Vec<i32> = (0..8).map(|_| a.range(1, 100)).collect();
    let rb: Vec<i32> = (0..8).map(|_| b.range(1, 100)).collect();
    assert_eq!(ra, vec![72, 62, 66, 51, 52, 28, 33, 93]);
    assert_eq!(rb, vec![79, 11, 92, 9, 82, 100, 77, 24]);
    assert_ne!(ra, rb);
}

#[test]
fn zero_tick_uses_documented_deterministic_fallback() {
    // Retail re-reads GetTickCount when the state becomes zero, so a single
    // captured zero tick cannot reproduce retail. This build documents and
    // pins the deterministic substitution: zero maps to EVENT_RESEED.
    assert_eq!(seed_from_clock_ticks(0), 0);
    let mut zero = WolfRng::from_clock_ticks(0);
    assert_eq!(zero.event_seed(), EVENT_RESEED);
    assert_eq!(
        draws(&mut zero, 8),
        draws(&mut WolfRng::from_event_seed(EVENT_RESEED), 8)
    );
    // EVENT_RESEED is 1, so the fallback equals a captured tick of 1; compare
    // against a different tick to prove the fallback is a concrete substitution.
    assert_eq!(
        draws(&mut WolfRng::from_clock_ticks(0), 8),
        draws(&mut WolfRng::from_clock_ticks(1), 8)
    );
    assert_ne!(
        draws(&mut WolfRng::from_clock_ticks(0), 8),
        draws(&mut WolfRng::from_clock_ticks(1_000), 8)
    );
}

#[test]
fn clock_seed_api_does_not_change_existing_defaults() {
    // Existing public behaviour must stay byte-for-byte: new() and
    // from_event_seed(EVENT_SEED) still produce the fixed pinned stream.
    assert_eq!(EVENT_SEED, 1);
    assert_eq!(WolfRng::new().event_seed(), EVENT_SEED);
    assert_eq!(
        draws(&mut WolfRng::new(), 8),
        draws(&mut WolfRng::from_event_seed(EVENT_SEED), 8)
    );
    assert_eq!(
        draws(&mut WolfRng::new(), 8),
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
}
