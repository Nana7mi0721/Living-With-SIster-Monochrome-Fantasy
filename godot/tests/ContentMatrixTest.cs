using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of crates/wolf-game/tests/m13_random_events_injected.rs: the 17
/// condition-injection entries of docs/research/M13_CONTENT_MATRIX.md §1.2
/// (16 DLC random events + 特4 seal). The natural 111 random pool and the
/// nine endings remain Rust-matrix coverage; this test only pins the C# runtime
/// rendering of the injected branches and their opcode-111 gate payloads.
/// </summary>
public static class ContentMatrixTest
{
    private sealed class Case
    {
        public int Ce;
        public int[] Selfs = [];
        public int[] Globals = [];
        public int Pc;
        public string Expected = "";
        public string Label = "";
    }

    private static readonly Case[] Cases =
    [
        new() { Ce = 460, Selfs = [610, 1], Globals = [619, 1], Pc = 5, Expected = "慰劳品", Label = "#10" },
        new() { Ce = 465, Selfs = [], Globals = [42, 20, 593, 1, 619, 1], Pc = 104, Expected = "由衣，你没事吧？", Label = "#28" },
        new() { Ce = 452, Selfs = [], Globals = [619, 1], Pc = 581, Expected = "向熊镇", Label = "#39" },
        new() { Ce = 461, Selfs = [610, 4, 620, 0], Globals = [619, 1], Pc = 1081, Expected = "小猫", Label = "#45" },
        new() { Ce = 452, Selfs = [], Globals = [619, 1], Pc = 2983, Expected = "烤曼陀罗草", Label = "#54" },
        new() { Ce = 461, Selfs = [], Globals = [619, 1], Pc = 1355, Expected = "东西基本买好了", Label = "#58" },
        new() { Ce = 461, Selfs = [610, 1], Globals = [619, 1], Pc = 1314, Expected = "被坏心眼的客人缠上了", Label = "#59" },
        new() { Ce = 452, Selfs = [], Globals = [619, 1], Pc = 4750, Expected = "由香莉姐还在睡", Label = "#62" },
        new() { Ce = 461, Selfs = [623, 0], Globals = [565, 1, 619, 1], Pc = 258, Expected = "和香澄美一起寻找素材了", Label = "#63" },
        new() { Ce = 461, Selfs = [622, 0], Globals = [573, 1, 619, 1], Pc = 43, Expected = "与由衣的亲密度上升了", Label = "#65" },
        new() { Ce = 449, Selfs = [], Globals = [619, 1], Pc = 45, Expected = "源头很可能是", Label = "#69" },
        new() { Ce = 461, Selfs = [], Globals = [11, 10, 619, 1], Pc = 1409, Expected = "用炸弹把委托人都炸飞了", Label = "#76" },
        new() { Ce = 461, Selfs = [621, 0], Globals = [619, 1], Pc = 460, Expected = "和加奈一起锻炼了", Label = "#80" },
        new() { Ce = 461, Selfs = [610, 4, 620, 0], Globals = [619, 1], Pc = 1081, Expected = "喵——", Label = "#84" },
        new() { Ce = 461, Selfs = [623, 0], Globals = [565, 1, 619, 1], Pc = 258, Expected = "和香澄美一起寻找素材了", Label = "#101" },
        new() { Ce = 461, Selfs = [610, 8], Globals = [619, 1], Pc = 1433, Expected = "今天可真忙啊", Label = "#111" },
        new() { Ce = 437, Selfs = [611, 0], Globals = [533, 1, 535, 0], Pc = 22, Expected = "海豹驱除作战", Label = "特4-a" },
        new() { Ce = 438, Selfs = [], Globals = [533, 1, 449, 1], Pc = 82, Expected = "海豹跃出水面", Label = "特4-b" },
    ];

    // (ce, pc, [var, threshold, comparison]...) — comparison 0 >, 1 >=, 2 ==.
    private static readonly (int Ce, int Pc, int[] Triples)[] Gates =
    [
        (460, 1, [1_600_010, 1, 2]),
        (465, 97, [2_000_042, 10, 1]),
        (465, 95, [2_000_593, 1, 2]),
        (461, 1079, [1_600_010, 4, 2, 1_600_010, 5, 2, 1_600_010, 6, 2, 1_600_010, 7, 2]),
        (461, 1082, [1_600_020, 0, 2, 1_600_020, 1, 2, 1_600_020, 2, 2]),
        (461, 1320, [1_600_010, 1, 2, 1_600_010, 2, 2, 1_600_010, 3, 2]),
        (461, 256, [2_000_565, 1, 2]),
        (461, 259, [1_600_023, 0, 2, 1_600_023, 1, 2, 1_600_023, 2, 2]),
        (461, 41, [2_000_573, 1, 2, 2_000_573, 2, 2, 2_000_573, 3, 2]),
        (461, 44, [1_600_022, 0, 2, 1_600_022, 1, 2, 1_600_022, 2, 2]),
        (461, 1374, [2_000_011, 90, 1, 2_000_011, 60, 1, 2_000_011, 30, 1, 2_000_011, 0, 1]),
        (461, 461, [1_600_021, 0, 2, 1_600_021, 1, 2, 1_600_021, 2, 2, 1_600_021, 3, 1]),
        (461, 1431, [1_600_010, 8, 2, 1_600_010, 9, 2, 1_600_010, 10, 2]),
        (437, 10, [2_000_535, 0, 2]),
        (438, 32, [2_000_449, 1, 1]),
    ];

