using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Presentation;

/// <summary>
/// VmBridge (interface frozen in M2 W1): the only boundary between the deterministic
/// C# core and the presentation layer.
/// Emission: PictureRequested, SoundRequested, MessageRequested, MessageClosed,
/// ChoicesRequested, ChoiceSelected, TeleportRequested, TransitionRequested,
/// CommonEventCalled, EffectRequested, EventFinished.
/// Queries/commands: GetVar/SetVar, GetStrVar/SetStrVar, IsWaitingForChoice,
/// IsWaitingForMessage, IsRunning, GetCurrentChoices, SelectChoice, DismissMessage,
/// StepFrame/StepFrames.
/// Timing: GameRoot/GameSession owns the fixed 1/60s clock; VmBridge never adds a second one.
/// </summary>
[GlobalClass]
public partial class VmController : Node, IHost, IVmSession
{
    [Signal]
    public delegate void PictureRequestedEventHandler(int number, int action, string file, string text, int[] args);

    [Signal]
    public delegate void SoundRequestedEventHandler(int channel, int action, string file, int volume, int frames);

    [Signal]
    public delegate void MessageRequestedEventHandler(string text, int face);

    [Signal]
    public delegate void MessageClosedEventHandler();

    [Signal]
    public delegate void ChoicesRequestedEventHandler(string[] options);

    [Signal]
    public delegate void ChoiceSelectedEventHandler(int index);

    [Signal]
    public delegate void TeleportRequestedEventHandler(int target, int map, int x, int y, int options);

    [Signal]
    public delegate void TransitionRequestedEventHandler(int kind, int flags);

    [Signal]
    public delegate void CommonEventCalledEventHandler(int target, int[] args);

    [Signal]
    public delegate void EffectRequestedEventHandler(int kind, int duration, int[] args);

    [Signal]
    public delegate void EventFinishedEventHandler(int eventId);

    public EventVm Vm { get; } = new();

    /// <summary>Direction bits opcode 126 currently denies for basic movement.</summary>
    public uint MovementDeny => Vm.MovementDeny;

    /// <summary>Key bits opcode 126 currently denies for all basic input (map action/choice).</summary>
    public uint BasicAllDeny => Vm.BasicAllDeny;

    /// <summary>Extra fields of the last 140 command; the frozen signal keeps its 5 args.</summary>
    public int SoundLoopPoint => _lastSoundLoopPoint;
    public int SoundTempo => _lastSoundTempo;

    private ChoiceReply? _pendingChoiceReply;
    private bool _messageDismissed = true;
    private bool _isWaitingChoice;
    private bool _isWaitingMessage;
    private string[] _currentChoices = Array.Empty<string>();
    private int _lastSoundLoopPoint;
    private int _lastSoundTempo = 100;

    /// <summary>Anchor and kind of the last 150 command (the frozen signal omits both).</summary>
    public int LastPictureAnchor => _lastPictureAnchor;
    public int LastPictureKind => _lastPictureKind;

    private int _lastPictureAnchor;
    private int _lastPictureKind;
    private readonly Dictionary<int, PictureCmd> _pictures = new();

    /// <summary>Pointer in the 1024x576 logical space; the VM never guesses it.</summary>
    public float PointerX { get; private set; }
    public float PointerY { get; private set; }
    private bool _pointerPressed;
    private bool _pointerPressPending;
    private bool _pointerPressFrame;
    private bool _rightPressed;
    private bool _rightPressPending;
    private bool _rightPressFrame;
    private bool _middlePressed;
    private bool _middlePressPending;
    private bool _middlePressFrame;
    private GodotObject? _pictureStateTarget;
    private StringName _pictureStateMethod = "";
    private AutoEvents? _autoEvents;

    /// <summary>Opcode 123 input source; the screen supplies it, the VM never guesses.</summary>
    public Func<uint, int>? KeyCodeProvider { get; set; }

    /// <summary>Opcode 250 store loaded from game_data/database.json.</summary>
    public GameDatabase? DatabaseStore { get; private set; }

    public override void _Ready()
    {
        Vm.Host = this;
    }

