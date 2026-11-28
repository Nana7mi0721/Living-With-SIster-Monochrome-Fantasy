using System;
using System.Collections.Generic;

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
    public bool IsParallel { get; init; }
}

public readonly struct AutoSyncReport
{
    public int Started { get; init; }
    public int Stopped { get; init; }
    public int Running { get; init; }
}

/// <summary>
/// Scheduler for run_condition == 2 common events, plus the two battle events
/// (242/243) that the data gates on 2000350 without an explicit caller. Mirrors the
/// edge-triggered scheduler semantics: a coroutine that ended on its own is not
/// restarted while its condition stays true. Only the HUD painters (216/217) and
/// H-scene gauge painter (339) are allow-listed in ResumeOnLoad to replay every frame;
/// CE240 is strictly excluded because it consumes PRNG.
/// Event 43 is skipped here because the title entry point starts it explicitly.
/// </summary>
public sealed class AutoEvents
{
    /// <summary>Event 43 is the title menu; the title entry point starts it explicitly.</summary>
    public static readonly int[] Skipped = { WolfIds.TitleMenu };

    /// <summary>run_condition == 0 events the battle state machine needs woken by condition.</summary>
    public static readonly int[] GatedCallOnly = { WolfIds.BattleCommandPhase, WolfIds.BattleEnemyAction };

    /// <summary>Linear HUD and gauge painters that must be re-created every frame while their condition holds.</summary>
    public static readonly int[] ResumeOnLoad =
    {
        WolfIds.HudPainterA,
        WolfIds.HudPainterB,
        WolfIds.HSceneGauge
    };

    /// <summary>
    /// Continuous battle and late-night H-scene visual painters exempt from post-load suppression
    /// when LoopVisualPainters is active (imported retail H-scene saves have no coroutine stack).
    /// </summary>
    private static readonly int[] LiveLoadResumePainters =
    {
        WolfIds.BattleGauge,
        334, 335, 338, 348, 349, 350, 351, 352, 353, 355, 356, 357, 367, 413, 414
    };

    private static bool IsLiveLoadResumeExempt(EventVm vm, int eventId)
    {
        if (Array.IndexOf(LiveLoadResumePainters, eventId) >= 0) return true;
        // CE404 (cowgirl differential motion) gates on var[137]==6, which can remain 6 after
        // leaving H-scene; resume on load only while var[62]>=1 confirms an active H-scene.
        return eventId == 404 && vm.Vars.Get(62) >= 1;
    }

    private readonly List<AutoEvent> _events;
    private readonly HashSet<int> _rebindEventIds = new();
    private readonly Dictionary<int, bool> _latched = new();
    private readonly Dictionary<int, int> _running = new();
    private readonly Dictionary<int, int> _starts = new();
    // Non-allow-listed run_condition==2 events whose condition already held at save load
    // stay suppressed until their gate variable drops back to false (prevents stale CE506/CE337 replay).
    private readonly HashSet<int> _suppressedUntilFalse = new();
    private readonly HashSet<int> _liveEventIds = new();
    private readonly HashSet<int> _liveInstanceIds = new();
    private readonly Dictionary<int, int> _parallelFirstId = new();
    private readonly Dictionary<int, List<int>> _parallelExtraIds = new();
    private bool _hasExtraParallel;

    private AutoEvents(List<AutoEvent> events)
    {
        _events = events;
        for (int i = 0; i < events.Count; i++)
        {
            var ev = events[i];
            if (ev.IsParallel || ev.Operator != 0)
            {
                _rebindEventIds.Add(ev.Id);
            }
        }
    }

    public bool Enabled { get; set; } = true;
    /// <summary>When true, all parallel (run_condition == 2) common events except sentinel CE38 loop while their condition holds.</summary>
    public bool LoopVisualPainters { get; set; }
    public int EventCount => _events.Count;
    public IReadOnlyList<AutoEvent> Events => _events;

