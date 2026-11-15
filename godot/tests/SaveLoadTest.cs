using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Walks the core title/new-game chain into map 1, writes save slot 01, mutates the live
/// state, then reloads and checks that variables, leader and map came back. Also
/// checkpoints an open message and an open choice: reload must reopen the window
/// and resume the same instance at the same pc/wait instead of replaying opcode 101/102.
/// </summary>
public partial class SaveLoadTest : Node
{
    private static int _checks;
    private static int _failures;

    private static void Check(bool ok, string what)
    {
        _checks++;
        if (ok) return;
        _failures++;
        GD.PrintErr("[SaveLoadTest] FAIL: " + what);
    }

    public override void _Ready()
    {
        // Never touch the player's real slot: the whole run works inside a private dir.
        string saveDir = Path.Combine(Path.GetTempPath(), "lwsmf-save-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(saveDir);
        string savePath = Path.Combine(saveDir, "save_01.json");
        string dataPath = Path.Combine(
            ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\'), "..", "game_data", "programs.json");

        SessionHarness harness;
        try
        {
            harness = new SessionHarness(this, dataPath, savePath);
        }
        catch (Exception ex)
        {
            Check(false, "core session setup failed: " + ex.Message);
            GD.PrintErr($"SAVE_LOAD_TEST_FAILED {_failures}/{_checks}");
            GetTree().Quit();
            return;
        }

        GameSession session = harness.Session;
        VmController vm = harness.Vm;
        TestSessionView view = harness.View;

        Check(harness.RunToBedroom(4000), "title chain must reach the map (" + session.Phase + ")");
        if (session.Phase != "bedroom")
        {
            GD.PrintErr($"SAVE_LOAD_TEST_FAILED {_failures}/{_checks}");
            GetTree().Quit();
            return;
        }

        // A fresh slot so the test never depends on earlier runs.
        if (File.Exists(savePath)) File.Delete(savePath);
        Check(!session.HasSave, "slot 01 must start empty");

        var vars = vm.Vm.Vars;
        vars.Set(500, 7);
        vars.SetStr(3, "save-test");
        session.MoveLeader(-1, 0, 4);
        int savedX = session.LeaderX;
        int savedY = session.LeaderY;
        var commonSelf = new SelfVars();
        commonSelf.Set(600, 42);
        commonSelf.SetStr(605, "common");
        vm.Vm.CommonSelf[99] = commonSelf;

        Check(session.SaveGame(), "SaveGame must succeed on the map");
        Check(session.HasSave, "slot 01 must exist after SaveGame");
        Check(new FileInfo(savePath).Length > 0, "slot 01 must not be empty");
        Check(!File.Exists(savePath + ".tmp"), "atomic save must not leave a temp file");

        // Diverge from the saved state, then load it back.
        vars.Set(500, 99);
        vars.SetStr(3, "mutated");
        vm.Vm.CommonSelf[99]?.Set(600, 0);
        session.MoveLeader(3, 2, 2);
        Check(session.LoadGame(), "LoadGame must succeed on slot 01");
        Check(session.Phase == "bedroom", "load must land back on a map");
        Check(vars.Get(500) == 7, "number variable must come back as 7 (got " + vars.Get(500) + ")");
        Check(vars.GetStr(3) == "save-test", "string variable must come back");
        Check(session.LeaderX == savedX && session.LeaderY == savedY,
            $"leader must come back at {savedX},{savedY} (got {session.LeaderX},{session.LeaderY})");
        Check(session.MapRuntime != null && session.MapData != null && session.MapData.Id == 1, "map 1 must be live after load");
        Check(vm.Vm.Instances.Count > 0, "automatic pages should restart after load");
        Check(vm.Vm.CommonSelf.TryGetValue(99, out var restoredCommon) && restoredCommon.Get(600) == 42,
            "common-event self bank must survive save/load");
        Check(vm.Vm.CommonSelf.TryGetValue(99, out var restoredCommonStr) && restoredCommonStr.GetStr(605) == "common",
            "common-event self strings must survive save/load");

        // Picture slots are part of the slot: the layer is rebuilt from the file, not from
        // whatever happened to be on screen when the player pressed load.
        view.DrawPicture(new PictureCmd
        {
            Number = 3, Action = PictureAction.Draw, File = "Picture/face01.png",
            Args = new[] { 0, 0, 0, 0, 200, 300, 400, 50 }, Anchor = 3
        });
        view.DrawPicture(new PictureCmd
        {
            Number = 12000, Action = PictureAction.Draw, File = "Picture/title01.jpg",
            Args = new[] { 0, 0, 0, 0, 255, 0, 0, 100 }
        });
        Check(view.PictureSnapshots().Count == 2, "two picture slots must be live before save");
        Check(session.SaveGame(), "SaveGame must succeed with pictures on screen");
        view.ClearPictures();
        Check(view.PictureSnapshots().Count == 0, "ClearPictures must drop the snapshot slots");
        Check(session.LoadGame(), "LoadGame must succeed with saved pictures");
        var restoredPics = view.PictureSnapshots();
        Check(restoredPics.Count == 2, $"two picture slots must come back (got {restoredPics.Count})");
        var restored3 = restoredPics.FirstOrDefault(p => p.Number == 3);
        Check(restored3 != null && restored3.File == "Picture/face01.png" && restored3.Anchor == 3
              && restored3.Args.Count == 8 && restored3.Args[7] == 50,
            "restored picture must keep file, anchor and args");
        Check(restoredPics.Count(p => p.Number == 12000) == 1, "backdrop slot must be restored once");
        view.ClearPictures();

        // Arm the map's conversation page and save in the middle of its message: the load
        // must resume the same coroutine instead of replaying the opcode that opened it.
        vars.Set(30, 1);
        int messageGuard = 0;
        while (!vm.IsWaitingForMessage() && messageGuard++ < 2000) session.AdvanceFrame();
        Check(vm.IsWaitingForMessage(), "event 4 must open a message");
        Check(view.HasMessage, "view must show the message window");
        string messageText = view.MessageText;
        MapEventState? event4 = session.MapRuntime?.Events.FirstOrDefault(e => e.Def.Id == 4);
        int liveId = event4?.InstanceId ?? -1;
        Instance? liveMessage = liveId > 0 ? vm.Vm.FindInstance(liveId) : null;
        Check(liveMessage != null, "event 4 must have a live instance while waiting");
        int savedPc = liveMessage?.Pc ?? -1;
        int savedWait = (int)(liveMessage?.Wait.Kind ?? WaitStateKind.None);
        Check(session.SaveGame(), "SaveGame must succeed while a message is open");

        session.DismissMessage();
        for (int i = 0; i < 8; i++) session.AdvanceFrame();
        Check(session.LoadGame(), "LoadGame must succeed from a message checkpoint");
        Check(view.HasMessage, "message must be visible again after load");
        Check(view.MessageText == messageText, "message text must be identical after load");
        Instance? resumed = vm.Vm.FindInstance(liveId);
        Check(resumed != null, "the same instance id must be live after load");
        Check(resumed?.Pc == savedPc, $"pc must resume at {savedPc} (got {resumed?.Pc})");
        Check((int)(resumed?.Wait.Kind ?? WaitStateKind.None) == savedWait, "wait state must resume unchanged");
        Check(session.MapRuntime?.Events.FirstOrDefault(e => e.Def.Id == 4)?.InstanceId == liveId,
            "map event 4 must point at the resumed instance");

        // Same checkpoint rule for a choice: reopen the options instead of rerunning the
        // opcode 102, and keep the waiting instance at its pc.
        session.DismissMessage();
        int choiceGuard = 0;
        while (!vm.IsWaitingForChoice() && choiceGuard++ < 2000) session.AdvanceFrame();
        Check(vm.IsWaitingForChoice(), "event 4 must open a choice");
        Check(view.HasChoices, "view must show the choice window");
        string choiceText = string.Join("|", vm.GetCurrentChoices());
        Instance? liveChoice = vm.Vm.FindInstance(liveId);
        int choicePc = liveChoice?.Pc ?? -1;
        int choiceWait = (int)(liveChoice?.Wait.Kind ?? WaitStateKind.None);
        Check(session.SaveGame(), "SaveGame must succeed while a choice is open");

        vm.SelectChoice(0);
        for (int i = 0; i < 4; i++) session.AdvanceFrame();
        Check(session.LoadGame(), "LoadGame must succeed from a choice checkpoint");
        Check(view.HasChoices, "choice window must be visible again after load");
        Check(string.Join("|", vm.GetCurrentChoices()) == choiceText, "choice options must be identical after load");
        Instance? resumedChoice = vm.Vm.FindInstance(liveId);
        Check(resumedChoice?.Pc == choicePc, $"choice pc must resume at {choicePc} (got {resumedChoice?.Pc})");
        Check((int)(resumedChoice?.Wait.Kind ?? WaitStateKind.None) == choiceWait, "choice wait state must resume unchanged");

        // Title menu row 1 (load) must restore the same slot from a fresh boot.
        SessionHarness harness2 = new(this, dataPath, savePath);
        harness2.Session.AutoPickChoice = false;
        GameSession session2 = harness2.Session;
        VmController vm2 = harness2.Vm;
        int titleGuard = 0;
        while (session2.Phase == "title" && !session2.HasChoices && titleGuard++ < 3000)
        {
            session2.AdvanceFrame();
        }
        Check(session2.HasChoices, "title menu must open");
        if (session2.HasChoices) session2.PickChoice(1);
        int loadGuard = 0;
        while (session2.Phase != "bedroom" && session2.Phase != "failed" && loadGuard++ < 2000)
        {
            session2.AdvanceFrame();
        }
        Check(session2.Phase == "bedroom", "title load must reach the map (" + session2.Phase + ")");
        Check(vm2.Vm.Vars.Get(500) == 7, "title load must restore variables");
        Check(session2.LeaderX == savedX && session2.LeaderY == savedY, "title load must restore the leader");

        // A teleport is queued and applied at the frame boundary, never during the VM step
        // (M1-P1-06): the map switch must not free instances the frame is still walking.
        vm2.EmitSignal(VmController.SignalName.TeleportRequested, 0, 1008, 5, 7, 0);
        Check(session2.MapData?.Id == 1, "a teleport must not apply inside the VM frame");
        session2.AdvanceFrame();
        Check(session2.MapData?.Id == 1008, $"a teleport must switch maps at the frame boundary (got {session2.MapData?.Id})");
        Check(session2.LeaderX == 5 && session2.LeaderY == 7,
            $"a teleport must place the leader at 5,7 (got {session2.LeaderX},{session2.LeaderY})");
        session2.AdvanceFrame();
        Check(session2.MapData?.Id == 1008, "an applied teleport must not repeat");

        // Only the leader (target -1 or 0) may switch maps; a non-leader target is ignored.
        vm2.EmitSignal(VmController.SignalName.TeleportRequested, 5, 1, 1, 1, 0);
        session2.AdvanceFrame();
        Check(session2.MapData?.Id == 1008, "a non-leader teleport must not switch maps");
        Check(session2.LeaderX == 5 && session2.LeaderY == 7, "a non-leader teleport must not move the leader");

        // One programs.json parse must be shared by every loader (M1-P2-02).
        Check(ProgramLoader.CachedArtifactCount >= 1, "programs.json must be cached after the title chain");
        int cachedBefore = ProgramLoader.CachedArtifactCount;
        ProgramLoader.LoadMapEvents(dataPath, "Map001.mps", 1);
        Check(ProgramLoader.CachedArtifactCount == cachedBefore, "a repeated load must reuse the cached document");
        ProgramLoader.ClearCache();
        Check(ProgramLoader.CachedArtifactCount == 0, "ClearCache must drop parsed artifacts");

        // Sparse SelfVars: trailing zero slots and empty strings must not be written.
        var packed = EventSelfVars.Pack(9, new[] { 3, 4, 0, 0 }, new[] { "x", "", "" });
        Check(packed.NumMap?.Count == 2 && packed.StrMap?.Count == 1,
            $"sparse self vars must keep only used slots (nums={packed.NumMap?.Count} str={packed.StrMap?.Count})");
        var expanded = new SelfVars();
        expanded.Restore(packed);
        Check(expanded.Get(0) == 3 && expanded.Get(1) == 4 && expanded.GetStr(0) == "x",
            "sparse self vars must expand back into the dense bank");

        // The slot must stay small enough for a single-screen save; full 1000-slot
        // SelfVars snapshots used to push it past 700 KB.
        long saveBytes = new FileInfo(savePath).Length;
        Check(saveBytes < 50 * 1024, $"save slot must stay under 50 KB (got {saveBytes} bytes)");

        // Schema guard: a future version must be rejected rather than half-parsed.
        string savedJson = File.ReadAllText(savePath);
        Check(SaveCodec.FromJson(savedJson.Replace("\"schema_version\": 1", "\"schema_version\": 99")) == null,
            "schema_version 99 must be rejected");
        Check(SaveCodec.FromJson(savedJson.Replace("\"schema_version\": 1", "\"schema_version\": 0")) == null,
            "schema_version 0 must be rejected");

        // Corrupt main + good .bak: Read must fall back, and the backup must survive
        // a rewrite attempt from the corrupt file.
        var beforeBackup = SaveCodec.Read(savePath);
        Check(beforeBackup != null, "slot 01 must be readable before backup test");
        if (beforeBackup != null) SaveCodec.Write(savePath, beforeBackup);
        Check(File.Exists(savePath + ".bak"), "second save must rotate a .bak");
        if (beforeBackup != null)
        {
            File.WriteAllText(savePath, "{ broken");
            var recovered = SaveCodec.Read(savePath);
            Check(recovered != null, "corrupt main must recover from .bak");
            Check(recovered?.MapId == 1 && recovered?.Vars.Count > 0, "backup must restore the saved map and vars");
            if (recovered != null) SaveCodec.Write(savePath, recovered);
        }

        // A corrupt main must never clobber the last good backup.
        var good = SaveCodec.Read(savePath);
        Check(good != null, "slot must be readable before bak-protection test");
        if (good != null && File.Exists(savePath + ".bak"))
        {
            string bakBefore = File.ReadAllText(savePath + ".bak");
            File.WriteAllText(savePath, "{ not json");
            SaveCodec.Write(savePath, good);
            Check(File.ReadAllText(savePath + ".bak") == bakBefore,
                "a corrupt main must not overwrite the good .bak during rotation");
            Check(!File.Exists(savePath + ".bak.tmp"), "backup rotation must not leave a temp file");
        }
        Check(SaveCodec.Read(savePath) != null, "slot must be writable again after corruption");
        File.Delete(savePath);
        Check(session.HasSave, "a lone .bak must still count as an existing slot");
        Check(SaveCodec.Read(savePath) != null, "a lone .bak must be loadable");
        var restoredState = SaveCodec.Read(savePath);
        if (restoredState != null) SaveCodec.Write(savePath, restoredState);

        // Codec guards for hand-edited JSON: out-of-range sparse keys, duplicate
        // instance ids and unknown wait kinds must be rejected, not expanded.
        var probe = new SaveState { MapId = 1, MapFile = "Map001.mps", LeaderX = 2, LeaderY = 3 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) != null, "minimal state must validate");
        probe.MapFile = "../Map001.mps";
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "a map file escaping the asset root must be rejected");
        probe.MapFile = "Map001.mps";
        probe.SelfVars.Add(new EventSelfVars { Id = 4, NumMap = new Dictionary<int, int> { [-1] = 5 } });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "negative self-var key must be rejected");
        probe.SelfVars[0] = new EventSelfVars { Id = 4, NumMap = new Dictionary<int, int> { [1000] = 5 } };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "self-var key 1000 must be rejected");
        probe.SelfVars[0] = new EventSelfVars { Id = 4, NumMap = new Dictionary<int, int> { [999] = 5 } };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) != null, "self-var key 999 must be accepted");
        probe.SelfVars.Add(new EventSelfVars { Id = 4 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "duplicate self-var ids must be rejected");

        // Picture slots follow the same rule: nullable JSON stays loadable, duplicates and
        // unknown actions reject the whole slot.
        var picProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        picProbe.Pictures.Add(new SavedPicture { Number = 1, Action = 0, File = "a.png", Args = new() { 1 } });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) != null, "a valid picture slot must serialize");
        picProbe.Pictures.Add(new SavedPicture { Number = 1, Action = 0 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) == null, "duplicate picture numbers must be rejected");
        picProbe.Pictures.Clear();
        picProbe.Pictures.Add(new SavedPicture { Number = 1, Action = 9 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) == null, "unknown picture action must be rejected");
        picProbe.Pictures.Clear();
        picProbe.Pictures.Add(new SavedPicture { Number = 2, Action = 0, Args = new() { 1, 2 } });
        picProbe.Pictures[0].File = null!;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) != null, "null picture file must normalize to empty");
        picProbe.Pictures.Clear();
        picProbe.Pictures.Add(new SavedPicture { Number = 3, Action = 0, File = "../escape.png" });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) == null, "a picture path escaping the asset root must be rejected");
        picProbe.Pictures[0].File = "Picture/..\\evil.png";
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) == null, "a nested .. segment must be rejected");
        picProbe.Pictures[0].File = "C:/windows/evil.png";
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) == null, "an absolute drive path must be rejected");
        picProbe.Pictures[0].File = "Picture/face01.png";
        Check(SaveCodec.FromJson(SaveCodec.ToJson(picProbe)) != null, "a nested asset name must still load");

        // W4: audio tracks are part of the slot; channel/file/volume/loop/tempo are checked.
        var audioProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        audioProbe.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/x.ogg", Volume = 70, Loop = true, LoopPoint = 120, Tempo = 100 });
        audioProbe.Audio.Add(new SavedAudio { Channel = 2, File = "Sound/x.ogg", Volume = 100 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) != null, "a valid audio track set must serialize");
        audioProbe.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/y.ogg" });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "duplicate audio channels must be rejected");
        audioProbe.Audio.RemoveAt(2);
        audioProbe.Audio[0].Channel = 3;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "an audio channel above BGS must be rejected");
        audioProbe.Audio[0].Channel = 0;
        audioProbe.Audio[0].Volume = 250;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "an out-of-range audio volume must be rejected");
        audioProbe.Audio[0].Volume = 70;
        audioProbe.Audio[0].Tempo = 500;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "an out-of-range tempo must be rejected");
        audioProbe.Audio[0].Tempo = 100;
        audioProbe.Audio[0].LoopPoint = 86_400 * 60 + 1;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "an unbounded audio loop point must be rejected");
        audioProbe.Audio[0].LoopPoint = 0;
        audioProbe.Audio[0].File = "../escape.ogg";
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "an audio path escaping the asset root must be rejected");
        audioProbe.Audio[0].File = "BGM/x.ogg";
        audioProbe.Audio[0].PositionMs = -1;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(audioProbe)) == null, "a negative audio position must be rejected");
        probe.SelfVars.RemoveAt(1);
        probe.Instances.Add(new SavedInstance { Id = 1, EventId = 1, Kind = "ce", Pc = 0 });
        probe.Instances.Add(new SavedInstance { Id = 1, EventId = 1, Kind = "ce", Pc = 0 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "duplicate instance ids must be rejected");
        probe.Instances.Clear();
        probe.Instances.Add(new SavedInstance { Id = 1, EventId = 1, Kind = "ce", Pc = 0, WaitKind = 9 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "unknown wait kind must be rejected");
        probe.Instances.Clear();
        probe.Instances.Add(new SavedInstance { Id = 2, ParentId = 9, EventId = 1, Kind = "ce", Pc = 0 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(probe)) == null, "orphan parent id must be rejected");

        // Strict gate: the whole title -> new game -> load chain must not meet an
        // unimplemented opcode; either it threw already or this counter caught it.
        Check(!vm.Vm.Faulted, "save-load chain must not fault the VM (" + vm.Vm.FaultReason + ")");
        Check(vm.Vm.UnhandledOpcodeCount == 0,
            $"save-load chain saw {vm.Vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!vm2.Vm.Faulted, "title-load chain must not fault the VM (" + vm2.Vm.FaultReason + ")");
        Check(vm2.Vm.UnhandledOpcodeCount == 0,
            $"title-load chain saw {vm2.Vm.UnhandledOpcodeCount} unhandled opcodes");

        try { Directory.Delete(saveDir, true); }
        catch (IOException) { }

        GD.Print($"[SaveLoadTest] phase={session.Phase} map={session.MapData?.Id} leader={session.LeaderX},{session.LeaderY} vars={vars.Get(500)} str='{vars.GetStr(3)}' titleLoadPhase={session2.Phase}");
        if (_failures == 0)
        {
            GD.Print($"SAVE_LOAD_TEST_OK {_checks}/{_checks}");
        }
        else
        {
            GD.PrintErr($"SAVE_LOAD_TEST_FAILED {_failures}/{_checks}");
        }
        GetTree().Quit();
    }
}