    public bool LoadPrograms(string? path = null)
    {
        if (string.IsNullOrEmpty(path))
        {
            string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
            path = Path.GetFullPath(Path.Combine(rootDir, "..", "game_data", "programs.json"));
        }

        if (!File.Exists(path))
        {
            GD.PrintErr($"[VmController] programs.json not found at: {path}");
            return false;
        }

        var programs = ProgramLoader.LoadCommonEvents(path);
        foreach (var prog in programs.Values)
        {
            Vm.RegisterProgram(prog);
        }
        GD.Print($"[VmController] Loaded {programs.Count} common event programs.");

        string dataDir = Path.GetDirectoryName(Path.GetFullPath(path)) ?? ".";
        string projectRoot = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string basicDir = Path.GetFullPath(Path.Combine(projectRoot, "..", "work", "all", "BasicData"));
        string databasePath = Path.Combine(dataDir, "database.json");
        DatabaseStore = GameDatabase.Load(databasePath, basicDir);
        if (DatabaseStore == null)
            GD.PrintErr($"[VmController] database.json not found at: {databasePath}");
        else
            GD.Print($"[VmController] Loaded database from {DatabaseStore.SourcePath}.");
        return true;
    }

    public Instance? StartCommonEvent(int eventId)
    {
        var inst = Vm.CreateInstance(eventId);
        if (inst == null)
        {
            GD.PrintErr($"[VmController] Common Event {eventId} could not start");
            return null;
        }
        GD.Print($"[VmController] Started Common Event {eventId} (Instance {inst.Id})");
        return inst;
    }

    /// <summary>
    /// Start a common event at a specific slot. Used by focused UI regressions that need the
    /// real event body after a long story prologue (for example CE251's menu section), without
    /// inventing a parallel event. The program/control map is still the retail one.
    /// </summary>
    public bool StartEventAt(int eventId, int pc)
    {
        var inst = Vm.CreateInstance(eventId);
        if (inst == null)
        {
            GD.PrintErr($"[VmController] Common Event {eventId} could not start at pc {pc}");
            return false;
        }
        if (pc < 0 || pc >= inst.Program.Len)
        {
            Vm.StopInstance(inst.Id);
            GD.PrintErr($"[VmController] Common Event {eventId} pc {pc} out of range (len {inst.Program.Len})");
            return false;
        }
        inst.Pc = pc;
        inst.Wait.Clear();
        return true;
    }

    /// <summary>Active map runtime: pages start on their own each frame (map_runtime.rs start_pending).</summary>
    public MapRuntime? Map { get; set; }

    /// <summary>One-shot action-key press for the current frame (map_runtime.rs try_trigger).</summary>
    public bool ActionKeyRequested { get; set; }
    public int LeaderX { get; set; }
    public int LeaderY { get; set; }
    public int LeaderFacing { get; set; } = 2;
    public int TriggeredPages { get; private set; }

    public void StepFrame()
    {
        if (Map != null && ActionKeyRequested)
        {
            var candidates = Map.TriggerCandidates(true, false, LeaderX, LeaderY, LeaderFacing);
            foreach (var candidate in candidates)
            {
                if (Map.StartTriggerPage(candidate.State.Def.Id, candidate.Page, Vm))
                {
                    TriggeredPages++;
                    break;
                }
            }
        }
        ActionKeyRequested = false;

        Vm.StepFrame();
        Map?.StartPending(Vm);
    }

    public void StepFrames(int count)
    {
        for (int i = 0; i < count; i++)
        {
            StepFrame();
        }
    }

    public void SelectChoice(int index)
    {
        _pendingChoiceReply = ChoiceReply.Select(index);
        _isWaitingChoice = false;
        EmitSignal(SignalName.ChoiceSelected, index);
    }

    public void CancelChoice()
    {
        _pendingChoiceReply = ChoiceReply.Cancel;
        _isWaitingChoice = false;
    }

    public void DismissMessage()
    {
        _messageDismissed = true;
        _isWaitingMessage = false;
    }

