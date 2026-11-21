using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Shared driver for the M13 C# matrix: same program loading, message capture,
/// choice auto-pick and instance lifetime rules as ContentMatrixTest.
/// </summary>
internal sealed class M13Run
{
    public List<string> Messages { get; } = new();
    public int Unhandled { get; set; }
    public bool Faulted { get; set; }
    public string FaultReason { get; set; } = "";
    public bool Ended { get; set; }
    public VmController Vm { get; set; } = null!;
    public Instance Root { get; set; } = null!;
}

internal static class M13Harness
{
    public static string ProgramsPath()
    {
        string res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data", "programs.json");
    }

    public static VmController NewVm()
    {
        var vm = new VmController();
        vm.Vm.Host = vm;
        vm.StrictMode = true;
        if (!vm.LoadPrograms(ProgramsPath()))
            throw new Exception("M13: cannot load programs.json");
        return vm;
    }

    public static Instance StartAt(VmController vm, int eventId, int pc)
    {
        if (!vm.StartEventAt(eventId, pc))
            throw new Exception($"M13: CE{eventId}@{pc} failed to start");
        var inst = vm.Vm.Instances.FirstOrDefault(i => i.EventId == eventId);
        if (inst == null) throw new Exception($"M13: CE{eventId} instance missing");
        return inst;
    }

    public static M13Run DriveAt(int eventId, int pc, (int Slot, int Value)[]? selfs = null,
        (int Id, int Value)[]? globals = null, int maxFrames = 6000)
    {
        var vm = NewVm();
        var root = StartAt(vm, eventId, pc);
        foreach (var pair in selfs ?? Array.Empty<(int, int)>()) root.Self.Set(pair.Slot, pair.Value);
        foreach (var pair in globals ?? Array.Empty<(int, int)>()) vm.Vm.Vars.Set(pair.Id, pair.Value);

        var run = new M13Run { Vm = vm, Root = root };
        vm.MessageRequested += (text, face) =>
        {
            string clean = Clean(text);
            if (clean.Length > 0 && (run.Messages.Count == 0 || run.Messages[^1] != clean))
                run.Messages.Add(clean);
        };

        int frame = 0;
        vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
        for (; frame < maxFrames; frame++)
        {
            vm.StepFrame();
            if (vm.IsWaitingForMessage()) vm.DismissMessage();
            if (vm.IsWaitingForChoice()) vm.SelectChoice(0);
            if (!vm.Vm.Instances.Any(i => i.Id == root.Id))
            {
                run.Ended = true;
                break;
            }
        }

        run.Unhandled = vm.Vm.UnhandledOpcodeCount;
        run.Faulted = vm.Faulted;
        run.FaultReason = vm.FaultReason;
        return run;
    }

    public static Program Program(VmController vm, int eventId)
    {
        if (!vm.Vm.Programs.TryGetValue(eventId, out var program))
            throw new Exception($"M13: CE{eventId} missing");
        return program;
    }

    public static List<(int Var, int Value, int Op)> CondTriples(Program program, int pc)
    {
        if (pc < 0 || pc >= program.Slots.Count)
            throw new Exception($"M13: CE{program.Id} pc {pc} out of range");
        var slot = program.Slots[pc];
        if (slot.Opcode != 111)
            throw new Exception($"M13: CE{program.Id}@{pc} is opcode {slot.Opcode}, not 111");
        int count = slot.Params.Length > 0 ? slot.Params[0] & 0x0F : 0;
        var list = new List<(int, int, int)>(count);
        for (int i = 0; i < count; i++)
        {
            int o = 1 + 3 * i;
            list.Add((slot.Params[o], slot.Params[o + 1], slot.Params[o + 2]));
        }
        return list;
    }

    public static int FindGatePc(Program program, params (int Var, int Value, int Op)[] want)
    {
        for (int pc = 0; pc < program.Slots.Count; pc++)
        {
            var slot = program.Slots[pc];
            if (slot.Opcode != 111) continue;
            var got = CondTriples(program, pc);
            if (got.Count != want.Length) continue;
            bool same = true;
            for (int i = 0; i < want.Length; i++)
            {
                if (got[i] != want[i]) { same = false; break; }
            }
            if (same) return pc;
        }
        throw new Exception($"M13: CE{program.Id} gate {string.Join(",", want)} not found");
    }

    public static void GateAt(VmController vm, int eventId, int pc, params (int Var, int Value, int Op)[] want)
    {
        var got = CondTriples(Program(vm, eventId), pc);
        Check(got.Count == want.Length, $"CE{eventId}@{pc}: expected {want.Length} gate triples, got {got.Count}");
        for (int i = 0; i < want.Length; i++)
            Check(got[i] == want[i], $"CE{eventId}@{pc}: gate triple #{i} differs from M13_CONTENT_MATRIX");
    }

    public static int FindMessagePc(Program program, string needle)
    {
        for (int pc = 0; pc < program.Slots.Count; pc++)
        {
            var slot = program.Slots[pc];
            if (slot.Opcode == 101 && string.Join("", slot.Strings).Contains(needle, StringComparison.Ordinal))
                return pc;
        }
        throw new Exception($"M13: CE{program.Id} message '{needle}' not found");
    }

    public static string Clean(string text)
    {
        string t = text.Trim();
        if (t.StartsWith("\\E", StringComparison.Ordinal)) t = t[2..].Trim();
        return t;
    }

    public static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("M13_TEST_FAILED: " + message);
    }

    public static void Reached(string label, M13Run run, params string[] expected)
    {
        foreach (string expectedText in expected)
        {
            bool hit = run.Messages.Any(m => m.Contains(expectedText, StringComparison.Ordinal));
            Check(hit, $"{label}: expected '{expectedText}' not rendered; messages=[{string.Join(" | ", run.Messages)}]");
        }
        Check(run.Unhandled == 0, $"{label}: unhandled opcodes={run.Unhandled}");
        Check(!run.Faulted, $"{label}: faulted {run.FaultReason}");
    }

    public static void GateExists(VmController vm, int eventId, params (int Var, int Value, int Op)[] want)
    {
        var program = Program(vm, eventId);
        FindGatePc(program, want);
    }
}
