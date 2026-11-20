using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// M4 Gate 1: production GameSession cold start -> day1/2/3 CE251 -> day4 CE253/CE239
/// -> day5 08:30, with a wolf.route trace of every VM instruction and a final F5/F9
/// save/load state round-trip. The Rust side runs the same state-driven driver through
/// wolf-cli route-full; oracle/diff_route.py compares the two traces.
/// </summary>
public partial class M4FullRouteTraceTest : Node
{
    private const int BedroomGuard = 1600;
    private const string RouteName = "m4_full_day_ce253_f5f9";

    private VmController _bridge = null!;
    private GameSession _session = null!;
    private M4RouteView _view = null!;
    private M4RouteHost _host = null!;
    private readonly List<TraceStep> _steps = new(65536);
    private readonly List<int> _mapIds = new(65536);
    private readonly List<int> _stepInstanceIds = new(65536);
    private readonly List<string> _failures = new();
    private Dictionary<int, int> _lastVars = new();
    private Dictionary<int, string> _lastStrs = new();
    private Instance? _lastInstance;
    private int _lastInstanceId = -1;
    private bool _lastStepFinalized;
    private double _pointerX = -1, _pointerY = -1;
    private bool _mouseDown;
    private bool _actionQueued;
    private string _programsPath = "";

    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string repo = Path.GetFullPath(Path.Combine(rootDir, ".."));
        _programsPath = Path.Combine(repo, "game_data", "programs.json");
        string outPath = Path.Combine(repo, "reports", "csharp_m4_route_trace.json");
        string? dir = Path.GetDirectoryName(outPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        _view = new M4RouteView();
        _bridge = new VmController();
        AddChild(_bridge);
        _bridge.KeyCodeProvider = _ => KeyCodes.CodeOk;
        _bridge.StrictMode = true;
        if (!_bridge.LoadPrograms(_programsPath))
        {
            Fail("cannot load programs.json");
            FinishAll();
            return;
        }

        _host = new M4RouteHost(_bridge);
        _bridge.Vm.Host = _host;
        // Keep the trace session's save files out of reports/: the port stores the script
        // operand "save/system.sav" beside the session save, and a stale rotation from an
        // earlier gate run would make the first 221 leg read old play time (ROUTE_DIFF 39/...).
        string saveDir = Path.Combine(Path.GetTempPath(), "lwsmf-m4-route-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(saveDir);
        _session = new GameSession(_bridge, _view, _programsPath, Path.Combine(saveDir, "unused_save.json"));
        string systemSavePath = Path.Combine(saveDir, "system.json");

        _session.ScriptedInput = true;
        _session.AutoPickChoice = true;
        _bridge.MessageRequested += (text, face) => _session.ShowMessage(text, face);
        _bridge.MessageClosed += () => _session.NotifyMessageClosed();
        _bridge.ChoicesRequested += options => _session.ShowChoices(options);
        _bridge.TeleportRequested += (target, map, x, y, options) =>
            _session.QueueTeleport(target, map, x, y, options);
        _bridge.SetPointer(-1, -1);

        StartTrace();

        if (!_session.StartTitle())
        {
            Fail("TitleMap event 2 missing");
            FinishAll();
            return;
        }

        int guard = 0;
        while (_session.Phase != "bedroom" && guard < BedroomGuard)
        {
            Step();
            guard++;
        }
        if (_session.Phase != "bedroom" || (_session.MapData?.Id ?? -1) != 1005)
        {
            Fail($"bedroom not reached: phase={_session.Phase} map={_session.MapData?.Id ?? -1} frames={_session.Frames}");
            FinishAll();
            return;
        }
        if (guard >= BedroomGuard)
        {
            Fail("bedroom guard exhausted");
            FinishAll();
            return;
        }

        // Same normalization as ColdStartDayTest / m9: the retail transfer leaves two
        // opening-day counters latched. These are pre-route state, not settlement data.
        _bridge.SetVar(295, 0);
        _bridge.SetVar(68, 0);

        RunNaturalRoute();

        if (_failures.Count == 0 && !_bridge.Vm.Faulted && _bridge.Vm.UnhandledOpcodeCount == 0)
        {
            int day = Var(160), hour = Var(4), minute = Var(5);
            if (day != 5 || hour != 8 || minute < 30)
                Fail($"final clock day={day} {hour}:{minute}, expected day5 08:30");
            if (!_session.BattleWiring.SawDispatch || !_session.BattleWiring.SawCaller)
                Fail($"battle telemetry saw_dispatch={_session.BattleWiring.SawDispatch} saw_caller={_session.BattleWiring.SawCaller}");
            if (_session.BattleWiring.StartedRounds == 0 || _session.BattleWiring.TurnStarts == 0)
                Fail($"battle rounds={_session.BattleWiring.StartedRounds} turns={_session.BattleWiring.TurnStarts}");
        }

        int stepsCount = _steps.Count;
        int frames = _session.Frames;
        _bridge.Vm.StepHook = null;

        // F5/F9 leg: stop tracing, then round-trip the complete GameSession state.
        int framesAtSave = _session.Frames;
        int leaderXAtSave = _session.LeaderX;
        int leaderYAtSave = _session.LeaderY;
        int mapAtSave = _session.MapData?.Id ?? -1;
        bool saved = _session.SaveGame();
        _session.AdvanceFrame();
        _session.AdvanceFrame();
        bool loaded = _session.LoadGame();
        bool saveOk = saved && loaded
            && _session.Frames == framesAtSave
            && _session.LeaderX == leaderXAtSave
            && _session.LeaderY == leaderYAtSave
            && (_session.MapData?.Id ?? -1) == mapAtSave;
        if (File.Exists(_session.SavePath)) File.Delete(_session.SavePath);
        if (File.Exists(systemSavePath)) File.Delete(systemSavePath);
        try { if (Directory.Exists(saveDir)) Directory.Delete(saveDir, true); } catch (IOException) { }
        if (!saveOk)
            Fail($"save/load saved={saved} loaded={loaded} frames={_session.Frames}/{framesAtSave} "
                + $"leader={_session.LeaderX},{_session.LeaderY}/{leaderXAtSave},{leaderYAtSave}");

        WriteTrace(outPath, frames, stepsCount);

        GD.Print($"[M4FullRouteTraceTest] steps={stepsCount} frames={frames} phase={_session.Phase} "
            + $"map={_session.MapData?.Id ?? -1} leader={_session.LeaderX},{_session.LeaderY} "
            + $"battle_rounds={_session.BattleWiring.StartedRounds} turns={_session.BattleWiring.TurnStarts} "
            + $"save_load={(saveOk ? "ok" : "fail")} unhandled={_bridge.Vm.UnhandledOpcodeCount} "
            + $"faulted={_bridge.Vm.Faulted}");

        if (_failures.Count > 0 || stepsCount == 0 || _bridge.Vm.UnhandledOpcodeCount != 0 || _bridge.Vm.Faulted)
        {
            GD.PrintErr($"[M4FullRouteTraceTest] FAIL count={_failures.Count} steps={stepsCount} "
                + $"unhandled={_bridge.Vm.UnhandledOpcodeCount} fault={_bridge.Vm.FaultReason}");
            foreach (string failure in _failures) GD.PrintErr("  - " + failure);
            GetTree().Quit(1);
            return;
        }

        GD.Print($"M4_FULL_ROUTE_TRACE_TEST_OK steps={stepsCount} day=5 hour=8 minute={Var(5)} "
            + $"save_load=ok map={_session.MapData?.Id ?? -1}");
        GetTree().Quit();
    }

    private void RunNaturalRoute()
    {
        // Day 1: natural dinner -> night menu -> sleep -> day 2 08:30.
        SleepToNextMorning(1);
        if (_failures.Count > 0) return;

        // Day 2 and day 3: real CE251 daytime command rows.
        SpendDay(2);
        if (_failures.Count > 0) return;
        SleepToNextMorning(2);
        if (_failures.Count > 0) return;

        SpendDay(3);
        if (_failures.Count > 0) return;

        // Day 4 is dispatched naturally from the day-3 night script. Seed only the
        // documented battle database precondition (party stats / command slots / stock).
        int seeded = _bridge.SeedBattle(1, 1);
        if (seeded < 26) Fail($"battle seed wrote {seeded} cells, expected >= 26");

        SleepToNextMorning(3);
        if (_failures.Count > 0) return;

        // Day 4 morning CE253 already ran during the sleep leg; spend the rest of day 4.
        SpendDay(4);
        if (_failures.Count > 0) return;
        SleepToNextMorning(4);
    }

    private void SpendDay(int day)
    {
        int beforeEnergy = Var(14);
        SelectDayRow(day);
        if (_failures.Count > 0) return;

        bool evening = false;
        for (int i = 0; i < 3200; i++)
        {
            if (Var(4) >= 17 && !_session.HasEvent(251))
            {
                evening = true;
                break;
            }
            Step();
        }
        if (!evening) Fail($"day {day}: CE251 daytime did not end (hour={Var(4)} ev251={_session.HasEvent(251)})");
        if (Var(14) >= beforeEnergy) Fail($"day {day}: energy did not fall ({beforeEnergy}->{Var(14)})");

        bool settled = false;
        for (int i = 0; i < 6000; i++)
        {
            if (!_session.HasEvent(300) && !_session.HasEvent(326) && Var(4) >= 18)
            {
                settled = true;
                break;
            }
            Step();
        }
        if (!settled) Fail($"day {day}: evening did not settle (hour={Var(4)} ev300={_session.HasEvent(300)} ev326={_session.HasEvent(326)})");
        if (!WaitPicture(120, 2600)) Fail($"day {day}: sister picture 120 did not appear");
    }

    private void SelectDayRow(int day)
    {
        if (!WaitPicture(1201, 2200)) { Fail($"day {day}: day row picture 1201 missing"); return; }
        if (!WaitPicture(1200, 400)) { Fail($"day {day}: day row picture 1200 missing"); return; }
        if (Var(160) != day || Var(4) < 8) Fail($"day {day}: row menu opened at day={Var(160)} {Var(4)}:{Var(5)}");

        int beforeCommand = Var(48);
        int rowX = PictureState(1200, 0) ?? 512;
        int rowY = PictureState(1200, 1) ?? 0;
        MouseMove(rowX, rowY);
        Tick(12);
        for (int i = 0; i < 6; i++) { MouseDown(true); Step(); }
        for (int i = 0; i < 5; i++) { MouseDown(false); Step(); }
        MouseMove(-1, -1);

        bool latched = false;
        for (int i = 0; i < 2600; i++)
        {
            if (Var(48) != beforeCommand) { latched = true; break; }
            Step();
        }
        if (!latched) Fail($"day {day}: CE251 row click did not change command var48={Var(48)}");
    }

    private void SleepToNextMorning(int day)
    {
        int expected = day + 1;
        if (Var(295) == 1) SetVar(295, 0);

        bool evening = false;
        for (int i = 0; i < 3000; i++)
        {
            if (Var(160) == day && Var(4) >= 18) { evening = true; break; }
            Step();
        }
        if (!evening) { Fail($"sleep day {day}: evening not reached (day={Var(160)} {Var(4)}:{Var(5)})"); return; }

        bool hasPicture = WaitPicture(120, 600);
        if (day > 1)
        {
            bool menu = false;
            for (int i = 0; i < 6000; i++)
            {
                if (_session.HasEvent(245)) { menu = true; break; }
                Step();
            }
            if (!menu) Fail($"sleep day {day}: night menu CE245 did not appear");
        }
        if (!hasPicture && !WaitPicture(120, 600))
        {
            Fail($"sleep day {day}: sister picture 120 missing");
            return;
        }

        _session.AutoPickChoice = false;
        int sx = PictureState(120, 0) ?? 240;
        int sy = PictureState(120, 1) ?? 340;
        MouseMove(sx, sy);
        Tick(8);
        Action();
        Tick(40);
        bool menuAfter = false;
        for (int i = 0; i < 400; i++)
        {
            if (_session.HasEvent(245)) { menuAfter = true; break; }
            Step();
        }
        if (!menuAfter) Fail($"sleep day {day}: CE245 disappeared before sleep choice");

        ClickAt(512, 450);
        bool gotChoice = false;
        for (int i = 0; i < 500; i++)
        {
            if (_session.HasChoices) { gotChoice = true; break; }
            Step();
        }
        string[] options = _bridge.GetCurrentChoices();
        if (!gotChoice || options.Length != 2 || options[0] != "是" || options[1] != "算了")
            Fail($"sleep day {day}: choice missing or wrong [{string.Join(",", options)}]");
        _session.PickChoice(0);
        _session.AutoPickChoice = true;

        if (day == 3)
        {
            bool morning = false;
            for (int i = 0; i < 3200; i++)
            {
                if (Var(160) == 4 && Var(4) == 8 && Var(5) >= 30) { morning = true; break; }
                if ((i & 1) == 0) Action();
                Step();
            }
            if (!morning) Fail($"sleep day 3: day4 08:30 not reached (day={Var(160)} {Var(4)}:{Var(5)})");
        }
        else
        {
            if (!WaitClock(expected, 8, 30, 3200))
                Fail($"sleep day {day}: day{expected} 08:30 not reached (day={Var(160)} {Var(4)}:{Var(5)})");
        }
    }

    private bool WaitPicture(int number, int limit)
    {
        for (int i = 0; i < limit; i++)
        {
            if (PictureState(number, 9) == 1) return true;
            Step();
        }
        return PictureState(number, 9) == 1;
    }

    private bool WaitClock(int day, int hour, int minute, int limit)
    {
        for (int i = 0; i < limit; i++)
        {
            if (Var(160) == day && Var(4) == hour && Var(5) >= minute) return true;
            Step();
        }
        return Var(160) == day && Var(4) == hour && Var(5) >= minute;
    }

    private void StartTrace()
    {
        _bridge.Vm.StepHook = (inst, pc) =>
        {
            if (_steps.Count > 0 && !_lastStepFinalized)
                Finish(_steps[^1], _stepInstanceIds[^1], _lastVars, _lastStrs, _lastInstance,
                    _lastInstanceId, _bridge.Vm, _host, inst.Id);
            int op = pc < inst.Program.Slots.Count ? inst.Program.Slots[pc].Opcode : 0;
            _steps.Add(new TraceStep
            {
                Step = _steps.Count + 1,
                Frame = _bridge.Vm.Frame,
                EventId = inst.EventId,
                Pc = pc,
                Op = op,
            });
            _mapIds.Add(_session.MapData?.Id ?? -1);
            _stepInstanceIds.Add(inst.Id);
            _lastVars = Snapshot(_bridge.Vm, inst);
            _lastStrs = SnapshotStrings(_bridge.Vm, inst);
            _lastInstance = inst;
            _lastInstanceId = inst.Id;
            _lastStepFinalized = false;
        };
    }

    /// <summary>Attribute the last instruction's effect at the real frame boundary.</summary>
    private void FinalizeLastStep()
    {
        if (_steps.Count == 0 || _lastStepFinalized) return;
        Finish(_steps[^1], _stepInstanceIds[^1], _lastVars, _lastStrs, _lastInstance,
            _lastInstanceId, _bridge.Vm, _host, null);
        _lastVars = Snapshot(_bridge.Vm, _lastInstance);
        _lastStrs = SnapshotStrings(_bridge.Vm, _lastInstance);
        _lastStepFinalized = true;
    }

    private void Step()
    {
        _bridge.SetPointer((float)_pointerX, (float)_pointerY);
        _bridge.SetPointerPressed(_mouseDown);
        _bridge.SetPointerRightPressed(false);
        _bridge.SetPointerMiddlePressed(false);
        _bridge.BeginFrameInput();
        if (_actionQueued)
        {
            if (!_session.MessageOpen && !_session.HasChoices) _session.PressActionKey();
            _actionQueued = false;
        }
        _session.AdvanceFrame();
        FinalizeLastStep();
    }

    private void Tick(int count) { for (int i = 0; i < count; i++) Step(); }
    private void Action() => _actionQueued = true;
    private void MouseMove(int x, int y) { _pointerX = x; _pointerY = y; }
    private void MouseDown(bool down) => _mouseDown = down;
    private void ClickAt(int x, int y)
    {
        MouseMove(x, y);
        Tick(10);
        for (int i = 0; i < 5; i++) { MouseDown(true); Step(); }
        for (int i = 0; i < 5; i++) { MouseDown(false); Step(); }
    }

    private int Var(int id) => _bridge.GetVar(id);
    private void SetVar(int id, int value) => _bridge.SetVar(id, value);
    private int? PictureState(int number, int field) => _host.PictureState(number, field);

    private void Fail(string message)
    {
        if (!_failures.Contains(message)) _failures.Add(message);
        GD.PrintErr("[M4FullRouteTraceTest] " + message);
    }

    private void FinishAll()
    {
        if (_steps.Count > 0)
            Finish(_steps[^1], _stepInstanceIds[^1], _lastVars, _lastStrs, _lastInstance,
                _lastInstanceId, _bridge.Vm, _host, null);
        GetTree().Quit(1);
    }

    private void WriteTrace(string outPath, int frames, int stepsCount)
    {
        var sb = new StringBuilder(stepsCount * 160 + 256);
        sb.Append("{\"schema\":\"wolf.route\",\"schema_version\":1,");
        sb.Append("\"route\":\"").Append(RouteName).Append("\",\"frames\":").Append(frames);
        sb.Append(",\"total_steps\":").Append(stepsCount).Append(",\"steps\":[");
        for (int i = 0; i < stepsCount; i++)
        {
            if (i > 0) sb.Append(',');
            var s = _steps[i];
            sb.Append("{\"step\":").Append(i + 1)
              .Append(",\"frame\":").Append(s.Frame)
              .Append(",\"map_id\":").Append(_mapIds[i])
              .Append(",\"event_id\":").Append(s.EventId)
              .Append(",\"pc\":").Append(s.Pc)
              .Append(",\"op\":").Append(s.Op)
              .Append(",\"vars\":{");
            bool first = true;
            foreach (var pair in s.Vars.OrderBy(pair => pair.Key))
            {
                if (!first) sb.Append(',');
                first = false;
                sb.Append('"').Append(pair.Key).Append("\":").Append(pair.Value);
            }
            sb.Append("},\"str_vars\":{");
            first = true;
            foreach (var pair in s.StrVars.OrderBy(pair => pair.Key))
            {
                if (!first) sb.Append(',');
                first = false;
                sb.Append('"').Append(pair.Key).Append("\":")
                  .Append(JsonSerializer.Serialize(pair.Value));
            }
            sb.Append("},\"effects\":[");
            for (int e = 0; e < s.Effects.Count; e++)
            {
                if (e > 0) sb.Append(',');
                sb.Append(s.Effects[e].ToJsonString());
            }
            sb.Append("]}");
        }
        sb.Append("]}");
        File.WriteAllText(outPath, sb.ToString(), new UTF8Encoding(false));
    }

    private static Dictionary<int, int> Snapshot(EventVm vm, Instance? inst)
    {
        var map = new Dictionary<int, int>();
        var nums = vm.Vars.SnapshotNums();
        for (int i = 0; i < nums.Count; i++) map[i] = nums[i];
        var sys = vm.Vars.SnapshotSysNums();
        for (int i = 0; i < sys.Count; i++) map[9_000_000 + i] = sys[i];
        if (inst != null)
        {
            int[] self = inst.Self.SnapshotNums();
            for (int i = 0; i < self.Length; i++)
            {
                int group = i / 100;
                int slot = i % 100;
                map[1_000_000 + group * 100_000 + slot] = self[i];
            }
        }
        return map;
    }

    private static Dictionary<int, string> SnapshotStrings(EventVm vm, Instance? inst)
    {
        var map = new Dictionary<int, string>();
        var strings = vm.Vars.SnapshotStrings();
        for (int i = 0; i < strings.Count; i++)
        {
            if (!string.IsNullOrEmpty(strings[i])) map[3_000_000 + i] = strings[i];
        }
        var sys = vm.Vars.SnapshotSysStrings();
        for (int i = 0; i < sys.Count; i++)
        {
            if (!string.IsNullOrEmpty(sys[i])) map[4_000_000 + i] = sys[i];
        }
        if (inst != null)
        {
            string[] self = inst.Self.SnapshotStrings();
            for (int i = 0; i < self.Length; i++)
            {
                if (string.IsNullOrEmpty(self[i])) continue;
                int group = i / 100;
                int slot = i % 100;
                map[2_000_000 + group * 100_000 + slot] = self[i];
            }
        }
        return map;
    }

    private static void Finish(TraceStep step, int stepInstanceId, Dictionary<int, int> before,
                               Dictionary<int, string> beforeStrs, Instance? previous,
                               int beforeInstanceId, EventVm vm, M4RouteHost host,
                               int? currentInstanceId)
    {
        // Always finalize the previous instruction. At a coroutine switch this
        // is the parent's block-end finalize, exactly like Rust's finalize_trace
        // running before the child event starts. The snapshot still reads the
        // previous instance's self bank through `previous`.
        _ = currentInstanceId;
        var after = Snapshot(vm, previous);
        foreach (var pair in after)
        {
            if (!before.TryGetValue(pair.Key, out int old) ? pair.Value != 0 : old != pair.Value)
                step.Vars[pair.Key] = pair.Value;
        }
        foreach (var pair in before)
        {
            if (pair.Value != 0 && !after.ContainsKey(pair.Key)) step.Vars[pair.Key] = 0;
        }
        var afterStrs = SnapshotStrings(vm, previous);
        foreach (var pair in afterStrs)
        {
            if (!beforeStrs.TryGetValue(pair.Key, out string? old) || old != pair.Value)
                step.StrVars[pair.Key] = pair.Value;
        }
        foreach (var pair in beforeStrs)
        {
            if (!string.IsNullOrEmpty(pair.Value) && !afterStrs.ContainsKey(pair.Key))
                step.StrVars[pair.Key] = "";
        }
        foreach (var effect in host.PendingEffects) step.Effects.Add(effect);
        host.PendingEffects.Clear();
    }
}

/// <summary>Picture slot table mirroring PictureStage.track_picture/picture_state (opcode 124).</summary>
internal sealed class M4RouteHost : IHost
{
    private readonly VmController _inner;
    private readonly Dictionary<int, M4PictureEntry> _pictures = new();
    public List<JsonObject> PendingEffects { get; } = new();