    /// <summary>Reopen the saved dialogue window without replaying the opcode that produced it.</summary>
    public void RestoreDialogue(string? message, string[] choices)
    {
        _pendingChoiceReply = null;
        if (choices.Length > 0)
        {
            _currentChoices = choices;
            _isWaitingChoice = true;
            _isWaitingMessage = false;
            EmitSignal(SignalName.ChoicesRequested, choices);
        }
        else if (!string.IsNullOrEmpty(message))
        {
            _messageDismissed = false;
            _isWaitingMessage = true;
            _isWaitingChoice = false;
            EmitSignal(SignalName.MessageRequested, message, -1);
        }
    }

    public int GetVar(int varId) => Vm.Vars.Get(varId);
    public void SetVar(int varId, int value) => Vm.Vars.Set(varId, value);

    public int GetSys(int sysId) => Vm.Vars.GetSys(sysId);
    public void SetSys(int sysId, int value) => Vm.Vars.SetSys(sysId, value);
    public string GetStrVar(int varId) => Vm.Vars.GetStr(varId);
    public void SetStrVar(int varId, string value) => Vm.Vars.SetStr(varId, value);

    public bool IsWaitingForChoice() => _isWaitingChoice;
    public bool IsWaitingForMessage() => _isWaitingMessage;
    public bool IsRunning() => Vm.Instances.Count > 0;

    /// <summary>Start the run-condition-2 helpers once, like Game::new_game (H scene fixtures).</summary>
    public void PrimeAutoEvents()
    {
        _autoEvents ??= AutoEvents.FromPrograms(Vm.Programs.Values);
        _autoEvents.PrimeLatched(Vm);
    }

    /// <summary>Advance the auto-event latch once per frame; no-op until PrimeAutoEvents runs.</summary>
    public void SyncAutoEvents()
    {
        _autoEvents?.Sync(Vm);
    }

    /// <summary>True while an instance of the event is live (H scene spawn probes).</summary>
    public bool HasEvent(int eventId)
    {
        foreach (var inst in Vm.Instances)
        {
            if (inst.EventId == eventId)
            {
                return true;
            }
        }

        return false;
    }

    public string[] GetCurrentChoices() => _currentChoices;

    /// <summary>True once the core met an unhandled opcode. Presentation must stop the
    /// clock and must never overwrite a save slot while this holds (DECISIONS D-13).</summary>
    public bool StrictMode
    {
        get => Vm.StrictMode;
        set => Vm.StrictMode = value;
    }

    public bool Faulted => Vm.Faulted;

    public string FaultReason => Vm.FaultReason ?? "";

    // --- IHost Implementation ---

    public void Wait(uint frames)
    {
    }

    public void ShowMessage(string text, int? face)
    {
        _messageDismissed = false;
        _isWaitingMessage = true;
        EmitSignal(SignalName.MessageRequested, text, face ?? -1);
    }

    public bool MessageDismissed() => _messageDismissed;

    public void CloseMessage()
    {
        EmitSignal(SignalName.MessageClosed);
    }

    public void ShowChoices(string[] options)
    {
        _currentChoices = options;
        _isWaitingChoice = true;
        _pendingChoiceReply = null;
        EmitSignal(SignalName.ChoicesRequested, options);
    }

    public ChoiceReply? ChoiceMade()
    {
        var reply = _pendingChoiceReply;
        if (reply != null)
        {
            _pendingChoiceReply = null;
            return reply;
        }
        return null;
    }

    public void CommonEventCall(int target, int[] args, string[] strings)
    {
        EmitSignal(SignalName.CommonEventCalled, target, args);
    }

    /// <summary>Test hook: stop emitting the signal while still tracking picture state.</summary>
    public bool EmitPictureSignals { get; set; } = true;

    private readonly System.Collections.Generic.List<PictureCmd> _pictureQueue = new();

    /// <summary>Test hook: queue picture commands and let GDScript drain them after the VM step.</summary>
    public bool QueuePictureSignals { get; set; }

    /// <summary>Test hook: drain queued pictures into track-only state instead of drawing Controls.</summary>
    public bool TrackPicturesOnly { get; set; }

