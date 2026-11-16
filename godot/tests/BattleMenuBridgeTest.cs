using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of crates/wolf-game/tests/m11_party_live_bridge.rs: build the smallest real
/// two-member party, drive the shipped CE188 command menu with real OK pulses, let
/// BattleWiring harvest the CE157 registrations, then feed those rows into a real
/// CE253 -> CE239 battle. The battle half is save-parity limited: Rust loads
/// work/retail_backup/Save/SaveData01.sav (CE143 materializes CData type 10) and sees
/// four command bodies settle +1, while this headless VM only has the 26 seed cells,
/// so it settles after two real bodies with -1. Both halves still prove the live
/// CE188 -> CE157 -> BattleWiring -> CE253 -> CE242 chain.
/// </summary>
public static class BattleMenuBridgeTest
{
    private const int CmdListType = 14;
    private const int PartyCountType = 18;
    private const int PartyTableType = 6;
    private const int StatusType = 0;
    private const int LeaderRow = 12;
    private const int Member2Row = 13;

    private sealed class MenuOutcome
    {
        public int Rows;
        public int RealRows;
        public readonly List<int> Skills = new();
        public readonly List<int> Activators = new();
        public bool Faulted;
        public string? FaultReason;
    }

    private sealed class BattleOutcome
    {
        public long StartedRounds;
        public long TurnStarts;
        public readonly List<(int Pc, int Arg)> Bodies = new();
        public int Settle;
        public readonly int[] FinalPools = new int[4];
        public readonly List<int> Gauges = new();
        public int SaveVars;
        public int SaveCells;
        public bool Decided;
        public bool Faulted;
        public string? FaultReason;
    }

    public static void Run()
    {
        var menu = RunMenu();
        Check(!menu.Faulted, "menu faulted: " + menu.FaultReason);
        Check(menu.Rows >= 2, $"expected at least two registered menu rows, got {menu.Rows}");
        Check(menu.RealRows >= 2, $"expected two real action rows, got {menu.RealRows}: {Join(menu.Skills)}");
        Check(menu.Skills[0] == 26, $"first real action must be 26, got {menu.Skills[0]}");
        Check(menu.Skills[1] == 25, $"second real action must be 25, got {menu.Skills[1]}");
        Check(menu.Activators[0] == 0, $"row 1 activator must be 0, got {menu.Activators[0]}");
        Check(menu.Activators[1] == 1, $"row 2 activator must be 1, got {menu.Activators[1]}");

        var queue = new List<int>();
        for (int i = 0; i < 4 && i < menu.Rows; i++) queue.Add(Math.Clamp(i + 1, 1, 8));
        while (queue.Count < 4) queue.Add(Math.Clamp(queue.Count + 1, 1, 8));

        var battle = RunBattle(queue);
        Check(!battle.Faulted, "battle faulted: " + battle.FaultReason);
        Check(battle.StartedRounds == 4, $"headless fixture must run four command rounds, got {battle.StartedRounds}");
        Check(battle.Bodies.Count == 4, $"expected four CE242 bodies, got {battle.Bodies.Count}");
        Check(battle.Bodies.ConvertAll(b => b.Pc).SequenceEqual(new[] { 338, 645, 792, 996 }), "CE242 bodies must be 338,645,792,996");
        Check(battle.Bodies.ConvertAll(b => b.Arg).SequenceEqual(new[] { 1, 2, 3, 4 }), "menu queue must be consumed front-first");
        Check(battle.TurnStarts == 3, $"expected three later turn starts, got {battle.TurnStarts}");
        Check(battle.Decided, "2000343 never set");
        Check(battle.Settle == 1, $"CE244 must settle +1 on the save-backed fixture, got {battle.Settle}");
        var finalPools = new[] { battle.FinalPools[0], battle.FinalPools[1], battle.FinalPools[2], battle.FinalPools[3] };
        Check(finalPools.SequenceEqual(new[] { 3, 2, 3, 2 }), $"final action pools must be 3,2,3,2, got {string.Join(",", finalPools)}");

        Check(battle.Gauges.Count == 4, $"victory must observe four round gauges, got {battle.Gauges.Count}");
        Check(battle.Gauges.Zip(battle.Gauges.Skip(1), (a, b) => a > b).All(x => x), $"victory gauge must fall every round: {string.Join(",", battle.Gauges)}");

        var defeat = RunBattle(queue, weakenParty: true);
        Check(!defeat.Faulted, "defeat fixture faulted: " + defeat.FaultReason);
        Check(defeat.Decided, "defeat fixture never settled");
        Check(defeat.Settle == -1, $"weakened party must lose, got {defeat.Settle}");
        Check(defeat.StartedRounds > 4, $"defeat must outlast the four-round budget, got {defeat.StartedRounds}");
        Check(defeat.Gauges.Count == defeat.StartedRounds && defeat.Gauges.All(g => g > 0), $"defeat must leave enemy gauge positive: {string.Join(",", defeat.Gauges)}");
        GD.Print($"BATTLE_MATRIX_TEST_OK win=1 loss={defeat.Settle} win_rounds={battle.StartedRounds} loss_rounds={defeat.StartedRounds} win_gauges={string.Join(",", battle.Gauges)} loss_gauges={string.Join(",", defeat.Gauges)}");

        GD.Print($"BATTLE_MENU_BRIDGE_TEST_OK rows={menu.Rows} real={menu.RealRows} rounds={battle.StartedRounds} turns={battle.TurnStarts} result={battle.Settle} pcs={Join(battle.Bodies.ConvertAll(b => b.Pc))} pools={string.Join(",", finalPools)} save_vars={battle.SaveVars} save_cells={battle.SaveCells}");
    }

