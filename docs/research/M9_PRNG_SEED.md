# M9 W5-a - Production event-RNG seed hook and deterministic replay

Status: implemented in crates/wolf-vm/src/rng.rs, proof in
crates/wolf-vm/tests/prng_seed.rs (10/10). Interactive `wolf-cli play` now seeds
from the local clock; the library default is unchanged.

## Problem

The retail WOLF build self-seeds the opcode 0x6000 event RNG from the clock at
startup, so two retail runs of the same save with the same input can diverge.
The library uses a fixed seed by default: EVENT_SEED = 1 (park-miller MINSTD,
state = state * 16807 % 0x7FFFFFFF, reject while the state exceeds 2,000,000,000,
reseed on zero). That keeps tests and diagnostic commands deterministic.
Interactive `wolf-cli play` seeds from the local clock, so normal play gets a
fresh stream each launch; `WOLF_EVENT_SEED` restores an explicit seed for
byte-for-byte replay.

## Added API (crates/wolf-vm/src/rng.rs)

All additions are additive; no existing signature or default changed.

- pub const EVENT_SEED_ENV: &str = "WOLF_EVENT_SEED"
  - Name of the environment variable the production hook reads.
- pub fn resolve_event_seed(raw: Option<&str>) -> u32
  - Pure resolver. None (variable absent) or any unparsable value (empty,
    whitespace, non-numeric, negative, overflow) returns EVENT_SEED (1), so the
    default build is unchanged. A valid u32 is trimmed and then zero-normalised
    exactly like from_event_seed: 0 becomes EVENT_RESEED (1).
- pub fn resolve_session_event_seed(raw: Option<&str>, clock_ticks: u32) -> u32
  - Production policy for an interactive front end. Some(raw) returns
    resolve_event_seed(raw); None returns seed_from_clock_ticks(clock_ticks) with
    zero normalised to EVENT_RESEED. Pure: it reads neither the environment nor
    the clock, which keeps it directly testable.
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

## Production wiring (2026-11-03)

`crates/wolf-cli/src/play.rs` installs the session seed after `Game::new(root)`
succeeds and before any boot/load-save event runs:

    let seed_override = std::env::var(EVENT_SEED_ENV).ok();
    let clock_ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u32);
    let event_seed = resolve_session_event_seed(seed_override.as_deref(), clock_ticks);
    game.vm.rng.set_event_seed(event_seed);

It logs `startup: event RNG seed <seed> (clock|WOLF_EVENT_SEED)`. The 32-bit
milliseconds-since-epoch value is a portable stand-in for GetTickCount; because
`seed_from_clock_ticks` is identity folding, only the 32-bit tick value matters
to the stream, not the epoch offset. Real runs: two launches produced seeds
2675025206 and 2675026572; `WOLF_EVENT_SEED=24680` produced 24680 with source
WOLF_EVENT_SEED.

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
9. production_policy_uses_the_clock_when_no_override_is_supplied - None with
   clock 4321 gives 4321; a large tick is preserved; zero clock gives
   EVENT_RESEED; equal ticks give equal streams and different ticks differ.
10. production_policy_prefers_an_explicit_seed_over_the_clock - Some("24680")
    wins over any clock including 1 and u32::MAX; Some("0") gives EVENT_RESEED;
    Some("not-a-number") gives EVENT_SEED rather than falling back to the clock.

## Optional retail-run parity check

Production seeding is no longer a gap: `wolf-cli play` seeds from the local
clock and `WOLF_EVENT_SEED` replays an explicit tick. What remains is only an
optional verification against one specific retail run:

- `docs/research/M11_PRNG_CLOCK.md` establishes that the retail binary calls
  GetTickCount at RVA 0x35847 and stores the raw DWORD into the event state at
  VA 0x011b17f0 (RVA 0x35852); a zero state re-reads the clock. So the clock
  source is known, not merely assumed.
- To align bit for bit with a particular retail run, capture that run's raw
  GetTickCount and pass it as `WOLF_EVENT_SEED=<tick>`. Without that captured
  number, streams still differ from the first event draw, which is expected
  because the clock differs; this is a verification convenience, not a
  functional defect in the port.
- Zero handling is still assumed: this port maps a zero seed to EVENT_RESEED
  (1). Whether retail does the same on a zero clock tick remains unverified.