    /// <summary>Drain queued picture commands as a JSON array (avoids managed Variant containers).</summary>
    public string TakePicturesJson()
    {
        if (_pictureQueue.Count == 0)
            return string.Empty;
        var sb = new System.Text.StringBuilder(256);
        sb.Append('[');
        for (int i = 0; i < _pictureQueue.Count; i++)
        {
            var cmd = _pictureQueue[i];
            if (i > 0) sb.Append(',');
            sb.Append("{\"number\":").Append(cmd.Number);
            sb.Append(",\"action\":").Append((int)cmd.Action);
            sb.Append(",\"file\":");
            AppendJsonString(sb, cmd.File);
            sb.Append(",\"text\":");
            AppendJsonString(sb, cmd.Text);
            sb.Append(",\"args\":[");
            var args = cmd.Args;
            if (args != null)
            {
                for (int k = 0; k < args.Length; k++)
                {
                    if (k > 0) sb.Append(',');
                    sb.Append(args[k]);
                }
            }
            sb.Append("],\"anchor\":").Append(cmd.Anchor);
            sb.Append(",\"kind\":").Append(cmd.Kind).Append('}');
        }
        sb.Append(']');
        _pictureQueue.Clear();
        return sb.ToString();
    }

    private static void AppendJsonString(System.Text.StringBuilder sb, string? value)
    {
        if (value == null) { sb.Append("\"\""); return; }
        sb.Append('"');
        foreach (char c in value)
        {
            switch (c)
            {
                case '"': sb.Append("\\\""); break;
                case '\\': sb.Append("\\\\"); break;
                case '\n': sb.Append("\\n"); break;
                case '\r': sb.Append("\\r"); break;
                case '\t': sb.Append("\\t"); break;
                default:
                    if (c < 0x20) sb.Append("\\u").Append(((int)c).ToString("x4"));
                    else sb.Append(c);
                    break;
            }
        }
        sb.Append('"');
    }

    public void Picture(PictureCmd cmd)
    {
        _lastPictureAnchor = cmd.Anchor;
        _lastPictureKind = cmd.Kind;
        switch (cmd.Action)
        {
            case PictureAction.Draw:
            case PictureAction.Move:
            case PictureAction.Set:
                _pictures[cmd.Number] = cmd;
                break;
            default:
                // Erase and Hide both free the slot (crates/wolf-game/src/picture.rs).
                _pictures.Remove(cmd.Number);
                break;
        }
        if (!EmitPictureSignals)
            return;
        if (QueuePictureSignals)
        {
            _pictureQueue.Add(cmd);
            return;
        }
        EmitSignal(SignalName.PictureRequested, cmd.Number, (int)cmd.Action,
            cmd.File ?? string.Empty, cmd.Text ?? string.Empty,
            cmd.Args ?? System.Array.Empty<int>());
    }

    public void Sound(SoundCmd cmd)
    {
        _lastSoundLoopPoint = cmd.LoopPoint;
        _lastSoundTempo = cmd.Tempo;
        EmitSignal(SignalName.SoundRequested, (int)cmd.Channel, (int)cmd.Action, cmd.File, cmd.Volume, cmd.Frames);
    }

    public DbValue? Database(DbOp op)
    {
        var value = DatabaseStore?.Execute(op);
        if (op.Container == DbContainer.Data && op.DbType == 9)
        {
            BattleDataReads++;
            BattleDataLast = "row=" + op.DataId + " col=" + op.ItemId + " dir=" + op.Direction
                + " -> " + (value != null ? (value.IsString ? "s:" + value.Str : value.Num.ToString()) : "null");
        }
        return value;
    }

    /// <summary>Test hook: number of opcode 250 reads against DataBase type 9.</summary>
    public int BattleDataReads { get; private set; }

    /// <summary>Test hook: the most recent DataBase type 9 read, formatted.</summary>
    public string BattleDataLast { get; private set; } = string.Empty;

    public string DescribeBattleData()
    {
        var sb = new System.Text.StringBuilder();
        for (int c = 0; c < 7; c++)
        {
            var v = DatabaseStore?.Read(new DbOp { Container = DbContainer.Data, Direction = DbDirection.Read, DbType = 9, DataId = 0, ItemId = c, Flags = 0x1200 });
            if (c > 0) sb.Append(' ');
            sb.Append(c).Append('=');
            sb.Append(v != null ? (v.IsString ? v.Str : v.Num.ToString()) : "null");
        }
        return sb.ToString();
    }