    private static MenuOutcome RunMenu()
    {
        var outcome = new MenuOutcome();
        var vm = new VmController();
        try
        {
            Load(vm, out var store);
            SeedBattle(store!);
            SeedTwoMemberParty(store!);

            var autoEvents = AutoEvents.FromPrograms(vm.Vm.Programs.Values);
            var wiring = new BattleWiring();
            wiring.SetAutoArm(true);
            int frame = 0;
            vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
            Check(vm.StartEvent(188), "CE188 failed to start");

            int lastGrowth = 0;
            int best = 0;
            for (frame = 0; frame < 2500; frame++)
            {
                autoEvents.Sync(vm.Vm);
                wiring.Tick(vm.Vm, op => vm.Database(op));
                if (vm.IsWaitingForChoice()) vm.SelectChoice(0);
                vm.StepFrame();
                if (vm.IsWaitingForMessage()) vm.DismissMessage();

                if (wiring.MenuSelections.Count > best)
                {
                    best = wiring.MenuSelections.Count;
                    lastGrowth = frame;
                }
                if (best >= 2 && frame - lastGrowth > 80) break;
            }

            outcome.Rows = wiring.MenuSelections.Count;
            foreach (var selection in wiring.MenuSelections)
            {
                outcome.Skills.Add(selection.Skill);
                outcome.Activators.Add(selection.Activator);
                if (selection.Skill is 26 or 25 or 30 or 29) outcome.RealRows++;
            }
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

    private static BattleOutcome RunBattle(IReadOnlyList<int> queue, bool weakenParty = false)
    {
        var outcome = new BattleOutcome();
        var vm = new VmController();
        try
        {
            Load(vm, out var store);
            var savePath = System.IO.Path.GetFullPath(System.IO.Path.Combine(ProjectSettings.GlobalizePath("res://"), "..", "work", "retail_backup", "Save", "SaveData01.sav"));
            Check(System.IO.File.Exists(savePath), "retail save missing: " + savePath);
            var save = LegacySaveImporter.ImportFile(savePath, 0);
            int saveVars = 0;
            foreach (var pair in save.Recovered.Variables)
            {
                if (int.TryParse(pair.Key, out int index)) { vm.Vm.Vars.Set(index, pair.Value); saveVars++; }
            }
            int saveCells = store!.ApplySavedCells(save.Recovered.DatabaseCells);
            outcome.SaveVars = saveVars;
            outcome.SaveCells = saveCells;
            if (weakenParty)
            {
                // Keep the same save-backed fixture but strip the party's offense so
                // the enemy gauge cannot reach zero inside the round budget; CE244
                // then takes its defeat branch (352 = -1).
                for (int i = 0; i < 6; i++) vm.Vm.Vars.Set(170 + i, 0);
                vm.Vm.Vars.Set(28, 1);
                vm.Vm.Vars.Set(392, 0);
            }
            SeedBattle(store!);
            SeedTwoMemberParty(store!);

            var autoEvents = AutoEvents.FromPrograms(vm.Vm.Programs.Values);
            autoEvents.PrimeLatched(vm.Vm);
            var wiring = new BattleWiring();
            wiring.SetAutoArm(true);
            wiring.SetEnabled(true);
            wiring.SetRoundLoop(true);
            wiring.SetMenuCommandQueue(queue);
            vm.Vm.Rng.SetEventSeed(9);
            // The m11 fixture raises the boss gauge; the party numbers themselves
            // come from the retail save, not from these two variables.
            vm.Vm.Vars.Set(445, 1);
            vm.Vm.Vars.Set(446, 4000);

            var seen = new HashSet<int>();
            long lastRounds = 0;
            int frame = 0;
            vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
            // Rust m11 open_fixture_game calls start_engine_events(): the always-on
            // system events plus the auto-start and implicit entries.
            foreach (int id in new[] { 48, 261, 269, 270, 271, 224, 225, 213, 214, 41, 241, 63 }) vm.StartEvent(id);
            Check(vm.StartEvent(253), "CE253 failed to start");

            for (frame = 0; frame < 40000; frame++)
            {
                autoEvents.Sync(vm.Vm);
                wiring.Tick(vm.Vm, op => vm.Database(op));
                if (wiring.StartedRounds > lastRounds)
                {
                    lastRounds = wiring.StartedRounds;
                    outcome.Gauges.Add(vm.Vm.Vars.Get(342));
                }
                if (vm.IsWaitingForChoice()) vm.SelectChoice(0);
                vm.StepFrame();
                if (vm.IsWaitingForMessage()) vm.DismissMessage();

                foreach (var inst in vm.Vm.Instances)
                {
                    if (inst.EventId == 242 && inst.Pc > 143 && seen.Add(inst.Id))
                        outcome.Bodies.Add((inst.Pc, inst.Self.Get(Magic.SelfArgBase)));
                }
                if (vm.Vm.Vars.Get(343) == 1 && vm.Vm.Vars.Get(350) == 0)
                {
                    outcome.Settle = vm.Vm.Vars.Get(352);
                    outcome.Decided = true;
                    break;
                }
                if (vm.Vm.Faulted) break;
            }

            for (int i = 0; i < 4; i++) outcome.FinalPools[i] = vm.Vm.Vars.Get(345 + i);
            outcome.StartedRounds = wiring.StartedRounds;
            outcome.TurnStarts = wiring.TurnStarts;
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

    private static void Load(VmController vm, out GameDatabase? store)
    {
        var dataDir = DataDirectory();
        Check(vm.LoadPrograms(Path.Combine(dataDir, "programs.json")), "programs failed to load");
        store = vm.DatabaseStore;
        Check(store != null, "database store missing");
        vm.Vm.Host = vm;
        vm.Vm.StrictMode = false;
    }

    private static void SeedBattle(GameDatabase store)
    {
        Set(store, 18, 4, 0, 1);   // party count; overwritten to 2 by SeedTwoMemberParty
        Set(store, 18, 69, 0, 1);  // enemy count
        int[] slots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < slots.Length; i++) Set(store, StatusType, LeaderRow, 47 + i, slots[i]);
        int[] seeds = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < seeds.Length; i++) Set(store, StatusType, LeaderRow, columns[i], seeds[i]);
        int[] gear = [20000, 30000, 30000, 30000, 30000];
        for (int i = 0; i < gear.Length; i++) Set(store, StatusType, LeaderRow, 28 + i, gear[i]);
        Set(store, 7, 0, 0, 1);
        Set(store, 8, 0, 0, 1);
        Set(store, 8, 1, 0, 1);
        Set(store, 8, 2, 0, 1);
        Set(store, 18, 73, 0, 1);  // コマンド入力位置 starts at row 1
        Set(store, 20, 73, 0, 0);  // the old wrong-position cell stays zero
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

        int[] seeds = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < seeds.Length; i++) Set(store, StatusType, Member2Row, columns[i], seeds[i]);
        int[] commands = [4, 3, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < commands.Length; i++) Set(store, StatusType, Member2Row, 47 + i, commands[i]);
    }

    private static void Set(GameDatabase store, int type, int row, int column, int value)
    {
        store.Execute(new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value)
        });
    }

    private static int ReadNum(GameDatabase store, int type, int row, int column)
    {
        var value = store.Read(new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Read,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Destination = 0
        });
        return value == null || value.IsString ? 0 : value.Num;
    }

    private static string Join(IEnumerable<int> values) => string.Join(",", values);

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', (char)92);
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"BATTLE_MENU_BRIDGE_TEST_FAILED: {message}");
    }
}