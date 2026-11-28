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

public sealed partial class EventVm
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

    private int _nextInstanceId = 1;
    private int _saveUi131Count;
    public bool EndGameRequested { get; set; }
    public bool ReturnToTitleRequested { get; set; }
    private readonly Dictionary<string, int> _titleToId = new(StringComparer.Ordinal);
    private readonly HashSet<int> _enteredInstanceIds = new();
    private readonly HashSet<int> _budgetExhausted = new();
    private readonly HashSet<int> _stopDoomed = new();

    // Transition setting installed by opcode 160 and consumed by 161/162
    // (Rust crates/wolf-vm/src/exec.rs:910-937).
    private int _transitionKind;
    private int _transitionFlags;
    private bool _transitionPrepared;
    private int _runCursor;

    /// <summary>True between 161 PrepareTransition and 162 ExecuteTransition.</summary>
    public bool TransitionPrepared => _transitionPrepared;

    /// <summary>O(1) indicator for whether Save/Load UI common event 131 is currently live.</summary>
    public bool HasSaveUi131 => _saveUi131Count > 0;

    /// <summary>True while an instance of the specified event id is live.</summary>
    public bool HasEvent(int eventId)
    {
        if (eventId == 131) return _saveUi131Count > 0;
        for (int i = 0; i < Instances.Count; i++)
        {
            if (Instances[i].EventId == eventId) return true;
        }
        return false;
    }

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
        Span<int> seen = stackalloc int[MaxCallDepth + 1];
        int seenCount = 0;
        while (current.ParentId.HasValue && depth <= MaxCallDepth)
        {
            int parentId = current.ParentId.Value;
            bool alreadySeen = false;
            for (int i = 0; i < seenCount; i++)
            {
                if (seen[i] == parentId)
                {
                    alreadySeen = true;
                    break;
                }
            }
            if (alreadySeen) break;
            seen[seenCount++] = parentId;
            var parent = FindInstance(parentId);
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

    /// <summary>Reattach a coroutine loaded from a save slot without restarting its program.</summary>
    public void RestoreInstance(Instance inst)
    {
        Instances.Add(inst);
        if (inst.EventId == 131) _saveUi131Count++;
        if (inst.Id >= _nextInstanceId) _nextInstanceId = inst.Id + 1;
    }

    /// <summary>Start a coroutine for a program that is not in the common-event set (map pages).</summary>
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
        if (program.Id == 131) _saveUi131Count++;
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
        BindCallArgs(inst, args, strings);
        return inst;
    }

    private static void BindCallArgs(Instance target, int[] args, string[] strings)
    {
        for (int i = 0; i < args.Length; i++)
        {
            target.Self.Set(Magic.SelfArgBase + i, args[i]);
        }
        for (int k = 0; k < strings.Length; k++)
        {
            target.Self.SetStr(Magic.SelfArgBase + 5 + k, strings[k] ?? "");
        }
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
        for (int i = 0; i < Instances.Count; i++)
        {
            var inst = Instances[i];
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
        if (CommonSelf.TryGetValue(inst.EventId, out var existing))
        {
            existing.CopyFrom(inst.Self);
        }
        else
        {
            CommonSelf[inst.EventId] = inst.Self.Clone();
        }
    }

    /// <summary>Decode the operand table shared by 210 and 300 (exec.rs call_arguments).</summary>
    private (int[] Args, string[] Strings, int? ReturnTo) DecodeCallArguments(Slot slot, Instance inst)
    {
        int status = slot.Params.Length > 1 ? slot.Params[1] : 0;
        int numCount = status & 0x0F;
        int rawStrCount = (status >> 4) & 0x0F;
        int strCount = Math.Min(rawStrCount, 4);
        bool hasRet = ((status >> 24) & 1) != 0;

        int[] args = numCount == 0 ? Array.Empty<int>() : new int[numCount];
        for (int i = 0; i < numCount; i++)
        {
            args[i] = EvalNum(2 + i < slot.Params.Length ? slot.Params[2 + i] : 0, inst);
        }

        string[] strings = strCount == 0 ? Array.Empty<string>() : new string[strCount];
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

    private readonly Func<int, int, int, int, string> _readDbTextDelegate;

    public EventVm()
    {
        _readDbTextDelegate = ReadDbText;
    }

    public string Expand(string text, Instance inst)
    {
        return TextExpander.Expand(text, inst, Vars, _readDbTextDelegate);
    }

    /// <summary>Database cell text for \cdb (0 = CData), \sdb (1 = Sys), and \udb (2 = Data) escapes.</summary>
    private string ReadDbText(int kind, int dbType, int row, int column)
        => Host.DbText(kind, dbType, row, column);

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

            var ctrl = pc < prog.ControlMap.Length ? prog.ControlMap[pc] : Control.None;
            if (TryStepControlFlow(inst, prog, slot, ctrl, out Signal flowSignal))
            {
                if (flowSignal != Signal.None) return flowSignal;
                continue;
            }

            if (TryStepDataOpcode(inst, slot, out Signal dataSignal))
            {
                if (dataSignal != Signal.None) return dataSignal;
                continue;
            }

            if (TryStepMediaOpcode(inst, prog, slot, ctrl, out Signal mediaSignal))
            {
                if (mediaSignal != Signal.None) return mediaSignal;
                continue;
            }

            if (TryStepControlOpcode(inst, prog, slot, out Signal ctrlSignal))
            {
                if (ctrlSignal != Signal.None) return ctrlSignal;
                continue;
            }

            UnhandledOpcodeCount++;
            FaultReason ??= $"unhandled opcode {slot.Opcode} at event {inst.EventId} pc {pc}";
            Host.Log($"unhandled opcode {slot.Opcode} at event {inst.EventId} pc {pc}");
            if (StrictMode)
            {
                throw new InvalidOperationException(FaultReason);
            }
            inst.Pc++;
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
                    int target = reply.IsCancel
                        ? block.CancelPc ?? block.EndTarget(inst.Program.Len)
                        : block.CaseTarget(reply.Index + 2) ?? block.EndTarget(inst.Program.Len);
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
        _stopDoomed.Clear();
        _stopDoomed.Add(id);
        bool grew = true;
        while (grew)
        {
            grew = false;
            for (int i = 0; i < Instances.Count; i++)
            {
                var other = Instances[i];
                if (_stopDoomed.Contains(other.Id)) continue;
                if (other.ParentId.HasValue && _stopDoomed.Contains(other.ParentId.Value))
                {
                    _stopDoomed.Add(other.Id);
                    grew = true;
                }
            }
        }

        for (int i = 0; i < Instances.Count; i++)
        {
            var other = Instances[i];
            if (_stopDoomed.Contains(other.Id)) PersistCommonSelf(other, isForcedStop: true);
        }
        for (int i = Instances.Count - 1; i >= 0; i--)
        {
            var other = Instances[i];
            if (_stopDoomed.Contains(other.Id))
            {
                _enteredInstanceIds.Remove(other.Id);
                _autoKeysDelivered.Remove(other.Id);
                if (other.EventId == 131 && _saveUi131Count > 0) _saveUi131Count--;
                Instances.RemoveAt(i);
            }
        }
        for (int i = 0; i < Instances.Count; i++)
        {
            var other = Instances[i];
            if (other.Wait.Kind == WaitStateKind.Child && _stopDoomed.Contains(other.Wait.ChildId))
            {
                other.Wait.Clear();
            }
        }
        _stopDoomed.Clear();
    }

    /// <summary>Drop every coroutine (map switch / teleport).</summary>
    public void StopAll()
    {
        EndGameRequested = false;
        ReturnToTitleRequested = false;
        ClearInstancesState();
    }

    private void ClearInstancesState()
    {
        ResetTransitionState();
        _enteredInstanceIds.Clear();
        _autoKeysDelivered.Clear();
        _runCursor = 0;
        _saveUi131Count = 0;
        Instances.Clear();
    }

    private void ResetTransitionState()
    {
        _transitionKind = 0;
        _transitionFlags = 0;
        _transitionPrepared = false;
        TransitionPhase = 0;
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

        for (int i = start; i < Instances.Count; i++)
        {
            if (!StepInstanceAt(i, ref budget, budgetExhausted, out stopped)) break;
        }
        if (!stopped)
        {
            for (int i = 0; i < start && i < Instances.Count; i++)
            {
                if (!StepInstanceAt(i, ref budget, budgetExhausted, out stopped)) break;
            }
        }
        if (!stopped)
        {
            _runCursor = 0;
        }
    }

    private bool StepInstanceAt(int index, ref int totalBudget, HashSet<int>? budgetExhausted, out bool stopped)
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
        stopped = false;
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

    public int ReapFinished()
    {
        int woken = 0;
        for (int i = Instances.Count - 1; i >= 0; i--)
        {
            var inst = Instances[i];
            if (inst.Pc >= inst.Program.Len && inst.Wait.Kind == WaitStateKind.None && !inst.RetainAfterFinish)
            {
                Instance? parent = inst.ParentId.HasValue ? FindInstance(inst.ParentId.Value) : null;
                if (parent != null && inst.ReturnTo.HasValue && inst.Program.ReturnValueId != Program.NoReturnValue)
                {
                    int val = inst.Self.Get((int)(Magic.SelfArgBase + inst.Program.ReturnValueId));
                    StoreNum(parent, inst.ReturnTo.Value, val);
                }

                // exec.rs reap_finished: capture the bank before dropping it.
                PersistCommonSelf(inst, isForcedStop: false);
                _enteredInstanceIds.Remove(inst.Id);
                OnInstanceFinished?.Invoke(inst);

                if (parent != null && parent.Wait.Kind == WaitStateKind.Child && parent.Wait.ChildId == inst.Id)
                {
                    parent.Wait.Clear();
                    parent.JustResumedFromChild = true;
                    woken++;
                }

                if (inst.EventId == 131 && _saveUi131Count > 0) _saveUi131Count--;
                Instances.RemoveAt(i);
            }
        }
        return woken;
    }
}
