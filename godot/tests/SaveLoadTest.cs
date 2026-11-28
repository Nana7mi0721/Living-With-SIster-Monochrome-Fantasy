using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Walks the core title/new-game chain into SampleMapA (artifact 1005), writes save slot 01, mutates the live
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
            GetTree().Quit(1);
            return;
        }

        GameSession session = harness.Session;
        VmController vm = harness.Vm;
        TestSessionView view = harness.View;

        Check(harness.RunToBedroom(4000), "title chain must reach the map (" + session.Phase + ")");
        if (session.Phase != "bedroom")
        {
            GD.PrintErr($"SAVE_LOAD_TEST_FAILED {_failures}/{_checks}");
            GetTree().Quit(1);
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
        Check(session.MapRuntime != null && session.MapData != null && session.MapData.Id == 1005, "SampleMapA (1005) must be live after load");
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
        Check(view.PictureSnapshots().Count == 2, "scene and save-UI picture slots must be live before save");
        Check(session.SaveGame(), "SaveGame must succeed with pictures on screen");
        view.ClearPictures();
        Check(view.PictureSnapshots().Count == 0, "ClearPictures must drop the snapshot slots");
        Check(session.LoadGame(), "LoadGame must succeed with saved pictures");
        var restoredPics = view.PictureSnapshots();
        Check(restoredPics.Count == 1, $"only scene pictures must come back (got {restoredPics.Count})");
        var restored3 = restoredPics.FirstOrDefault(p => p.Number == 3);
        Check(restored3 != null && restored3.File == "Picture/face01.png" && restored3.Anchor == 3
              && restored3.Args.Count == 8 && restored3.Args[7] == 50,
            "restored picture must keep file, anchor and args");
        Check(restoredPics.All(p => p.Number < 12000 || p.Number > 12599),
            "save-UI picture slots must never be persisted");
        view.ClearPictures();

        // Menu coroutines are session UI, not gameplay. Their descendants must not leak
        // into a load, while an unrelated scene coroutine still does.
        var saveRoot = Program.FromSlots(14, "SaveMenu", new List<Slot> { Slot.Command(180, [10000]) });
        var saveChild = Program.FromSlots(131, "SaveSlot", new List<Slot> { Slot.Command(180, [10000]) });
        var saveCaller = Program.FromSlots(90013, "SaveCaller", new List<Slot> { Slot.Command(172) });
        vm.Vm.RegisterProgram(saveRoot);
        vm.Vm.RegisterProgram(saveChild);
        vm.Vm.RegisterProgram(saveCaller);
        Instance saveUi = vm.Vm.CreateInstance(14)!;
        Instance saveUiChild = vm.Vm.CreateInstance(131, saveUi.Id)!;
        Instance saveCallerInst = vm.Vm.CreateInstance(90013)!;
        saveCallerInst.Wait.WaitForChild(saveUi.Id);
        SaveState? filtered = session.BuildSaveState();
        Check(filtered != null, "save state must build with menu UI live");
        Check(filtered != null && filtered.Instances.All(i => i.EventId is not (14 or 131)),
            "CE14/CE131 and their descendants must be filtered from saved instances");
        SavedInstance? savedCaller = filtered?.Instances.FirstOrDefault(i => i.Id == saveCallerInst.Id);
        Check(savedCaller != null, "a non-save caller must survive the filter");
        Check(savedCaller?.WaitKind == (int)WaitStateKind.None && savedCaller.WaitChild == 0
              && savedCaller.WaitFrames == 0 && savedCaller.WaitBlock == 0,
            "a caller waiting on filtered save UI must not keep a dangling child wait");
        vm.Vm.StopInstance(saveCallerInst.Id);
        vm.Vm.StopInstance(saveUiChild.Id);
        vm.Vm.StopInstance(saveUi.Id);

        // Arm the map's conversation page and save in the middle of its message: the load
        // must resume the same coroutine instead of replaying the opcode that opened it.
        // SampleMapA event 4 page 4 (parallel): 63=1, 62=0, 464=0, 60=0 opens its
        // message; the same page later offers the two-option sleep choice (pc 102).
        vars.Set(63, 1);
        vars.Set(62, 0);
        vars.Set(464, 0);
        vars.Set(60, 0);
        int messageGuard = 0;
        while (!vm.IsWaitingForMessage() && messageGuard++ < 2000) session.AdvanceFrame();
        Check(vm.IsWaitingForMessage(), "event 4 must open a message");
        Check(view.HasMessage, "view must show the message window");
        string messageText = view.MessageText;
        MapEventState? event4 = session.MapRuntime?.Events.FirstOrDefault(e => e.Def.Id == 4 && e.InstanceId > 0);
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
        Check(session.MapRuntime?.Events.FirstOrDefault(e => e.Def.Id == 4 && e.InstanceId == liveId) != null,
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
        SaveState? choiceSlot = SaveCodec.Read(session.SavePath);
        Check(choiceSlot?.Dialogue != null, "choice checkpoint must save its dialogue block");
        if (choiceSlot?.Dialogue != null)
        {
            choiceSlot.Dialogue.AllowCancel = false;
            SaveCodec.Write(session.SavePath, choiceSlot);
        }

        vm.SelectChoice(0);
        for (int i = 0; i < 4; i++) session.AdvanceFrame();
        Check(session.LoadGame(), "LoadGame must succeed from a choice checkpoint");
        Check(view.HasChoices, "choice window must be visible again after load");
        Check(!session.ChoiceAllowsCancel, "a non-cancellable choice must stay locked after load");
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
        Check(session2.MapData?.Id == 1005, "a teleport must not apply inside the VM frame");
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

        // Schema guard: future versions are rejected; v1 and pre-versioned (< 1) slots migrate to current.
        string savedJson = File.ReadAllText(savePath);
        string currentSchemaToken = $"\"schema_version\": {SaveState.SchemaVersionNumber}";
        Check(SaveCodec.FromJson(savedJson.Replace(currentSchemaToken, "\"schema_version\": 99")) == null,
            "schema_version 99 must be rejected");
        SaveState? migratedV1 = SaveCodec.FromJson(savedJson.Replace(currentSchemaToken, "\"schema_version\": 1"));
        Check(migratedV1 != null && migratedV1.SchemaVersion == SaveState.SchemaVersionNumber && migratedV1.DbOverlays != null,
            "schema_version 1 must migrate to current schema version with DbOverlays initialized");
        SaveState? migratedV0 = SaveCodec.FromJson(savedJson.Replace(currentSchemaToken, "\"schema_version\": 0"));
        Check(migratedV0 != null && migratedV0.SchemaVersion == SaveState.SchemaVersionNumber,
            "schema_version 0 must migrate to current schema version");
        SaveState? migratedNeg = SaveCodec.FromBytes(System.Text.Encoding.UTF8.GetBytes(
            savedJson.Replace(currentSchemaToken, "\"schema_version\": -1")));
        Check(migratedNeg != null && migratedNeg.SchemaVersion == SaveState.SchemaVersionNumber,
            "schema_version < 1 via FromBytes must migrate to current schema version");

        // Corrupt main + good .bak: ReadWithSource must fall back, and the backup must
        // survive a rewrite attempt from the corrupt file.
        var beforeBackup = SaveCodec.Read(savePath);
        Check(beforeBackup != null, "slot 01 must be readable before backup test");
        if (beforeBackup != null) SaveCodec.Write(savePath, beforeBackup);
        Check(File.Exists(savePath + ".bak"), "second save must rotate a .bak");
        if (beforeBackup != null)
        {
            File.WriteAllText(savePath, "{ broken");
            var readResult = SaveCodec.ReadWithSource(savePath);
            Check(readResult.State != null, "corrupt main must recover from .bak");
            Check(readResult.Source == SaveSlotSource.Backup, "read must report the backup source");
            Check(readResult.State?.MapId == 1005 && readResult.State?.Vars.Count > 0,
                "backup must restore the saved map and vars");
            // The live session must expose the source and remain playable after the fallback.
            Check(session.LoadGame(), "LoadGame must succeed from the .bak fallback");
            Check(session.LoadedFrom == SaveSlotSource.Backup && session.LoadedFromBackup,
                "session must report the backup slot");
            Check(session.Phase == "bedroom", "backup load must leave a playable bedroom");
            Check(view.Status.Contains("from backup"), "status must name the backup slot");

            var recovered = SaveCodec.Read(savePath);
            Check(recovered != null, "Read compatibility API must recover from .bak");
            if (recovered != null) SaveCodec.Write(savePath, recovered);

            // Both files corrupt: a failed load must not disturb the live bedroom.
            string phaseBefore = session.Phase;
            int mapBefore = session.MapData?.Id ?? -1;
            string leaderBefore = $"{session.LeaderX},{session.LeaderY}";
            File.WriteAllText(savePath, "{ broken");
            File.WriteAllText(savePath + ".bak", "{ broken");
            Check(!session.LoadGame(), "LoadGame must fail when both slots are corrupt");
            Check(session.Phase == phaseBefore && (session.MapData?.Id ?? -1) == mapBefore
                && $"{session.LeaderX},{session.LeaderY}" == leaderBefore,
                "a failed load must keep the live bedroom");
            // Restore both slots for the rotation-protection checks below.
            SaveCodec.Write(savePath, recovered!);
            SaveCodec.Write(savePath, recovered!);
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
        var loneResult = SaveCodec.ReadWithSource(savePath);
        Check(loneResult.State != null && loneResult.Source == SaveSlotSource.Backup,
            "a missing main must load from the backup source");
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

        // D1: opcodes 220/221/222 reach the session that owns the slot files. Before this
        // the host answered null, so the retail title's system-data probes all read 0.
        string sysPath = Path.Combine(saveDir, "System.json");
        if (File.Exists(sysPath)) File.Delete(sysPath);
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 221,
            File = new SaveTarget("Save/System.sav"),
            Address = 2_000_300,
        }) == 0, "221 on a missing tier must answer 0");
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 222,
            File = new SaveTarget("Save/System.sav"),
            Address = 2_000_300,
            Value = 7,
        }) == null, "222 must write without a VM result");
        Check(File.Exists(sysPath), "a path operand must map beside slot 01 (" + sysPath + ")");
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 221,
            File = new SaveTarget("Save/System.sav"),
            Address = 2_000_300,
        }) == 7, "221 must read back what 222 wrote");
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 221,
            File = new SaveTarget("save/system.sav"),
            Address = 2_000_300,
        }) == 7, "221 must resolve save/system.sav case-insensitively to System.json");

        // 222/221 also persists StrVar and SysVar above index 12.
        session.HandleSaveLoad(new SaveOp
        {
            Opcode = 222,
            File = new SaveTarget("Save/System.sav"),
            Address = Magic.StrBase + 5,
            StringValue = "persist-str",
            IsString = true,
        });
        var readStrOp = new SaveOp
        {
            Opcode = 221,
            File = new SaveTarget("Save/System.sav"),
            Address = Magic.StrBase + 5,
            Destination = Magic.StrBase + 5,
        };
        session.HandleSaveLoad(readStrOp);
        Check(readStrOp.LoadedString == "persist-str",
            $"221/222 StrVar roundtrip failed (got '{readStrOp.LoadedString}')");
        session.HandleSaveLoad(new SaveOp
        {
            Opcode = 222,
            File = new SaveTarget("Save/System.sav"),
            Address = Magic.SysBase + 100,
            Value = 80,
        });
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 221,
            File = new SaveTarget("Save/System.sav"),
            Address = Magic.SysBase + 100,
        }) == 80, "221/222 must support SysVar > 12 (Sys[100])");

        // 220 with a path operand is the retail system-data file: banks only, no map and
        // no coroutine state, and the load is deferred to the frame boundary.
        vars.Set(300, 41);
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 220,
            Mode = 0,
            File = new SaveTarget("Save/System.sav"),
        }) == null, "220 save of system data must answer null");
        vars.Set(300, 0);
        Check(session.HandleSaveLoad(new SaveOp
        {
            Opcode = 220,
            Mode = 1,
            File = new SaveTarget("Save/System.sav"),
        }) == null, "220 load of system data must defer to the frame boundary");
        session.AdvanceFrame();
        Check(vars.Get(300) == 41, "220 system load must restore the normal bank (got " + vars.Get(300) + ")");
        Check(session.Phase == "bedroom", "220 system load must not leave the live map (" + session.Phase + ")");

        // 221 with flag == 1 indirect address + CData (1_100_000_000) numeric and string cells
        // (CE94 pc 335/345/348 save-card party & hero icon lookup). Use slot 2 so slot 1 stays untouched.
        var op221Prog = Program.FromSlots(940, "Op221CData", new List<Slot>
        {
            Slot.Command(221, [1_600_011, 2, 1_600_062, 1]),
            Slot.Command(221, [1_600_007, 2, 1_600_064, 1]),
            Slot.Command(172)
        });
        vm.Vm.RegisterProgram(op221Prog);
        Instance? op221Inst = vm.Vm.CreateInstance(940);
        Check(op221Inst != null, "221 CData test instance must start");
        if (op221Inst != null)
        {
            vm.Database(new DbOp
            {
                Container = DbContainer.CData,
                Direction = DbDirection.Write,
                DbType = 6,
                DataId = 0,
                ItemId = 1,
                Source = new DbValue(12)
            });
            string slot2Path = session.SlotSavePath(2);
            Check(session.SaveGameTo(slot2Path, 2), "SaveGameTo(slot 2) must persist op250 CData overlays");
            // Mutate live CData after saving so 221 proves it reads slot 2's overlay, not live memory.
            vm.Database(new DbOp
            {
                Container = DbContainer.CData,
                Direction = DbDirection.Write,
                DbType = 6,
                DataId = 0,
                ItemId = 1,
                Source = new DbValue(77)
            });
            op221Inst.Self.Set(662, 1_106_000_001); // CData[6][0][1] -> hero id 12 from slot 2
            op221Inst.Self.Set(664, 1_100_001_203); // CData[0][12][3] -> "SystemFile/charicon.png"
            int budget = 32;
            vm.Vm.StepInstance(op221Inst, ref budget);
            Check(op221Inst.Self.Get(611) == 12,
                $"221 flag==1 CData numeric must return 12 from saved overlay (got {op221Inst.Self.Get(611)})");
            Check(op221Inst.Self.GetStr(607) == "SystemFile/charicon.png",
                $"221 flag==1 CData string must store into cself[7] (got '{op221Inst.Self.GetStr(607)}')");

            // LoadGameFrom(slot 2) must clear live overlay (77) and restore saved overlay (12).
            Check(session.LoadGameFrom(slot2Path), "LoadGameFrom(slot 2) must restore persisted op250 overlays");
            DbValue? restoredCell = vm.Database(new DbOp
            {
                Container = DbContainer.CData,
                Direction = DbDirection.Read,
                DbType = 6,
                DataId = 0,
                ItemId = 1
            });
            Check(restoredCell != null && !restoredCell.IsString && restoredCell.Num == 12,
                $"LoadGameFrom must restore CData[6][0][1]==12 and wipe stale live 77 (got {restoredCell?.Num})");
        }

        // Strict gate: the whole title -> new game -> load chain must not meet an
        // unimplemented opcode; either it threw already or this counter caught it.
        Check(!vm.Vm.Faulted, "save-load chain must not fault the VM (" + vm.Vm.FaultReason + ")");
        Check(vm.Vm.UnhandledOpcodeCount == 0,
            $"save-load chain saw {vm.Vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!vm2.Vm.Faulted, "title-load chain must not fault the VM (" + vm2.Vm.FaultReason + ")");
        Check(vm2.Vm.UnhandledOpcodeCount == 0,
            $"title-load chain saw {vm2.Vm.UnhandledOpcodeCount} unhandled opcodes");

        // Legacy slots may still name a child that the save-UI filter removed. Restore
        // must release that waiter rather than parking the caller forever.
        SaveState? dangling = session.BuildSaveState();
        Check(dangling != null, "save state must build for the dangling-wait probe");
        const int danglingCallerId = 9100;
        dangling?.Instances.Add(new SavedInstance
        {
            Id = danglingCallerId,
            EventId = 90013,
            Kind = "ce",
            Pc = 0,
            WaitKind = (int)WaitStateKind.Child,
            WaitChild = 999_999
        });
        string danglingPath = Path.Combine(saveDir, "save_dangling.json");
        if (dangling != null) SaveCodec.Write(danglingPath, dangling);
        Check(session.LoadGameFrom(danglingPath), "a slot with a missing child wait must still load");
        Instance? restoredCaller = vm.Vm.FindInstance(danglingCallerId);
        Check(restoredCaller != null && restoredCaller.Wait.Kind == WaitStateKind.None,
            "restore must release a caller whose saved child no longer exists");
        Check(restoredCaller?.JustResumedFromChild == true,
            "a released child waiter must resume like a normal child return");

        // Save/Load must filter transient universal/menu popup slots (15000..16999, e.g. CE64/CE101).
        SaveState? filteredState = session.BuildSaveState();
        filteredState?.Pictures.Add(new SavedPicture { Number = 15000, Kind = 4, File = "SystemFile/WindowBase.png", Args = [0, 1, 1, 1, 255, 512, 288, 100, 0] });
        filteredState?.Pictures.Add(new SavedPicture { Number = 16000, Kind = 4, File = "SystemFile/WindowBase.png", Args = [0, 1, 1, 1, 255, 512, 384, 100, 0] });
        if (filteredState != null)
        {
            filteredState.PlayFrames = -5;
            filteredState.Audio.Add(new SavedAudio { Channel = 2, File = "SE/cancel1.mp3", Volume = 80, Loop = false });
            filteredState.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/title.ogg", Volume = 70, Loop = true });
        }
        string filteredPath = Path.Combine(saveDir, "save_filtered.json");
        if (filteredState != null) SaveCodec.Write(filteredPath, filteredState);
        SaveReadResult migratedRead = SaveCodec.ReadWithSource(filteredPath);
        Check(migratedRead.State != null && migratedRead.State.PlayFrames == 0,
            "SaveMigrator must clamp negative play frames at ReadWithSource");
        Check(migratedRead.State != null && !migratedRead.State.Pictures.Any(p => p.Number is 15000 or 16000),
            "SaveMigrator must strip transient UI pictures at ReadWithSource");
        Check(migratedRead.State != null && migratedRead.State.Audio.Count == 1 && migratedRead.State.Audio[0].Channel == 0,
            "SaveMigrator must drop transient SE audio while keeping BGM/BGS");
        Check(session.LoadGameFrom(filteredPath), "filtered UI picture slot must load cleanly");
        Check(!view.SnapshotPictures().Any(p => p.Number is 15000 or 16000),
            "load must exclude transient menu/popup picture slots 15000 and 16000");

        // Loading a save during an input-denied cutscene or with pending 211 reservations must
        // wipe both opcode 126 permissions and queued reservations so the loaded save is interactive.
        vm.Vm.ApplyInputPermission(0x0201);      // deny OK
        vm.Vm.ApplyInputPermission(0x1000_0200); // device deny
        vm.Reserve(4);
        Check(vm.PendingReserveCount == 1, "test setup must queue one 211 reservation");
        Check(session.LoadGameFrom(filteredPath), "reload with active deny/reserve must succeed");
        Check(session.BasicAllDeny == 0 && !vm.Vm.DeviceDeny,
            "load must reset opcode 126 input permissions from prior cutscene");
        Check(vm.PendingReserveCount == 0, "load must clear queued 211 reservations from prior state");

        // Killing a coroutine while waiting on choice/message must resync VmController wait flags
        // and let GameSession close the stale UI overlay immediately.
        Check(session.LoadGame(), "reload slot while choice wait is open");
        Check(vm.IsWaitingForChoice() && view.HasChoices, "loaded slot must reopen choice UI");
        Instance? killedWaiter = vm.Vm.FindInstance(liveId);
        Check(killedWaiter != null, "choice waiter must exist before force-stop");
        if (killedWaiter != null) vm.Vm.StopInstance(killedWaiter.Id);
        session.AdvanceFrame();
        Check(!vm.IsWaitingForChoice() && !view.HasChoices,
            "killing a choice-waiting coroutine must clear IsWaitingForChoice and close choices");

        // Audio tracks saved with tempo <= 0 normalize cleanly to 100 instead of failing validation,
        // and WaitStateKind.KeyInput round-trips across save/load.
        SaveState? audioAndKeyState = session.BuildSaveState();
        Check(audioAndKeyState != null, "save state must build for tempo/keywait probe");
        if (audioAndKeyState != null)
        {
            audioAndKeyState.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/sparrows.mp3", Volume = 80, Loop = true, Tempo = 0 });
            audioAndKeyState.Instances.Add(new SavedInstance
            {
                Id = 9200,
                EventId = 90013,
                Kind = "ce",
                Pc = 1,
                WaitKind = (int)WaitStateKind.KeyInput,
                WaitKeyTarget = 2_000_055,
                WaitKeyMask = 177
            });
            string zeroTempoPath = Path.Combine(saveDir, "save_zero_tempo.json");
            SaveCodec.Write(zeroTempoPath, audioAndKeyState);
            Check(session.LoadGameFrom(zeroTempoPath), "slot with tempo=0 BGM and KeyInput wait must load cleanly");
            SaveState? normalized = SaveCodec.Read(zeroTempoPath);
            Check(normalized != null && normalized.Audio.Count == 1 && normalized.Audio[0].Tempo == 100,
                "SaveCodec must normalize tempo == 0 to 100");
            Instance? keyWaiter = vm.Vm.FindInstance(9200);
            Check(keyWaiter != null && keyWaiter.Wait.Kind == WaitStateKind.KeyInput && keyWaiter.Wait.KeyMask == 177,
                "KeyInput wait state must round-trip across save/load");
        }

        // SaveMigrator & SaveCodec boundaries: >256 transient UI pictures stripped before MaxPictures,
        // negative screen-layer pic.Number preserved, null JSON elements/arrays safe, negative tempo
        // rejected, O(N) instance cycle/chain validation, dangling references, and FromBytes size cap.
        var uiOverflow = new SaveState { MapId = 1, MapFile = "Map001.mps", SchemaVersion = 0 };
        for (int i = 0; i < 270; i++)
        {
            uiOverflow.Pictures.Add(new SavedPicture { Number = SaveMigrator.UiPictureFirst + i, Action = 0 });
        }
        uiOverflow.Pictures.Add(new SavedPicture { Number = -2, Action = 0, File = "Picture/bg.png" });
        uiOverflow.Pictures.Add(new SavedPicture { Number = 5, Action = 0, File = "Picture/fg.png" });
        SaveState? migratedOverflow = SaveCodec.FromJson(SaveCodec.ToJson(uiOverflow));
        Check(migratedOverflow != null && migratedOverflow.SchemaVersion == SaveState.SchemaVersionNumber
              && migratedOverflow.Pictures.Count == 2
              && migratedOverflow.Pictures.Any(p => p.Number == -2)
              && migratedOverflow.Pictures.Any(p => p.Number == 5),
            "SaveMigrator must strip >256 transient UI pictures before MaxPictures check while keeping negative screen-layer numbers");

        string nullStringsSelf = "{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"self_vars\":[{\"id\":1,\"nums\":null,\"strings\":null}]}";
        Check(SaveCodec.FromJson(nullStringsSelf) != null,
            "EventSelfVars with null nums/strings arrays must load without throwing");
        string nullStringElem = "{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"self_vars\":[{\"id\":1,\"strings\":[\"ok\",null]}]}";
        Check(SaveCodec.FromJson(nullStringElem) == null,
            "EventSelfVars with null element inside strings must be rejected cleanly");

        foreach (string field in new[] { "audio", "pictures", "instances", "map_events" })
        {
            string corruptNullElem = $"{{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"{field}\":[null]}}";
            Check(SaveCodec.FromJson(corruptNullElem) == null,
                $"null element in {field} must be rejected without throwing");
        }

        // Corrupt main with null array element must fall back to .bak and never crash Write.
        string nullElemPath = Path.Combine(saveDir, "save_null_elem.json");
        SaveCodec.Write(nullElemPath, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        SaveCodec.Write(nullElemPath, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        File.WriteAllText(nullElemPath, "{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"pictures\":[null]}");
        SaveReadResult nullFallback = SaveCodec.ReadWithSource(nullElemPath);
        Check(nullFallback.Ok && nullFallback.Source == SaveSlotSource.Backup,
            "null element in main save must trigger .bak fallback without NRE");
        SaveCodec.Write(nullElemPath, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        Check(SaveCodec.Read(nullElemPath) != null, "Write must succeed over a main slot containing null elements");

        var negTempo = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        negTempo.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/x.ogg", Tempo = -10 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(negTempo)) == null,
            "negative audio tempo must be rejected rather than normalized to 100");

        var refProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        refProbe.Instances.Add(new SavedInstance { Id = 1, EventId = 1, Kind = "ce", Pc = 0 });
        refProbe.MapEvents.Add(new SavedMapEvent { Id = 1, InstanceId = 999 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(refProbe)) == null,
            "dangling MapEvent.InstanceId must be rejected");
        refProbe.MapEvents.Clear();
        refProbe.Instances[0] = new SavedInstance { Id = 1, EventId = 1, Kind = "ce", WaitKind = (int)WaitStateKind.KeyInput, WaitKeyTarget = -1 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(refProbe)) == null,
            "negative WaitKeyTarget on KeyInput wait must be rejected");
        refProbe.Instances[0] = new SavedInstance { Id = 1, EventId = 1, Kind = "ce", WaitKind = (int)WaitStateKind.Child, WaitChild = 1 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(refProbe)) == null,
            "self-referential WaitChild must be rejected");
        refProbe.Instances[0] = new SavedInstance { Id = 1, EventId = 1, Kind = "ce", WaitKind = (int)WaitStateKind.Child, WaitChild = 0 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(refProbe)) == null,
            "zero WaitChild must be rejected");
        refProbe.Instances.Clear();
        refProbe.Instances.Add(new SavedInstance { Id = 1, EventId = 1, Kind = "ce", WaitKind = (int)WaitStateKind.Child, WaitChild = 77 });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(refProbe)) != null,
            "missing WaitChild must load so GameSession.ClearMissingChildWaits can release legacy callers");
        Check(SaveCodec.FromJson("{\"schema\":\"" + "\uD800" + "\"}") == null,
            "a lone UTF-16 surrogate must fail validation instead of throwing");
        Check(!GameSession.ValidateSaveJson("{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"map_file\":\"" + "\uD800" + "\"}", out string surrogateErr)
              && surrogateErr.Length > 0,
            "ValidateSaveJson must reject lone UTF-16 surrogates without throwing");
        Check(!GameSession.ValidateSaveJson(new string('\uD800', SaveCodec.MaxSaveBytes), out string bigSurrogateErr)
              && bigSurrogateErr.Length > 0,
            "ValidateSaveJson must reject oversized lone UTF-16 surrogates without throwing");
        Check(!GameSession.ValidateSaveJson(new string(' ', SaveCodec.MaxSaveBytes) + "{}", out string oversizeErr)
              && oversizeErr.Length > 0,
            "ValidateSaveJson must reject payloads above MaxSaveBytes before parsing");
        var strVars = new Variables();
        strVars.SetStr(1, null!);
        strVars.SetSysStr(1, null!);
        Check(strVars.GetStr(1) == "" && strVars.GetSysStr(1) == "", "SetStr/SetSysStr must coerce null to empty");
        var selfBank = new SelfVars();
        selfBank.SetStr(605, null!);
        Check(selfBank.GetStr(605) == "", "SelfVars.SetStr must coerce null to empty");

        var cycleProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        cycleProbe.Instances.Add(new SavedInstance { Id = 1, ParentId = 2, EventId = 1, Kind = "ce" });
        cycleProbe.Instances.Add(new SavedInstance { Id = 2, ParentId = 1, EventId = 1, Kind = "ce" });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(cycleProbe)) == null,
            "2-node parent cycle must be rejected");

        var chainProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        for (int i = 1; i <= 2000; i++)
        {
            chainProbe.Instances.Add(new SavedInstance { Id = i, ParentId = i == 1 ? null : i - 1, EventId = 1, Kind = "ce" });
        }
        Check(SaveCodec.FromJson(SaveCodec.ToJson(chainProbe)) != null,
            "deep 2000-node acyclic instance chain must validate cleanly");

        byte[] oversized = new byte[SaveCodec.MaxSaveBytes + 1];
        Check(SaveCodec.FromBytes(oversized) == null, "FromBytes must reject payloads above MaxSaveBytes");
        Check(SaveCodec.FromBytes(new byte[] { 0xFF, 0xFE }) == null, "FromBytes must reject invalid UTF-8");
        Check(SaveCodec.FromBytes(Array.Empty<byte>()) == null, "FromBytes must reject empty byte arrays");

        // Slot save + System.json sync + CE131 cself helper checks.
        vars.Set(531, 42);
        string slot6Path = Path.Combine(saveDir, "save_06.json");
        string systemJsonPath = Path.Combine(saveDir, "System.json");
        if (File.Exists(slot6Path)) File.Delete(slot6Path);
        if (File.Exists(systemJsonPath)) File.Delete(systemJsonPath);
        Check(session.SlotSavePath(6) == slot6Path, "SlotSavePath(6) must resolve to save_06.json");
        Check(!session.SlotExists(6) && !session.SlotExists(7), "slots 6 and 7 must start empty");
        Check(session.SaveGameToSlot(6), "SaveGameToSlot(6) must succeed on the map");
        Check(File.Exists(slot6Path) && session.SlotExists(6) && !session.SlotExists(7),
            "SaveGameToSlot(6) must create save_06.json without touching slot 7");
        SaveState? syncedSystem = SaveCodec.Read(systemJsonPath);
        Check(syncedSystem != null && syncedSystem.Vars.Count > 626
              && syncedSystem.Vars[0] == 6 && syncedSystem.Vars[546] == 6 && syncedSystem.Vars[626] == 42,
            "SaveGameToSlot(6) must sync System.json vars 0, 546 and 626");

        Instance? ce131 = vm2.Vm.CreateInstance(131);
        Check(ce131 != null, "CE131 must exist in programs.json");
        if (ce131 != null)
        {
            ce131.Self.Set(Magic.SelfArgBase + 0, 1);
            ce131.Self.Set(Magic.SelfArgBase + 27, 4);
            Check(vm2.GetCommonEventSelf(131, 0) == 1 && vm2.GetCommonEventSelf(131, -1) == -1,
                "GetCommonEventSelf must read cself[argIndex] and return -1 for negative indices");
            session2.RefreshSaveUiForSlot(6);
            Check(vm2.GetCommonEventSelf(131, 10) == 1 && vm2.GetCommonEventSelf(131, 11) == 1
                  && vm2.GetCommonEventSelf(131, 12) == 6,
                "RefreshSaveUiForSlot(6) must sync CE131 row/page/slot cself args");
            Check(vm2.Vm.Instances.Any(i => i.EventId == 94),
                "RefreshSaveUiForSlot must start CE94 to repaint the save UI");
            foreach (Instance ce94 in vm2.Vm.Instances.Where(i => i.EventId == 94).ToList())
                vm2.Vm.StopInstance(ce94.Id);
            Check(session2.SaveGameToSlot(6, false), "SaveGameToSlot(6, false) must succeed on the map");
            Check(!vm2.Vm.Instances.Any(i => i.EventId == 94),
                "SaveGameToSlot(6, false) must not start CE94 when refreshUi is false");
            Check(session2.SaveGameToSlot(6, true), "SaveGameToSlot(6, true) must succeed on the map");
            Check(vm2.Vm.Instances.Any(i => i.EventId == 94),
                "SaveGameToSlot(6, true) must start CE94 when refreshUi is true");
            foreach (Instance ce94 in vm2.Vm.Instances.Where(i => i.EventId == 94).ToList())
                vm2.Vm.StopInstance(ce94.Id);
            vm2.Vm.StopInstance(ce131.Id);
        }

        string ioTempDir = Path.Combine(Path.GetTempPath(), "lwsmf-jsonio-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(ioTempDir);
        try
        {
            string jsonFile = Path.Combine(ioTempDir, "artifact.json");
            string brFile = jsonFile + ".br";
            File.WriteAllText(jsonFile, "{}");
            File.WriteAllBytes(brFile, new byte[] { 0x06 });
            DateTime baseUtc = new DateTime(2026, 1, 1, 12, 0, 0, DateTimeKind.Utc);

            File.SetLastWriteTimeUtc(brFile, baseUtc);
            File.SetLastWriteTimeUtc(jsonFile, baseUtc.AddSeconds(10));
            Check(JsonArtifactIo.ResolvePath(jsonFile) == Path.GetFullPath(jsonFile),
                "JsonArtifactIo.ResolvePath must prefer newer plaintext .json over stale .json.br");

            File.SetLastWriteTimeUtc(jsonFile, baseUtc);
            File.SetLastWriteTimeUtc(brFile, baseUtc);
            Check(JsonArtifactIo.ResolvePath(jsonFile) == Path.GetFullPath(brFile),
                "JsonArtifactIo.ResolvePath must prefer .json.br when timestamps are equal");

            File.SetLastWriteTimeUtc(brFile, baseUtc.AddSeconds(10));
            Check(JsonArtifactIo.ResolvePath(jsonFile) == Path.GetFullPath(brFile),
                "JsonArtifactIo.ResolvePath must prefer newer .json.br over older plaintext .json");
        }
        finally
        {
            try { Directory.Delete(ioTempDir, true); }
            catch (IOException) { }
        }

        // Opcode 220 autosave (CE435, slot 1) succeeds on map and reports [AutoSave] status,
        // while refusing during "title" and "failed" phases.
        if (File.Exists(savePath)) File.Delete(savePath);
        session.HandleSaveLoad(new SaveOp { Opcode = 220, Mode = 0, File = new SaveTarget(1), Owner = 435 });
        Check(File.Exists(savePath) && session.SlotExists(1),
            "opcode 220 autosave (Owner=435) must write slot 1 on the map");
        Check(view.Status == "[AutoSave] slot=1 ok",
            $"opcode 220 autosave must report '[AutoSave] slot=1 ok' (got '{view.Status}')");

        string titleSaveDir = Path.Combine(saveDir, "title_autosave");
        Directory.CreateDirectory(titleSaveDir);
        string titleSavePath = Path.Combine(titleSaveDir, "save_01.json");
        SessionHarness harnessTitle = new(this, dataPath, titleSavePath);
        harnessTitle.Session.HandleSaveLoad(new SaveOp { Opcode = 220, Mode = 0, File = new SaveTarget(1), Owner = 435 });
        Check(!File.Exists(titleSavePath) && harnessTitle.View.Status == "[AutoSave] slot=1 fail",
            $"opcode 220 autosave must refuse during title phase (got '{harnessTitle.View.Status}')");
        harnessTitle.Session.Fail("autosave-guard-probe");
        harnessTitle.Session.HandleSaveLoad(new SaveOp { Opcode = 220, Mode = 0, File = new SaveTarget(1), Owner = 435 });
        Check(!File.Exists(titleSavePath) && harnessTitle.View.Status == "[AutoSave] slot=1 fail",
            $"opcode 220 autosave must refuse during failed phase (got '{harnessTitle.View.Status}')");

        // SaveCodec hardening: PlayFrames > int.MaxValue, NUL path, DbOverlays bounds, and SavedPicture.Flags bounds.
        Check(SaveCodec.FromJson("{\"schema\":\"lwsmf.save\",\"schema_version\":2,\"play_frames\":2147483648}") == null,
            "SaveCodec must reject play_frames > int.MaxValue");
        Check(SaveCodec.FromJson("{\"schema\":\"lwsmf.save\",\"schema_version\":2,\"map_file\":\"bad\\u0000.mps\"}") == null,
            "SaveCodec must reject NUL byte in map_file");
        Check(SaveCodec.FromJson("{\"schema\":\"lwsmf.save\",\"schema_version\":2,\"db_overlays\":{\"nums\":[{\"c\":0,\"t\":0,\"r\":100001,\"k\":0,\"v\":1}]}}") == null,
            "SaveCodec must reject out-of-bounds db_overlays cell coordinates");

        var flagsProbe = new SaveState { MapId = 1, MapFile = "Map001.mps" };
        flagsProbe.Pictures.Add(new SavedPicture
        {
            Number = 1,
            Action = 0,
            File = "cg/a.png",
            Flags = new Dictionary<string, int> { ["blend"] = 1, [new string('k', 32)] = 2 }
        });
        Check(SaveCodec.FromJson(SaveCodec.ToJson(flagsProbe)) != null,
            "SaveCodec must accept valid SavedPicture.Flags within count and key length limits");
        flagsProbe.Pictures[0].Flags = new Dictionary<string, int> { [""] = 1 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(flagsProbe)) == null,
            "SaveCodec must reject empty SavedPicture.Flags key");
        flagsProbe.Pictures[0].Flags = new Dictionary<string, int> { [new string('k', 33)] = 1 };
        Check(SaveCodec.FromJson(SaveCodec.ToJson(flagsProbe)) == null,
            "SaveCodec must reject SavedPicture.Flags key longer than 32 chars");
        var tooManyFlags = new Dictionary<string, int>();
        for (int i = 0; i < 17; i++) tooManyFlags[$"f{i}"] = i;
        flagsProbe.Pictures[0].Flags = tooManyFlags;
        Check(SaveCodec.FromJson(SaveCodec.ToJson(flagsProbe)) == null,
            "SaveCodec must reject SavedPicture.Flags with more than 16 entries");

        // Opcode 221 Sys24 existence probe must reject corrupt JSON when no valid .bak exists,
        // while still returning 1 for valid saves or corrupt main + valid .bak.
        string probeSlot9Path = session.SlotSavePath(9);
        if (File.Exists(probeSlot9Path)) File.Delete(probeSlot9Path);
        if (File.Exists(probeSlot9Path + ".bak")) File.Delete(probeSlot9Path + ".bak");
        session.InvalidateSaveCache(probeSlot9Path);
        var sys24Slot9 = new SaveOp { Opcode = 221, File = new SaveTarget(9), Address = Magic.SysBase + 24 };
        Check(session.HandleSaveLoad(sys24Slot9) == 0,
            "op 221 Sys24 must return 0 for empty slot");
        File.WriteAllText(probeSlot9Path, "{ corrupted");
        Check(session.HandleSaveLoad(sys24Slot9) == 0,
            "op 221 Sys24 must return 0 when main slot is corrupt and .bak is missing");
        File.WriteAllText(probeSlot9Path + ".bak", "{ also corrupted");
        Check(session.HandleSaveLoad(sys24Slot9) == 0,
            "op 221 Sys24 must return 0 when both main slot and .bak are corrupt");
        SaveCodec.Write(probeSlot9Path, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        SaveCodec.Write(probeSlot9Path, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        session.InvalidateSaveCache(probeSlot9Path);
        Check(session.HandleSaveLoad(sys24Slot9) == 1,
            "op 221 Sys24 must return 1 for a valid slot");
        File.WriteAllText(probeSlot9Path, "{ corrupted");
        Check(session.HandleSaveLoad(sys24Slot9) == 1,
            "op 221 Sys24 must return 1 when main slot is corrupt but .bak is valid");
        File.Delete(probeSlot9Path);
        Check(session.HandleSaveLoad(sys24Slot9) == 1,
            "op 221 Sys24 must return 1 when main slot is missing but .bak is valid");
        File.Delete(probeSlot9Path + ".bak");
        session.InvalidateSaveCache(probeSlot9Path);

        // StartNewGameNow must merge System.json vars 503/626 when system2.json is absent,
        // and skip disk sync entirely when ScriptedInput is true.
        string ngSyncDir = Path.Combine(saveDir, "ng_sync");
        Directory.CreateDirectory(ngSyncDir);
        string ngSlotPath = Path.Combine(ngSyncDir, "save_01.json");
        var ngSysState = new SaveState { Vars = new List<int>(new int[700]) };
        ngSysState.Vars[503] = 9;
        ngSysState.Vars[626] = 14;
        SaveCodec.Write(Path.Combine(ngSyncDir, "System.json"), ngSysState);
        var ngHarness = new SessionHarness(this, dataPath, ngSlotPath);
        ngHarness.Session.StartTitle();
        ngHarness.Vm.Vm.Vars.Set(503, 0);
        ngHarness.Vm.Vm.Vars.Set(626, 0);
        ngHarness.Session.ScriptedInput = true;
        ngHarness.Session.QueueTeleport(0, 3, 0, 0, 0);
        ngHarness.Session.AdvanceFrame();
        Check(ngHarness.Vm.Vm.Vars.Get(503) == 0 && ngHarness.Vm.Vm.Vars.Get(626) == 0,
            $"StartNewGameNow must skip System.json disk sync when ScriptedInput is true (got 503={ngHarness.Vm.Vm.Vars.Get(503)} 626={ngHarness.Vm.Vm.Vars.Get(626)})");
        ngHarness.Session.ScriptedInput = false;
        ngHarness.Session.QueueTeleport(0, 3, 0, 0, 0);
        ngHarness.Session.AdvanceFrame();
        Check(ngHarness.Vm.Vm.Vars.Get(503) == 9 && ngHarness.Vm.Vm.Vars.Get(626) == 14,
            $"StartNewGameNow must merge vars 503 and 626 from System.json when system2.json is absent (got 503={ngHarness.Vm.Vm.Vars.Get(503)} 626={ngHarness.Vm.Vm.Vars.Get(626)})");

        using (var iconGuard = new WindowIcons())
        {
            Check(!iconGuard.Apply(0, Array.Empty<byte>(), Array.Empty<byte>()),
                "WindowIcons.Apply must reject zero window handle and empty ICO payloads");
            Check(!iconGuard.Refresh(0),
                "WindowIcons.Refresh must reject a zero window handle before icons are applied");
            iconGuard.Release();
        }

        // Retail toilet-DLC leaked #-6 eye overlay must be stripped on bedroom saves (no #-8 body),
        // while preserved when paired with #-8 DLC_data/dlc_imouto01_01.jpg.
        var leakedEyeSave = new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" };
        leakedEyeSave.Pictures.Add(new SavedPicture { Number = -7, Action = 0, File = "cg/bg_01_home_d.jpg" });
        leakedEyeSave.Pictures.Add(new SavedPicture { Number = -6, Action = 0, File = "DLC_data/dlc_imouto01_eye.png", Anchor = 5 });
        SaveState? cleanedEye = SaveCodec.FromJson(SaveCodec.ToJson(leakedEyeSave));
        Check(cleanedEye != null && !cleanedEye.Pictures.Any(p => p.Number == -6) && cleanedEye.Pictures.Any(p => p.Number == -7),
            "SaveMigrator must strip leaked #-6 DLC_data/dlc_imouto01_eye.png when #-8 toilet CG body is absent");

        var pairedToiletPictures = new List<SavedPicture>
        {
            new() { Number = -8, Action = 0, File = "DLC_data/dlc_imouto01_01.jpg" },
            new() { Number = -6, Action = 0, File = "DLC_data/dlc_imouto01_eye.png", Anchor = 5 },
        };
        List<SavedPicture> sanitizedPaired = SaveMigrator.SanitizeRestoredPictures(pairedToiletPictures);
        Check(sanitizedPaired.Any(p => p.Number == -6) && sanitizedPaired.Any(p => p.Number == -8),
            "SaveMigrator.SanitizeRestoredPictures must preserve #-6 eye overlay when #-8 toilet body is present");

        // Retail post-load normalization: coroutine-less SampleMapA saves with v302==1 (manual save) or
        // v302==0 && v68==1 && v9==1 (AutoSave) normalize v302=2 so SampleMapA Ev3 unlocks bedroom clicks;
        // native saves carrying live Instances stay untouched.
        var retailManual = new SaveState { MapId = 1005, MapFile = "SampleMapA.mps", Vars = new List<int>(new int[310]) };
        retailManual.Vars[302] = 1;
        SaveMigrator.ApplyRetailPostLoadTriggerIfNeeded(retailManual);
        Check(retailManual.Vars[302] == 2, "coroutine-less retail manual save (v302==1) must normalize to v302=2");

        var retailAuto = new SaveState { MapId = 1005, MapFile = "SampleMapA.mps", Vars = new List<int>(new int[310]) };
        retailAuto.Vars[68] = 1;
        retailAuto.Vars[9] = 1;
        SaveMigrator.ApplyRetailPostLoadTriggerIfNeeded(retailAuto);
        Check(retailAuto.Vars[302] == 2, "coroutine-less retail AutoSave (v68==1,v9==1) must normalize to v302=2");

        var nativeWithInst = new SaveState { MapId = 1005, MapFile = "SampleMapA.mps", Vars = new List<int>(new int[310]) };
        nativeWithInst.Vars[302] = 1;
        nativeWithInst.Instances.Add(new SavedInstance { Id = 1, EventId = 15, Kind = "map", Page = 0, Pc = 18 });
        SaveMigrator.ApplyRetailPostLoadTriggerIfNeeded(nativeWithInst);
        Check(nativeWithInst.Vars[302] == 1, "native save with live Instances must not alter v302");

        // SaveAuditReport: captures schema migration, stripped transient artifacts, post-load trigger,
        // encoding warnings, and read-only suspicious locks without mutating any game variables.
        var auditedState = new SaveState
        {
            SchemaVersion = 1,
            MapId = 1005,
            MapFile = "SampleMapA.mps",
            Vars = new List<int>(new int[600]),
        };
        auditedState.Vars[302] = 1;
        auditedState.Vars[545] = 3;
        auditedState.Vars[439] = 1;
        auditedState.Vars[464] = 2;
        auditedState.Vars[60] = 1;
        auditedState.Vars[127] = 4;
        auditedState.Pictures.Add(new SavedPicture { Number = 12050, Action = 0, File = "SystemFile/WindowBase.png" });
        auditedState.Pictures.Add(new SavedPicture { Number = 15001, Action = 0, File = "SystemFile/WindowBase.png" });
        auditedState.Pictures.Add(new SavedPicture { Number = -6, Action = 0, File = "DLC_data/dlc_imouto01_eye.png" });
        auditedState.Pictures.Add(new SavedPicture { Number = -7, Action = 0, File = "cg/bg_01_home_d.jpg" });
        auditedState.Audio.Add(new SavedAudio { Channel = 2, File = "SE/cancel1.mp3", Volume = 80 });
        auditedState.Audio.Add(new SavedAudio { Channel = 0, File = "BGM/title.ogg", Volume = 70, Loop = true });

        var audit = new SaveAuditReport { SourceKind = SaveAuditSource.NativeMain };
        audit.EncodingWarnings.Add("str_var[0] at 0x0010: invalid UTF-8, replacement applied");
        SaveMigrator.Normalize(auditedState, audit);
        SaveMigrator.ApplyRetailPostLoadTriggerIfNeeded(auditedState, audit);
        Check(audit.MigratedFromSchema == 1, $"audit must record schema 1 migration (got {audit.MigratedFromSchema})");
        Check(audit.StrippedTransientPictures == 2, $"audit must count 2 stripped transient pics (got {audit.StrippedTransientPictures})");
        Check(audit.StrippedLeakedToiletEye, "audit must record leaked toilet eye stripping");
        Check(audit.StrippedTransientSe == 1, $"audit must count 1 stripped SE track (got {audit.StrippedTransientSe})");
        Check(audit.RetailPostLoadTriggerApplied, "audit must record retail post-load trigger application");
        Check(audit.EncodingWarnings.Count == 1, "audit must preserve encoding warnings");
        Check(audit.SuspiciousLocks.Count == 4, $"audit must detect all 4 suspicious locks (got {audit.SuspiciousLocks.Count})");
        Check(auditedState.Vars[545] == 3 && auditedState.Vars[439] == 1 && auditedState.Vars[464] == 2
              && auditedState.Vars[60] == 1 && auditedState.Vars[127] == 4,
            "SaveAuditReport suspicious lock inspection must stay strictly read-only and never mutate game vars");
        string auditSummary = audit.FormatSummary();
        Check(auditSummary.Contains("source=NativeMain") && auditSummary.Contains("schema=1->2")
              && auditSummary.Contains("stripped_pics=2") && auditSummary.Contains("stripped_eye=true")
              && auditSummary.Contains("stripped_se=1") && auditSummary.Contains("post_load_trigger=true")
              && auditSummary.Contains("encoding_warnings=1") && auditSummary.Contains("var[545]=3")
              && auditSummary.Contains("var[439]=1") && auditSummary.Contains("var[464]=2")
              && auditSummary.Contains("var[60]=1,var[127]=4"),
            $"FormatSummary missing expected fields: {auditSummary}");

        // Night-raid lock requires both var[60]!=0 and var[127]!=0; var[60]!=0 alone must not flag.
        var nightOnly = new List<int>(new int[600]);
        nightOnly[60] = 1;
        var nightAudit = new SaveAuditReport();
        SaveMigrator.InspectSuspiciousLocks(nightOnly, nightAudit);
        Check(nightAudit.SuspiciousLocks.Count == 0 && nightOnly[60] == 1,
            "var[60]!=0 without var[127]!=0 must not flag night_raid_lock");

        // End-to-end GameSession.LoadGameFrom updates LastSaveAudit for both NativeMain and NativeBackupFallback.
        var sessionAuditSave = session.BuildSaveState();
        Check(sessionAuditSave != null, "save state must build for session SaveAudit probe");
        if (sessionAuditSave != null)
        {
            while (sessionAuditSave.Vars.Count <= 545) sessionAuditSave.Vars.Add(0);
            sessionAuditSave.Vars[545] = 9;
            sessionAuditSave.Pictures.Add(new SavedPicture { Number = 12010, Action = 0, File = "SystemFile/WindowBase.png" });
            sessionAuditSave.Audio.Add(new SavedAudio { Channel = 2, File = "SE/click.mp3", Volume = 80 });
            string auditSlotPath = Path.Combine(saveDir, "save_audit.json");
            SaveCodec.Write(auditSlotPath, sessionAuditSave);
            Check(session.LoadGameFrom(auditSlotPath), "LoadGameFrom must succeed on audit slot");
            Check(session.LastSaveAudit != null
                  && session.LastSaveAudit.SourceKind == SaveAuditSource.NativeMain
                  && session.LastSaveAudit.StrippedTransientPictures == 1
                  && session.LastSaveAudit.StrippedTransientSe == 1
                  && session.LastSaveAudit.SuspiciousLocks.Any(l => l.Contains("var[545]=9")),
                $"LoadGameFrom must populate LastSaveAudit (got {session.LastSaveAudit?.FormatSummary() ?? "null"})");
            Check(vm.Vm.Vars.Get(545) == 9, "LoadGameFrom must not mutate var[545] during lock inspection");

            // Corrupt main with valid .bak -> LastSaveAudit reports NativeBackupFallback.
            SaveCodec.Write(auditSlotPath, sessionAuditSave);
            File.WriteAllText(auditSlotPath, "{ broken");
            Check(session.LoadGameFrom(auditSlotPath), "LoadGameFrom must recover audit slot from .bak");
            Check(session.LastSaveAudit != null
                  && session.LastSaveAudit.SourceKind == SaveAuditSource.NativeBackupFallback,
                $"backup fallback must report NativeBackupFallback (got {session.LastSaveAudit?.SourceKind})");
        }

        // MapRuntime must not abort an in-flight autorun/child-waiting page (such as battle Ev8 p1 calling CE242)
        // when its own pc=0 clears self[9]=0 and makes lower-priority p0 eligible (want == 0, not null).
        var battleVm = new EventVm();
        var childCe = Program.FromSlots(901, "child-ce", new List<Slot>
        {
            Slot.Command(180, [2]),
            Slot.Command(121, [Magic.VarBase + 77, 123, 0, 0x0C]),
        });
        battleVm.RegisterProgram(childCe);
        var page0Prog = Program.FromSlots(8, "p0", new List<Slot> { Slot.Command(104) });
        var page1Prog = Program.FromSlots(8, "p1", new List<Slot>
        {
            Slot.Command(121, [1_100_000, 0, 0, 0x0C]),
            Slot.Command(210, [500901, 0]),
        });
        var ev8Def = new MapEventDef { Id = 8, Name = "battle-click", PageCount = 2 };
        ev8Def.Pages.Add(new MapEventPage { Page = 0, Trigger = MapTrigger.Parallel, Program = page0Prog });
        var p1Page = new MapEventPage { Page = 1, Trigger = MapTrigger.Autorun, Program = page1Prog };
        p1Page.Conditions[0] = new PageCondition { Enabled = true, Operator = 2, Variable = 1_100_000, Value = 1 };
        ev8Def.Pages.Add(p1Page);
        var battleMap = new MapDefinition { Id = 1005, File = "SampleMapA.mps", Width = 10, Height = 10 };
        battleMap.Events.Add(ev8Def);
        var battleRt = new MapRuntime(battleMap);
        battleRt.Find(8)!.SelfVars.Set(100, 1);
        battleRt.StartPending(battleVm);
        for (int f = 0; f < 6; f++)
        {
            battleVm.StepFrame();
            battleRt.StartPending(battleVm);
        }
        Check(battleVm.Vars.Get(77) == 123,
            $"in-flight autorun page calling child CE must finish even when lower-priority p0 matches (got v77={battleVm.Vars.Get(77)} active={battleRt.Find(8)?.Active} starts={battleRt.Find(8)?.Starts} insts={battleVm.Instances.Count} fault={battleVm.FaultReason})");

        // Negative schema_version rejection, IsSafeAssetPath, AtomicFileIo no-BOM, WriteVarDb null collections,
        // and MediaPakReader Slicing-by-8 CRC32 / 8-byte ulong XOR parity.
        Check(SaveCodec.FromJson("{\"schema\":\"lwsmf.save\",\"schema_version\":-1}") == null,
            "SaveCodec must reject negative schema_version rather than migrating to v2");
        Check(SaveCodec.IsSafeAssetPath("Picture/face01.png")
              && !SaveCodec.IsSafeAssetPath("bad\0.png")
              && !SaveCodec.IsSafeAssetPath("../evil.png")
              && !SaveCodec.IsSafeAssetPath("/abs/evil.png")
              && !SaveCodec.IsSafeAssetPath("C:/evil.png"),
            "SaveCodec.IsSafeAssetPath must enforce relative path invariants");

        string bomCheckPath = Path.Combine(saveDir, "atomic_nobom.json");
        SaveCodec.Write(bomCheckPath, new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" });
        byte[] writtenBytes = File.ReadAllBytes(bomCheckPath);
        Check(writtenBytes.Length > 0 && !(writtenBytes.Length >= 3 && writtenBytes[0] == 0xEF && writtenBytes[1] == 0xBB && writtenBytes[2] == 0xBF),
            "AtomicFileIo/SaveCodec must write UTF-8 without BOM");

        var nullOverlayColsState = new SaveState
        {
            MapId = 1005,
            MapFile = "SampleMapA.mps",
            DbOverlays = new SavedDbOverlays { Nums = null!, Texts = null!, RowCounts = null! }
        };
        byte[] exportedNullOverlay = LegacySaveExporter.ExportSaveState(nullOverlayColsState);
        Check(exportedNullOverlay.Length > 0,
            "LegacySaveExporter.WriteVarDb must tolerate null DbOverlays collections");

        static uint ReferenceCrc32(ReadOnlySpan<byte> span)
        {
            uint crc = 0xFFFF_FFFFu;
            foreach (byte b in span)
            {
                crc ^= b;
                for (int bit = 0; bit < 8; bit++)
                {
                    crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xEDB8_8320u : (crc >> 1);
                }
            }
            return crc ^ 0xFFFF_FFFFu;
        }
        var rng = new Random(12345);
        foreach (int len in new[] { 0, 1, 7, 8, 9, 15, 63, 64, 65, 137 })
        {
            byte[] sampleBytes = new byte[len];
            rng.NextBytes(sampleBytes);
            Check(MediaPakReader.Crc32(sampleBytes) == ReferenceCrc32(sampleBytes),
                $"MediaPakReader Slicing-by-8 Crc32 mismatch at length {len}");

            byte[] a = (byte[])sampleBytes.Clone();
            byte[] b = (byte[])sampleBytes.Clone();
            MediaPakReader.XorBytes(a, 13);
            MediaPakReader.XorBytes(a, 13);
            Check(a.AsSpan().SequenceEqual(b),
                $"MediaPakReader 8-byte XorBytes involution mismatch at length {len}");
        }

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
        GetTree().Quit(_failures == 0 ? 0 : 1);
    }
}
