using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Exports the instruction-level trace of map event 0 of SampleMapA in the
/// oracle/wolf.trace shape, so oracle/diff_trace.py can diff it against
/// `wolf-cli run --map SampleMapA --event 0 --frames 418 --step-trace ...`.
///
/// Every step carries the integer variables it changed and the host effects it
/// produced. Effects are drained when the next instruction starts (or when the
/// frame ends), which is the same moment the Rust VM finalizes its entry.
/// </summary>
public partial class OracleStepTraceTest : Node
{
    private const int Frames = 418;

    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string repo = Path.GetFullPath(Path.Combine(rootDir, ".."));
        string programsPath = Path.Combine(repo, "game_data", "programs.json");
        string outPath = Path.Combine(repo, "reports", "csharp_step_trace.json");
        string? dir = Path.GetDirectoryName(outPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        Program? program = ProgramLoader.LoadMapEvent(programsPath, "SampleMapA.mps", 0);
        if (program == null)
        {
            GD.PrintErr("[OracleStepTraceTest] FAIL: SampleMapA event 0 not found in programs.json");
            GetTree().Quit(1);
            return;
        }

        var host = new TraceRecorderHost();
        var vm = new EventVm { Host = host, StrictMode = true };
        foreach (var common in ProgramLoader.LoadCommonEvents(programsPath))
        {
            vm.RegisterProgram(common.Value);
        }

        var steps = new List<TraceStep>(4096);
        Dictionary<int, int> lastVars = new();
        Dictionary<int, string> lastStrs = new();
        Instance? lastInstance = null;
        vm.StepHook = (inst, pc) =>
        {
            if (steps.Count > 0)
            {
                Finish(steps[^1], lastVars, lastStrs, lastInstance, vm, host);
            }

            int op = pc < inst.Program.Slots.Count ? inst.Program.Slots[pc].Opcode : 0;
            steps.Add(new TraceStep
            {
                Step = steps.Count + 1,
                Frame = vm.Frame,
                EventId = inst.EventId,
                Pc = pc,
                Op = op,
            });
            lastVars = Snapshot(vm, inst);
            lastStrs = SnapshotStrings(vm, inst);
            lastInstance = inst;
        };

        if (vm.CreateInstance(program) == null)
        {
            GD.PrintErr("[OracleStepTraceTest] FAIL: could not start map event 0");
            GetTree().Quit(2);
            return;
        }

        for (int i = 0; i < Frames && vm.Instances.Count > 0; i++)
        {
            vm.StepFrame();
        }

        if (steps.Count > 0)
        {
            Finish(steps[^1], lastVars, lastStrs, lastInstance, vm, host);
        }

        var sb = new StringBuilder(steps.Count * 128 + 256);
        sb.Append("{\"schema\":\"wolf.trace\",\"schema_version\":1,\"seed\":0,");
        sb.Append("\"event_id\":0,\"event_name\":\"\",\"total_steps\":").Append(steps.Count).Append(",\"steps\":[");
        for (int i = 0; i < steps.Count; i++)
        {
            if (i > 0) sb.Append(',');
            var s = steps[i];
            sb.Append("{\"step\":").Append(i + 1)
              .Append(",\"frame\":").Append(s.Frame)
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

        int varsSteps = steps.Count(s => s.Vars.Count > 0);
        int effectSteps = steps.Count(s => s.Effects.Count > 0);
        GD.Print($"[OracleStepTraceTest] slots={program.Len} steps={steps.Count} frames={Frames} " +
                 $"vars_steps={varsSteps} effect_steps={effectSteps} " +
                 $"instances={vm.Instances.Count} faulted={vm.Faulted} unhandled={vm.UnhandledOpcodeCount}");
        GD.Print($"[OracleStepTraceTest] wrote {outPath}");

        if (steps.Count == 0)
        {
            GD.PrintErr("[OracleStepTraceTest] FAIL: no instructions executed");
            GetTree().Quit(3);
            return;
        }
        if (vm.UnhandledOpcodeCount != 0 || vm.Faulted)
        {
            GD.PrintErr($"[OracleStepTraceTest] FAIL: unhandled={vm.UnhandledOpcodeCount} fault={vm.FaultReason}");
            GetTree().Quit(4);
            return;
        }

        GD.Print($"ORACLE_STEP_TRACE_OK slots={program.Len} steps={steps.Count}");
        GetTree().Quit();
    }

    /// <summary>Every integer slot in trace addressing (ordinary, system, self).</summary>
    private static Dictionary<int, int> Snapshot(EventVm vm, Instance? inst)
    {
        var map = new Dictionary<int, int>();
        var nums = vm.Vars.SnapshotNums();
        for (int i = 0; i < nums.Count; i++)
        {
            map[i] = nums[i];
        }
        var sys = vm.Vars.SnapshotSysNums();
        for (int i = 0; i < sys.Count; i++)
        {
            map[9_000_000 + i] = sys[i];
        }
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

    /// <summary>Every non-empty string slot in trace addressing.</summary>
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
                               EventVm vm, TraceRecorderHost host)
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
