using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>One real battle-menu confirmation captured from a live CE157 instance.</summary>
public sealed class MenuSelection
{
    public int Activator { get; init; }
    public int Skill { get; init; }
    public int Row { get; init; }
    public int CommandIndex { get; init; }
}

/// <summary>
/// Port of crates/wolf-game/src/battle_wiring.rs: the shipped event graph never closes the
/// command-phase bootstrap (CE242 needs 2000350 == 1 and is its only writer), so the engine
/// starts CE242 on the 2000340 rising edge and, in round-loop mode, chains the real CE241
/// turn-start event between rounds. It never writes 2000350/352/343 itself.
/// </summary>
public sealed class BattleWiring
{
    public const int VarCommandPhase = WolfIds.VarBattleCommandPhase;
    public const int VarBattleActive = WolfIds.VarBattleActive;
    public const int VarBattleDecided = WolfIds.VarBattleDecided;
    public const int EventCommandPhase = WolfIds.BattleCommandPhase;
    public const int EventTurnStart = WolfIds.BattleTurnStart;
    public const int EventEnemyAction = WolfIds.BattleEnemyAction;
    public const int EventBattleDispatch = WolfIds.BattleDispatch;
    public const int EventBattleCaller = WolfIds.BattleCaller;
    public const int EventCommandRegister = WolfIds.BattleCommandRegister;
    public const int CmdListType = 14;
    private const int ActingPartySlot = 1;

    private bool? _manualEnable;
    private bool? _manualRoundLoop;
    private bool _autoArm = true;
    private bool _roundLoop;
    private bool _prevBattleActive;
    private List<int> _schedule = [ActingPartySlot, 3];
    private int _cursor;
    private readonly List<MenuSelection> _menuSelections = new();
    private readonly HashSet<int> _harvestedRows = new();
    private readonly Queue<int> _menuQueue = new();
    private int _menuCursor = -1;
    private bool _sawDispatch;
    private bool _sawCaller;

    public long StartedRounds { get; private set; }
    public long TurnStarts { get; private set; }
    /// <summary>CE253 was resident at least once (natural day-4 dispatch observed).</summary>
    public bool SawDispatch => _sawDispatch;
    /// <summary>CE239 with a parent script was resident (CE253 called the real battle).</summary>
    public bool SawCaller => _sawCaller;
    public bool AutoArm => _autoArm;
    public bool? ManualEnable => _manualEnable;
    public bool? ManualRoundLoop => _manualRoundLoop;
    public IReadOnlyList<MenuSelection> MenuSelections => _menuSelections;
    public int MenuQueueLength => _menuQueue.Count;

    /// <summary>Reset all battle wiring state on session transitions (title, new game, load).</summary>
    public void Reset()
    {
        _prevBattleActive = false;
        _cursor = 0;
        _menuCursor = -1;
        _harvestedRows.Clear();
        _menuQueue.Clear();
        _menuSelections.Clear();
        StartedRounds = 0;
        TurnStarts = 0;
        _sawDispatch = false;
        _sawCaller = false;
    }

    public void SetEnabled(bool enabled) => _manualEnable = enabled;

    public void SetAutoArm(bool enabled) => _autoArm = enabled;

    public void SetRoundLoop(bool enabled)
    {
        _roundLoop = enabled;
        _manualRoundLoop = enabled;
    }

    public void SetCommandSchedule(IReadOnlyList<int> schedule)
    {
        _schedule = schedule.Count == 0 ? [ActingPartySlot] : new List<int>(schedule);
        _cursor = 0;
    }

    public void SetMenuCommandQueue(IEnumerable<int> commands)
    {
        _menuQueue.Clear();
        foreach (int command in commands) _menuQueue.Enqueue(CommandIndexForRow(command));
    }

    public static int CommandIndexForRow(int row) => Math.Clamp(row, 1, 8);

    private static bool HasInstance(EventVm vm, int eventId)
    {
        var instances = vm.Instances;
        for (int i = 0; i < instances.Count; i++)
        {
            if (instances[i].EventId == eventId) return true;
        }
        return false;
    }

    private static bool HasChildInstance(EventVm vm, int eventId)
    {
        var instances = vm.Instances;
        for (int i = 0; i < instances.Count; i++)
        {
            var inst = instances[i];
            if (inst.EventId == eventId && inst.ParentId.HasValue) return true;
        }
        return false;
    }

    /// <summary>True when CE253 is resident or CE239 was started by a parent script.</summary>
    public static bool RealBattleRunning(EventVm vm)
        => HasInstance(vm, EventBattleDispatch) || HasChildInstance(vm, EventBattleCaller);

