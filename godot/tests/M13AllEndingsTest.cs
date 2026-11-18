using System;
using System.Collections.Generic;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// C# half of the M13 remaining-seven-endings matrix. Mirrors
/// crates/wolf-game/tests/m13_all_endings.rs: static gate triples plus
/// runtime drives that inject satisfying state and assert the ending text.
/// </summary>
public static class M13AllEndingsTest
{
    private static readonly (int Var, int Value, int Op)[] PregnancyGate = { (2_000_021, 300, 1) };
    private static readonly (int Var, int Value, int Op)[] TrustGate = { (2_000_021, 5, 3) };
    private static readonly (int Var, int Value, int Op)[] MoodGate = { (2_000_023, 1, 3) };
    private static readonly (int Var, int Value, int Op)[] ArmyGate = { (2_000_151, 1, 1) };
    private static readonly (int Var, int Value, int Op)[] SickDispatch =
    {
        (1_600_000, 4, 2), (1_600_000, 5, 2), (1_600_000, 6, 2)
    };
    private static readonly (int Var, int Value, int Op)[] ShadowGate =
    {
        (1_600_030, 1, 2), (1_600_030, -1, 3)
    };
    private static readonly (int Var, int Value, int Op)[] FarmDispatch =
    {
        (1_600_000, 13, 2), (1_600_000, 14, 2), (1_600_000, 15, 2)
    };

    public static void Run()
    {
        int cases = 0;
        int gates = 0;

        // 1. 妹妹怀孕 — CE405 good/bad arms.
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindGatePc(M13Harness.Program(vm, 405), PregnancyGate);
            var run = M13Harness.DriveAt(405, pc, globals: new[] { (21, 350) });
            M13Harness.Reached("pregnancy good", run, "怀孕END", "good end");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindGatePc(M13Harness.Program(vm, 405), PregnancyGate);
            var run = M13Harness.DriveAt(405, pc, globals: new[] { (21, 100) });
            M13Harness.Reached("pregnancy bad", run, "不被期待的怀孕");
            cases++;
        }

        // 2. 参军 / 返家 — CE326 trust+mood gate then army gate.
        {
            var vm = M13Harness.NewVm();
            var program = M13Harness.Program(vm, 326);
            int pc = M13Harness.FindGatePc(program, TrustGate);
            M13Harness.FindGatePc(program, MoodGate);
            M13Harness.FindGatePc(program, ArmyGate);
            var run = M13Harness.DriveAt(326, pc, globals: new[] { (21, 3), (23, 0), (151, 1) });
            M13Harness.Reached("military service", run, "兵役");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindGatePc(M13Harness.Program(vm, 326), TrustGate);
            var run = M13Harness.DriveAt(326, pc, globals: new[] { (21, 3), (23, 0), (151, 0) });
            M13Harness.Reached("return home", run, "返家");
            cases++;
        }

