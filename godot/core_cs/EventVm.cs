using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

public enum Signal
{
    None,
    Continue,
    Blocked,
    Finished,
    Call,
    ReturnToTitle,
    EndGame
}

public sealed class EventVm
{
    public Variables Vars { get; } = new();
    public WolfRng Rng { get; } = new();
    public List<Instance> Instances { get; } = new();
    public Dictionary<int, Program> Programs { get; } = new();
    public ulong Frame { get; private set; }
    public IHost Host { get; set; } = new NullHost();
    /// <summary>Persistent CSelf bank per common event (exec.rs common_self).</summary>
    public Dictionary<int, SelfVars> CommonSelf { get; } = new();
    public const int MaxInstances = 4096;
    public int UnhandledOpcodeCount { get; private set; }
    public int RejectedSpawnCount { get; private set; }
    public bool StrictMode { get; set; }
    /// <summary>Call depth ceiling; a deeper 210/300 call is dropped (exec.rs spawn_child).</summary>
    public const int MaxCallDepth = 64;
    /// <summary>Latest unimplemented-opcode error, also the save-blocking fault reason.</summary>
    public string? FaultReason { get; private set; }
    public bool Faulted => FaultReason != null;
    /// <summary>Auto-input mask requested by opcode 125.</summary>
    public uint AutoKeys { get; private set; }
    /// <summary>126 state 1: these keys stop walking but still reach 123.</summary>
    public uint BasicMoveDeny { get; private set; }
    /// <summary>126 state 2: these keys stop both walking and 123.</summary>
    public uint BasicAllDeny { get; private set; }
    /// <summary>Dedupe key for the reserved sound-type warning (R7-NEW-02).</summary>
    private readonly HashSet<(int EventId, int Pc, int SoundType)> _unknownSoundTypeLogs = new();
    /// <summary>126 device form: physical input denied (125 auto input still passes).</summary>
    public bool DeviceDeny { get; private set; }
    /// <summary>Bits blocked for walking (both 126 permission states).</summary>
    public uint MovementDeny => BasicMoveDeny | BasicAllDeny;
    /// <summary>Target bits in the low byte of a 126 argument.</summary>
    public const uint InputTargetMask = 0xFF;
    /// <summary>Bit 28 selects 126's device form.</summary>
    public const uint InputDeviceFlag = 0x1000_0000;

    /// <summary>
    /// Apply one 126 argument: target in bits 0..7, state in bits 8..9.
    /// State 0 restores, 1 blocks movement only, 2 blocks movement and key input.
    /// Bit 28 switches to the device form, which only has allow/deny.
    /// </summary>
    public void ApplyInputPermission(uint raw)
    {
        uint target = raw & InputTargetMask;
        uint state = (raw >> 8) & 0x3;
        if ((raw & InputDeviceFlag) != 0)
        {
            if (target != 0) DeviceDeny = state != 0;
            return;
        }
        if (target == 0) return;
        switch (state)
        {
            case 0:
                BasicMoveDeny &= ~target;
                BasicAllDeny &= ~target;
                break;
            case 1:
                BasicMoveDeny |= target;
                BasicAllDeny &= ~target;
                break;
            case 2:
                BasicAllDeny |= target;
                BasicMoveDeny &= ~target;
                break;
        }
    }

    /// <summary>Clear the session input policy (new game / return to title).</summary>
    public void ResetInputPermissions()
    {
        AutoKeys = 0;
        BasicMoveDeny = 0;
        BasicAllDeny = 0;
        DeviceDeny = false;
    }

    /// <summary>
    /// Filter a 123 host answer through the 126 basic-input policy. Script-driven
    /// auto input still passes a device denial, matching 125's keyboard bypass.
    /// </summary>
    public int FilterKeyCode(int code)
    {
        uint bits = KeyCodes.BitsForCode(code);
        if (bits == 0) return code;
        if ((bits & BasicAllDeny) != 0) return 0;
        if (DeviceDeny && (bits & AutoKeys) == 0) return 0;
        return code;
    }
    private int _nextInstanceId = 1;
    private readonly Dictionary<string, int> _titleToId = new(StringComparer.Ordinal);
    private readonly HashSet<int> _freshInstanceIds = new();

    public void RegisterProgram(Program program)
    {
        Programs[program.Id] = program;
        if (!string.IsNullOrEmpty(program.Title)) _titleToId.TryAdd(program.Title, program.Id);
    }

    /// <summary>Resolve the target of a 300 CallByName (exec.rs id_by_name).</summary>
    public Program? GetProgramByTitle(string title)
    {
        return _titleToId.TryGetValue(title, out int id) ? GetProgram(id) : null;
    }

    /// <summary>Drops the latched fault so a new game can start from a clean slate.</summary>
    public void ResetDiagnostics()
    {
        FaultReason = null;
        UnhandledOpcodeCount = 0;
        RejectedSpawnCount = 0;
    }

    /// <summary>Number of live parents above <paramref name="inst"/>.</summary>
    public int CallDepth(Instance inst)
    {
        int depth = 0;
        var current = inst;
        var seen = new HashSet<int>();
        while (current.ParentId.HasValue && depth <= MaxCallDepth && seen.Add(current.ParentId.Value))
        {
            var parent = FindInstance(current.ParentId.Value);
            if (parent == null) break;
            depth++;
            current = parent;
        }
        return depth;
    }

    public Program? GetProgram(int id)
    {
        return Programs.TryGetValue(id, out var prog) ? prog : null;
    }

    /// <summary>Start a coroutine for a program that is not in the common-event set (map pages).</summary>
    /// <summary>Reattach a coroutine loaded from a save slot without restarting its program.</summary>
    public void RestoreInstance(Instance inst)
    {
        Instances.Add(inst);
        if (inst.Id >= _nextInstanceId) _nextInstanceId = inst.Id + 1;
    }

