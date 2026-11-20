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
/// Runs the production GameSession path (TitleMap event 2 -> new game -> SampleMapA 1005)
/// with scripted input and exports a wolf.route trace. The Rust side runs the
/// same frames through wolf-cli route; oracle/diff_route.py compares the two.
/// </summary>
public partial class RouteTraceTest : Node
{
    private const int Frames = 1500;
    private const string RouteName = "title_newgame_map1005";

    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string repo = Path.GetFullPath(Path.Combine(rootDir, ".."));
        string programsPath = Path.Combine(repo, "game_data", "programs.json");
        string outPath = Path.Combine(repo, "reports", "csharp_route_trace.json");
        string? dir = Path.GetDirectoryName(outPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        var view = new RouteView();
        var bridge = new VmController();
        AddChild(bridge);
        bridge.KeyCodeProvider = _ => KeyCodes.CodeOk;
        bridge.StrictMode = true;
        if (!bridge.LoadPrograms(programsPath))
        {
            GD.PrintErr("[RouteTraceTest] FAIL: cannot load programs.json");
            GetTree().Quit(1);
            return;
        }

        var host = new RouteHost(bridge);
        bridge.Vm.Host = host;

        // Same isolation as M4FullRouteTraceTest: a reports/ save dir accumulates the port's
        // system save (and its .bak rotation, which SaveCodec falls back to), which changes
        // the 221 reads the oracle compares against.
        string routeSaveDir = Path.Combine(Path.GetTempPath(), "lwsmf-route-trace-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(routeSaveDir);
        var session = new GameSession(bridge, view, programsPath, Path.Combine(routeSaveDir, "unused_save.json"));
        string routeSystemSavePath = Path.Combine(routeSaveDir, "system.json");

        session.ScriptedInput = true;
        session.AutoPickChoice = true;
        bridge.MessageRequested += (text, face) => session.ShowMessage(text, face);
        bridge.MessageClosed += () => session.NotifyMessageClosed();
        bridge.ChoicesRequested += options => session.ShowChoices(options);
        bridge.TeleportRequested += (target, map, x, y, options) =>
            session.QueueTeleport(target, map, x, y, options);

        var steps = new List<TraceStep>(8192);
        var mapIds = new List<int>(8192);
        Dictionary<int, int> lastVars = new();
        Dictionary<int, string> lastStrs = new();
        Instance? lastInstance = null;
        bridge.Vm.StepHook = (inst, pc) =>
        {
            if (steps.Count > 0)
            {
                Finish(steps[^1], lastVars, lastStrs, lastInstance, bridge.Vm, host);
            }

            int op = pc < inst.Program.Slots.Count ? inst.Program.Slots[pc].Opcode : 0;
            steps.Add(new TraceStep
            {
                Step = steps.Count + 1,
                Frame = bridge.Vm.Frame,
                EventId = inst.EventId,
                Pc = pc,
                Op = op,
            });
            mapIds.Add(session.MapData?.Id ?? -1);
            lastVars = Snapshot(bridge.Vm, inst);
            lastStrs = SnapshotStrings(bridge.Vm, inst);
            lastInstance = inst;
        };

        if (!session.StartTitle())
        {
            GD.PrintErr("[RouteTraceTest] FAIL: TitleMap event 2 missing");
            GetTree().Quit(2);
            return;
        }

        for (int i = 0; i < Frames; i++)
        {
            session.AdvanceFrame();
        }

        if (steps.Count > 0)
        {
            Finish(steps[^1], lastVars, lastStrs, lastInstance, bridge.Vm, host);
        }

        // Stop tracing before the save/load leg: the oracle compares exactly the
        // scripted route frames, while save/load is verified as a state round-trip.
        bridge.Vm.StepHook = null;

        // The route's final leg: the real GameSession must round-trip its full
        // state (map, leader, frames, vars, pictures, audio) through the save codec.
        int framesAtSave = session.Frames;
        int leaderXAtSave = session.LeaderX;
        int leaderYAtSave = session.LeaderY;
        int mapAtSave = session.MapData?.Id ?? -1;
        bool saved = session.SaveGame();
        session.AdvanceFrame();
        session.AdvanceFrame();
        bool loaded = session.LoadGame();
        bool saveOk = saved && loaded
            && session.Frames == framesAtSave
            && session.LeaderX == leaderXAtSave
            && session.LeaderY == leaderYAtSave
            && (session.MapData?.Id ?? -1) == mapAtSave;
        if (File.Exists(session.SavePath)) File.Delete(session.SavePath);
        if (File.Exists(routeSystemSavePath)) File.Delete(routeSystemSavePath);
        try { if (Directory.Exists(routeSaveDir)) Directory.Delete(routeSaveDir, true); } catch (IOException) { }
        if (!saveOk)
        {
            GD.PrintErr($"[RouteTraceTest] FAIL: save/load saved={saved} loaded={loaded} "
                + $"frames={session.Frames}/{framesAtSave} "
                + $"leader={session.LeaderX},{session.LeaderY}");
            GetTree().Quit(3);
            return;
        }

        var sb = new StringBuilder(steps.Count * 160 + 256);
        sb.Append("{\"schema\":\"wolf.route\",\"schema_version\":1,");
        sb.Append("\"route\":\"").Append(RouteName).Append("\",\"frames\":").Append(Frames);
        sb.Append(",\"total_steps\":").Append(steps.Count).Append(",\"steps\":[");
        for (int i = 0; i < steps.Count; i++)
        {
            if (i > 0) sb.Append(',');
            var s = steps[i];
            sb.Append("{\"step\":").Append(i + 1)
              .Append(",\"frame\":").Append(s.Frame)
              .Append(",\"map_id\":").Append(mapIds[i])
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

        int effects = steps.Count(s => s.Effects.Count > 0);
        GD.Print($"[RouteTraceTest] steps={steps.Count} frames={Frames} " +
                 $"effects_steps={effects} phase={session.Phase} " +
                 $"map={session.MapData?.Id ?? -1} leader={session.LeaderX},{session.LeaderY} " +
                 $"faulted={bridge.Vm.Faulted} unhandled={bridge.Vm.UnhandledOpcodeCount}");
        if (steps.Count == 0 || bridge.Vm.UnhandledOpcodeCount != 0 || bridge.Vm.Faulted)
        {
            GD.PrintErr($"[RouteTraceTest] FAIL: steps={steps.Count} unhandled={bridge.Vm.UnhandledOpcodeCount} fault={bridge.Vm.FaultReason}");
            GetTree().Quit(3);
            return;
        }
        if (session.Phase != "bedroom" || (session.MapData?.Id ?? -1) != 1005)
        {
            GD.PrintErr($"[RouteTraceTest] FAIL: route did not reach map 1005 (phase={session.Phase} map={session.MapData?.Id ?? -1})");
            GetTree().Quit(4);
            return;
        }

        GD.Print($"ROUTE_TRACE_TEST_OK steps={steps.Count} map={session.MapData?.Id ?? -1} "
            + $"leader={session.LeaderX},{session.LeaderY} save_load=ok");
        GetTree().Quit();
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

    private static void Finish(TraceStep step, Dictionary<int, int> before,
                               Dictionary<int, string> beforeStrs, Instance? previous,
                               EventVm vm, RouteHost host)
    {
        var after = Snapshot(vm, previous);
        foreach (var pair in after)
        {
            if (!before.TryGetValue(pair.Key, out int old) ? pair.Value != 0 : old != pair.Value)
            {
                step.Vars[pair.Key] = pair.Value;
            }
        }
        foreach (var pair in before)
        {
            if (pair.Value != 0 && !after.ContainsKey(pair.Key))
            {
                step.Vars[pair.Key] = 0;
            }
        }
        var afterStrs = SnapshotStrings(vm, previous);
        foreach (var pair in afterStrs)
        {
            if (!beforeStrs.TryGetValue(pair.Key, out string? old) || old != pair.Value)
            {
                step.StrVars[pair.Key] = pair.Value;
            }
        }
        foreach (var pair in beforeStrs)
        {
            if (!string.IsNullOrEmpty(pair.Value) && !afterStrs.ContainsKey(pair.Key))
            {
                step.StrVars[pair.Key] = "";
            }
        }
        foreach (var effect in host.PendingEffects)
        {
            step.Effects.Add(effect);
        }
        host.PendingEffects.Clear();
    }
}

/// <summary>Forwards every VM callback to the real VmController while recording effects.</summary>
internal sealed class RouteHost : IHost
{
    private readonly VmController _inner;
    private readonly Dictionary<int, RoutePictureEntry> _pictures = new();

    public List<JsonObject> PendingEffects { get; } = new();

    public RouteHost(VmController inner)
    {
        _inner = inner;
    }

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
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "call",
            ["target_event_id"] = target,
            ["args"] = array
        });
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
            ["type"] = cmd.Channel switch
            {
                SoundChannel.Bgm => "bgm",
                SoundChannel.Bgs => "bgs",
                _ => "se"
            },
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
        PendingEffects.Add(new JsonObject
        {
            ["kind"] = "effect",
            ["type"] = kind,
            ["duration"] = duration,
            ["args"] = array
        });
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
            var merged = new RoutePictureEntry
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
        var entry = new RoutePictureEntry
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

    private static Vector2 BaseSize(RoutePictureEntry entry)
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

    private Rect2 HitRect(RoutePictureEntry entry)
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

internal sealed class RoutePictureEntry
{
    public int Kind;
    public string File = "";
    public string Text = "";
    public int[] Args = Array.Empty<int>();
    public int Anchor;
    public Vector2 Base;
}

/// <summary>Headless GameSession view: keeps dialogue state, ignores the rest.</summary>
internal sealed class RouteView : IGameSessionView
{
    public string Status { get; private set; } = "";
    public bool HasMessage { get; private set; }
    public string MessageText { get; private set; } = "";

    public void SetStatus(string text) => Status = text;
    public void SetMapHud(int leaderX, int leaderY, string text) { }
    public void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY) { }
    public void ShowMessage(string text, int face)
    {
        HasMessage = true;
        MessageText = text;
    }
    public void HideMessage() => HasMessage = false;
    public void ShowChoices(string[] options, int selected) { }
    public void HideChoices() { }
    public void ClearPictures() { }
    public void RestorePictures(IReadOnlyList<SavedPicture> pictures) { }
    public IReadOnlyList<SavedPicture> SnapshotPictures() => Array.Empty<SavedPicture>();
    public void RestoreAudio(IReadOnlyList<SavedAudio> audio) { }
    public IReadOnlyList<SavedAudio> SnapshotAudio() => Array.Empty<SavedAudio>();
}
