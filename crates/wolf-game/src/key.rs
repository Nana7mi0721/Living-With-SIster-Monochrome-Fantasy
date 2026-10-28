//! WOLF's key bitmask, as polled by opcode 123.
//!
//! The game never asks about a specific physical key: every poll carries a mask and
//! only tests whether the result is non-zero, so what matters is that the port keeps
//! ONE mapping between physical keys and these bits.
//!
//! Hypothesis H-KEY-1 (see docs/UNVERIFIED.md): bits 0..3 are the action keys and bits
//! 4..7 the four directions. Evidence is indirect but consistent across all 218 polls in
//! this game: the masks actually used are {1, 7, 16, 17, 32, 33, 48, 49, 64, 144, 145,
//! 160, 176, 177, 183, 184, 241, 257, 512, 771, 784}. The single most frequent, 145
//! (91 uses), is 128|16|1 - a direction pair plus confirm, which is exactly how a menu
//! cursor is polled. 241 = all four directions plus confirm, the classic "any direction
//! or confirm" wait. 183 and 184 are near-total masks used as "any key".
//!
//! The ORDER of the four direction bits is the part that is not yet proven; it only
//! matters for menu navigation, so it is recorded as a hypothesis rather than a fact.

/// Confirm / "decide" key (Enter, Z, Space in the original).
pub const OK: u32 = 1;
/// Cancel / back (Escape, X).
pub const CANCEL: u32 = 2;
/// Sub key, WOLF's third action button (C, Shift).
pub const SUB: u32 = 4;
/// Pause key.
pub const PAUSE: u32 = 8;
/// Up.
pub const UP: u32 = 16;
/// Down.
pub const DOWN: u32 = 32;
/// Left.
pub const LEFT: u32 = 64;
/// Right.
pub const RIGHT: u32 = 128;
/// Bit 8 - meaning unverified, only ever seen inside the 771/784/257 masks.
pub const UNKNOWN_8: u32 = 256;
/// Bit 9 - meaning unverified, same masks as UNKNOWN_8.
pub const UNKNOWN_9: u32 = 512;

/// All four directions.
pub const DIRECTIONS: u32 = UP | DOWN | LEFT | RIGHT;
/// Every bit a keyboard can plausibly set in this game.
pub const ALL_KEYS: u32 = 0x3FF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_masks_the_game_actually_polls_are_all_within_our_bitset() {
        for mask in [
            1u32, 7, 16, 17, 32, 33, 48, 49, 64, 144, 145, 160, 176, 177, 183, 184, 241, 257, 771,
            784,
        ] {
            assert_eq!(mask & !ALL_KEYS, 0, "mask {mask} has bits we do not model");
        }
    }

    #[test]
    fn the_two_most_frequent_masks_decompose_as_expected() {
        // 145 = right or up plus confirm; 241 = any direction or confirm.
        assert_eq!(145, 128 | 16 | OK);
        assert_eq!(241, DIRECTIONS | OK);
    }
}