    public Instance? CreateInstance(Program program, int? parentId = null, int? returnTo = null)
    {
        if (Instances.Count >= MaxInstances)
        {
            RejectedSpawnCount++;
            Host.Log($"instance limit reached; start dropped (event {program.Id})");
            return null;
        }
        var inst = new Instance
        {
            Id = _nextInstanceId++,
            ParentId = parentId,
            EventId = program.Id,
            Program = program,
            Pc = 0,
            ReturnTo = returnTo
        };
        if (program.SelfBase == Program.CommonSelfBase && CommonSelf.TryGetValue(program.Id, out var bank))
        {
            inst.Self.CopyFrom(bank);
        }
        Instances.Add(inst);
        return inst;
    }

    public Instance? CreateInstance(int eventId, int? parentId = null, int? returnTo = null)
    {
        var prog = GetProgram(eventId);
        if (prog == null)
        {
            RejectedSpawnCount++;
            Host.Log($"unknown common event id {eventId}; call dropped");
            return null;
        }
        return CreateInstance(prog, parentId, returnTo);
    }

    /// <summary>
    /// Spawn a synthetic event with a blank self bank and no persistence write-back
    /// (exec.rs start_event_fresh). CE241/CE242 have no script-issued 210 call, so their
    /// per-invocation state must not leak through the common-event store.
    /// </summary>
    public Instance? CreateFreshInstance(int eventId, int? parentId = null, int? returnTo = null)
    {
        var instance = CreateInstance(eventId, parentId, returnTo);
        if (instance == null) return null;
        instance.Self.Restore(null, null);
        _freshInstanceIds.Add(instance.Id);
        return instance;
    }

    /// <summary>Decode the operand table shared by 210 and 300 (exec.rs call_arguments).</summary>
    private (int[] Args, string[] Strings, int? ReturnTo) DecodeCallArguments(Slot slot, Instance inst)
    {
        int status = slot.Params.Length > 1 ? slot.Params[1] : 0;
        int numCount = status & 0x0F;
        int strCount = (status >> 4) & 0x0F;
        bool hasRet = ((status >> 24) & 1) != 0;

        var args = new int[numCount];
        for (int i = 0; i < numCount; i++)
        {
            args[i] = EvalNum(2 + i < slot.Params.Length ? slot.Params[2 + i] : 0, inst);
        }

        var strings = new string[strCount];
        for (int k = 0; k < strCount && k < 4; k++)
        {
            bool literal = ((status >> (12 + k)) & 1) != 0;
            int wordIndex = 2 + numCount + k;
            int word = wordIndex < slot.Params.Length ? slot.Params[wordIndex] : 0;
            strings[k] = literal || word == 0
                ? (1 + k < slot.Strings.Length ? slot.Strings[1 + k] : "")
                : EvalStr(word, inst);
        }

        int retIndex = 2 + numCount + strCount;
        int? returnTo = hasRet && retIndex < slot.Params.Length ? slot.Params[retIndex] : null;
        return (args, strings, returnTo);
    }

    public string Expand(string text, Instance inst)
    {
        return TextExpander.Expand(text, inst, Vars);
    }

    public int EvalNum(int raw, Instance inst)
    {
        var magic = Magic.Decode(raw);
        return magic.Kind switch
        {
            MagicKind.Literal => (int)magic.Index,
            MagicKind.SelfVar => inst.Self.Get((int)magic.Index),
            MagicKind.Var => Vars.Get((int)magic.Index),
            MagicKind.SysVar => Vars.GetSys((int)magic.Index),
            MagicKind.StrVar => 0,
            MagicKind.Opaque => raw,
            _ => raw
        };
    }

    public void StoreNum(Instance inst, int raw, int value)
    {
        var magic = Magic.Decode(raw);
        switch (magic.Kind)
        {
            case MagicKind.SelfVar:
                inst.Self.Set((int)magic.Index, value);
                break;
            case MagicKind.Var:
                Vars.Set((int)magic.Index, value);
                break;
            case MagicKind.SysVar:
                Vars.SetSys((int)magic.Index, value);
                break;
            case MagicKind.Literal:
            case MagicKind.StrVar:
            case MagicKind.Opaque:
                break;
        }
    }

    public string EvalStr(int raw, Instance inst)
    {
        var magic = Magic.Decode(raw);
        return magic.Kind switch
        {
            MagicKind.StrVar => Vars.GetStr((int)magic.Index),
            MagicKind.SelfVar => inst.Self.GetStr((int)magic.Index),
            MagicKind.SysVar => Vars.GetSysStr((int)magic.Index),
            MagicKind.Literal => magic.Index.ToString(),
            MagicKind.Var => Vars.Get((int)magic.Index).ToString(),
            _ => ""
        };
    }

    public void StoreStr(Instance inst, int raw, string value)
    {
        var magic = Magic.Decode(raw);
        switch (magic.Kind)
        {
            case MagicKind.StrVar:
                Vars.SetStr((int)magic.Index, value);
                break;
            case MagicKind.SelfVar:
                inst.Self.SetStr((int)magic.Index, value);
                break;
            case MagicKind.SysVar:
                Vars.SetSysStr((int)magic.Index, value);
                break;
        }
    }

    public static int BinaryOp(byte kind, int a, int b)
    {
        unchecked
        {
            return kind switch
            {
                0x0 => a + b,
                0x1 => a - b,
                0x2 => a * b,
                0x3 => b == 0 || (a == int.MinValue && b == -1) ? (b == 0 ? a : int.MinValue) : a / b,
                0x4 => b == 0 ? a : a % b,
                0x5 => a & b,
                0x6 => a >> (b & 31),
                0x7 => a | b,
                0x8 => a ^ b,
                0x9 => a << (b & 31),
                _ => a
            };
        }
    }

