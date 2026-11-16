using System;
using System.Collections.Generic;
using System.Linq;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Drives the core session headless and checks the presentation hand-off: title event
/// runs, the new-game chain teleports through map 3, and the bedroom screen owns the
/// frame once the chain reaches SampleMapA (artifact 1005).
/// </summary>
public partial class PresentationTest : Node
{
    private readonly List<string> _failures = new();

    private void Check(bool condition, string message)
    {
        if (!condition) _failures.Add(message);
    }

    public override void _Ready()
    {
        string dataPath = System.IO.Path.Combine(
            ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\'), "..", "game_data", "programs.json");
        string savePath = System.IO.Path.Combine(
            System.IO.Path.GetTempPath(), "lwsmf-presentation-" + Guid.NewGuid().ToString("N") + ".json");

        SessionHarness harness;
        try
        {
            harness = new SessionHarness(this, dataPath, savePath);
        }
        catch (Exception ex)
        {
            GD.PrintErr("[PresentationTest] FAIL: " + ex.Message);
            GetTree().Quit(1);
            return;
        }

        GameSession session = harness.Session;
        EventVm vm = harness.Vm.Vm;

        var visits = new Dictionary<int, int>();
        int lastPc = -1;
        vm.StepHook = (inst, pc) =>
        {
            visits[pc] = visits.TryGetValue(pc, out int count) ? count + 1 : 1;
            lastPc = pc;
        };

        GD.Print($"[PresentationTest] phase={session.Phase} dataPath loaded=True");

        int frames = 0;
        for (; frames < 4000 && session.Phase is "title" or "newgame"; frames++)
        {
            session.AdvanceFrame();
        }

        GD.Print($"[PresentationTest] phase={session.Phase} frames={frames} vmFrames={session.Frames}");

        string topPcs = string.Join(" ", visits.OrderByDescending(kv => kv.Value).Take(14).Select(kv => $"{kv.Key}:{kv.Value}"));
        GD.Print($"[PresentationTest] lastPc={lastPc} topPcs={topPcs}");
        foreach (var inst in vm.Instances)
        {
            GD.Print($"[PresentationTest] inst={inst.Id} prog='{inst.Program.Title}' pc={inst.Pc}/{inst.Program.Len} wait={inst.Wait.Kind} frames={inst.Wait.Frames}");
        }

        Check(session.Phase == "bedroom", $"phase '{session.Phase}' != 'bedroom' after {frames} frames");
        Check(session.Frames > 0, "no VM frames were stepped");

        // The bedroom is the real SampleMapA runtime (SysDatabase row 1 -> artifact 1005):
        // pages get selected and automatic pages start.
        for (int i = 0; i < 240; i++) session.AdvanceFrame();

        var map = session.MapData;
        var runtime = session.MapRuntime;
        Check(map != null, "no map definition loaded after the teleport to SampleMapA");
        Check(runtime != null, "no map runtime after the teleport to SampleMapA");

        if (map != null && runtime != null)
        {
            GD.Print($"[PresentationTest] map id={map.Id} file={map.File} {map.Width}x{map.Height} " +
                     $"events={map.Events.Count} autorun={runtime.AutorunCount} parallel={runtime.ParallelCount} " +
                     $"leader={session.LeaderX},{session.LeaderY} instances={vm.Instances.Count}");

            foreach (var state in runtime.Events)
            {
                if (state.Starts == 0 && state.Active == null) continue;
                GD.Print($"[PresentationTest] event id={state.Def.Id} name='{state.Def.Name}' " +
                         $"x={state.Def.X} y={state.Def.Y} pages={state.Def.Pages.Count} " +
                         $"active={state.Active} starts={state.Starts} instance={state.InstanceId}");
            }

            Check(map.Id == 1005, $"map id {map.Id} != 1005");
            Check(map.Width > 0 && map.Height > 0, "map dimensions missing");
            Check(map.Events.Count > 0, "map has no events");

            int activePages = runtime.Events.Count(s => s.Active != null);
            Check(activePages > 0, "no map event page was selected");
            Check(runtime.Starts > 0, "no automatic map page ever started");
            Check(runtime.Events.Any(s => s.Starts > 0), "no event page start recorded");

            Check(session.LeaderX >= 0 && session.LeaderX < map.Width,
                $"leader x {session.LeaderX} outside 0..{map.Width - 1}");
            Check(session.LeaderY >= 0 && session.LeaderY < map.Height,
                $"leader y {session.LeaderY} outside 0..{map.Height - 1}");

            var mapEventIds = new HashSet<int>(map.Events.Select(e => e.Id));
            bool mapInstance = vm.Instances.Any(i => mapEventIds.Contains(i.EventId));
            Check(mapInstance, "no map event coroutine is running");
            Check(runtime.Events.Any(s => s.InstanceId != null), "map runtime tracks no live instance");
        }

        Check(vm.UnhandledOpcodeCount == 0, $"presentation chain saw {vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!vm.Faulted, "presentation chain faulted the VM (" + vm.FaultReason + ")");

        if (_failures.Count > 0)
        {
            foreach (string failure in _failures) GD.PrintErr($"[PresentationTest] FAIL: {failure}");
            GetTree().Quit(1);
            return;
        }

        GD.Print($"PRESENTATION_TEST_OK phase={session.Phase} frames={frames}");
        GetTree().Quit();
    }
}
