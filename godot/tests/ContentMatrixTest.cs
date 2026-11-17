using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Runtime half of the M13 condition-injection matrix: 18 cases (16 DLC random
/// events + 特4 two blocks) and 15 in-event opcode-111 gate payloads, all read from
/// oracle/fixtures/content_matrix_injected.json — the same fixture consumed by
/// crates/wolf-game/tests/m13_random_events_injected.rs (R10-A2 single source).
/// </summary>
public static class ContentMatrixTest
{
    private sealed class Case
    {
        public int Ce;
        public int[] Selfs = [];
        public int[] Globals = [];
        public int Pc;
        public int MaxFrames = 1200;
        public string Expected = "";
        public string Label = "";
        /// <summary>Matrix row that renders the same script block; not an independent case.</summary>
        public string AliasOf = "";
    }

    private sealed class Gate
    {
        public int Ce;
        public int Pc;
        public int[] Triples = [];
    }

    public static void Run()
    {
        using var doc = JsonDocument.Parse(File.ReadAllText(FixturePath()));
        var root = doc.RootElement;
        var cases = ParseCases(root.GetProperty("cases"));
        var gates = ParseGates(root.GetProperty("gates"));
        var externalGates = root.GetProperty("external_gates").EnumerateArray()
            .Select(e => e.GetString() ?? "").ToArray();

        Check(cases.Length == 18, $"expected 18 cases, got {cases.Length}");
        int unique = cases.Count(c => c.AliasOf.Length == 0);
        Check(unique == 17, $"expected 17 unique script blocks, got {unique}");
        foreach (var alias in cases.Where(c => c.AliasOf.Length > 0))
        {
            var baseCase = cases.FirstOrDefault(c => c.Label == alias.AliasOf);
            Check(baseCase != null, $"{alias.Label}: alias target {alias.AliasOf} missing");
            Check(alias.Ce == baseCase!.Ce && alias.Pc == baseCase.Pc && alias.Expected == baseCase.Expected
                  && alias.Selfs.SequenceEqual(baseCase.Selfs) && alias.Globals.SequenceEqual(baseCase.Globals),
                $"{alias.Label}: aliased row diverged from {alias.AliasOf}");
        }
        foreach (string label in externalGates)
        {
            Check(cases.Any(c => c.Label == label && c.AliasOf.Length == 0),
                $"external-gate row {label} missing from the case table");
        }

        var path = Path.Combine(DataDirectory(), "programs.json");
        foreach (var gate in gates) CheckGate(path, gate);

        foreach (var c in cases)
        {
            Drive(path, c);
        }

        GD.Print($"CONTENT_MATRIX_TEST_OK cases={cases.Length} unique={unique} gates={gates.Length} external={externalGates.Length}");
    }

    private static Case[] ParseCases(JsonElement array)
    {
        var list = new List<Case>();
        foreach (var item in array.EnumerateArray())
        {
            var c = new Case
            {
                Ce = item.GetProperty("ce").GetInt32(),
                Pc = item.GetProperty("pc").GetInt32(),
                Expected = item.GetProperty("expected").GetString() ?? "",
                Label = item.GetProperty("label").GetString() ?? "",
                MaxFrames = item.TryGetProperty("max_frames", out var mf) ? mf.GetInt32() : 1200,
                AliasOf = item.TryGetProperty("alias_of", out var ao) ? ao.GetString() ?? "" : "",
            };
            c.Selfs = FlattenPairs(item.GetProperty("self"));
            c.Globals = FlattenPairs(item.GetProperty("globals"));
            list.Add(c);
        }
        return list.ToArray();
    }

    private static int[] FlattenPairs(JsonElement array)
    {
        var values = new List<int>();
        foreach (var pair in array.EnumerateArray())
        {
            values.Add(pair[0].GetInt32());
            values.Add(pair[1].GetInt32());
        }
        return values.ToArray();
    }

    private static Gate[] ParseGates(JsonElement array)
    {
        var list = new List<Gate>();
        foreach (var item in array.EnumerateArray())
        {
            var gate = new Gate
            {
                Ce = item.GetProperty("ce").GetInt32(),
                Pc = item.GetProperty("pc").GetInt32(),
            };
            var triples = new List<int>();
            foreach (var triple in item.GetProperty("triples").EnumerateArray())
            {
                triples.Add(triple[0].GetInt32());
                triples.Add(triple[1].GetInt32());
                triples.Add(triple[2].GetInt32());
            }
            gate.Triples = triples.ToArray();
            list.Add(gate);
        }
        return list.ToArray();
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
        for (; frame < c.MaxFrames; frame++)
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

    private static void CheckGate(string programsPath, Gate gate)
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

    private static string FixturePath()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "oracle", "fixtures", "content_matrix_injected.json");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"CONTENT_MATRIX_TEST_FAILED: {message}");
    }
}
