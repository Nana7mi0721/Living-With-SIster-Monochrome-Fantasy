using System;

namespace Lwsmf.Core;

public sealed class WolfRng
{
    public const uint DefaultSeed = 22227;
    public const uint EventSeedDefault = 1;
    public const uint EventReseed = 1;

    private const uint Multiplier = 0x000343FD;
    private const uint Increment = 0x00269EC3;
    private const ulong ParkMillerMultiplier = 16807;
    private const ulong ParkMillerModulus = 0x7FFFFFFF;
    private const ulong EventCeiling = 2_000_000_000;

    private uint _state;
    private uint _eventState;
    private uint _eventSeed;
    private ulong _draws;

    public WolfRng() : this(DefaultSeed, EventSeedDefault) { }

    public WolfRng(uint seed) : this(seed, EventSeedDefault) { }

    public WolfRng(uint seed, uint eventSeed)
    {
        _state = seed;
        _eventSeed = NormaliseEventSeed(eventSeed);
        _eventState = _eventSeed;
        _draws = 0;
    }

    public static WolfRng FromEventSeed(uint eventSeed) => new(DefaultSeed, eventSeed);

    /// <summary>
    /// Rust keeps the table cursor at zero unless WOLF_LIVE_RNG is set; the Godot windowed
    /// entry opts into clock seeding with that switch while headless runs stay deterministic.
    /// The C# port mirrors the policy, not the table: absent or "0" means the fixed default.
    /// </summary>
    public static WolfRng FromEnvironment()
    {
        if (!CoreRuntimeConfig.LiveRng)
            return new WolfRng();
        // Opcode randomness drains the event stream rather than _state; Keep _state on
        // its retail default and seed only the event cursor from the clock.
        return FromEventSeed(unchecked((uint)Environment.TickCount64));
    }

    public uint State => _state;
    public uint EventState => _eventState;
    public uint EventSeed => _eventSeed;
    public ulong Draws => _draws;

    public void SetEventSeed(uint eventSeed)
    {
        _eventSeed = NormaliseEventSeed(eventSeed);
        _eventState = _eventSeed;
    }

    /// <summary>Restore the exact generator state stored in a save slot.</summary>
    public void RestoreState(uint state, uint eventState, uint eventSeed, ulong draws)
    {
        _state = state;
        _eventState = eventState;
        _eventSeed = NormaliseEventSeed(eventSeed);
        _draws = draws;
    }

    private static uint NormaliseEventSeed(uint seed) => seed == 0 ? EventReseed : seed;

    public ushort NextRaw()
    {
        unchecked
        {
            _state = _state * Multiplier + Increment;
            return (ushort)((_state >> 16) & 0x7FFF);
        }
    }

    public uint NextEventDraw()
    {
        while (true)
        {
            ulong next = ((ulong)_eventState * ParkMillerMultiplier) % ParkMillerModulus;
            uint n = (uint)next;
            _eventState = n == 0 ? EventReseed : n;
            if ((ulong)_eventState <= EventCeiling)
            {
                return _eventState;
            }
        }
    }

    public int Range(int min, int max)
    {
        _draws++;
        uint draw = NextEventDraw();
        long lo = Math.Min((long)min, (long)max);
        long hi = Math.Max((long)min, (long)max);
        ulong span = (ulong)(hi - lo) + 1UL;
        ulong offset = (ulong)draw % span;
        return unchecked((int)(lo + (long)offset));
    }
}
