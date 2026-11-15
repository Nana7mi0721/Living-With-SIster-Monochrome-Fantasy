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
}