    public static int Compound(int kind, int current, int value)
    {
        unchecked
        {
            return (kind & 0x0F00) switch
            {
                0x000 => value,
                0x100 => current + value,
                0x200 => current - value,
                0x300 => current * value,
                0x400 => value == 0 || (current == int.MinValue && value == -1) ? (value == 0 ? current : int.MinValue) : current / value,
                0x500 => value == 0 ? current : current % value,
                0x600 => current,
                0x700 => current,
                0x800 => current < 0 ? (current == int.MinValue ? int.MaxValue : -current) : current,
                0x900 => (int)(Math.Atan2(value, current) * (180.0 / Math.PI) * 10.0),
                0xA00 => (int)(Math.Sin((value / 10.0) * (Math.PI / 180.0)) * 1000.0),
                0xB00 => (int)(Math.Cos((value / 10.0) * (Math.PI / 180.0)) * 1000.0),
                0xC00 => (int)(Math.Sqrt(Math.Max(0, value)) * 1000.0),
                _ => value
            };
        }
    }

    public static bool Compare(int lhs, int rhs, int op)
    {
        return op switch
        {
            0 => lhs > rhs,
            1 => lhs >= rhs,
            2 => lhs == rhs,
            3 => lhs <= rhs,
            4 => lhs < rhs,
            5 => lhs != rhs,
            6 => (lhs & rhs) != 0,
            _ => lhs == rhs
        };
    }

    public void ExecuteSetVariable(int[] args, Instance inst)
    {
        int target = args[0];
        int a = EvalNum(args[1], inst);
        int b = EvalNum(args[2], inst);
        uint bits = (uint)args[3];
        byte bin = (byte)((bits >> 12) & 0x0F);

        int result = bin == 6 ? Rng.Range(a, b) : BinaryOp(bin, a, b);

        int lastOffset = 0;
        if ((bits & 0x10000) != 0 && args.Length >= 5)
        {
            int evalOffset = EvalNum(args[4], inst);
            lastOffset = Math.Clamp(evalOffset, 0, 1024);
        }

        unchecked
        {
            for (int offset = 0; offset <= lastOffset; offset++)
            {
                int slot = target + offset;
                int current = EvalNum(slot, inst);
                int value = Compound((int)(bits & 0x0FFF), current, result);
                StoreNum(inst, slot, value);
            }
        }
    }

    /// <summary>Called once per executed instruction, before dispatch, with the program counter.</summary>
    public Action<Instance, int>? StepHook { get; set; }

