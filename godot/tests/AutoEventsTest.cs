using System;
using System.Collections.Generic;
using System.Linq;
using Lwsmf.Core;

namespace Lwsmf.Tests;

public static class AutoEventsTest
{
    public static void Run()
    {
        var vm = new EventVm();
        Program Add(int id, int run, int op, int variable, int value)
        {
            var program = Program.FromSlots(id, $"auto-{id}", new List<Slot> { Slot.Command(104) });
            program.RunCondition = run;
            program.ConditionOperator = op;
            program.ConditionVariable = variable;
            program.ConditionValue = value;
            vm.RegisterProgram(program);
            return program;
        }

        Add(38, 2, 2, 2_000_003, 1);
        Add(216, 2, 2, 2_000_003, 1);
        Add(217, 2, 2, 2_000_003, 1);
        Add(43, 2, 2, 2_000_003, 1);
        Add(242, 0, 2, 2_000_350, 1);
        Add(244, 0, 2, 2_000_350, 1);

        var auto = AutoEvents.FromPrograms(vm.Programs.Values);
        Check(auto.EventCount == 4, $"expected 4 tracked events, got {auto.EventCount}");
        Check(auto.Events.All(e => e.Id != 43), "title menu 43 must be skipped");
        Check(auto.Events.All(e => e.Id != 244), "run=0 event 244 must not be tracked");
        Check(auto.Events.Any(e => e.Id == 242), "gated run=0 event 242 must be tracked");

        var idle = auto.Sync(vm);
        Check(idle.Started == 0 && idle.Running == 0, "false conditions must not start anything");

        vm.Vars.Set(3, 1);
        var opened = auto.Sync(vm);
        Check(opened.Started == 3, $"expected 3 starts, got {opened.Started}");
        Check(opened.Running == 3, $"expected 3 running, got {opened.Running}");

        vm.StepFrame();
        var replayed = auto.Sync(vm);
        Check(replayed.Started == 2, $"only the 216/217 HUD painters replay while true, got {replayed.Started}");
        Check(replayed.Stopped == 0, "a true condition must not stop a painter");
        Check(auto.IsRunning(vm, 216) && auto.IsRunning(vm, 217), "HUD painters must be live again");
        Check(!auto.IsRunning(vm, 38), "edge-triggered event 38 must not restart while its condition still holds");

        vm.Vars.Set(3, 0);
        var closed = auto.Sync(vm);
        Check(closed.Stopped == 2, $"expected 2 stopped painters, got {closed.Stopped}");
        Check(closed.Running == 0, $"expected 0 running, got {closed.Running}");

        vm.Vars.Set(350, 1);
        var gated = auto.Sync(vm);
        Check(gated.Started == 1 && auto.IsRunning(vm, 242), "350 == 1 must wake CE242");
        vm.Vars.Set(350, 2);
        var handover = auto.Sync(vm);
        Check(handover.Stopped == 1, $"350 leaving 1 must stop CE242, stopped={handover.Stopped}");
        Check(!auto.IsRunning(vm, 242), "CE242 must not run in the action phase");

        // StopAll clears the coroutines but the edge latch stays true: only the HUD
        // painters (the replay allow-list) come back, matching Rust sync.
        vm.Vars.Set(3, 1);
        auto.Sync(vm);
        vm.StopAll();
        var afterStopAll = auto.Sync(vm);
        Check(afterStopAll.Started == 2, $"only the replay allow-list must resume after StopAll, started={afterStopAll.Started}");
        Check(!auto.IsRunning(vm, 38), "StopAll must not re-arm edge-triggered event 38 while its latch is true");

        var primedVm = new EventVm();
        Program AddPrime(int id, int run, int op, int variable, int value)
        {
            var program = Program.FromSlots(id, $"prime-{id}", new List<Slot> { Slot.Command(104) });
            program.RunCondition = run;
            program.ConditionOperator = op;
            program.ConditionVariable = variable;
            program.ConditionValue = value;
            primedVm.RegisterProgram(program);
            return program;
        }
        AddPrime(38, 2, 2, 2_000_003, 1);
        AddPrime(216, 2, 2, 2_000_003, 1);
        AddPrime(242, 0, 2, 2_000_350, 1);
        var primed = AutoEvents.FromPrograms(primedVm.Programs.Values);
        primedVm.Vars.Set(3, 1);
        primedVm.Vars.Set(350, 1);
        primed.PrimeLatched(primedVm);
        Check(primedVm.Instances.Count == 1, $"prime must start only the replay allow-list event, got {primedVm.Instances.Count}");
        var afterPrime = primed.Sync(primedVm);
        Check(afterPrime.Started == 0, $"already-live primed events must not start twice, started={afterPrime.Started}");
        Check(!primed.IsRunning(primedVm, 242), "prime must leave the edge-triggered battle command event to BattleWiring");

        // LoopVisualPainters keeps parallel painters (e.g. CE335 H-scene sprites) updating
        // continuously in live play while excluding sentinel CE38 and keeping gated CE242 singular.
        var loopVm = new EventVm();
        AddLoopProg(loopVm, 38, 2, 2_000_003, 1);
        AddLoopProg(loopVm, 335, 2, 2_000_062, 1);
        var autoLoop = AutoEvents.FromPrograms(loopVm.Programs.Values);
        autoLoop.LoopVisualPainters = true;
        loopVm.Vars.Set(3, 1);
        loopVm.Vars.Set(62, 1);
        autoLoop.Sync(loopVm);
        loopVm.StepFrame();
        var secondPass = autoLoop.Sync(loopVm);
        Check(secondPass.Started == 1 && autoLoop.IsRunning(loopVm, 335) && !autoLoop.IsRunning(loopVm, 38),
            $"LoopVisualPainters must replay CE335 without re-looping CE38, started={secondPass.Started}");

        // In live play (LoopVisualPainters == true), a non-ResumeOnLoad event already owned by
        // another subsystem (such as BattleWiring owning CE242) must not be double-started.
        var nonLoopVm = new EventVm();
        AddLoopProg(nonLoopVm, 242, 0, 2_000_350, 1);
        var autoNonLoop = AutoEvents.FromPrograms(nonLoopVm.Programs.Values);
        autoNonLoop.LoopVisualPainters = true;
        nonLoopVm.Vars.Set(350, 0);
        autoNonLoop.Sync(nonLoopVm);
        nonLoopVm.CreateInstance(242);
        nonLoopVm.Vars.Set(350, 1);
        var guardedStart = autoNonLoop.Sync(nonLoopVm);
        Check(guardedStart.Started == 0 && nonLoopVm.Instances.Count == 1,
            $"live play must not double-start live CE242, started={guardedStart.Started} count={nonLoopVm.Instances.Count}");

        // Loaded parallel auto-event instances must stop once their condition turns false.
        var restoredVm = new EventVm();
        var waitProg = Program.FromSlots(335, "restored-335", new List<Slot> { Slot.Command(180, [10]), Slot.Command(104) });
        waitProg.RunCondition = 2;
        waitProg.ConditionOperator = 2;
        waitProg.ConditionVariable = 2_000_062;
        waitProg.ConditionValue = 1;
        restoredVm.RegisterProgram(waitProg);
        var loadedInst = new Instance { Id = 77, EventId = 335, Program = waitProg, Pc = 0, IsParallel = true };
        restoredVm.RestoreInstance(loadedInst);
        restoredVm.Vars.Set(62, 1);
        var restoredAuto = AutoEvents.FromPrograms(restoredVm.Programs.Values);
        restoredAuto.PrimeLatched(restoredVm);
        Check(restoredAuto.IsRunning(restoredVm, 335), "PrimeLatched must rebind restored parallel auto-event instance");
        restoredVm.Vars.Set(62, 0);
        var stoppedLoaded = restoredAuto.Sync(restoredVm);
        Check(stoppedLoaded.Stopped == 1 && restoredVm.FindInstance(77) == null,
            $"restored parallel auto-event must stop when condition turns false (stopped={stoppedLoaded.Stopped})");

        // Loaded parallel event whose condition already held at PrimeLatched must stay suppressed under LoopVisualPainters
        // until its condition drops false first (matches retail load suppression of stale CE506 / v610=1).
        var loadEyeVm = new EventVm();
        var eyeProg = Program.FromSlots(506, "eye-loop", new List<Slot> { Slot.Command(104) });
        eyeProg.RunCondition = 2;
        eyeProg.ConditionOperator = 1; // >=
        eyeProg.ConditionVariable = 2_000_610;
        eyeProg.ConditionValue = 1;
        loadEyeVm.RegisterProgram(eyeProg);
        loadEyeVm.Vars.Set(610, 1);
        var loadEyeAuto = AutoEvents.FromPrograms(loadEyeVm.Programs.Values);
        loadEyeAuto.PrimeLatched(loadEyeVm);
        loadEyeAuto.LoopVisualPainters = true;
        var postLoadSync = loadEyeAuto.Sync(loadEyeVm);
        Check(postLoadSync.Started == 0 && !loadEyeAuto.IsRunning(loadEyeVm, 506),
            "stale loaded condition (v610==1) must remain suppressed under LoopVisualPainters until cleared");
        loadEyeVm.Vars.Set(610, 0);
        loadEyeAuto.Sync(loadEyeVm);
        loadEyeVm.Vars.Set(610, 1);
        var rearmedSync = loadEyeAuto.Sync(loadEyeVm);
        Check(rearmedSync.Started == 1 && loadEyeAuto.IsRunning(loadEyeVm, 506),
            "once condition clears to 0 and returns to 1, CE506 must trigger normally");

        // Imported retail H-scene saves (no live coroutine stack) must still resume CE335 under LoopVisualPainters,
        // and live play must not auto-spawn GatedCallOnly CE243 on var350==2 (BattleWiring hands off to CE241).
        var liveBattleVm = new EventVm();
        AddLoopProg(liveBattleVm, 335, 2, 2_000_062, 1);
        AddLoopProg(liveBattleVm, 243, 0, 2_000_350, 2);
        var liveBattleAuto = AutoEvents.FromPrograms(liveBattleVm.Programs.Values);
        liveBattleVm.Vars.Set(62, 1);
        liveBattleAuto.PrimeLatched(liveBattleVm);
        liveBattleAuto.LoopVisualPainters = true;
        var hSceneResume = liveBattleAuto.Sync(liveBattleVm);
        Check(hSceneResume.Started == 1 && liveBattleAuto.IsRunning(liveBattleVm, 335),
            "imported H-scene save must resume CE335 under LoopVisualPainters");
        liveBattleVm.Vars.Set(350, 2);
        var noDup243 = liveBattleAuto.Sync(liveBattleVm);
        Check(!liveBattleAuto.IsRunning(liveBattleVm, 243) && noDup243.Started == 0,
            "live play (LoopVisualPainters) must not auto-spawn duplicate CE243 on var350==2");

        // Imported retail H-scene saves must also resume continuous motion/blink painters (CE348/CE413),
        // and CE404 only when var[62]>=1 confirms an active H-scene (never on bedroom saves with stale var[137]==6).
        var hMotionVm = new EventVm();
        AddLoopProg(hMotionVm, 348, 2, 2_000_099, 1);
        AddLoopProg(hMotionVm, 413, 2, 2_000_137, 6);
        AddLoopProg(hMotionVm, 404, 2, 2_000_137, 6);
        var hMotionAuto = AutoEvents.FromPrograms(hMotionVm.Programs.Values);
        hMotionVm.Vars.Set(99, 1);
        hMotionVm.Vars.Set(137, 6);
        hMotionVm.Vars.Set(62, 0);
        hMotionAuto.PrimeLatched(hMotionVm);
        hMotionAuto.LoopVisualPainters = true;
        hMotionAuto.Sync(hMotionVm);
        Check(hMotionAuto.IsRunning(hMotionVm, 348) && hMotionAuto.IsRunning(hMotionVm, 413),
            "imported H-scene save must resume CE348 and CE413 under LoopVisualPainters");
        Check(!hMotionAuto.IsRunning(hMotionVm, 404),
            "CE404 must stay suppressed on non-H-scene load when var[62]==0 even if var[137]==6");

        var hCowgirlVm = new EventVm();
        AddLoopProg(hCowgirlVm, 404, 2, 2_000_137, 6);
        var hCowgirlAuto = AutoEvents.FromPrograms(hCowgirlVm.Programs.Values);
        hCowgirlVm.Vars.Set(137, 6);
        hCowgirlVm.Vars.Set(62, 1);
        hCowgirlAuto.PrimeLatched(hCowgirlVm);
        hCowgirlAuto.LoopVisualPainters = true;
        hCowgirlAuto.Sync(hCowgirlVm);
        Check(hCowgirlAuto.IsRunning(hCowgirlVm, 404),
            "imported H-scene save with var[62]>=1 and var[137]==6 must resume CE404");

        Console.WriteLine($"AUTO_EVENTS_TEST_OK count={auto.EventCount} start={opened.Started} resume={replayed.Started} gated={gated.Started}");
    }

    private static void AddLoopProg(EventVm vm, int id, int run, int variable, int value)
    {
        var p = Program.FromSlots(id, $"loop-{id}", new List<Slot> { Slot.Command(104) });
        p.RunCondition = run;
        p.ConditionOperator = 2;
        p.ConditionVariable = variable;
        p.ConditionValue = value;
        vm.RegisterProgram(p);
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("AUTO_EVENTS_TEST_FAILED: " + message);
    }
}