    public static AutoEvents FromPrograms(IEnumerable<Program> programs)
    {
        var events = new List<AutoEvent>();
        foreach (var p in programs)
        {
            if (Array.IndexOf(Skipped, p.Id) >= 0) continue;
            if (p.RunCondition != 2 && !(p.RunCondition == 0 && Array.IndexOf(GatedCallOnly, p.Id) >= 0))
                continue;
            events.Add(new AutoEvent
            {
                Id = p.Id,
                Title = p.Title,
                Operator = p.ConditionOperator,
                Variable = p.ConditionVariable,
                Value = p.ConditionValue,
                ResumeOnLoad = Array.IndexOf(ResumeOnLoad, p.Id) >= 0,
                IsParallel = p.RunCondition == 2
            });
        }
        events.Sort((a, b) => a.Id.CompareTo(b.Id));
        return new AutoEvents(events);
    }

    /// <summary>Seed the latched state after a load, rebind restored parallel instances, then replay HUD painters once.</summary>
    public void PrimeLatched(EventVm vm)
    {
        if (!Enabled) return;
        RebindRunning(vm);
        CollectInstanceIndex(vm);
        _suppressedUntilFalse.Clear();
        foreach (var ev in _events)
        {
            bool now = ConditionMet(ev.Operator, ConditionValue(vm, ev.Variable), ev.Value);
            _latched[ev.Id] = now;
            if (!ev.ResumeOnLoad && !IsLiveLoadResumeExempt(vm, ev.Id)
                && now && !_liveEventIds.Contains(ev.Id))
            {
                _suppressedUntilFalse.Add(ev.Id);
            }
        }
        Sync(vm);
    }

    /// <summary>Rebuild the active parallel instance index from live VM coroutines (e.g. after save restore).</summary>
    public void RebindRunning(EventVm vm)
    {
        _running.Clear();
        var instances = vm.Instances;
        for (int i = 0; i < instances.Count; i++)
        {
            var inst = instances[i];
            if (inst.IsParallel && _rebindEventIds.Contains(inst.EventId))
            {
                _running.TryAdd(inst.EventId, inst.Id);
            }
        }
    }

    public AutoSyncReport Sync(EventVm vm)
    {
        int started = 0, stopped = 0;
        if (!Enabled) return new AutoSyncReport { Running = RunningCount(vm) };
        CollectInstanceIndex(vm);
        for (int i = 0; i < _events.Count; i++)
        {
            var ev = _events[i];
            bool now = ConditionMet(ev.Operator, ConditionValue(vm, ev.Variable), ev.Value);
            if (!now)
            {
                _suppressedUntilFalse.Remove(ev.Id);
            }
            bool alive = _liveEventIds.Contains(ev.Id);
            bool before = _latched.TryGetValue(ev.Id, out bool latched) && latched;
            bool suppressed = _suppressedUntilFalse.Contains(ev.Id);
            bool shouldStart;
            if (LoopVisualPainters)
            {
                // In live manual play, Map1005 Ev8 calls CE242 and CE242 calls CE243 directly;
                // AutoEvents must not auto-spawn non-parallel GatedCallOnly (242/243) or CE243
                // duplicates the enemy turn and steals the var350==2 handoff from CE241.
                if (!ev.IsParallel)
                {
                    shouldStart = false;
                }
                else
                {
                    bool replay = !suppressed && (ev.ResumeOnLoad || ev.Id != 38);
                    shouldStart = !alive && (replay ? now : (now && !before));
                }
            }
            else
            {
                // Keep Rust auto_events.rs edge-triggered parity when LoopVisualPainters is false;
                // live play sets LoopVisualPainters = true, which enforces !alive above.
                shouldStart = ev.ResumeOnLoad && now ? !alive : now && !before;
            }
            if (shouldStart)
            {
                var inst = vm.CreateInstance(ev.Id);
                if (inst != null)
                {
                    inst.IsParallel = true;
                    _running[ev.Id] = inst.Id;
                    _liveInstanceIds.Add(inst.Id);
                    _starts[ev.Id] = StartsFor(ev.Id) + 1;
                    started++;
                    TraceStart(ev.Id, inst.Id);
                }
            }
            else if (!now)
            {
                int trackedId = -1;
                if (_running.Remove(ev.Id, out int instanceId))
                {
                    trackedId = instanceId;
                    if (_liveInstanceIds.Contains(instanceId) && vm.FindInstance(instanceId) != null)
                    {
                        vm.StopInstance(instanceId);
                        stopped++;
                        TraceStop(ev.Id, instanceId);
                    }
                }
                if ((ev.IsParallel || ev.Operator != 0) && _parallelFirstId.TryGetValue(ev.Id, out int firstParallelId))
                {
                    if (firstParallelId != trackedId && vm.FindInstance(firstParallelId) != null)
                    {
                        vm.StopInstance(firstParallelId);
                        stopped++;
                        TraceStop(ev.Id, firstParallelId);
                    }
                    if (_hasExtraParallel && _parallelExtraIds.TryGetValue(ev.Id, out var extras))
                    {
                        for (int k = 0; k < extras.Count; k++)
                        {
                            int orphanId = extras[k];
                            if (orphanId != trackedId && vm.FindInstance(orphanId) != null)
                            {
                                vm.StopInstance(orphanId);
                                stopped++;
                                TraceStop(ev.Id, orphanId);
                            }
                        }
                    }
                }
            }
            _latched[ev.Id] = now;
        }
        int running = stopped == 0 ? CountTrackedRunningFromIndex() : RunningCount(vm);
        return new AutoSyncReport { Started = started, Stopped = stopped, Running = running };
    }

