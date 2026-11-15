using System;
using System.Collections.Generic;
using System.Linq;

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
    public const int VarCommandPhase = 350;
    public const int VarBattleActive = 340;
    public const int VarBattleDecided = 343;
    public const int EventCommandPhase = 242;
    public const int EventTurnStart = 241;
    public const int EventEnemyAction = 243;
    public const int EventBattleDispatch = 253;
    public const int EventBattleCaller = 239;
    public const int EventCommandRegister = 157;
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

    public long StartedRounds { get; private set; }
    public long TurnStarts { get; private set; }
    public bool AutoArm => _autoArm;
    public bool? ManualEnable => _manualEnable;
    public bool? ManualRoundLoop => _manualRoundLoop;
    public IReadOnlyList<MenuSelection> MenuSelections => _menuSelections;
    public int MenuQueueLength => _menuQueue.Count;

    public void SetEnabled(bool enabled) => _manualEnable = enabled;

    public void SetAutoArm(bool enabled) => _autoArm = enabled;

    public void SetRoundLoop(bool enabled)
    {
        _roundLoop = enabled;
        _manualRoundLoop = enabled;
    }

    public void SetCommandSchedule(IReadOnlyList<int> schedule)
    {
        _schedule = schedule.Count == 0 ? [ActingPartySlot] : schedule.ToList();
        _cursor = 0;
    }

    public void SetMenuCommandQueue(IEnumerable<int> commands)
    {
        _menuQueue.Clear();
        foreach (int command in commands) _menuQueue.Enqueue(CommandIndexForRow(command));
    }

    public static int CommandIndexForRow(int row) => Math.Clamp(row, 1, 8);

    /// <summary>True when CE253 is resident or CE239 was started by a parent script.</summary>
    public static bool RealBattleRunning(EventVm vm)
        => vm.Instances.Any(i => i.EventId == EventBattleDispatch)
           || vm.Instances.Any(i => i.EventId == EventBattleCaller && i.ParentId.HasValue);

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
    {
        bool battleActive = vm.Vars.Get(VarBattleActive) != 0;
        bool rising = battleActive && !_prevBattleActive;
        _prevBattleActive = battleActive;

        bool auto = _autoArm && RealBattleRunning(vm);
        bool drive = _manualEnable ?? auto;

        // Harvest even when the drive is off: the menu is confirmed in ordinary map state,
        // before the CE253 dispatch arms the drive.
        HarvestMenuRegistrations(vm, database);
        if (!drive) return null;

        bool roundLoop = _manualRoundLoop ?? auto;

        if (!roundLoop)
        {
            if (!rising) return null;
            if (vm.Vars.Get(VarCommandPhase) != 0 || vm.Vars.Get(VarBattleDecided) != 0) return null;
            return StartCommand(vm, NextCommand());
        }

        if (!battleActive) return null;
        if (vm.Vars.Get(VarBattleDecided) != 0) return null;

        int phase = vm.Vars.Get(VarCommandPhase);
        bool c241 = vm.Instances.Any(i => i.EventId == EventTurnStart);
        bool c242 = vm.Instances.Any(i => i.EventId == EventCommandPhase);
        bool c243 = vm.Instances.Any(i => i.EventId == EventEnemyAction);

        if (rising && phase == 0 && !c242 && !c241 && !c243)
        {
            return StartCommand(vm, NextCommand());
        }

        if (phase == 2 && !c241 && !c242 && !c243)
        {
            if (vm.CreateFreshInstance(EventTurnStart) == null) return null;
            TurnStarts++;
            return EventTurnStart;
        }

        if (phase == 0 && !c241 && !c242 && !c243 && StartedRounds > 0)
        {
            return StartCommand(vm, NextCommand());
        }

        return null;
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
