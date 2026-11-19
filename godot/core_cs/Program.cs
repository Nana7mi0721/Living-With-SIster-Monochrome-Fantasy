using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

public enum BlockKind
{
    Condition,
    Choices,
    Loop
}

public sealed class Block
{
    public int Head { get; init; }
    public BlockKind Kind { get; init; }
    public List<(int CaseId, int Pc)> Cases { get; } = new();
    public int? ElsePc { get; set; }
    public int? CancelPc { get; set; }
    public int EndPc { get; set; } = int.MaxValue;

    public int EndTarget(int len)
    {
        return EndPc == int.MaxValue ? len : EndPc + 1;
    }

    public int? CaseTarget(int caseId)
    {
        for (int i = 0; i < Cases.Count; i++)
        {
            if (Cases[i].CaseId == caseId)
                return Cases[i].Pc;
        }
        return null;
    }
}

public enum ControlKind
{
    None,
    Block,
    Case,
    LoopEnd,
    LabelJump
}

public readonly struct Control
{
    public ControlKind Kind { get; init; }
    public int BlockIndex { get; init; }

    public static Control None => new() { Kind = ControlKind.None };
    public static Control Block(int index) => new() { Kind = ControlKind.Block, BlockIndex = index };
    public static Control Case(int index) => new() { Kind = ControlKind.Case, BlockIndex = index };
    public static Control LoopEnd(int index) => new() { Kind = ControlKind.LoopEnd, BlockIndex = index };
    public static Control LabelJump => new() { Kind = ControlKind.LabelJump };
}

public sealed class Program
{
    public const uint NoReturnValue = 0xFFFFFFFF;
    public const int CommonSelfBase = 600;
    public const int MapSelfBase = 100;

    public int Id { get; init; }
    public string Title { get; init; } = "";
    public uint ReturnValueId { get; set; } = NoReturnValue;
    public int SelfBase { get; set; } = CommonSelfBase;
    /// <summary>Header run condition: 0 called only, 1 auto start, 2 parallel condition, 3 always.</summary>
    public int RunCondition { get; set; }
    public int ConditionOperator { get; set; }
    public int ConditionVariable { get; set; }
    public int ConditionValue { get; set; }
    public List<Slot> Slots { get; init; } = new();
    public Control[] ControlMap { get; private set; } = Array.Empty<Control>();
    public List<Block> Blocks { get; } = new();
    public Dictionary<string, int> Labels { get; } = new();

    public int Len => Slots.Count;

    public static Program FromSlots(int id, string title, List<Slot> slots)
    {
        int len = slots.Count;
        var control = new Control[len];
        var blocks = new List<Block>();
        var open = new List<int>();

        for (int pc = 0; pc < len; pc++)
        {
            var slot = slots[pc];
            int opcode = slot.Opcode;
            if (opcode == 0) continue;

            switch (opcode)
            {
                case 111:
                case 112:
                case 102:
                case 170:
                case 179:
                {
                    var kind = opcode switch
                    {
                        102 => BlockKind.Choices,
                        111 or 112 => BlockKind.Condition,
                        _ => BlockKind.Loop
                    };
                    var block = new Block { Head = pc, Kind = kind };
                    blocks.Add(block);
                    int index = blocks.Count - 1;
                    open.Add(index);
                    control[pc] = Control.Block(index);
                    break;
                }
                case 401:
                {
                    int? openIdx = FindOpen(blocks, open, k => k != BlockKind.Loop);
                    if (openIdx.HasValue)
                    {
                        int idArg = slot.Params.Length > 0 ? slot.Params[0] : 0;
                        blocks[openIdx.Value].Cases.Add((idArg, pc + 1));
                        control[pc] = Control.Case(openIdx.Value);
                    }
                    break;
                }
                case 420:
                {
                    int? openIdx = FindOpen(blocks, open, k => k == BlockKind.Condition);
                    if (openIdx.HasValue)
                    {
                        blocks[openIdx.Value].ElsePc = pc + 1;
                        control[pc] = Control.Case(openIdx.Value);
                    }
                    break;
                }
                case 421:
                {
                    int? openIdx = FindOpen(blocks, open, k => k == BlockKind.Choices);
                    if (openIdx.HasValue)
                    {
                        blocks[openIdx.Value].CancelPc = pc + 1;
                        control[pc] = Control.Case(openIdx.Value);
                    }
                    break;
                }
                case 499:
                {
                    int? openIdx = FindOpen(blocks, open, k => k != BlockKind.Loop);
                    if (openIdx.HasValue)
                    {
                        CloseFrom(blocks, open, openIdx.Value, pc);
                        control[pc] = Control.Case(openIdx.Value);
                    }
                    break;
                }
                case 498:
                {
                    int? openIdx = FindOpen(blocks, open, k => k == BlockKind.Loop);
                    if (openIdx.HasValue)
                    {
                        CloseFrom(blocks, open, openIdx.Value, pc);
                        blocks[openIdx.Value].EndPc = pc;
                        control[pc] = Control.LoopEnd(openIdx.Value);
                    }
                    break;
                }
                case 213:
                {
                    control[pc] = Control.LabelJump;
                    break;
                }
            }
        }

        var prog = new Program
        {
            Id = id,
            Title = title,
            Slots = slots,
            ControlMap = control,
        };
        prog.Blocks.AddRange(blocks);

        // Second pass: 212 labels
        for (int pc = 0; pc < len; pc++)
        {
            var slot = slots[pc];
            if (slot.Opcode == 212)
            {
                string name = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                prog.Labels[name] = pc + 1;
            }
        }

        return prog;
    }

    private static int? FindOpen(List<Block> blocks, List<int> open, Func<BlockKind, bool> pred)
    {
        for (int i = open.Count - 1; i >= 0; i--)
        {
            int idx = open[i];
            if (pred(blocks[idx].Kind))
                return idx;
        }
        return null;
    }

    private static void CloseFrom(List<Block> blocks, List<int> open, int index, int pc)
    {
        while (open.Count > 0)
        {
            int top = open[^1];
            open.RemoveAt(open.Count - 1);
            if (blocks[top].EndPc == int.MaxValue)
            {
                blocks[top].EndPc = pc;
            }
            if (top == index)
                break;
        }
    }
}
