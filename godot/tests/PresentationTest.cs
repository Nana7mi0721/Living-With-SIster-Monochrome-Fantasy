using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

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

        RunBridgeRegressions(harness);

        if (_failures.Count > 0)
        {
            foreach (string failure in _failures) GD.PrintErr($"[PresentationTest] FAIL: {failure}");
            GetTree().Quit(1);
            return;
        }

        GD.Print($"PRESENTATION_TEST_OK phase={session.Phase} frames={frames}");
        GetTree().Quit();
    }

    private void RunBridgeRegressions(SessionHarness harness)
    {
        var bridge = new VmController();
        AddChild(bridge);
        bridge.Vm.Host = bridge;

        // 1. StopAll clears dialogue state, choice reply, and cached pictures.
        var dummyProg = Program.FromSlots(95001, "BridgeProbe", new List<Slot> { Slot.Command(172) });
        bridge.Vm.RegisterProgram(dummyProg);
        Instance? inst = bridge.Vm.CreateInstance(95001);
        Check(inst != null, "bridge probe instance must start");
        if (inst != null)
        {
            inst.Wait.WaitForChoice(0);
            bridge.ShowMessageForChoice("keep", -1);
            bridge.ShowChoices(new[] { "A", "B" });
            bridge.SelectChoice(1);
            bridge.Picture(new PictureCmd { Number = 7, Action = PictureAction.Draw, File = "a.png" });
            bridge.StopAll();
            Check(!bridge.IsWaitingForChoice(), "StopAll must clear IsWaitingForChoice");
            Check(bridge.ChoiceMade() == null, "StopAll must clear pending choice reply");
            Check(bridge.GetCurrentChoices().Length == 0, "StopAll must clear current choices");
            Check(!bridge.MessageKeptForChoice && bridge.MessageDismissed(), "StopAll must reset dialogue flags");
        }

        // 2. Killing a waiting coroutine after SelectChoice must clear stale reply;
        //    killing a message waiter resets MessageDismissed; 101->102 keeps MessageKeptForChoice.
        Instance? choiceInst = bridge.Vm.CreateInstance(95001);
        if (choiceInst != null)
        {
            choiceInst.Wait.WaitForChoice(0);
            bridge.ShowMessageForChoice("prompt", -1);
            bridge.ShowChoices(new[] { "Yes", "No" });
            Check(!bridge.IsWaitingForMessage() && bridge.MessageKeptForChoice,
                "IsWaitingForMessage must not clear MessageKeptForChoice while choice is open");
            bridge.SelectChoice(0);
            bridge.Vm.StopInstance(choiceInst.Id);
            Check(!bridge.IsWaitingForChoice(), "killed choice waiter must clear IsWaitingForChoice");
            Check(bridge.ChoiceMade() == null, "killed choice waiter must drop _pendingChoiceReply");
            Check(!bridge.MessageKeptForChoice, "killed choice waiter must clear MessageKeptForChoice");
        }
        Instance? msgInst = bridge.Vm.CreateInstance(95001);
        if (msgInst != null)
        {
            msgInst.Wait.WaitForMessage();
            bridge.ShowMessage("hello", -1);
            Check(bridge.IsWaitingForMessage() && !bridge.MessageDismissed(), "message wait setup");
            bridge.Vm.StopInstance(msgInst.Id);
            Check(!bridge.IsWaitingForMessage() && bridge.MessageDismissed(),
                "killed message waiter must reset MessageDismissed");
        }

        // 3. KeyCodeForPoll must ignore autoKeys on device poll masks like 771.
        Check(bridge.KeyCodeForPoll(771, 0, 0x01, 0) == 0,
            "device poll mask 771 must reject scripted autoKeys");
        Check(bridge.KeyCodeForPoll(771, 0, 0x01, -1) == -1,
            "device poll mask 771 must still report wheel notch");
        Check(bridge.KeyCodeForPoll(160, 0, 0x01, 0) == 0,
            "cancel-only mask 160 must reject OK autoKey");
        Check(bridge.KeyCodeForPoll(160, 0, 0x02, 0) == KeyCodes.CodeCancel,
            "cancel-only mask 160 must accept Cancel autoKey");

        // 4. Picture range delete with hi == int.MaxValue must terminate cleanly.
        bridge.Picture(new PictureCmd { Number = int.MaxValue - 1, Action = PictureAction.Draw, File = "a.png" });
        bridge.Picture(new PictureCmd { Number = int.MaxValue, Action = PictureAction.Draw, File = "b.png" });
        bridge.Picture(new PictureCmd
        {
            Number = int.MaxValue - 2,
            Action = PictureAction.Erase,
            HasSpan = true,
            Args = new[] { 0, 0, 0, int.MaxValue }
        });

        // 5. Left-click TTL release & single-observation edge signal & ResetPointer clearing wheel.
        int observedCount = 0;
        bridge.PointerPressObserved += () => observedCount++;
        bridge.SetPointerPressed(true);
        bridge.BeginFrameInput();
        Check(bridge.EngineStateBridge(7) == 2 && bridge.EngineStateBridge(7) == 2,
            "same-frame EngineState(7) polls must both see edge 2");
        Check(observedCount == 1, $"PointerPressObserved must emit once per press edge (got {observedCount})");

        bridge.ResetPointer();
        bridge.SetPointerPressed(true);
        bridge.BeginFrameInput();
        bridge.SetPointerPressed(false);
        bridge.BeginFrameInput();
        Check(bridge.EngineStateBridge(7) == 0,
            "released left button must not report edge 2 via TTL");

        bridge.SetFrameWheel(2);
        bridge.ResetPointer();
        Check(bridge.EngineStateBridge(10) == 0, "ResetPointer must clear wheel delta");

        // 6. FileExists rejects traversal, preserves subdirs under SaveDirectory, and confines to PackageRoot.
        string tempRoot = Path.Combine(Path.GetTempPath(), "lwsmf-fileexists-" + Guid.NewGuid().ToString("N"));
        string subDir = Path.Combine(tempRoot, "sub");
        Directory.CreateDirectory(subDir);
        File.WriteAllBytes(Path.Combine(subDir, "thumb.jpg"), new byte[] { 1 });
        bridge.SaveDirectory = tempRoot;
        Check(!bridge.FileExists("../game_data/programs.json"), "FileExists must reject leading .. traversal");
        Check(!bridge.FileExists("save/../../game_data/programs.json"), "FileExists must reject nested .. traversal");
        Check(!bridge.FileExists("save/thumb.jpg"), "FileExists must not flatten subdirectories");
        Check(bridge.FileExists("save/sub/thumb.jpg"), "FileExists must preserve subdirectory structure");
        Check(bridge.FileExists("game_data/programs.json"), "FileExists must resolve files confined to PackageRoot");
        try { Directory.Delete(tempRoot, true); } catch (IOException) { }

        // 7. LoadPrograms JSON error handling sets LastError cleanly instead of throwing.
        string badJsonPath = Path.Combine(Path.GetTempPath(), "lwsmf-bad-prog-" + Guid.NewGuid().ToString("N") + ".json");
        File.WriteAllText(badJsonPath, "{ not valid json");
        Check(!bridge.LoadPrograms(badJsonPath) && bridge.LastError.Length > 0,
            "LoadPrograms must catch JSON errors and record LastError");
        try { File.Delete(badJsonPath); } catch (IOException) { }

        // 8. MediaPakReader failure cleanup and Dispose.
        string badPakPath = Path.Combine(Path.GetTempPath(), "lwsmf-bad-pak-" + Guid.NewGuid().ToString("N") + ".pak");
        File.WriteAllBytes(badPakPath, new byte[] { 1, 2, 3, 4 });
        var pak = new MediaPakReader();
        Check(!pak.Open(badPakPath) && !pak.IsOpen && pak.Count == 0 && pak.LastError.Length > 0,
            "MediaPakReader.Open failure must leave closed empty state with LastError");
        pak.Dispose();
        Check(!pak.IsOpen && pak.Count == 0, "MediaPakReader.Dispose must close cleanly");
        try { File.Delete(badPakPath); } catch (IOException) { }

        // 9. GameSessionController QuickSavePath honors custom savePath and Rebuild clears LastError.
        var uiStub = new Godot.Control();
        AddChild(uiStub);
        var ctrl = new GameSessionController();
        AddChild(ctrl);
        string customSaveDir = Path.Combine(Path.GetTempPath(), "lwsmf-custom-save-" + Guid.NewGuid().ToString("N"));
        string customSavePath = Path.Combine(customSaveDir, "custom_01.json");
        ctrl.Setup(bridge, uiStub, customSavePath);
        Check(Path.GetFullPath(ctrl.QuickSavePath()) == Path.GetFullPath(Path.Combine(customSaveDir, "quicksave.json")),
            "QuickSavePath must derive from custom _savePath");
        Check(ctrl.Rebuild() && ctrl.LastError.Length == 0, "Rebuild must clear LastError");
        try { Directory.Delete(customSaveDir, true); } catch (IOException) { }
    }
}
