using System;
using System.Collections.Generic;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Unit tests for the map page runtime: reversed page selection, automatic start for
/// autorun/parallel pages, action-key dispatch geometry and instance reuse. Uses synthetic
/// maps so it stays independent of the retail data tree.
/// </summary>
public partial class MapRuntimeTest : Node
{
    private readonly List<string> _failures = new();
    private int _checks;

    private void Check(bool condition, string message)
    {
        _checks++;
        if (!condition) _failures.Add(message);
    }

    public override void _Ready()
    {
        ConditionOperators();
        PageSelectionAndAutoStart();
        EventCollision();

        if (_failures.Count > 0)
        {
            foreach (string failure in _failures) GD.PrintErr($"[MapRuntimeTest] FAIL: {failure}");
            GetTree().Quit(1);
            return;
        }

        GD.Print($"MAP_RUNTIME_TEST_OK {_checks}/{_checks}");
        GetTree().Quit();
    }

    private void ConditionOperators()
    {
        var c = new PageCondition { Value = 5 };
        for (int op = 0; op <= 5; op++)
        {
            c.Operator = op;
            bool[] expected = { 10 > 5, 10 >= 5, false, false, false, 10 != 5 };
            Check(c.Compare(10) == expected[op], $"operator {op} compare(10) wrong");
            bool[] expected5 = { false, true, true, true, false, false };
            Check(c.Compare(5) == expected5[op], $"operator {op} compare(5) wrong");
        }
        c.Operator = 6;
        Check(!c.Compare(5), "unknown operator 6 must never match");
        c.Operator = 7;
        Check(!c.Compare(5), "unknown operator 7 must never match");
    }

    private static Program Nops(int count)
    {
        var slots = new List<Slot>();
        for (int i = 0; i < count; i++) slots.Add(new Slot { Opcode = 0 });
        return Program.FromSlots(999, "nops", slots);
    }

    /// <summary>event_occupies: an active non-slip-through page blocks its own tile; bounds always block.</summary>
    private void EventCollision()
    {
        var vm = new EventVm();
        var map = new MapDefinition { Id = 9, File = "Collision.mps", Name = "collision", Width = 10, Height = 8 };
        var def = new MapEventDef { Id = 1, Name = "npc", X = 3, Y = 3, PageCount = 2 };
        def.Pages.Add(new MapEventPage { Page = 0, Trigger = MapTrigger.ActionKey, BehaviorOptions = 0x03 });
        def.Pages.Add(new MapEventPage { Page = 1, Trigger = MapTrigger.ActionKey, BehaviorOptions = 0x08 });
        map.Events.Add(def);
        var runtime = new MapRuntime(map);

        Check(runtime.CanEnter(3, 3), "inactive event page must not block its tile");
        runtime.Events[0].Active = 0;
        Check(runtime.EventOccupies(3, 3), "solid active page occupies its tile");
        Check(!runtime.CanEnter(3, 3), "solid active page blocks entry");
        Check(runtime.CanEnter(4, 3), "neighbour tile stays open");

        runtime.Events[0].Active = 1;
        Check(!runtime.EventOccupies(3, 3), "slip-through page does not occupy its tile");
        Check(runtime.CanEnter(3, 3), "slip-through page is walkable");

        Check(!runtime.CanEnter(-1, 0) && !runtime.CanEnter(0, -1), "negative coordinates are blocked");
        Check(!runtime.CanEnter(10, 3) && !runtime.CanEnter(3, 8), "map edges are blocked");

        // Contact box: base is one tile wide, half a tile high; square makes it a full tile.
        var touchDef = new MapEventDef { Id = 2, Name = "touch", X = 5, Y = 4, PageCount = 1 };
        touchDef.Pages.Add(new MapEventPage { Page = 0, Trigger = MapTrigger.PlayerTouch, Program = Nops(2) });
        map.Events.Add(touchDef);
        var touchRuntime = new MapRuntime(map);
        touchRuntime.StartPending(vm);
        Check(touchRuntime.Find(2)?.Active == 0, "player-touch page should become active");

        var box = MapRuntime.ContactBox(touchRuntime.Events[1], touchRuntime.Events[1].Def.Pages[0]);
        Check(box == (5, 4, 5, 4), $"plain contact box should be one tile, got {box}");
        var squarePage = new MapEventPage { BehaviorOptions = 0x20 };
        var squareBox = MapRuntime.ContactBox(touchRuntime.Events[1], squarePage);
        Check(squareBox == (5, 4, 5, 4), $"square contact box should stay one tile, got {squareBox}");
        var widePage = new MapEventPage { CollisionWidth = 1, CollisionHeight = 1 };
        var wideBox = MapRuntime.ContactBox(touchRuntime.Events[1], widePage);
        Check(wideBox == (4, 3, 6, 5), $"collision 1/1 should extend one tile each way, got {wideBox}");

        Check(touchRuntime.ContactTriggers(3, 4, 4, 4).Count == 0, "touch fires only inside the box");
        Check(touchRuntime.ContactTriggers(3, 4, 5, 4).Count == 1, "entering the box fires player touch");
        Check(touchRuntime.ContactTriggers(5, 4, 5, 4).Count == 0, "already inside must not fire again");

        touchRuntime.StartTriggerPage(2, 0, vm);
        Check(touchRuntime.Events[1].Starts == 1, "touch page should start once");
    }

