using System;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of the settled-battle assertions in crates/wolf-game/tests/m7_battle_playable.rs:
/// a directly started CE239 with the explicit single-shot wiring, the 26 battle seed cells
/// and real OK pulses must raise 2000340, open CE242 once, settle 2000352 to +/-1 and set
/// 2000343, after which CE239 returns and leaves the instance list.
/// </summary>
public static class BattleWiringTest
{
    private sealed class Trace
    {
        public bool Ce239;
        public bool Ce242;
        public bool Ce241;
        public bool Ce243;
        public bool BattleActive;
        public bool PhaseOne;
        public bool Decided;
        public int Result;
        public int MaxCe242Pc;
        public int MaxCe239Pc;
        public int End239Pc;
        public int End340;
        public int End350;
        public int End343;
        public bool Faulted;
        public string? FaultReason;
        public long Rounds;
        public bool Ce239AliveAtEnd;
    }

    public static void Run()
    {
        var trace = RunLine();
        Check(!trace.Faulted, "faulted: " + trace.FaultReason);
        Check(trace.Ce239, "CE239 never started");
        Check(trace.BattleActive, "2000340 never rose");
        Check(trace.Rounds >= 1, "wiring never started CE242");
        Check(trace.Ce242, "CE242 never observed");
        Check(trace.PhaseOne, "2000350 never reached 1");
        Check(trace.Decided, "2000343 never set");
        Check(Math.Abs(trace.Result) == 1, "2000352 must settle to +/-1, got " + trace.Result);
        Check(trace.MaxCe242Pc >= 200, "CE242 did not execute its command body");
        Check(!trace.Ce239AliveAtEnd, $"CE239 still alive after settlement pc={trace.End239Pc} v340={trace.End340} v350={trace.End350} v343={trace.End343}");
        TestSecondBattleReharvestAndReset();
        TestManualModeNoAutoCommandAndEndCleanup();
        GD.Print($"BATTLE_WIRING_TEST_OK rounds={trace.Rounds} result={trace.Result} ce242_pc={trace.MaxCe242Pc}");
    }

    private static void TestManualModeNoAutoCommandAndEndCleanup()
    {
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(WolfIds.BattleDispatch, "Dispatch", [Slot.Command(180, [10])]));
        vm.RegisterProgram(Program.FromSlots(WolfIds.BattleCommandPhase, "Cmd", [Slot.Command(180, [10])]));
        vm.RegisterProgram(Program.FromSlots(WolfIds.BattleTurnStart, "Turn", [Slot.Command(180, [10])]));
        vm.RegisterProgram(Program.FromSlots(WolfIds.BattleGauge, "Gauge", [Slot.Command(180, [10])]));
        var wiring = new BattleWiring();
        vm.CreateInstance(WolfIds.BattleDispatch);

        vm.Vars.Set(WolfIds.VarBattleActive, 1);
        vm.Vars.Set(WolfIds.VarBattleCommandPhase, 0);
        int? started = wiring.Tick(vm, _ => null, allowAutoCommands: false);
        Check(started == null && wiring.StartedRounds == 0,
            "manual live play must not auto-start CE242 on battle rising edge");

        vm.Vars.Set(WolfIds.VarBattleCommandPhase, 2);
        int? turnStarted = wiring.Tick(vm, _ => null, allowAutoCommands: false);
        Check(turnStarted == WolfIds.BattleTurnStart,
            "manual live play must still advance CE241 on command phase 2");

