using System;

namespace Lwsmf.Core;

/// <summary>
/// Port of crates/wolf-game/src/key.rs: opcode 123 hands scripts a KEY CODE
/// (10 decide, 11 cancel, 12 sub, numpad directions), not a bit mask.
/// The bits below are the port's own host-side layout.
/// </summary>
public static class KeyCodes
{
    public const uint Ok = 1;
    public const uint Cancel = 2;
    public const uint Sub = 4;
    public const uint Pause = 8;
    public const uint Up = 16;
    public const uint Down = 32;
    public const uint Left = 64;
    public const uint Right = 128;
    public const uint Unknown8 = 256;
    public const uint Unknown9 = 512;
    public const uint Directions = Up | Down | Left | Right;
    public const uint AllKeys = 0x3FF;

    public const int CodeNone = 0;
    public const int CodeOk = 10;
    public const int CodeCancel = 11;
    public const int CodeSub = 12;
    public const int CodeDown = 2;
    public const int CodeLeft = 4;
    public const int CodeRight = 6;
    public const int CodeUp = 8;
    public const int CodeDownLeft = 1;
    public const int CodeDownRight = 3;
    public const int CodeUpLeft = 7;
    public const int CodeUpRight = 9;

    /// <summary>The host bit a script key code corresponds to (key.rs bits_for_code).</summary>
    public static uint BitsForCode(int code) => code switch
    {
        CodeOk => Ok,
        CodeCancel => Cancel,
        CodeSub => Sub,
        CodeDown => Down,
        CodeLeft => Left,
        CodeRight => Right,
        CodeUp => Up,
        CodeDownLeft => Down | Left,
        CodeDownRight => Down | Right,
        CodeUpLeft => Up | Left,
        CodeUpRight => Up | Right,
        13 => Pause,
        _ => 0
    };

    /// <summary>The key code a script sees for the currently held keys (key.rs code).</summary>
    public static int Code(uint held)
    {
        if ((held & Ok) != 0) return CodeOk;
        if ((held & Cancel) != 0) return CodeCancel;
        if ((held & Sub) != 0) return CodeSub;
        if ((held & Pause) != 0) return 13;

        bool up = (held & Up) != 0;
        bool down = (held & Down) != 0;
        bool left = (held & Left) != 0;
        bool right = (held & Right) != 0;

        if (up && left) return CodeUpLeft;
        if (up && right) return CodeUpRight;
        if (down && left) return CodeDownLeft;
        if (down && right) return CodeDownRight;
        if (up) return CodeUp;
        if (down) return CodeDown;
        if (left) return CodeLeft;
        if (right) return CodeRight;
        return CodeNone;
    }

    /// <summary>True when the mask (the argument of opcode 123) has any known key bit.</summary>
    public static bool MaskTouchesKnownKey(uint mask) => (mask & AllKeys) != 0;

    /// <summary>
    /// Translate WOLF's editor key layout (decide 1, cancel 2, sub 4, down 0x10,
    /// left 0x20, right 0x40, up 0x80) into this port's host bits. Opcodes 125
    /// and 126 both name keys in the editor layout; direction bits differ.
    /// </summary>
    public static uint FromWolfBits(uint bits)
    {
        uint result = 0;
        if ((bits & 0x01) != 0) result |= Ok;
        if ((bits & 0x02) != 0) result |= Cancel;
        if ((bits & 0x04) != 0) result |= Sub;
        if ((bits & 0x10) != 0) result |= Down;
        if ((bits & 0x20) != 0) result |= Left;
        if ((bits & 0x40) != 0) result |= Right;
        if ((bits & 0x80) != 0) result |= Up;
        return result;
    }

    /// <summary>
    /// True when an opcode-123 poll accepts at least one bit of a WOLF-layout
    /// automatic-input mask (wolf-vm KeyQuery::accepts_wolf_bits). Device polls
    /// accept no scripted basic-input keys.
    /// </summary>
    public static bool AcceptsWolfBits(uint query, uint bits)
    {
        if (((query >> 8) & 0x0F) != 0) return false;
        uint options = query & 0xFF;
        uint directions = (options & 0x0F) switch
        {
            1 or 2 => 0x10 | 0x20 | 0x40 | 0x80,
            3 => 0x80,
            4 => 0x10,
            5 => 0x20,
            6 => 0x40,
            7 => 0x10 | 0x80,
            8 => 0x20 | 0x40,
            _ => 0
        };
        uint actions = ((options & 0x10) != 0 ? 0x01u : 0u)
            | ((options & 0x20) != 0 ? 0x02u : 0u)
            | ((options & 0x40) != 0 ? 0x04u : 0u);
        return (bits & (directions | actions)) != 0;
    }

    /// <summary>Host bit mask accepted by an opcode-123 query when device nibble is 0.</summary>
    public static uint AcceptedHostMask(uint query)
    {
        if (((query >> 8) & 0x0F) != 0) return AllKeys;
        uint options = query & 0xFF;
        uint dirs = (options & 0x0F) switch
        {
            1 or 2 => Directions,
            3 => Up,
            4 => Down,
            5 => Left,
            6 => Right,
            7 => Up | Down,
            8 => Left | Right,
            _ => 0
        };
        uint actions = ((options & 0x10) != 0 ? Ok : 0u)
            | ((options & 0x20) != 0 ? Cancel : 0u)
            | ((options & 0x40) != 0 ? Sub : 0u);
        uint accepted = dirs | actions;
        return accepted != 0 ? accepted : AllKeys;
    }
}
