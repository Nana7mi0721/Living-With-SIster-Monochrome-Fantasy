using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Runs TitleMap event 2 - the real title screen ("新タイトル用イベント") - headless and
/// asserts the behaviour the Rust oracle describes: the picture calls, the counted loop,
/// the choice block, the key poll, the label jump, the door SE and the call into CE42.
/// </summary>
public partial class TitleTest : Node
{
    private const string TitleMapFile = "MapData/TitleMap.mps";
    private const int TitleMapId = 1008;
    private const int TitleEventId = 2;
    private const int FrameBudget = 4000;

    private sealed class TitleHost : RecordingHost
    {
        public int KeyPolls;

        // Only pc95/pc96 are the title's own key poll; the splash CE549 also polls keys to
        // skip, and answering there would swallow the scripted press.
        public override int KeyCode(uint mask)
        {
            KeyPolls++;
            return CurrentPc == 95 || CurrentPc == 96 ? 20 : 0;
        }
    }

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
        Check(commonEvents.Count == 550, $"common event count {commonEvents.Count} != 550");

        var titleProgram = ProgramLoader.LoadMapEvents(programPath, TitleMapFile, TitleMapId)
            .FirstOrDefault(p => p.Id == TitleEventId);
        if (titleProgram == null)
        {
            GD.PrintErr("[TitleTest] FAIL: title program not found");
            GetTree().Quit(1);
            return;
        }

        GD.Print($"[TitleTest] title program '{titleProgram.Title}' len={titleProgram.Len} " +
                 $"blocks={titleProgram.Blocks.Count} labels={titleProgram.Labels.Count} selfBase={titleProgram.SelfBase}");
        Check(titleProgram.Len == 133, $"title program length {titleProgram.Len} != 133");
        Check(titleProgram.Blocks.Count == 9, $"title block count {titleProgram.Blocks.Count} != 9");
        Check(titleProgram.Labels.ContainsKey("A"), "label A missing");
        Check(titleProgram.SelfBase == 100, $"self base {titleProgram.SelfBase} != 100");

        var host = new TitleHost();
        var vm = new EventVm { Host = host, StrictMode = true };
        foreach (var prog in commonEvents.Values) vm.RegisterProgram(prog);

        var inst = vm.CreateInstance(titleProgram)!;
        var childSteps = new Dictionary<int, int>();
        var visits = new Dictionary<int, int>();
        int titleSteps = 0;

        vm.StepHook = (i, pc) =>
        {
            host.Frame = vm.Frame;
            host.CurrentPc = pc;
            if (i.Id == inst.Id)
            {
                titleSteps++;
                visits[pc] = visits.GetValueOrDefault(pc) + 1;
            }
            else
            {
                childSteps[i.EventId] = childSteps.GetValueOrDefault(i.EventId) + 1;
            }
        };

        int frames = 0;
        for (; frames < FrameBudget; frames++) vm.StepFrame();
        int Visits(int pc) => visits.GetValueOrDefault(pc);

        foreach (var line in host.Describe()) GD.Print("[TitleTest] " + line);
        GD.Print($"[TitleTest] frames={frames} titleSteps={titleSteps} keyPolls={host.KeyPolls} " +
                 $"children=[{string.Join(",", childSteps.Select(kv => $"{kv.Key}:{kv.Value}"))}]");

        // 1. The intro branch: 111 [1, 2000717, 0, 2] must match (variable is 0 at boot).
        Check(Visits(26) == 1, $"pc26 op111 visits {Visits(26)} != 1");
        Check(host.Drew("Kagura/bg.png"), "opening picture Kagura/bg.png not drawn");

        // 2. 420 else escape: the taken case falls through the 420 marker and must leave the
        //    block at its end target instead of sliding into the else body.
        Check(Visits(43) >= 1, "420 else marker never executed");
        Check(Visits(46) >= 1, "else body pc46 never executed");

        // 3. Counted loop 179 [2] closed by 498 runs exactly twice.
        Check(Visits(49) == 1, $"loop head pc49 visits {Visits(49)} != 1");
        Check(Visits(69) == 2, $"loop end pc69 visits {Visits(69)} != 2");
        Check(Visits(70) == 1, $"pc70 after loop visits {Visits(70)} != 1");

        // 4. The splash call into CE549 returns before the parent moves on.
        Check(host.Called(549), "CE549 splash call missing");
        Check(childSteps.ContainsKey(549) && childSteps[549] > 0, "CE549 child coroutine never ran");

        // 5. Save flags are both 0, so the second title graphic is chosen and the missing
        //    SystemGraphic variant is skipped.
        Check(Visits(80) == 1, "title graphic condition pc80 not executed once");
        Check(host.Drew("Picture/title01.jpg"), "Picture/title01.jpg not drawn");
        Check(!host.Drew("SystemGraphic/TitleGraphic.png"), "SystemGraphic/TitleGraphic.png drawn although 2000299 is 0");

        // 6. The menu button graphic and its effect.
        Check(host.Drew("SystemFile/Button01.png"), "Button01.png not drawn");
        Check(host.Effects.Any(e => e.Kind == 5 && e.Duration == 30), "opcode 290 effect missing");

        // 7. Key poll: 123 stores a key code, 111 compares it with 20/11/10; with 20 the
        //    "A" label is reached (target = pc+1), the door SE plays and CE42 is called.
        Check(Visits(95) >= 1 && Visits(96) >= 1, "key poll pc95/pc96 not executed");
        Check(Visits(111) == 1, $"label A landing pc111 visits {Visits(111)} != 1");
        Check(Visits(115) == 1, $"pc115 (2000299 = 1) visits {Visits(115)} != 1");
        Check(host.Played("SE/[Action]DoorOpen_Komori.ogg"), "door SE missing");
        Check(host.Called(42), "CE42 title menu call missing");
        Check(childSteps.ContainsKey(42) && childSteps[42] > 0, "CE42 child coroutine never ran");

        // 8. CE42 asks the host for the three-row menu; the scripted host picks row 0, which
        //    is the new game hook: BGM, the title graphic and a transfer to map 3 at (60, 34).
        Check(host.Teleports.Any(t => t.Map == 3 && t.X == 60 && t.Y == 34 && t.Options == 1),
              "new game transfer to map 3 never requested");
        Check(host.Played("BGM/White_Base.mp3"), "title BGM missing");

        // 9. Nothing may log a warning or trap the VM in a runaway loop.
        Check(host.Logs.Count == 0, "host logged: " + string.Join(" | ", host.Logs));
        Check(vm.UnhandledOpcodeCount == 0, $"title chain saw {vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!vm.Faulted, "title chain faulted the VM (" + vm.FaultReason + ")");
        Check(titleSteps < 200000, $"title executed {titleSteps} instructions (runaway loop?)");

        if (_failures.Count > 0)
        {
            foreach (var f in _failures) GD.PrintErr("[TitleTest] FAIL: " + f);
            GD.Print("TITLE_TEST_FAILED");
            GetTree().Quit(1);
            return;
        }

        GD.Print("TITLE_TEST_OK");
        GetTree().Quit(0);
    }
}
