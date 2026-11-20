using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace Lwsmf.Core;

/// <summary>
/// The VM operations a session needs. Implemented by the presentation VmController so the
/// core session can drive the deterministic VM without knowing about Godot signals.
/// </summary>
public interface IVmSession
{
    EventVm Vm { get; }
    MapRuntime? Map { get; set; }
    int LeaderX { get; set; }
    int LeaderY { get; set; }
    int LeaderFacing { get; set; }
    uint MovementDeny { get; }
    uint BasicAllDeny { get; }
    /// <summary>Map file registered for a map id (SysDatabase type 0, column 0).</summary>
    string MapPath(int mapId);
    bool ActionKeyRequested { get; set; }
    bool Faulted { get; }
    string FaultReason { get; }
    void StepFrame();
    /// <summary>Fire one queued opcode 211 reservation when the foreground is idle.</summary>
    void StartReservedEvent();
    void StopAll();
    void ResetDiagnostics();
    /// <summary>D-33/R7-NEW-04: clear session-scoped database overlays, vars and input policy.</summary>
    void ResetSessionState();
    bool IsWaitingForChoice();
    bool IsWaitingForMessage();
    string[] GetCurrentChoices();
    void SelectChoice(int index);
    void CancelChoice();
    void DismissMessage();
    void RestoreDialogue(string? message, string[] choices);
    DbValue? Database(DbOp op);
    /// <summary>Apply the 250 retail database overlay cells captured by the importer.</summary>
    void ApplySavedCells(IReadOnlyList<LegacySaveImporter.DatabaseCell> cells);
    /// <summary>220/221/222 hook: the session owns the slot files, the VM only asks.</summary>
    Func<SaveOp, int?>? SaveLoadHandler { get; set; }
}

/// <summary>
/// Presentation surface the session drives. Implemented by the Godot controller and test views
/// adapter and by the GameRoot.gd relay so both front-ends run the same core logic.
/// </summary>
public interface IGameSessionView
{
    void SetStatus(string text);
    void SetMapHud(int leaderX, int leaderY, string text);
    void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY);
    void ShowMessage(string text, int face);
    void HideMessage();
    void ShowChoices(string[] options, int selected);
    void HideChoices();
    void ClearPictures();
    void RestorePictures(IReadOnlyList<SavedPicture> pictures);
    IReadOnlyList<SavedPicture> SnapshotPictures();
    void RestoreAudio(IReadOnlyList<SavedAudio> audio);
    IReadOnlyList<SavedAudio> SnapshotAudio();
    bool HasMessage { get; }
    string MessageText { get; }
}

/// <summary>
/// W3 Day4: the title -> new game -> map -> save/load chain extracted from the legacy title screen.
/// Pure core code: it owns map state, the frame clock, the teleport queue and the save
/// format, and talks to the VM and the screen through the two interfaces above.
/// </summary>
public sealed class GameSession
{
    private readonly IVmSession _vm;
    private readonly IGameSessionView _view;
    private readonly string _dataPath;
    private readonly string _savePath;
    private readonly Queue<PendingTeleport> _teleports = new();
    private AutoEvents? _autoEvents;
    private readonly BattleWiring _battleWiring = new();

    private MapDefinition? _map;
    private MapRuntime? _mapRuntime;
    private int _leaderX;
    private int _leaderY;
    private int _facing = 2;
    private bool _actionPressed;
    private bool _messageOpen;
    private bool _hasChoices;
    private bool _pendingLoad;
    private bool _started;
    // 220 full loads cannot run inside a coroutine, so they are queued for the frame
    // boundary the same way teleports are (crates/wolf-game/src/game.rs save_requests).
    private string? _pendingGameLoad;
    private readonly Queue<string> _pendingSystemLoads = new();

    public GameSession(IVmSession vm, IGameSessionView view, string dataPath, string savePath)
    {
        _vm = vm;
        _view = view;
        _dataPath = dataPath;
        _savePath = savePath;
        // 220/221/222 must reach the session that owns the slot files (audit D1).
        _vm.SaveLoadHandler = HandleSaveLoad;
    }

    public string DataPath => _dataPath;
    public string SavePath => _savePath;
    public string Phase { get; private set; } = "boot";
    public MapDefinition? MapData => _map;
    public MapRuntime? MapRuntime => _mapRuntime;
    public int LeaderX => _leaderX;
    public int LeaderY => _leaderY;
    public int Facing => _facing;
    public int Frames { get; private set; }
    public bool HasChoices => _hasChoices;
    public bool MessageOpen => _messageOpen;
    /// <summary>Number of conditional common events tracked by the auto-event scheduler.</summary>
    public int AutoEventCount => EnsureAutoEvents().EventCount;
    /// <summary>Fresh starts recorded for one auto event in this session (D-33 test hook).</summary>
    public int AutoEventStarts(int eventId) => EnsureAutoEvents().StartsFor(eventId);
    /// <summary>True while a VM instance for the event is resident (test telemetry).</summary>
    public bool HasEvent(int eventId) => _vm.Vm.Instances.Any(inst => inst.EventId == eventId);
    /// <summary>Test-only: compact live instance list (event:id@pc,parent,wait).</summary>
    public string DescribeEvent(int eventId)
    {
        var state = _mapRuntime?.Events.FirstOrDefault(e => e.Def.Id == eventId);
        if (state == null) return "missing";
        var inst = state.InstanceId is int id ? _vm.Vm.FindInstance(id) : null;
        return $"active={state.Active} starts={state.Starts} on={state.StartsOnPage} inst={state.InstanceId} pc={inst?.Pc} wait={inst?.Wait.Kind}";
    }