    /// <summary>
    /// Harvest command rows (CData type 14) once CE157 has advanced the committed cursor
    /// (CData 18/73/0). The cursor only moves after a registration is written, so reading
    /// rows 1..cursor-1 avoids the zero-filled cells CE188 seeds at menu start. Row indices
    /// are deduplicated; the row index is the CE242 command index.
    /// </summary>
    public int HarvestMenuRegistrations(EventVm vm, Func<DbOp, DbValue?> database)
    {
        int cursor = ReadNum(database, 18, 73, 0) ?? 1;
        if (cursor < 1) cursor = 1;
        if (_menuCursor < 0)
        {
            _menuCursor = cursor;
            return 0;
        }
        if (cursor < _menuCursor)
        {
            // Next battle menu reset CData 18/73/0 back down; clear per-battle harvest state.
            _menuCursor = cursor;
            _harvestedRows.Clear();
            _menuQueue.Clear();
        }
        if (cursor <= _menuCursor) return 0;

        int added = 0;
        int first = Math.Max(1, _menuCursor);
        int last = Math.Min(8, cursor - 1);
        for (int row = first; row <= last; row++)
        {
            if (_harvestedRows.Contains(row)) continue;
            var activator = ReadNum(database, CmdListType, row, 0);
            if (activator is not int slot || slot < 0) continue;
            int skill = ReadNum(database, CmdListType, row, 3) ?? 0;
            _harvestedRows.Add(row);
            int command = CommandIndexForRow(row);
            _menuSelections.Add(new MenuSelection
            {
                Activator = slot,
                Skill = skill,
                Row = row,
                CommandIndex = command
            });
            _menuQueue.Enqueue(command);
            added++;
        }
        _menuCursor = cursor;
        return added;
    }

    private static int? ReadNum(Func<DbOp, DbValue?> database, int type, int row, int column)
    {
        var value = database(new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Read,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Destination = 0
        });
        return value == null || value.IsString ? null : value.Num;
    }

    /// <summary>
    /// One engine tick, called after the automatic events are synced and before the VM steps.
    /// Returns the event id it started, or null.
    /// </summary>
    public int? Tick(EventVm vm, Func<DbOp, DbValue?> database)
        => Tick(vm, database, allowAutoCommands: true);

    /// <summary>
    /// One engine tick; when <paramref name="allowAutoCommands"/> is false (live manual play),
    /// player commands come from Map1005 Ev8 instead of synthetic CE242 injection, while CE241
    /// turn-start handoffs still run on phase 2 and battle coroutines stop when 2000340 clears.
    /// </summary>
    public int? Tick(EventVm vm, Func<DbOp, DbValue?> database, bool allowAutoCommands)
    {
        if (!_sawDispatch)
        {
            _sawDispatch = HasInstance(vm, EventBattleDispatch);
        }
        if (!_sawCaller)
        {
            _sawCaller = HasChildInstance(vm, EventBattleCaller);
        }
        bool wasBattleActive = _prevBattleActive;
        bool battleActive = vm.Vars.Get(VarBattleActive) != 0;
        bool rising = battleActive && !wasBattleActive;
        _prevBattleActive = battleActive;
        if (wasBattleActive && !battleActive)
        {
            // Keep cumulative telemetry (StartedRounds/TurnStarts/Saw*/MenuSelections) for callers,
            // while clearing per-battle cursor/queue so the next battle harvests cleanly.
            _menuCursor = -1;
            _harvestedRows.Clear();
            _menuQueue.Clear();
            _cursor = 0;
            if (!allowAutoCommands)
            {
                StopBattleInstances(vm);
            }
        }

        bool auto = _autoArm && RealBattleRunning(vm);
        bool drive = _manualEnable ?? auto;

        // Harvest even when the drive is off: the menu is confirmed in ordinary map state,
        // before the CE253 dispatch arms the drive.
        HarvestMenuRegistrations(vm, database);
        if (!drive) return null;

        bool roundLoop = _manualRoundLoop ?? auto;
        bool autoCommands = allowAutoCommands || _manualEnable == true;

        if (!roundLoop)
        {
            if (!autoCommands) return null;
            if (!rising) return null;
            if (vm.Vars.Get(VarCommandPhase) != 0 || vm.Vars.Get(VarBattleDecided) != 0) return null;
            return StartCommand(vm, NextCommand());
        }

        if (!battleActive) return null;
        if (vm.Vars.Get(VarBattleDecided) != 0) return null;

        int phase = vm.Vars.Get(VarCommandPhase);
        bool c241 = HasInstance(vm, EventTurnStart);
        bool c242 = HasInstance(vm, EventCommandPhase);
        bool c243 = HasInstance(vm, EventEnemyAction);

        if (autoCommands && rising && phase == 0 && !c242 && !c241 && !c243)
        {
            return StartCommand(vm, NextCommand());
        }

        if (phase == 2 && !c241 && !c242 && !c243)
        {
            if (vm.CreateFreshInstance(EventTurnStart) == null) return null;
            TurnStarts++;
            return EventTurnStart;
        }

        if (autoCommands && phase == 0 && !c241 && !c242 && !c243 && StartedRounds > 0)
        {
            return StartCommand(vm, NextCommand());
        }

        return null;
    }

    public static void StopBattleInstances(EventVm vm)
    {
        var ids = new List<int>();
        foreach (var inst in vm.Instances)
        {
            if (inst.EventId is WolfIds.BattleGauge or EventTurnStart or EventCommandPhase or EventEnemyAction)
            {
                ids.Add(inst.Id);
            }
        }
        foreach (int id in ids) vm.StopInstance(id);
    }

    private int? StartCommand(EventVm vm, int command)
    {
        var instance = vm.CreateFreshInstance(EventCommandPhase);
        if (instance == null) return null;
        instance.Self.Set(Magic.SelfArgBase, command);
        StartedRounds++;
        _cursor++;
        return EventCommandPhase;
    }

    private int NextCommand()
    {
        if (_menuQueue.Count > 0) return _menuQueue.Dequeue();
        int len = Math.Max(1, _schedule.Count);
        return _schedule.Count == 0 ? ActingPartySlot : _schedule[_cursor % len];
    }
}
