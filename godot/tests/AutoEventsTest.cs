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
        Check(replayed.Started == 2, $"only the two HUD painters may replay, got {replayed.Started}");
        Check(replayed.Stopped == 0, "a true condition must not stop a painter");
        Check(auto.IsRunning(vm, 216) && auto.IsRunning(vm, 217), "HUD painters must be live again");
        Check(!auto.IsRunning(vm, 38), "a one-shot event may not restart without a new edge");

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

        // D-33: StopAll keeps the edge latch, matching Rust's session-scoped
        // AutoEvents. A still-true one-shot must not be re-armed by clearing state.
        vm.Vars.Set(3, 1);
        auto.Sync(vm);
        vm.StopAll();
        var afterStopAll = auto.Sync(vm);
        Check(afterStopAll.Started == 2, $"only the HUD painters may resume after StopAll, started={afterStopAll.Started}");
        Check(!auto.IsRunning(vm, 38), "StopAll must not re-arm the still-true one-shot event 38");

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
        Check(primedVm.Instances.Count == 1, $"prime must start only the HUD painter, got {primedVm.Instances.Count}");
        var afterPrime = primed.Sync(primedVm);
        Check(afterPrime.Started == 0, $"primed conditions must not fire fresh edges, started={afterPrime.Started}");
        Check(!primed.IsRunning(primedVm, 242), "prime must not wake the battle command event");

        Console.WriteLine($"AUTO_EVENTS_TEST_OK count={auto.EventCount} start={opened.Started} resume={replayed.Started} gated={gated.Started}");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("AUTO_EVENTS_TEST_FAILED: " + message);
    }
}
