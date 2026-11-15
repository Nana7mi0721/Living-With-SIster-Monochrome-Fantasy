using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Port of crates/wolf-game/tests/m7_battle_target.rs: CE190 lays out the enemy from
/// CData 18/69, then CE162 selects enemy index 0 and exits at pc57. Clearing only the
/// enemy count reproduces the pc49/pc50 spin.
/// </summary>
public static class BattleTargetTest
{
    private sealed class Line
    {
        public int EnemyCount;
        public int EnemyHp;
        public int EnemyX;
        public int EnemyY;
        public int EnemyDrawX;
        public int EnemyDrawY;
        public bool SawPc12;
        public bool SawPc57;
        public bool Ce162Alive;
        public int EndPc;
        public bool Faulted;
        public string? FaultReason;
        public readonly SortedSet<int> Pcs = new();
    }

    public static void Run()
    {
        var seeded = RunSeeded();
        Check(!seeded.Faulted, "seeded faulted: " + seeded.FaultReason);
        Check(seeded.EnemyCount == 1, "seeded enemy count must stay 1, got " + seeded.EnemyCount);
        Check(seeded.EnemyHp > 0, "CE144 did not copy a live enemy HP row, got " + seeded.EnemyHp);
        Check(seeded.EnemyX == 512, "CE175 enemy x must be 512, got " + seeded.EnemyX);
        Check(seeded.EnemyY == -288, "CE175 enemy y must be -288, got " + seeded.EnemyY);
        Check(seeded.EnemyDrawX == 512, "CE175 enemy draw x scale must be 512, got " + seeded.EnemyDrawX);
        Check(seeded.EnemyDrawY == 312, "CE175 enemy draw y scale must be 312, got " + seeded.EnemyDrawY);
        Check(seeded.SawPc12, "CE162 must read the enemy count at pc12, saw " + string.Join(",", seeded.Pcs));
        Check(seeded.SawPc57, "CE162 must reach pc57 (enemy index 0 selected), saw " + string.Join(",", seeded.Pcs));
        Check(!seeded.Ce162Alive, "CE162 must exit after a successful selection");

        var control = RunControl();
        Check(!control.Faulted, "control faulted: " + control.FaultReason);
        Check(control.EnemyCount == 0, "control enemy count must stay 0, got " + control.EnemyCount);
        Check(!control.SawPc57, "control CE162 must not reach pc57 without an enemy count");
        Check(control.Ce162Alive, "control CE162 must remain active");
        Check(control.EndPc == 49 || control.EndPc == 50, "control CE162 must spin at pc49/pc50, got pc" + control.EndPc);

        GD.Print($"BATTLE_TARGET_TEST_OK selected=1 control_pc={control.EndPc} enemy_hp={seeded.EnemyHp}");
    }

    private static Line RunSeeded()
    {
        var line = new Line();
        var vm = new VmController();
        try
        {
            Prepare(vm);
            Seed(vm.DatabaseStore!);
            vm.StartEvent(190);
            Drive(vm, 600, null);
            line.EnemyCount = ReadNum(vm.DatabaseStore!, 18, 69, 0);
            line.EnemyHp = ReadNum(vm.DatabaseStore!, 10, 0, 2);
            line.EnemyX = ReadNum(vm.DatabaseStore!, 18, 35, 0);
            line.EnemyY = ReadNum(vm.DatabaseStore!, 18, 36, 0);
            line.EnemyDrawX = ReadNum(vm.DatabaseStore!, 18, 37, 0);
            line.EnemyDrawY = ReadNum(vm.DatabaseStore!, 18, 38, 0);
            Check(vm.StartEvent(162), "CE162 failed to start");
            Drive(vm, 300, line);
            line.Ce162Alive = Array.Exists(vm.Vm.Instances.ToArray(), i => i.EventId == 162);
            foreach (var inst in vm.Vm.Instances)
            {
                if (inst.EventId == 162) line.EndPc = inst.Pc;
            }
            line.Faulted = vm.Vm.Faulted;
            line.FaultReason = vm.Vm.FaultReason;
        }
        catch (Exception e)
        {
            line.Faulted = true;
            line.FaultReason = e.Message;
        }
        finally
        {
            vm.Free();
        }
        return line;
    }

    private static Line RunControl()
    {
        var line = new Line();
        var vm = new VmController();
        try
        {
            Prepare(vm);
            Seed(vm.DatabaseStore!);
            Set(vm.DatabaseStore!, 18, 69, 0, 0);
            vm.StartEvent(190);
            Drive(vm, 400, null);
            line.EnemyCount = ReadNum(vm.DatabaseStore!, 18, 69, 0);
            Check(vm.StartEvent(162), "control CE162 failed to start");
            Drive(vm, 240, line);
            line.Ce162Alive = Array.Exists(vm.Vm.Instances.ToArray(), i => i.EventId == 162);
            foreach (var inst in vm.Vm.Instances)
            {
                if (inst.EventId == 162) line.EndPc = inst.Pc;
            }
            line.Faulted = vm.Vm.Faulted;
            line.FaultReason = vm.Vm.FaultReason;
        }
        catch (Exception e)
        {
            line.Faulted = true;
            line.FaultReason = e.Message;
        }
        finally
        {
            vm.Free();
        }
        return line;
    }

    private static void Prepare(VmController vm)
    {
        Check(vm.LoadPrograms(Path.Combine(DataDirectory(), "programs.json")), "programs failed to load");
        Check(vm.DatabaseStore != null, "database store missing");
        vm.Vm.Host = vm;
        vm.Vm.StrictMode = false;
    }

    private static void Drive(VmController vm, int frames, Line? line)
    {
        int currentFrame = 0;
        vm.KeyCodeProvider = _ => currentFrame % 2 == 0 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
        for (int i = 0; i < frames; i++)
        {
            currentFrame = i;
            vm.StepFrame();
            if (vm.IsWaitingForMessage()) vm.DismissMessage();
            if (line == null) continue;
            foreach (var inst in vm.Vm.Instances)
            {
                if (inst.EventId != 162) continue;
                line.Pcs.Add(inst.Pc);
                if (inst.Pc == 12) line.SawPc12 = true;
                if (inst.Pc == 57) line.SawPc57 = true;
            }
        }
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

    private static string DataDirectory()
    {
        var res = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        var root = Path.GetDirectoryName(res) ?? res;
        return Path.Combine(root, "game_data");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("BATTLE_TARGET_TEST_FAILED: " + message);
    }
}
