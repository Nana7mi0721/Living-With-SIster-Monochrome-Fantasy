using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Presentation;

/// <summary>
/// Godot-side adapter for <see cref="GameSession"/>: owns the core session and turns its
/// view callbacks into calls on the GDScript presentation root (GameRoot.gd). Keeping the
/// adapter here means the core stays free of Godot types while the scene keeps no game rules.
/// </summary>
[GlobalClass]
public partial class GameSessionController : Node
{
    private GameSession? _session;
    private VmController? _vm;
    private Godot.Control? _ui;
    private string _savePath = "";
    private bool _faultReported;

    public bool Valid => _session != null;
    public string LastError { get; private set; } = "";
    /// <summary>Increments whenever a fresh GameSession (and fresh AutoEvents) is constructed.</summary>
    public int Generation { get; private set; }
    public bool ScriptedInput => _session?.ScriptedInput ?? false;
    public string Phase => _session?.Phase ?? "boot";
    public int Frames => _session?.Frames ?? 0;
    public bool HasSave => _session?.HasSave ?? false;
    public int LeaderX => _session?.LeaderX ?? 0;
    public int LeaderY => _session?.LeaderY ?? 0;
    public int Facing => _session?.Facing ?? 2;
    public int MapId => _session?.MapData?.Id ?? -1;
    public int GetEventSelf(int eventId, int slot) => _vm?.GetEventSelf(eventId, slot) ?? -1;
    public int GetCommonEventSelf(int eventId, int argIndex) => _vm?.GetCommonEventSelf(eventId, argIndex) ?? -1;
    public int[] GetEventSelfAll(int eventId, int slot) => _vm?.GetEventSelfAll(eventId, slot) ?? Array.Empty<int>();
    public int GetSaveSlotMode() => GetCommonEventSelf(131, 0);
    public string SavePath => _session?.SavePath ?? "";
    public string SlotSavePath(int slot) => _session?.SlotSavePath(slot) ?? "";
    public bool SlotExists(int slot) => _session?.SlotExists(slot) ?? false;
    public string SaveDirectory => Path.GetDirectoryName(_session?.SavePath ?? "") ?? "";
    public bool MessageOpen => _session?.MessageOpen ?? false;
    public bool HasChoices => _session?.HasChoices ?? false;
    public bool ChoiceAllowsCancel => _session?.ChoiceAllowsCancel ?? true;
    public bool Faulted => _vm?.Faulted ?? false;
    public int AutoEventStarts(int eventId) => _session?.AutoEventStarts(eventId) ?? 0;
    /// <summary>CE242 command rounds driven by the rebuilt battle wiring.</summary>
    public long BattleRounds => _session?.BattleWiring.StartedRounds ?? 0;
    public long BattleTurnStarts => _session?.BattleWiring.TurnStarts ?? 0;
    public bool BattleSawDispatch => _session?.BattleWiring.SawDispatch ?? false;
    public bool BattleSawCaller => _session?.BattleWiring.SawCaller ?? false;
    public bool HasEvent(int eventId) => _session?.HasEvent(eventId) ?? false;

    public string DescribeEvent(int eventId) => _session?.DescribeEvent(eventId) ?? "missing";
    public string DescribeInstances() => _session?.DescribeInstances() ?? "";
    public uint MovementDeny => _session?.MovementDeny ?? 0;
    public uint BasicAllDeny => _session?.BasicAllDeny ?? 0;

    /// <summary>Wire the session to the live VM and the GDScript root. Call before StartTitle.</summary>
    public void Setup(VmController vm, Godot.Control ui) => Setup(vm, ui, "");

    public void Setup(VmController vm, Godot.Control ui, string savePath)
    {
        _vm = vm;
        _ui = ui;
        LastError = "";
        _faultReported = false;
        _savePath = savePath.Length > 0 ? savePath : DefaultSavePath();
        _session = new GameSession(vm, new CallableView(ui), DefaultDataPath(), _savePath);
        vm.SaveDirectory = Path.GetDirectoryName(_session.SavePath) ?? "";
        Generation++;
    }