        vm.CreateInstance(WolfIds.BattleGauge);
        vm.CreateInstance(WolfIds.BattleCommandPhase);
        vm.Vars.Set(WolfIds.VarBattleActive, 0);
        wiring.Tick(vm, _ => null, allowAutoCommands: false);
        Check(!Array.Exists(vm.Instances.ToArray(), i => i.EventId is WolfIds.BattleGauge or WolfIds.BattleTurnStart or WolfIds.BattleCommandPhase),
            "battle exit must stop lingering CE240/241/242 instances");
    }

    private static void TestSecondBattleReharvestAndReset()
    {
        var vm = new EventVm();
        var store = GameDatabase.Load(Path.Combine(DataDirectory(), "database.json"))!;
        var wiring = new BattleWiring();
        Set(store, 18, 73, 0, 1);
        wiring.Tick(vm, op => store.Execute(op)); // baseline _menuCursor=1

        // Battle 1 registers row 1 -> cursor advances to 2
        Set(store, BattleWiring.CmdListType, 1, 0, 1);
        Set(store, BattleWiring.CmdListType, 1, 3, 26);
        Set(store, 18, 73, 0, 2);
        int added1 = wiring.HarvestMenuRegistrations(vm, op => store.Execute(op));
        Check(added1 == 1 && wiring.MenuSelections.Count == 1, "battle 1 should harvest row 1");

        // Battle 2 resets cursor back down to 1, then commits row 1 with skill 99
        Set(store, 18, 73, 0, 1);
        wiring.Tick(vm, op => store.Execute(op));
        Set(store, BattleWiring.CmdListType, 1, 0, 1);
        Set(store, BattleWiring.CmdListType, 1, 3, 99);
        Set(store, 18, 73, 0, 2);
        int added2 = wiring.HarvestMenuRegistrations(vm, op => store.Execute(op));
        Check(added2 == 1 && wiring.MenuSelections[^1].Skill == 99, "battle 2 should re-harvest row 1 after cursor reset");

        wiring.Reset();
        Check(wiring.MenuSelections.Count == 0 && wiring.MenuQueueLength == 0 && wiring.StartedRounds == 0 && !wiring.SawDispatch,
            "BattleWiring.Reset() must clear all battle state");
        wiring.SetRoundLoop(true);
        Check(wiring.ManualRoundLoop == true, "SetRoundLoop(true) must update ManualRoundLoop");
        wiring.SetRoundLoop(false);
        Check(wiring.ManualRoundLoop == false, "SetRoundLoop(false) must update ManualRoundLoop");
    }

    private static Trace RunLine()
    {
        var trace = new Trace();
        var vm = new VmController();
        try
        {
            Check(vm.LoadPrograms(Path.Combine(DataDirectory(), "programs.json")), "programs failed to load");
            var store = vm.DatabaseStore;
            Check(store != null, "database store missing");
            vm.Vm.Host = vm;
            vm.Vm.StrictMode = false;
            Seed(store!);

            var autoEvents = AutoEvents.FromPrograms(vm.Vm.Programs.Values);
            var wiring = new BattleWiring();
            wiring.SetEnabled(true);
            wiring.SetRoundLoop(false);

            int frame = 0;
            vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
            Check(vm.StartEvent(239), "CE239 failed to start");

            for (frame = 0; frame < 20000; frame++)
            {
                autoEvents.Sync(vm.Vm);
                wiring.Tick(vm.Vm, op => vm.Database(op));
                vm.StepFrame();
                if (vm.IsWaitingForMessage()) vm.DismissMessage();

                foreach (var inst in vm.Vm.Instances)
                {
                    if (inst.EventId == 239)
                    {
                        trace.Ce239 = true;
                        trace.MaxCe239Pc = Math.Max(trace.MaxCe239Pc, inst.Pc);
                    }
                    else if (inst.EventId == 243) trace.Ce243 = true;
                    else if (inst.EventId == 241) trace.Ce241 = true;
                    else if (inst.EventId == 242)
                    {
                        trace.Ce242 = true;
                        trace.MaxCe242Pc = Math.Max(trace.MaxCe242Pc, inst.Pc);
                    }
                }
                if (vm.Vm.Vars.Get(340) != 0) trace.BattleActive = true;
                if (vm.Vm.Vars.Get(350) == 1) trace.PhaseOne = true;
                if (vm.Vm.Vars.Get(343) != 0) trace.Decided = true;
                int result = vm.Vm.Vars.Get(352);
                if (Math.Abs(result) > Math.Abs(trace.Result)) trace.Result = result;
                trace.Rounds = wiring.StartedRounds;
                bool alive239 = Array.Exists(vm.Vm.Instances.ToArray(), i => i.EventId == 239);
                if (trace.Decided && !alive239) break;
            }

            trace.Faulted = vm.Vm.Faulted;
            trace.FaultReason = vm.Vm.FaultReason;
            trace.Ce239AliveAtEnd = Array.Exists(vm.Vm.Instances.ToArray(), i => i.EventId == 239);
            foreach (var inst in vm.Vm.Instances)
            {
                if (inst.EventId == 239) trace.End239Pc = inst.Pc;
            }
            trace.End340 = vm.Vm.Vars.Get(340);
            trace.End350 = vm.Vm.Vars.Get(350);
            trace.End343 = vm.Vm.Vars.Get(343);
            trace.Rounds = wiring.StartedRounds;
        }
        catch (Exception e)
        {
            trace.Faulted = true;
            trace.FaultReason = e.Message;
        }
        finally
        {
            vm.Free();
        }
        return trace;
    }

    private static void Seed(GameDatabase store)
    {
        Set(store, 18, 4, 0, 1);   // party count
        Set(store, 18, 69, 0, 1);  // enemy count
        int[] slots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < slots.Length; i++) Set(store, 0, 12, 47 + i, slots[i]);
        int[] stats = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < stats.Length; i++) Set(store, 0, 12, columns[i], stats[i]);
        Set(store, 7, 0, 0, 1);
        Set(store, 8, 0, 0, 1);
        Set(store, 8, 1, 0, 1);
        Set(store, 8, 2, 0, 1);
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

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"BATTLE_WIRING_TEST_FAILED: {message}");
    }
}
