using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.IO;
using System.Linq;
using System.Text.Json;

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
    /// <summary>Whether the choice currently waiting on the host may be cancelled.</summary>
    bool ChoiceAllowsCancel => true;
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
    /// <summary>Clear session-scoped database overlays, vars and input policy.</summary>
    void ResetSessionState();
    void ReturnToTitle();
    bool IsWaitingForChoice();
    bool IsWaitingForMessage();
    string[] GetCurrentChoices();
    void SelectChoice(int index);
    void CancelChoice();
    void DismissMessage();
    /// <summary>True when the current 101 prompt is held for the following 102 choice.</summary>
    bool MessageKeptForChoice => false;
    /// <summary>Forget stale host-side dialogue state before replaying a loaded slot.</summary>
    void ResetDialogueState() { }
    void RestoreDialogue(string? message, string[] choices, bool allowCancel = true);
    DbValue? Database(DbOp op);
    /// <summary>Snapshot runtime op250 database overlays for SaveState persistence.</summary>
    SavedDbOverlays? SnapshotDbOverlays() => null;
    /// <summary>Clear and restore runtime op250 database overlays from a loaded SaveState.</summary>
    void RestoreDbOverlays(SavedDbOverlays? overlays) { }
    /// <summary>Read a database cell against base .dat tables plus an explicit save's overlay layer.</summary>
    DbValue? ReadDatabaseWithOverlay(DbOp op, SavedDbOverlays? overlays) => Database(op);
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
    /// <summary>Close transient front-end overlays after a successful load/rollback.</summary>
    void OnLoadCompleted() { HideMessage(); HideChoices(); }
}

/// <summary>
/// Core session orchestrating title, new game, map runtime, and save/load state.
/// Owns map state, the frame clock, the teleport queue, and the save format, talking
/// to the VM and presentation layers through <see cref="IVmSession"/> and <see cref="IGameSessionView"/>.
/// </summary>
public sealed class GameSession
{
    private const int SaveUiPictureFirst = 12000;
    private const int SaveUiPictureLast = 12599;

    private readonly IVmSession _vm;
    private readonly IGameSessionView _view;
    private readonly string _dataPath;
    private readonly string _savePath;
    private readonly Queue<PendingTeleport> _teleports = new();
    private AutoEvents? _autoEvents;
    private readonly BattleWiring _battleWiring = new();

