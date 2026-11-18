using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// B1 natural battle closure without an external save: start a synthetic two-member
/// party in CE188, let the shipped command menu register rows in CData 18/73/0, then
/// hand the first two harvested commands to a real CE239/CE242/CE244 battle. CE188's
/// own CE189/CE143 materializes CData type 10 from CData type 0, so this path does not
/// import work/retail_backup/Save/SaveData01.sav. The enemy row is deliberately weakened
/// so the fixture is deterministic; the point under test is the natural event chain, not
/// retail balance.
/// </summary>
public static class BattleNaturalLoopTest
{
    private const int CmdListType = 14;
    private const int PartyCountType = 18;
    private const int PartyTableType = 6;
    private const int StatusType = 0;
    private const int LeaderRow = 12;
    private const int Member2Row = 13;

    private sealed class Outcome
    {
        public int MenuRows;
        public readonly List<int> Skills = new();
        public readonly List<int> Activators = new();
        public readonly List<(int Pc, int Arg)> Bodies = new();
        public long Rounds;
        public long Turns;
        public int Result;
        public int Gauge;
        public int PartyHp;
        public bool Saw143;
        public bool Saw159;
        public bool Saw162;
        public bool BattleStarted;
        public bool Decided;
        public bool Faulted;
        public string? FaultReason;
    }

    public static void Run()
    {
        var outcome = RunNatural();
        Check(!outcome.Faulted, "natural battle faulted: " + outcome.FaultReason);
        Check(outcome.MenuRows >= 8, $"expected eight registered menu rows, got {outcome.MenuRows}");
        Check(outcome.Skills.Count >= 2, $"expected two real skills, got {string.Join(",", outcome.Skills)}");
        Check(outcome.Skills[0] == 26, $"first real skill must be 26, got {outcome.Skills[0]}");
        Check(outcome.Skills[1] == 25, $"second real skill must be 25, got {outcome.Skills[1]}");
        Check(outcome.Activators.Count >= 2 && outcome.Activators[0] == 0 && outcome.Activators[1] == 1,
            $"real menu activators must be 0,1, got {string.Join(",", outcome.Activators)}");
        Check(outcome.BattleStarted, "CE239 never started from the harvested menu");
        Check(outcome.Decided, "CE244 never decided the battle");
        Check(outcome.Result == 1, $"natural battle must settle +1, got {outcome.Result}");
        Check(outcome.Rounds >= 2, $"natural battle must run at least two command rounds, got {outcome.Rounds}");
        Check(outcome.Turns >= 1, $"natural battle must start a later turn, got {outcome.Turns}");
        Check(outcome.Bodies.Count >= 2, $"expected at least two CE242 bodies, got {outcome.Bodies.Count}");
        Check(outcome.Bodies[0].Arg == 1 && outcome.Bodies[1].Arg == 2,
            $"first CE242 args must be 1,2, got {string.Join(",", outcome.Bodies.Select(b => b.Arg))}");
        Check(outcome.Gauge <= 0, $"enemy gauge must be exhausted, got {outcome.Gauge}");
        Check(outcome.PartyHp > 0, $"party must survive, got {outcome.PartyHp}");
        Check(outcome.Saw143, "CE188 never ran CE143 ability materialization");
        Check(outcome.Saw159, "natural menu never dispatched CE159");
        Check(outcome.Saw162, "CE162 target selection never ran");

        GD.Print($"BATTLE_NATURAL_LOOP_TEST_OK rows={outcome.MenuRows} real=2 rounds={outcome.Rounds} turns={outcome.Turns} result={outcome.Result} pcs={string.Join(",", outcome.Bodies.Select(b => b.Pc))} args={string.Join(",", outcome.Bodies.Select(b => b.Arg))} gauge={outcome.Gauge} hp={outcome.PartyHp} ce143={outcome.Saw143} ce159={outcome.Saw159} ce162={outcome.Saw162} save_vars=0 save_cells=0");

        var defeat = RunNatural(weakEnemy: false);
        Check(!defeat.Faulted, "natural defeat faulted: " + defeat.FaultReason);
        Check(defeat.BattleStarted, "CE239 never started for the defeat fixture");
        Check(defeat.Decided, "CE244 never decided the defeat fixture");
        Check(defeat.Result == -1, $"natural defeat must settle -1, got {defeat.Result}");
        Check(defeat.Rounds > 2, $"natural defeat must outlast the victory budget, got {defeat.Rounds}");
        Check(defeat.PartyHp <= 0, $"defeat must drain the party, got {defeat.PartyHp}");
        GD.Print($"BATTLE_NATURAL_DEFEAT_TEST_OK rows={defeat.MenuRows} rounds={defeat.Rounds} turns={defeat.Turns} result={defeat.Result} gauge={defeat.Gauge} hp={defeat.PartyHp} save_vars=0 save_cells=0");
    }