    public string DescribeInstances()
    {
        var parts = new List<string>();
        foreach (var inst in _vm.Vm.Instances)
        {
            parts.Add($"{inst.EventId}:{inst.Id}@{inst.Pc} p={inst.ParentId?.ToString() ?? "-"} w={inst.Wait.Kind}");
        }
        return string.Join(" | ", parts);
    }
    /// <summary>Rebuilt CE242 drive (crates/wolf-game/src/battle_wiring.rs).</summary>
    public BattleWiring BattleWiring => _battleWiring;
    /// <summary>Direction/action bits currently denied by opcode 126 (D-20).</summary>
    public uint MovementDeny => _vm.MovementDeny;
    public uint BasicAllDeny => _vm.BasicAllDeny;
    public bool ScriptedInput { get; set; }
    public bool AutoPickChoice { get; set; } = true;
    public bool HasSave => _savePath.Length > 0 && SaveCodec.Exists(_savePath);
    /// <summary>File that supplied the last successful <see cref="LoadGame"/> call.</summary>
    public SaveSlotSource LoadedFrom { get; private set; } = SaveSlotSource.None;
    public bool LoadedFromBackup => LoadedFrom == SaveSlotSource.Backup;
    public string LoadedFromName => SaveCodec.SourceName(LoadedFrom);

    /// <summary>Record a fatal data error without starting the VM.</summary>
    public void Fail(string reason)
    {
        Phase = "failed";
        _view.SetStatus(reason);
    }

    /// <summary>Close the choice window and tell the VM the selection was cancelled.</summary>
    public void CancelChoice()
    {
        _hasChoices = false;
        _view.HideChoices();
        _vm.CancelChoice();
    }

    /// <summary>Start the real title event (TitleMap event 2). False means the data is missing.</summary>
    public bool StartTitle()
    {
        Program? title = ProgramLoader.LoadMapEvent(_dataPath, "TitleMap.mps", 2);
        if (title == null)
        {
            _view.SetStatus("TitleMap.mps event 2 missing");
            Phase = "failed";
            return false;
        }

        _messageOpen = false;
        _hasChoices = false;
        _actionPressed = false;
        _pendingLoad = false;
        _teleports.Clear();
        _vm.Vm.ResetInputPermissions();
        _vm.Vm.CreateInstance(title);
        _started = true;
        Phase = "title";
        _view.SetStatus("title: " + title.Title + $" ({title.Len} slots)");
        return true;
    }

    private int _titleMenuInstance = -1;

    /// <summary>
    /// CE43 paints the three title rows as pictures 3/4/5 and highlights the row Sys20 names.
    /// The port skipped it as dead code (docs/ENGINE_EVENTS.md), but the retail title menu is
    /// exactly this event: run one parallel instance while the title choice is open.
    /// </summary>
    public bool SetTitleMenuVisible(bool visible)
    {
        // A restart creates a new instance each time the cursor moves; drop every live CE43
        // first so a title hand-off never leaves a painter running in the map (perf gate).
        StopTitleMenuInstances();
        if (!visible) return true;
        var inst = _vm.Vm.CreateInstance(43);
        if (inst == null) return false;
        inst.IsParallel = true;
        _titleMenuInstance = inst.Id;
        return true;
    }

    private void StopTitleMenuInstances()
    {
        if (_vm.Vm.Instances.Count > 0)
        {
            var ids = new List<int>();
            foreach (var inst in _vm.Vm.Instances)
            {
                if (inst.EventId == 43) ids.Add(inst.Id);
            }
            foreach (var id in ids) _vm.Vm.StopInstance(id);
        }
        _titleMenuInstance = -1;
    }

    /// <summary>Re-run CE43 so its Sys20 branch repaints the highlighted row.</summary>
    public void RestartTitleMenu()
    {
        SetTitleMenuVisible(false);
        SetTitleMenuVisible(true);
    }

    /// <summary>One VM frame; the deterministic entry point used by tests.</summary>
    public void AdvanceFrame()
    {
        if (!_started || Phase == "failed") return;
        if (ScriptedInput)
        {
            if (_messageOpen) DismissMessage();
            if (_hasChoices && AutoPickChoice) PickChoice(0);
        }

        Frames++;
        _vm.LeaderX = _leaderX;
        _vm.LeaderY = _leaderY;
        _vm.LeaderFacing = _facing;
        _vm.ActionKeyRequested = _actionPressed && !_messageOpen && !_hasChoices
            && (BasicAllDeny & KeyCodes.Ok) == 0;
        _actionPressed = false;
        // Rust Game::frame syncs conditional common events before the VM step, so a
        // newly-triggered event starts executing in this same frame.
        EnsureAutoEvents().Sync(_vm.Vm);
        _battleWiring.Tick(_vm.Vm, op => _vm.Database(op));
        _vm.StepFrame();
        ApplyPending();
        _vm.StartReservedEvent();
        if (_messageOpen && !_vm.IsWaitingForMessage())
        {
            NotifyMessageClosed();
        }
        if (_hasChoices && !_vm.IsWaitingForChoice())
        {
            _hasChoices = false;
            _view.HideChoices();
        }
        UpdateMapHud(false);
    }

