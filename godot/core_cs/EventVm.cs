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
    public WolfRng Rng { get; } = WolfRng.FromEnvironment();
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
    /// <summary>When true, opcode 123 with bit 0x80 suspends the coroutine until an accepted key is pressed.</summary>
    public bool BlockOnKeyWait { get; set; }
    /// <summary>When true, 600xxx common-event call targets resolve relative to the caller (raw - 600100).</summary>
    public bool RelativeCommonCalls { get; set; }
    /// <summary>When true, parents woken by a zero-wait 210 child finish resume in the same frame.</summary>
    public bool ResumeOnChildReturn { get; set; }
    /// <summary>Optional guard queried before stepping a parallel instance at Pc == 0.</summary>
    public Func<Instance, bool>? CanStepParallelInstance { get; set; }
    /// <summary>Optional hook returning true to skip a 210 CallCommonEvent invocation.</summary>
    public Func<Instance, int, bool>? ShouldSkipCall { get; set; }
    /// <summary>Optional hook invoked when a child/common coroutine finishes and is reaped.</summary>
    public Action<Instance>? OnInstanceFinished { get; set; }
    /// <summary>Transition sub-phase reported alongside TransitionRequested (0=store, 1=freeze, 2=fadeout).</summary>
    public int TransitionPhase { get; private set; }
    /// <summary>Auto-input mask requested by opcode 125, in WOLF editor key layout.</summary>
    public uint AutoKeys { get; private set; }
    /// <summary>Remaining frames the 125 injection stays available (0/1/2).</summary>
    private int _autoKeysTtl;
    /// <summary>Coroutines that already consumed the current 125 injection.</summary>
    private readonly HashSet<int> _autoKeysDelivered = new();
    /// <summary>126 state 1: these keys stop walking but still reach 123.</summary>
    public uint BasicMoveDeny { get; private set; }
    /// <summary>126 state 2: these keys stop both walking and 123.</summary>
    public uint BasicAllDeny { get; private set; }
    /// <summary>Dedupe key for the reserved sound-type warning.</summary>
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
        uint rawTarget = raw & InputTargetMask;
        uint state = (raw >> 8) & 0x3;
        if ((raw & InputDeviceFlag) != 0)
        {
            if (rawTarget != 0) DeviceDeny = state != 0;
            return;
        }
        uint target = KeyCodes.FromWolfBits(rawTarget);
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
        _autoKeysTtl = 0;
        _autoKeysDelivered.Clear();
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
        if (DeviceDeny && (bits & KeyCodes.FromWolfBits(AutoKeys)) == 0) return 0;
        return code;
    }
    private int _nextInstanceId = 1;
    public bool EndGameRequested { get; set; }
    public bool ReturnToTitleRequested { get; set; }
    private readonly Dictionary<string, int> _titleToId = new(StringComparer.Ordinal);
    private readonly HashSet<int> _enteredInstanceIds = new();
    private readonly HashSet<int> _budgetExhausted = new();

    // Transition setting installed by opcode 160 and consumed by 161/162
    // (Rust crates/wolf-vm/src/exec.rs:910-937).
    private int _transitionKind;
    private int _transitionFlags;
    private bool _transitionPrepared;
    private int _runCursor;

    /// <summary>True between 161 PrepareTransition and 162 ExecuteTransition.</summary>
    public bool TransitionPrepared => _transitionPrepared;

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

    public Instance? CreateInstance(Program program, int? parentId = null, int? returnTo = null,
        SelfBankPolicy selfPolicy = SelfBankPolicy.PersistentAll)
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
            ReturnTo = returnTo,
            SelfPolicy = selfPolicy
        };
        if (selfPolicy != SelfBankPolicy.Ephemeral
            && program.SelfBase == Program.CommonSelfBase
            && CommonSelf.TryGetValue(program.Id, out var bank))
        {
            inst.Self.CopyFrom(bank);
        }
        Instances.Add(inst);
        return inst;
    }

    public Instance? CreateInstance(int eventId, int? parentId = null, int? returnTo = null,
        SelfBankPolicy selfPolicy = SelfBankPolicy.PersistentAll)
    {
        var prog = GetProgram(eventId);
        if (prog == null)
        {
            RejectedSpawnCount++;
            Host.Log($"unknown common event id {eventId}; call dropped");
            return null;
        }
        return CreateInstance(prog, parentId, returnTo, selfPolicy);
    }

    /// <summary>
    /// Spawn a synthetic event with a blank self bank and no persistence write-back
    /// (exec.rs start_event_fresh). CE241/CE242 have no script-issued 210 call, so their
    /// per-invocation state must not leak through the common-event store.
    /// </summary>
    public Instance? CreateFreshInstance(int eventId, int? parentId = null, int? returnTo = null) =>
        CreateInstance(eventId, parentId, returnTo, SelfBankPolicy.Ephemeral);

    /// <summary>Start a common event with 211-style call arguments bound before its first slot.</summary>
    public Instance? StartEventWithArgs(int eventId, int[] args, string[] strings, bool parallel = false)
    {
        var inst = CreateInstance(eventId);
        if (inst == null) return null;
        inst.IsParallel = parallel;
        for (int i = 0; i < args.Length; i++)
        {
            inst.Self.Set(Magic.SelfArgBase + i, args[i]);
        }
        for (int k = 0; k < strings.Length; k++)
        {
            inst.Self.SetStr(Magic.SelfArgBase + 5 + k, strings[k] ?? "");
        }
        return inst;
    }

    /// <summary>exec.rs run_instances: last live foreground event, +500000 for common events.</summary>
    private int? ForegroundEventId()
    {
        for (int i = Instances.Count - 1; i >= 0; i--)
        {
            var other = Instances[i];
            if (other.IsParallel) continue;
            if (other.Pc >= other.Program.Len && other.Wait.Kind == WaitStateKind.None) continue;
            return other.Program.SelfBase == Program.CommonSelfBase ? other.EventId + 500_000 : other.EventId;
        }
        return null;
    }

    /// <summary>exec.rs foreground_busy: a non-parallel instance that has not finished or is blocked.</summary>
    public bool ForegroundBusy()
    {
        foreach (var inst in Instances)
        {
            if (inst.IsParallel) continue;
            if (inst.Pc < inst.Program.Len || inst.Wait.Kind != WaitStateKind.None) return true;
        }
        return false;
    }

    /// <summary>Common-event call addresses: 500000+ absolute, 600100+ caller-relative in live mode.</summary>
    public static int? ResolveCallTarget(int raw, int callerEventId, bool relative600 = false)
    {
        if (raw >= 500_000 && raw < 600_000) return raw - 500_000;
        if (raw >= 600_000 && raw < 700_000)
        {
            if (relative600)
            {
                int resolved = callerEventId + (raw - 600_100);
                return resolved >= 0 ? resolved : null;
            }
            return raw - 600_000;
        }
        return raw >= 0 ? raw : null;
    }

    /// <summary>
    /// exec.rs store_common_self: fold a finished/stopped common event's bank back
    /// according to its <see cref="SelfBankPolicy"/>. A running instance never sees
    /// another instance's writes.
    /// </summary>
    private void PersistCommonSelf(Instance inst, bool isForcedStop)
    {
        if (inst.Program.SelfBase != Program.CommonSelfBase) return;
        if (inst.SelfPolicy == SelfBankPolicy.Ephemeral) return;
        if (isForcedStop && inst.SelfPolicy == SelfBankPolicy.PersistentOnFinishOnly) return;
        if (CoreRuntimeConfig.TraceAutoEvents)
        {
            for (int i = 0; i < Instances.Count; i++)
            {
                var other = Instances[i];
                if (other.Id != inst.Id
                    && other.EventId == inst.EventId
                    && other.Program.SelfBase == Program.CommonSelfBase
                    && other.SelfPolicy != SelfBankPolicy.Ephemeral)
                {
                    Console.Error.WriteLine(
                        $"[EventVm] concurrent CommonSelf write on event {inst.EventId}: persisting instance {inst.Id} while instance {other.Id} is still active");
                    break;
                }
            }
        }
        CommonSelf[inst.EventId] = inst.Self.Clone();
    }

    /// <summary>Decode the operand table shared by 210 and 300 (exec.rs call_arguments).</summary>
    private (int[] Args, string[] Strings, int? ReturnTo) DecodeCallArguments(Slot slot, Instance inst)
    {
        int status = slot.Params.Length > 1 ? slot.Params[1] : 0;
        int numCount = status & 0x0F;
        int rawStrCount = (status >> 4) & 0x0F;
        int strCount = Math.Min(rawStrCount, 4);
        bool hasRet = ((status >> 24) & 1) != 0;

        var args = new int[numCount];
        for (int i = 0; i < numCount; i++)
        {
            args[i] = EvalNum(2 + i < slot.Params.Length ? slot.Params[2 + i] : 0, inst);
        }

        var strings = new string[strCount];
        for (int k = 0; k < strCount; k++)
        {
            bool literal = ((status >> (12 + k)) & 1) != 0;
            int wordIndex = 2 + numCount + k;
            int word = wordIndex < slot.Params.Length ? slot.Params[wordIndex] : 0;
            strings[k] = (literal || word == 0
                ? (1 + k < slot.Strings.Length ? slot.Strings[1 + k] : "")
                : EvalStr(word, inst)) ?? "";
        }

        int retIndex = 2 + numCount + rawStrCount;
        int? returnTo = hasRet && retIndex < slot.Params.Length ? slot.Params[retIndex] : null;
        return (args, strings, returnTo);
    }

    public string Expand(string text, Instance inst)
    {
        return TextExpander.Expand(text, inst, Vars, ReadDbText);
    }

    /// <summary>Database cell text for \cdb/\sdb (CData) and \udb (Data) escapes.</summary>
    private string ReadDbText(int kind, int dbType, int row, int column)
        => Host.DbText(kind, dbType, row, column);

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
            MagicKind.SysVar => Vars.GetSysStr(magic.IsSysStr ? (int)(magic.Index - Magic.SysStrOffset) : (int)magic.Index),
            MagicKind.Literal => magic.Index.ToString(),
            MagicKind.Var => Vars.Get((int)magic.Index).ToString(),
            _ => ""
        };
    }

    public void StoreStr(Instance inst, int raw, string value)
    {
        value ??= "";
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
                Vars.SetSysStr(magic.IsSysStr ? (int)(magic.Index - Magic.SysStrOffset) : (int)magic.Index, value);
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
                0x4 => b == 0 ? a : (a == int.MinValue && b == -1 ? 0 : a % b),
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
                0x500 => value == 0 ? current : (current == int.MinValue && value == -1 ? 0 : current % value),
                // exec.rs compound(): 0x600/0x700 are unary transforms that leave the
                // target's current value alone, never lower/upper bounds of the operand.
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

    /// <summary>121 SetVariable: low flag bits control literal/pointer addressing (0x04 literal a, 0x08 literal b, 0x10 indirect target, 0x20 indirect a).</summary>
    public void ExecuteSetVariable(int[] args, Instance inst)
    {
        if (args.Length < 4) return;
        uint bits = (uint)args[3];
        int target = (bits & 0x10) != 0 ? EvalNum(args[0], inst) : args[0];
        int a = (bits & 0x04) != 0
            ? args[1]
            : ((bits & 0x20) != 0 ? EvalNum(EvalNum(args[1], inst), inst) : EvalNum(args[1], inst));
        int b = (bits & 0x08) != 0 ? args[2] : EvalNum(args[2], inst);
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
                    if ((uint)ctrl.BlockIndex >= (uint)prog.Blocks.Count)
                    {
                        inst.Pc++;
                        break;
                    }
                    var block = prog.Blocks[ctrl.BlockIndex];
                    switch (block.Kind)
                    {
                        case BlockKind.Condition:
                        {
                            int head = slot.Params.Length > 0 ? slot.Params[0] : 0;
                            if (slot.Opcode == 112)
                            {
                                // 112 StringCondition: high nibble of Params[1+i] selects the operator
                                // (0=eq, 1=ne, 2=contains, 3=startsWith), bit 24 (0x0100_0000) reads
                                // RHS from trailing Params[1+strCount+rhsOffset] instead of Strings[i],
                                // and bit 0x10 of head selects the else branch.
                                int strCount = head & 0x0F;
                                bool useElse = (head & 0x10) != 0;
                                int? matchedStr = null;
                                int rhsOffset = 0;
                                for (int i = 0; i < strCount; i++)
                                {
                                    int varArg = slot.Params.Length > 1 + i ? slot.Params[1 + i] : 0;
                                    int opNibble = (int)(((uint)varArg >> 28) & 0x0F);
                                    bool rhsFromVar = (varArg & 0x0100_0000) != 0;
                                    string lhs = EvalStr(varArg & 0x00FF_FFFF, inst);
                                    string rhs;
                                    if (rhsFromVar)
                                    {
                                        int rhsIdx = 1 + strCount + rhsOffset;
                                        rhsOffset++;
                                        int rhsRaw = rhsIdx < slot.Params.Length ? slot.Params[rhsIdx] : 0;
                                        rhs = EvalStr(rhsRaw & 0x00FF_FFFF, inst);
                                    }
                                    else
                                    {
                                        rhs = Expand(i < slot.Strings.Length ? slot.Strings[i] : "", inst);
                                    }

                                    bool matchedOp = opNibble switch
                                    {
                                        0 => lhs == rhs,
                                        1 => lhs != rhs,
                                        2 => lhs.Contains(rhs, StringComparison.Ordinal),
                                        3 => lhs.StartsWith(rhs, StringComparison.Ordinal),
                                        _ => lhs == rhs,
                                    };
                                    if (matchedOp)
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
                                if (3 + 3 * i >= slot.Params.Length) break;
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
                            bool allowCancel = slot.Params.Length == 0 || (slot.Params[0] & 0x10) == 0;
                            Host.ShowChoices(options, allowCancel);
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
                    if ((uint)ctrl.BlockIndex >= (uint)prog.Blocks.Count)
                    {
                        inst.Pc++;
                        continue;
                    }
                    var block = prog.Blocks[ctrl.BlockIndex];
                    inst.Pc = block.EndTarget(prog.Len);
                    continue;
                }
                case ControlKind.LoopEnd:
                {
                    // Reached loop end -> decrement and loop or exit
                    CloseLoop(prog, inst, ctrl.BlockIndex);
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
                    while (inst.Loops.Count > 0)
                    {
                        int loopBlockIdx = inst.Loops[^1].Block;
                        if ((uint)loopBlockIdx < (uint)prog.Blocks.Count)
                        {
                            var loopBlock = prog.Blocks[loopBlockIdx];
                            if (inst.Pc > loopBlock.Head && inst.Pc < loopBlock.EndTarget(prog.Len))
                                break;
                        }
                        inst.Loops.RemoveAt(inst.Loops.Count - 1);
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
                        int end = 1;
                        while (end < expanded.Length && char.IsAsciiDigit(expanded[end]))
                        {
                            end++;
                        }
                        if (int.TryParse(expanded.AsSpan(1, end - 1), out int parsedFace))
                        {
                            face = parsedFace;
                        }
                        messageText = end < expanded.Length ? expanded[end..] : "";
                    }
                    bool pairedChoice = inst.Pc + 1 < prog.Len && prog.Slots[inst.Pc + 1].Opcode == 102;
                    Host.ShowMessage(messageText, face, pairedChoice);
                    if (pairedChoice)
                    {
                        inst.Pc++;
                        break;
                    }
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
                    bool allowCancel = slot.Params.Length == 0 || (slot.Params[0] & 0x10) == 0;
                    Host.ShowChoices(options, allowCancel);
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
                    int rawTarget = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int mode = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int source = slot.Params.Length > 2 ? slot.Params[2] : 0;
                    int target = ResolveStringTarget(rawTarget, mode, inst);
                    int operation = mode & 0xFF00;
                    int sourceKind = mode & 0x0F;

                    string Literal(int idx) =>
                        idx < slot.Strings.Length ? Expand(slot.Strings[idx], inst) : "";

                    int? stringSource = null;
                    if (slot.Params.Length >= 3)
                    {
                        if (sourceKind == 1)
                        {
                            stringSource = source;
                        }
                        else if (sourceKind == 2)
                        {
                            int address = EvalNum(source, inst);
                            var kind = Magic.Decode(address).Kind;
                            if (kind is MagicKind.StrVar or MagicKind.SysVar or MagicKind.SelfVar)
                                stringSource = address;
                        }
                    }

                    string ReadSource() =>
                        stringSource.HasValue ? EvalStr(stringSource.Value, inst) : Literal(0);

                    void WriteBack(string value)
                    {
                        if (stringSource.HasValue)
                            StoreStr(inst, stringSource.Value, value);
                    }

                    switch (operation)
                    {
                        case 0x0000:
                            StoreStr(inst, target, ReadSource());
                            break;
                        case 0x0100:
                            StoreStr(inst, target, EvalStr(target, inst) + ReadSource());
                            break;
                        case 0x0200:
                        {
                            SplitFirstLine(ReadSource(), out string line, out string rest);
                            StoreStr(inst, target, line);
                            WriteBack(rest);
                            break;
                        }
                        case 0x0300:
                        {
                            SplitFirstLine(ReadSource(), out string line, out _);
                            StoreStr(inst, target, line);
                            break;
                        }
                        case 0x0400:
                        {
                            string text = ReadSource();
                            int count = FirstUnitLen(text);
                            StoreStr(inst, target, text.Substring(0, count));
                            WriteBack(text.Substring(count));
                            break;
                        }
                        case 0x0500:
                        {
                            if (sourceKind == 0)
                            {
                                string text = Literal(0);
                                const string FileExistPrefix = "<<GET_FILE_EXIST>>";
                                if (text.StartsWith(FileExistPrefix, StringComparison.Ordinal))
                                {
                                    string query = text.Substring(FileExistPrefix.Length);
                                    StoreStr(inst, target, Host.FileExists(query) ? "1" : "0");
                                    break;
                                }
                                int at = LastUnitStart(text);
                                StoreStr(inst, target, text.Substring(at));
                            }
                            else
                            {
                                string text = ReadSource();
                                int at = LastUnitStart(text);
                                StoreStr(inst, target, text.Substring(at));
                                WriteBack(text.Substring(0, at));
                            }
                            break;
                        }
                        case 0x0800:
                        {
                            string needle = Literal(0);
                            string text = EvalStr(target, inst);
                            if (!string.IsNullOrEmpty(needle))
                                text = text.Replace(needle, string.Empty);
                            StoreStr(inst, target, text);
                            break;
                        }
                        case 0x0900:
                        {
                            string from = Literal(0);
                            string to = Literal(1);
                            string text = EvalStr(target, inst);
                            string replaced = string.IsNullOrEmpty(from) ? text : text.Replace(from, to);
                            StoreStr(inst, target, replaced);
                            break;
                        }
                        case 0x0A00:
                        {
                            string marker = Literal(0);
                            string text = EvalStr(target, inst);
                            int idx = string.IsNullOrEmpty(marker)
                                ? -1
                                : text.IndexOf(marker, StringComparison.Ordinal);
                            StoreStr(inst, target, idx >= 0 ? text.Substring(0, idx) : "<<NotFound>>");
                            break;
                        }
                        case 0x0B00:
                        {
                            string marker = Literal(0);
                            string text = EvalStr(target, inst);
                            int idx = string.IsNullOrEmpty(marker)
                                ? -1
                                : text.IndexOf(marker, StringComparison.Ordinal);
                            StoreStr(inst, target, idx >= 0 ? text.Substring(idx + marker.Length) : "<<NotFound>>");
                            break;
                        }
                        default:
                            StoreStr(inst, target, ReadSource());
                            break;
                    }
                    inst.Pc++;
                    break;
                }
                case 123:
                {
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    // The mask is a composite UI selector, not a set of allowed keys;
                    // 126 is applied to the key code the host answers with.
                    uint mask = slot.Params.Length > 1 ? (uint)slot.Params[1] : 0;
                    bool blocking = BlockOnKeyWait && (mask & 0x80) != 0;
                    bool autoAvailable = AutoKeys != 0 && !_autoKeysDelivered.Contains(inst.Id);
                    bool autoConsumed = autoAvailable && KeyCodes.AcceptsWolfBits(mask, AutoKeys);
                    uint effectiveAuto = autoAvailable ? AutoKeys : 0;
                    int key = FilterKeyCode(Host.KeyCodeAuto(mask, effectiveAuto));
                    if (blocking && !(autoConsumed && key != 0))
                    {
                        inst.Wait.WaitForKeyInput(target, mask, requireRelease: key != 0);
                        inst.Pc++;
                        return Signal.Blocked;
                    }
                    if (autoConsumed) _autoKeysDelivered.Add(inst.Id);
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
                        0x3000 => field switch
                        {
                            // exec.rs 2887-2901: field 17 is the running common-event id
                            // (-1 for a map event); field 18 prefers the host's foreground
                            // answer, then the last foreground coroutine, then this instance.
                            17 => inst.Program.SelfBase == Program.CommonSelfBase ? inst.EventId : -1,
                            18 => Host.EngineState(18) ?? ForegroundEventId() ??
                                (inst.Program.SelfBase == Program.CommonSelfBase ? inst.EventId + 500_000 : inst.EventId),
                            _ => Host.EngineState(field),
                        },
                        0x1000 or 0x1200 => Host.CharacterState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                        0x4000 or 0x4400 => Host.PictureState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                        _ => null
                    };
                    if (val.HasValue)
                    {
                        int storeVal = (sub & 0xF000) == 0x4000
                            ? Compound(sub & 0x0F00, EvalNum(target, inst), val.Value)
                            : val.Value;
                        StoreNum(inst, target, storeVal);
                    }
                    inst.Pc++;
                    break;
                }
                case 176:
                    // exec.rs 3150-3158: ContinueLoop closes the innermost loop. It is
                    // not a block head (program.rs 293), so Program must not push a
                    // frame here; a hand-made Program without one just advances.
                    if (inst.Loops.Count == 0)
                    {
                        Host.Log($"[EventVm] continue-loop 176 without a loop at event {inst.EventId} pc {inst.Pc}");
                        inst.Pc++;
                        break;
                    }
                    CloseLoop(prog, inst, inst.Loops[^1].Block);
                    break;
                case 130:
                {
                    // exec.rs 130: raw layout is target, x, y, map, options; none of
                    // these are dereferenced.
                    int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int x = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    int y = slot.Params.Length > 2 ? slot.Params[2] : 0;
                    int map = slot.Params.Length > 3 ? slot.Params[3] : 0;
                    int options = slot.Params.Length > 4 ? slot.Params[4] : 0;
                    Host.Teleport(target, map, x, y, options);
                    inst.Wait.WaitForFrames(1);
                    inst.Pc++;
                    return Signal.Blocked;
                }
                case 140:
                {
                    int bits = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    // Bits 4..6 select channel: 0=BGM, 1=BGS, 2=SE; 0x20 alone misclassifies BGS as BGM.
                    int soundType = (bits >> 4) & 0x07;
                    if (soundType > 2 && _unknownSoundTypeLogs.Add((inst.EventId, inst.Pc, soundType)))
                    {
                        // 0/1/2 are BGM/BGS/SE; 3..7 are reserved and fall back to BGM once per site.
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
                        string expanded = Expand(rawStr, inst);
                        (string cellFile, int? cellVolume, int? cellTempo) = SplitSoundCell(expanded);
                        int rawFrames = slot.Params.Length >= 6 ? Math.Max(0, EvalNum(slot.Params[1], inst)) : 0;
                        int rawVol = slot.Params.Length >= 6 ? EvalNum(slot.Params[4], inst) : 100;
                        int rawTempo = slot.Params.Length >= 6 ? EvalNum(slot.Params[5], inst) : 100;
                        file = cellFile;
                        volume = rawVol > 0 || cellVolume == null ? rawVol : cellVolume.Value;
                        frames = rawFrames;
                        tempo = rawTempo > 0 || cellTempo == null ? rawTempo : cellTempo.Value;
                        loopPoint = slot.Params.Length >= 7 ? Math.Max(0, EvalNum(slot.Params[6], inst)) : 0;
                        action = SoundAction.Play;
                    }
                    else
                    {
                        file = "";
                        volume = 0;
                        frames = slot.Params.Length <= 5 && slot.Params.Length > 1 ? slot.Params[1] : 0;
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
                    byte blendMode = (byte)((head >> 8) & 0x0F);
                    byte anchor = (byte)((head >> 12) & 0x07);
                    PictureAction action = (head & 0x0F) switch
                    {
                        0 => PictureAction.Draw,
                        1 => PictureAction.Move,
                        2 => PictureAction.Erase,
                        // Retail WOLF calls wire 3 "Delay Reset"; the legacy enum name
                        // is retained so existing trace fixtures stay byte-compatible.
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
                                if (magic.Kind is MagicKind.SelfVar or MagicKind.StrVar or MagicKind.SysVar)
                                    file = Expand(EvalStr(slot.Params[11], inst), inst);
                            }
                            break;
                    }

                    int argsCount = Math.Max(0, slot.Params.Length - 2);
                    int[] args = new int[argsCount];
                    for (int i = 0; i < argsCount; i++)
                    {
                        int raw = slot.Params[2 + i];
                        int body = raw & 0x0FFF_FFFF;
                        // Only a flagged packed value whose body would otherwise decode as a
                        // self/var/string address must stay raw. Negative picture sentinels,
                        // packed RGB colors and opaque DB magic are all safe through EvalNum.
                        if (((uint)raw & 0x0F00_0000) != 0 && body >= Magic.SelfBase && body < Magic.StrBase)
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
                        Animate = animate,
                        ScaleMode = (byte)((head >> 20) & 0x0F),
                        BlendMode = blendMode,
                        KeepShift = (head & 0x0040_0000) != 0,
                        Relative = (head & 0x0001_0000) != 0,
                        HasSpan = (head & 0x0100_0000) != 0
                    };

                    Host.Picture(cmd);
                    inst.Pc++;
                    break;
                }
                case 160 or 161 or 162:
                {
                    // Rust exec.rs:2354-2383 keeps one TransitionState for the whole game:
                    // 160 installs the setting, 161 freezes the drawn screen, and 162 runs
                    // the fade and blocks for the low byte of flags (zero means one frame).
                    int kind = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int flags = slot.Params.Length > 1 ? slot.Params[1] : 0;
                    if (slot.Opcode == 160)
                    {
                        _transitionKind = kind;
                        _transitionFlags = flags;
                        TransitionPhase = 0;
                        Host.Transition(kind, flags);
                        inst.Pc++;
                        break;
                    }
                    if (slot.Opcode == 161)
                    {
                        _transitionPrepared = true;
                        TransitionPhase = 1;
                        Host.Transition(_transitionKind, _transitionFlags);
                        inst.Pc++;
                        break;
                    }
                    _transitionPrepared = false;
                    int frames = Math.Max(1, _transitionFlags & 0xFF);
                    TransitionPhase = 2;
                    Host.Transition(_transitionKind, _transitionFlags);
                    inst.Wait.WaitForFrames((uint)frames);
                    inst.Pc++;
                    // Blocking immediately after arming the wait prevents post-fade instructions
                    // from advancing in the same frame.
                    return Signal.Blocked;
                }
                case 171:
                {
                    if (inst.Loops.Count > 0)
                    {
                        var top = inst.Loops[^1];
                        inst.Loops.RemoveAt(inst.Loops.Count - 1);
                        inst.Pc = (uint)top.Block < (uint)prog.Blocks.Count
                            ? prog.Blocks[top.Block].EndTarget(prog.Len)
                            : prog.Len;
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
                    int rawTarget = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    if (Magic.Decode(rawTarget).IsStorage)
                    {
                        rawTarget = EvalNum(rawTarget, inst);
                    }
                    int? targetEvent = ResolveCallTarget(rawTarget, inst.EventId, RelativeCommonCalls);
                    if (targetEvent == null)
                    {
                        // exec.rs 210: only the two call-address ranges are valid targets.
                        RejectedSpawnCount++;
                        Host.Log($"210 CallCommonEvent: unsupported target {rawTarget}");
                        inst.Pc++;
                        break;
                    }
                    if (ShouldSkipCall != null && ShouldSkipCall(inst, targetEvent.Value))
                    {
                        inst.Pc++;
                        break;
                    }

                    var (childArgs, childStrings, returnTo) = DecodeCallArguments(slot, inst);

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
                    child.IsParallel = inst.IsParallel;
                    for (int i = 0; i < childArgs.Length; i++)
                    {
                        child.Self.Set(Magic.SelfArgBase + i, childArgs[i]);
                    }
                    for (int k = 0; k < childStrings.Length; k++)
                    {
                        child.Self.SetStr(Magic.SelfArgBase + 5 + k, childStrings[k] ?? "");
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
                    // Operand order per docs/engine/OPCODE_220_SAVE.md: the file is arg1, the
                    // mode (220) and destination address (221) are arg0, the address is
                    // arg2 for 221/222, and the trailing flag is arg3 when present.
                    byte opc = (byte)slot.Opcode;
                    int argc = slot.Params.Length;
                    int fileRaw = argc > 1 ? slot.Params[1] : 0;
                    var fileMagic = Magic.Decode(fileRaw);
                    SaveTarget target = fileMagic.IsStringSlot
                        ? new SaveTarget(EvalStr(fileRaw, inst))
                        : new SaveTarget(EvalNum(fileRaw, inst));
                    int mode = opc == 220 && argc > 0 ? EvalNum(slot.Params[0], inst) : 0;
                    int dest = opc == 221 && argc > 0 ? slot.Params[0] : 0;
                    int flag = argc >= 4 ? EvalNum(slot.Params[3], inst) : 0;
                    int addr = opc == 220 || argc <= 2
                        ? 0
                        : (flag == 1 ? EvalNum(slot.Params[2], inst) : slot.Params[2]);
                    bool isStr222 = opc == 222 && argc > 0
                        && (Magic.Decode(slot.Params[0]).IsStringSlot || Magic.Decode(addr).IsStringSlot);
                    int val = opc == 222 && argc > 0 && !isStr222 ? EvalNum(slot.Params[0], inst) : 0;
                    string? strVal = isStr222 ? EvalStr(slot.Params[0], inst) : null;

                    var saveOp = new SaveOp
                    {
                        Opcode = opc,
                        Mode = mode,
                        File = target,
                        Destination = dest,
                        Address = addr,
                        Value = val,
                        StringValue = strVal,
                        IsString = isStr222,
                        Flag = flag,
                        Owner = inst.EventId
                    };
                    int? loaded = Host.SaveLoad(saveOp);
                    if (saveOp.LoadedString != null)
                    {
                        StoreStr(inst, dest, saveOp.LoadedString);
                    }
                    else if (loaded.HasValue)
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
                    int reserveTarget = ResolveCallTarget(rawReserve, inst.EventId, RelativeCommonCalls)
                        ?? Math.Max(0, rawReserve);
                    var (reserveArgs, reserveStrings, _) = DecodeCallArguments(slot, inst);
                    Host.Reserve(reserveTarget, reserveArgs, reserveStrings);
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
                            if (valMagic.IsStringSlot)
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
                {
                    uint bits = slot.Params.Length > 0 ? (uint)slot.Params[0] & 0xFF : 0;
                    AutoKeys = bits;
                    _autoKeysTtl = bits != 0 ? 2 : 0;
                    _autoKeysDelivered.Clear();
                    inst.Pc++;
                    break;
                }
                case 126:
                    ApplyInputPermission(slot.Params.Length > 0 ? (uint)slot.Params[0] : 0);
                    inst.Pc++;
                    break;
                case 151:
                {
                    int color = slot.Params.Length > 0 ? slot.Params[0] : 0;
                    int frames = slot.Params.Length > 1 ? EvalNum(slot.Params[1], inst) : 0;
                    Host.ChangeColor(color, frames);
                    inst.Pc++;
                    break;
                }
                case 174:
                    ReturnToTitleRequested = true;
                    ResetTransitionState();
                    _enteredInstanceIds.Clear();
                    _autoKeysDelivered.Clear();
                    _runCursor = 0;
                    Instances.Clear();
                    return Signal.ReturnToTitle;
                case 175:
                    EndGameRequested = true;
                    Host.EndGame();
                    ResetTransitionState();
                    _enteredInstanceIds.Clear();
                    _autoKeysDelivered.Clear();
                    _runCursor = 0;
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
                    child.IsParallel = inst.IsParallel;
                    // Both 210 and 300 dispatch a common event, so notify the host for both.
                    Host.CommonEventCall(callee.Id, callArgs, callStrings);
                    for (int i = 0; i < callArgs.Length; i++)
                    {
                        child.Self.Set(Magic.SelfArgBase + i, callArgs[i]);
                    }
                    for (int k = 0; k < callStrings.Length; k++)
                    {
                        child.Self.SetStr(Magic.SelfArgBase + 5 + k, callStrings[k] ?? "");
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
                    string path = slot.Strings.Length > 0 ? Expand(slot.Strings[0], inst) : "";
                    Host.ProFeature(kind, milliseconds, path);
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
        for (int i = 0; i < Instances.Count; i++)
        {
            Instances[i].JustResumedFromChild = false;
        }
        PollWaits();
        int budget = 200_000;
        if (!ResumeOnChildReturn)
        {
            RunInstances(ref budget);
            ReapFinished();
        }
        else
        {
            if (_budgetExhausted.Count > 0) _budgetExhausted.Clear();
            for (int pass = 0; pass < 16 && budget > 0; pass++)
            {
                RunInstances(ref budget, _budgetExhausted);
                int woken = ReapFinished();
                if (woken == 0 || ReturnToTitleRequested || EndGameRequested)
                {
                    break;
                }
            }
        }
        // 125 auto input lives for up to two frames, and each coroutine may
        // consume it once per injection (exec.rs step_frame).
        if (_autoKeysTtl > 0)
        {
            _autoKeysTtl--;
            if (_autoKeysTtl == 0)
            {
                AutoKeys = 0;
                _autoKeysDelivered.Clear();
            }
        }
        else
        {
            AutoKeys = 0;
            _autoKeysDelivered.Clear();
        }
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
                    if ((uint)inst.Wait.Block >= (uint)inst.Program.Blocks.Count)
                    {
                        inst.Pc = inst.Program.Len;
                        inst.Wait.Clear();
                        continue;
                    }
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
            else if (inst.Wait.Kind == WaitStateKind.KeyInput)
            {
                bool autoAvailable = AutoKeys != 0 && !_autoKeysDelivered.Contains(inst.Id);
                bool autoConsumed = autoAvailable && KeyCodes.AcceptsWolfBits(inst.Wait.KeyMask, AutoKeys);
                uint effectiveAuto = autoAvailable ? AutoKeys : 0;
                int key = FilterKeyCode(Host.KeyCodeAuto(inst.Wait.KeyMask, effectiveAuto));
                if (inst.Wait.KeyRequireRelease && !(autoConsumed && key != 0))
                {
                    if (key == 0) inst.Wait.KeyRequireRelease = false;
                }
                else if (key != 0)
                {
                    if (autoConsumed) _autoKeysDelivered.Add(inst.Id);
                    StoreNum(inst, inst.Wait.KeyTarget, key);
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
            if (doomed.Contains(other.Id)) PersistCommonSelf(other, isForcedStop: true);
        }
        foreach (var other in Instances)
        {
            if (doomed.Contains(other.Id))
            {
                _enteredInstanceIds.Remove(other.Id);
                _autoKeysDelivered.Remove(other.Id);
            }
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
        EndGameRequested = false;
        ReturnToTitleRequested = false;
        ResetTransitionState();
        _runCursor = 0;
        _enteredInstanceIds.Clear();
        _autoKeysDelivered.Clear();
        Instances.Clear();
    }

    private void ResetTransitionState()
    {
        _transitionKind = 0;
        _transitionFlags = 0;
        _transitionPrepared = false;
        TransitionPhase = 0;
    }

    /// <summary>Shared 498/176 logic: count down or jump back to the matching loop head.</summary>
    private static void CloseLoop(Program prog, Instance inst, int blockIdx)
    {
        int loopPos = -1;
        for (int i = inst.Loops.Count - 1; i >= 0; i--)
        {
            if (inst.Loops[i].Block == blockIdx)
            {
                loopPos = i;
                break;
            }
        }
        if (loopPos < 0)
        {
            inst.Pc++;
            return;
        }
        if ((uint)blockIdx >= (uint)prog.Blocks.Count)
        {
            inst.Loops.RemoveRange(loopPos, inst.Loops.Count - loopPos);
            inst.Pc++;
            return;
        }

        // LoopCtx is a struct: mutate the list element itself, never a copy,
        // or a counted loop never reaches its last iteration.
        long rem = inst.Loops[loopPos].Remaining;
        bool again;
        if (rem < 0)
        {
            again = true; // 170 unconditional loop head
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

    public int EnteredInstanceCount => _enteredInstanceIds.Count;
    internal int AutoKeysDeliveredCount => _autoKeysDelivered.Count;
    public const int PerInstanceFrameBudget = 50_000;

    public void RunInstances(ref int budget, HashSet<int>? budgetExhausted = null)
    {
        if (Instances.Count == 0)
        {
            _runCursor = 0;
            return;
        }
        int start = _runCursor % Instances.Count;
        bool stopped = false;

        bool StepAt(int index, ref int totalBudget)
        {
            if (ReturnToTitleRequested || EndGameRequested)
            {
                _runCursor = 0;
                stopped = true;
                return false;
            }
            if (totalBudget <= 0)
            {
                _runCursor = index;
                stopped = true;
                return false;
            }
            var inst = Instances[index];
            if (inst.IsRunnable)
            {
                if (budgetExhausted != null && budgetExhausted.Contains(inst.Id))
                {
                    return true;
                }
                if (inst.IsParallel && inst.Pc == 0 && CanStepParallelInstance != null && !CanStepParallelInstance(inst))
                {
                    return true;
                }
                if (!inst.IsParallel && _enteredInstanceIds.Add(inst.Id))
                {
                    Host.EventEntered(inst.EventId, !inst.ParentId.HasValue);
                }
                int instBudget = Math.Min(totalBudget, PerInstanceFrameBudget);
                int rem = instBudget;
                StepInstance(inst, ref rem);
                int consumed = instBudget - rem;
                totalBudget -= consumed;
                if (rem <= 0 && consumed >= PerInstanceFrameBudget && inst.IsRunnable)
                {
                    budgetExhausted?.Add(inst.Id);
                    Host.Log($"Event {inst.EventId} (pc={inst.Pc}) hit per-instance frame budget ({PerInstanceFrameBudget})");
                }
            }
            return true;
        }

        for (int i = start; i < Instances.Count; i++)
        {
            if (!StepAt(i, ref budget)) break;
        }
        if (!stopped)
        {
            for (int i = 0; i < start && i < Instances.Count; i++)
            {
                if (!StepAt(i, ref budget)) break;
            }
        }
        if (!stopped)
        {
            _runCursor = 0;
        }
    }

    public int ReapFinished()
    {
        int woken = 0;
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

                // exec.rs reap_finished: capture the bank before dropping it.
                PersistCommonSelf(inst, isForcedStop: false);
                _enteredInstanceIds.Remove(inst.Id);
                OnInstanceFinished?.Invoke(inst);

                // Wake parent
                if (inst.ParentId.HasValue)
                {
                    var parent = Instances.Find(p => p.Id == inst.ParentId.Value);
                    if (parent != null && parent.Wait.Kind == WaitStateKind.Child && parent.Wait.ChildId == inst.Id)
                    {
                        parent.Wait.Clear();
                        parent.JustResumedFromChild = true;
                        woken++;
                    }
                }

                Instances.RemoveAt(i);
            }
        }
        return woken;
    }

    private int ResolveStringTarget(int raw, int mode, Instance inst)
    {
        if ((mode & 0x0010) != 0)
        {
            int address = EvalNum(raw, inst);
            var kind = Magic.Decode(address).Kind;
            if (kind is MagicKind.StrVar or MagicKind.SysVar or MagicKind.SelfVar)
                return address;
        }
        return raw;
    }

    private static void SplitFirstLine(string text, out string line, out string rest)
    {
        int nl = text.IndexOf('\n');
        if (nl < 0)
        {
            line = text.EndsWith('\r') ? text.Substring(0, text.Length - 1) : text;
            rest = string.Empty;
            return;
        }
        line = nl > 0 && text[nl - 1] == '\r' ? text.Substring(0, nl - 1) : text.Substring(0, nl);
        rest = text.Substring(nl + 1);
    }

    private static (string File, int? Volume, int? Tempo) SplitSoundCell(string raw)
    {
        int nl = raw.IndexOf('\n');
        if (nl < 0)
            return (raw.TrimEnd('\r'), null, null);
        string[] lines = raw.Replace("\r\n", "\n").Split('\n');
        string file = lines[0].TrimEnd();
        int? vol = lines.Length > 1 && int.TryParse(lines[1].Trim(), out int v) ? v : null;
        int? tempo = lines.Length > 2 && int.TryParse(lines[2].Trim(), out int t) ? t : null;
        return (file, vol, tempo);
    }

    private static int FirstUnitLen(ReadOnlySpan<char> text)
    {
        if (text.IsEmpty)
            return 0;
        if (text[0] != '\\')
            return char.IsHighSurrogate(text[0]) && text.Length > 1 && char.IsLowSurrogate(text[1]) ? 2 : 1;

        int nameEnd = 1;
        while (nameEnd < text.Length && char.IsAsciiLetter(text[nameEnd]))
            nameEnd++;
        if (nameEnd == 1)
            return 1;
        if (nameEnd < text.Length && text[nameEnd] == '[')
        {
            int relClose = text.Slice(nameEnd + 1).IndexOf(']');
            if (relClose >= 0)
                return nameEnd + 1 + relClose + 1;
        }
        return nameEnd;
    }

    private static int LastUnitStart(string text)
    {
        ReadOnlySpan<char> span = text.AsSpan();
        int end = 0;
        int last = 0;
        while (end < span.Length)
        {
            last = end;
            end += FirstUnitLen(span.Slice(end));
        }
        return last;
    }
}
