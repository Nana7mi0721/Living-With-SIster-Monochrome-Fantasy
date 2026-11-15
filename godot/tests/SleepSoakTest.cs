using System;
using Godot;
using Lwsmf.Core;

/// <summary>
/// W4 Day3-4: thirty consecutive sleeps through the real CE329 -> CE317 chain. The test
/// talks to the VM directly (no map/mouse needed) and picks the retail sleep menu by
/// text, so it checks the calendar contract rather than a scripted shortcut.
/// </summary>
public partial class SleepSoakTest : Node
{
    private int _checks;
    private int _failures;

    public override void _Ready()
    {
        var repo = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var dataPath = System.IO.Path.GetFullPath(System.IO.Path.Combine(repo, "..", "game_data", "programs.json"));
        var host = new TraceRecorderHost
        {
            ChoiceSelector = options =>
            {
                int rest = Array.IndexOf(options, "今天还是睡吧");
                if (rest >= 0) return ChoiceReply.Select(rest);
                int yes = Array.IndexOf(options, "是的");
                if (yes >= 0) return ChoiceReply.Select(yes);
                return ChoiceReply.Select(0);
            }
        };
        var vm = new EventVm { Host = host, StrictMode = true };
        foreach (var program in ProgramLoader.LoadCommonEvents(dataPath)) vm.RegisterProgram(program.Value);
        if (vm.GetProgram(329) == null) Fail("CE329 must exist");
        if (vm.GetProgram(317) == null) Fail("CE317 must exist");

        int day = 1;
        for (int cycle = 0; cycle < 30 && _failures == 0; cycle++)
        {
            int weekday = (day - 1) % 7;
            vm.Vars.Set(63, 1);
            vm.Vars.Set(GameClock.VarDay, day);
            vm.Vars.Set(GameClock.VarHour, 23);
            vm.Vars.Set(GameClock.VarMinute, 30);
            vm.Vars.Set(GameClock.VarWeekday, weekday);
            vm.Vars.Set(14, 40);

            var instance = vm.CreateInstance(329);
            if (instance == null)
            {
                Fail($"day {day}: CE329 could not start");
                break;
            }
            instance.Self.Set(600, 1);
            int frames = 0;
            while (vm.Instances.Count > 0 && frames < 20000)
            {
                vm.StepFrame();
                frames++;
            }
            if (vm.Instances.Count > 0)
            {
                Fail($"day {day}: sleep chain did not finish in 20000 frames");
                break;
            }
            if (vm.Faulted)
            {
                Fail($"day {day}: VM faulted: {vm.FaultReason}");
                break;
            }

            int next = day + 1;
            int expectWeekday = (weekday + 1) % 7;
            int hour = vm.Vars.Get(GameClock.VarHour);
            int minute = vm.Vars.Get(GameClock.VarMinute);
            int gotDay = vm.Vars.Get(GameClock.VarDay);
            if (gotDay != next)
            {
                Fail($"day {day}: day counter should be {next} (got {gotDay})");
                break;
            }
            Expect(vm.Vars.Get(GameClock.VarWeekday) == expectWeekday,
                $"day {day}: weekday should be {expectWeekday}");
            bool weekendWake = expectWeekday is 5 or 6;
            Expect(hour == (weekendWake ? 9 : 8) && minute == 30,
                $"day {day}: wake time should be {(weekendWake ? "9" : "8")}:30 (got {hour}:{minute:00})");
            day = next;
        }

        Expect(day == 31, $"30 sleeps must reach day 31 (got {day})");
        if (_failures == 0)
        {
            GD.Print($"SLEEP_SOAK_TEST_OK days=30 checks={_checks}");
            GetTree().Quit(0);
        }
        else
        {
            GD.PrintErr($"SLEEP_SOAK_TEST_FAILED checks={_checks} failures={_failures}");
            GetTree().Quit(1);
        }
    }

    private void Expect(bool ok, string message)
    {
        _checks++;
        if (!ok)
        {
            _failures++;
            GD.PrintErr("SLEEP_SOAK_ASSERT " + message);
        }
    }

    private void Fail(string message) => Expect(false, message);
}