    private AutoEvents EnsureAutoEvents()
    {
        _autoEvents ??= AutoEvents.FromPrograms(_vm.Vm.Programs.Values);
        return _autoEvents;
    }

    /// <summary>Map changes are applied between frames, never inside a running coroutine.</summary>
    private void ApplyPending()
    {
        while (_teleports.Count > 0) ApplyTeleport(_teleports.Dequeue());
        while (_pendingSystemLoads.Count > 0) LoadSystemData(_pendingSystemLoads.Dequeue());
        if (_pendingGameLoad != null)
        {
            string path = _pendingGameLoad;
            _pendingGameLoad = null;
            LoadGameFrom(path);
        }
        if (_pendingLoad)
        {
            _pendingLoad = false;
            LoadGame();
        }
    }

    private void UpdateMapHud(bool blocked)
    {
        if (_mapRuntime == null || _map == null) return;
        string hud = blocked
            ? $"MAP {_map.Id} {_map.File}  {_map.Width}x{_map.Height}    BLOCKED    " +
              $"LEADER {_leaderX},{_leaderY}    INST {_vm.Vm.Instances.Count}    SAVE {(HasSave ? "01" : "-")}    F {Frames}"
            : $"MAP {_map.Id} {_map.File}  {_map.Width}x{_map.Height}    " +
              $"EVENTS {_mapRuntime.Events.Count}  AUTO {_mapRuntime.AutorunCount}  PAR {_mapRuntime.ParallelCount}    " +
              $"LEADER {_leaderX},{_leaderY}    INST {_vm.Vm.Instances.Count}    SAVE {(HasSave ? "01" : "-")}    F {Frames}";
        _view.SetMapHud(_leaderX, _leaderY, hud);
    }

    // ---- dialogue -------------------------------------------------------

    public void ShowMessage(string text, int face)
    {
        _messageOpen = true;
        _view.ShowMessage(text, face);
    }

    public void NotifyMessageClosed()
    {
        _messageOpen = false;
        _view.HideMessage();
    }

    public void DismissMessage()
    {
        _messageOpen = false;
        _view.HideMessage();
        _vm.DismissMessage();
    }

    public void ShowChoices(string[] options)
    {
        _hasChoices = true;
        _lastChoices = options;
        _view.ShowChoices(options, 0);
    }

    /// <summary>Last window contents; the title menu reopens on a load with no slot.</summary>
    private string[] _lastChoices = System.Array.Empty<string>();

    public void PickChoice(int index)
    {
        _hasChoices = false;
        _view.HideChoices();
        // Title menu: row 0 new game (the event teleports to map 3), row 1 load, row 2 quit.
        // SaveLoadTest calls PickChoice(1) directly when HasSave is true to quick-load slot 01.
        if (Phase == "title" && index == 1 && HasSave)
        {
            _pendingLoad = true;
            return;
        }
        _vm.SelectChoice(index);
    }

    /// <summary>
    /// Interactive title menu selection: forward every option (including Continue=1 -> CE42 -> CE14)
    /// directly to the VM so clicking "继续咯" opens the 4-slot Save/Load UI.
    /// </summary>
    public void SelectTitleChoice(int index)
    {
        _hasChoices = false;
        _view.HideChoices();
        _vm.SelectChoice(index);
    }

    // ---- movement and teleports ----------------------------------------

    public const int MaxTeleportsPerFrame = 64;

    public void QueueTeleport(int target, int map, int x, int y, int options)
    {
        if (_teleports.Count >= MaxTeleportsPerFrame)
        {
            _view.SetStatus($"teleport queue full ({MaxTeleportsPerFrame}); dropping map={map}");
            return;
        }
        _teleports.Enqueue(new PendingTeleport(target, map, x, y, options));
    }

    /// <summary>Opcode 116 applied at the frame boundary (target 0 = leader).</summary>
    private void ApplyTeleport(PendingTeleport request)
    {
        _view.SetStatus($"teleport map={request.Map} x={request.X} y={request.Y} options={request.Options}");
        // 130 target -1 (and legacy 0) is the leader; any other id names a different
        // event, which the leader-mode session cannot move (round-4 review P1).
        if (request.Target != -1 && request.Target != 0)
        {
            _view.SetStatus($"teleport target {request.Target} is not the leader; ignored");
            return;
        }
        if (request.Map == 3)
        {
            StartNewGameNow();
            return;
        }
        if (_map != null && _map.Id == request.Map && _leaderX == request.X && _leaderY == request.Y) return;

        _leaderX = request.X;
        _leaderY = request.Y;
        EnterMap(request.Map);
    }