    public string MapPath(int mapId) =>
        DatabaseStore?.ReadText(DbContainer.Sys, 0, mapId, 0) ?? "";

    /// <summary>Battle data precondition (battle_seed.rs); tests call this, the game never does.</summary>
    public int SeedBattle(int partyCount = 1, int enemyCount = 1) =>
        DatabaseStore == null
            ? 0
            : DatabaseStore.SeedBattleCells(partyCount, enemyCount)
              + DatabaseStore.SeedBattleMenuText();

    public void Teleport(int target, int map, int x, int y, int options)
    {
        EmitSignal(SignalName.TeleportRequested, target, map, x, y, options);
    }

    public void Transition(int kind, int flags)
    {
        EmitSignal(SignalName.TransitionRequested, kind, flags);
    }

    public int? SaveLoad(SaveOp op) => null;

    /// <summary>
    /// Opcode 124. The geometry lives in PictureStage (it knows the real texture extent),
    /// so the GDScript side answers the tuple; fields 8/9 answer for absent pictures too.
    /// </summary>
    public int? PictureState(int number, int field)
    {
        if (_pictureStateTarget == null || !GodotObject.IsInstanceValid(_pictureStateTarget)) return null;
        Variant answer = _pictureStateTarget.Call(_pictureStateMethod, number, field, new Vector2(PointerX, PointerY));
        if (answer.VariantType == Variant.Type.Nil) return null;
        return answer.AsInt32();
    }

    public int? CharacterState(int target, int field) => null;

    /// <summary>
    /// Opcode 124 0x3000. WOLF reports the three mouse buttons as a tri-state:
    /// 0 = up, 1 = held, 2 = went down this frame (docs/OPCODE_124.md:97).
    /// Field 7 is left, 8 right, 9 middle; each uses the same one-frame edge latch.
    /// </summary>
    public int? EngineState(int field)
    {
        return field switch
        {
            7 => TriState(_pointerPressed, _pointerPressFrame),
            8 => TriState(_rightPressed, _rightPressFrame),
            9 => TriState(_middlePressed, _middlePressFrame),
            _ => 0,
        };
    }

    private static int TriState(bool held, bool pressedThisFrame) => pressedThisFrame ? 2 : (held ? 1 : 0);

    // Godot's C# marshalling refuses Nullable<int>, so the two IHost queries would vanish
    // from the GDScript method table. These bridges use int.MinValue for "no answer".
    public int PictureStateBridge(int number, int field) => PictureState(number, field) ?? int.MinValue;

    public int EngineStateBridge(int field) => EngineState(field) ?? int.MinValue;

    public int KeyCode(uint mask) => KeyCodeAuto(mask, 0);

    /// <summary>
    /// A GDScript lambda kept in a C# Callable field loses its binding here, so the bridge
    /// stores the target node plus the method name and calls it through Object.Call.
    /// </summary>
    public void SetPictureStateProvider(GodotObject target, StringName method)
    {
        _pictureStateTarget = target;
        _pictureStateMethod = method;
    }

    /// <summary>Test/probe bridge for a raw opcode 126 argument (target bits + state bits).</summary>
    public void ApplyInputPermission(uint raw) => Vm.ApplyInputPermission(raw);
    public uint BasicMoveDeny => Vm.BasicMoveDeny;
    public bool DeviceDeny => Vm.DeviceDeny;

    public void SetPointer(float x, float y)
    {
        PointerX = x;
        PointerY = y;
    }

    public void SetPointerPressed(bool pressed)
    {
        if (pressed && !_pointerPressed) _pointerPressPending = true;
        _pointerPressed = pressed;
    }

    /// <summary>
    /// Set the held state without producing the one-frame "went down" edge. Menus that
    /// poll opcode 124 for held==1 need this so a scripted click is not read as 2.
    /// </summary>
    public void SetPointerHeld(bool held)
    {
        _pointerPressed = held;
        _pointerPressPending = false;
        _pointerPressFrame = false;
    }

