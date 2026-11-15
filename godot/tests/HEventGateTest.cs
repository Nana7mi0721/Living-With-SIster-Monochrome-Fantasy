using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of the data-side assertions in crates/wolf-game/tests/m13_h_thresholds.rs:
/// every documented H gate (CE310/318/323/335) has to keep its exact opcode-111
/// payload, and each single-condition threshold must flip at threshold-1 / threshold.
/// Multi-condition gates are first-true chains, so their fallback tier is checked too.
/// This covers the bytecode/data baseline; the H scene bodies themselves still need
/// an end-to-end drive (CE318 -> CE360) before the H wiring can be called done.
/// </summary>
public static class HEventGateTest
{
    private static readonly (int Event, int Pc, int[] Ints)[] Gates =
    [
        // 周末 H — CE310 @159..165, four separate menu gates.
        (310, 159, [17, 2_000_016, 1, 1]),
        (310, 161, [1, 2_000_022, 200, 1]),
        (310, 163, [17, 2_000_018, 40, 1]),
        (310, 165, [17, 2_000_424, 3, 1]),
        // 泳池/海边 — CE310 @731..743.
        (310, 731, [1, 2_000_265, 1, 2]),
        (310, 733, [1, 2_000_455, 1, 1]),
        (310, 735, [1, 2_000_424, 1, 1]),
        (310, 739, [17, 2_000_022, 120, 1]),
        (310, 741, [17, 2_000_102, 30, 1]),
        (310, 1099, [1, 2_000_102, 60, 1]),
        // 浴室 — CE318 trust tiers 100/120/150 and two 2-case gates.
        (318, 213, [17, 2_000_021, 100, 1]),
        (318, 217, [17, 2_000_021, 100, 1]),
        (318, 227, [17, 2_000_021, 120, 1]),
        (318, 237, [17, 2_000_021, 150, 1]),
        (318, 338, [2, 2_000_103, 2, 1, 2_000_102, 75, 1]),
        (318, 415, [2, 2_000_103, 2, 1, 2_000_102, 75, 1]),
        // 厕所 — CE323.
        (323, 1055, [2, 2_000_021, 200, 1, 2_000_021, 0, 1]),
        (323, 1058, [2, 2_000_023, 5, 1, 2_000_023, 0, 1]),
        (323, 1060, [3, 2_000_022, 250, 1, 2_000_022, 150, 1, 2_000_022, 0, 1]),
        (323, 1062, [2, 2_000_222, 0, 2, 2_000_222, 1, 1]),
        (323, 1849, [2, 2_000_131, 10, 1, 2_000_131, 0, 1]),
        (323, 1913, [2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        // 夜袭 — CE335.
        (335, 3100, [2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3416, [2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3444, [2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3470, [2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3102, [2, 2_000_102, 60, 1, 2_000_102, 0, 1]),
        (335, 3230, [2, 2_000_102, 60, 1, 2_000_102, 0, 1]),
        (335, 3457, [2, 2_000_102, 60, 1, 2_000_102, 0, 1]),
        (335, 2561, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 2703, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3063, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3418, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3446, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3472, [2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3429, [2, 2_000_102, 50, 1, 2_000_102, 0, 1]),
        (335, 3483, [2, 2_000_102, 70, 1, 2_000_102, 0, 1]),
    ];

    public static void Run()
    {
        var vm = new VmController();
        Check(vm.LoadPrograms(Path.Combine(DataDirectory(), "programs.json")),
            "programs failed to load");

        int boundaries = 0;
        foreach (var gate in Gates)
        {
            Check(vm.Vm.Programs.TryGetValue(gate.Event, out var program),
                $"CE{gate.Event} missing");
            Check(gate.Pc < program!.Len, $"CE{gate.Event}@{gate.Pc} outside program");
            var slot = program.Slots[gate.Pc];
            Check(slot.Opcode == 111,
                $"CE{gate.Event}@{gate.Pc} must be opcode 111, got {slot.Opcode}");
            Check(slot.Params.Length == gate.Ints.Length,
                $"CE{gate.Event}@{gate.Pc} payload length {slot.Params.Length}, want {gate.Ints.Length}");
            for (int i = 0; i < gate.Ints.Length; i++)
            {
                Check(slot.Params[i] == gate.Ints[i],
                    $"CE{gate.Event}@{gate.Pc} param[{i}]={slot.Params[i]}, want {gate.Ints[i]}");
            }

            // Single-condition gates: threshold-1 must fall through, threshold must take case 1.
            if ((gate.Ints[0] & 0x0F) == 1)
            {
                CheckBoundary(vm, gate, gate.Ints[1] - 2_000_000, gate.Ints[2], true);
                CheckBoundary(vm, gate, gate.Ints[1] - 2_000_000, gate.Ints[2], false);
                boundaries += 2;
            }
        }

        // Multi-condition chains: the first true condition wins, else the fallback tier.
        AssertCase(vm, 318, 338, new Dictionary<int, int> { [103] = 1, [102] = 74 }, -1);
        AssertCase(vm, 318, 338, new Dictionary<int, int> { [103] = 2, [102] = 74 }, 0);
        AssertCase(vm, 318, 338, new Dictionary<int, int> { [103] = 1, [102] = 75 }, 1);
        AssertCase(vm, 318, 415, new Dictionary<int, int> { [103] = 1, [102] = 74 }, -1);
        AssertCase(vm, 318, 415, new Dictionary<int, int> { [103] = 2, [102] = 74 }, 0);
        AssertCase(vm, 318, 415, new Dictionary<int, int> { [103] = 1, [102] = 75 }, 1);
        AssertCase(vm, 323, 1055, new Dictionary<int, int> { [21] = 199 }, 1);
        AssertCase(vm, 323, 1055, new Dictionary<int, int> { [21] = 200 }, 0);
        AssertCase(vm, 323, 1058, new Dictionary<int, int> { [23] = 4 }, 1);
        AssertCase(vm, 323, 1058, new Dictionary<int, int> { [23] = 5 }, 0);
        AssertCase(vm, 323, 1060, new Dictionary<int, int> { [22] = 0 }, 2);
        AssertCase(vm, 323, 1060, new Dictionary<int, int> { [22] = 149 }, 2);
        AssertCase(vm, 323, 1060, new Dictionary<int, int> { [22] = 150 }, 1);
        AssertCase(vm, 323, 1060, new Dictionary<int, int> { [22] = 249 }, 1);
        AssertCase(vm, 323, 1060, new Dictionary<int, int> { [22] = 250 }, 0);
        AssertCase(vm, 323, 1062, new Dictionary<int, int> { [222] = 0 }, 0);
        AssertCase(vm, 323, 1062, new Dictionary<int, int> { [222] = 1 }, 1);
        AssertCase(vm, 323, 1849, new Dictionary<int, int> { [131] = 9 }, 1);
        AssertCase(vm, 323, 1849, new Dictionary<int, int> { [131] = 10 }, 0);
        AssertCase(vm, 323, 1913, new Dictionary<int, int> { [22] = 199 }, 1);
        AssertCase(vm, 323, 1913, new Dictionary<int, int> { [22] = 200 }, 0);
        AssertCase(vm, 335, 3100, new Dictionary<int, int> { [22] = 199 }, 1);
        AssertCase(vm, 335, 3100, new Dictionary<int, int> { [22] = 200 }, 0);
        AssertCase(vm, 335, 3102, new Dictionary<int, int> { [102] = 59 }, 1);
        AssertCase(vm, 335, 3102, new Dictionary<int, int> { [102] = 60 }, 0);
        AssertCase(vm, 335, 2561, new Dictionary<int, int> { [102] = 39 }, 1);
        AssertCase(vm, 335, 2561, new Dictionary<int, int> { [102] = 40 }, 0);
        AssertCase(vm, 335, 3429, new Dictionary<int, int> { [102] = 49 }, 1);
        AssertCase(vm, 335, 3429, new Dictionary<int, int> { [102] = 50 }, 0);
        AssertCase(vm, 335, 3483, new Dictionary<int, int> { [102] = 69 }, 1);
        AssertCase(vm, 335, 3483, new Dictionary<int, int> { [102] = 70 }, 0);
        boundaries += 31;

        GD.Print($"H_EVENT_GATE_TEST_OK gates={Gates.Length} boundaries={boundaries} data_only=1");
    }

    private static void CheckBoundary(VmController vm, (int Event, int Pc, int[] Ints) gate,
        int var, int value, bool below)
    {
        int v = below ? value - 1 : value;
        for (int i = 0; i < (gate.Ints[0] & 0x0F); i++)
        {
            vm.SetVar(gate.Ints[1 + 3 * i] - 2_000_000, 0);
        }
        vm.SetVar(var, v);
        int selected = FirstTrueCase(gate.Ints, vm.GetVar);
        int want = below ? -1 : 0;
        Check(selected == want,
            $"CE{gate.Event}@{gate.Pc} var{var}={v} selected case {selected}, want {want}");
    }

    private static void AssertCase(VmController vm, int eventId, int pc,
        Dictionary<int, int> vars, int want)
    {
        var slot = vm.Vm.Programs[eventId].Slots[pc];
        for (int i = 0; i < (slot.Params[0] & 0x0F); i++)
        {
            vm.SetVar(slot.Params[1 + 3 * i] - 2_000_000, 0);
        }
        foreach (var pair in vars) vm.SetVar(pair.Key, pair.Value);
        int selected = FirstTrueCase(slot.Params, vm.GetVar);
        Check(selected == want,
            $"CE{eventId}@{pc} vars selected case {selected}, want {want}");
    }

    private static int FirstTrueCase(int[] ints, Func<int, int> read)
    {
        int count = ints[0] & 0x0F;
        for (int i = 0; i < count; i++)
        {
            int lhs = read(ints[1 + 3 * i] - 2_000_000);
            int rhs = ints[2 + 3 * i];
            if (Compare(lhs, rhs, ints[3 + 3 * i])) return i;
        }
        return -1;
    }

    private static bool Compare(int lhs, int rhs, int op) => op switch
    {
        0 => lhs > rhs,
        1 => lhs >= rhs,
        2 => lhs == rhs,
        3 => lhs <= rhs,
        4 => lhs < rhs,
        5 => lhs != rhs,
        6 => (lhs & rhs) != 0,
        _ => lhs == rhs,
    };

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"H_EVENT_GATE_TEST_FAILED: {message}");
    }
}