    /// <summary>Turn towards the step and move one tile unless the map or an active event page blocks it.</summary>
    public void MoveLeader(int dx, int dy, int facing)
    {
        if (_map == null) return;
        // A 126 state 1/2 ban on this direction still faces nothing and does not move
        // (crates/wolf-game/src/game.rs advance_player_input filters held directions).
        uint direction = dy < 0 ? KeyCodes.Up
            : dy > 0 ? KeyCodes.Down
            : dx < 0 ? KeyCodes.Left
            : dx > 0 ? KeyCodes.Right
            : 0;
        if ((MovementDeny & direction) != 0) return;
        _facing = facing;
        _vm.LeaderFacing = facing;
        int fromX = _leaderX;
        int fromY = _leaderY;
        int toX = _leaderX + dx;
        int toY = _leaderY + dy;
        int step = dy < 0 ? 0 : dx > 0 ? 1 : dx < 0 ? 2 : 3;
        bool blocked = _mapRuntime != null && !_mapRuntime.CanEnter(toX, toY, step);
        if (blocked)
        {
            // The attempted step still touches player-touch pages (map_runtime.rs:1031).
            FireContactTriggers(fromX, fromY, toX, toY);
            UpdateMapHud(true);
            return;
        }
        _leaderX = toX;
        _leaderY = toY;
        FireContactTriggers(fromX, fromY, toX, toY);
        UpdateMapHud(false);
    }

    private void FireContactTriggers(int fromX, int fromY, int toX, int toY)
    {
        if (_mapRuntime == null) return;
        foreach (var contact in _mapRuntime.ContactTriggers(fromX, fromY, toX, toY))
        {
            _mapRuntime.StartTriggerPage(contact.State.Def.Id, contact.Page, _vm.Vm);
        }
    }

    public void PressActionKey() => _actionPressed = true;

    // ---- save / load ----------------------------------------------------

    /// <summary>Build the complete slot payload from the live map state.</summary>
    public SaveState? BuildSaveState()
    {
        if (Phase != "bedroom" || _map == null || _mapRuntime == null) return null;
        if (_vm.Faulted) return null;

        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = Frames,
            MapId = _map.Id,
            MapFile = _map.File,
            LeaderX = _leaderX,
            LeaderY = _leaderY,
            Facing = _facing,
            Vars = _vm.Vm.Vars.SnapshotNums(),
            StrVars = _vm.Vm.Vars.SnapshotStrings(),
            SysVars = _vm.Vm.Vars.SnapshotSysNums(),
            SysStrVars = _vm.Vm.Vars.SnapshotSysStrings(),
            RngState = _vm.Vm.Rng.State,
            RngEventState = _vm.Vm.Rng.EventState,
            RngEventSeed = _vm.Vm.Rng.EventSeed,
            RngDraws = _vm.Vm.Rng.Draws
        };

        foreach (var snapshot in _mapRuntime.SnapshotSelfVars())
        {
            state.SelfVars.Add(SavedSelf(snapshot.Key, snapshot.Value));
        }
        foreach (var bank in _vm.Vm.CommonSelf)
        {
            state.CommonSelf.Add(SavedSelf(bank.Key, bank.Value));
        }

        state.MapEvents = _mapRuntime.SnapshotStates(_vm.Vm);
        foreach (Instance inst in _vm.Vm.Instances)
        {
            MapEventState? owner = _mapRuntime.Events.FirstOrDefault(e => e.InstanceId == inst.Id);
            var saved = new SavedInstance
            {
                Id = inst.Id,
                ParentId = inst.ParentId,
                EventId = inst.EventId,
                Pc = inst.Pc,
                ReturnTo = inst.ReturnTo,
                Kind = owner != null ? "map" : "ce",
                Page = owner?.Active ?? -1,
                WaitKind = (int)inst.Wait.Kind,
                WaitFrames = inst.Wait.Frames,
                WaitBlock = inst.Wait.Block,
                WaitChild = inst.Wait.ChildId,
                Self = SavedSelf(inst.EventId, inst.Self)
            };
            foreach (LoopCtx loop in inst.Loops)
            {
                saved.Loops.Add(new SavedLoop { Block = loop.Block, Remaining = loop.Remaining });
            }
            state.Instances.Add(saved);
        }

        state.Pictures.AddRange(_view.SnapshotPictures());
        state.Audio.AddRange(_view.SnapshotAudio());

        if (_view.HasMessage)
        {
            state.Dialogue = new SavedDialogue { Message = _view.MessageText };
        }
        else if (_vm.IsWaitingForChoice())
        {
            state.Dialogue = new SavedDialogue { Choices = _vm.GetCurrentChoices().ToList() };
        }

