using System;
using System.Collections.Generic;
using System.Linq;

namespace Lwsmf.Core;

/// <summary>One common event whose header carries a run condition.</summary>
public sealed class AutoEvent
{
    public int Id { get; init; }
    public string Title { get; init; } = "";
    public int Operator { get; init; }
    public int Variable { get; init; }
    public int Value { get; init; }
    public bool ResumeOnLoad { get; init; }
}

public readonly struct AutoSyncReport
{
    public int Started { get; init; }
    public int Stopped { get; init; }
    public int Running { get; init; }
}

/// <summary>
/// Scheduler for run_condition == 2 common events, plus the two battle events
/// (242/243) that the data gates on 2000350 but never calls. Mirrors the Rust
/// engine's auto_events.rs: the scheduler is edge-triggered, so a coroutine that
/// ended on its own is not restarted while its condition stays true. The two HUD
/// painters 216/217 are the allow-listed exception and are replayed every frame.
/// Event 43 is skipped here because this port's title menu is TitleMap event 2.
/// </summary>
public sealed class AutoEvents
{
    /// <summary>Event 43 is the title menu; the title entry point starts it explicitly.</summary>
    public static readonly int[] Skipped = { 43 };

    /// <summary>run_condition == 0 events the battle state machine needs woken by condition.</summary>
    public static readonly int[] GatedCallOnly = { 242, 243 };

    /// <summary>Linear HUD painters that must be re-created every frame while their condition holds.</summary>
    public static readonly int[] ResumeOnLoad = { 216, 217 };

    private readonly List<AutoEvent> _events;
    private readonly Dictionary<int, bool> _latched = new();
    private readonly Dictionary<int, int> _running = new();
    private readonly Dictionary<int, int> _starts = new();

    private AutoEvents(List<AutoEvent> events)
    {
        _events = events;
    }

    public bool Enabled { get; set; } = true;
    public int EventCount => _events.Count;
    public IReadOnlyList<AutoEvent> Events => _events;

    public static AutoEvents FromPrograms(IEnumerable<Program> programs)
    {
        var events = programs
            .OrderBy(p => p.Id)
            .Where(p => !Skipped.Contains(p.Id)
                && (p.RunCondition == 2
                    || (p.RunCondition == 0 && GatedCallOnly.Contains(p.Id))))
            .Select(p => new AutoEvent
            {
                Id = p.Id,
                Title = p.Title,
                Operator = p.ConditionOperator,
                Variable = p.ConditionVariable,
                Value = p.ConditionValue,
                ResumeOnLoad = ResumeOnLoad.Contains(p.Id)
            })
            .ToList();
        return new AutoEvents(events);
    }

    /// <summary>Seed the latched state after a load, then replay the HUD painters once.</summary>
    public void PrimeLatched(EventVm vm)
    {
        if (!Enabled) return;
        foreach (var ev in _events)
        {
            _latched[ev.Id] = ConditionMet(ev.Operator, ConditionValue(vm, ev.Variable), ev.Value);
        }
        Sync(vm);
    }

    public AutoSyncReport Sync(EventVm vm)
    {
        int started = 0, stopped = 0;
        if (!Enabled) return new AutoSyncReport { Running = RunningCount(vm) };
        foreach (var ev in _events)
        {
            bool now = ConditionMet(ev.Operator, ConditionValue(vm, ev.Variable), ev.Value);
            // Another subsystem may own a live instance of the same event (battle
            // command events 242/243). Treat it as alive so we never double-start it.
            bool alive = IsEventLive(vm, ev.Id);
            bool before = _latched.TryGetValue(ev.Id, out bool latched) && latched;
            // Rust auto_events.rs:252 — the scheduler is edge-triggered. Only the
            // 216/217 HUD painters are replayed while their condition holds, because
            // their rendered text must replace the SavePart5 snapshot after a load.
            bool shouldStart = ev.ResumeOnLoad && now ? !alive : now && !before;
            if (shouldStart)
            {
                var inst = vm.CreateInstance(ev.Id);
                if (inst != null)
                {
                    inst.IsParallel = true;
                    _running[ev.Id] = inst.Id;
                    _starts[ev.Id] = StartsFor(ev.Id) + 1;
                    started++;
                    TraceStart(ev.Id, inst.Id);
                }
            }
            else if (!now && _running.Remove(ev.Id, out int instanceId))
            {
                if (vm.FindInstance(instanceId) != null)
                {
                    vm.StopInstance(instanceId);
                    stopped++;
                    TraceStop(ev.Id, instanceId);
                }
            }
            _latched[ev.Id] = now;
        }
        return new AutoSyncReport { Started = started, Stopped = stopped, Running = RunningCount(vm) };
    }

    public int RunningCount(EventVm vm) =>
        _running.Values.Count(id => vm.FindInstance(id) != null);

    public bool IsRunning(EventVm vm, int eventId) =>
        _running.TryGetValue(eventId, out int id) && vm.FindInstance(id) != null;

    /// <summary>Number of fresh starts recorded for one event by this scheduler instance.</summary>
    public int StartsFor(int eventId) => _starts.TryGetValue(eventId, out int count) ? count : 0;

    private static bool Trace => Environment.GetEnvironmentVariable("LWsmf_TRACE_AUTO") == "1";

    private static void TraceStart(int eventId, int instanceId)
    {
        if (Trace) Console.Error.WriteLine($"[AutoEvents] start event {eventId} instance {instanceId}");
    }

    private static void TraceStop(int eventId, int instanceId)
    {
        if (Trace) Console.Error.WriteLine($"[AutoEvents] stop event {eventId} instance {instanceId}");
    }

    private static bool IsEventLive(EventVm vm, int eventId)
    {
        foreach (var inst in vm.Instances)
        {
            if (inst.EventId != eventId) continue;
            if (inst.Pc < inst.Program.Len || inst.Wait.Kind != WaitStateKind.None) return true;
        }
        return false;
    }

    private static int ConditionValue(EventVm vm, int raw)
    {
        int index = raw - Magic.VarBase;
        return index >= 0 && index < 800 ? vm.Vars.Get(index) : 0;
    }

    private static bool ConditionMet(int op, int left, int right) => op switch
    {
        1 => left >= right,
        2 => left == right,
        3 => left <= right,
        _ => false
    };
}