    private static Outcome RunNatural(bool weakEnemy = true)
    {
        var outcome = new Outcome();
        var vm = new VmController();
        try
        {
            Load(vm, out var store);
            SeedBattle(store);
            SeedTwoMemberParty(store);
            // Make skills 26/25 battle-usable and selectable against a single enemy.
            SetData(store, 0, 26, 2, 10); SetData(store, 0, 26, 6, 1);
            SetData(store, 0, 25, 2, 10); SetData(store, 0, 25, 6, 1);
            // Deterministic weak enemy for the victory fixture: CE239 reads CData
            // type 9 row 0 cols 2..5. The defeat fixture keeps the retail boss row.
            if (weakEnemy)
            {
                SetData(store, 9, 0, 2, 100); SetData(store, 9, 0, 3, 50);
                SetData(store, 9, 0, 4, 10); SetData(store, 9, 0, 5, 5);
            }

            var autoEvents = AutoEvents.FromPrograms(vm.Vm.Programs.Values);
            var wiring = new BattleWiring();
            wiring.SetAutoArm(true);
            wiring.SetEnabled(true);
            wiring.SetRoundLoop(true);
            vm.Vm.Rng.SetEventSeed(9);
            vm.Vm.Vars.Set(445, 1);
            vm.Vm.Vars.Set(446, 4000);
            int frame = 0;
            vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
            Check(vm.StartEvent(188), "CE188 failed to start");

            var seenBodies = new HashSet<int>();
            for (frame = 0; frame < 40000; frame++)
            {
                autoEvents.Sync(vm.Vm);
                wiring.Tick(vm.Vm, op => vm.Database(op));
                if (!outcome.BattleStarted && wiring.MenuSelections.Count >= 2)
                {
                    outcome.BattleStarted = true;
                    // This fixture bypasses CE253's region dispatch, which normally
                    // fills the party battle-power vars from the live party.
                    for (int i = 0; i < 6; i++)
                    {
                        vm.Vm.Vars.Set(40 + i, 3000);
                        vm.Vm.Vars.Set(170 + i, 3000);
                    }
                    Check(vm.StartEvent(239), "CE239 failed to start");
                }

                if (vm.IsWaitingForChoice()) vm.SelectChoice(0);
                vm.StepFrame();
                if (vm.IsWaitingForMessage()) vm.DismissMessage();

                foreach (var inst in vm.Vm.Instances)
                {
                    if (inst.EventId == 143) outcome.Saw143 = true;
                    if (inst.EventId == 159) outcome.Saw159 = true;
                    if (inst.EventId == 162) outcome.Saw162 = true;
                    if (inst.EventId == 242 && inst.Pc > 143 && seenBodies.Add(inst.Id))
                        outcome.Bodies.Add((inst.Pc, inst.Self.Get(Magic.SelfArgBase)));
                }

                if (vm.Vm.Vars.Get(343) == 1 && vm.Vm.Vars.Get(350) == 0)
                {
                    outcome.Decided = true;
                    break;
                }
                if (vm.Vm.Faulted) break;
            }

            outcome.MenuRows = wiring.MenuSelections.Count;
            foreach (var selection in wiring.MenuSelections)
            {
                outcome.Skills.Add(selection.Skill);
                outcome.Activators.Add(selection.Activator);
            }
            outcome.Rounds = wiring.StartedRounds;
            outcome.Turns = wiring.TurnStarts;
            outcome.Result = vm.Vm.Vars.Get(352);
            outcome.Gauge = vm.Vm.Vars.Get(342);
            outcome.PartyHp = vm.Vm.Vars.Get(29);
        }
        catch (Exception e)
        {
            outcome.Faulted = true;
            outcome.FaultReason = e.Message;
        }
        finally
        {
            vm.Free();
        }
        return outcome;
    }

    private static void Load(VmController vm, out GameDatabase store)
    {
        var dataDir = DataDirectory();
        Check(vm.LoadPrograms(Path.Combine(dataDir, "programs.json")), "programs failed to load");
        store = vm.DatabaseStore!;
        Check(store != null, "database store missing");
        vm.Vm.Host = vm;
        vm.Vm.StrictMode = false;
    }

    private static void SeedBattle(GameDatabase store)
    {
        Set(store, PartyCountType, 4, 0, 1);
        Set(store, PartyCountType, 69, 0, 1);
        int[] slots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < slots.Length; i++) Set(store, StatusType, LeaderRow, 47 + i, slots[i]);
        int[] stats = [2000, 2000, 800, 400, 200, 200, 30, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < stats.Length; i++) Set(store, StatusType, LeaderRow, columns[i], stats[i]);
        int[] gear = [20000, 30000, 30000, 30000, 30000];
        for (int i = 0; i < gear.Length; i++) Set(store, StatusType, LeaderRow, 28 + i, gear[i]);
        Set(store, 7, 0, 0, 1); Set(store, 8, 0, 0, 1);
        Set(store, 8, 1, 0, 1); Set(store, 8, 2, 0, 1);
        Set(store, PartyCountType, 73, 0, 1);
        Set(store, 20, 73, 0, 0);
        for (int row = 0; row <= 8; row++)
        {
            Set(store, CmdListType, row, 0, -1);
            Set(store, CmdListType, row, 3, -1);
        }
    }

    private static void SeedTwoMemberParty(GameDatabase store)
    {
        Set(store, PartyCountType, 4, 0, 2);
        Set(store, PartyTableType, 0, 1, LeaderRow);
        Set(store, PartyTableType, 0, 2, Member2Row);
        Set(store, StatusType, Member2Row, 5, 99);
        Set(store, StatusType, Member2Row, 6, 99);
        Set(store, StatusType, Member2Row, 7, 1);
        Set(store, StatusType, Member2Row, 8, 1);
        int[] stats = [2000, 2000, 800, 400, 200, 200, 30, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < stats.Length; i++) Set(store, StatusType, Member2Row, columns[i], stats[i]);
        int[] commands = [4, 3, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < commands.Length; i++) Set(store, StatusType, Member2Row, 47 + i, commands[i]);
    }

    private static void Set(GameDatabase store, int type, int row, int column, int value)
        => store.Execute(new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value)
        });

    private static void SetData(GameDatabase store, int type, int row, int column, int value)
        => store.Execute(new DbOp
        {
            Container = DbContainer.Data,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value)
        });

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("BATTLE_NATURAL_LOOP_TEST_FAILED: " + message);
    }
}