    public void SetPointerRightPressed(bool pressed)
    {
        if (pressed && !_rightPressed) _rightPressPending = true;
        _rightPressed = pressed;
    }

    public void SetPointerMiddlePressed(bool pressed)
    {
        if (pressed && !_middlePressed) _middlePressPending = true;
        _middlePressed = pressed;
    }

    /// <summary>Clear position and every held/edge latch, e.g. before rebuilding the title.</summary>
    public void ResetPointer()
    {
        PointerX = -1.0f;
        PointerY = -1.0f;
        _pointerPressed = false;
        _pointerPressPending = false;
        _pointerPressFrame = false;
        _rightPressed = false;
        _rightPressPending = false;
        _rightPressFrame = false;
        _middlePressed = false;
        _middlePressPending = false;
        _middlePressFrame = false;
    }

    /// <summary>Turn the latched press into this frame's answer, once per 1/60 s step.</summary>
    public void BeginFrameInput()
    {
        _pointerPressFrame = _pointerPressPending;
        _pointerPressPending = false;
        _rightPressFrame = _rightPressPending;
        _rightPressPending = false;
        _middlePressFrame = _middlePressPending;
        _middlePressPending = false;
    }

    /// <summary>Scripted sessions answer every poll with the OK key (legacy title-path parity).</summary>
    public int ScriptedKeyCode => KeyCodes.CodeOk;

    /// <summary>
    /// Mask-to-code bridge for the GDScript router: C# statics are not visible from
    /// GDScript, so KeyCodes.Code (the single source of truth) is exposed here instead of
    /// being mirrored in presentation_gd (W2-P2-02). A save/load list polls mask 771 to
    /// read the signed wheel notch before the held bits are consulted.
    /// </summary>
    public int KeyCodeForPoll(uint mask, uint held, uint autoKeys, int wheelNotch)
    {
        if (mask == 771 && wheelNotch != 0) return wheelNotch;
        return KeyCodes.Code((held | autoKeys) & KeyCodes.AllKeys);
    }

    // Opcode 123 hands the poll mask plus the bits opcode 125 auto-pressed to the host, and
    // mask 771 asks for a signed wheel notch rather than a key code. GDScript cannot marshal
    // Func, so the host side is a Callable taking both numbers (W2-P1-03).
    private Callable _keyStateProvider;
    private bool _hasKeyStateProvider;

    public void SetKeyStateProvider(Callable provider)
    {
        _keyStateProvider = provider;
        _hasKeyStateProvider = true;
    }

    public int KeyCodeAuto(uint mask, uint autoKeys)
    {
        if (_hasKeyStateProvider)
        {
            return _keyStateProvider.Call((int)mask, (int)autoKeys).AsInt32();
        }
        return KeyCodeProvider?.Invoke(mask) ?? 0;
    }

    // GDScript cannot marshal Func<uint,int>; wrap the Callable here instead.
    public void SetKeyCodeProvider(Callable provider)
    {
        KeyCodeProvider = mask => provider.Call(mask).AsInt32();
    }

    /// End every running script; GDScript cannot reach the EventVm property (Vm).
    public void StopAll() => Vm.StopAll();
    public void ResetDiagnostics() => Vm.ResetDiagnostics();

    /// <summary>
    /// D-33/R7-NEW-04: a rebuilt session must not inherit the previous run's 250 overlays,
    /// variables or 126 input policy. AutoEvents is session-scoped; these live on the VM.
    /// </summary>
    public void ResetSessionState()
    {
        Vm.Vars.Clear();
        Vm.ResetInputPermissions();
        DatabaseStore?.ClearOverlays();
    }

    /// Start a common event by id. The real starter returns Instance, a project type that
    /// GDScript cannot marshal, so the presentation layer gets a bool (W3 Day 1).
    public bool StartEvent(int eventId) => StartCommonEvent(eventId) != null;

    public void Effect(int kind, int duration, int[] args)
    {
        EmitSignal(SignalName.EffectRequested, kind, duration, args);
    }

    public void Reserve(int target) { }

    public void Log(string message)
    {
        GD.Print($"[EventVm Log] {message}");
    }
}
