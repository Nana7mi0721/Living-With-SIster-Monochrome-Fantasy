using System;
using System.Linq;
using Godot;
using Lwsmf.Core;

/// <summary>
/// W4 Day3-4: thirty consecutive sleeps through the real CE329 -> CE317 chain. The test
/// talks to the VM directly (no map/mouse needed) and picks the retail sleep menu by
/// text, so it checks the calendar contract rather than a scripted shortcut.
/// </summary>
public partial class SleepSoakTest : Node
{
    private const int MaxFramesPerSleep = 100000;
    private int _checks;
    private int _failures;

    public override void _Ready()
    {
        var repo = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var dataPath = System.IO.Path.GetFullPath(System.IO.Path.Combine(repo, "..", "game_data", "programs.json"));
        var databasePath = System.IO.Path.GetFullPath(System.IO.Path.Combine(repo, "..", "game_data", "database.json"));
        var basicDir = System.IO.Path.GetFullPath(System.IO.Path.Combine(repo, "..", "work", "all", "BasicData"));
        var database = GameDatabase.Load(databasePath, basicDir);
        if (database == null)
        {
            Fail("database.json failed to load");
            return;
        }
        database.SeedBattleCells(1, 1);
        database.SeedBattleMenuText();
        var host = new TraceRecorderHost
        {
            DatabaseProvider = op => database.Execute(op),
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
        var wiring = new BattleWiring();
        wiring.SetAutoArm(true);
        wiring.SetEnabled(true);
        wiring.SetRoundLoop(true);
        foreach (var program in ProgramLoader.LoadCommonEvents(dataPath)) vm.RegisterProgram(program.Value);
        if (vm.GetProgram(329) == null) Fail("CE329 must exist");
        if (vm.GetProgram(317) == null) Fail("CE317 must exist");

        int days = ReadSoakDays();
        int day = 1;
        bool storyJumpSeen = false;
        for (int cycle = 0; cycle < days && _failures == 0; cycle++)
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
            while (vm.Instances.Count > 0 && frames < MaxFramesPerSleep)
            {
                wiring.Tick(vm, op => database.Execute(op));
                vm.StepFrame();
                frames++;
            }
            if (vm.Instances.Count > 0)
            {
                var active = string.Join(" | ", vm.Instances.Select(i => $"event={i.EventId} pc={i.Pc} wait={i.Wait.Kind}"));
                Fail($"day {day}: sleep chain did not finish in {MaxFramesPerSleep} frames ({active})");
                break;
            }
            if (vm.Faulted)
            {
                Fail($"day {day}: VM faulted: {vm.FaultReason}");
                break;
            }

            int next = day + 1;
            int hour = vm.Vars.Get(GameClock.VarHour);
            int minute = vm.Vars.Get(GameClock.VarMinute);
            int gotDay = vm.Vars.Get(GameClock.VarDay);
            if (gotDay < next)
            {
                Fail($"day {day}: day counter should advance past {day} (got {gotDay})");
                break;
            }
            int expectWeekday = (gotDay - 1) % 7;
            Expect(vm.Vars.Get(GameClock.VarWeekday) == expectWeekday,
                $"day {day}: weekday should be {expectWeekday} (got {vm.Vars.Get(GameClock.VarWeekday)})");
            if (gotDay > next)
            {
                storyJumpSeen = true;
                GD.Print($"SLEEP_SOAK_JUMP from={day} to={gotDay} clock={hour}:{minute:00}");
            }
            if (gotDay == next && !storyJumpSeen)
            {
                bool weekendWake = expectWeekday is 5 or 6;
                Expect(hour == (weekendWake ? 9 : 8) && minute == 30,
                    $"day {day}: wake time should be {(weekendWake ? "9" : "8")}:30 (got {hour}:{minute:00})");
            }
            else
            {
                Expect(hour is >= 0 and < 24 && minute is >= 0 and < 60,
                    $"day {day}: invalid clock {hour}:{minute:00}");
            }
            if (days > 30) GD.Print($"SLEEP_SOAK_DAY day={day} frames={frames} next={gotDay}");
            day = gotDay;
        }

        Expect(day >= days + 1, $"{days} sleeps must reach at least day {days + 1} (got {day})");
        if (_failures == 0)
        {
            GD.Print($"SLEEP_SOAK_TEST_OK days={days} checks={_checks}");
            GetTree().Quit(0);
        }
        else
        {
            GD.PrintErr($"SLEEP_SOAK_TEST_FAILED checks={_checks} failures={_failures}");
            GetTree().Quit(1);
        }
    }

    /// <summary>M5 long-run acceptance: default 30 days; LWsmf_SOAK_DAYS=90 for release soak.</summary>
    private static int ReadSoakDays()
    {
        var raw = OS.GetEnvironment("LWsmf_SOAK_DAYS");
        if (int.TryParse(raw, out var days) && days > 0) return days;
        return 30;
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