    private void PageSelectionAndAutoStart()
    {
        var map = new MapDefinition { Id = 7, File = "Test.mps", Name = "synthetic", Width = 20, Height = 10 };

        var actionPage = new MapEventPage
        {
            Page = 0,
            Trigger = MapTrigger.ActionKey,
            Program = Program.FromSlots(1, "action", new List<Slot>()),
            Conditions = new[]
            {
                new PageCondition { Enabled = true, Operator = 2, Variable = Magic.VarBase + 10, Value = 0 },
                new PageCondition(),
                new PageCondition(),
                new PageCondition()
            }
        };
        var parallelPage = new MapEventPage
        {
            Page = 1,
            Trigger = MapTrigger.Parallel,
            Program = Nops(4),
            Conditions = new[]
            {
                new PageCondition { Enabled = true, Operator = 2, Variable = Magic.VarBase + 10, Value = 1 },
                new PageCondition(),
                new PageCondition(),
                new PageCondition()
            }
        };

        var def = new MapEventDef { Id = 1, Name = "switch", X = 5, Y = 5, PageCount = 2 };
        def.Pages.Add(actionPage);
        def.Pages.Add(parallelPage);
        map.Events.Add(def);

        // A disabled condition is skipped by conditions_met even when it would fail.
        var map2 = new MapDefinition { Id = 8, File = "Test2.mps", Name = "disabled", Width = 10, Height = 10 };
        var disabledDef = new MapEventDef { Id = 1, Name = "disabled", X = 2, Y = 2, PageCount = 1 };
        disabledDef.Pages.Add(new MapEventPage
        {
            Page = 0,
            Trigger = MapTrigger.Parallel,
            Program = Nops(2),
            Conditions = new[]
            {
                new PageCondition { Enabled = false, Operator = 2, Value = 99 },
                new PageCondition(),
                new PageCondition(),
                new PageCondition()
            }
        });
        map2.Events.Add(disabledDef);
        var runtime2 = new MapRuntime(map2);

        var vm = new EventVm();
        var runtime = new MapRuntime(map);

        Check(runtime.SelectPage(runtime.Events[0], vm) == 0, "unset variable must select fallback page 0");
        Check(runtime.StartPending(vm) == 0, "action-key page must not auto-start");
        Check(runtime.Events[0].Active == 0, "page 0 should be active");

        Check(runtime.TriggerCandidates(true, false, 5, 5, 2).Count == 1, "action key on the event tile must match");
        Check(runtime.TriggerCandidates(true, false, 0, 0, 2).Count == 0, "action key away from the event must not match");
        Check(runtime.TriggerCandidates(false, true, 5, 5, 2).Count == 0, "touch must not match an action page");

        Check(runtime.StartTriggerPage(1, 0, vm), "start_trigger_page should launch the active action page");
        Check(runtime.Events[0].Starts == 1, "start counter should be 1");
        Check(vm.Instances.Count == 1, "one instance expected after the trigger");
        Check(!runtime.StartTriggerPage(1, 0, vm), "a live page must not start twice");
        Check(!runtime.StartTriggerPage(1, 1, vm), "a page that is not active must not start");

        vm.StopAll();
        runtime.Events[0].InstanceId = null;

        Check(runtime2.SelectPage(runtime2.Events[0], vm) == 0, "disabled condition must not block its page");

        vm.Vars.Set(10, 1);
        Check(runtime.SelectPage(runtime.Events[0], vm) == 1, "variable == 1 must select page 1");
        Check(runtime.StartPending(vm) == 1, "parallel page should auto-start");
        Check(runtime.Events[0].Active == 1, "page 1 should be active");
        Check(runtime.ParallelCount == 1, "parallel page count should be 1");
        var parallelInst = vm.FindInstance(runtime.Events[0].InstanceId ?? -1);
        Check(parallelInst != null && parallelInst.IsParallel, "parallel map page must mark instance IsParallel = true");
        Check(!vm.ForegroundBusy(), "parallel map page must never block ForegroundBusy()");
        Check(runtime.TriggerCandidates(true, false, 5, 5, 2).Count == 0, "parallel page has no action trigger");

        // Page switch while a page is waiting on a child call is refused.
        var inst = vm.FindInstance(runtime.Events[0].InstanceId ?? -1);
        Check(inst != null, "parallel page instance should exist");
        if (inst != null)
        {
            vm.Vars.Set(10, 2);
            inst.Wait.WaitForChild(999);
            Check(runtime.SelectPage(runtime.Events[0], vm) == null, "no page matches while var == 2");
            runtime.StartPending(vm);
            Check(runtime.Events[0].Active == 1, "page must not change to none while the instance waits on a child");
            Check(runtime.Events[0].InstanceId == inst.Id, "the child-waiting instance must survive");

            // On the frame ReapFinished clears WaitStateKind.Child, JustResumedFromChild
            // spares the caller through StartPending until the next StepFrame clears it.
            var childProg = Program.FromSlots(998, "child", new List<Slot> { Slot.Command(172) });
            vm.RegisterProgram(childProg);
            var childInst = vm.CreateInstance(998, parentId: inst.Id)!;
            inst.Wait.WaitForChild(childInst.Id);
            vm.StepFrame();
            Check(inst.JustResumedFromChild, "ReapFinished must latch JustResumedFromChild on parent");
            runtime.StartPending(vm);
            Check(runtime.Events[0].Active == 1 && runtime.Events[0].InstanceId == inst.Id,
                "parent must survive StartPending on the frame its child returned");
            vm.StepFrame();
            Check(!inst.JustResumedFromChild, "next StepFrame must clear JustResumedFromChild");
        }

        // Autorun fires once per activation; a finished autorun must not be
        // recreated by the next StartPending (parallel pages are the only trigger
        // that restarts on its own, map_runtime.rs:1310).
        var autorunMap = new MapDefinition { Id = 10, File = "Autorun.mps", Name = "autorun", Width = 8, Height = 8 };
        var autorunDef = new MapEventDef { Id = 1, Name = "autorun", X = 1, Y = 1, PageCount = 1 };
        autorunDef.Pages.Add(new MapEventPage
        {
            Page = 0,
            Trigger = MapTrigger.Autorun,
            Program = Program.FromSlots(2, "empty", new List<Slot>())
        });
        autorunMap.Events.Add(autorunDef);
        var autorunRuntime = new MapRuntime(autorunMap);
        Check(autorunRuntime.StartPending(vm) == 1, "autorun page should start once");
        Check(autorunRuntime.Events[0].Starts == 1 && autorunRuntime.Events[0].StartsOnPage == 1,
            "autorun start counters should record one activation");
        Check(autorunRuntime.StartPending(vm) == 0, "finished autorun must not restart");
        Check(autorunRuntime.Events[0].Starts == 1, "autorun start counter must stay at one");
        Check(autorunRuntime.Events[0].Active == 0, "autorun page must stay active without a live instance");

        // Switching to another page resets the per-page autorun budget
        // (map_runtime.rs:1281), so returning to page 0 must run it again.
        autorunDef.PageCount = 2;
        autorunDef.Pages.Add(new MapEventPage
        {
            Page = 1,
            Trigger = MapTrigger.Autorun,
            Program = Program.FromSlots(2, "second", new List<Slot>()),
            Conditions = new[]
            {
                new PageCondition { Enabled = true, Operator = 2, Variable = Magic.VarBase + 10, Value = 1 },
                new PageCondition(),
                new PageCondition(),
                new PageCondition()
            }
        });
        vm.Vars.Set(10, 1);
        Check(autorunRuntime.StartPending(vm) == 1, "page 1 should start after the switch");
        Check(autorunRuntime.Events[0].Active == 1 && autorunRuntime.Events[0].StartsOnPage == 1,
            "page switch should reset the autorun budget for page 1");
        vm.Vars.Set(10, 0);
        Check(autorunRuntime.StartPending(vm) == 1, "returning to page 0 should start its autorun again");
        Check(autorunRuntime.Events[0].Active == 0 && autorunRuntime.Events[0].Starts == 3,
            "page 0 should own a second autorun start after switching back");

        // An in-flight autorun coroutine (!IsParallel) whose own condition clears mid-WAIT
        // (e.g. SampleMapA Ev24 clearing var[60]=0 inside its loop before calling CE329)
        // must finish to completion instead of being killed on want == null.
        var condAutoMap = new MapDefinition { Id = 11, File = "CondAutorun.mps", Name = "condauto", Width = 8, Height = 8 };
        var condAutoDef = new MapEventDef { Id = 1, Name = "ev24", X = 1, Y = 1, PageCount = 1 };
        condAutoDef.Pages.Add(new MapEventPage
        {
            Page = 0,
            Trigger = MapTrigger.Autorun,
            Program = Program.FromSlots(3, "ev24", new List<Slot>
            {
                Slot.Command(121, [2_000_060, 0, 0, 0x0000]),
                Slot.Command(180, [2]),
                Slot.Command(121, [2_000_061, 42, 0, 0x0000]),
                Slot.Command(172)
            }),
            Conditions = new[]
            {
                new PageCondition { Enabled = true, Operator = 2, Variable = Magic.VarBase + 60, Value = 4 },
                new PageCondition(),
                new PageCondition(),
                new PageCondition()
            }
        });
        condAutoMap.Events.Add(condAutoDef);
        var condRuntime = new MapRuntime(condAutoMap);
        vm.Vars.Set(60, 4);
        vm.Vars.Set(61, 0);
        Check(condRuntime.StartPending(vm) == 1, "conditional autorun must start when var60 == 4");
        vm.StepFrame();
        Check(condRuntime.SelectPage(condRuntime.Events[0], vm) == null, "var60 cleared mid-WAIT -> want == null");
        condRuntime.StartPending(vm);
        Check(condRuntime.Events[0].InstanceId is int condInstId && vm.FindInstance(condInstId) != null,
            "in-flight autorun must survive StartPending when its condition clears mid-WAIT");
        vm.StepFrame();
        vm.StepFrame();
        Check(vm.Vars.Get(61) == 42, "in-flight autorun must finish trailing instructions after WAIT");
    }
}
