using System;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// W3 Day3 core check: the C# Terrain port must reproduce the Rust passability oracle
/// (Dungeon 348 blocked cells, SampleMapB 160, story maps 0) and decode full/quarter
/// passability words the same way (crates/wolf-game/src/map_runtime.rs).
/// </summary>
public partial class TerrainTest : Node
{
    private int _failures;
    private int _checks;

    public override void _Ready() => CallDeferred(nameof(Run));

    private void Check(bool ok, string label)
    {
        _checks++;
        if (ok) return;
        _failures++;
        GD.PrintErr($"[TerrainTest] FAIL {label}");
    }

    private void Run()
    {
        string godotDir = ProjectSettings.GlobalizePath("res://");
        string repo = Path.GetFullPath(Path.Combine(godotDir, ".."));
        string programs = Path.Combine(repo, "game_data", "programs.json");
        string tiles = Path.Combine(repo, "game_data", "map_tiles.json");

        MapTileData? data = MapTileData.Load(tiles);
        Check(data != null, "map_tiles.json loads");
        if (data == null)
        {
            GD.PrintErr("TERRAIN_TEST_FAILED missing map_tiles.json");
            GetTree().Quit(1);
            return;
        }

        Check(data.Tilesets.Count == 6, $"tileset count 6, got {data.Tilesets.Count}");
        Check(data.Maps.Count == 9, $"map count 9, got {data.Maps.Count}");
        Check(data.Find(1000) != null, "Dungeon entry present");
        Check(data.Find(1005) != null, "SampleMapA entry present");
        Check(MapTileData.CachedArtifactCount >= 1, "artifact cached");

        // Full tiles: bits 4..7 are blocked directions. Quarter tiles: corners are passable.
        Check(Terrain.MaskFor(0x10u) == 1, "full tile up-blocked mask");
        Check(Terrain.MaskFor(0x80u) == 8, "full tile down-blocked mask");
        Check(Terrain.MaskFor(0x0Fu) == 0x0F, "full tile solid mask");
        Check(Terrain.MaskFor(0x04u) == 0x0F, "quarter tile with no passable corner is solid");
        Check(Terrain.MaskFor(0x04u | 0xF0u) == 0, "quarter tile with all corners passable is open");
        Check(Terrain.MaskFor(0x04u | 0x10u | 0x20u) == 8, "quarter tile top corners passable blocks down");
        Check(Terrain.MaskFor(0x04u | 0x10u | 0x40u) == 2, "quarter tile left corners passable blocks right");
        Check(Terrain.MaskFor(0x04u | 0x20u | 0x80u) == 4, "quarter tile right corners passable blocks left");
        Check(Terrain.MaskFor(0x04u | 0x40u | 0x80u) == 1, "quarter tile bottom corners passable blocks up");

        // Layer decoding: base chip, autotile word, empty sentinel.
        var synthetic = Terrain.FromLayers(3, 1, new[] { new[] { 0u, 100001u, Terrain.EmptyTile } }, new[] { 0x10u, 0x0Fu });
        Check(synthetic.BlockedMask(0, 0) == 1, "base chip decodes passability");
        Check(synthetic.BlockedMask(1, 0) == 15, "autotile word resolves quotient chip");
        Check(synthetic.BlockedMask(2, 0) == 0, "empty tile sentinel is skipped");
        Check(synthetic.IsSolidCell(1, 0) && !synthetic.IsPassableCell(1, 0), "solid cell helpers");
        Check(synthetic.IsBlocked(0, 0, 0) && !synthetic.IsBlocked(0, 0, 3), "directional block mask");
        Check(!synthetic.IsBlocked(-1, 0, 0) && !synthetic.IsBlocked(0, 0, 4), "out-of-range and bad step are not blocked");

        int CountBlocked(int mapId, out int solid)
        {
            solid = 0;
            MapDefinition? map = ProgramLoader.LoadMap(programs, "", mapId);
            if (map == null) return -1;
            Terrain? terrain = data.BuildTerrain(map);
            if (terrain == null) return -1;
            solid = terrain.SolidCount();
            return terrain.BlockedCount();
        }

        int dungeon = CountBlocked(1000, out int dungeonSolid);
        int sampleA = CountBlocked(1005, out int sampleASolid);
        int sampleB = CountBlocked(1007, out int sampleBSolid);
        Check(dungeon == 348, $"Dungeon blocked cells 348, got {dungeon}");
        Check(dungeonSolid == 114, $"Dungeon solid cells 114, got {dungeonSolid}");
        Check(sampleA == 0, $"SampleMapA blocked cells 0, got {sampleA}");
        Check(sampleASolid == 0, $"SampleMapA solid cells 0, got {sampleASolid}");
        Check(sampleB == 160, $"SampleMapB blocked cells 160, got {sampleB}");
        Check(sampleBSolid == 158, $"SampleMapB solid cells 158, got {sampleBSolid}");

        // MapRuntime consumes the grid: (32,3) is solid, (34,3) blocks only from above.
        MapDefinition? dungeonMap = ProgramLoader.LoadMap(programs, "", 1000);
        if (dungeonMap != null)
        {
            Terrain? terrain = data.BuildTerrain(dungeonMap);
            Check(terrain != null, "Dungeon terrain built");
            if (terrain != null)
            {
                var runtime = new MapRuntime(dungeonMap);
                runtime.SetTerrain(terrain);
                Check(!runtime.CanEnter(32, 3), "solid tile rejects entry");
                Check(!runtime.CanEnter(34, 3, 0), "up-blocked tile rejects entry from above");
                Check(runtime.CanEnter(34, 3, 3), "up-blocked tile accepts entry from below");
                Check(runtime.CanEnter(1, 1), "open tile still enterable");
            }
        }

        // TerrainBridge regressions: null map / missing tiles guard and unified empty-terrain passability.
        var bridge = new TerrainBridge();
        AddChild(bridge);
        Check(!bridge.LoadForMap(999_999, programs) && !bridge.Valid && bridge.Width == 0 && bridge.Height == 0,
            "TerrainBridge.LoadForMap with unknown map must return false without throwing");
        Check(bridge.Mask(0, 0) == 0 && !bridge.IsBlocked(0, 0, 0),
            "TerrainBridge empty terrain must report Mask==0 and IsBlocked==false");
        Check(!bridge.LoadForMap(1000, "") && !bridge.Valid,
            "TerrainBridge.LoadForMap with missing tiles path must return false cleanly");
        Check(bridge.LoadForMap(1000, programs) && bridge.Valid && bridge.Width > 0 && bridge.Height > 0,
            "TerrainBridge.LoadForMap loads Dungeon");
        Check(bridge.Mask(32, 3) == 0b1111 && bridge.IsBlocked(34, 3, 0) && !bridge.IsBlocked(34, 3, 3),
            "TerrainBridge Mask and IsBlocked match Dungeon terrain");

        int failures = _failures;
        if (failures == 0)
        {
            GD.Print($"TERRAIN_TEST_OK checks={_checks} blocked={dungeon},{sampleA},{sampleB} solid={dungeonSolid},{sampleASolid},{sampleBSolid}");
            GetTree().Quit(0);
        }
        else
        {
            GD.PrintErr($"TERRAIN_TEST_FAILED {failures}/{_checks}");
            GetTree().Quit(1);
        }
    }
}