    public static void Run()
    {
        Check(Cases.Length == 18, $"expected 18 cases, got {Cases.Length}");
        var path = Path.Combine(DataDirectory(), "programs.json");
        foreach (var gate in Gates) CheckGate(path, gate);

        foreach (var c in Cases)
        {
            Drive(path, c);
        }

        GD.Print($"CONTENT_MATRIX_TEST_OK cases={Cases.Length} gates={Gates.Length}");
    }

    private static void Drive(string programsPath, Case c)
    {
        var vm = new VmController();
        vm.Vm.Host = vm;
        Check(vm.LoadPrograms(programsPath), $"CE{c.Ce} programs failed to load");
        Check(!vm.Faulted, $"CE{c.Ce} faulted before start");

        var messages = new List<string>();
        vm.MessageRequested += (text, face) =>
        {
            var clean = Clean(text);
            if (clean.Length > 0 && (messages.Count == 0 || messages[^1] != clean))
            {
                messages.Add(clean);
            }
        };

        Check(vm.StartEventAt(c.Ce, c.Pc), $"CE{c.Ce}@{c.Pc} failed to start");
        Instance? inst = null;
        foreach (var candidate in vm.Vm.Instances)
        {
            if (candidate.EventId == c.Ce) { inst = candidate; break; }
        }
        Check(inst != null, $"CE{c.Ce} instance missing after start");
        for (int i = 0; i + 1 < c.Selfs.Length; i += 2)
        {
            inst!.Self.Set(c.Selfs[i], c.Selfs[i + 1]);
        }
        for (int i = 0; i + 1 < c.Globals.Length; i += 2)
        {
            vm.Vm.Vars.Set(c.Globals[i], c.Globals[i + 1]);
        }

        int frame = 0;
        vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
        for (; frame < 1200; frame++)
        {
            vm.StepFrame();
            if (vm.IsWaitingForMessage()) vm.DismissMessage();
            if (vm.IsWaitingForChoice()) vm.SelectChoice(0);
            if (!vm.HasEvent(c.Ce)) break;
        }

        string expected = c.Expected;
        bool hit = messages.Exists(m => m.Contains(expected, StringComparison.Ordinal));
        Check(hit,
            $"{c.Label} CE{c.Ce}@{c.Pc}: expected {expected} not rendered; messages=[{string.Join(" | ", messages)}]");
        Check(vm.Vm.UnhandledOpcodeCount == 0,
            $"{c.Label} CE{c.Ce}@{c.Pc}: unhandled opcodes={vm.Vm.UnhandledOpcodeCount}");
        Check(!vm.Faulted, $"{c.Label} CE{c.Ce}@{c.Pc}: faulted {vm.FaultReason}");
    }

    private static void CheckGate(string programsPath, (int Ce, int Pc, int[] Triples) gate)
    {
        var vm = new VmController();
        Check(vm.LoadPrograms(programsPath), "programs failed to load for gate check");
        Check(vm.Vm.Programs.TryGetValue(gate.Ce, out var program), $"CE{gate.Ce} missing");
        var slot = program!.Slots[gate.Pc];
        Check(slot.Opcode == 111, $"CE{gate.Ce}@{gate.Pc} must be opcode 111, got {slot.Opcode}");
        int count = slot.Params[0] & 0x0F;
        int expectedCount = gate.Triples.Length / 3;
        Check(count == expectedCount,
            $"CE{gate.Ce}@{gate.Pc} case count {count}, want {expectedCount}");
        for (int i = 0; i < expectedCount; i++)
        {
            for (int j = 0; j < 3; j++)
            {
                int got = slot.Params[1 + 3 * i + j];
                int want = gate.Triples[3 * i + j];
                Check(got == want,
                    $"CE{gate.Ce}@{gate.Pc} triple[{i}][{j}]={got}, want {want}");
            }
        }
    }

    private static string Clean(string text)
    {
        var t = text.Trim();
        if (t.StartsWith("\\E", StringComparison.Ordinal)) t = t[2..].Trim();
        return t;
    }

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"CONTENT_MATRIX_TEST_FAILED: {message}");
    }
}