    private MapDefinition? _map;
    private MapRuntime? _mapRuntime;
    private Program? _titleProgram;
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
        _actionPressed = false;
        // 220/221/222 must reach the session that owns the slot files.
        _vm.SaveLoadHandler = HandleSaveLoad;
        _vm.Vm.BlockOnKeyWait = true;
        _vm.Vm.RelativeCommonCalls = true;
    }

    private void ErasePictureSpan(int first, int last)
    {
        _vm.Vm.Host.Picture(new PictureCmd
        {
            Number = first,
            Action = PictureAction.Erase,
            HasSpan = true,
            Args = new[] { 0, 0, 0, last }
        });
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

    /// <summary>Non-fatal data loss reported by the last successful slot import; empty when nothing was dropped.</summary>
    public string LastImportNotice { get; private set; } = "";
    public bool HasChoices => _hasChoices;
    public bool ChoiceAllowsCancel { get; private set; } = true;
    public bool MessageOpen => _messageOpen;
    /// <summary>Number of conditional common events tracked by the auto-event scheduler.</summary>
    public int AutoEventCount => EnsureAutoEvents().EventCount;
    /// <summary>Fresh starts recorded for one auto event in this session (test hook).</summary>
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

    /// <summary>Stable foreground non-parallel event ID chain (excludes pc/wait ticks and parallel loops).</summary>
    public string GetForegroundStackSignature()
    {
        var ids = new List<string>();
        foreach (var inst in _vm.Vm.Instances)
        {
            if (!inst.IsParallel) ids.Add(inst.EventId.ToString());
        }
        return string.Join(",", ids);
    }
    /// <summary>Rebuilt CE242 drive (crates/wolf-game/src/battle_wiring.rs).</summary>
    public BattleWiring BattleWiring => _battleWiring;
    /// <summary>Direction/action bits currently denied by opcode 126.</summary>
    public uint MovementDeny => _vm.MovementDeny;
    public uint BasicAllDeny => _vm.BasicAllDeny;
    private bool _scriptedInput;
    public bool ScriptedInput
    {
        get => _scriptedInput;
        set
        {
            _scriptedInput = value;
            _vm.Vm.BlockOnKeyWait = !value;
            _vm.Vm.RelativeCommonCalls = !value;
        }
    }
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
        ChoiceAllowsCancel = true;
        _view.HideChoices();
        ClosePairedMessage();
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

        _titleProgram = title;
        _vm.StopAll();
        _vm.ResetSessionState();
        _vm.ResetDiagnostics();
        _vm.Vm.CommonSelf.Clear();
        ResetTransientSessionState();
        _map = null;
        _mapRuntime = null;
        _vm.Map = null;
        Frames = 0;
        _leaderX = 0;
        _leaderY = 0;
        _facing = 2;
        _view.ClearPictures();
        _vm.Vm.CreateInstance(title);
        _started = true;
        Phase = "title";
        _view.SetStatus("title: " + title.Title + $" ({title.Len} slots)");
        return true;
    }

    private int _titleMenuInstance = -1;

    /// <summary>
    /// CE43 paints the three title rows as pictures 3/4/5 and highlights the row Sys20 names
    /// (docs/engine/ENGINE_EVENTS.md). Run one parallel instance while the title choice is open.
    /// </summary>
    public bool SetTitleMenuVisible(bool visible)
    {
        // Drop every live CE43 before restarting so a title hand-off never leaves a
        // painter running in the map.
        StopTitleMenuInstances();
        if (!visible) return true;
        var inst = _vm.Vm.CreateInstance(WolfIds.TitleMenu);
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
                if (inst.EventId == WolfIds.TitleMenu) ids.Add(inst.Id);
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
        _vm.Vm.BlockOnKeyWait = !ScriptedInput;
        _vm.Vm.RelativeCommonCalls = !ScriptedInput;
        // Rust Game::frame syncs conditional common events before the VM step, so a
        // newly-triggered event starts executing in this same frame.
        var autoEvents = EnsureAutoEvents();
        autoEvents.LoopVisualPainters = !ScriptedInput;
        autoEvents.Sync(_vm.Vm);
        _battleWiring.Tick(_vm.Vm, op => _vm.Database(op), allowAutoCommands: ScriptedInput);
        bool battleBeforeStep = _vm.Vm.Vars.Get(WolfIds.VarBattleActive) != 0;
        _vm.StepFrame();
        if (!ScriptedInput && battleBeforeStep && _vm.Vm.Vars.Get(WolfIds.VarBattleActive) == 0)
        {
            BattleWiring.StopBattleInstances(_vm.Vm);
            ErasePictureSpan(2799, 4010);
            ErasePictureSpan(5000, 5020);
        }
        if (!_vm.Vm.ReturnToTitleRequested && !_vm.Vm.EndGameRequested)
        {
            ApplyPending();
            _vm.StartReservedEvent();
            // TitleMap.mps event 2 is an autorun (trigger=1) map event: in retail WOLF it restarts
            // from pc 0 once CE42 returns cleanly (e.g. canceling CE14/CE131 Save/Load UI) while still
            // on the title screen, keeping the title menu operable instead of deadlocking.
            if (Phase == "title" && _titleProgram != null
                && !_messageOpen && !_hasChoices && !_pendingLoad
                && _teleports.Count == 0 && _pendingGameLoad == null
                && !_vm.Vm.ForegroundBusy())
            {
                _vm.Vm.CreateInstance(_titleProgram);
            }
        }
        if (_hasChoices && !_vm.IsWaitingForChoice())
        {
            _hasChoices = false;
            ChoiceAllowsCancel = true;
            _view.HideChoices();
        }
        if (_messageOpen && !_vm.IsWaitingForMessage() && !_hasChoices)
        {
            NotifyMessageClosed();
        }
        UpdateMapHud(false);
        if (_vm.Vm.ReturnToTitleRequested)
        {
            _vm.Vm.ReturnToTitleRequested = false;
            _vm.ReturnToTitle();
        }
    }

    private AutoEvents EnsureAutoEvents()
    {
        _autoEvents ??= AutoEvents.FromPrograms(_vm.Vm.Programs.Values);
        return _autoEvents;
    }

    /// <summary>Map changes and loads are applied between frames, never inside a running coroutine.</summary>
    private void ApplyPending()
    {
        if (_pendingGameLoad != null)
        {
            _teleports.Clear();
            _pendingSystemLoads.Clear();
            _pendingLoad = false;
            string path = _pendingGameLoad;
            _pendingGameLoad = null;
            LoadGameFrom(path);
            return;
        }
        if (_pendingLoad)
        {
            _teleports.Clear();
            _pendingSystemLoads.Clear();
            _pendingLoad = false;
            LoadGame();
            return;
        }
        while (_teleports.Count > 0) ApplyTeleport(_teleports.Dequeue());
        while (_pendingSystemLoads.Count > 0) LoadSystemData(_pendingSystemLoads.Dequeue());
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
        ChoiceAllowsCancel = _vm.ChoiceAllowsCancel;
        _lastChoices = options;
        _view.ShowChoices(options, 0);
    }

    /// <summary>Last window contents; the title menu reopens on a load with no slot.</summary>
    private string[] _lastChoices = System.Array.Empty<string>();

    public void PickChoice(int index)
    {
        _hasChoices = false;
        ChoiceAllowsCancel = true;
        _view.HideChoices();
        ClosePairedMessage();
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
        ChoiceAllowsCancel = true;
        _view.HideChoices();
        ClosePairedMessage();
        _vm.SelectChoice(index);
    }

    private void ClosePairedMessage()
    {
        if (!_messageOpen) return;
        _messageOpen = false;
        _view.HideMessage();
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
        // event, which the leader-mode session cannot move.
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
        MapDefinition? resolvedTarget = ResolveMapDefinition(request.Map);
        int targetMapId = resolvedTarget?.Id ?? request.Map;
        if (_map != null && _map.Id == targetMapId && _leaderX == request.X && _leaderY == request.Y) return;

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

    [MemberNotNullWhen(true, nameof(_map), nameof(_mapRuntime))]
    private bool CanSaveMapState() =>
        _map != null && _mapRuntime != null
        && Phase is not ("title" or "boot" or "failed" or "newgame");

    /// <summary>Build the complete slot payload from the live map state.</summary>
    public SaveState? BuildSaveState()
    {
        if (!CanSaveMapState()) return null;
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
        var liveInstances = new Dictionary<int, Instance>();
        foreach (Instance inst in _vm.Vm.Instances) liveInstances[inst.Id] = inst;
        var persistedInstances = new List<Instance>();
        foreach (Instance inst in _vm.Vm.Instances)
        {
            if (!HasSaveUiAncestor(inst, liveInstances)) persistedInstances.Add(inst);
        }
        var persistedIds = persistedInstances.Select(i => i.Id).ToHashSet();
        foreach (Instance inst in persistedInstances)
        {
            MapEventState? owner = _mapRuntime.Events.FirstOrDefault(e => e.InstanceId == inst.Id);
            bool childDropped = inst.Wait.Kind == WaitStateKind.Child
                && !persistedIds.Contains(inst.Wait.ChildId);
            var saved = new SavedInstance
            {
                Id = inst.Id,
                ParentId = inst.ParentId,
                EventId = inst.EventId,
                Pc = inst.Pc,
                ReturnTo = inst.ReturnTo,
                Kind = owner != null ? "map" : "ce",
                Page = owner?.Active ?? -1,
                IsParallel = inst.IsParallel,
                WaitKind = childDropped ? (int)WaitStateKind.None : (int)inst.Wait.Kind,
                WaitFrames = childDropped ? 0 : inst.Wait.Frames,
                WaitBlock = childDropped ? 0 : inst.Wait.Block,
                WaitChild = childDropped ? 0 : inst.Wait.ChildId,
                WaitKeyTarget = childDropped || inst.Wait.Kind != WaitStateKind.KeyInput ? 0 : inst.Wait.KeyTarget,
                WaitKeyMask = childDropped || inst.Wait.Kind != WaitStateKind.KeyInput ? 0u : inst.Wait.KeyMask,
                Self = SavedSelf(inst.EventId, inst.Self)
            };
            foreach (LoopCtx loop in inst.Loops)
            {
                saved.Loops.Add(new SavedLoop { Block = loop.Block, Remaining = loop.Remaining });
            }
            state.Instances.Add(saved);
        }

        state.Pictures.AddRange(_view.SnapshotPictures().Where(p => !IsSaveUiPicture(p.Number)));
        state.Audio.AddRange(_view.SnapshotAudio());
        state.DbOverlays = _vm.SnapshotDbOverlays();

        bool hasMessageWait = persistedInstances.Any(i => i.Wait.Kind == WaitStateKind.Message);
        bool hasChoiceWait = persistedInstances.Any(i => i.Wait.Kind == WaitStateKind.Choice);
        if (hasMessageWait && _view.HasMessage)
        {
            state.Dialogue = new SavedDialogue { Message = _view.MessageText };
        }
        else if (hasChoiceWait && _vm.IsWaitingForChoice())
        {
            state.Dialogue = new SavedDialogue
            {
                Message = _messageOpen && _view.HasMessage ? _view.MessageText : "",
                Choices = _vm.GetCurrentChoices().ToList(),
                AllowCancel = ChoiceAllowsCancel
            };
        }

        return state;
    }

    // Exclude transient screen-overlay UI slots via the single SaveMigrator predicate.
    private static bool IsSaveUiPicture(int number) => SaveMigrator.IsTransientUiPicture(number);

    private static bool IsSaveUiEvent(int eventId) => eventId is 14 or 94 or 131;

    private static bool HasSaveUiAncestor(Instance inst, IReadOnlyDictionary<int, Instance> byId)
    {
        var seen = new HashSet<int>();
        Instance? current = inst;
        while (current != null && seen.Add(current.Id))
        {
            if (IsSaveUiEvent(current.EventId)) return true;
            current = current.ParentId is int parent && byId.TryGetValue(parent, out Instance? found)
                ? found
                : null;
        }
        return false;
    }

    private static bool HasSaveUiAncestor(SavedInstance inst, IReadOnlyDictionary<int, SavedInstance> byId)
    {
        var seen = new HashSet<int>();
        SavedInstance? current = inst;
        while (current != null && seen.Add(current.Id))
        {
            if (IsSaveUiEvent(current.EventId)) return true;
            current = current.ParentId is int parent && byId.TryGetValue(parent, out SavedInstance? found)
                ? found
                : null;
        }
        return false;
    }

    public bool SaveGame() => SaveGameTo(_savePath);

    /// <summary>Resolve the JSON file path for a 1-based save slot.</summary>
    public string SlotSavePath(int slot) => ResolveSaveTarget(new SaveTarget(Math.Max(1, slot)));

    /// <summary>Return true when the given 1-based save slot exists on disk.</summary>
    public bool SlotExists(int slot) => SaveCodec.Exists(SlotSavePath(slot));

    /// <summary>Load a 1-based slot directly from disk.</summary>
    public bool LoadGameFromSlot(int slot) => LoadGameFrom(SlotSavePath(slot));

    /// <summary>Write one full slot to an explicit file (opcode 220 with a numeric slot).</summary>
    public bool SaveGameTo(string path) => SaveGameTo(path, 1);

    /// <summary>Write one full slot to an explicit file and report the target slot number.</summary>
    public bool SaveGameTo(string path, int slot)
    {
        if (!CanSaveMapState())
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
        catch (Exception ex)
        {
            _view.SetStatus("save failed: " + ex.Message);
            return false;
        }

        _view.SetStatus($"saved slot {Math.Max(1, slot):00} (map {state.MapId} {state.LeaderX},{state.LeaderY})");
        UpdateMapHud(false);
        return true;
    }

    /// <summary>Save directly to a 1-based slot, sync System.json slot markers, and optionally redraw CE131.</summary>
    public bool SaveGameToSlot(int slot, bool refreshUi = true)
    {
        int clamped = Math.Max(1, slot);
        string path = SlotSavePath(clamped);
        if (!SaveGameTo(path, clamped)) return false;

        _vm.Vm.Vars.Set(546, clamped);
        string systemPath = ResolveSaveTarget(new SaveTarget("Save/System.sav"));
        try
        {
            SaveState systemState = SaveCodec.Read(systemPath) ?? new SaveState();
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 0, Value = clamped });
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 626, Value = _vm.Vm.Vars.Get(531) });
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 546, Value = clamped });
            SaveCodec.Write(systemPath, systemState);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException
                                   or ArgumentException or NotSupportedException or InvalidOperationException)
        {
            _view.SetStatus("system save failed: " + ex.Message);
        }

        if (refreshUi) RefreshSaveUiForSlot(clamped);
        return true;
    }

    /// <summary>Sync live CE131 selection args and invoke CE94 to repaint the save slot list.</summary>
    public void RefreshSaveUiForSlot(int slot)
    {
        int clamped = Math.Max(1, slot);
        Instance? ce131 = null;
        foreach (Instance inst in _vm.Vm.Instances)
        {
            if (inst.EventId == 131)
            {
                ce131 = inst;
                break;
            }
        }
        if (ce131 == null) return;

        int perPage = ce131.Self.Get(Magic.SelfArgBase + 27);
        if (perPage <= 0) perPage = 4;
        int row = (clamped - 1) % perPage;
        int page = (clamped - 1) / perPage;

        ce131.Self.Set(Magic.SelfArgBase + 10, row);
        ce131.Self.Set(Magic.SelfArgBase + 11, page);
        ce131.Self.Set(Magic.SelfArgBase + 12, clamped);

        for (int i = _vm.Vm.Instances.Count - 1; i >= 0; i--)
        {
            if (_vm.Vm.Instances[i].EventId == 94)
            {
                _vm.Vm.StopInstance(_vm.Vm.Instances[i].Id);
            }
        }
        int mode = ce131.Self.Get(Magic.SelfArgBase + 0) != 0 ? 1 : 0;
        _vm.Vm.StartEventWithArgs(94, new[] { mode, row, page }, Array.Empty<string>(), parallel: true);
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

        // Resolve the destination before touching the live VM, so a stale slot
        // cannot strand the player outside any map.
        if (!CanEnterMap(state.MapId))
        {
            _view.SetStatus($"load failed: map {state.MapId} missing from programs.json");
            return false;
        }

        SaveState? rollback = CanSaveMapState() && !_vm.Faulted ? BuildSaveState() : null;
        if (!ApplyLoadedState(state))
        {
            if (rollback != null && RestoreAfterFailedLoad(rollback))
            {
                _view.RestorePictures(rollback.Pictures.Where(p => !IsSaveUiPicture(p.Number)).ToList());
                _view.RestoreAudio(rollback.Audio);
                _view.OnLoadCompleted();
                RestoreLoadedDialogue(rollback);
                _view.SetStatus($"load failed: kept map {rollback.MapId} at {rollback.LeaderX},{rollback.LeaderY}");
            }
            return false;
        }

        LoadedFrom = read.Source;
        // EnterMap -> ShowMap clears the layer, so the saved slots are replayed after it.
        _view.RestorePictures(state.Pictures.Where(p => !IsSaveUiPicture(p.Number)).ToList());
        // Audio last: the map load above may start its own track, and the slot wins.
        _view.RestoreAudio(state.Audio);
        _view.OnLoadCompleted();
        RestoreLoadedDialogue(state);
        _view.SetStatus($"loaded slot 01 from {SaveCodec.SourceName(read.Source)} (map {state.MapId} {state.LeaderX},{state.LeaderY})");
        UpdateMapHud(false);
        return true;
    }

    private void RestoreLoadedDialogue(SaveState state)
    {
        if (state.Dialogue == null) return;
        bool messageWait = _vm.Vm.Instances.Any(i => i.Wait.Kind == WaitStateKind.Message);
        bool choiceWait = _vm.Vm.Instances.Any(i => i.Wait.Kind == WaitStateKind.Choice);
        if (messageWait && !string.IsNullOrEmpty(state.Dialogue.Message))
        {
            _vm.RestoreDialogue(state.Dialogue.Message, Array.Empty<string>());
        }
        else if (choiceWait && state.Dialogue.Choices.Count > 0)
        {
            _vm.RestoreDialogue(state.Dialogue.Message, state.Dialogue.Choices.ToArray(),
                state.Dialogue.AllowCancel);
        }
        else if (choiceWait)
        {
            // A legacy slot can carry a choice wait without labels; resolve it rather
            // than leaving an invisible modal gate forever.
            _vm.CancelChoice();
        }
    }

    // ---- script save/load (opcodes 220/221/222) -------------------------

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
            string name = Path.GetFileName(file.Path.Replace('\\', '/'));
            if (name.Length == 0) name = "System.sav";
            string targetName = Path.ChangeExtension(name, ".json");
            string candidate = Path.Combine(dir, targetName);
            if (!File.Exists(candidate) && Directory.Exists(dir))
            {
                // Retail scripts mix "save/system.sav" (TitleMap) and "Save/System.sav" (CE131);
                // share one backing file on case-sensitive filesystems without breaking callers
                // that inspect System.json on creation.
                foreach (string existing in Directory.EnumerateFiles(dir, "*.json"))
                {
                    if (string.Equals(Path.GetFileName(existing), targetName, StringComparison.OrdinalIgnoreCase))
                        return existing;
                }
                string bakName = targetName + ".bak";
                foreach (string existingBak in Directory.EnumerateFiles(dir, "*.json.bak"))
                {
                    if (string.Equals(Path.GetFileName(existingBak), bakName, StringComparison.OrdinalIgnoreCase))
                        return existingBak[..^4];
                }
            }
            return candidate;
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
                    int slot = Math.Max(1, op.File.Slot);
                    bool ok = SaveGameTo(path, slot);
                    if (op.Owner == 435)
                        _view.SetStatus($"[AutoSave] slot={slot} {(ok ? "ok" : "fail")}");
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
                if (WriteSaveAddress(state, op))
                {
                    try
                    {
                        SaveCodec.Write(path, state);
                    }
                    catch (Exception ex)
                    {
                        _view.SetStatus("save write failed: " + ex.Message);
                    }
                }
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
        catch (Exception ex)
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

    /// <summary>Read a 221 address: CData cells read target save's overlay; variable banks decode via Magic.</summary>
    private int ReadSaveAddress(SaveState state, SaveOp op, string path)
    {
        int address = op.Address;
        if (address is >= 1_100_000_000 and < 1_200_000_000)
        {
            int offset = address - 1_100_000_000;
            int dbType = offset / 1_000_000;
            int rem = offset % 1_000_000;
            int row = rem / 100;
            int col = rem % 100;
            DbValue? cell = _vm.ReadDatabaseWithOverlay(new DbOp
            {
                Container = DbContainer.CData,
                Direction = DbDirection.Read,
                DbType = dbType,
                DataId = row,
                ItemId = col,
                Destination = op.Destination
            }, state.DbOverlays);
            if (cell == null) return 0;
            if (cell.IsString)
            {
                op.LoadedString = cell.Str;
                return 0;
            }
            return cell.Num;
        }

        var magic = Magic.Decode(address);
        switch (magic.Kind)
        {
            case MagicKind.Var:
            {
                int index = (int)magic.Index;
                return index >= 0 && index < state.Vars.Count ? state.Vars[index] : 0;
            }
            case MagicKind.StrVar:
            {
                int index = (int)magic.Index;
                op.LoadedString = index >= 0 && index < state.StrVars.Count ? state.StrVars[index] : "";
                return 0;
            }
            case MagicKind.SelfVar:
            {
                EventSelfVars? bank = FindSelfBank(state, op.Owner);
                int index = (int)magic.Index;
                if (magic.IsStringSlot)
                {
                    if (bank == null)
                    {
                        op.LoadedString = "";
                        return 0;
                    }
                    if (bank.StrMap != null && bank.StrMap.TryGetValue(index, out string? sVal))
                    {
                        op.LoadedString = sVal ?? "";
                        return 0;
                    }
                    op.LoadedString = index >= 0 && index < bank.Strings.Length ? (bank.Strings[index] ?? "") : "";
                    return 0;
                }
                if (bank == null) return 0;
                if (bank.NumMap != null && bank.NumMap.TryGetValue(index, out int value)) return value;
                return index >= 0 && index < bank.Nums.Length ? bank.Nums[index] : 0;
            }
            case MagicKind.SysVar:
            {
                if (magic.IsSysStr)
                {
                    int strIdx = (int)(magic.Index - Magic.SysStrOffset);
                    op.LoadedString = strIdx >= 0 && strIdx < state.SysStrVars.Count ? state.SysStrVars[strIdx] : "";
                    return 0;
                }
                int index = (int)magic.Index;
                if (index == 24) return SaveCodec.Exists(path) ? 1 : 0;
                if (index == 29) return (int)(state.PlayFrames / 60);
                return index >= 0 && index < state.SysVars.Count ? state.SysVars[index] : 0;
            }
            default:
                return 0;
        }
    }

    /// <summary>Write one 222 value; false means the address is outside every bank.</summary>
    private static bool WriteSaveAddress(SaveState state, SaveOp op)
    {
        var magic = Magic.Decode(op.Address);
        switch (magic.Kind)
        {
            case MagicKind.Var:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.Vars.Count <= index) state.Vars.Add(0);
                state.Vars[index] = op.Value;
                return true;
            }
            case MagicKind.StrVar:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.StrVars.Count <= index) state.StrVars.Add("");
                state.StrVars[index] = op.StringValue ?? "";
                return true;
            }
            case MagicKind.SelfVar:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= SelfVars.TotalSlots) return false;
                EventSelfVars? bank = FindSelfBank(state, op.Owner);
                if (bank == null)
                {
                    bank = new EventSelfVars { Id = op.Owner };
                    state.SelfVars.Add(bank);
                }
                if (magic.IsStringSlot || op.IsString)
                {
                    bank.StrMap ??= new Dictionary<int, string>();
                    bank.StrMap[index] = op.StringValue ?? "";
                }
                else
                {
                    bank.NumMap ??= new Dictionary<int, int>();
                    bank.NumMap[index] = op.Value;
                }
                return true;
            }
            case MagicKind.SysVar:
            {
                if (magic.IsSysStr)
                {
                    int strIdx = (int)(magic.Index - Magic.SysStrOffset);
                    if (strIdx < 0 || strIdx >= Variables.MaxSlots) return false;
                    while (state.SysStrVars.Count <= strIdx) state.SysStrVars.Add("");
                    state.SysStrVars[strIdx] = op.StringValue ?? "";
                    return true;
                }
                int index = (int)magic.Index;
                if (index == 29)
                {
                    state.PlayFrames = Math.Max(0, op.Value) * 60L;
                    return true;
                }
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.SysVars.Count <= index) state.SysVars.Add(0);
                state.SysVars[index] = op.Value;
                return true;
            }
            default:
                return false;
        }
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
        _vm.StopAll();
        ResetTransientSessionState();
        _view.ClearPictures();
        _vm.Map = null;
        _mapRuntime = null;
        _map = null;
        Frames = (int)state.PlayFrames;
        _leaderX = state.LeaderX;
        _leaderY = state.LeaderY;
        _facing = state.Facing;
        _vm.Vm.Vars.Restore(state.Vars, state.StrVars, state.SysVars, state.SysStrVars);
        _vm.Vm.Rng.RestoreState(state.RngState, state.RngEventState, state.RngEventSeed, state.RngDraws);
        _vm.RestoreDbOverlays(state.DbOverlays);
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

    /// <summary>Resolve a SystemDB map row or artifact id into its MapDefinition.</summary>
    private MapDefinition? ResolveMapDefinition(int mapRowId)
    {
        string mapPath = _vm.MapPath(mapRowId);
        return mapPath.Length > 0
            ? ProgramLoader.LoadMap(_dataPath, mapPath, null)
            : ProgramLoader.LoadMap(_dataPath, "", mapRowId);
    }

    /// <summary>True when programs.json resolves the slot's map exactly like EnterMap.</summary>
    private bool CanEnterMap(int mapId) => ResolveMapDefinition(mapId) != null;

    /// <summary>Validate remake save JSON against the SaveCodec schema and semantic constraints.</summary>
    public static bool ValidateSaveJson(string json, out string error)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            error = "save JSON is empty";
            return false;
        }
        try
        {
            if (new System.Text.UTF8Encoding(false, true).GetByteCount(json) > SaveCodec.MaxSaveBytes)
            {
                error = $"save JSON exceeds {SaveCodec.MaxSaveBytes} bytes";
                return false;
            }
            using JsonDocument doc = JsonDocument.Parse(json);
            if (doc.RootElement.ValueKind != JsonValueKind.Object)
            {
                error = "save JSON root must be an object";
                return false;
            }
            if (!doc.RootElement.TryGetProperty("schema", out JsonElement schemaProp)
                || schemaProp.ValueKind != JsonValueKind.String
                || schemaProp.GetString() != SaveState.SchemaName)
            {
                error = $"missing or invalid save schema (expected {SaveState.SchemaName})";
                return false;
            }
            if (!doc.RootElement.TryGetProperty("schema_version", out JsonElement verProp)
                || verProp.ValueKind != JsonValueKind.Number)
            {
                error = "missing or invalid save schema_version";
                return false;
            }
        }
        catch (Exception ex) when (ex is JsonException or ArgumentException or System.Text.EncoderFallbackException)
        {
            error = "malformed save JSON: " + ex.Message;
            return false;
        }
        if (SaveCodec.FromJson(json) == null)
        {
            error = "save JSON failed schema or semantic validation";
            return false;
        }
        error = "";
        return true;
    }

    /// <summary>
    /// Import a retail <c>.sav</c> file into a remake slot file (<c>save_NN.json</c>) without
    /// mutating the active live session.
    /// </summary>
    public bool ImportRetailSaveToSlot(string savPath, int slot, out string error)
    {
        LastImportNotice = "";
        if (slot < 1 || slot > 99)
        {
            error = $"save slot {slot} out of range (must be 1..99)";
            return false;
        }
        if (string.IsNullOrWhiteSpace(savPath) || !File.Exists(savPath))
        {
            error = $"retail save file not found: {savPath}";
            return false;
        }

        LegacySaveImporter.SlotResult imported;
        try
        {
            imported = LegacySaveImporter.ImportFile(savPath, Math.Max(0, slot - 1), rejectBadChecksum: true);
        }
        catch (Exception ex)
        {
            error = "retail import failed: " + ex.Message;
            return false;
        }

        if (!imported.ChecksumOk)
        {
            error = "retail import failed: checksum mismatch";
            return false;
        }

        LegacySaveImporter.RecoveredData data = imported.Recovered;
        var numbers = new List<int>(Enumerable.Repeat(0, WolfIds.VarPollMax));
        foreach (KeyValuePair<string, int> pair in data.Variables)
        {
            if (int.TryParse(pair.Key, out int index) && index >= 0 && index < numbers.Count)
            {
                numbers[index] = pair.Value;
            }
        }

        var sysVars = new List<int>();
        if (data.ChoiceAnchor is { Length: >= 2 })
        {
            int needed = Math.Max(WolfIds.SysChoiceAnchorX, WolfIds.SysChoiceAnchorY) + 1;
            while (sysVars.Count < needed) sysVars.Add(0);
            sysVars[WolfIds.SysChoiceAnchorX] = data.ChoiceAnchor[0];
            sysVars[WolfIds.SysChoiceAnchorY] = data.ChoiceAnchor[1];
        }

        string mapPath = _vm.MapPath(1);
        MapDefinition? map1 = mapPath.Length > 0
            ? ProgramLoader.LoadMap(_dataPath, mapPath, null)
            : ProgramLoader.LoadMap(_dataPath, "SampleMapA.mps", 1005);

        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = data.PlayTime.Frames,
            MapId = map1?.Id ?? 1005,
            MapFile = map1?.File ?? "SampleMapA.mps",
            LeaderX = 0,
            LeaderY = 0,
            Facing = 2,
            Vars = numbers,
            SysVars = sysVars,
        };

        foreach (KeyValuePair<int, SelfVars> bank in BuildCommonSelfFromRetail(data))
        {
            state.CommonSelf.Add(SavedSelf(bank.Key, bank.Value));
        }

        // Prevent new-game setup event 0 from resetting the imported state on load.
        state.MapEvents.Add(new SavedMapEvent
        {
            Id = 0,
            Active = null,
            InstanceId = null,
            Runs = false,
            Starts = 1,
            StartsOnPage = 1,
        });

        var picByNum = new Dictionary<int, SavedPicture>();
        foreach (SavedPicture pic in data.PictureStates)
        {
            if (!IsSaveUiPicture(pic.Number))
            {
                picByNum[pic.Number] = pic;
            }
        }
        state.Pictures.AddRange(picByNum.Values.Take(SaveCodec.MaxPictures));

        if (data.DatabaseCells.Count > 0)
        {
            var overlays = new SavedDbOverlays();
            foreach (var cell in data.DatabaseCells)
            {
                if (cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
                overlays.Nums.Add(new SavedDbNumCell
                {
                    Container = (int)DbContainer.CData,
                    Type = cell.Type,
                    Row = cell.Row,
                    Column = cell.Column,
                    Value = cell.Value
                });
            }
            state.DbOverlays = overlays;
        }

        if (!ValidateSaveJson(SaveCodec.ToJson(state), out string validationError))
        {
            error = "converted save failed validation: " + validationError;
            return false;
        }

        string dir = Path.GetDirectoryName(_savePath) ?? ".";
        int normalizedSlot = slot <= 1 ? 1 : slot;
        string targetPath = Path.Combine(dir, $"save_{normalizedSlot:00}.json");
        try
        {
            SaveCodec.Write(targetPath, state);
        }
        catch (Exception ex)
        {
            error = "save write failed: " + ex.Message;
            return false;
        }

        error = "";
        return true;
    }

    private static Dictionary<int, SelfVars> BuildCommonSelfFromRetail(LegacySaveImporter.RecoveredData data)
    {
        var result = new Dictionary<int, SelfVars>();
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
            if (any) result[pair.Key] = bank;
        }
        foreach (KeyValuePair<int, string[]> pair in data.CommonSelfStrings)
        {
            SelfVars? bank = result.GetValueOrDefault(pair.Key);
            for (int i = 0; i < pair.Value.Length; i++)
            {
                if (string.IsNullOrEmpty(pair.Value[i])) continue;
                if (bank == null)
                {
                    bank = new SelfVars();
                    result[pair.Key] = bank;
                }
                bank.SetStr(605 + i, pair.Value[i]);
            }
        }
        return result;
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
        _vm.StopAll();
        _vm.ResetSessionState();
        ResetTransientSessionState();
        _vm.Map = null;
        _mapRuntime = null;
        _map = null;

        Frames = (int)data.PlayTime.Frames;
        _leaderX = 0;
        _leaderY = 0;
        _facing = 2;

        var numbers = new List<int>(Enumerable.Repeat(0, WolfIds.VarPollMax));
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
            _vm.Vm.Vars.SetSys(WolfIds.SysChoiceAnchorX, data.ChoiceAnchor[0]);
            _vm.Vm.Vars.SetSys(WolfIds.SysChoiceAnchorY, data.ChoiceAnchor[1]);
        }

        _vm.Vm.CommonSelf.Clear();
        foreach (KeyValuePair<int, SelfVars> bank in BuildCommonSelfFromRetail(data))
        {
            _vm.Vm.CommonSelf[bank.Key] = bank.Value;
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
        _view.SetStatus($"loaded retail save {slot.File} (map 1 day {_vm.Vm.Vars.Get(WolfIds.VarDay)})");
        UpdateMapHud(false);
        return true;
    }

    /// <summary>Drop queued loads/teleports, dialogue flags and front-end popups shared by every session restart.</summary>
    private void ResetTransientSessionState()
    {
        _vm.Vm.ResetInputPermissions();
        _vm.ResetDialogueState();
        _battleWiring.Reset();
        _messageOpen = false;
        _hasChoices = false;
        ChoiceAllowsCancel = true;
        _actionPressed = false;
        _pendingLoad = false;
        _pendingGameLoad = null;
        _pendingSystemLoads.Clear();
        _teleports.Clear();
        _view.HideMessage();
        _view.HideChoices();
    }

    private void StartNewGameNow()
    {
        Phase = "newgame";
        _vm.StopAll();
        // TitleMap initializes system paths and volume variables before teleporting to map 3;
        // preserve Vm.Vars across ResetSessionState while clearing op250 DB overlays.
        List<int> savedNums = _vm.Vm.Vars.SnapshotNums();
        List<string> savedStrings = _vm.Vm.Vars.SnapshotStrings();
        List<int> savedSysNums = _vm.Vm.Vars.SnapshotSysNums();
        List<string> savedSysStrings = _vm.Vm.Vars.SnapshotSysStrings();
        _vm.ResetSessionState();
        _vm.Vm.Vars.Restore(savedNums, savedStrings, savedSysNums, savedSysStrings);
        ResetTransientSessionState();
        // AutoEvents is session-scoped; StopAll does not clear its edge latch. Re-arming
        // here would replay event 38 and shift instance ids. A fresh engine boot
        // constructs a fresh scheduler instead.
        _vm.ResetDiagnostics();
        _vm.Vm.CommonSelf.Clear();
        _vm.Map = null;
        _mapRuntime = null;
        _map = null;
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
        var savedById = new Dictionary<int, SavedInstance>();
        foreach (SavedInstance saved in state.Instances) savedById[saved.Id] = saved;
        foreach (SavedInstance saved in state.Instances)
        {
            if (HasSaveUiAncestor(saved, savedById)) continue;
            bool isParallel = false;
            bool retainAfterFinish = false;
            Program? program = null;
            if (saved.Kind == "map" && saved.Page >= 0)
            {
                MapEventDef? def = _map?.Events.FirstOrDefault(e => e.Id == saved.EventId);
                if (def != null && saved.Page < def.Pages.Count)
                {
                    program = def.Pages[saved.Page].Program;
                    isParallel = saved.IsParallel ?? (def.Pages[saved.Page].Trigger == MapTrigger.Parallel);
                    retainAfterFinish = true;
                }
            }
            else
            {
                program = _vm.Vm.GetProgram(saved.EventId);
                if (program != null)
                {
                    isParallel = saved.IsParallel ?? InferLegacyParallel(saved, program, savedById);
                }
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
                ReturnTo = saved.ReturnTo,
                IsParallel = isParallel,
                RetainAfterFinish = retainAfterFinish
            };
            inst.Wait.Kind = (WaitStateKind)saved.WaitKind;
            inst.Wait.Frames = saved.WaitFrames;
            inst.Wait.Block = saved.WaitBlock;
            inst.Wait.ChildId = saved.WaitChild;
            if (inst.Wait.Kind == WaitStateKind.KeyInput)
            {
                if (saved.WaitKeyMask != 0)
                {
                    inst.Wait.WaitForKeyInput(saved.WaitKeyTarget, saved.WaitKeyMask, requireRelease: false);
                }
                else
                {
                    inst.Wait.Clear();
                    if (inst.Pc > 0) inst.Pc--;
                }
            }
            inst.Self.Restore(saved.Self);
            foreach (SavedLoop loop in saved.Loops)
            {
                inst.Loops.Add(new LoopCtx { Block = loop.Block, Remaining = loop.Remaining });
            }
            _vm.Vm.RestoreInstance(inst);
        }
        ClearMissingChildWaits();
    }

    private bool InferLegacyParallel(SavedInstance saved, Program program, IReadOnlyDictionary<int, SavedInstance> byId)
    {
        if (program.RunCondition == 2 || Array.IndexOf(AutoEvents.GatedCallOnly, program.Id) >= 0) return true;
        var seen = new HashSet<int> { saved.Id };
        int? parentId = saved.ParentId;
        while (parentId is int pid && seen.Add(pid) && byId.TryGetValue(pid, out SavedInstance? parent))
        {
            if (parent.IsParallel == true) return true;
            if (parent.Kind == "map" && parent.Page >= 0)
            {
                MapEventDef? def = _map?.Events.FirstOrDefault(e => e.Id == parent.EventId);
                if (def != null && parent.Page < def.Pages.Count && def.Pages[parent.Page].Trigger == MapTrigger.Parallel)
                    return true;
            }
            else
            {
                Program? parentProg = _vm.Vm.GetProgram(parent.EventId);
                if (parentProg is { RunCondition: 2 }
                    || (parentProg != null && Array.IndexOf(AutoEvents.GatedCallOnly, parentProg.Id) >= 0))
                {
                    return true;
                }
            }
            parentId = parent.ParentId;
        }
        return false;
    }

    /// <summary>Save-UI children are intentionally omitted; release any caller left waiting on them.</summary>
    private void ClearMissingChildWaits()
    {
        var liveIds = _vm.Vm.Instances.Select(i => i.Id).ToHashSet();
        foreach (Instance inst in _vm.Vm.Instances)
        {
            if (inst.Wait.Kind != WaitStateKind.Child || liveIds.Contains(inst.Wait.ChildId)) continue;
            inst.Wait.Clear();
            inst.JustResumedFromChild = true;
        }
    }

    private void EnterMap(int mapId, Dictionary<int, SelfVars>? selfVars = null, SaveState? restore = null,
        int? skipEventId = null)
    {
        _vm.StopAll();
        _vm.ResetDialogueState();
        _messageOpen = false;
        _hasChoices = false;
        ChoiceAllowsCancel = true;
        _view.HideMessage();
        _view.HideChoices();
        _vm.Map = null;
        _mapRuntime = null;

        // Opcode 130 carries a SystemDB row, not the artifact map id: row 1 is
        // SampleMapA, while programs.json's id 1 is SampleMapA_1. Resolve the file first.
        _map = ResolveMapDefinition(mapId);
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
