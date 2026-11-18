using System;
using System.Collections.Generic;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// C# half of the M13 random-event matrix. Mirrors
/// crates/wolf-game/tests/m13_random_events.rs: all call targets resolve,
/// umbrella-pool dispatch samples, named block pc jumps, gate triples and
/// the full pool sweep (the Rust file's Group E).
/// </summary>
public static class M13RandomEventsTest
{
    private static readonly (int EventId, int Pc)[] DeadAllow =
    {
        (270, 631), (270, 637), (270, 705), (270, 711),
        (270, 2065), (270, 2071), (270, 2112), (270, 2119),
    };

    public static void Run()
    {
        var vm = M13Harness.NewVm();
        var (checkedCount, dynamicCount, allowedCount, dead) = ScanCallTargets(vm);
        M13Harness.Check(dead.Count == 0, "dead call targets:\n" + string.Join("\n", dead));

        int cases = 0;
        int gates = 0;

        // Group B — umbrella pools dispatch into the expected block.
        var poolCases = new (int Ce, int SelFn, int SelVal, int CondPc, string Expected, string Label)[]
        {
            (277, 0, 8, 1, "摔了一跤受伤了", "CE277 #66"),
            (277, 0, 6, 1, "肚子饿了", "CE277 #99"),
            (277, 0, 11, 1, "感觉没啥干劲", "CE277 #71"),
            (277, 0, 14, 1, "小指撞到桌角了", "CE277 #108"),
            (278, 10, 1, 11, "竟然是熊头", "CE278 #15"),
            (279, 0, 3, 1, "亲亲我我的时间", "CE279 #5/#33"),
            (279, 0, 4, 1, "留的作业很难", "CE279 #94"),
            (280, 0, 4, 0, "吃东西得再买一点", "CE280 #110"),
        };
        foreach (var c in poolCases)
        {
            var run = M13Harness.DriveAt(c.Ce, c.CondPc, selfs: new[] { (600 + c.SelFn, c.SelVal) }, maxFrames: 2000);
            M13Harness.Reached(c.Label, run, c.Expected);
            cases++;
        }

        // Group C — named day/dinner/DLC/seal blocks reachable by pc jump.
        var blockCases = new (int Ce, int Pc, string Expected, string Label)[]
        {
            (301, 1145, "留下来继续训练了", "CE301 #43"),
            (326, 1145, "感觉食欲爆发", "CE326 #79"),
            (326, 1323, "好好吃！！！", "CE326 #14"),
            (326, 1378, "要珍惜粮食", "CE326 #97"),
            (326, 1412, "超级像鸡鸡", "CE326 #98"),
            (449, 45, "源头很可能是", "CE449 #69"),
            (461, 1355, "看啊，那些家伙", "CE461 #58/#76"),
            (465, 104, "由衣，你没事吧？", "CE465 #28"),
            (437, 0, "海豹驱除作战", "CE437"),
            (438, 5, "海豹跃出水面", "CE438"),
            (317, 163, "比平时还有精神", "CE317 #52"),
            (317, 330, "干劲下降了", "CE317 #51"),
            (250, 26, "午休有妹妹送慰问品", "CE250 #37"),
            (248, 257, "泡杯茶吧", "CE248 #57/#70"),
            (312, 53, "得到点心了", "CE312 #102"),
        };
        foreach (var c in blockCases)
        {
            var run = M13Harness.DriveAt(c.Ce, c.Pc, maxFrames: 800);
            M13Harness.Reached(c.Label, run, c.Expected);
            cases++;
        }

        // Group D — cross-check gate triples from M13_CONTENT_MATRIX.
        M13Harness.GateAt(vm, 326, 36, (2_000_021, 5, 3));
        M13Harness.GateAt(vm, 326, 38, (2_000_023, 1, 3));
        M13Harness.GateAt(vm, 310, 159, (2_000_016, 1, 1));
        M13Harness.GateAt(vm, 310, 161, (2_000_022, 200, 1));
        M13Harness.GateAt(vm, 310, 163, (2_000_018, 40, 1));
        M13Harness.GateAt(vm, 310, 165, (2_000_424, 3, 1));
        M13Harness.GateAt(vm, 318, 338, (2_000_103, 2, 1), (2_000_102, 75, 1));
        M13Harness.GateAt(vm, 318, 415, (2_000_103, 2, 1), (2_000_102, 75, 1));
        gates = 8;

        // Group E — full umbrella-pool sweep (Rust #[ignore] group).
        int sweep = 0;
        for (int value = 1; value <= 15; value++)
        {
            var run = M13Harness.DriveAt(277, 1, selfs: new[] { (600, value) }, maxFrames: 2000);
            M13Harness.Check(run.Unhandled == 0, $"CE277 sel={value}: unhandled={run.Unhandled}");
            M13Harness.Check(run.Ended, $"CE277 sel={value}: did not end");
            sweep++;
        }
        for (int value = 1; value <= 10; value++)
        {
            var run = M13Harness.DriveAt(279, 1, selfs: new[] { (600, value) }, maxFrames: 2000);
            M13Harness.Check(run.Unhandled == 0, $"CE279 sel={value}: unhandled={run.Unhandled}");
            sweep++;
        }
        for (int value = 1; value <= 4; value++)
        {
            var run = M13Harness.DriveAt(280, 0, selfs: new[] { (600, value) }, maxFrames: 2000);
            M13Harness.Check(run.Unhandled == 0, $"CE280 sel={value}: unhandled={run.Unhandled}");
            sweep++;
        }
        for (int value = 1; value <= 9; value++)
        {
            var run = M13Harness.DriveAt(278, 11, selfs: new[] { (610, value) }, maxFrames: 2000);
            M13Harness.Check(run.Unhandled == 0, $"CE278 sel={value}: unhandled={run.Unhandled}");
            sweep++;
        }

        GD.Print($"M13_RANDOM_EVENTS_TEST_OK cases={cases} gates={gates} sweep={sweep} " +
                 $"checked={checkedCount} dynamic={dynamicCount} allowed={allowedCount}");
    }

