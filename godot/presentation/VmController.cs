using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Presentation;

/// <summary>
/// VmBridge (interface frozen in M2 W1): the only boundary between the deterministic
/// C# core and the presentation layer.
/// Emission: PictureRequested, SoundRequested, MessageRequested, MessageClosed,
/// ChoicesRequested, ChoiceSelected, TeleportRequested, TransitionRequested,
/// EffectRequested, EventFinished.
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

    // 124 engine_state telemetry: wheel notch for this frame plus per-frame pointer motion.
    private int _wheelDelta;
    private float _pointerDeltaX;
    private float _pointerDeltaY;
    private float _prevPointerX = -1f;
    private float _prevPointerY = -1f;
    private readonly Queue<(int EventId, int[] Args, string[] Strings)> _reservedEvents = new();

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
            string rootDir = AppPaths.PackageRoot();
            path = Path.GetFullPath(Path.Combine(rootDir, "game_data", "programs.json"));
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
        string projectRoot = AppPaths.PackageRoot();
        string basicDir = Path.GetFullPath(Path.Combine(projectRoot, "work", "all", "BasicData"));
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
        _wheelDelta = 0;
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

    /// <summary>Live instance self slot, falling back to the persisted common-self bank.</summary>
    public int GetEventSelf(int eventId, int slot)
    {
        foreach (var inst in Vm.Instances)
        {
            if (inst.EventId == eventId) return inst.Self.Get(slot);
        }
        if (Vm.CommonSelf.TryGetValue(eventId, out SelfVars? bank)) return bank.Get(slot);
        return -1;
    }

    /// <summary>All live instance self slots for an event (test telemetry: stage28/29 coexist).</summary>
    public int[] GetEventSelfAll(int eventId, int slot)
    {
        var values = new List<int>();
        foreach (var inst in Vm.Instances)
        {
            if (inst.EventId == eventId) values.Add(inst.Self.Get(slot));
        }
        return values.ToArray();
    }

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

    // Part of the frozen IHost contract; common-event routing stays inside EventVm and
    // has no presentation listener, so this no longer emits a signal (R10-A1).
    public void CommonEventCall(int target, int[] args, string[] strings)
    {
    }

    /// <summary>Test hook: stop emitting the signal while still tracking picture state.</summary>
    public bool EmitPictureSignals { get; set; } = true;

    private readonly System.Collections.Generic.List<PictureCmd> _pictureQueue = new();

    /// <summary>Queue picture commands as JSON for the presentation layer. Defaults on; set
    /// false only in focused tests that still consume the legacy PictureRequested signal.</summary>
    public bool QueuePictureSignals { get; set; } = true;

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
            sb.Append(",\"kind\":").Append(cmd.Kind);
            sb.Append(",\"animate\":[");
            var animate = cmd.Animate;
            if (animate != null)
            {
                for (int k = 0; k < animate.Length; k++)
                {
                    if (k > 0) sb.Append(',');
                    sb.Append(animate[k]);
                }
            }
            sb.Append("]}");
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

    /// <summary>
    /// Opcode 124 0x1000/0x1200. Target -2 is WOLF's mouse sentinel (tile and pixel
    /// coordinates); other positives address map events (map_runtime.rs character_state).
    /// </summary>
    public int? CharacterState(int target, int field)
    {
        const int tilePx = 16;
        if (target == -2)
        {
            // -2 is the party leader (game.rs character_state), not the mouse.
            return field switch
            {
                0 => LeaderX,
                1 => LeaderY,
                2 => LeaderX * 2,
                3 => LeaderY * 2,
                4 => 0,
                5 => LeaderFacing,
                6 or 8 => LeaderX * tilePx + tilePx / 2,
                7 or 9 => (LeaderY + 1) * tilePx,
                10 => -1,
                _ => null,
            };
        }
        if (target < 0) return null;
        var ev = Map?.Events.FirstOrDefault(e => e.Def.Id == target);
        if (ev == null) return null;
        int direction = ev.Active is int page && page >= 0 && page < ev.Def.Pages.Count
            ? ev.Def.Pages[page].Direction
            : 2;
        return field switch
        {
            0 => ev.Def.X,
            1 => ev.Def.Y,
            2 => ev.Def.X * 2,
            3 => ev.Def.Y * 2,
            4 => 0,
            5 => direction,
            6 or 8 => ev.Def.X * tilePx + tilePx / 2,
            7 or 9 => (ev.Def.Y + 1) * tilePx,
            10 => ev.Def.Id,
            _ => null,
        };
    }

    /// <summary>
    /// Opcode 124 0x3000. WOLF reports the three mouse buttons as a tri-state:
    /// 0 = up, 1 = held, 2 = went down this frame (docs/OPCODE_124.md:97).
    /// Field 7 is left, 8 right, 9 middle; each uses the same one-frame edge latch.
    /// </summary>
    public int? EngineState(int field)
    {
        return field switch
        {
            // game.rs 6898-6900: the static map id and BGM id are 0 and -1.
            0 => 0,
            1 => -1,
            7 => TriState(_pointerPressed, _pointerPressFrame),
            8 => TriState(_rightPressed, _rightPressFrame),
            9 => TriState(_middlePressed, _middlePressFrame),
            10 => _wheelDelta,
            11 => (int)_pointerDeltaX,
            12 => (int)_pointerDeltaY,
            20 => (int)PointerX,
            21 => (int)PointerY,
            // Unknown fields have no answer (game.rs returns None), so 124 must not
            // store a fabricated 0; field 18 in particular falls through to EventVm.
            _ => null,
        };
    }

    private static int TriState(bool held, bool pressedThisFrame) => pressedThisFrame ? 2 : (held ? 1 : 0);

    // Godot's C# marshalling refuses Nullable<int>, so the two IHost queries would vanish
    // from the GDScript method table. These bridges use int.MinValue for "no answer".
    public int PictureStateBridge(int number, int field) => PictureState(number, field) ?? int.MinValue;

    public int EngineStateBridge(int field) => EngineState(field) ?? int.MinValue;

    public int CharacterStateBridge(int target, int field) => CharacterState(target, field) ?? int.MinValue;

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

    /// <summary>Wheel notch sampled by the router once per frame (124 field 10).</summary>
    public void SetFrameWheel(int notch) => _wheelDelta = notch;

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
        _prevPointerX = -1f;
        _prevPointerY = -1f;
        _pointerDeltaX = 0f;
        _pointerDeltaY = 0f;
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

        if (_prevPointerX < 0f || _prevPointerY < 0f)
        {
            _pointerDeltaX = 0f;
            _pointerDeltaY = 0f;
        }
        else
        {
            _pointerDeltaX = PointerX - _prevPointerX;
            _pointerDeltaY = PointerY - _prevPointerY;
        }
        _prevPointerX = PointerX;
        _prevPointerY = PointerY;
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
        // Returning to the title drops every pending 211 reservation; a rebuilt
        // session must not fire the previous run's queued events (game.rs queue).
        _reservedEvents.Clear();
    }

    /// <summary>Apply retail SavePart3 database overlay cells to the shared database.</summary>
    public void ApplySavedCells(IReadOnlyList<LegacySaveImporter.DatabaseCell> cells)
    {
        DatabaseStore?.ApplySavedCells(cells);
    }

    /// Start a common event by id. The real starter returns Instance, a project type that
    /// GDScript cannot marshal, so the presentation layer gets a bool (W3 Day 1).
    public bool StartEvent(int eventId) => StartCommonEvent(eventId) != null;

    public void Effect(int kind, int duration, int[] args)
    {
        EmitSignal(SignalName.EffectRequested, kind, duration, args);
    }

    public void Reserve(int target) => Reserve(target, Array.Empty<int>(), Array.Empty<string>());

    /// <summary>Pending 211 reservations, for diagnostics and tests.</summary>
    public int PendingReserveCount => _reservedEvents.Count;

    /// <summary>Opcode 211: queue one common event; the frame scheduler fires it when idle.</summary>
    public void Reserve(int target, int[] args, string[] strings)
    {
        _reservedEvents.Enqueue((target, args ?? Array.Empty<int>(), strings ?? Array.Empty<string>()));
    }

    /// <summary>
    /// exec.rs frame tail: at most one reservation per frame, and only while no
    /// non-parallel instance is still running or blocked.
    /// </summary>
    public void StartReservedEvent()
    {
        if (_reservedEvents.Count == 0) return;
        if (Vm.ForegroundBusy()) return;
        var next = _reservedEvents.Dequeue();
        var inst = Vm.StartEventWithArgs(next.EventId, next.Args, next.Strings);
        if (inst == null)
        {
            GD.PushError($"211 Reserve: event {next.EventId} is not in programs.json");
        }
    }

    public void Log(string message)
    {
        GD.Print($"[EventVm Log] {message}");
    }
}