        // 3. 妹妹病亡 — CE277 mild arm and severe-arm side effects.
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindGatePc(M13Harness.Program(vm, 277), SickDispatch);
            var run = M13Harness.DriveAt(277, pc, selfs: new[] { (600, 4) }, maxFrames: 3000);
            M13Harness.Reached("sick mild", run, "妹妹的身体状况好像不太好");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindMessagePc(M13Harness.Program(vm, 277), "今天特别的色呢");
            var baseline = M13Harness.NewVm();
            int a151 = baseline.Vm.Vars.Get(151);
            int a18 = baseline.Vm.Vars.Get(18);
            int a102 = baseline.Vm.Vars.Get(102);
            var run = M13Harness.DriveAt(277, pc, maxFrames: 3000);
            M13Harness.Check(run.Messages.Count > 0, "sick severe: rendered nothing");
            M13Harness.Check(run.Unhandled == 0, $"sick severe: unhandled={run.Unhandled}");
            M13Harness.Check(!run.Faulted, $"sick severe: faulted {run.FaultReason}");
            M13Harness.Check(run.Vm.Vm.Vars.Get(151) == a151 + 1, "sick severe: var151 should +1");
            M13Harness.Check(run.Vm.Vm.Vars.Get(18) == a18 - 5, "sick severe: var18 should -5");
            M13Harness.Check(run.Vm.Vm.Vars.Get(102) == a102 - 10, "sick severe: var102 should -10");
            cases++;
        }

        // 4/5. 暗影 / 魔界 — CE305 stage25 battle-result branch at fixed pc 2915.
        {
            var vm = M13Harness.NewVm();
            var program = M13Harness.Program(vm, 305);
            const int shadowPc = 2915;
            var triples = M13Harness.CondTriples(program, shadowPc);
            M13Harness.Check(triples.SequenceEqual(ShadowGate), "CE305@2915 is not the shadow/makai gate");
            var run = M13Harness.DriveAt(305, shadowPc, selfs: new[] { (630, 1) }, maxFrames: 4000);
            M13Harness.Reached("shadow end", run, "阴影");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            const int shadowPc = 2915;
            var run = M13Harness.DriveAt(305, shadowPc, selfs: new[] { (630, -1) }, maxFrames: 4000);
            M13Harness.Reached("makai end", run, "魔界");
            cases++;
        }

        // 6. 农民 END — CE305 stage13/14/15 dispatch + var534==2.
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindGatePc(M13Harness.Program(vm, 305), FarmDispatch);
            var run = M13Harness.DriveAt(305, pc, selfs: new[] { (600, 13) }, globals: new[] { (534, 2) }, maxFrames: 6000);
            M13Harness.Reached("farmer end", run, "农民END");
            cases++;
        }

        // 7. 妹妹爆炸 BADEND — CE322/CE323/CE359 and CE297 foreshadow.
        foreach (int eventId in new[] { 322, 323, 359 })
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindMessagePc(M13Harness.Program(vm, eventId), "妹妹爆炸");
            var run = M13Harness.DriveAt(eventId, pc, maxFrames: 400);
            M13Harness.Reached($"CE{eventId} explosion", run, "妹妹爆炸");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            int pc = M13Harness.FindMessagePc(M13Harness.Program(vm, 297), "积攒了大量魔力");
            var run = M13Harness.DriveAt(297, pc, maxFrames: 400);
            M13Harness.Reached("CE297 oracle", run, "爆炸");
            cases++;
        }

        // Ignored-long-chain parity: CE326 -> CE405 pregnancy, both arms.
        {
            var vm = M13Harness.NewVm();
            var program = M13Harness.Program(vm, 326);
            int pc = M13Harness.FindGatePc(program, (2_000_105, 3, 1));
            var run = M13Harness.DriveAt(326, pc, globals: new[] { (105, 3), (21, 350) }, maxFrames: 12000);
            M13Harness.Reached("CE326 to CE405 good", run, "怀孕END");
            cases++;
        }
        {
            var vm = M13Harness.NewVm();
            var program = M13Harness.Program(vm, 326);
            int pc = M13Harness.FindGatePc(program, (2_000_105, 3, 1));
            var run = M13Harness.DriveAt(326, pc, globals: new[] { (105, 3), (21, 100) }, maxFrames: 12000);
            M13Harness.Reached("CE326 to CE405 bad", run, "不被期待的怀孕");
            cases++;
        }

        // Static ending gate cross-check.
        {
            var vm = M13Harness.NewVm();
            M13Harness.GateExists(vm, 405, PregnancyGate);
            M13Harness.GateExists(vm, 326, TrustGate);
            M13Harness.GateExists(vm, 326, MoodGate);
            M13Harness.GateExists(vm, 326, ArmyGate);
            M13Harness.GateExists(vm, 277, SickDispatch);
            M13Harness.GateExists(vm, 305, ShadowGate);
            M13Harness.GateExists(vm, 305, FarmDispatch);
            gates = 7;
        }

        GD.Print($"M13_ALL_ENDINGS_TEST_OK cases={cases} gates={gates}");
    }
}
