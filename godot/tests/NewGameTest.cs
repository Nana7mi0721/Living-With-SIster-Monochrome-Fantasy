using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Runs map 3 event 1 ("ニューゲーム処理"), the script the title screen transfers to when
/// the New Game row is chosen. It must clear the screen, hand over to the opening common
/// event (CE264) and then transfer the party to the bedroom.
/// </summary>
public partial class NewGameTest : Node
{
    private const string MapFile = "MapData/Map003.mps";
    private const int MapId = 3;
    private const int EventId = 1;
    private const int FrameBudget = 4000;

    private readonly List<string> _failures = new();
    private void Check(bool condition, string message)
    {
        if (!condition) _failures.Add(message);
    }

    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string programPath = Path.GetFullPath(Path.Combine(rootDir, "..", "game_data", "programs.json"));

        var commonEvents = ProgramLoader.LoadCommonEvents(programPath);
        var program = ProgramLoader.LoadMapEvents(programPath, MapFile, MapId)
            .FirstOrDefault(p => p.Id == EventId);
        if (program == null)
        {
            GD.PrintErr("[NewGameTest] FAIL: map 3 event 1 not found");
            GetTree().Quit(1);
            return;
        }

        GD.Print($"[NewGameTest] program '{program.Title}' len={program.Len} blocks={program.Blocks.Count}");

        var host = new RecordingHost();
        var vm = new EventVm { Host = host, StrictMode = true };
        foreach (var prog in commonEvents.Values) vm.RegisterProgram(prog);

        var inst = vm.CreateInstance(program)!;
        var childSteps = new Dictionary<int, int>();
        var visits = new Dictionary<int, int>();
        vm.StepHook = (i, pc) =>
        {
            host.Frame = vm.Frame;
            host.CurrentPc = pc;
            if (i.Id == inst.Id) visits[pc] = visits.GetValueOrDefault(pc) + 1;
            else childSteps[i.EventId] = childSteps.GetValueOrDefault(i.EventId) + 1;
        };

        int frames = 0;
        for (; frames < FrameBudget; frames++) vm.StepFrame();
        int Visits(int pc) => visits.GetValueOrDefault(pc);

        foreach (var line in host.Describe()) GD.Print("[NewGameTest] " + line);
        GD.Print($"[NewGameTest] frames={frames} titleVisits={visits.Count} " +
                 $"children=[{string.Join(",", childSteps.Select(kv => $"{kv.Key}:{kv.Value}"))}]");

        // The new game script first paints a black screen and opens the message window.
        Check(host.Drew("cg/blackscreen.jpg"), "blackscreen picture missing");
        Check(Visits(9) == 1, $"pc9 (blackscreen) visits {Visits(9)} != 1");
        Check(Visits(11) == 1, $"pc11 (2000000 = 1) visits {Visits(11)} != 1");

        // The opening hand-off: CE264 with three numeric arguments [4, 2002, 0] plus two
        // string words. ParamStatus 4131 = 0x1023 -> number_count 3, string_count 2, no
        // return slot, so the argument block ends right after the two string words.
        Check(host.Called(264), "CE264 opening call missing");
        var call264 = host.Calls.FirstOrDefault(c => c.Target == 264);
        Check(call264.Args != null && call264.Args.Length == 3 && call264.Args[0] == 4
              && call264.Args[1] == 2002 && call264.Args[2] == 0,
              "CE264 arguments not forwarded: " + (call264.Args == null ? "<none>" : string.Join(",", call264.Args)));
        Check(childSteps.ContainsKey(264) && childSteps[264] > 0, "CE264 child coroutine never ran");

        // And the party transfer into the bedroom.
        Check(host.Teleports.Any(t => t.X == 62 && t.Y == 38 && t.Options == 1),
              "teleport to the bedroom (62, 38) missing");
        // The opening chain now implements every opcode it meets (201/270 included), so
        // both the strict gate and the diagnostic counter must stay at zero.
        Check(host.Logs.Count == 0, "host logged: " + string.Join(" | ", host.Logs));
        Check(vm.UnhandledOpcodeCount == 0, $"new game chain saw {vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!vm.Faulted, "new game chain faulted the VM (" + vm.FaultReason + ")");

        if (_failures.Count > 0)
        {
            foreach (var f in _failures) GD.PrintErr("[NewGameTest] FAIL: " + f);
            GD.Print("NEW_GAME_TEST_FAILED");
            GetTree().Quit(1);
            return;
        }

        GD.Print("NEW_GAME_TEST_OK");
        GetTree().Quit(0);
    }
}
