using System;
using System.IO;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// W3 Day4 parity gate, retired-adapter form: the core GameSession must reproduce the
/// frozen title -> new game -> SampleMapA -> save state (oracle/fixtures/
/// session_core_save_state.json, play_frames=418). The fixture was regenerated after D-39
/// fixed opcode 130 to resolve SysDatabase row 1 as SampleMapA (artifact 1005).
/// </summary>
public partial class SessionParityTest : Node
{
    private static int _checks;
    private static int _failures;

    private static void Check(bool ok, string what)
    {
        _checks++;
        if (ok) return;
        _failures++;
        GD.PrintErr("[SessionParityTest] FAIL: " + what);
    }

    private static string Normalize(string json)
    {
        SaveState? state = SaveCodec.FromJson(json);
        if (state == null) return "";
        state.SavedAt = "";
        return SaveCodec.ToJson(state);
    }

    public override void _Ready()
    {
        string root = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string repo = Path.GetFullPath(Path.Combine(root, ".."));
        string fixture = Path.Combine(repo, "oracle", "fixtures", "session_core_save_state.json");

        string saveDir = Path.Combine(Path.GetTempPath(), "lwsmf-parity-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(saveDir);
        string coreSave = Path.Combine(saveDir, "core.json");
        string dataPath = Path.Combine(repo, "game_data", "programs.json");

        SessionHarness harness;
        try
        {
            harness = new SessionHarness(this, dataPath, coreSave);
        }
        catch (Exception ex)
        {
            GD.PrintErr("[SessionParityTest] setup failed: " + ex.Message);
            GD.PrintErr($"SESSION_PARITY_TEST_FAILED 1/1");
            GetTree().Quit(1);
            return;
        }

        Check(harness.RunToBedroom(), "core title chain must reach a map (" + harness.Session.Phase + ")");
        Check(harness.Session.AutoEventCount == 98, $"auto-event scheduler must track 98 common events (got {harness.Session.AutoEventCount})");
        Check(harness.Session.SaveGame(), "core SaveGame on the reached map");
        SaveState? core = SaveCodec.Read(coreSave);
        Check(core != null, "core slot must be readable");
        if (core == null)
        {
            GD.PrintErr($"SESSION_PARITY_TEST_FAILED {_failures}/{_checks}");
            GetTree().Quit(1);
            return;
        }

        string coreJson = Normalize(SaveCodec.ToJson(core));
        Check(coreJson.Length > 0, "core state must serialize");
        Check(core.PlayFrames == 418, $"frozen route must stay at 418 frames (got {core.PlayFrames})");
        Check(core.MapId == 1005, $"frozen route must land on SampleMapA artifact 1005 (got {core.MapId})");
        Check(core.LeaderX == 62 && core.LeaderY == 35,
            $"frozen route must place the leader at 62,35 (got {core.LeaderX},{core.LeaderY})");

        if (!File.Exists(fixture))
        {
            Check(false, "frozen fixture missing: " + fixture);
        }
        else
        {
            string expected = Normalize(File.ReadAllText(fixture));
            Check(expected.Length > 0, "frozen fixture must parse");
            Check(expected == coreJson, "core SaveState must be field-identical to the frozen fixture");
            if (expected != coreJson)
            {
                string diffPath = Path.Combine(repo, "reports", "session_core_state.actual.json");
                File.WriteAllText(diffPath, coreJson);
                GD.PrintErr("actual state: " + diffPath);
            }
        }

        if (_failures == 0) GD.Print($"SESSION_PARITY_TEST_OK checks={_checks} frames={harness.Session.Frames} core=frozen");
        else GD.PrintErr($"SESSION_PARITY_TEST_FAILED {_failures}/{_checks}");
        GetTree().Quit(_failures == 0 ? 0 : 1);
    }
}