    private static (int Checked, int Dynamic, int Allowed, List<string> Dead) ScanCallTargets(VmController vm)
    {
        int checkedCount = 0;
        int dynamicCount = 0;
        int allowedCount = 0;
        var dead = new List<string>();
        var allow = new HashSet<(int, int)>(DeadAllow);

        foreach (var id in vm.Vm.Programs.Keys.OrderBy(k => k))
        {
            if (id < 0 || id >= 2000) continue;
            var program = vm.Vm.Programs[id];
            for (int pc = 0; pc < program.Slots.Count; pc++)
            {
                var slot = program.Slots[pc];
                if (slot.Opcode == 210)
                {
                    int raw = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    if (Magic.Decode(raw).IsStorage) { dynamicCount++; continue; }
                    int? target = raw >= 500_000 && raw <= 599_999 ? raw - 500_000
                        : raw >= 600_000 && raw <= 699_999 ? raw - 600_000
                        : null;
                    if (target == null)
                    {
                        dead.Add($"CE{id}@{pc} op210 literal target {raw} unsupported");
                        continue;
                    }
                    checkedCount++;
                    if (vm.Vm.GetProgram(target.Value) == null)
                        dead.Add($"CE{id}@{pc} op210 -> event {target.Value} MISSING");
                }
                else if (slot.Opcode == 211)
                {
                    int raw = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    if (Magic.Decode(raw).IsStorage) { dynamicCount++; continue; }
                    int target = raw >= 500_000 && raw <= 599_999 ? raw - 500_000
                        : raw >= 600_000 && raw <= 699_999 ? raw - 600_000
                        : Math.Max(0, raw);
                    checkedCount++;
                    if (target != 0 && vm.Vm.GetProgram(target) == null)
                        dead.Add($"CE{id}@{pc} op211 -> event {target} MISSING");
                }
                else if (slot.Opcode == 300)
                {
                    string name = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    if (name.Length == 0 || name.Contains('\\') || name.Contains("cself") || name.Contains('{'))
                    {
                        dynamicCount++;
                        continue;
                    }
                    checkedCount++;
                    if (vm.Vm.GetProgramByTitle(name) == null)
                    {
                        if (allow.Contains((id, pc))) allowedCount++;
                        else dead.Add($"CE{id}@{pc} op300 name '{name}' UNRESOLVED");
                    }
                }
            }
        }
        return (checkedCount, dynamicCount, allowedCount, dead);
    }
}