        return state;
    }

    public bool SaveGame() => SaveGameTo(_savePath);

    /// <summary>Write one full slot to an explicit file (opcode 220 with a numeric slot).</summary>
    public bool SaveGameTo(string path)
    {
        if (Phase != "bedroom" || _map == null || _mapRuntime == null)
        {
            _view.SetStatus("save: only while walking a map");
            return false;
        }
        if (_vm.Faulted)
        {
            // An unimplemented opcode means the live state diverged from the retail VM;
            // writing it would produce a slot that cannot be trusted on load.
            _view.SetStatus("save disabled: " + _vm.FaultReason);
            return false;
        }

        SaveState? state = BuildSaveState();
        if (state == null) return false;

        try
        {
            SaveCodec.Write(path, state);
        }
        catch (IOException ex)
        {
            _view.SetStatus("save failed: " + ex.Message);
            return false;
        }

        _view.SetStatus($"saved slot 01 (map {state.MapId} {state.LeaderX},{state.LeaderY})");
        UpdateMapHud(false);
        return true;
    }

    /// <summary>
    /// Load slot 01, preferring the main file and falling back to its .bak rotation.
    /// <see cref="LoadedFrom"/> records which file won so F5/F9 can say "main" or "backup".
    /// A missing or unreadable slot (and a slot whose map left programs.json) leaves the
    /// live bedroom untouched; a late fault is rolled back to the pre-load snapshot.
    /// </summary>
    public bool LoadGame() => LoadGameFrom(_savePath);

    /// <summary>Load one full slot from an explicit file, same gates as slot 01.</summary>
    public bool LoadGameFrom(string path)
    {
        LoadedFrom = SaveSlotSource.None;
        SaveReadResult read = SaveCodec.ReadWithSource(path);
        SaveState? state = read.State;
        if (state == null)
        {
            _view.SetStatus("load: slot 01 missing or unreadable");
            return false;
        }

        // M5-D: resolve the destination before touching the live VM, so a stale slot
        // cannot strand the player outside any map.
        if (!CanEnterMap(state.MapId))
        {
            _view.SetStatus($"load failed: map {state.MapId} missing from programs.json");
            return false;
        }

        SaveState? rollback = Phase == "bedroom" && !_vm.Faulted ? BuildSaveState() : null;
        if (!ApplyLoadedState(state))
        {
            if (rollback != null && RestoreAfterFailedLoad(rollback))
            {
                _view.SetStatus($"load failed: kept map {rollback.MapId} at {rollback.LeaderX},{rollback.LeaderY}");
            }
            return false;
        }

        LoadedFrom = read.Source;
        // EnterMap -> ShowMap clears the layer, so the saved slots are replayed after it.
        _view.RestorePictures(state.Pictures);
        // Audio last: the map load above may start its own track, and the slot wins.
        _view.RestoreAudio(state.Audio);
        _view.SetStatus($"loaded slot 01 from {SaveCodec.SourceName(read.Source)} (map {state.MapId} {state.LeaderX},{state.LeaderY})");
        UpdateMapHud(false);
        return true;
    }

    // ---- script save/load (opcodes 220/221/222, audit D1) ----------------

    /// <summary>
    /// Map a 220/221/222 file operand onto this port's slots. A numeric slot is one-based
    /// (WOLF names them SaveDataNN); an explicit path keeps only its file name and lives
    /// beside slot 01, so a retail "Save/System.sav" can never write outside the user dir.
    /// </summary>
    private string ResolveSaveTarget(SaveTarget file)
    {
        string dir = Path.GetDirectoryName(_savePath) ?? ".";
        if (file.IsPath)
        {
            string name = Path.GetFileName(file.Path);
            if (name.Length == 0) name = "System.sav";
            return Path.Combine(dir, Path.ChangeExtension(name, ".json"));
        }
        int slot = file.Slot <= 1 ? 1 : file.Slot;
        return Path.Combine(dir, $"save_{slot:00}.json");
    }

    /// <summary>220/221/222 entry point. Null means the VM has nothing to store back.</summary>
    public int? HandleSaveLoad(SaveOp op)
    {
        string path = ResolveSaveTarget(op.File);
        switch (op.Opcode)
        {
            case 220:
                // A path target is the retail system data (banks only); a slot target is a
                // whole game save. Full loads are queued for the frame boundary.
                if (op.File.IsPath)
                {
                    if (op.Mode == 1) _pendingSystemLoads.Enqueue(path);
                    else SaveSystemData(path);
                }
                else if (op.Mode == 1)
                {
                    _pendingGameLoad = path;
                }
                else
                {
                    SaveGameTo(path);
                }
                return null;
            case 221:
            {
                SaveState? state = SaveCodec.Read(path);
                // A missing tier is the normal "no data here" answer for the existence probes.
                return state == null ? 0 : ReadSaveAddress(state, op, path);
            }
            case 222:
            {
                SaveState state = SaveCodec.Read(path) ?? new SaveState();
                if (WriteSaveAddress(state, op)) SaveCodec.Write(path, state);
                return null;
            }
        }
        return null;
    }

    /// <summary>Write the retail system-data file: variable banks only, no map state.</summary>
    private SaveState BuildSystemState()
    {
        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = Frames,
            Vars = _vm.Vm.Vars.SnapshotNums(),
            StrVars = _vm.Vm.Vars.SnapshotStrings(),
            SysVars = _vm.Vm.Vars.SnapshotSysNums(),
            SysStrVars = _vm.Vm.Vars.SnapshotSysStrings(),
        };
        foreach (var bank in _vm.Vm.CommonSelf) state.CommonSelf.Add(SavedSelf(bank.Key, bank.Value));
        return state;
    }

    private void SaveSystemData(string path)
    {
        try
        {
            SaveCodec.Write(path, BuildSystemState());
        }
        catch (IOException ex)
        {
            _view.SetStatus("system save failed: " + ex.Message);
        }
    }

    /// <summary>Restore a system-data file: banks only, the live map stays where it is.</summary>
    private void LoadSystemData(string path)
    {
        SaveState? state = SaveCodec.Read(path);
        if (state == null)
        {
            _view.SetStatus("system load: no system data at " + Path.GetFileName(path));
            return;
        }
        _vm.Vm.Vars.Restore(state.Vars, state.StrVars, state.SysVars, state.SysStrVars);
        _view.SetStatus("system load: restored " + Path.GetFileName(path));
    }

    /// <summary>Read a 221 address: normal, self and system banks share the VM's magic ids.</summary>
    private int ReadSaveAddress(SaveState state, SaveOp op, string path)
    {
        int address = op.Address;
        if (address is >= 2_000_000 and <= 2_999_999)
        {
            int index = address - 2_000_000;
            return index < state.Vars.Count ? state.Vars[index] : 0;
        }
        if (address is >= 1_600_000 and <= 1_699_999)
        {
            EventSelfVars? bank = FindSelfBank(state, op.Owner);
            if (bank == null) return 0;
            int index = address - 1_600_000;
            if (bank.NumMap != null && bank.NumMap.TryGetValue(index, out int value)) return value;
            return index < bank.Nums.Length ? bank.Nums[index] : 0;
        }
        if (address is >= 9_000_000 and <= 9_099_999)
        {
            int index = address - 9_000_000;
            if (index == 24) return SaveCodec.Exists(path) ? 1 : 0;
            if (index == 29) return (int)(state.PlayFrames / 60);
            if (index <= 12) return index < state.SysVars.Count ? state.SysVars[index] : 0;
        }
        return 0;
    }

    /// <summary>Write one 222 value; false means the address is outside every bank.</summary>
    private static bool WriteSaveAddress(SaveState state, SaveOp op)
    {
        int address = op.Address;
        if (address is >= 2_000_000 and <= 2_999_999)
        {
            int index = address - 2_000_000;
            if (index >= Variables.MaxSlots) return false;
            while (state.Vars.Count <= index) state.Vars.Add(0);
            state.Vars[index] = op.Value;
            return true;
        }
        if (address is >= 1_600_000 and <= 1_699_999)
        {
            EventSelfVars? bank = FindSelfBank(state, op.Owner);
            if (bank == null)
            {
                bank = new EventSelfVars { Id = op.Owner, NumMap = new Dictionary<int, int>() };
                state.SelfVars.Add(bank);
            }
            bank.NumMap ??= new Dictionary<int, int>();
            bank.NumMap[address - 1_600_000] = op.Value;
            return true;
        }
        if (address is >= 9_000_000 and <= 9_099_999)
        {
            int index = address - 9_000_000;
            if (index == 29)
            {
                state.PlayFrames = Math.Max(0, op.Value) * 60L;
                return true;
            }
            if (index > 12) return false;
            while (state.SysVars.Count <= index) state.SysVars.Add(0);
            state.SysVars[index] = op.Value;
            return true;
        }
        return false;
    }

    private static EventSelfVars? FindSelfBank(SaveState state, int owner)
    {
        foreach (var bank in state.SelfVars) if (bank.Id == owner) return bank;
        foreach (var bank in state.CommonSelf) if (bank.Id == owner) return bank;
        return null;
    }

    /// <summary>Apply a slot that already passed the codec gates and the map-file check.</summary>
    private bool ApplyLoadedState(SaveState state)
    {
        _vm.Vm.StopAll();
        _messageOpen = false;
        _hasChoices = false;
        _view.HideMessage();
        _view.HideChoices();
        _vm.Map = null;
        _mapRuntime = null;
        _map = null;
        Frames = (int)state.PlayFrames;
        _leaderX = state.LeaderX;
        _leaderY = state.LeaderY;
        _facing = state.Facing;
        _vm.Vm.Vars.Restore(state.Vars, state.StrVars, state.SysVars, state.SysStrVars);
        _vm.Vm.Rng.RestoreState(state.RngState, state.RngEventState, state.RngEventSeed, state.RngDraws);
        _vm.Vm.CommonSelf.Clear();
        foreach (var saved in state.CommonSelf)
        {
            var bank = new SelfVars();
            bank.Restore(saved);
            _vm.Vm.CommonSelf[saved.Id] = bank;
        }

        var selfVars = new Dictionary<int, SelfVars>();
        foreach (var saved in state.SelfVars)
        {
            var vars = new SelfVars();
            vars.Restore(saved);
            selfVars[saved.Id] = vars;
        }

        EnterMap(state.MapId, selfVars, state);
        EnsureAutoEvents().PrimeLatched(_vm.Vm);
        if (_vm.Faulted)
        {
            _view.SetStatus("load faulted: " + _vm.FaultReason);
            return false;
        }
        if (Phase == "failed") return false;
        // A load into a freshly rebuilt session (fault recovery) must resume the clock.
        _started = true;
        return true;
    }

    /// <summary>Best-effort rollback after a late load failure; diagnostics must not latch.</summary>
    private bool RestoreAfterFailedLoad(SaveState snapshot)
    {
        _vm.ResetDiagnostics();
        return ApplyLoadedState(snapshot);
    }

    /// <summary>True when programs.json resolves the slot's map exactly like EnterMap.</summary>
    private bool CanEnterMap(int mapId)
    {
        string mapPath = _vm.MapPath(mapId);
        return mapPath.Length > 0
            ? ProgramLoader.LoadMap(_dataPath, mapPath, null) != null
            : ProgramLoader.LoadMap(_dataPath, "", mapId) != null;
    }

    /// <summary>
    /// Load a retail <c>.sav</c> into a live bedroom session. The save's 800 normal variables,
    /// common-event self banks (Part3), picture layer (Part5) and 250 DB overlay cells are
    /// restored; map 1 (SampleMapA) is used and event 0 (new-game setup) is skipped so the
    /// loaded day is not reset to day 1. This mirrors Rust's <c>resume(slot)</c> path.
    /// </summary>
    public bool LoadRetailSave(string path, int slotIndex = 0)
    {
        LegacySaveImporter.SlotResult slot;
        try
        {
            slot = LegacySaveImporter.ImportFile(path, slotIndex, rejectBadChecksum: true);
        }
        catch (Exception ex)
        {
            _view.SetStatus("retail load failed: " + ex.Message);
            return false;
        }

        if (!slot.ChecksumOk)
        {
            _view.SetStatus("retail load: checksum mismatch");
            return false;
        }

        LegacySaveImporter.RecoveredData data = slot.Recovered;
        _vm.Vm.StopAll();
        _vm.ResetSessionState();
        _vm.Map = null;
        _mapRuntime = null;
        _map = null;
        _messageOpen = false;
        _hasChoices = false;
        _actionPressed = false;
        _pendingLoad = false;
        _teleports.Clear();

        Frames = (int)data.PlayTime.Frames;
        _leaderX = 0;
        _leaderY = 0;
        _facing = 2;

        var numbers = new List<int>(Enumerable.Repeat(0, 800));
        foreach (KeyValuePair<string, int> pair in data.Variables)
        {
            if (int.TryParse(pair.Key, out int index) && index >= 0 && index < numbers.Count)
            {
                numbers[index] = pair.Value;
            }
        }
        _vm.Vm.Vars.Restore(numbers, null, null, null);
        if (data.ChoiceAnchor is { Length: >= 2 })
        {
            _vm.Vm.Vars.SetSys(3, data.ChoiceAnchor[0]);
            _vm.Vm.Vars.SetSys(4, data.ChoiceAnchor[1]);
        }

        _vm.Vm.CommonSelf.Clear();
        foreach (KeyValuePair<int, int[]> pair in data.CommonSelfNumbers)
        {
            var bank = new SelfVars();
            bool any = false;
            for (int i = 0; i < pair.Value.Length; i++)
            {
                if (pair.Value[i] == 0) continue;
                bank.Set(600 + i, pair.Value[i]);
                any = true;
            }
            if (any) _vm.Vm.CommonSelf[pair.Key] = bank;
        }
        foreach (KeyValuePair<int, string[]> pair in data.CommonSelfStrings)
        {
            if (!_vm.Vm.CommonSelf.TryGetValue(pair.Key, out SelfVars? bank))
            {
                bank = new SelfVars();
                _vm.Vm.CommonSelf[pair.Key] = bank;
            }
            for (int i = 0; i < pair.Value.Length; i++)
            {
                if (!string.IsNullOrEmpty(pair.Value[i])) bank.SetStr(605 + i, pair.Value[i]);
            }
        }

        _vm.ApplySavedCells(data.DatabaseCells);
        _view.ClearPictures();
        EnterMap(1, null, null, skipEventId: 0);
        if (_vm.Faulted)
        {
            _view.SetStatus("retail load faulted: " + _vm.FaultReason);
            return false;
        }
        if (Phase == "failed") return false;

        EnsureAutoEvents().PrimeLatched(_vm.Vm);
        _view.RestorePictures(data.PictureStates);
        _started = true;
        _view.SetStatus($"loaded retail save {slot.File} (map 1 day {_vm.Vm.Vars.Get(160)})");
        UpdateMapHud(false);
        return true;
    }

    private void StartNewGameNow()
    {
        Phase = "newgame";
        _vm.Vm.StopAll();
        _messageOpen = false;
        _hasChoices = false;
        _view.HideMessage();
        _view.HideChoices();
        // AutoEvents is session-scoped like Rust's Game::auto_events; StopAll does not
        // clear its edge latch. Re-arming here would replay event 38 and shift instance ids
        // (D-33). A fresh engine boot constructs a fresh scheduler instead.
        _vm.ResetDiagnostics();
        _vm.Vm.ResetInputPermissions();
        _vm.Vm.CommonSelf.Clear();
        _vm.Map = null;
        _view.ClearPictures();
        Program? newGame = ProgramLoader.LoadMapEvent(_dataPath, "Map003.mps", 1);
        if (newGame == null)
        {
            _view.SetStatus("Map003 event 1 (new game) missing");
            Phase = "failed";
            return;
        }
        _vm.Vm.CreateInstance(newGame);
        _view.SetStatus("new game: " + newGame.Title);
    }

    private static EventSelfVars SavedSelf(int id, SelfVars vars)
    {
        return EventSelfVars.Pack(id, vars.SnapshotNums(), vars.SnapshotStrings());
    }

    private void RestoreInstances(SaveState state)
    {
        foreach (SavedInstance saved in state.Instances)
        {
            Program? program = null;
            if (saved.Kind == "map" && saved.Page >= 0)
            {
                MapEventDef? def = _map?.Events.FirstOrDefault(e => e.Id == saved.EventId);
                if (def != null && saved.Page < def.Pages.Count) program = def.Pages[saved.Page].Program;
            }
            else
            {
                program = _vm.Vm.GetProgram(saved.EventId);
            }

            if (program == null)
            {
                Console.Error.WriteLine($"[Session] cannot restore instance {saved.Id}: program {saved.EventId} page {saved.Page}");
                continue;
            }

            if (saved.Pc < 0 || saved.Pc > program.Len)
            {
                Console.Error.WriteLine($"[Session] cannot restore instance {saved.Id}: pc {saved.Pc} outside program {saved.EventId} (len {program.Len})");
                continue;
            }

            var inst = new Instance
            {
                Id = saved.Id,
                ParentId = saved.ParentId,
                EventId = saved.EventId,
                Program = program,
                Pc = saved.Pc,
                ReturnTo = saved.ReturnTo
            };
            inst.Wait.Kind = (WaitStateKind)saved.WaitKind;
            inst.Wait.Frames = saved.WaitFrames;
            inst.Wait.Block = saved.WaitBlock;
            inst.Wait.ChildId = saved.WaitChild;
            inst.Self.Restore(saved.Self);
            foreach (SavedLoop loop in saved.Loops)
            {
                inst.Loops.Add(new LoopCtx { Block = loop.Block, Remaining = loop.Remaining });
            }
            _vm.Vm.RestoreInstance(inst);
        }
    }

    private void EnterMap(int mapId, Dictionary<int, SelfVars>? selfVars = null, SaveState? restore = null,
        int? skipEventId = null)
    {
        _vm.Vm.StopAll();
        _messageOpen = false;
        _hasChoices = false;
        _view.HideMessage();
        _view.HideChoices();
        _vm.Map = null;
        _mapRuntime = null;

        // Opcode 130 carries a SystemDB row, not the artifact map id: row 1 is
        // SampleMapA, while programs.json's id 1 is SampleMapA_1. Resolve the file first.
        string mapPath = _vm.MapPath(mapId);
        _map = mapPath.Length > 0
            ? ProgramLoader.LoadMap(_dataPath, mapPath, null)
            : ProgramLoader.LoadMap(_dataPath, "", mapId);
        if (_map == null)
        {
            _view.SetStatus($"map {mapId} missing from programs.json");
            Phase = "failed";
            return;
        }

        _mapRuntime = new MapRuntime(_map);
        MapTileData? tileData = MapTileData.LoadAlongside(_dataPath);
        if (tileData != null) _mapRuntime.SetTerrain(tileData.BuildTerrain(_map));
        _vm.Map = _mapRuntime;
        Phase = "bedroom";

        if (_leaderX < 0 || _leaderX >= _map.Width) _leaderX = Math.Clamp(_leaderX, 0, Math.Max(0, _map.Width - 1));
        if (_leaderY < 0 || _leaderY >= _map.Height) _leaderY = Math.Clamp(_leaderY, 0, Math.Max(0, _map.Height - 1));

        if (selfVars != null) _mapRuntime.RestoreSelfVars(selfVars);
        if (restore != null)
        {
            _mapRuntime.RestoreStates(restore.MapEvents);
            RestoreInstances(restore);
            // A skipped instance (missing program/page) must not leave a page pointing
            // at a coroutine that does not exist; the next SelectPage would trust it.
            foreach (var state in _mapRuntime.Events)
            {
                if (state.InstanceId is int restoredId && _vm.Vm.FindInstance(restoredId) == null)
                {
                    state.InstanceId = null;
                }
            }
        }
        if (skipEventId is int skipped)
        {
            MapEventState? state = _mapRuntime.Events.FirstOrDefault(e => e.Def.Id == skipped);
            if (state != null)
            {
                state.Runs = false;
                state.Active = null;
                state.InstanceId = null;
            }
        }
        _view.ShowMap(_map, _mapRuntime, _leaderX, _leaderY);
        _mapRuntime.StartPending(_vm.Vm);
        if (restore?.Dialogue != null)
        {
            _vm.RestoreDialogue(restore.Dialogue.Message, restore.Dialogue.Choices.ToArray());
        }
    }

    private readonly struct PendingTeleport
    {
        public PendingTeleport(int target, int map, int x, int y, int options)
        {
            Target = target;
            Map = map;
            X = x;
            Y = y;
            Options = options;
        }

        public readonly int Target;
        public readonly int Map;
        public readonly int X;
        public readonly int Y;
        public readonly int Options;
    }
}