    public Signal StepInstance(Instance inst, ref int budget)
    {
        var prog = inst.Program;
        while (inst.Pc < prog.Len)
        {
            if (budget <= 0) return Signal.Continue;
            budget--;

            int pc = inst.Pc;
            var slot = prog.Slots[pc];
            StepHook?.Invoke(inst, pc);

            // 1. Control flow
            var ctrl = pc < prog.ControlMap.Length ? prog.ControlMap[pc] : Control.None;
            switch (ctrl.Kind)
            {
                case ControlKind.Block:
                {
                    var block = prog.Blocks[ctrl.BlockIndex];
                    switch (block.Kind)
                    {
                        case BlockKind.Condition:
                        {
                            int head = slot.Params.Length > 0 ? slot.Params[0] : 0;
                            if (slot.Opcode == 112)
                            {
                                // 112 StringCondition: strings[i] is the literal to match,
                                // Params[1+i] the string operand, bit 0x10 selects the else branch.
                                int strCount = head & 0x0F;
                                bool useElse = (head & 0x10) != 0;
                                int? matchedStr = null;
                                for (int i = 0; i < strCount; i++)
                                {
                                    string lhs = EvalStr(slot.Params.Length > 1 + i ? slot.Params[1 + i] : 0, inst);
                                    string rhs = Expand(i < slot.Strings.Length ? slot.Strings[i] : "", inst);
                                    if (lhs == rhs)
                                    {
                                        matchedStr = i + 1;
                                        break;
                                    }
                                }
                                int? strTarget = matchedStr.HasValue ? block.CaseTarget(matchedStr.Value) : null;
                                if (strTarget == null && useElse)
                                {
                                    strTarget = block.ElsePc;
                                }
                                inst.Pc = strTarget ?? block.EndTarget(prog.Len);
                                continue;
                            }
                            int count = head & 0x0F;
                            int? matched = null;
                            for (int i = 0; i < count; i++)
                            {
                                int varArg = slot.Params[1 + 3 * i];
                                int valArg = slot.Params[2 + 3 * i];
                                int opArg = slot.Params[3 + 3 * i];
                                int lhs = EvalNum(varArg, inst);
                                int rhs = EvalNum(valArg, inst);
                                if (Compare(lhs, rhs, opArg))
                                {
                                    matched = i + 1;
                                    break;
                                }
                            }

                            int? target = matched.HasValue ? block.CaseTarget(matched.Value) : null;
                            target ??= block.ElsePc;
                            inst.Pc = target ?? block.EndTarget(prog.Len);
                            continue;
                        }
                        case BlockKind.Loop:
                        {
                            long remaining = -1;
                            if (slot.Opcode == 179)
                            {
                                int count = slot.Params.Length > 0 ? EvalNum(slot.Params[0], inst) : 0;
                                if (count <= 0)
                                {
                                    inst.Pc = block.EndTarget(prog.Len);
                                    continue;
                                }
                                remaining = count;
                            }
                            inst.Loops.Add(new LoopCtx { Block = ctrl.BlockIndex, Remaining = remaining });
                            inst.Pc++;
                            continue;
                        }
                        case BlockKind.Choices:
                        {
                            string[] options = new string[slot.Strings.Length];
                            for (int i = 0; i < slot.Strings.Length; i++)
                            {
                                options[i] = Expand(slot.Strings[i], inst);
                            }
                            Host.ShowChoices(options);
                            inst.Wait.WaitForChoice(ctrl.BlockIndex);
                            inst.Pc++;
                            return Signal.Blocked;
                        }
                    }
                    break;
                }
                case ControlKind.Case:
                {
                    // Reached case marker via sequential execution -> escape to end of block!
                    var block = prog.Blocks[ctrl.BlockIndex];
                    inst.Pc = block.EndTarget(prog.Len);
                    continue;
                }
                case ControlKind.LoopEnd:
                {
                    // Reached loop end -> decrement and loop or exit
                    int blockIdx = ctrl.BlockIndex;
                    int loopPos = -1;
                    for (int i = inst.Loops.Count - 1; i >= 0; i--)
                    {
                        if (inst.Loops[i].Block == blockIdx)
                        {
                            loopPos = i;
                            break;
                        }
                    }
                    if (loopPos >= 0)
                    {
                        // LoopCtx is a struct: mutate the list element itself, never a copy,
                        // or a counted loop never reaches its last iteration.
                        long rem = inst.Loops[loopPos].Remaining;
                        bool again;
                        if (rem < 0)
                        {
                            again = true; // 170/176 unconditional loop heads
                        }
                        else if (rem > 1)
                        {
                            var ctx = inst.Loops[loopPos];
                            ctx.Remaining = rem - 1;
                            inst.Loops[loopPos] = ctx;
                            again = true;
                        }
                        else
                        {
                            again = false;
                        }
                        if (again)
                        {
                            inst.Pc = prog.Blocks[blockIdx].Head + 1;
                        }
                        else
                        {
                            // Rust close_loop truncates everything from the matching frame up.
                            inst.Loops.RemoveRange(loopPos, inst.Loops.Count - loopPos);
                            inst.Pc = prog.Blocks[blockIdx].EndTarget(prog.Len);
                        }
                    }
                    else
                    {
                        inst.Pc++;
                    }
                    continue;
                }
                case ControlKind.LabelJump:
                {
                    string name = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    string expandedName = Expand(name, inst);
                    if (prog.Labels.TryGetValue(expandedName, out int target))
                    {
                        inst.Pc = target;
                    }
                    else
                    {
                        inst.Pc = prog.Len;
                    }
                    continue;
                }
            }

            // 2. Opcode execution
            switch (slot.Opcode)
            {
                case 101:
                {
                    string raw = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    string expanded = Expand(raw, inst);
                    int? face = null;
                    string messageText = expanded;
                    if (expanded.StartsWith('@') && expanded.Length >= 2 && char.IsAsciiDigit(expanded[1]))
                    {
                        face = expanded[1] - '0';
                        messageText = expanded.Length > 2 ? expanded[2..] : "";
                    }
                    Host.ShowMessage(messageText, face);
                    inst.Wait.WaitForMessage();
                    inst.Pc++;
                    return Signal.Blocked;
                }
                case 102:
                {
                    string[] options = new string[slot.Strings.Length];
                    for (int i = 0; i < slot.Strings.Length; i++)
                    {
                        options[i] = Expand(slot.Strings[i], inst);
                    }
                    int blockIdx = ctrl.Kind == ControlKind.Block ? ctrl.BlockIndex : 0;
                    Host.ShowChoices(options);
                    inst.Wait.WaitForChoice(blockIdx);
                    inst.Pc++;
                    return Signal.Blocked;
                }
                case 103: // Comment
                    inst.Pc++;
                    break;
                case 105:
                    Host.CloseMessage();
                    inst.Pc++;
                    break;
                case 106:
                {
                    string raw = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    Host.Log(Expand(raw, inst));
                    inst.Pc++;
                    break;
                }
                case 111: // Condition
                    inst.Pc++;
                    break;
                case 121:
                    ExecuteSetVariable(slot.Params, inst);
                    inst.Pc++;
                    break;
                case 122:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    string raw = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    string expanded = Expand(raw, inst);
                    StoreStr(inst, target, expanded);
                    inst.Pc++;
                    break;
                }
                case 123:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    // The mask is a composite UI selector, not a set of allowed keys;
                    // 126 is applied to the key code the host answers with.
                    uint mask = slot.Params.Length > 1 ? (uint)slot.Params[1] : 0;
                    int key = FilterKeyCode(Host.KeyCodeAuto(mask, AutoKeys));
                    StoreNum(inst, target, key);
                    inst.Pc++;
                    break;
                }
                case 124:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int sub = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int field = slot.Params.Length >= 4 ? slot.Params[3] : (slot.Params.Length > 2 ? slot.Params[2] : 0);
                    int? val = sub switch
                    {
                        0x3000 => field == 18 ? inst.EventId + 500_000 : Host.EngineState(field),
                        0x1000 or 0x1200 => Host.CharacterState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                        0x4000 or 0x4400 => Host.PictureState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                        _ => null
                    };
                    if (val.HasValue)
                    {
                        StoreNum(inst, target, val.Value);
                    }
                    inst.Pc++;
                    break;
                }
                case 130:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : -1;
                    int x = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int y = slot.Params.Length > 2 ? slot.Params[2] : 0;
                    int map = slot.Params.Length > 3 ? slot.Params[3] : -1;
                    int options = slot.Params.Length > 4 ? slot.Params[4] : 0;
                    Host.Teleport(target, map, x, y, options);
                    inst.Wait.WaitForFrames(1);
                    inst.Pc++;
                    return Signal.Blocked;
                }
                case 140:
                {
                    int bits = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    // intArgs[0] >> 4 is the sound type: 0=BGM, 1=BGS, 2=SE. Testing 0x20
                    // alone classified the 43 BGS commands as BGM and replaced the music.
                    int soundType = (bits >> 4) & 0x07;
                    if (soundType > 2 && _unknownSoundTypeLogs.Add((inst.EventId, inst.Pc, soundType)))
                    {
                        // 0/1/2 are BGM/BGS/SE; 3..7 are reserved. Keep the legacy BGM
                        // fallback but make it visible once per site instead of on every loop.
                        Host.Log($"[EventVm] unknown sound type {soundType} at event {inst.EventId} pc {inst.Pc}; mapped to BGM");
                    }
                    SoundChannel channel = soundType switch
                    {
                        1 => SoundChannel.Bgs,
                        2 => SoundChannel.Se,
                        _ => SoundChannel.Bgm
                    };
                    bool playing = bits >= 0 && (bits & 0x0200_0000) != 0;

                    SoundAction action;
                    string file;
                    int volume;
                    int frames;
                    int loopPoint = 0;
                    int tempo = 100;

                    if (playing)
                    {
                        string rawStr = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                        file = Expand(rawStr, inst);
                        volume = slot.Params.Length >= 6 ? slot.Params[4] : 100;
                        frames = slot.Params.Length >= 6 ? Math.Max(0, slot.Params[1]) : 0;
                        tempo = slot.Params.Length >= 6 ? slot.Params[5] : 100;
                        loopPoint = slot.Params.Length >= 7 ? Math.Max(0, slot.Params[6]) : 0;
                        action = SoundAction.Play;
                    }
                    else
                    {
                        file = "";
                        volume = 0;
                        frames = slot.Params.Length <= 5 ? (slot.Params.Length > 1 ? slot.Params[1] : 0) : 0;
                        action = SoundAction.Stop;
                    }

                    var cmd = new SoundCmd
                    {
                        Channel = channel,
                        Action = action,
                        File = file,
                        Volume = volume,
                        Frames = frames,
                        LoopPoint = loopPoint,
                        Tempo = tempo
                    };
                    Host.Sound(cmd);
                    inst.Pc++;
                    break;
                }
                case 150:
                {
                    int head = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    byte kind = (byte)((head >> 4) & 0x07);
                    byte anchor = (byte)((head >> 12) & 0x07);
                    PictureAction action = (head & 0x0F) switch
                    {
                        0 => PictureAction.Draw,
                        1 => PictureAction.Move,
                        2 => PictureAction.Erase,
                        3 => PictureAction.Hide,
                        _ => PictureAction.Set
                    };

                    int rawNumber = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int number = EvalNum(rawNumber, inst);
                    string rawStr = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    string expanded = Expand(rawStr, inst);

                    string file = "";
                    string text = "";
                    switch (kind)
                    {
                        case 0 or 3:
                            file = expanded;
                            break;
                        case 2:
                            text = expanded;
                            break;
                        case 1 or 4:
                            if (slot.Params.Length > 11)
                            {
                                var magic = Magic.Decode(slot.Params[11]);
                                file = magic.Kind switch
                                {
                                    MagicKind.SelfVar => inst.Self.GetStr((int)magic.Index),
                                    MagicKind.StrVar => Vars.GetStr((int)magic.Index),
                                    _ => ""
                                };
                            }
                            break;
                    }

                    int argsCount = Math.Max(0, slot.Params.Length - 2);
                    int[] args = new int[argsCount];
                    for (int i = 0; i < argsCount; i++)
                    {
                        int raw = slot.Params[2 + i];
                        if (((uint)raw & 0x0F00_0000) != 0)
                        {
                            args[i] = raw;
                        }
                        else
                        {
                            args[i] = EvalNum(raw, inst);
                        }
                    }

                    int[] animate = args.Length > 11 ? args[11..] : Array.Empty<int>();

                    var cmd = new PictureCmd
                    {
                        Number = number,
                        Kind = kind,
                        Action = action,
                        File = file,
                        Text = text,
                        Args = args,
                        Anchor = anchor,
                        Animate = animate
                    };

                    Host.Picture(cmd);
                    inst.Pc++;
                    break;
                }
                case 160 or 161 or 162:
                {
                    int kind = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int flags = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    Host.Transition(kind, flags);
                    inst.Pc++;
                    break;
                }
                case 171:
                {
                    if (inst.Loops.Count > 0)
                    {
                        var top = inst.Loops[^1];
                        inst.Loops.RemoveAt(inst.Loops.Count - 1);
                        inst.Pc = prog.Blocks[top.Block].EndTarget(prog.Len);
                    }
                    else
                    {
                        inst.Pc++;
                    }
                    break;
                }
                case 172 or 173:
                    inst.Pc = prog.Len;
                    return Signal.Finished;
                case 180:
                {
                    int frames = slot.Params.Length > 0 ? EvalNum(slot.Params[0], inst) : 1;
                    uint uFrames = (uint)Math.Max(1, frames);
                    Host.Wait(uFrames);
                    inst.Wait.WaitForFrames(uFrames);
                    inst.Pc++;
                    return Signal.Blocked;
                }
                case 210:
                {
                    int rawTarget = slot.Params[0];
                    if (Magic.Decode(rawTarget).IsStorage)
                    {
                        rawTarget = EvalNum(rawTarget, inst);
                    }
                    int? targetEvent = rawTarget switch
                    {
                        >= 500000 and < 600000 => rawTarget - 500000,
                        >= 600000 and < 700000 => rawTarget - 600000,
                        _ => null
                    };
                    if (targetEvent == null)
                    {
                        // exec.rs 210: only the two call-address ranges are valid targets.
                        RejectedSpawnCount++;
                        Host.Log($"210 CallCommonEvent: unsupported target {rawTarget}");
                        inst.Pc++;
                        break;
                    }

                    int paramStatus = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int numCount = paramStatus & 0x0F;
                    int strCount = (paramStatus >> 4) & 0x0F;
                    bool hasRet = ((paramStatus >> 24) & 1) != 0;

                    int[] childArgs = new int[numCount];
                    for (int i = 0; i < numCount; i++)
                    {
                        childArgs[i] = EvalNum(slot.Params[2 + i], inst);
                    }

                    // String operands live after the numeric ones; bit 12+k marks a literal
                    // (top-level strings[1+k]) instead of a magic address into a string bank.
                    string[] childStrings = new string[strCount];
                    for (int k = 0; k < strCount && k < 4; k++)
                    {
                        bool literal = ((paramStatus >> (12 + k)) & 1) != 0;
                        int wordIdx = 2 + numCount + k;
                        int word = wordIdx < slot.Params.Length ? slot.Params[wordIdx] : 0;
                        childStrings[k] = literal || word == 0
                            ? (1 + k < slot.Strings.Length ? slot.Strings[1 + k] : "")
                            : EvalStr(word, inst);
                    }

                    // Numeric args, then string args, then the optional return slot:
                    // 2 + total_count, not 2 + number_count.
                    int retIdx = 2 + numCount + strCount;
                    int? returnTo = hasRet && retIdx < slot.Params.Length ? slot.Params[retIdx] : null;

                    if (CallDepth(inst) >= MaxCallDepth)
                    {
                        RejectedSpawnCount++;
                        Host.Log($"max call depth {MaxCallDepth} reached; call dropped (event {targetEvent.Value})");
                        inst.Pc++;
                        break;
                    }

                    var child = CreateInstance(targetEvent.Value, inst.Id, returnTo);
                    if (child == null)
                    {
                        // exec.rs spawn_child: a rejected call is dropped without
                        // blocking the caller; execution continues at the next slot.
                        inst.Pc++;
                        break;
                    }
                    Host.CommonEventCall(targetEvent.Value, childArgs, childStrings);
                    for (int i = 0; i < childArgs.Length; i++)
                    {
                        child.Self.Set(Magic.SelfArgBase + i, childArgs[i]);
                    }
                    for (int k = 0; k < childStrings.Length; k++)
                    {
                        child.Self.SetStr(Magic.SelfArgBase + 5 + k, childStrings[k]);
                    }

                    inst.Wait.WaitForChild(child.Id);
                    inst.Pc++;
                    return Signal.Call;
                }
                case 212: // Label marker
                    inst.Pc++;
                    break;
                case 220 or 221 or 222:
                {
                    // Operand order per docs/OPCODE_220_SAVE.md: the file is arg1, the
                    // mode (220) and destination address (221) are arg0, the address is
                    // arg2 for 221/222, and the trailing flag is arg3 when present.
                    byte opc = (byte)slot.Opcode;
                    int argc = slot.Params.Length;
                    int fileRaw = argc > 1 ? slot.Params[1] : 0;
                    SaveTarget target = Magic.Decode(fileRaw).Kind == MagicKind.StrVar
                        ? new SaveTarget(EvalStr(fileRaw, inst))
                        : new SaveTarget(EvalNum(fileRaw, inst));
                    int mode = opc == 220 && argc > 0 ? EvalNum(slot.Params[0], inst) : 0;
                    int dest = opc == 221 && argc > 0 ? slot.Params[0] : 0;
                    int addr = opc == 220 || argc <= 2 ? 0 : slot.Params[2];
                    int val = opc == 222 && argc > 0 ? EvalNum(slot.Params[0], inst) : 0;
                    int flag = argc >= 4 ? EvalNum(slot.Params[3], inst) : 0;

                    var saveOp = new SaveOp
                    {
                        Opcode = opc,
                        Mode = mode,
                        File = target,
                        Destination = dest,
                        Address = addr,
                        Value = val,
                        Flag = flag,
                        Owner = inst.EventId
                    };
                    int? loaded = Host.SaveLoad(saveOp);
                    if (loaded.HasValue)
                    {
                        // The oracle stores the loaded word into destination for every
                        // save/load opcode (destination is 0 for 220/222).
                        StoreNum(inst, dest, loaded.Value);
                    }
                    inst.Pc++;
                    break;
                }
                case 211: // Reserve a common event without calling it
                {
                    int rawReserve = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    if (Magic.Decode(rawReserve).IsStorage)
                    {
                        rawReserve = EvalNum(rawReserve, inst);
                    }
                    int reserveTarget = rawReserve >= 500000 && rawReserve < 600000
                        ? rawReserve - 500000
                        : rawReserve >= 600000 && rawReserve < 700000
                            ? rawReserve - 600000
                            : Math.Max(0, rawReserve);
                    Host.Reserve(reserveTarget);
                    inst.Pc++;
                    break;
                }
                case 290: // Effect playback (flash, shake, balloon, ...)
                {
                    // Every operand is magic, not a constant: the balloon library passes
                    // self variables as picture ids.
                    int[] effArgs = new int[slot.Params.Length];
                    for (int i = 0; i < effArgs.Length; i++)
                    {
                        effArgs[i] = EvalNum(slot.Params[i], inst);
                    }
                    int effKind = effArgs.Length > 0 ? effArgs[0] >> 4 : 0;
                    int effDuration = effArgs.Length > 1 ? effArgs[1] : 0;
                    Host.Effect(effKind, effDuration, effArgs);
                    inst.Pc++;
                    break;
                }
                case 250:
                {
                    int flags = slot.Params.Length > 3 ? slot.Params[3] : 0;
                    DbContainer container = ((flags >> 8) & 3) switch
                    {
                        1 => DbContainer.Sys,
                        2 => DbContainer.Data,
                        _ => DbContainer.CData
                    };
                    DbDirection direction = (flags & 0x1000) != 0 ? DbDirection.Read : DbDirection.Write;
                    byte op = (byte)((flags >> 4) & 7);

                    int dbType = slot.Params.Length > 0 ? EvalNum(slot.Params[0], inst) : 0;
                    int dataId = slot.Params.Length > 1 ? EvalNum(slot.Params[1], inst) : 0;
                    int itemId = slot.Params.Length > 2 ? EvalNum(slot.Params[2], inst) : 0;

                    int? destination = null;
                    DbValue? source = null;

                    if (direction == DbDirection.Read)
                    {
                        if (slot.Params.Length >= 5)
                            destination = slot.Params[4];
                    }
                    else
                    {
                        if (slot.Params.Length >= 5)
                        {
                            int valRaw = slot.Params[4];
                            var valMagic = Magic.Decode(valRaw);
                            if (valMagic.IsString || (valMagic.Kind == MagicKind.SelfVar && !string.IsNullOrEmpty(EvalStr(valRaw, inst))))
                            {
                                source = new DbValue(EvalStr(valRaw, inst));
                            }
                            else
                            {
                                source = new DbValue(EvalNum(valRaw, inst));
                            }
                        }
                        else if (slot.Strings.Length > 0)
                        {
                            source = new DbValue(slot.Strings[0]);
                        }
                    }

                    string? typeLabel = slot.Strings.Length > 1 && !string.IsNullOrEmpty(slot.Strings[1]) ? slot.Strings[1] : null;
                    string? fieldLabel = slot.Strings.Length > 3 && !string.IsNullOrEmpty(slot.Strings[3]) ? slot.Strings[3] : null;

                    var dbOp = new DbOp
                    {
                        Container = container,
                        Direction = direction,
                        Op = op,
                        DbType = dbType,
                        DataId = dataId,
                        ItemId = itemId,
                        Flags = flags,
                        Destination = destination,
                        Source = source,
                        TypeLabel = typeLabel,
                        FieldLabel = fieldLabel
                    };

                    var dbVal = Host.Database(dbOp);
                    if (direction == DbDirection.Read && destination.HasValue && dbVal != null)
                    {
                        if (dbVal.IsString)
                        {
                            StoreStr(inst, destination.Value, dbVal.Str);
                        }
                        else
                        {
                            StoreNum(inst, destination.Value, dbVal.Num);
                        }
                    }

                    inst.Pc++;
                    break;
                }
                case 104 or 213 or 421 or 498:
                    // Present in the retail programs but without a side effect (exec.rs).
                    inst.Pc++;
                    break;
                case 107:
                    Host.ClearDebug();
                    inst.Pc++;
                    break;
                case 125:
                    AutoKeys = slot.Params.Length > 0 ? (uint)slot.Params[0] : 0;
                    inst.Pc++;
                    break;
                case 126:
                    ApplyInputPermission(slot.Params.Length > 0 ? (uint)slot.Params[0] : 0);
                    inst.Pc++;
                    break;
                case 151:
                {
                    int color = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int frames = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    Host.ChangeColor(color, frames);
                    inst.Pc++;
                    break;
                }
                case 174:
                    Host.ReturnToTitle();
                    inst.Pc = prog.Len;
                    return Signal.ReturnToTitle;
                case 175:
                    Host.EndGame();
                    Instances.Clear();
                    return Signal.EndGame;
                case 201:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    if (slot.MoveRoute != null) Host.MoveRoute(target, slot.MoveRoute);
                    inst.Pc++;
                    break;
                }
                case 270:
                    Host.Party(slot.Params);
                    inst.Pc++;
                    break;
                case 281:
                    Host.Scroll(slot.Params);
                    inst.Pc++;
                    break;
                case 300:
                {
                    string name = Expand(slot.Strings.Length > 0 ? slot.Strings[0] : "", inst);
                    var callee = GetProgramByTitle(name);
                    if (callee == null)
                    {
                        // exec.rs 300: an unknown title logs and continues instead of blocking.
                        RejectedSpawnCount++;
                        Host.Log($"300 CallByName: no event titled {name}");
                        inst.Pc++;
                        break;
                    }
                    if (CallDepth(inst) >= MaxCallDepth)
                    {
                        RejectedSpawnCount++;
                        Host.Log($"max call depth {MaxCallDepth} reached; call dropped (event titled {name})");
                        inst.Pc++;
                        break;
                    }

                    var (callArgs, callStrings, returnTo) = DecodeCallArguments(slot, inst);
                    var child = CreateInstance(callee, inst.Id, returnTo);
                    if (child == null)
                    {
                        inst.Pc++;
                        break;
                    }
                    // 210 and 300 both reach a common event; the trace/side-effect host must
                    // see the call either way (round-4 review: 300 was silently missing it).
                    Host.CommonEventCall(callee.Id, callArgs, callStrings);
                    for (int i = 0; i < callArgs.Length; i++)
                    {
                        child.Self.Set(Magic.SelfArgBase + i, callArgs[i]);
                    }
                    for (int k = 0; k < callStrings.Length; k++)
                    {
                        child.Self.SetStr(Magic.SelfArgBase + 5 + k, callStrings[k]);
                    }
                    inst.Wait.WaitForChild(child.Id);
                    inst.Pc++;
                    return Signal.Call;
                }
                case 1000:
                {
                    int rawKind = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int milliseconds = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int kind = rawKind >> 16;
                    Host.ProFeature(kind, milliseconds);
                    inst.Pc++;
                    if (kind == 5)
                    {
                        uint waitFrames = (uint)Math.Max(1, milliseconds / 16);
                        inst.Wait.WaitForFrames(waitFrames);
                        return Signal.Blocked;
                    }
                    break;
                }
                case 0 or 99 or 401 or 420 or 499:
                    // 0 and 99 are NOP markers in the retail programs; 401/420/499 are
                    // block structure markers. None of them is an unimplemented opcode.
                    inst.Pc++;
                    break;
                default:
                    UnhandledOpcodeCount++;
                    FaultReason ??= $"unhandled opcode {slot.Opcode} at event {inst.EventId} pc {pc}";
                    Host.Log($"unhandled opcode {slot.Opcode} at event {inst.EventId} pc {pc}");
                    if (StrictMode)
                    {
                        throw new InvalidOperationException(FaultReason);
                    }
                    inst.Pc++;
                    break;
            }
        }

        return Signal.Finished;
    }

    public void StepFrame()
    {
        Frame++;
        PollWaits();
        int budget = 200_000;
        RunInstances(ref budget);
        ReapFinished();
    }

    public void PollWaits()
    {
        for (int i = 0; i < Instances.Count; i++)
        {
            var inst = Instances[i];
            if (inst.Wait.Kind == WaitStateKind.Frames)
            {
                if (inst.Wait.Frames <= 1)
                {
                    inst.Wait.Clear();
                }
                else
                {
                    inst.Wait.Frames--;
                }
            }
            else if (inst.Wait.Kind == WaitStateKind.Choice)
            {
                var reply = Host.ChoiceMade();
                if (reply != null)
                {
                    var block = inst.Program.Blocks[inst.Wait.Block];
                    int target;
                    if (reply.IsCancel)
                    {
                        target = block.CancelPc ?? block.EndTarget(inst.Program.Len);
                    }
                    else
                    {
                        target = block.CaseTarget(reply.Index + 2) ?? block.EndTarget(inst.Program.Len);
                    }
                    inst.Pc = target;
                    inst.Wait.Clear();
                }
            }
            else if (inst.Wait.Kind == WaitStateKind.Message)
            {
                if (Host.MessageDismissed())
                {
                    Host.CloseMessage();
                    inst.Wait.Clear();
                }
            }
        }
    }

    public Instance? FindInstance(int id)
    {
        for (int i = 0; i < Instances.Count; i++)
        {
            if (Instances[i].Id == id) return Instances[i];
        }
        return null;
    }

    /// <summary>Kill one coroutine and every coroutine it spawned, waking any parent that was waiting for it.</summary>
    public void StopInstance(int id)
    {
        var inst = FindInstance(id);
        if (inst == null) return;

        // Descendants die with their ancestor: a child coroutine without its caller
        // has nobody left to return to. This matters when a map page transitions:
        // the old page may be parked in a 210 call chain (CE4 -> CE245 -> ... -> CE329),
        // and leaving the chain alive leaks a whole story event into the next page.
        var doomed = new HashSet<int> { id };
        bool grew = true;
        while (grew)
        {
            grew = false;
            foreach (var other in Instances)
            {
                if (doomed.Contains(other.Id)) continue;
                if (other.ParentId.HasValue && doomed.Contains(other.ParentId.Value))
                {
                    doomed.Add(other.Id);
                    grew = true;
                }
            }
        }

        foreach (var other in Instances)
        {
            if (doomed.Contains(other.Id)) PersistCommonSelf(other);
        }
        Instances.RemoveAll(other => doomed.Contains(other.Id));
        for (int i = 0; i < Instances.Count; i++)
        {
            var other = Instances[i];
            if (other.Wait.Kind == WaitStateKind.Child && doomed.Contains(other.Wait.ChildId)) other.Wait.Clear();
        }
    }

    /// <summary>Drop every coroutine (map switch / teleport).</summary>
    public void StopAll()
    {
        foreach (var inst in Instances) PersistCommonSelf(inst);
        Instances.Clear();
    }

    private void PersistCommonSelf(Instance inst)
    {
        // Fresh synthetic events never write their bank back; drop the marker here so the
        // set cannot grow across generations.
        if (_freshInstanceIds.Remove(inst.Id)) return;
        if (inst.Program.SelfBase == Program.CommonSelfBase)
        {
            CommonSelf[inst.EventId] = inst.Self.Clone();
        }
    }

    public void RunInstances(ref int budget)
    {
        for (int i = 0; i < Instances.Count; i++)
        {
            if (budget <= 0) break;
            var inst = Instances[i];
            if (inst.IsRunnable)
            {
                StepInstance(inst, ref budget);
            }
        }
    }

    public void ReapFinished()
    {
        for (int i = Instances.Count - 1; i >= 0; i--)
        {
            var inst = Instances[i];
            if (inst.Pc >= inst.Program.Len && inst.Wait.Kind == WaitStateKind.None && !inst.RetainAfterFinish)
            {
                // Deliver return value to parent
                if (inst.ParentId.HasValue && inst.ReturnTo.HasValue)
                {
                    var parent = Instances.Find(p => p.Id == inst.ParentId.Value);
                    if (parent != null)
                    {
                        if (inst.Program.ReturnValueId != Program.NoReturnValue)
                        {
                            int val = inst.Self.Get((int)(Magic.SelfArgBase + inst.Program.ReturnValueId));
                            StoreNum(parent, inst.ReturnTo.Value, val);
                        }
                    }
                }

                PersistCommonSelf(inst);

                // Wake parent
                if (inst.ParentId.HasValue)
                {
                    var parent = Instances.Find(p => p.Id == inst.ParentId.Value);
                    if (parent != null && parent.Wait.Kind == WaitStateKind.Child && parent.Wait.ChildId == inst.Id)
                    {
                        parent.Wait.Clear();
                    }
                }

                Instances.RemoveAt(i);
            }
        }
    }
}