    private void CollectInstanceIndex(EventVm vm)
    {
        _liveEventIds.Clear();
        _liveInstanceIds.Clear();
        _parallelFirstId.Clear();
        if (_hasExtraParallel)
        {
            foreach (var list in _parallelExtraIds.Values) list.Clear();
            _hasExtraParallel = false;
        }

        var instances = vm.Instances;
        for (int i = 0; i < instances.Count; i++)
        {
            var inst = instances[i];
            _liveInstanceIds.Add(inst.Id);
            if (inst.Pc < inst.Program.Len || inst.Wait.Kind != WaitStateKind.None)
            {
                _liveEventIds.Add(inst.EventId);
            }
            if (inst.IsParallel && !_parallelFirstId.TryAdd(inst.EventId, inst.Id))
            {
                if (!_parallelExtraIds.TryGetValue(inst.EventId, out var extra))
                {
                    extra = new List<int>(2);
                    _parallelExtraIds[inst.EventId] = extra;
                }
                extra.Add(inst.Id);
                _hasExtraParallel = true;
            }
        }
    }

    private int CountTrackedRunningFromIndex()
    {
        int count = 0;
        foreach (int id in _running.Values)
        {
            if (_liveInstanceIds.Contains(id)) count++;
        }
        return count;
    }

    public int RunningCount(EventVm vm)
    {
        int count = 0;
        foreach (int id in _running.Values)
        {
            if (vm.FindInstance(id) != null) count++;
        }
        return count;
    }

    public bool IsRunning(EventVm vm, int eventId) =>
        _running.TryGetValue(eventId, out int id) && vm.FindInstance(id) != null;

    /// <summary>Number of fresh starts recorded for one event by this scheduler instance.</summary>
    public int StartsFor(int eventId) => _starts.TryGetValue(eventId, out int count) ? count : 0;

    private static void TraceStart(int eventId, int instanceId)
    {
        if (CoreRuntimeConfig.TraceAutoEvents)
            Console.Error.WriteLine($"[AutoEvents] start event {eventId} instance {instanceId}");
    }

    private static void TraceStop(int eventId, int instanceId)
    {
        if (CoreRuntimeConfig.TraceAutoEvents)
            Console.Error.WriteLine($"[AutoEvents] stop event {eventId} instance {instanceId}");
    }

    private static int ConditionValue(EventVm vm, int raw)
    {
        int index = raw - Magic.VarBase;
        return index >= 0 && index < WolfIds.VarPollMax ? vm.Vars.Get(index) : 0;
    }

    private static bool ConditionMet(int op, int left, int right) => op switch
    {
        1 => left >= right,
        2 => left == right,
        3 => left <= right,
        _ => false
    };
}
