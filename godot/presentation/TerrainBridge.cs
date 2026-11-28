using System;
using System.Collections.Generic;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Presentation;

/// <summary>
/// Thin GDScript-facing wrapper around the single C# <see cref="Terrain"/> implementation.
/// MapLayer must not re-implement passability decoding (DECISIONS §2.3); it asks this
/// bridge for the direction mask of each cell after loading map_tiles.json(.br).
/// </summary>
[GlobalClass]
public partial class TerrainBridge : Godot.Node
{
    private Terrain? _terrain;
    private int _width;
    private int _height;
    private string _mapFile = "";
    private int _mapWidth;
    private int _mapHeight;
    private int _tilesetId;
    private List<uint[]> _layers = new();

    public bool LoadForMap(int mapId, string programsPath)
    {
        if (string.IsNullOrWhiteSpace(programsPath))
        {
            _terrain = null;
            _width = 0;
            _height = 0;
            return false;
        }
        MapDefinition? map = ProgramLoader.LoadMap(programsPath, "", mapId);
        MapTileData? tiles = MapTileData.LoadAlongside(programsPath);
        _terrain = (map != null && tiles != null) ? tiles.BuildTerrain(map) : null;
        _width = _terrain?.Width ?? 0;
        _height = _terrain?.Height ?? 0;
        return _terrain != null;
    }

    public bool LoadMapFromTiles(int mapId, string mapTilesPath, string programsPath)
    {
        _terrain = null;
        _width = 0;
        _height = 0;
        _mapFile = "";
        _mapWidth = 0;
        _mapHeight = 0;
        _tilesetId = 0;
        _layers = new List<uint[]>();

        if (string.IsNullOrWhiteSpace(mapTilesPath)) return false;
        MapTileData? tiles = MapTileData.Load(mapTilesPath);
        MapTileEntry? entry = tiles?.Find(mapId);
        if (tiles == null || entry == null) return false;

        bool hasTileset = false;
        foreach (MapTileset candidate in tiles.Tilesets)
        {
            if (candidate.Index == entry.TilesetId)
            {
                hasTileset = true;
                break;
            }
        }
        if (!hasTileset) return false;

        _mapFile = entry.File;
        _mapWidth = entry.Width;
        _mapHeight = entry.Height;
        _tilesetId = entry.TilesetId;
        _layers = entry.Layers;

        if (!string.IsNullOrWhiteSpace(programsPath))
        {
            MapDefinition? map = ProgramLoader.LoadMap(programsPath, "", mapId);
            _terrain = map != null ? tiles.BuildTerrain(map) : null;
            _width = _terrain?.Width ?? 0;
            _height = _terrain?.Height ?? 0;
        }
        return true;
    }

    public int Width => _width;
    public int Height => _height;
    public bool Valid => _terrain != null;
    public string MapFile => _mapFile;
    public int MapWidth => _mapWidth;
    public int MapHeight => _mapHeight;
    public int TilesetId => _tilesetId;
    public int LayerCount => _layers.Count;

    public long[] GetLayerWords(int index)
    {
        if (index < 0 || index >= _layers.Count) return Array.Empty<long>();
        uint[] src = _layers[index];
        var dst = new long[src.Length];
        for (int i = 0; i < src.Length; i++)
        {
            dst[i] = src[i];
        }
        return dst;
    }

    public int Mask(int x, int y) =>
        _terrain == null ? 0 : _terrain.BlockedMask(x, y);

    /// <summary>Return all cell passability masks in row-major order (y * Width + x) in one bridge call.</summary>
    public byte[] GetAllMasks()
    {
        if (_terrain == null || _width <= 0 || _height <= 0) return Array.Empty<byte>();
        int w = _width;
        int h = _height;
        var masks = new byte[w * h];
        for (int y = 0; y < h; y++)
        {
            int row = y * w;
            for (int x = 0; x < w; x++)
            {
                masks[row + x] = _terrain.BlockedMask(x, y);
            }
        }
        return masks;
    }

    public bool IsBlocked(int x, int y, int step) =>
        _terrain != null && _terrain.IsBlocked(x, y, step);

    public int BlockedCount() => _terrain?.BlockedCount() ?? 0;
    public int SolidCount() => _terrain?.SolidCount() ?? 0;
}