    /// <summary>
    /// Session recovery: replace the core session instead of reusing it. AutoEvents is session-scoped,
    /// so a new session is the only way to re-arm a one-shot condition after a fault return.
    /// </summary>
    public bool Rebuild()
    {
        if (_vm == null || _ui == null) return false;
        LastError = "";
        _faultReported = false;
        _vm.ResetSessionState();
        Setup(_vm, _ui, _savePath);
        return true;
    }

    public void SetScriptedInput(bool value)
    {
        if (_session != null) _session.ScriptedInput = value;
    }

    public void SetAutoPickChoice(bool value)
    {
        if (_session != null) _session.AutoPickChoice = value;
    }

    public bool StartTitle() => _session?.StartTitle() ?? false;

    /// <summary>CE43 draws the title rows; the view drives it through this pair.</summary>
    public bool SetTitleMenuVisible(bool visible) => _session?.SetTitleMenuVisible(visible) ?? false;
    public void RestartTitleMenu() => _session?.RestartTitleMenu();
    public void AdvanceFrame()
    {
        if (_session == null) return;
        try
        {
            _session.AdvanceFrame();
        }
        catch (Exception ex)
        {
            LastError = ex.ToString();
            WriteCrashLog(LastError);
            _session.Fail("unhandled exception: " + ex.Message);
        }
        if (_session.Phase == "failed" && !_faultReported)
        {
            _faultReported = true;
            string reason = LastError.Length > 0 ? LastError : "session entered failed phase";
            if (_ui != null && GodotObject.IsInstanceValid(_ui))
            {
                try
                {
                    _ui.Call("report_fault", reason);
                }
                catch (Exception ex)
                {
                    GD.PushError($"[GameSessionController] failed to show fault modal: {ex.Message}");
                }
            }
        }
    }
    public bool SaveGame() => _session?.SaveGame() ?? false;
    public bool SaveGameToSlot(int slot, bool refreshUi = true) => _session?.SaveGameToSlot(slot, refreshUi) ?? false;
    public void RefreshSaveUiForSlot(int slot) => _session?.RefreshSaveUiForSlot(slot);
    public bool LoadGame() => _session?.LoadGame() ?? false;
    public bool LoadGameFrom(string path) => _session?.LoadGameFrom(path) ?? false;
    public bool LoadGameFromSlot(int slot) => _session?.LoadGameFromSlot(slot) ?? false;
    public bool SaveQuick() => _session?.SaveGameTo(QuickSavePath()) ?? false;
    public bool LoadQuick() => _session?.LoadGameFrom(QuickSavePath()) ?? false;
    /// <summary>Same load entry point, kept distinct for callers that want the status line.</summary>
    public bool LoadGameWithStatus() => _session?.LoadGame() ?? false;
    /// <summary>True when the last successful load came from the .bak rotation.</summary>
    public bool LoadedFromBackup => _session?.LoadedFromBackup ?? false;
    /// <summary>"main", "backup" or "none" for the last load attempt.</summary>
    public string LoadedFrom => _session?.LoadedFromName ?? "none";
    public bool LoadRetailSave(string path, int slotIndex = 0) =>
        _session?.LoadRetailSave(path, slotIndex) ?? false;

    /// <summary>Validate save JSON and return an empty string on success or an error message.</summary>
    public string ValidateSaveJson(string json) =>
        GameSession.ValidateSaveJson(json, out string error) ? "" : error;