    public M4RouteHost(VmController inner) => _inner = inner;

    public void Wait(uint frames) => _inner.Wait(frames);
    public void ShowMessage(string text, int? face) => _inner.ShowMessage(text, face);
    public bool MessageDismissed() => _inner.MessageDismissed();
    public void CloseMessage() => _inner.CloseMessage();
    public void ShowChoices(string[] options)
    {
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "choice",
            ["options"] = new JsonArray(options.Select(option => (JsonNode?)JsonValue.Create(option)).ToArray()),
            ["selected"] = 0
        });
        _inner.ShowChoices(options);
    }
    public ChoiceReply? ChoiceMade() => _inner.ChoiceMade();

    public void CommonEventCall(int target, int[] args, string[] strings)
    {
        var array = new JsonArray();
        foreach (int arg in args) array.Add(arg);
        PendingEffects.Add(new JsonObject { ["kind"] = "call", ["target_event_id"] = target, ["args"] = array });
        _inner.CommonEventCall(target, args, strings);
    }

    public void Picture(PictureCmd cmd)
    {
        var args = new JsonArray();
        foreach (int arg in cmd.Args) args.Add(arg);
        string action = cmd.Action switch
        {
            PictureAction.Draw => "draw",
            PictureAction.Move => "move",
            PictureAction.Erase => "erase",
            PictureAction.Hide => "hide",
            _ => "set"
        };
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "picture",
            ["action"] = action,
            ["slot"] = cmd.Number,
            ["file"] = cmd.File,
            ["args"] = args
        });
        Track(cmd);
        _inner.Picture(cmd);
    }

    public void Sound(SoundCmd cmd)
    {
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "sound",
            ["action"] = cmd.Action == SoundAction.Play ? "play" : "stop",
            ["type"] = cmd.Channel switch { SoundChannel.Bgm => "bgm", SoundChannel.Bgs => "bgs", _ => "se" },
            ["file"] = cmd.File,
            ["vol"] = cmd.Volume,
            ["frames"] = cmd.Frames
        });
        _inner.Sound(cmd);
    }

    public DbValue? Database(DbOp op)
    {
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "database",
            ["type"] = op.DbType,
            ["data"] = op.DataId,
            ["item"] = op.ItemId,
            ["value"] = op.Source?.Num ?? 0
        });
        return _inner.Database(op);
    }

    public void Teleport(int target, int map, int x, int y, int options)
    {
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "teleport",
            ["map_id"] = map,
            ["x"] = x,
            ["y"] = y,
            ["direction"] = options
        });
        _inner.Teleport(target, map, x, y, options);
    }

    public void Transition(int kind, int flags) { }
    public void Reserve(int target) => _inner.Reserve(target);
    public int? SaveLoad(SaveOp op) => _inner.SaveLoad(op);
    public int? PictureState(int number, int field) => PictureStateInternal(number, field);
    public int? CharacterState(int target, int field) => _inner.CharacterState(target, field);
    public int? EngineState(int field) => _inner.EngineState(field);
    public int KeyCode(uint mask) => _inner.KeyCode(mask);
    public int KeyCodeAuto(uint mask, uint autoKeys) => _inner.KeyCodeAuto(mask, autoKeys);

    public void Effect(int kind, int duration, int[] args)
    {
        var array = new JsonArray();
        foreach (int arg in args) array.Add(arg);
        PendingEffects.Add(new JsonObject { ["kind"] = "effect", ["type"] = kind, ["duration"] = duration, ["args"] = array });
        _inner.Effect(kind, duration, args);
    }

    public void Log(string message) => _inner.Log(message);

    private void Track(PictureCmd cmd)
    {
        if (cmd.Action is PictureAction.Erase or PictureAction.Hide)
        {
            _pictures.Remove(cmd.Number);
            return;
        }
        if (cmd.Action == PictureAction.Move)
        {
            if (!_pictures.TryGetValue(cmd.Number, out var prev)) return;
            var merged = new M4PictureEntry
            {
                Kind = cmd.Kind,
                File = string.IsNullOrEmpty(cmd.File) ? prev.File : cmd.File,
                Text = string.IsNullOrEmpty(cmd.Text) ? prev.Text : cmd.Text,
                Args = cmd.Args.Length == 0 ? prev.Args : cmd.Args,
                Anchor = cmd.Anchor,
            };
            merged.Base = BaseSize(merged);
            _pictures[cmd.Number] = merged;
            return;
        }
        var entry = new M4PictureEntry
        {
            Kind = cmd.Kind,
            File = cmd.File,
            Text = cmd.Text,
            Args = cmd.Args,
            Anchor = cmd.Anchor,
        };
        entry.Base = BaseSize(entry);
        _pictures[cmd.Number] = entry;
    }

    private static Vector2 BaseSize(M4PictureEntry entry)
    {
        int kind = entry.Kind;
        if (kind == 2 || kind == 4)
            return new Vector2(Math.Max(0, Arg(entry.Args, 1, 0)), Math.Max(0, Arg(entry.Args, 2, 0)));
        if (kind == 3 && Arg(entry.Args, 1, 0) > 2 && Arg(entry.Args, 2, 0) > 2)
            return new Vector2(Arg(entry.Args, 1, 0), Arg(entry.Args, 2, 0));
        return new Vector2(420, 140);
    }

    private int? PictureStateInternal(int number, int field)
    {
        bool exists = _pictures.TryGetValue(number, out var entry);
        if (field == 8)
        {
            if (!exists) return -1;
            return HitRect(entry!).HasPoint(new Vector2(_inner.PointerX, _inner.PointerY)) ? 1 : 0;
        }
        if (field == 9) return exists ? 1 : 0;
        if (!exists) return null;
        return field switch
        {
            0 => Unit(Arg(entry!.Args, 5, 0)),
            1 => Unit(Arg(entry.Args, 6, 0)),
            2 or 3 => Unit(Arg(entry.Args, 7, 100)),
            4 => Arg(entry.Args, 9, 0),
            5 => 0,
            10 => entry.Kind is 2 or 4 ? 1 : -1,
            _ => null
        };
    }

    private Rect2 HitRect(M4PictureEntry entry)
    {
        int number = _pictures.FirstOrDefault(pair => ReferenceEquals(pair.Value, entry)).Key;
        if (number < 0 || number >= 12000) return new Rect2(0, 0, 1024, 576);
        var baseSize = entry.Base;
        if (baseSize == Vector2.Zero) return new Rect2();
        int scalePercent = Unit(Arg(entry.Args, 7, 100));
        float factor = scalePercent == 0 ? 1f : Math.Abs(scalePercent) / 100f;
        var size = baseSize * factor;
        int x = Unit(Arg(entry.Args, 5, 0));
        int y = Unit(Arg(entry.Args, 6, 0));
        var pos = entry.Anchor switch
        {
            1 => new Vector2(x - size.X * 0.5f, y - size.Y * 0.5f),
            2 => new Vector2(x, y - size.Y),
            3 => new Vector2(x - size.X, y),
            4 => new Vector2(x - size.X, y - size.Y),
            5 => new Vector2(x - size.X * 0.5f, y),
            6 => new Vector2(x - size.X * 0.5f, y - size.Y),
            _ => new Vector2(x, y)
        };
        return new Rect2(pos, size);
    }

    private static int Arg(int[] args, int index, int fallback) => index < args.Length ? args[index] : fallback;
    private static int Unit(int raw) => raw > 100_000 ? raw & 0xFFFF : raw;
}

