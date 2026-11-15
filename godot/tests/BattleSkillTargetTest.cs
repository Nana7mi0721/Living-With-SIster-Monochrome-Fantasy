using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// CE159 <c>X[戦]技能選択実行</c> tail: the skill row must advertise a usable scene
/// (Data 0/row/6 = 1 or 2) and an effect target column (Data 0/row/2 in 10..12).
/// When both hold, pc115 copies the target into self1600029, pc244 admits it and
/// pc247 starts CE162 <c>X[戦]敵対象選択実行</c>, which selects enemy index 0 at pc57.
/// A zero target skips CE162; a zero enemy count leaves CE162 spinning at pc49/pc50.
/// </summary>
public static class BattleSkillTargetTest
{
    private sealed class Line
    {
        public bool Faulted;
        public string? FaultReason;
        public bool SawPc110;
        public bool SawPc115;
        public bool SawPc247;
        public bool Saw162Pc12;
        public bool Saw162Pc57;
        public bool Ce159Ended;
        public bool Ce162Alive;
        public int End162Pc;
        public readonly SortedSet<int> Pcs159 = new();
        public readonly SortedSet<int> Pcs162 = new();
    }

    public static void Run()
    {
        var success = RunCase(10, 1);
        Check(!success.Faulted, "seeded faulted: " + success.FaultReason);
        Check(success.SawPc110, "CE159 must read the skill scene at pc110, saw " + string.Join(",", success.Pcs159));
        Check(success.SawPc115, "CE159 must copy the effect target at pc115, saw " + string.Join(",", success.Pcs159));
        Check(success.SawPc247, "CE159 must call CE162 at pc247, saw " + string.Join(",", success.Pcs159));
        Check(success.Saw162Pc12, "CE162 must read the enemy count at pc12, saw " + string.Join(",", success.Pcs162));
        Check(success.Saw162Pc57, "CE162 must reach pc57 after selecting enemy 0, saw " + string.Join(",", success.Pcs162));
        Check(!success.Ce162Alive, "CE162 must exit after a successful selection");
        Check(success.Ce159Ended, "CE159 must finish after the target call returns");

        var noTarget = RunCase(0, 1);
        Check(!noTarget.Faulted, "no-target faulted: " + noTarget.FaultReason);
        Check(!noTarget.Saw162Pc12, "CE162 must not start when the effect target is 0");
        Check(noTarget.Ce159Ended, "CE159 must finish when the effect target is 0");

        var noEnemy = RunCase(10, 0);
        Check(!noEnemy.Faulted, "no-enemy faulted: " + noEnemy.FaultReason);
        Check(noEnemy.Saw162Pc12, "CE162 must start with a valid target even without enemies");
        Check(!noEnemy.Saw162Pc57, "CE162 must not select an enemy when the enemy count is 0");
        Check(noEnemy.Ce162Alive, "CE162 must stay active without an enemy count");
        Check(noEnemy.End162Pc == 49 || noEnemy.End162Pc == 50, "CE162 must spin at pc49/pc50, got pc" + noEnemy.End162Pc);

        GD.Print($"BATTLE_SKILL_TARGET_TEST_OK selected=1 no_target=1 no_enemy_pc={noEnemy.End162Pc}");
    }

    private static Line RunCase(int target, int enemyCount)
    {
        var line = new Line();
        var vm = new VmController();
        try
        {
            Prepare(vm);
            Seed(vm.DatabaseStore!);
            SetCData(vm.DatabaseStore!, 18, 69, 0, enemyCount);
            SetData(vm.DatabaseStore!, 0, 2, 2, target);
            SetData(vm.DatabaseStore!, 0, 2, 6, 1);
            var inst = vm.Vm.CreateInstance(159);
            Check(inst != null, "CE159 failed to start");
            inst!.Self.Set(600, 1);
            inst.Self.Set(601, 2);
            inst.Self.Set(602, 0);
            inst.Self.Set(622, 2);
            inst.Pc = 94;
            vm.Vm.StepHook = (seen, pc) =>
            {
                if (seen.EventId == 159)
                {
                    line.Pcs159.Add(pc);
                    if (pc == 110) line.SawPc110 = true;
                    if (pc == 115) line.SawPc115 = true;
                    if (pc == 247) line.SawPc247 = true;
                }
                else if (seen.EventId == 162)
                {
                    line.Pcs162.Add(pc);
                    if (pc == 12) line.Saw162Pc12 = true;
                    if (pc == 57) line.Saw162Pc57 = true;
                }
            };
            int frame = 0;
            for (int i = 0; i < 300; i++)
            {
                frame = i;
                vm.KeyCodeProvider = _ => frame % 2 == 1 ? KeyCodes.CodeOk : KeyCodes.CodeNone;
                vm.StepFrame();
                if (vm.IsWaitingForMessage()) vm.DismissMessage();
                foreach (var live in vm.Vm.Instances)
                {
                    if (live.EventId == 162) line.End162Pc = live.Pc;
                }
            }
            line.Ce159Ended = !vm.Vm.Instances.Any(i => i.EventId == 159);
            line.Ce162Alive = vm.Vm.Instances.Any(i => i.EventId == 162);
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

    private static void Seed(GameDatabase store)
    {
        SetCData(store, 18, 4, 0, 1);
        int[] slots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < slots.Length; i++) SetCData(store, 0, 12, 47 + i, slots[i]);
        int[] stats = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        int[] columns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < stats.Length; i++) SetCData(store, 0, 12, columns[i], stats[i]);
        SetCData(store, 7, 0, 0, 1);
        SetCData(store, 8, 0, 0, 1);
        SetCData(store, 8, 1, 0, 1);
        SetCData(store, 8, 2, 0, 1);
    }

    private static void SetCData(GameDatabase store, int type, int row, int column, int value)
    {
        store.Execute(new DbOp
        {
            Container = DbContainer.CData,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value),
        });
    }

    private static void SetData(GameDatabase store, int type, int row, int column, int value)
    {
        store.Execute(new DbOp
        {
            Container = DbContainer.Data,
            Direction = DbDirection.Write,
            Op = 0,
            DbType = type,
            DataId = row,
            ItemId = column,
            Source = new DbValue(value),
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
        if (!condition) throw new Exception("BATTLE_SKILL_TARGET_TEST_FAILED: " + message);
    }
}