    /// <summary>Import a retail .sav file into save_NN.json without altering the live session.</summary>
    public bool ImportRetailSaveToSlot(string savPath, int targetSlot)
    {
        if (_session == null)
        {
            LastError = "session not initialized";
            return false;
        }
        bool ok = _session.ImportRetailSaveToSlot(savPath, targetSlot, out string error);
        LastError = ok ? "" : error;
        return ok;
    }
    public void MoveLeader(int dx, int dy, int facing) => _session?.MoveLeader(dx, dy, facing);
    public void PressActionKey() => _session?.PressActionKey();
    public void PickChoice(int index) => _session?.PickChoice(index);
    public void SelectTitleChoice(int index) => _session?.SelectTitleChoice(index);
    public void CancelChoice() => _session?.CancelChoice();
    public void DismissMessage() => _session?.DismissMessage();
    public void ShowMessage(string text, int face) => _session?.ShowMessage(text, face);
    public void NotifyMessageClosed() => _session?.NotifyMessageClosed();
    public void ShowChoices(string[] options) => _session?.ShowChoices(options);
    public void QueueTeleport(int target, int map, int x, int y, int options) =>
        _session?.QueueTeleport(target, map, x, y, options);

    public void Fail(string reason) => _session?.Fail(reason);

    private static string DefaultDataPath() => Path.Combine(AppPaths.GameDataDir(), "programs.json");

    private static void WriteCrashLog(string detail)
    {
        try
        {
            string path = ProjectSettings.GlobalizePath("user://crash.log");
            File.AppendAllText(path,
                $"[{DateTime.UtcNow:O}] {detail}{System.Environment.NewLine}{System.Environment.NewLine}");
        }
        catch (Exception ex)
        {
            GD.PushError($"[GameSessionController] crash log write failed: {ex.Message}");
        }
    }

    // ProjectSettings owns the user:// mapping; the core session layer writes plain OS paths.
    // Tests point LWsmf_SAVE_DIR at a scratch dir: opcodes 220/221/222 persist system data,
    // and a previous run's file would otherwise flip the first-run route branches.
    private static string DefaultSavePath()
    {
        string? scratch = System.Environment.GetEnvironmentVariable("LWsmf_SAVE_DIR");
        if (!string.IsNullOrWhiteSpace(scratch))
        {
            // One directory per test process: a shared scratch would carry system data from
            // the previous gate step into the next, the same leak this isolates against.
            string dir = Path.Combine(scratch, "run-" + System.Environment.ProcessId);
            Directory.CreateDirectory(dir);
            return Path.Combine(dir, "save_01.json");
        }
        return ProjectSettings.GlobalizePath("user://save_01.json");
    }

    public string QuickSavePath()
    {
        string basePath = _savePath.Length > 0 ? _savePath : DefaultSavePath();
        return Path.Combine(Path.GetDirectoryName(basePath) ?? ".", "quicksave.json");
    }

    /// <summary>View bridge: every core callback becomes one named call on the GDScript root.</summary>
    private sealed class CallableView : IGameSessionView
    {
        private readonly Godot.Control _host;
        private readonly JsonSerializerOptions _json = new();

        public CallableView(Godot.Control host) => _host = host;

        private bool HostAlive => GodotObject.IsInstanceValid(_host);

        private static bool TryGetInt(Godot.Collections.Dictionary dict, string key, out int value)
        {
            if (dict.TryGetValue(key, out Variant raw) && raw.VariantType != Variant.Type.Nil)
            {
                value = raw.AsInt32();
                return true;
            }
            value = 0;
            return false;
        }

        private static int GetInt(Godot.Collections.Dictionary dict, string key, int fallback) =>
            TryGetInt(dict, key, out int value) ? value : fallback;

        private static string GetString(Godot.Collections.Dictionary dict, string key, string fallback) =>
            dict.TryGetValue(key, out Variant raw) && raw.VariantType != Variant.Type.Nil
                ? raw.AsString()
                : fallback;

        private static bool GetBool(Godot.Collections.Dictionary dict, string key, bool fallback) =>
            dict.TryGetValue(key, out Variant raw) && raw.VariantType != Variant.Type.Nil
                ? raw.AsBool()
                : fallback;