internal sealed class M4PictureEntry
{
    public int Kind;
    public string File = "";
    public string Text = "";
    public int[] Args = Array.Empty<int>();
    public int Anchor;
    public Vector2 Base;
}

internal sealed class M4RouteView : IGameSessionView
{
    public string Status { get; private set; } = "";
    public bool HasMessage { get; private set; }
    public string MessageText { get; private set; } = "";
    public string[] Choices { get; private set; } = Array.Empty<string>();

    public void SetStatus(string text) => Status = text;
    public void SetMapHud(int leaderX, int leaderY, string text) { }
    public void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY) { }
    public void ShowMessage(string text, int face) { HasMessage = true; MessageText = text; }
    public void HideMessage() => HasMessage = false;
    public void ShowChoices(string[] options, int selected) { Choices = options; }
    public void HideChoices() { Choices = Array.Empty<string>(); }
    public void ClearPictures() { }
    public void RestorePictures(IReadOnlyList<SavedPicture> pictures) { }
    public IReadOnlyList<SavedPicture> SnapshotPictures() => Array.Empty<SavedPicture>();
    public void RestoreAudio(IReadOnlyList<SavedAudio> audio) { }
    public IReadOnlyList<SavedAudio> SnapshotAudio() => Array.Empty<SavedAudio>();
}
