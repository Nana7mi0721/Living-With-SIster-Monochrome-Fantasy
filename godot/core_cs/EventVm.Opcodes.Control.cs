using System;

namespace Lwsmf.Core;

public sealed partial class EventVm
{
    private bool TryStepControlFlow(Instance inst, Program prog, Slot slot, Control ctrl, out Signal signal)
    {
        switch (ctrl.Kind)
        {
            case ControlKind.Block:
            {
                if ((uint)ctrl.BlockIndex >= (uint)prog.Blocks.Count)
                {
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
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
                            signal = Signal.None;
                            return true;
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
                        signal = Signal.None;
                        return true;
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
                                signal = Signal.None;
                                return true;
                            }
                            remaining = count;
                        }
                        inst.Loops.Add(new LoopCtx { Block = ctrl.BlockIndex, Remaining = remaining });
                        inst.Pc++;
                        signal = Signal.None;
                        return true;
                    }
                    case BlockKind.Choices:
                        signal = BeginChoiceWait(inst, slot, ctrl.BlockIndex);
                        return true;
                }
                break;
            }
            case ControlKind.Case:
            {
                // Sequential execution reaching a case marker jumps to block end.
                if ((uint)ctrl.BlockIndex >= (uint)prog.Blocks.Count)
                {
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                var block = prog.Blocks[ctrl.BlockIndex];
                inst.Pc = block.EndTarget(prog.Len);
                signal = Signal.None;
                return true;
            }
            case ControlKind.LoopEnd:
                CloseLoop(prog, inst, ctrl.BlockIndex);
                signal = Signal.None;
                return true;
            case ControlKind.LabelJump:
            {
                string name = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                string expandedName = Expand(name, inst);
                inst.Pc = prog.Labels.TryGetValue(expandedName, out int target) ? target : prog.Len;
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
                signal = Signal.None;
                return true;
            }
        }
        signal = Signal.None;
        return false;
    }

    private bool TryStepControlOpcode(Instance inst, Program prog, Slot slot, out Signal signal)
    {
        switch (slot.Opcode)
        {
            case 103:
            case 111:
            case 212:
            case 104 or 213 or 421 or 498:
            case 0 or 99 or 401 or 420 or 499:
                // 0/99 are NOP markers and 401/420/499 are block markers; 104/213/421/498 are no-ops (exec.rs).
                inst.Pc++;
                signal = Signal.None;
                return true;
            case 106:
            {
                string raw = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                Host.Log(Expand(raw, inst));
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 107:
                Host.ClearDebug();
                inst.Pc++;
                signal = Signal.None;
                return true;
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
                signal = Signal.Blocked;
                return true;
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
                signal = Signal.None;
                return true;
            }
            case 172 or 173:
                inst.Pc = prog.Len;
                signal = Signal.Finished;
                return true;
            case 174:
                // D-62: Defer Host.ReturnToTitle() until GameSession.AdvanceFrame frame-end so
                // in-flight StepFrame/ReapFinished unwinds cleanly instead of re-entering title
                // setup inside the VM loop (unlike 175 EndGame, which quits immediately).
                ReturnToTitleRequested = true;
                ClearInstancesState();
                signal = Signal.ReturnToTitle;
                return true;
            case 175:
                EndGameRequested = true;
                Host.EndGame();
                ClearInstancesState();
                signal = Signal.EndGame;
                return true;
            case 176:
                // exec.rs 3150-3158: ContinueLoop closes the innermost loop. It is
                // not a block head (program.rs 293), so Program must not push a
                // frame here; a hand-made Program without one just advances.
                if (inst.Loops.Count == 0)
                {
                    Host.Log($"[EventVm] continue-loop 176 without a loop at event {inst.EventId} pc {inst.Pc}");
                    inst.Pc++;
                }
                else
                {
                    CloseLoop(prog, inst, inst.Loops[^1].Block);
                }
                signal = Signal.None;
                return true;
            case 180:
            {
                int frames = slot.Params.Length > 0 ? EvalNum(slot.Params[0], inst) : 1;
                uint uFrames = (uint)Math.Max(1, frames);
                Host.Wait(uFrames);
                inst.Wait.WaitForFrames(uFrames);
                inst.Pc++;
                signal = Signal.Blocked;
                return true;
            }
            case 201:
            {
                int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                if (slot.MoveRoute != null) Host.MoveRoute(target, slot.MoveRoute);
                inst.Pc++;
                signal = Signal.None;
                return true;
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
                    signal = Signal.None;
                    return true;
                }
                if (ShouldSkipCall != null && ShouldSkipCall(inst, targetEvent.Value))
                {
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }

                var (childArgs, childStrings, returnTo) = DecodeCallArguments(slot, inst);

                if (CallDepth(inst) >= MaxCallDepth)
                {
                    RejectedSpawnCount++;
                    Host.Log($"max call depth {MaxCallDepth} reached; call dropped (event {targetEvent.Value})");
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }

                var child = CreateInstance(targetEvent.Value, inst.Id, returnTo);
                if (child == null)
                {
                    // exec.rs spawn_child: a rejected call is dropped without
                    // blocking the caller; execution continues at the next slot.
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                Host.CommonEventCall(targetEvent.Value, childArgs, childStrings);
                child.IsParallel = inst.IsParallel;
                BindCallArgs(child, childArgs, childStrings);

                inst.Wait.WaitForChild(child.Id);
                inst.Pc++;
                signal = Signal.Call;
                return true;
            }
            case 211:
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
                signal = Signal.None;
                return true;
            }
            case 270:
                Host.Party(slot.Params);
                inst.Pc++;
                signal = Signal.None;
                return true;
            case 281:
                Host.Scroll(slot.Params);
                inst.Pc++;
                signal = Signal.None;
                return true;
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
                    signal = Signal.None;
                    return true;
                }
                if (CallDepth(inst) >= MaxCallDepth)
                {
                    RejectedSpawnCount++;
                    Host.Log($"max call depth {MaxCallDepth} reached; call dropped (event titled {name})");
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }

                var (callArgs, callStrings, returnTo) = DecodeCallArguments(slot, inst);
                var child = CreateInstance(callee, inst.Id, returnTo);
                if (child == null)
                {
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                child.IsParallel = inst.IsParallel;
                // Both 210 and 300 dispatch a common event, so notify the host for both.
                Host.CommonEventCall(callee.Id, callArgs, callStrings);
                BindCallArgs(child, callArgs, callStrings);
                inst.Wait.WaitForChild(child.Id);
                inst.Pc++;
                signal = Signal.Call;
                return true;
            }
            default:
                signal = Signal.None;
                return false;
        }
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

        // LoopCtx is a reference type; update Remaining on the active frame in place.
        LoopCtx ctx = inst.Loops[loopPos];
        long rem = ctx.Remaining;
        bool again;
        if (rem < 0)
        {
            again = true; // 170 unconditional loop head
        }
        else if (rem > 1)
        {
            ctx.Remaining = rem - 1;
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
}
