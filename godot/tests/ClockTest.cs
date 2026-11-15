using System;
using System.Linq;
using Godot;
using Lwsmf.Core;

public partial class ClockTest : Node
{
    private int _checks;
    private int _failures;

    public override void _Ready()
    {
        var repo = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var dataPath = System.IO.Path.GetFullPath(System.IO.Path.Combine(repo, "..", "game_data", "programs.json"));

        // 1. Pure helper: the retail single-if carry, including its deliberate overflow.
        var vm = new EventVm { Host = new TraceRecorderHost(), StrictMode = true };
        vm.Vars.Set(GameClock.VarHour, 12);
        vm.Vars.Set(GameClock.VarMinute, 0);
        vm.Vars.Set(GameClock.VarWeekday, 0);
        GameClock.ApplyMinutes(vm, 130);
        Expect(vm.Vars.Get(GameClock.VarHour) == 13 && vm.Vars.Get(GameClock.VarMinute) == 70,
            "130 minutes must produce 13:70 (single-if carry)");
        Expect(vm.Vars.Get(GameClock.VarWeekday) == 0, "the 13:70 carry must not roll the weekday");

        // 2. Midnight rollover.
        vm.Vars.Set(GameClock.VarHour, 23);
        vm.Vars.Set(GameClock.VarMinute, 55);
        GameClock.ApplyMinutes(vm, 10);
        Expect(vm.Vars.Get(GameClock.VarHour) == 0 && vm.Vars.Get(GameClock.VarMinute) == 5,
            "23:55 + 10 must wrap to 0:05");
        Expect(vm.Vars.Get(GameClock.VarWeekday) == 1, "midnight must advance the weekday");

        // 3. Weekend flag: Friday/Saturday set it, and it is never cleared.
        vm.Vars.Set(GameClock.VarWeekday, 4);
        GameClock.ApplyMinutes(vm, 0);
        Expect(vm.Vars.Get(GameClock.VarWeekendFlag) == 25, "Friday must set the weekend flag");
        vm.Vars.Set(GameClock.VarWeekday, 0);
        GameClock.ApplyMinutes(vm, 0);
        Expect(vm.Vars.Get(GameClock.VarWeekendFlag) == 25, "the weekend flag must stay latched");

        // 4. 6200 formatting.
        vm.Vars.Set(GameClock.VarHour, 7);
        vm.Vars.Set(GameClock.VarMinute, 5);
        Expect(GameClock.FormatTime(vm) == "7:05", "minute 5 must format as 7:05");
        vm.Vars.Set(GameClock.VarMinute, 0);
        Expect(GameClock.FormatTime(vm) == "7:00", "minute 0 must format as 7:00");
        Expect(GameClock.WeekdayName(5) == "周六", "weekday 5 must be 周六");

        // 5. The real CE217 must produce the same result as the helper.
        var runVm = new EventVm { Host = new TraceRecorderHost(), StrictMode = true };
        foreach (var program in ProgramLoader.LoadCommonEvents(dataPath)) runVm.RegisterProgram(program.Value);
        runVm.Vars.Set(GameClock.VarHour, 12);
        runVm.Vars.Set(GameClock.VarMinute, 0);
        runVm.Vars.Set(GameClock.VarWeekday, 0);
        var instance = runVm.CreateInstance(217);
        if (instance == null)
        {
            Fail("CE217 must exist");
        }
        else
        {
            instance.Self.Set(600, 130);
            for (int frame = 0; frame < 30 && runVm.Instances.Count > 0; frame++) runVm.StepFrame();
            Expect(runVm.Vars.Get(GameClock.VarHour) == 13 && runVm.Vars.Get(GameClock.VarMinute) == 70,
                $"CE217 +130 must match the helper (got {runVm.Vars.Get(GameClock.VarHour)}:" +
                $"{runVm.Vars.Get(GameClock.VarMinute)})");
            Expect(runVm.Vars.Get(GameClock.VarWeekday) == 0, "CE217 +130 must keep the weekday");
        }

        if (_failures == 0)
        {
            GD.Print($"CLOCK_TEST_OK checks={_checks}");
            GetTree().Quit(0);
        }
        else
        {
            GD.PrintErr($"CLOCK_TEST_FAILED checks={_checks} failures={_failures}");
            GetTree().Quit(1);
        }
    }

    private void Expect(bool ok, string message)
    {
        _checks++;
        if (!ok)
        {
            _failures++;
            GD.PrintErr("CLOCK_ASSERT " + message);
        }
    }

    private void Fail(string message) => Expect(false, message);
}
