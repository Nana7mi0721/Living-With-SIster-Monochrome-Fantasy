using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>
/// Per-cell, per-direction passability grid (crates/wolf-game/src/map_runtime.rs Terrain).
/// Bit order is the TilePassability::blocked order: bit 0 up, bit 1 right, bit 2 left,
/// bit 3 down. A set bit means entering the cell while moving that way is blocked.
/// </summary>
public sealed class Terrain
{
    public const uint EmptyTile = 0xFFFFFFFF;

    public int Width { get; }
    public int Height { get; }

    private readonly byte[] _blocked;

    /// <summary>An all-passable grid of the given tile size.</summary>
    public Terrain(int width, int height)
    {
        Width = Math.Max(0, width);
        Height = Math.Max(0, height);
        _blocked = new byte[checked(Width * Height)];
    }

    /// <summary>
    /// Build the grid from the map's three tile layers and the tileset's chip passability
    /// (map_runtime.rs Terrain::from_map). A raw tile word decodes as
    /// autotile_id = raw / 100_000; when that is non-zero the low digits select the
    /// autotile and the chip id is the quotient, otherwise raw is the base chip id.
    /// </summary>
    public static Terrain FromLayers(
        int width,
        int height,
        IReadOnlyList<uint[]> layers,
        IReadOnlyList<uint> passability)
    {
        var terrain = new Terrain(width, height);
        if (terrain.Width <= 0 || terrain.Height <= 0) return terrain;
        int count = terrain.Width * terrain.Height;
        foreach (uint[]? layer in layers)
        {
            if (layer == null || layer.Length < count) continue;
            for (int i = 0; i < count; i++)
            {
                uint raw = layer[i];
                if (raw == EmptyTile) continue;
                uint autotile = raw / 100_000u;
                uint chip = autotile > 0 ? autotile : raw;
                if (chip >= passability.Count) continue;
                terrain._blocked[i] |= MaskFor(passability[(int)chip]);
            }
        }
        return terrain;
    }

    /// <summary>
    /// Direction mask for one raw passability word (wolf-format tileset.rs decode).
    /// Full tiles: bits 4-7 are "not passable" per direction. Quarter tiles: the same
    /// bits are corner passability, and a side is blocked only when both corners fail.
    /// </summary>
    public static byte MaskFor(uint word)
    {
        bool quarter = (word & 0b100u) != 0;
        bool d0 = (word & 0x10u) != 0;
        bool d1 = (word & 0x20u) != 0;
        bool d2 = (word & 0x40u) != 0;
        bool d3 = (word & 0x80u) != 0;
        if (!quarter)
        {
            byte full = 0;
            if (d0) full |= 1;
            if (d1) full |= 2;
            if (d2) full |= 4;
            if (d3) full |= 8;
            return full;
        }
        // d0 = top-left passable, d1 = top-right, d2 = bottom-left, d3 = bottom-right.
        byte bits = 0;
        if (!(d0 || d1)) bits |= 1; // up
        if (!(d1 || d3)) bits |= 2; // right
        if (!(d0 || d2)) bits |= 4; // left
        if (!(d2 || d3)) bits |= 8; // down
        return bits;
    }

    /// <summary>Entering (x, y) while moving this way is blocked; out-of-range or bad step is false.</summary>
    public bool IsBlocked(int x, int y, int step)
    {
        if (step < 0 || step > 3) return false;
        if (x < 0 || y < 0 || x >= Width || y >= Height) return false;
        return (_blocked[y * Width + x] & (1 << step)) != 0;
    }

    /// <summary>All four directions blocked (or out of range), matching map_runtime.rs.</summary>
    public bool IsSolidCell(int x, int y)
    {
        if (x < 0 || y < 0 || x >= Width || y >= Height) return true;
        return _blocked[y * Width + x] == 0b1111;
    }

    public bool IsPassableCell(int x, int y) => !IsSolidCell(x, y);

    public byte BlockedMask(int x, int y)
    {
        if (x < 0 || y < 0 || x >= Width || y >= Height) return 0b1111;
        return _blocked[y * Width + x];
    }

    /// <summary>Force one cell's mask (tests / synthetic grids).</summary>
    public void SetTileBlocked(int x, int y, byte bits)
    {
        if (x < 0 || y < 0 || x >= Width || y >= Height) return;
        _blocked[y * Width + x] = bits;
    }

    /// <summary>Cells whose mask has at least one blocked direction.</summary>
    public int BlockedCount()
    {
        int count = 0;
        foreach (byte bits in _blocked)
        {
            if (bits != 0) count++;
        }
        return count;
    }

    /// <summary>Cells whose mask blocks all four directions.</summary>
    public int SolidCount()
    {
        int count = 0;
        foreach (byte bits in _blocked)
        {
            if (bits == 0b1111) count++;
        }
        return count;
    }
}