        public void SetStatus(string line)
        {
            if (HostAlive) _host.Call("_on_session_status", line);
        }

        public void SetMapHud(int x, int y, string text)
        {
            if (HostAlive) _host.Call("_on_session_hud", x, y, text);
        }

        public void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY)
        {
            if (HostAlive) _host.Call("_on_session_map", map.Id, leaderX, leaderY);
        }

        public void ShowMessage(string text, int face)
        {
            if (HostAlive) _host.Call("_on_message", text, face);
        }

        public void HideMessage()
        {
            if (HostAlive) _host.Call("_on_message_closed");
        }

        public void ShowChoices(string[] options, int selected)
        {
            if (HostAlive) _host.Call("_on_choices", options, selected);
        }

        public void HideChoices()
        {
            if (HostAlive) _host.Call("_on_choices_closed");
        }

        public void OnLoadCompleted()
        {
            if (HostAlive) _host.Call("_on_load_completed");
        }

        public void ClearPictures()
        {
            if (HostAlive) _host.Call("_on_clear_pictures");
        }

        public void RestorePictures(IReadOnlyList<SavedPicture> pictures)
        {
            if (HostAlive) _host.Call("_on_restore_pictures", JsonSerializer.Serialize(pictures, _json));
        }

        public IReadOnlyList<SavedPicture> SnapshotPictures()
        {
            var result = new List<SavedPicture>();
            if (!HostAlive) return result;
            Variant raw = _host.Call("_on_snapshot_pictures");
            if (raw.VariantType != Variant.Type.Array) return result;
            foreach (var item in raw.AsGodotArray())
            {
                if (item.VariantType != Variant.Type.Dictionary) continue;
                var entry = item.AsGodotDictionary();
                if (!TryGetInt(entry, "number", out int number)) continue;
                var saved = new SavedPicture
                {
                    Number = number,
                    Kind = GetInt(entry, "kind", 0),
                    Action = GetInt(entry, "action", 0),
                    File = GetString(entry, "file", ""),
                    Text = GetString(entry, "text", ""),
                    Anchor = GetInt(entry, "anchor", 0),
                };
                if (entry.TryGetValue("args", out Variant argsVar) && argsVar.VariantType == Variant.Type.Array)
                {
                    foreach (var arg in argsVar.AsGodotArray())
                    {
                        saved.Args.Add(arg.AsInt32());
                    }
                }
                result.Add(saved);
            }
            return result;
        }

        public void RestoreAudio(IReadOnlyList<SavedAudio> audio)
        {
            if (HostAlive) _host.Call("_on_restore_audio", JsonSerializer.Serialize(audio, _json));
        }

        public IReadOnlyList<SavedAudio> SnapshotAudio()
        {
            var result = new List<SavedAudio>();
            if (!HostAlive) return result;
            Variant raw = _host.Call("_on_snapshot_audio");
            if (raw.VariantType != Variant.Type.Array) return result;
            foreach (var item in raw.AsGodotArray())
            {
                if (item.VariantType != Variant.Type.Dictionary) continue;
                var entry = item.AsGodotDictionary();
                if (!TryGetInt(entry, "channel", out int channel)) continue;
                result.Add(new SavedAudio
                {
                    Channel = channel,
                    File = GetString(entry, "file", ""),
                    Volume = GetInt(entry, "volume", 100),
                    Loop = GetBool(entry, "loop", false),
                    LoopPoint = GetInt(entry, "loop_point", 0),
                    Tempo = GetInt(entry, "tempo", 100),
                    PositionMs = GetInt(entry, "position_ms", 0),
                });
            }
            return result;
        }

        public bool HasMessage => HostAlive && _host.Call("_on_has_message").AsBool();

        public string MessageText => HostAlive ? _host.Call("_on_message_text").AsString() : "";
    }
}
