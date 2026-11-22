using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

public enum WaitStateKind
{
    None,
    Frames,
    Message,
    Choice,
    Child,
    KeyInput
}

public sealed class WaitState
{
    public WaitStateKind Kind { get; set; } = WaitStateKind.None;
    public uint Frames { get; set; }
    public int Block { get; set; }
    public int ChildId { get; set; }
    public int KeyTarget { get; set; }
    public uint KeyMask { get; set; }
    public bool KeyRequireRelease { get; set; }

    public void Clear()
    {
        Kind = WaitStateKind.None;
        Frames = 0;
        Block = 0;
        ChildId = 0;
        KeyTarget = 0;
        KeyMask = 0;
        KeyRequireRelease = false;
    }

    public void WaitForFrames(uint frames)
    {
        Kind = WaitStateKind.Frames;
        Frames = frames;
    }

    public void WaitForChild(int childId)
    {
        Kind = WaitStateKind.Child;
        ChildId = childId;
    }

    public void WaitForChoice(int block)
    {
        Kind = WaitStateKind.Choice;
        Block = block;
    }

    public void WaitForMessage()
    {
        Kind = WaitStateKind.Message;
    }

    public void WaitForKeyInput(int target, uint mask, bool requireRelease)
    {
        Kind = WaitStateKind.KeyInput;
        KeyTarget = target;
        KeyMask = mask;
        KeyRequireRelease = requireRelease;
    }
}

public sealed class LoopCtx
{
    public int Block { get; init; }
    public long Remaining { get; set; }
}

public sealed class Instance
{
    public int Id { get; init; }
    public int? ParentId { get; init; }
    public int EventId { get; init; }
    public Program Program { get; set; } = null!;
    public int Pc { get; set; }
    public SelfVars Self { get; } = new();
    public WaitState Wait { get; } = new();
    public List<LoopCtx> Loops { get; } = new();
    public int? ReturnTo { get; set; }

    /// <summary>Map page instances outlive their program until MapRuntime copies the self bank.</summary>
    public bool RetainAfterFinish { get; set; }

    /// <summary>Parallel scheduler instance (exec.rs Instance::is_parallel): never blocks the 211 foreground check.</summary>
    public bool IsParallel { get; set; }

    /// <summary>Set by ReapFinished for the rest of the frame so MapRuntime does not kill the caller before its next step.</summary>
    public bool JustResumedFromChild { get; set; }

    public bool IsRunnable => Wait.Kind == WaitStateKind.None;
}
