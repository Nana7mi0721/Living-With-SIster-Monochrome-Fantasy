# M9 W5-a - Production event-RNG seed hook and deterministic replay

Status: implemented in crates/wolf-vm/src/rng.rs, proof in
crates/wolf-vm/tests/prng_seed.rs. Default behaviour is unchanged.

## Problem

The retail WOLF build self-seeds the opcode 0x6000 event RNG from the clock at
startup, so two retail runs of the same save with the same input can diverge.
This port replaced that with a fixed seed: EVENT_SEED = 1 (park-miller MINSTD,
state = state * 16807 % 0x7FFFFFFF, reject while the state exceeds 2,000,000,000,
reseed on zero). That makes the port deterministic but leaves a residual parity
gap: a retail run cannot be reproduced bit for bit until the retail clock seed
for that run is known and can be supplied.

## Added API (crates/wolf-vm/src/rng.rs)

All additions are additive; no existing signature or default changed.

- pub const EVENT_SEED_ENV: &str = "WOLF_EVENT_SEED"
  - Name of the environment variable the production hook reads.
- pub fn resolve_event_seed(raw: Option<&str>) -> u32
  - Pure resolver. None (variable absent) or any unparsable value (empty,
    whitespace, non-numeric, negative, overflow) returns EVENT_SEED (1), so the
    default build is unchanged. A valid u32 is trimmed and then zero-normalised
    exactly like from_event_seed: 0 becomes EVENT_RESEED (1).
- pub fn event_seed_from_env() -> u32
  - Reads WOLF_EVENT_SEED and returns resolve_event_seed of it. This is the only
    place the process environment is consulted.
- pub fn WolfRng::from_event_seed_env() -> Self
  - Production constructor: from_event_seed(event_seed_from_env()). Leaves the
    MSVC static-table stream at its default (from_event_seed keeps DEFAULT_SEED
    22227 for state).
- pub const fn WolfRng::event_seed(&self) -> u32
  - The seed the event generator was last (re)seeded with, after zero
    normalisation. Recorded in a new field event_seed: u32, set by from_seeds and
    by set_event_seed. This is the number to log and to feed back for a replay.

Default path is deliberately untouched: WolfRng::new(), from_seed(),
from_seeds(), from_event_seed() and set_event_seed() never read the
environment, so the fixed EVENT_SEED sequence is identical to before this
change.

## Replay recipe

To reproduce a captured run:

    WOLF_EVENT_SEED=<recorded event_seed> cargo run ... (or from_event_seed_env)

The recorded number is WolfRng::event_seed() from the original run. Because the
event generator is independent of the MSVC static-table stream (see below), the
same save plus the same input plus the same seed replays the same event stream.
Two generators built from the same seed produce identical next_event_draw() and
range(a,b) sequences (verified for 1000 steps in the test).

## Guarantees proven by crates/wolf-vm/tests/prng_seed.rs

1. absent_or_invalid_seed_keeps_the_fixed_default - None, "", "   ", "abc",
   "-5", "4294967296", "0x10" all resolve to EVENT_SEED; WolfRng::new() records
   EVENT_SEED.
2. supplied_seed_is_normalised_and_recorded - "12345" -> 12345, " 678 " -> 678,
   "0" -> EVENT_RESEED; from_event_seed(seed) records it; set_event_seed updates
   the recorded seed.
3. same_seed_replays_the_identical_event_stream - two independent generators
   from the same seed match on 1000 next_event_draw / range calls and end with
   equal draws() and event_state().
4. different_seeds_diverge - seeds 1 and 2 produce different event draws and
   different range sequences.
5. park_miller_seed_one_matches_the_retail_vector - the known MINSTD seed-1
   vector still holds.
6. msvc_static_stream_is_independent_of_the_event_seed - two generators with the
   same MSVC seed but different event seeds produce identical next_raw()
   sequences, while their event streams differ. This proves the two generators
   are separate, so the static table is unaffected by the event seed hook.
7. environment_hook_reads_and_records_a_seed - with WOLF_EVENT_SEED unset,
   event_seed_from_env() == EVENT_SEED; with it set to 24680, both
   event_seed_from_env() and from_event_seed_env().event_seed() return 24680
   while WolfRng::new() still yields the default event stream; "0" -> EVENT_RESEED;
   "not-a-number" -> EVENT_SEED. The test restores the environment.
8. zero_width_range_still_consumes_one_draw - range(7,7) returns 7 and still
   advances the draw counter by one; reversed bounds normalise to low = min.

## Exact residual retail clock-seed parity gap

What is still missing for bit-exact parity with a real retail run:

- The retail binary seeds the event RNG from a clock value at startup. Which
  clock (GetTickCount, time(), QueryPerformanceCounter) and exactly how that raw
  value is folded into the seed is not documented in the shipped data or the
  reverse-engineering notes, and this port does not capture it.
- This port fixes EVENT_SEED = 1 by default and only lets a caller inject a seed
  via WOLF_EVENT_SEED / from_event_seed_env(). If the retail clock seed for a
  run is captured by instrumenting the retail binary (or derived from a retail
  save that recorded it), supplying that number reproduces the run. Without that
  captured number, retail and port streams will differ from the first event draw.
- Zero handling is assumed: this port maps a zero seed to EVENT_RESEED (1).
  Whether retail does the same on a zero clock seed is unverified.

So: the hook removes the port's rigidity and makes same-seed replay provable,
but the retail clock read itself is still an open capture task, not a solved
seed.
