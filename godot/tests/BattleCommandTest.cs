using System;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of crates/wolf-game/tests/m6_battle_commands.rs: CE188 builds the attack command
/// list through CE152. With no launch items the four real attack rows are disabled; with
/// one item they become selectable. CE188 must reach CE192 and never spawn CE244.
/// </summary>
public static class BattleCommandTest
{
    private static readonly int[] RealAttackCodes = [26, 25, 30, 29];

    private sealed class Outcome
    {
        public bool Reached152;
        public bool Reached192;
        public bool Reached244;
        public bool RealSelectable;
        public bool Faulted;
        public string? FaultReason;
    }

    public static void Run()
    {
        var control = RunLine(inventory: false);
        var seeded = RunLine(inventory: true);

        Check(!control.Faulted, "control faulted: " + control.FaultReason);
        Check(!seeded.Faulted, "seeded faulted: " + seeded.FaultReason);
        Check(control.Reached192, "control must reach CE192");
        Check(seeded.Reached152, "seeded must reach CE152");
        Check(seeded.Reached192, "seeded must reach CE192");
        Check(!control.Reached244, "control must not spawn CE244");
        Check(!seeded.Reached244, "seeded must not spawn CE244");

        Check(!control.RealSelectable, "control exposes a selectable real attack row");
        Check(seeded.RealSelectable, "seeded has no selectable real attack row at any frame");

        GD.Print($"BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1");
    }

    private static Outcome RunLine(bool inventory)
    {
        var outcome = new Outcome();
        var vm = new VmController();
        try
        {
            var dataDir = DataDirectory();
            Check(vm.LoadPrograms(Path.Combine(dataDir, "programs.json")), "programs failed to load");
            var store = vm.DatabaseStore;
            Check(store != null, "database store missing");
            vm.Vm.Host = vm;
            vm.Vm.StrictMode = false;

            int frame = 0;
            vm.KeyCodeProvider = _ => frame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
            Check(vm.StartEvent(188), "CE188 failed to start");

            for (frame = 0; frame < 200; frame++)
            {
                if (frame < 60) Seed(store!, inventory);
                vm.StepFrame();
                foreach (var inst in vm.Vm.Instances)
                {
                    if (inst.EventId == 152) outcome.Reached152 = true;
                    else if (inst.EventId == 192) outcome.Reached192 = true;
                    else if (inst.EventId == 244) outcome.Reached244 = true;
                }
                if (HasRealSelectable(store!)) outcome.RealSelectable = true;
            }

            outcome.Faulted = vm.Vm.Faulted;
            outcome.FaultReason = vm.Vm.FaultReason;
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

    private static bool HasRealSelectable(GameDatabase store)
    {
        for (int row = 0; row < 8; row++)
        {
            if (Array.IndexOf(RealAttackCodes, ReadNum(store, 19, row, 1)) >= 0 && ReadNum(store, 19, row, 2) == 1) return true;
        }
        return false;
    }

    private static void Seed(GameDatabase store, bool inventory)
    {
        Set(store, 18, 4, 0, 1); // party count
        Set(store, 18, 69, 0, 1); // enemy count
        int[] slots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < slots.Length; i++) Set(store, 0, 12, 47 + i, slots[i]);
        int[] stats = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        Set(store, 0, 12, 20, stats[0]);
        Set(store, 0, 12, 21, stats[1]);
        Set(store, 0, 12, 22, stats[2]);
        Set(store, 0, 12, 23, stats[3]);
        Set(store, 0, 12, 24, stats[4]);
        Set(store, 0, 12, 25, stats[5]);
        Set(store, 0, 12, 26, stats[6]);
        Set(store, 0, 12, 27, stats[7]);
        Set(store, 0, 12, 28, stats[8]);
        Set(store, 0, 12, 9, stats[9]);
        Set(store, 0, 12, 10, stats[10]);
        Set(store, 0, 12, 4, stats[11]);
        if (inventory) Set(store, 7, 0, 0, 1);
    }

    private static void Set(GameDatabase store, int type, int row, int column, int value)
    {
        var op = new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value)
        };
        store.Execute(op);
    }

    private static int ReadNum(GameDatabase store, int type, int row, int column)
    {
        var op = new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Read,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Destination = 0
        };
        var value = store.Read(op);
        return value == null || value.IsString ? 0 : value.Num;
    }

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"BATTLE_COMMAND_TEST_FAILED: {message}");
    }
}
