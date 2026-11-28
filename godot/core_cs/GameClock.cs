using System;

namespace Lwsmf.Core;

/// <summary>
/// The retail time system (docs/TIME_SYSTEM.md). Time is not frame-driven: only scripts
/// write it, and CE217 "時刻表示" is the single incremental writer. This helper mirrors
/// that event exactly so C# callers can reason about the clock without replaying pictures.
///
/// Variables: 4 = hour 0..23, 5 = minute 0..59, 6 = weekday 0..6 (Monday..Sunday),
/// 8 = "tomorrow is the weekend" flag (25 when weekday is Friday or Saturday),
/// 160 = day counter.
/// </summary>
public static class GameClock
{
    public const int VarHour = 4;
    public const int VarMinute = 5;
    public const int VarWeekday = 6;
    public const int VarWeekendFlag = 8;
    public const int VarDay = 160;

    public readonly struct Snapshot
    {
        public int Hour { get; init; }
        public int Minute { get; init; }
        public int Weekday { get; init; }
        public int WeekendFlag { get; init; }
        public int Day { get; init; }
    }

    public static Snapshot Read(EventVm vm) => new()
    {
        Hour = vm.Vars.Get(VarHour),
        Minute = vm.Vars.Get(VarMinute),
        Weekday = vm.Vars.Get(VarWeekday),
        WeekendFlag = vm.Vars.Get(VarWeekendFlag),
        Day = vm.Vars.Get(VarDay),
    };

    /// <summary>
    /// CE217 pc16..pc27. The carry is a single if, not a loop: the retail engine really
    /// produces 13:70 for +130 minutes, and reproducing that quirk is required for story
    /// parity (docs/TIME_SYSTEM.md §2.3b).
    /// </summary>
    public static void ApplyMinutes(EventVm vm, int delta)
    {
        var vars = vm.Vars;
        int minute = vars.Get(VarMinute) + delta;
        vars.Set(VarMinute, minute);
        if (minute >= 60)
        {
            vars.Set(VarHour, vars.Get(VarHour) + 1);
            vars.Set(VarMinute, minute - 60);
            if (vars.Get(VarHour) >= 24)
            {
                vars.Set(VarHour, vars.Get(VarHour) - 24);
                int weekday = vars.Get(VarWeekday) + 1;
                if (weekday >= 7) weekday = 0;
                vars.Set(VarWeekday, weekday);
            }
        }
        // pc62/pc67 also rewrite the weekend flag on every call; nothing ever clears it.
        int current = vars.Get(VarWeekday);
        if (current is 4 or 5) vars.Set(VarWeekendFlag, 25);
    }

    /// <summary>CE217's 6200 text: minute 0 formats as :00, 1..9 as :0x.</summary>
    public static string FormatTime(EventVm vm)
    {
        var clock = Read(vm);
        return clock.Minute switch
        {
            0 => $"{clock.Hour}:00",
            < 10 => $"{clock.Hour}:0{clock.Minute}",
            _ => $"{clock.Hour}:{clock.Minute}"
        };
    }

    public static string WeekdayName(int weekday) => weekday switch
    {
        0 => "周一",
        1 => "周二",
        2 => "周三",
        3 => "周四",
        4 => "周五",
        5 => "周六",
        6 => "周日",
        _ => "?"
    };
}
