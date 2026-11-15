using System;

namespace Lwsmf.Core;

public enum MagicKind
{
    Literal,
    SelfVar,
    Var,
    StrVar,
    SysVar,
    Opaque
}

public readonly struct Magic
{
    public const int SelfBase = 1_000_000;
    public const int VarBase = 2_000_000;
    public const int StrBase = 3_000_000;
    public const int SysBase = 9_000_000;
    public const int SelfGroupStride = 100_000;
    public const int SelfSlotsPerGroup = 100;
    public const int SelfGroups = 10;
    public const int SelfArgBase = 6 * SelfSlotsPerGroup; // 600

    public int Raw { get; }
    public MagicKind Kind { get; }
    public uint Index { get; }

    public Magic(int raw, MagicKind kind, uint index)
    {
        Raw = raw;
        Kind = kind;
        Index = index;
    }

    public byte Flags => (byte)((Raw >> 28) & 0x0F);
    public bool IsStorage => Kind != MagicKind.Literal && Kind != MagicKind.Opaque;
    public bool IsString => Kind == MagicKind.StrVar;

    public static Magic Decode(int raw)
    {
        byte flags = (byte)((raw >> 28) & 0x0F);
        int body = raw & 0x0FFF_FFFF;
        MagicKind kind;
        uint index;

        if (flags != 0 && body >= SelfBase && body < StrBase)
        {
            (kind, index) = Classify(body);
        }
        else if (flags != 0)
        {
            kind = MagicKind.Opaque;
            index = 0;
        }
        else
        {
            (kind, index) = Classify(body);
        }

        return new Magic(raw, kind, index);
    }

    private static (MagicKind, uint) Classify(int body)
    {
        if (body >= 0 && body <= 999_999)
            return (MagicKind.Literal, (uint)body);
        if (body >= SelfBase && body < VarBase)
            return (MagicKind.SelfVar, SelfSlot(body));
        if (body >= VarBase && body < StrBase)
            return (MagicKind.Var, (uint)(body - VarBase));
        if (body >= StrBase && body < 4_000_000)
            return (MagicKind.StrVar, (uint)(body - StrBase));
        if (body >= SysBase && body <= 9_999_999)
            return (MagicKind.SysVar, (uint)(body - SysBase));
        return (MagicKind.Opaque, 0);
    }

    public static uint SelfSlot(int value)
    {
        int id = Math.Max(0, value - SelfBase);
        int group = id / SelfGroupStride;
        int slot = id % SelfGroupStride;
        if (group == 0)
        {
            return (uint)(slot % SelfSlotsPerGroup);
        }
        return (uint)(Math.Min(group, SelfGroups - 1) * SelfSlotsPerGroup + slot);
    }
}
