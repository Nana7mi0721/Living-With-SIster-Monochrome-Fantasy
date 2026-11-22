using Godot;
using Lwsmf.Core;

namespace Lwsmf.Presentation;

/// <summary>
/// Thin GDScript-facing wrapper around the single C# <see cref="Terrain"/> implementation.
/// MapLayer must not re-implement passability decoding (DECISIONS §2.3); it asks this
/// bridge for the direction mask of each cell after loading map_tiles.json.
/// </summary>
[GlobalClass]
public partial class TerrainBridge : Godot.Node
{
    private Terrain? _terrain;
    private int _width;
    private int _height;

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

    public int Width => _width;
    public int Height => _height;
    public bool Valid => _terrain != null;

    public int Mask(int x, int y) =>
        _terrain == null ? 0 : _terrain.BlockedMask(x, y);

    public bool IsBlocked(int x, int y, int step) =>
        _terrain != null && _terrain.IsBlocked(x, y, step);

    public int BlockedCount() => _terrain?.BlockedCount() ?? 0;
    public int SolidCount() => _terrain?.SolidCount() ?? 0;
}
