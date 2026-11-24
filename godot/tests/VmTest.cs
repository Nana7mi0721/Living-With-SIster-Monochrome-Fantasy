using System;
using System.Collections.Generic;
using Lwsmf.Core;

namespace Lwsmf.Tests;

public static class VmTest
{
    public static void Run()
    {
        Console.WriteLine("[VmTest] Starting EventVM unit tests...");

        TestWolfRng();
        TestMagic();
        TestOpcode121Overflow();
        TestOpcode121DivideByZero();
        TestOpcode121RandomRange();
        TestOpcode121RangeWrite();
        TestOpcode121FlagSemantics();
        TestOpcode211ReserveArguments();
        TestOpcode124EventIdentity();
        TestOpcode124PictureDivideAssign();
        TestOpcode112StringConditionFlags();
        TestOpcode221_222CSelfStringPath();
        TestOpcode150BlendMode();
        TestOpcode150DynamicFile();
        TestOpcode174ReturnToTitleDeferral();
        TestCallTargetResolution();
        TestOpcode111ConditionAndEscape();
        TestOpcode180WaitFrames();
        TestOpcode210CallChildAndReturn();
        TestOpcode179CountedLoopAndBreak();
        TestOpcode212LabelJump();
        TestCommonSelfPersistsAcrossCalls();
        TestUnknownEventRejected();
        TestUnhandledOpcodeDiagnostic();
        TestOpcode210UnknownTargetDropsCall();
        TestInstanceLimit();
        TestMinValueNegativeOneDivision();
        TestOpcode201MoveRoute();
        TestOpcode270Party();
        TestOpcode300CallByName();
        TestTraceBgsChannel();
        TestUnknownSoundType();
        TestOpcode125126AutoAndBanKeys();
        TestOpcode122StringOps();
        TestOpcode140SoundCellSplitAndEval();
        TestTextExpanderCselfEmptyAndSdb();
        TestOpcode102CancelFlag();
        TestOpcode101PairedChoice();
        TestOpcode101MultiDigitFacePrefix();
        TestGetFileExistHostDelegate();
        TestOpcode1000ProFeatureWait();
        TestEventEntryNotifications();
        TestPerInstanceBudgetPreventsStarvation();
        TestParallelEnteredInstanceDoesNotLeak();
        TestSoundChannelLayoutMatchesPresentation();
        TestOpcode123BlockingKeyWaitAndDebounce();
        TestMaxCallDepth();
        TestOpcode213LabelJumpPopsLoops();
        TestRoundRobinBudgetPreventsTotalStarvation();
        TestStopAllAnd174Cleanup();
        TestMalformedSlotBoundsAndCallStrings();
        TestGameSessionRestoreAndReset();
        DatabaseTest.Run();
        AutoEventsTest.Run();
        BattleCommandTest.Run();
        BattleWiringTest.Run();
        BattleTargetTest.Run();
        BattleSkillTargetTest.Run();
        BattleMenuBridgeTest.Run();
        BattleNaturalLoopTest.Run();
        HEventGateTest.Run();
        ContentMatrixTest.Run();
        M13AllEndingsTest.Run();
        M13RandomEventsTest.Run();
        Op150NegativeScaleScanTest.Run();

        Console.WriteLine("[VmTest] ALL VM TESTS PASSED!");
    }

    private static void TestWolfRng()
    {
        Console.WriteLine("  Testing WolfRng...");

        // MSVC srand(1) test vectors
        var rngMsvc = new WolfRng(1);
        ushort[] expectedMsvc = [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358];
        for (int i = 0; i < expectedMsvc.Length; i++)
        {
            ushort actual = rngMsvc.NextRaw();
            if (actual != expectedMsvc[i])
                throw new Exception($"MSVC vector mismatch at {i}: expected {expectedMsvc[i]}, got {actual}");
        }

        // Park-Miller event_seed(1) test vectors
        var rngEvent = WolfRng.FromEventSeed(1);
        uint[] expectedEvent = [
            16_807,
            282_475_249,
            1_622_650_073,
            984_943_658,
            1_144_108_930,
            470_211_272,
            101_027_544,
            1_457_850_878
        ];
        for (int i = 0; i < expectedEvent.Length; i++)
        {
            uint actual = rngEvent.NextEventDraw();
            if (actual != expectedEvent[i])
                throw new Exception($"Park-Miller vector mismatch at {i}: expected {expectedEvent[i]}, got {actual}");
        }

        // Rejection ceiling test: 2_000_000_001
        var rngCeil = WolfRng.FromEventSeed(2_000_000_001);
        uint firstDraw = rngCeil.NextEventDraw();
        if (firstDraw > 2_000_000_000)
            throw new Exception($"Ceiling rejection failed, draw was {firstDraw}");

        // Wide range must not overflow 32-bit signed difference.
        var rngWide = WolfRng.FromEventSeed(1);
        bool sawWide = false;
        for (int i = 0; i < 16; i++)
        {
            int v = rngWide.Range(-2_000_000_000, 2_000_000_000);
            if (v < -2_000_000_000 || v > 2_000_000_000)
                throw new Exception($"Wide Range out of bounds: {v}");
            if (Math.Abs((long)v) > 300_000_000L) sawWide = true;
        }
        if (!sawWide)
            throw new Exception("Wide Range stayed collapsed inside signed 32-bit overflow window");
        int full = rngWide.Range(int.MinValue, int.MaxValue);
        if (full < int.MinValue || full > int.MaxValue)
            throw new Exception($"Full int range failed: {full}");
    }

    private static void TestMagic()
    {
        Console.WriteLine("  Testing Magic decoding...");

        var m0 = Magic.Decode(0);
        if (m0.Kind != MagicKind.Literal || m0.Index != 0)
            throw new Exception("Magic 0 failed");

        var mVar = Magic.Decode(2_000_007);
        if (mVar.Kind != MagicKind.Var || mVar.Index != 7)
            throw new Exception("Magic 2_000_007 failed");

        var mStr = Magic.Decode(3_000_009);
        if (mStr.Kind != MagicKind.StrVar || mStr.Index != 9)
            throw new Exception("Magic 3_000_009 failed");

        var mSys = Magic.Decode(9_000_024);
        if (mSys.Kind != MagicKind.SysVar || mSys.Index != 24)
            throw new Exception("Magic 9_000_024 failed");

        // Self slot test: 1_000_000 (group 0 slot 0 -> 0), 1_600_000 (group 6 slot 0 -> 600)
        var mSelf0 = Magic.Decode(1_000_000);
        if (mSelf0.Kind != MagicKind.SelfVar || mSelf0.Index != 0)
            throw new Exception($"Self 1_000_000 failed: {mSelf0.Index}");

        var mSelf6 = Magic.Decode(1_600_000);
        if (mSelf6.Kind != MagicKind.SelfVar || mSelf6.Index != 600)
            throw new Exception($"Self 1_600_000 failed: {mSelf6.Index}");

        var mSelf6_22 = Magic.Decode(1_600_022);
        if (mSelf6_22.Kind != MagicKind.SelfVar || mSelf6_22.Index != 622)
            throw new Exception($"Self 1_600_022 failed: {mSelf6_22.Index}");
    }

    private static void TestOpcode121Overflow()
    {
        Console.WriteLine("  Testing TC-121-01: 32-bit wrapping overflow...");
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(1, "Test", new List<Slot>()));
        var inst = vm.CreateInstance(1)!;

        // arg: [target=2000000, a=int.MaxValue, b=1, bits=0x0000 (add + assign)]
        // 2_000_000 = int.MaxValue + 1 -> int.MinValue
        vm.ExecuteSetVariable([2_000_000, int.MaxValue, 1, 0x0000], inst);
        int val = vm.Vars.Get(0);
        if (val != int.MinValue)
            throw new Exception($"TC-121-01 failed: expected {int.MinValue}, got {val}");

        // Subtraction underflow: int.MinValue - 1 -> int.MaxValue
        // bits = 0x1000 (sub + assign)
        vm.ExecuteSetVariable([2_000_000, int.MinValue, 1, 0x1000], inst);
        val = vm.Vars.Get(0);
        if (val != int.MaxValue)
            throw new Exception($"TC-121-01 underflow failed: expected {int.MaxValue}, got {val}");
    }

    private static void TestOpcode121DivideByZero()
    {
        Console.WriteLine("  Testing TC-121-02: Divide-by-zero protection...");
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(1, "Test", new List<Slot>()));
        var inst = vm.CreateInstance(1)!;

        // Set var 0 to 42
        vm.Vars.Set(0, 42);

        // Binary divide by zero: 42 / 0 -> 42 (unchanged)
        // bits: (3 << 12) = 0x3000
        vm.ExecuteSetVariable([2_000_000, 42, 0, 0x3000], inst);
        int val = vm.Vars.Get(0);
        if (val != 42)
            throw new Exception($"TC-121-02 binary div zero failed: expected 42, got {val}");

        // Compound divide by zero: var 0 /= 0 -> 42 (unchanged)
        // bits: compound 0x400
        vm.ExecuteSetVariable([2_000_000, 0, 0, 0x0400], inst);
        val = vm.Vars.Get(0);
        if (val != 42)
            throw new Exception($"TC-121-02 compound div zero failed: expected 42, got {val}");

        // Remainder by zero: 42 % 0 -> 42 (unchanged)
        // bits: (4 << 12) = 0x4000
        vm.ExecuteSetVariable([2_000_000, 42, 0, 0x4000], inst);
        val = vm.Vars.Get(0);
        if (val != 42)
            throw new Exception($"TC-121-02 remainder zero failed: expected 42, got {val}");
    }

    private static void TestOpcode121RandomRange()
    {
        Console.WriteLine("  Testing TC-121-03: 0x6000 random range with min > max...");
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(1, "Test", new List<Slot>()));
        var inst = vm.CreateInstance(1)!;
        vm.Rng.SetEventSeed(1);

        // bits: (6 << 12) = 0x6000 (random range)
        // min > max: 100 to 50
        vm.ExecuteSetVariable([2_000_000, 100, 50, 0x6000], inst);
        int val = vm.Vars.Get(0);
        if (val < 50 || val > 100)
            throw new Exception($"TC-121-03 out of bounds: {val} not in [50, 100]");

        // Verify deterministic value matching Rust seed 1 first draw
        // draw = 16807, min=50, max=100, modulus=51. 16807 % 51 = 28 -> 50 + 28 = 78
        if (val != 78)
            throw new Exception($"TC-121-03 determinism mismatch: expected 78, got {val}");
    }

    private static void TestOpcode121RangeWrite()
    {
        Console.WriteLine("  Testing TC-121-04: Range write with arg[4]...");
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(1, "Test", new List<Slot>()));
        var inst = vm.CreateInstance(1)!;

        // Write 99 to self slots 1600022..=1600025 (4 slots)
        // bits: 0x10000 | 0x0000 (assign), arg4 = 3
        vm.ExecuteSetVariable([1_600_022, 99, 0, 0x10000, 3], inst);

        for (int i = 0; i < 4; i++)
        {
            int slotVal = inst.Self.Get(622 + i);
            if (slotVal != 99)
                throw new Exception($"TC-121-04 failed at slot {622 + i}: expected 99, got {slotVal}");
        }
        // Slot 626 must be untouched (0)
        if (inst.Self.Get(626) != 0)
            throw new Exception("TC-121-04 wrote beyond range!");
    }

    private static void TestOpcode111ConditionAndEscape()
    {
        Console.WriteLine("  Testing Opcode 111 (Condition), 401 (Case escape), 420 (Else)...");
        // Program structure:
        // pc 0: 111 [head=2, Var 0, 10, op=0 (>10), Var 0, 5, op=2 (==5)]
        // pc 1: 401 [case 1] -> Var 1 = 100
        // pc 2: 121 [Var 1 = 100]
        // pc 3: 401 [case 2] -> Var 1 = 200
        // pc 4: 121 [Var 1 = 200]
        // pc 5: 420 [else] -> Var 1 = 300
        // pc 6: 121 [Var 1 = 300]
        // pc 7: 499 [end]
        // pc 8: 121 [Var 2 = 999] (after block)
        var slots = new List<Slot>
        {
            Slot.Command(111, [2, 2_000_000, 10, 0, 2_000_000, 5, 2]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_001, 100, 0, 0x0000]),
            Slot.Command(401, [2]),
            Slot.Command(121, [2_000_001, 200, 0, 0x0000]),
            Slot.Command(420),
            Slot.Command(121, [2_000_001, 300, 0, 0x0000]),
            Slot.Command(499),
            Slot.Command(121, [2_000_002, 999, 0, 0x0000]),
        };
        var prog = Program.FromSlots(100, "CondTest", slots);

        // Test 1: Var 0 = 5 matches Case 2 -> Var 1 = 200, Var 2 = 999
        {
            var vm = new EventVm();
            vm.RegisterProgram(prog);
            var inst = vm.CreateInstance(100)!;
            vm.Vars.Set(0, 5);

            int budget = 100;
            var sig = vm.StepInstance(inst, ref budget);
            if (sig != Signal.Finished)
                throw new Exception($"TestOpcode111: expected Finished, got {sig}");
            if (vm.Vars.Get(1) != 200)
                throw new Exception($"TestOpcode111: expected Var 1 == 200, got {vm.Vars.Get(1)}");
            if (vm.Vars.Get(2) != 999)
                throw new Exception($"TestOpcode111: expected Var 2 == 999, got {vm.Vars.Get(2)}");
        }

        // Test 2: Var 0 = 15 matches Case 1 -> Var 1 = 100, then ESCAPES to after 499 (does NOT fall into case 2 or else)
        {
            var vm = new EventVm();
            vm.RegisterProgram(prog);
            var inst = vm.CreateInstance(100)!;
            vm.Vars.Set(0, 15);

            int budget = 100;
            var sig = vm.StepInstance(inst, ref budget);
            if (sig != Signal.Finished)
                throw new Exception($"TestOpcode111: expected Finished, got {sig}");
            if (vm.Vars.Get(1) != 100)
                throw new Exception($"TestOpcode111: expected Var 1 == 100, got {vm.Vars.Get(1)}");
            if (vm.Vars.Get(2) != 999)
                throw new Exception($"TestOpcode111: expected Var 2 == 999, got {vm.Vars.Get(2)}");
        }

        // Test 3: Var 0 = 0 matches neither -> Else branch -> Var 1 = 300
        {
            var vm = new EventVm();
            vm.RegisterProgram(prog);
            var inst = vm.CreateInstance(100)!;
            vm.Vars.Set(0, 0);

            int budget = 100;
            var sig = vm.StepInstance(inst, ref budget);
            if (sig != Signal.Finished)
                throw new Exception($"TestOpcode111: expected Finished, got {sig}");
            if (vm.Vars.Get(1) != 300)
                throw new Exception($"TestOpcode111: expected Var 1 == 300 (else), got {vm.Vars.Get(1)}");
            if (vm.Vars.Get(2) != 999)
                throw new Exception($"TestOpcode111: expected Var 2 == 999, got {vm.Vars.Get(2)}");
        }
    }

    private static void TestOpcode180WaitFrames()
    {
        Console.WriteLine("  Testing Opcode 180 (Wait frames)...");
        // Program:
        // pc 0: 180 [3 frames]
        // pc 1: 121 [Var 0 = 77]
        var slots = new List<Slot>
        {
            Slot.Command(180, [3]),
            Slot.Command(121, [2_000_000, 77, 0, 0x0000])
        };
        var prog = Program.FromSlots(200, "WaitTest", slots);
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(200)!;

        // Frame 1: executes pc 0, sets Wait.Frames = 3, blocks
        vm.StepFrame();
        if (vm.Vars.Get(0) != 0)
            throw new Exception("Frame 1: Var 0 should be 0");
        if (inst.Wait.Kind != WaitStateKind.Frames || inst.Wait.Frames != 3)
            throw new Exception($"Frame 1: Wait.Frames should be 3, got {inst.Wait.Frames}");

        // Frame 2: decrements wait frames to 2
        vm.StepFrame();
        if (vm.Vars.Get(0) != 0)
            throw new Exception("Frame 2: Var 0 should be 0");
        if (inst.Wait.Frames != 2)
            throw new Exception($"Frame 2: Wait.Frames should be 2, got {inst.Wait.Frames}");

        // Frame 3: decrements wait frames to 1
        vm.StepFrame();
        if (vm.Vars.Get(0) != 0)
            throw new Exception("Frame 3: Var 0 should be 0");
        if (inst.Wait.Frames != 1)
            throw new Exception($"Frame 3: Wait.Frames should be 1, got {inst.Wait.Frames}");

        // Frame 4: PollWaits clears wait state, executes pc 1, Var 0 = 77!
        vm.StepFrame();
        if (vm.Vars.Get(0) != 77)
            throw new Exception($"Frame 4: Var 0 should be 77, got {vm.Vars.Get(0)}");
    }

    private static void TestOpcode210CallChildAndReturn()
    {
        Console.WriteLine("  Testing Opcode 210 (Call Common Event with args & return value)...");
        var childSlots = new List<Slot>
        {
            Slot.Command(121, [1_600_000, 1_600_000, 1_600_001, 0x0000]),
            Slot.Command(172)
        };
        var childProg = Program.FromSlots(50, "ChildProg", childSlots);
        childProg.ReturnValueId = 0;

        int paramStatus = (1 << 24) | 2;
        var parentSlots = new List<Slot>
        {
            Slot.Command(210, [500_050, paramStatus, 15, 27, 2_000_005]),
            Slot.Command(121, [2_000_006, 888, 0, 0x0000])
        };
        var parentProg = Program.FromSlots(40, "ParentProg", parentSlots);

        var vm = new EventVm();
        vm.RegisterProgram(childProg);
        vm.RegisterProgram(parentProg);

        var parentInst = vm.CreateInstance(40)!;

        // Frame 1: Parent steps, spawns child
        vm.StepFrame();
        // Frame 2: Child steps (executes op 121, then 172) -> Child finishes, ReapFinished delivers return, Parent woken
        vm.StepFrame();
        // Frame 3: Parent steps (pc 1, sets Var 6)
        vm.StepFrame();

        int retVal = vm.Vars.Get(5);
        if (retVal != 42)
            throw new Exception($"TestOpcode210: expected Var 5 == 42, got {retVal}");

        int var6 = vm.Vars.Get(6);
        if (var6 != 888)
            throw new Exception($"TestOpcode210: expected Var 6 == 888, got {var6}");
    }

    private static void TestOpcode179CountedLoopAndBreak()
    {
        Console.WriteLine("  Testing Opcode 179 (Counted loop) & Opcode 171 (Break loop)...");
        // Loop 5 times: Var 0 += 1
        // pc 0: 179 [count=5]
        // pc 1: 121 [Var 0 += 1] (compound add: bits=0x0100)
        // pc 2: 498 [loop end]
        var slots = new List<Slot>
        {
            Slot.Command(179, [5]),
            Slot.Command(121, [2_000_000, 1, 0, 0x0100]),
            Slot.Command(498)
        };
        var prog = Program.FromSlots(300, "LoopTest", slots);
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(300)!;

        int budget = 1000;
        var sig = vm.StepInstance(inst, ref budget);
        if (sig != Signal.Finished)
            throw new Exception($"TestOpcode179: expected Finished, got {sig}");
        if (vm.Vars.Get(0) != 5)
            throw new Exception($"TestOpcode179: expected Var 0 == 5, got {vm.Vars.Get(0)}");

        // Loop with break:
        // pc 0: 179 [count=100]
        // pc 1: 121 [Var 1 += 1]
        // pc 2: 111 [1 cond: Var 1 == 3]
        // pc 3: 401 [case 1]
        // pc 4: 171 [break]
        // pc 5: 499 [end cond]
        // pc 6: 498 [end loop]
        var breakSlots = new List<Slot>
        {
            Slot.Command(179, [100]),
            Slot.Command(121, [2_000_001, 1, 0, 0x0100]),
            Slot.Command(111, [1, 2_000_001, 3, 2]),
            Slot.Command(401, [1]),
            Slot.Command(171),
            Slot.Command(499),
            Slot.Command(498)
        };
        var breakProg = Program.FromSlots(301, "BreakLoopTest", breakSlots);
        vm.RegisterProgram(breakProg);
        var breakInst = vm.CreateInstance(301)!;

        budget = 1000;
        sig = vm.StepInstance(breakInst, ref budget);
        if (sig != Signal.Finished)
            throw new Exception($"TestOpcode171: expected Finished, got {sig}");
        if (vm.Vars.Get(1) != 3)
            throw new Exception($"TestOpcode171: expected Var 1 == 3 after break, got {vm.Vars.Get(1)}");
    }

    private static void TestOpcode212LabelJump()
    {
        Console.WriteLine("  Testing Opcode 212/213 (Label Jump)...");
        // pc 0: 213 (Jump to "SKIP_TARGET")
        // pc 1: 121 (Var 0 = 111 - should be skipped)
        // pc 2: 212 (Label "SKIP_TARGET")
        // pc 3: 121 (Var 0 = 222)
        var slots = new List<Slot>
        {
            Slot.Command(213, null, ["SKIP_TARGET"]),
            Slot.Command(121, [2_000_000, 111, 0, 0x0000]),
            Slot.Command(212, null, ["SKIP_TARGET"]),
            Slot.Command(121, [2_000_000, 222, 0, 0x0000])
        };
        var prog = Program.FromSlots(400, "LabelTest", slots);
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(400)!;

        int budget = 100;
        var sig = vm.StepInstance(inst, ref budget);
        if (sig != Signal.Finished)
            throw new Exception($"TestOpcode212: expected Finished, got {sig}");
        if (vm.Vars.Get(0) != 222)
            throw new Exception($"TestOpcode212: expected Var 0 == 222, got {vm.Vars.Get(0)}");
    }

    private static void TestCommonSelfPersistsAcrossCalls()
    {
        Console.WriteLine("  Testing persistent common-event Self bank...");
        var slots = new List<Slot>
        {
            Slot.Command(121, [1_600_000, 1, 0, 0x0100]),
            Slot.Command(172)
        };
        var prog = Program.FromSlots(60, "PersistentSelf", slots);
        var vm = new EventVm();
        vm.RegisterProgram(prog);

        var first = vm.CreateInstance(60)!;
        first.Self.Set(600, 0);
        vm.StepFrame();
        if (!vm.CommonSelf.TryGetValue(60, out var bank) || bank.Get(600) != 1)
            throw new Exception($"CommonSelf: expected bank 1 after first call, got {(vm.CommonSelf.ContainsKey(60) ? vm.CommonSelf[60].Get(600) : -1)}");

        vm.CreateInstance(60);
        vm.StepFrame();
        if (vm.CommonSelf[60].Get(600) != 2)
            throw new Exception($"CommonSelf: expected 2 after second call, got {vm.CommonSelf[60].Get(600)}");
    }

    private static void TestUnknownEventRejected()
    {
        Console.WriteLine("  Testing unknown common event id...");
        var vm = new EventVm();
        if (vm.CreateInstance(9999) != null)
            throw new Exception("CreateInstance should reject an unknown event id");
        if (vm.RejectedSpawnCount != 1)
            throw new Exception($"RejectedSpawnCount expected 1, got {vm.RejectedSpawnCount}");
        if (vm.Instances.Count != 0)
            throw new Exception("Rejected spawn must not add an instance");
    }

    private static void TestUnhandledOpcodeDiagnostic()
    {
        Console.WriteLine("  Testing unhandled opcode diagnostics...");
        var prog = Program.FromSlots(80, "UnknownOpcode", new List<Slot> { Slot.Command(777) });
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        vm.CreateInstance(80);
        vm.StepFrame();
        if (vm.UnhandledOpcodeCount != 1)
            throw new Exception($"UnhandledOpcodeCount expected 1, got {vm.UnhandledOpcodeCount}");

        var strict = new EventVm { StrictMode = true };
        strict.RegisterProgram(prog);
        strict.CreateInstance(80);
        bool threw = false;
        try { strict.StepFrame(); }
        catch (InvalidOperationException) { threw = true; }
        if (!threw)
            throw new Exception("StrictMode should throw on an unhandled opcode");
    }

    private static void TestOpcode210UnknownTargetDropsCall()
    {
        Console.WriteLine("  Testing 210 with an unknown callee...");
        var slots = new List<Slot>
        {
            Slot.Command(210, [9999, 0]),
            Slot.Command(121, [2_000_000, 7, 0, 0x0000])
        };
        var prog = Program.FromSlots(81, "UnknownCallee", slots);
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        vm.CreateInstance(81);
        vm.StepFrame();
        if (vm.Vars.Get(0) != 7)
            throw new Exception($"210 drop-call should continue to the next slot, Var 0={vm.Vars.Get(0)}");
        if (vm.RejectedSpawnCount != 1)
            throw new Exception($"RejectedSpawnCount expected 1, got {vm.RejectedSpawnCount}");
    }

    private static void TestInstanceLimit()
    {
        Console.WriteLine("  Testing MaxInstances...");
        var prog = Program.FromSlots(70, "Limit", new List<Slot>());
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        for (int i = 0; i < EventVm.MaxInstances; i++)
        {
            if (vm.CreateInstance(70) == null)
                throw new Exception($"instance {i} should be accepted");
        }
        if (vm.CreateInstance(70) != null)
            throw new Exception("instance 4096 should be rejected");
        if (vm.RejectedSpawnCount != 1 || vm.Instances.Count != EventVm.MaxInstances)
            throw new Exception($"limit state wrong: rejected={vm.RejectedSpawnCount} count={vm.Instances.Count}");
    }

    private static void TestMinValueNegativeOneDivision()
    {
        Console.WriteLine("  Testing int.MinValue / -1 and % -1 safety...");
        int binary = EventVm.BinaryOp(0x3, int.MinValue, -1);
        if (binary != int.MinValue)
            throw new Exception($"BinaryOp min/-1 should wrap, got {binary}");
        int compound = EventVm.Compound(0x400, int.MinValue, -1);
        if (compound != int.MinValue)
            throw new Exception($"Compound min/-1 should wrap, got {compound}");
        int binMod = EventVm.BinaryOp(0x4, int.MinValue, -1);
        if (binMod != 0)
            throw new Exception($"BinaryOp min%-1 should return 0, got {binMod}");
        int compMod = EventVm.Compound(0x500, int.MinValue, -1);
        if (compMod != 0)
            throw new Exception($"Compound min%-1 should return 0, got {compMod}");
        if (EventVm.BinaryOp(0x3, 42, 7) != 6 || EventVm.Compound(0x400, 42, 7) != 6)
            throw new Exception("normal division regressed");
    }

    private static void TestOpcode201MoveRoute()
    {
        Console.WriteLine("  Testing Opcode 201 (Move route passthrough)...");
        var route = new MoveRouteData
        {
            MoveSpeed = 4,
            Commands = new List<RouteCommand>
            {
                new RouteCommand { Kind = 1, U32Args = [2, 3] },
                new RouteCommand { Kind = 4, U8Args = [0] }
            }
        };
        var prog = Program.FromSlots(90, "MoveRoute", new List<Slot> { Slot.Command(201, [6], moveRoute: route) });
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        vm.CreateInstance(90);
        vm.StepFrame();

        if (host.MoveRoutes.Count != 1)
            throw new Exception($"201 should forward exactly one route, got {host.MoveRoutes.Count}");
        var (target, forwarded) = host.MoveRoutes[0];
        if (target != 6 || forwarded.Commands.Count != 2 || forwarded.Commands[0].Kind != 1)
            throw new Exception($"201 route wrong: target={target} commands={forwarded.Commands.Count}");
        if (forwarded.Commands[0].Flattened().Length != 2)
            throw new Exception("201 route argument flattening lost u32 args");
        if (vm.UnhandledOpcodeCount != 0 || vm.Faulted)
            throw new Exception("201 must be a handled opcode");
    }

    private static void TestOpcode270Party()
    {
        Console.WriteLine("  Testing Opcode 270 (Party args)...");
        var prog = Program.FromSlots(91, "Party", new List<Slot> { Slot.Command(270, [0, 2]) });
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        vm.CreateInstance(91);
        vm.StepFrame();
        if (host.PartyCalls.Count != 1 || host.PartyCalls[0].Length != 2 || host.PartyCalls[0][0] != 0 || host.PartyCalls[0][1] != 2)
            throw new Exception("270 should forward raw party args");
    }

    private static void TestOpcode300CallByName()
    {
        Console.WriteLine("  Testing Opcode 300 (Call by title)...");
        var childSlots = new List<Slot>
        {
            Slot.Command(121, [2_000_000, 5, 0, 0x0000]),
            Slot.Command(172)
        };
        var child = Program.FromSlots(70, "Child300", childSlots);
        var parentSlots = new List<Slot>
        {
            Slot.Command(300, [0], ["Child300"]),
            Slot.Command(121, [2_000_001, 77, 0, 0x0000])
        };
        var parent = Program.FromSlots(95, "Caller300", parentSlots);

        var vm = new EventVm();
        var traceHost = new TraceRecorderHost();
        vm.Host = traceHost;
        vm.RegisterProgram(child);
        vm.RegisterProgram(parent);
        vm.CreateInstance(95);
        vm.StepFrame();
        vm.StepFrame();
        vm.StepFrame();
        if (vm.Vars.Get(1) != 77)
            throw new Exception($"300 should resume the caller, Var 1={vm.Vars.Get(1)}");
        if (vm.RejectedSpawnCount != 0 || vm.Instances.Count != 0)
            throw new Exception($"300 call left state behind: rejected={vm.RejectedSpawnCount} inst={vm.Instances.Count}");
        bool sawCall = false;
        foreach (var effect in traceHost.PendingEffects)
            sawCall |= effect.ToJsonString().Contains("\"kind\":\"call\"");
        if (!sawCall)
            throw new Exception("300 must report a call effect to the host");

        var missingSlots = new List<Slot>
        {
            Slot.Command(300, [0], ["NoSuchTitle"]),
            Slot.Command(121, [2_000_002, 88, 0, 0x0000])
        };
        var missing = Program.FromSlots(96, "MissingCaller", missingSlots);
        var vm2 = new EventVm();
        vm2.RegisterProgram(missing);
        vm2.CreateInstance(96);
        vm2.StepFrame();
        if (vm2.Vars.Get(2) != 88)
            throw new Exception("300 unknown title must continue the caller");
        if (vm2.RejectedSpawnCount != 1)
            throw new Exception($"300 unknown title should count one rejection, got {vm2.RejectedSpawnCount}");
    }

    private static void TestTraceBgsChannel()
    {
        Console.WriteLine("  Testing BGS trace channel mapping...");
        var host = new TraceRecorderHost();
        host.Sound(new SoundCmd
        {
            Channel = SoundChannel.Bgs,
            Action = SoundAction.Play,
            File = "BGS/rain.ogg",
            Volume = 40
        });
        if (host.PendingEffects.Count != 1)
            throw new Exception($"BGS trace expected 1 effect, got {host.PendingEffects.Count}");
        string json = host.PendingEffects[0].ToJsonString();
        if (!json.Contains("\"type\":\"bgs\""))
            throw new Exception($"BGS trace must say bgs, got {json}");
    }

    private static void TestUnknownSoundType()
    {
        Console.WriteLine("  Testing unknown opcode 140 sound type...");
        var prog = Program.FromSlots(92, "UnknownSoundType", new List<Slot>
        {
            Slot.Command(179, [2]),
            Slot.Command(140, [0x0200_0030], ["BGM/unknown.ogg"]),
            Slot.Command(498),
            Slot.Command(172)
        });
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        vm.CreateInstance(92);
        vm.StepFrame();
        if (host.Sounds.Count != 2)
            throw new Exception($"unknown sound type expected two commands from the loop, got {host.Sounds.Count}");
        if (host.Sounds[0].Channel != SoundChannel.Bgm)
            throw new Exception($"unknown sound type should keep the BGM fallback, got {host.Sounds[0].Channel}");
        if (!host.Logs.Exists(l => l.Contains("unknown sound type 3")))
            throw new Exception("unknown sound type must be logged instead of silently remapped");
    }

    private static void TestOpcode121FlagSemantics()
    {
        Console.WriteLine("  Testing Opcode 121 (editor flag bits are masked, exec.rs compound)...");
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(1, "Test", new List<Slot>()));
        var inst = vm.CreateInstance(1)!;

        // Bits 0..7 are editor flags the Rust oracle masks off; only bits 12..15 pick
        // the binary operator and bits 8..11 the transform.
        vm.Vars.Set(0, 3);
        vm.ExecuteSetVariable([2_000_000, 1, 2, 0x3300], inst);
        if (vm.Vars.Get(0) != 0)
            throw new Exception($"TC-121 integer division expected 0, got {vm.Vars.Get(0)}");
        vm.Vars.Set(0, 3);
        vm.ExecuteSetVariable([2_000_000, 1, 2, 0x3302], inst);
        if (vm.Vars.Get(0) != 0)
            throw new Exception($"TC-121 flag 0x02 must not enable real division, got {vm.Vars.Get(0)}");

        // No 0x01 clamp: 999999 + 999999 wraps nowhere and stores 1999998.
        vm.Vars.Set(1, 0);
        vm.ExecuteSetVariable([2_000_001, 999_999, 999_999, 0x0101], inst);
        if (vm.Vars.Get(1) != 1_999_998)
            throw new Exception($"TC-121 clamp flag must be ignored, got {vm.Vars.Get(1)}");

        // 0x04 treats arg[1] as a literal integer (even when it collides with a magic bank),
        // 0x08 treats arg[2] as a literal integer, 0x10 dereferences target indirectly, and
        // 0x20 dereferences arg[1] indirectly.
        vm.Vars.Set(0, 9);
        vm.ExecuteSetVariable([2_000_002, 2_000_000, 0, 0x0004], inst);
        if (vm.Vars.Get(2) != 2_000_000)
            throw new Exception($"TC-121 0x04 literal flag expected 2000000, got {vm.Vars.Get(2)}");
        vm.ExecuteSetVariable([2_000_003, 2_000_000, 2_000_000, 0x2008], inst);
        if (vm.Vars.Get(3) != 18_000_000)
            throw new Exception($"TC-121 0x08 literal b flag expected 18000000, got {vm.Vars.Get(3)}");
        vm.ExecuteSetVariable([2_000_005, 2_000_002, 0, 0x0020], inst);
        if (vm.Vars.Get(5) != 9)
            throw new Exception($"TC-121 0x20 indirect a expected 9, got {vm.Vars.Get(5)}");
        vm.Vars.Set(6, 2_000_007);
        vm.ExecuteSetVariable([2_000_006, 77, 0, 0x0010], inst);
        if (vm.Vars.Get(7) != 77)
            throw new Exception($"TC-121 0x10 indirect target expected 77, got {vm.Vars.Get(7)}");

        // 0x600 is a unary transform of the target's current value, not max(cur, value).
        vm.Vars.Set(4, 1);
        vm.ExecuteSetVariable([2_000_004, 5, 0, 0x0600], inst);
        if (vm.Vars.Get(4) != 1)
            throw new Exception($"TC-121 0x600 must keep current value, got {vm.Vars.Get(4)}");

        // 0x800 is abs(current), not abs(value).
        vm.Vars.Set(6, -7);
        vm.ExecuteSetVariable([2_000_006, 3, 0, 0x0800], inst);
        if (vm.Vars.Get(6) != 7)
            throw new Exception($"TC-121 0x800 must abs the current value, got {vm.Vars.Get(6)}");

        // 0x10000 batch count is an evaluated operand: +1 across Var 10..12.
        vm.Vars.Set(10, 0);
        vm.Vars.Set(11, 0);
        vm.Vars.Set(12, 0);
        vm.ExecuteSetVariable([2_000_010, 1, 0, 0x10100, 2], inst);
        if (vm.Vars.Get(10) != 1 || vm.Vars.Get(11) != 1 || vm.Vars.Get(12) != 1 || vm.Vars.Get(13) != 0)
            throw new Exception($"TC-121 batch failed: {vm.Vars.Get(10)},{vm.Vars.Get(11)},{vm.Vars.Get(12)},{vm.Vars.Get(13)}");
    }

    private static void TestOpcode211ReserveArguments()
    {
        Console.WriteLine("  Testing Opcode 211 (Reserve queue keeps call arguments)...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        // Caller 100 with the relative call address 600101 -> common event 101.
        var status = 2 | (1 << 4) | (1 << 12);
        var slots = new List<Slot>
        {
            Slot.Command(211, [600_101, status, 11, 22, 0], ["", "hello"]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(100, "Reserver", slots));
        vm.CreateInstance(100);
        vm.StepFrame();

        if (host.Reservations.Count != 1)
            throw new Exception($"211 expected one reservation, got {host.Reservations.Count}");
        var rec = host.Reservations[0];
        if (rec.Target != 101)
            throw new Exception($"211 target expected 101, got {rec.Target}");
        if (rec.Args.Length != 2 || rec.Args[0] != 11 || rec.Args[1] != 22)
            throw new Exception($"211 args mismatch: [{string.Join(",", rec.Args)}]");
        if (rec.Strings.Length != 1 || rec.Strings[0] != "hello")
            throw new Exception($"211 strings mismatch: [{string.Join(",", rec.Strings)}]");
    }

    private static void TestOpcode124EventIdentity()
    {
        Console.WriteLine("  Testing Opcode 124 field 17/18 (running event identity)...");
        var fgSlots = new List<Slot>
        {
            Slot.Command(180, [10]), // keep the foreground common event alive and blocked
            Slot.Command(172)
        };
        var bgSlots = new List<Slot>
        {
            Slot.Command(124, [2_000_000, 0x3000, 17]), // this common event id
            Slot.Command(124, [2_000_001, 0x3000, 18]), // last foreground event + 500000
            Slot.Command(172)
        };
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(100, "fg", fgSlots));
        vm.RegisterProgram(Program.FromSlots(200, "bg", bgSlots));
        vm.CreateInstance(100);
        var bg = vm.CreateInstance(200)!;
        bg.IsParallel = true;
        vm.StepFrame();
        if (vm.Vars.Get(0) != 200)
            throw new Exception($"124 field 17 expected 200, got {vm.Vars.Get(0)}");
        if (vm.Vars.Get(1) != 500_100)
            throw new Exception($"124 field 18 expected 500100, got {vm.Vars.Get(1)}");
    }

    private static void TestCallTargetResolution()
    {
        Console.WriteLine("  Testing common-event call target resolution...");
        if (EventVm.ResolveCallTarget(500_050, 9) != 50)
            throw new Exception("500050 must resolve to event 50");
        if (EventVm.ResolveCallTarget(600_101, 100) != 101)
            throw new Exception("600101 from caller 100 must resolve to 101");
        if (EventVm.ResolveCallTarget(600_094, 270) != 94)
            throw new Exception("600094 from caller 270 (absolute mode) must resolve to 94");
        if (EventVm.ResolveCallTarget(600_094, 270, relative600: true) != 264)
            throw new Exception("600094 from caller 270 (relative mode) must resolve to 264");
        if (EventVm.ResolveCallTarget(600_104, 264, relative600: true) != 268)
            throw new Exception("600104 from caller 264 (relative mode) must resolve to 268");
        if (EventVm.ResolveCallTarget(-7, 1) != null)
            throw new Exception("negative targets must be rejected");
    }

    private sealed class KeyHost : RecordingHost
    {
        public uint Held;

        public override int KeyCode(uint mask)
        {
            LastKeyMask = mask;
            return KeyCodes.Code(Held);
        }

        public override int KeyCodeAuto(uint mask, uint autoKeys)
        {
            LastKeyMask = mask;
            LastAutoKeys = autoKeys;
            return KeyCodes.Code(Held | autoKeys);
        }
    }

    private static void TestOpcode125126AutoAndBanKeys()
    {
        Console.WriteLine("  Testing Opcode 125/126/123 (input permissions)...");
        var slots = new List<Slot>
        {
            Slot.Command(125, [0]),       // no auto input for the direction phase
            Slot.Command(126, [0x1F0]),   // directions: movement blocked only
            Slot.Command(123, [2_000_000, 240]),
            Slot.Command(126, [0x2F0]),   // directions: movement + key input blocked
            Slot.Command(123, [2_000_001, 240]),
            Slot.Command(126, [0x0F0]),   // restore directions
            Slot.Command(123, [2_000_002, 240]),
            Slot.Command(125, [1]),       // auto OK
            Slot.Command(126, [0x201]),   // OK: both blocked, 125 must not bypass
            Slot.Command(123, [2_000_003, 1]),
            Slot.Command(126, [0x001]),   // restore OK
            Slot.Command(125, [0]),       // release auto input
            Slot.Command(126, [0x10000101]), // device form: deny
            Slot.Command(123, [2_000_004, 1]),
            Slot.Command(126, [0x10000001]), // device form: allow
            Slot.Command(123, [2_000_005, 1])
        };
        var prog = Program.FromSlots(92, "Keys", slots);
        var host = new KeyHost { Held = KeyCodes.Up };
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        vm.CreateInstance(92);
        vm.StepFrame();

        if (vm.Vars.Get(0) != 8)
            throw new Exception($"126 state 1 must only block walking, got {vm.Vars.Get(0)}");
        if (vm.Vars.Get(1) != 0)
            throw new Exception($"126 state 2 must block the 123 poll, got {vm.Vars.Get(1)}");
        if (vm.Vars.Get(2) != 8)
            throw new Exception($"126 state 0 must restore the key, got {vm.Vars.Get(2)}");
        if (vm.Vars.Get(3) != 0)
            throw new Exception($"126 basic deny must beat 125 auto input, got {vm.Vars.Get(3)}");
        if (vm.Vars.Get(4) != 0)
            throw new Exception($"126 device deny must block physical keys, got {vm.Vars.Get(4)}");
        if (vm.Vars.Get(5) != 8)
            throw new Exception($"126 device allow must restore, got {vm.Vars.Get(5)}");
        if (host.LastKeyMask != 1u)
            throw new Exception($"123 mask must reach the host verbatim, got {host.LastKeyMask:X}");
        if (vm.Faulted)
            throw new Exception("125/126/123 must be handled opcodes");

        // Single-direction WOLF editor bits (Down=0x10, Left=0x20, Right=0x40, Up=0x80) must map to host bits.
        var dirVm = new EventVm();
        dirVm.ApplyInputPermission(0x210); // ban WOLF Down (0x10)
        if ((dirVm.BasicAllDeny & KeyCodes.Down) == 0 || (dirVm.BasicAllDeny & KeyCodes.Up) != 0)
            throw new Exception($"126 WOLF Down (0x10) mapped incorrectly: 0x{dirVm.BasicAllDeny:X}");
        dirVm.ApplyInputPermission(0x280); // ban WOLF Up (0x80)
        if ((dirVm.BasicAllDeny & KeyCodes.Up) == 0 || (dirVm.BasicAllDeny & KeyCodes.Right) != 0)
            throw new Exception($"126 WOLF Up (0x80) mapped incorrectly: 0x{dirVm.BasicAllDeny:X}");

        // Opcode 250 writing an empty string self-var slot (CSelf5 = 1_600_005) must emit a string DbValue (not int 0).
        DbOp? last250 = null;
        var dbHost = new DelegateDbHost(op => { last250 = op; return null; });
        var dbVm = new EventVm { Host = dbHost };
        dbVm.RegisterProgram(Program.FromSlots(922, "Op250EmptyStr", new List<Slot>
        {
            Slot.Command(250, [0, 0, 0, 0, 1_600_005]),
            Slot.Command(172)
        }));
        var dbInst = dbVm.CreateInstance(922)!;
        dbInst.Self.SetStr(Magic.SelfArgBase + 5, "");
        dbVm.StepFrame();
        if (last250?.Source == null || !last250.Source.IsString || last250.Source.Str != "")
            throw new Exception($"250 empty CSelf5 write must construct string DbValue, got IsString={last250?.Source?.IsString}");

        // DeviceDeny must allow WOLF-layout direction auto-key (0x10 = Down) while still blocking physical Up.
        var devSlots = new List<Slot>
        {
            Slot.Command(126, [0x10000101]), // device deny
            Slot.Command(125, [0x10]),       // WOLF Down (16)
            Slot.Command(123, [2_000_010, 240]),
            Slot.Command(172)
        };
        var devHost = new KeyHost { Held = KeyCodes.Up };
        var devVm = new EventVm { Host = devHost };
        devVm.RegisterProgram(Program.FromSlots(921, "DevAutoDir", devSlots));
        devVm.CreateInstance(921);
        devVm.StepFrame();
        if (devVm.FilterKeyCode(KeyCodes.CodeUp) != 0)
            throw new Exception("DeviceDeny must block physical Up even when WOLF Down (0x10 == 16) is active");
        if (devVm.FilterKeyCode(KeyCodes.CodeDown) != KeyCodes.CodeDown)
            throw new Exception("DeviceDeny must allow 125 WOLF Down auto-input");
    }

    private static void TestOpcode1000ProFeatureWait()
    {
        Console.WriteLine("  Testing Opcode 1000 (ProFeature wait)...");
        var slots = new List<Slot>
        {
            Slot.Command(1000, [5 << 16, 32], ["Save/save1.jpg"]),
            Slot.Command(121, [2_000_000, 7, 0, 0x0000])
        };
        var prog = Program.FromSlots(93, "ProFeature", slots);
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(93)!;
        int budget = 100;
        var sig = vm.StepInstance(inst, ref budget);
        if (sig != Signal.Blocked || inst.Wait.Kind != WaitStateKind.Frames || inst.Wait.Frames != 2)
            throw new Exception($"1000 kind=5 should wait 2 frames, got {sig} {inst.Wait.Kind} {inst.Wait.Frames}");
        if (host.ProFeatures.Count != 1 || host.ProFeatures[0] != (5, 32))
            throw new Exception("1000 should report kind 5, 32ms");
        if (host.ProFeaturePaths.Count != 1 || host.ProFeaturePaths[0] != (5, 32, "Save/save1.jpg"))
            throw new Exception("1000 must forward the expanded screenshot path");
        vm.StepFrame();
        if (vm.Vars.Get(0) != 0)
            throw new Exception("1000 resumed before its wait elapsed");
        vm.StepFrame();
        if (vm.Vars.Get(0) != 7)
            throw new Exception($"1000 did not resume, Var 0={vm.Vars.Get(0)}");
    }

    private static void TestOpcode102CancelFlag()
    {
        Console.WriteLine("  Testing Opcode 102 cancel permission...");
        foreach ((int args, bool expected) in new[] { (0x11, false), (0x01, true) })
        {
            var slots = new List<Slot>
            {
                Slot.Command(102, [args], ["only"]),
                Slot.Command(401, [2]),
                Slot.Command(121, [2_000_000, 1, 0, 0]),
                Slot.Command(421, [0]),
                Slot.Command(121, [2_000_000, 2, 0, 0]),
                Slot.Command(499, [])
            };
            var vm = new EventVm { Host = new RecordingHost() };
            vm.RegisterProgram(Program.FromSlots(94, "Choices", slots));
            var inst = vm.CreateInstance(94)!;
            int budget = 100;
            Signal signal = vm.StepInstance(inst, ref budget);
            var host = (RecordingHost)vm.Host!;
            if (signal != Signal.Blocked || inst.Wait.Kind != WaitStateKind.Choice)
                throw new Exception("102 must block for a choice");
            if (host.ChoiceAllowsCancel.Count != 1 || host.ChoiceAllowsCancel[0] != expected)
                throw new Exception($"102 args 0x{args:X} must expose cancel={expected}");
        }
    }

    private static void TestOpcode101PairedChoice()
    {
        Console.WriteLine("  Testing Opcode 101 -> 102 same-screen prompt...");
        var pairedSlots = new List<Slot>
        {
            Slot.Command(101, [], ["Are you ready?"]),
            Slot.Command(102, [0x12], ["Yes", "No"]),
            Slot.Command(401, [2]),
            Slot.Command(121, [2_000_000, 1, 0, 0]),
            Slot.Command(421, [0]),
            Slot.Command(121, [2_000_000, -1, 0, 0]),
            Slot.Command(499, [])
        };
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(110, "PairedPrompt", pairedSlots));
        var inst = vm.CreateInstance(110)!;
        vm.StepFrame();

        if (inst.Wait.Kind != WaitStateKind.Choice)
            throw new Exception($"101 followed by 102 must block on the choice, got {inst.Wait.Kind}");
        if (host.Messages.Count != 1 || host.Messages[0].Text != "Are you ready?" || !host.Messages[0].KeepOpen)
            throw new Exception("paired 101 must keep the prompt open for the choice");
        if (host.ChoiceAllowsCancel.Count != 1 || host.ChoiceAllowsCancel[0] != false)
            throw new Exception("the adjacent 102 must still publish its own cancel permission");

        vm.StepFrame();
        if (vm.Vars.Get(0) != 1)
            throw new Exception($"auto-selected paired choice must enter case 1, got {vm.Vars.Get(0)}");

        var blockedSlots = new List<Slot>
        {
            Slot.Command(101, [], ["Wait for me"]),
            Slot.Command(103, [], ["not physically adjacent"]),
            Slot.Command(102, [2], ["A"]),
            Slot.Command(401, [2]),
            Slot.Command(499, [])
        };
        var blockedHost = new RecordingHost();
        var blockedVm = new EventVm { Host = blockedHost };
        blockedVm.RegisterProgram(Program.FromSlots(111, "SeparatedPrompt", blockedSlots));
        var blocked = blockedVm.CreateInstance(111)!;
        blockedVm.StepFrame();
        if (blocked.Wait.Kind != WaitStateKind.Message)
            throw new Exception("a non-adjacent 101 must retain the normal message wait");
        if (blockedHost.Messages.Count != 1 || blockedHost.Messages[0].KeepOpen)
            throw new Exception("only a physically adjacent 101/102 pair may keep the prompt open");
    }

    private static void TestEventEntryNotifications()
    {
        Console.WriteLine("  Testing foreground event entry notifications...");
        var child = Program.FromSlots(50, "EntryChild", new List<Slot>
        {
            Slot.Command(172)
        });
        var parent = Program.FromSlots(40, "EntryParent", new List<Slot>
        {
            Slot.Command(210, [500_050, 0]),
            Slot.Command(172)
        });
        var parallel = Program.FromSlots(60, "EntryParallel", new List<Slot>
        {
            Slot.Command(180, [2]),
            Slot.Command(172)
        });
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(child);
        vm.RegisterProgram(parent);
        vm.RegisterProgram(parallel);
        vm.CreateInstance(40);
        vm.CreateInstance(60)!.IsParallel = true;

        vm.StepFrame();
        vm.StepFrame();
        if (host.EnteredEvents.Count != 2 || host.EnteredEvents[0] != 40 || host.EnteredEvents[1] != 50)
            throw new Exception($"event entries expected [40,50], got [{string.Join(",", host.EnteredEvents)}]");
        if (host.EnteredEventRoots.Count != 2 || !host.EnteredEventRoots[0] || host.EnteredEventRoots[1])
            throw new Exception("event entry roots must report [root, child]");
        vm.StepFrame();
        if (host.EnteredEvents.Count != 2)
            throw new Exception("EventEntered must fire once per foreground instance");
    }

    private static void TestMaxCallDepth()
    {
        Console.WriteLine("  Testing MaxCallDepth...");
        var prog = Program.FromSlots(97, "Recurse", new List<Slot> { Slot.Command(210, [500_097, 0]) });
        var host = new RecordingHost();
        var vm = new EventVm { Host = host, StrictMode = true };
        vm.RegisterProgram(prog);
        vm.CreateInstance(97);
        for (int i = 0; i < 200 && vm.RejectedSpawnCount == 0; i++)
        {
            vm.StepFrame();
        }
        if (vm.RejectedSpawnCount < 1)
            throw new Exception("recursive 210 never hit MaxCallDepth");
        if (!host.Logs.Exists(l => l.Contains("max call depth")))
            throw new Exception("depth rejection must be logged");
        if (vm.Faulted)
            throw new Exception("depth rejection must not fault the VM");
    }

    private static void TestOpcode122StringOps()
    {
        Console.WriteLine("  Testing Opcode 122 string operations...");
        var slots = new List<Slot>
        {
            Slot.Command(122, [3_000_000, 0x0000, 0], ["Data\\SE/click.ogg"]),
            Slot.Command(122, [3_000_000, 0x0800, 0], ["Data\\"]),
            Slot.Command(122, [3_000_001, 0x0100, 0], ["A"]),
            Slot.Command(122, [3_000_001, 0x0100, 0], ["B"]),
            Slot.Command(122, [3_000_002, 0x0900, 0], ["\\n", "\n"]),
            Slot.Command(122, [3_000_003, 0x0200 | 1, 3_000_002]),
            Slot.Command(122, [3_000_004, 0x0400 | 1, 3_000_002]),
            Slot.Command(122, [3_000_005, 0x0500 | 1, 3_000_002]),
            Slot.Command(122, [3_000_006, 0x0500, 0, 1], ["<<GET_FILE_EXIST>>./save/save0.jpg"]),
            Slot.Command(122, [3_000_007, 0x0A00, 0], [":"]),
            Slot.Command(122, [3_000_008, 0x0B00, 0], [":"]),
            Slot.Command(122, [1_600_010, 0x0011, 3_000_000]),
        };
        var prog = Program.FromSlots(98, "Op122", slots);
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(98)!;
        vm.Vars.SetStr(2, "row1\\nrow2_xy");
        vm.Vars.SetStr(7, "left:right");
        vm.Vars.SetStr(8, "left:right");
        inst.Self.Set(610, 3_000_009);
        vm.StepFrame();
        if (vm.Vars.GetStr(0) != "SE/click.ogg")
            throw new Exception($"0x0800 remove failed: '{vm.Vars.GetStr(0)}'");
        if (vm.Vars.GetStr(1) != "AB")
            throw new Exception($"0x0100 append failed: '{vm.Vars.GetStr(1)}'");
        if (vm.Vars.GetStr(3) != "row1")
            throw new Exception($"0x0201 first-line head failed: '{vm.Vars.GetStr(3)}'");
        if (vm.Vars.GetStr(4) != "r" || vm.Vars.GetStr(5) != "y" || vm.Vars.GetStr(2) != "ow2_x")
            throw new Exception($"cut char failed: 2='{vm.Vars.GetStr(2)}' 4='{vm.Vars.GetStr(4)}' 5='{vm.Vars.GetStr(5)}'");
        if (vm.Vars.GetStr(6) != "0")
            throw new Exception($"GET_FILE_EXIST failed: '{vm.Vars.GetStr(6)}'");
        if (vm.Vars.GetStr(7) != "left" || vm.Vars.GetStr(8) != "right")
            throw new Exception($"0xA00/0xB00 failed: 7='{vm.Vars.GetStr(7)}' 8='{vm.Vars.GetStr(8)}'");
        if (vm.Vars.GetStr(9) != "SE/click.ogg")
            throw new Exception($"0x0011 indirect target failed: '{vm.Vars.GetStr(9)}'");
    }

    private static void TestOpcode140SoundCellSplitAndEval()
    {
        Console.WriteLine("  Testing Opcode 140 multiline cell split & param eval...");
        var slots = new List<Slot>
        {
            Slot.Command(140, [0x0200_0020, 0, 0, 0, 0, 0], ["SystemFile/[SE]Enter.ogg\r\n90\r\n125"]),
            Slot.Command(140, [0x0200_0000, 0, 0, 0, 0, 100], ["BGM/quiet.ogg"]),
            Slot.Command(140, [0x0200_0020, 1_600_001, 0, 0, 1_600_002, 1_600_003], ["SystemFile/[SE]Dyn.ogg"]),
        };
        var prog = Program.FromSlots(99, "Op140Split", slots);
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(99)!;
        inst.Self.Set(601, 15);
        inst.Self.Set(602, 75);
        inst.Self.Set(603, 110);
        vm.StepFrame();
        if (host.Sounds.Count != 3)
            throw new Exception($"expected 3 sounds, got {host.Sounds.Count}");
        var s0 = host.Sounds[0];
        if (s0.File != "SystemFile/[SE]Enter.ogg" || s0.Volume != 90 || s0.Tempo != 125)
            throw new Exception($"multiline SE cell failed: file='{s0.File}' vol={s0.Volume} tempo={s0.Tempo}");
        var s1 = host.Sounds[1];
        if (s1.File != "BGM/quiet.ogg" || s1.Volume != 0)
            throw new Exception($"single-line vol=0 must be preserved: file='{s1.File}' vol={s1.Volume}");
        var s2 = host.Sounds[2];
        if (s2.Frames != 15 || s2.Volume != 75 || s2.Tempo != 110)
            throw new Exception($"param EvalNum failed: frames={s2.Frames} vol={s2.Volume} tempo={s2.Tempo}");
    }

    private static void TestTextExpanderCselfEmptyAndSdb()
    {
        Console.WriteLine("  Testing TextExpander empty \\cself[5..9] and \\sdb kind...");
        var prog = Program.FromSlots(100, "Expand", new List<Slot>());
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        var inst = vm.CreateInstance(100)!;
        inst.Self.Set(600, 14);
        string expanded = TextExpander.Expand("\\f[\\cself[0]]<\\cself[7]>", inst, vm.Vars, (_, _, _, _) => "");
        if (expanded != "\\f[14]<>")
            throw new Exception($"\\cself[7] must stay empty when unset, got '{expanded}'");
        int seenKind = -1;
        TextExpander.Expand("\\sdb[1:2:3]", inst, vm.Vars, (k, _, _, _) => { seenKind = k; return "ok"; });
        if (seenKind != 1)
            throw new Exception($"\\sdb must pass kind=1 (Sys), got {seenKind}");
    }

    private sealed class StateQueryHost : RecordingHost
    {
        public int PictureAnswer;
        public int CharacterAnswer;
        public override int? PictureState(int number, int field) => PictureAnswer;
        public override int? CharacterState(int target, int field) => CharacterAnswer;
    }

    private static void TestOpcode124PictureDivideAssign()
    {
        Console.WriteLine("  Testing Opcode 124 (0x4400 divide-assign, div-by-0, 0x4000, 0x1200)...");
        var slots = new List<Slot>
        {
            Slot.Command(124, [1_600_091, 0x4400, 1, 2]), // /= PictureState (240)
            Slot.Command(124, [1_600_092, 0x4000, 1, 2]), // = PictureState (240)
            Slot.Command(124, [1_600_093, 0x1200, 0, 7]), // = CharacterState (15, NOT /=)
            Slot.Command(172)
        };
        var host = new StateQueryHost { PictureAnswer = 240, CharacterAnswer = 15 };
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(101, "Op124Div", slots));
        var inst = vm.CreateInstance(101)!;
        inst.Self.Set(691, 4800);
        inst.Self.Set(692, 4800);
        inst.Self.Set(693, 4800);
        vm.StepFrame();

        if (inst.Self.Get(691) != 20)
            throw new Exception($"124 0x4400 expected 20, got {inst.Self.Get(691)}");
        if (inst.Self.Get(692) != 240)
            throw new Exception($"124 0x4000 expected 240, got {inst.Self.Get(692)}");
        if (inst.Self.Get(693) != 15)
            throw new Exception($"124 0x1200 must not compound-divide, expected 15, got {inst.Self.Get(693)}");

        // Divide-by-zero leaves target unchanged.
        host.PictureAnswer = 0;
        var divZeroSlots = new List<Slot>
        {
            Slot.Command(124, [1_600_091, 0x4400, 1, 2]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(102, "Op124DivZero", divZeroSlots));
        var inst2 = vm.CreateInstance(102)!;
        inst2.Self.Set(691, 4800);
        vm.StepFrame();
        if (inst2.Self.Get(691) != 4800)
            throw new Exception($"124 0x4400 div-by-0 expected 4800, got {inst2.Self.Get(691)}");
    }

    private static void TestOpcode112StringConditionFlags()
    {
        Console.WriteLine("  Testing Opcode 112 (StringCondition flags !=, Contains, StartsWith, bit 24 RHS var)...");
        // Branch 1: cself[6] StartsWith cself[8] (0x31186a06, rhs in params[3])
        // Branch 2: cself[6] == "" (0x00186a06)
        // Else:     fallthrough
        var multiSlots = new List<Slot>
        {
            Slot.Command(112, [0x12, 0x3100_0000 | 1_600_006, 1_600_006, 1_600_008], ["unused", "", "", ""]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_000, 1, 0, 0]),
            Slot.Command(401, [2]),
            Slot.Command(121, [2_000_000, 2, 0, 0]),
            Slot.Command(420, [0]),
            Slot.Command(121, [2_000_000, 3, 0, 0]),
            Slot.Command(499),
            Slot.Command(172)
        };
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(103, "Op112Multi", multiSlots));

        var i1 = vm.CreateInstance(103)!;
        i1.Self.SetStr(606, "\nrest");
        i1.Self.SetStr(608, "\n");
        vm.StepFrame();
        if (vm.Vars.Get(0) != 1)
            throw new Exception($"112 StartsWith var RHS expected branch 1, got {vm.Vars.Get(0)}");

        var i2 = vm.CreateInstance(103)!;
        i2.Self.SetStr(606, "");
        i2.Self.SetStr(608, "\n");
        vm.StepFrame();
        if (vm.Vars.Get(0) != 2)
            throw new Exception($"112 eq empty expected branch 2, got {vm.Vars.Get(0)}");

        // != (0x10), Contains (0x20), == var RHS (0x01), and SysVar/StrVar high-flag stripping.
        var opSlots = new List<Slot>
        {
            Slot.Command(112, [0x01, 0x1000_0000 | 1_600_007], [""]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_001, 10, 0, 0]),
            Slot.Command(499),
            Slot.Command(112, [0x01, 0x2000_0000 | 3_000_001], ["needle"]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_002, 20, 0, 0]),
            Slot.Command(499),
            Slot.Command(112, [0x01, 0x0100_0000 | 1_600_007, 1_600_005]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_003, 30, 0, 0]),
            Slot.Command(499),
            Slot.Command(112, [0x01, 0x1100_0000 | 9_900_025, 1_600_005]),
            Slot.Command(401, [1]),
            Slot.Command(121, [2_000_004, 40, 0, 0]),
            Slot.Command(499),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(104, "Op112Ops", opSlots));
        var i3 = vm.CreateInstance(104)!;
        i3.Self.SetStr(607, "icon.png");
        i3.Self.SetStr(605, "icon.png");
        vm.Vars.SetStr(1, "hay-needle-stack");
        vm.StepFrame();
        if (vm.Vars.Get(1) != 10)
            throw new Exception($"112 != expected 10, got {vm.Vars.Get(1)}");
        if (vm.Vars.Get(2) != 20)
            throw new Exception($"112 Contains (flagged StrVar) expected 20, got {vm.Vars.Get(2)}");
        if (vm.Vars.Get(3) != 30)
            throw new Exception($"112 bit24 == var expected 30, got {vm.Vars.Get(3)}");
        if (vm.Vars.Get(4) != 40)
            throw new Exception($"112 flagged SysVar 9900025 != expected 40, got {vm.Vars.Get(4)}");
    }

    private sealed class SaveTargetRecordingHost : RecordingHost
    {
        public readonly List<SaveOp> SaveOps = new();
        public override int? SaveLoad(SaveOp op)
        {
            SaveOps.Add(op);
            return op.Opcode == 221 ? 7 : null;
        }
    }

    private static void TestOpcode221_222CSelfStringPath()
    {
        Console.WriteLine("  Testing Opcode 221/222 CSelf5 (1600005) string-path routing...");
        var host = new SaveTargetRecordingHost();
        var vm = new EventVm { Host = host };
        var slots = new List<Slot>
        {
            Slot.Command(222, [42, 1_600_005, 2_000_300, 0]),
            Slot.Command(221, [2_000_010, 1_600_005, 2_000_300, 0]),
            Slot.Command(221, [2_000_011, 1_600_000, 2_000_300, 0]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(105, "Op221CSelf5", slots));
        var inst = vm.CreateInstance(105)!;
        inst.Self.SetStr(605, "Save/System.sav");
        inst.Self.Set(600, 2);
        vm.StepFrame();

        if (host.SaveOps.Count != 3)
            throw new Exception($"expected 3 SaveOps, got {host.SaveOps.Count}");
        if (!host.SaveOps[0].File.IsPath || host.SaveOps[0].File.Path != "Save/System.sav")
            throw new Exception($"222 CSelf5 expected Path 'Save/System.sav', got IsPath={host.SaveOps[0].File.IsPath} Path='{host.SaveOps[0].File.Path}' Slot={host.SaveOps[0].File.Slot}");
        if (!host.SaveOps[1].File.IsPath || host.SaveOps[1].File.Path != "Save/System.sav")
            throw new Exception($"221 CSelf5 expected Path 'Save/System.sav', got IsPath={host.SaveOps[1].File.IsPath} Path='{host.SaveOps[1].File.Path}' Slot={host.SaveOps[1].File.Slot}");
        if (host.SaveOps[2].File.IsPath || host.SaveOps[2].File.Slot != 2)
            throw new Exception($"221 CSelf0 expected Slot 2, got IsPath={host.SaveOps[2].File.IsPath} Slot={host.SaveOps[2].File.Slot}");
    }

    private static void TestOpcode150BlendMode()
    {
        Console.WriteLine("  Testing Opcode 150 BlendMode decoding...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        // head: bits 8..11 = 1 (Add), action = 0 (Draw), kind = 0
        var slots = new List<Slot>
        {
            Slot.Command(150, [0x0100, 10], ["Picture/fx.png"]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(106, "Op150Blend", slots));
        vm.CreateInstance(106);
        vm.StepFrame();
        if (host.Pictures.Count != 1 || host.Pictures[0].BlendMode != 1)
            throw new Exception($"150 BlendMode expected 1, got {(host.Pictures.Count > 0 ? host.Pictures[0].BlendMode : -1)}");
    }

    private static void TestOpcode150DynamicFile()
    {
        Console.WriteLine("  Testing Opcode 150 fileString/SysStr path expansion...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.Vars.Set(3, 7);
        vm.Vars.SetStr(5, "Picture/tip_\\v[3].png");
        vm.Vars.SetSysStr(4, "Picture/sys.png");
        var slots = new List<Slot>
        {
            // head kind=1 (fileString), cmd=0 (Draw); [11] is the string source.
            Slot.Command(150, [0x0010, 10, 0, 1, 1, 1, 255, 100, 100, 100, 0, 3_000_005]),
            Slot.Command(150, [0x0010, 11, 0, 1, 1, 1, 255, 100, 100, 100, 0, 9_000_004]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(109, "Op150FileString", slots));
        vm.CreateInstance(109);
        vm.StepFrame();
        if (host.Pictures.Count != 2)
            throw new Exception($"150 fileString expected 2 pictures, got {host.Pictures.Count}");
        if (host.Pictures[0].File != "Picture/tip_7.png")
            throw new Exception($"150 StrVar path expected expansion, got '{host.Pictures[0].File}'");
        if (host.Pictures[1].File != "Picture/sys.png")
            throw new Exception($"150 SysStrVar path expected expansion, got '{host.Pictures[1].File}'");
    }

    private static void TestOpcode101MultiDigitFacePrefix()
    {
        Console.WriteLine("  Testing Opcode 101 multi-digit @<n> face prefix parsing...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        var slots = new List<Slot>
        {
            Slot.Command(101, [], ["@10你好"]),
            Slot.Command(101, [], ["@2hello"]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(110, "Op101MultiDigitFace", slots));
        var inst = vm.CreateInstance(110)!;
        vm.StepFrame();
        if (host.Messages.Count != 1)
            throw new Exception($"@10 prefix expected 1 message, got {host.Messages.Count}");
        if (host.Messages[0].Face != 10 || host.Messages[0].Text != "你好")
            throw new Exception($"@10 prefix expected face=10 text='你好', got face={host.Messages[0].Face} text='{host.Messages[0].Text}'");
        inst.Wait.Clear();
        vm.StepFrame();
        if (host.Messages.Count != 2 || host.Messages[1].Face != 2 || host.Messages[1].Text != "hello")
            throw new Exception($"@2 prefix expected face=2 text='hello', got count={host.Messages.Count} face={(host.Messages.Count > 1 ? host.Messages[1].Face : null)} text='{(host.Messages.Count > 1 ? host.Messages[1].Text : "")}'");
    }

    private static void TestOpcode174ReturnToTitleDeferral()
    {
        Console.WriteLine("  Testing Opcode 174 ReturnToTitle frame-end deferral...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(107, "Op174", new List<Slot> { Slot.Command(174) }));
        vm.RegisterProgram(Program.FromSlots(108, "After174", new List<Slot> { Slot.Command(121, [2_000_050, 99, 0, 0]), Slot.Command(172) }));
        vm.CreateInstance(107);
        vm.CreateInstance(108);
        vm.StepFrame();

        if (!vm.ReturnToTitleRequested)
            throw new Exception("174 must set ReturnToTitleRequested");
        if (vm.Instances.Count != 0)
            throw new Exception($"174 must clear all VM instances, remaining={vm.Instances.Count}");
        if (host.ReturnToTitleCount != 0)
            throw new Exception($"174 must not call Host.ReturnToTitle synchronously, count={host.ReturnToTitleCount}");
        if (vm.Vars.Get(50) != 0)
            throw new Exception("174 must stop subsequent instances in the same frame");
    }

    private sealed class FileCheckHost : NullHost
    {
        public string Existing = "./save/save1.jpg";
        public override bool FileExists(string path) => string.Equals(path, Existing, StringComparison.OrdinalIgnoreCase);
    }

    private static void TestGetFileExistHostDelegate()
    {
        Console.WriteLine("  Testing Opcode 122 GET_FILE_EXIST host lookup...");
        var host = new FileCheckHost();
        var vm = new EventVm { Host = host };
        var slots = new List<Slot>
        {
            Slot.Command(122, [3_000_001, 0x0500, 0, 1], ["<<GET_FILE_EXIST>>./save/save1.jpg"]),
            Slot.Command(122, [3_000_002, 0x0500, 0, 1], ["<<GET_FILE_EXIST>>./save/save9.jpg"]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(111, "GetFileExist", slots));
        vm.CreateInstance(111);
        vm.StepFrame();
        if (vm.Vars.GetStr(1) != "1" || vm.Vars.GetStr(2) != "0")
            throw new Exception($"GET_FILE_EXIST mismatch: s1='{vm.Vars.GetStr(1)}' s2='{vm.Vars.GetStr(2)}'");
    }

    private static void TestPerInstanceBudgetPreventsStarvation()
    {
        Console.WriteLine("  Testing per-instance frame budget prevents starvation...");
        var spinSlots = new List<Slot>
        {
            Slot.Command(170), // unconditional loop
            Slot.Command(121, [2_000_001, 1, 0, 0]),
            Slot.Command(498),
        };
        var tailSlots = new List<Slot>
        {
            Slot.Command(121, [2_000_050, 77, 0, 0]),
            Slot.Command(172)
        };
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(112, "SpinLoop", spinSlots));
        vm.RegisterProgram(Program.FromSlots(113, "TailWork", tailSlots));
        vm.CreateInstance(112);
        vm.CreateInstance(113);
        vm.StepFrame();
        if (vm.Vars.Get(50) != 77)
            throw new Exception($"runaway loop starved trailing coroutine: var50={vm.Vars.Get(50)}");
        if (!host.Logs.Exists(l => l.Contains("per-instance frame budget")))
            throw new Exception("per-instance budget limit should log a diagnostic warning");
    }

    private static void TestParallelEnteredInstanceDoesNotLeak()
    {
        Console.WriteLine("  Testing _enteredInstanceIds stays bounded across parallel & finished events...");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(114, "ParallelTick", new List<Slot> { Slot.Command(172) }));
        vm.RegisterProgram(Program.FromSlots(115, "FgTick", new List<Slot> { Slot.Command(172) }));
        for (int i = 0; i < 40; i++)
        {
            var p = vm.CreateInstance(114)!;
            p.IsParallel = true;
            vm.CreateInstance(115);
            vm.StepFrame();
        }
        if (host.EnteredEvents.Contains(114))
            throw new Exception("parallel instance must not fire EventEntered");
        if (vm.EnteredInstanceCount != 0)
            throw new Exception($"_enteredInstanceIds leaked {vm.EnteredInstanceCount} retired ids");
    }

    private static void TestSoundChannelLayoutMatchesPresentation()
    {
        Console.WriteLine("  Testing SoundChannel numeric contract matches AudioManager (BGM=0, BGS=1, SE=2)...");
        if ((int)SoundChannel.Bgm != 0 || (int)SoundChannel.Bgs != 1 || (int)SoundChannel.Se != 2)
            throw new Exception($"SoundChannel values must match AudioManager (0/1/2), got {(int)SoundChannel.Bgm}/{(int)SoundChannel.Bgs}/{(int)SoundChannel.Se}");
        var host = new RecordingHost();
        var vm = new EventVm { Host = host };
        var slots = new List<Slot>
        {
            Slot.Command(140, [0x0200_0000, 0, 0, 0, 100, 100], ["BGM/t.ogg"]),
            Slot.Command(140, [0x0200_0010, 0, 0, 0, 75, 100], ["BGM/bgs.ogg"]),
            Slot.Command(140, [0x0200_0020, 0, 0, 0, 90, 100], ["SE/s.mp3"]),
        };
        vm.RegisterProgram(Program.FromSlots(116, "SoundCh", slots));
        vm.CreateInstance(116);
        vm.StepFrame();
        if (host.Sounds.Count != 3 || host.Sounds[0].Channel != SoundChannel.Bgm
            || host.Sounds[1].Channel != SoundChannel.Bgs || host.Sounds[2].Channel != SoundChannel.Se)
        {
            throw new Exception("opcode 140 soundType 0/1/2 must map to Bgm/Bgs/Se");
        }
    }

    private static void TestOpcode123BlockingKeyWaitAndDebounce()
    {
        Console.WriteLine("  Testing Opcode 123 (0x80 blocking key wait + release debounce)...");
        var host = new KeyHost { Held = KeyCodes.Ok };
        var vm = new EventVm { Host = host, BlockOnKeyWait = true };
        var slots = new List<Slot>
        {
            Slot.Command(123, [2_000_060, 145]), // 0x80 wait | 0x10 Ok | 0x01 Dir
            Slot.Command(121, [2_000_061, 99, 0, 0x0000]),
            Slot.Command(172)
        };
        vm.RegisterProgram(Program.FromSlots(117, "Blocking123", slots));
        var inst = vm.CreateInstance(117)!;

        vm.StepFrame();
        if (inst.Wait.Kind != WaitStateKind.KeyInput || !inst.Wait.KeyRequireRelease || vm.Vars.Get(61) != 0)
            throw new Exception("blocking 123 entered with key held must require release before advancing");

        // Still held: must stay blocked.
        vm.StepFrame();
        if (inst.Wait.Kind != WaitStateKind.KeyInput || vm.Vars.Get(61) != 0)
            throw new Exception("blocking 123 must stay blocked while opening key remains held");

        // Release on next frame: arms without completing until a fresh press arrives.
        host.Held = 0;
        vm.StepFrame();
        if (inst.Wait.Kind != WaitStateKind.KeyInput || inst.Wait.KeyRequireRelease || vm.Vars.Get(61) != 0)
            throw new Exception("releasing held key should clear requireRelease and await a fresh press");

        // Fresh press unlocks and writes keycode.
        host.Held = KeyCodes.Ok;
        vm.StepFrame();
        if (vm.Vars.Get(60) != KeyCodes.CodeOk || vm.Vars.Get(61) != 99)
            throw new Exception($"fresh press should resume blocking 123: v60={vm.Vars.Get(60)} v61={vm.Vars.Get(61)}");

        // Blocking op123 receiving op125 auto-key must deliver directly without blocking/requiring release.
        var autoHost = new KeyHost();
        var autoVm = new EventVm { Host = autoHost, BlockOnKeyWait = true };
        var autoSlots = new List<Slot>
        {
            Slot.Command(125, [0x01]),       // auto OK
            Slot.Command(123, [2_000_062, 145]), // 0x80 blocking | 0x10 Ok | 0x01 Dir
            Slot.Command(121, [2_000_063, 77, 0, 0x0000]),
            Slot.Command(172)
        };
        autoVm.RegisterProgram(Program.FromSlots(118, "AutoBlocking123", autoSlots));
        autoVm.CreateInstance(118);
        autoVm.StepFrame();
        if (autoVm.Vars.Get(62) != KeyCodes.CodeOk || autoVm.Vars.Get(63) != 77 || autoVm.Instances.Count != 0)
            throw new Exception($"blocking 123 with 125 auto-key must deliver immediately: v62={autoVm.Vars.Get(62)} v63={autoVm.Vars.Get(63)} inst={autoVm.Instances.Count}");
    }

    private static void TestOpcode213LabelJumpPopsLoops()
    {
        Console.WriteLine("  Testing Opcode 213 label jump pops exited loop frames...");
        var slots = new List<Slot>
        {
            Slot.Command(179, [3]),                       // pc 0: outer loop
            Slot.Command(179, [3]),                       // pc 1: inner loop
            Slot.Command(213, null, ["MID"]),             // pc 2: jump out of inner loop, still inside outer
            Slot.Command(498),                            // pc 3: end inner
            Slot.Command(212, null, ["MID"]),             // pc 4: label MID -> pc 5
            Slot.Command(121, [2_000_000, 1, 0, 0x0100]), // pc 5: v0 += 1
            Slot.Command(213, null, ["OUT"]),             // pc 6: jump out of outer loop
            Slot.Command(498),                            // pc 7: end outer
            Slot.Command(212, null, ["OUT"]),             // pc 8: label OUT -> pc 9
            Slot.Command(171),                            // pc 9: break (must be no-op since loops were popped)
            Slot.Command(121, [2_000_001, 55, 0, 0]),     // pc 10: v1 = 55
            Slot.Command(172)                             // pc 11: finish
        };
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(119, "JumpPopLoop", slots));
        var inst = vm.CreateInstance(119)!;
        vm.StepFrame();
        if (vm.Vars.Get(0) != 1 || vm.Vars.Get(1) != 55 || inst.Loops.Count != 0)
            throw new Exception($"op213 must pop exited loops: v0={vm.Vars.Get(0)} v1={vm.Vars.Get(1)} loops={inst.Loops.Count}");
    }

    private static void TestRoundRobinBudgetPreventsTotalStarvation()
    {
        Console.WriteLine("  Testing round-robin scheduler prevents total 200_000 budget starvation...");
        var spinSlots = new List<Slot>
        {
            Slot.Command(170),
            Slot.Command(121, [2_000_001, 1, 0, 0]),
            Slot.Command(498),
        };
        var tailSlots = new List<Slot>
        {
            Slot.Command(121, [2_000_050, 88, 0, 0]),
            Slot.Command(172)
        };
        var vm = new EventVm();
        vm.RegisterProgram(Program.FromSlots(120, "Spin", spinSlots));
        vm.RegisterProgram(Program.FromSlots(121, "Worker", tailSlots));
        for (int i = 0; i < 5; i++) vm.CreateInstance(120);
        vm.CreateInstance(121);
        vm.StepFrame(); // frame 1: first 4 spins consume 200_000 budget; worker waits
        if (vm.Vars.Get(50) != 0)
            throw new Exception("frame 1 expected 4 spins to exhaust 200_000 budget before index 5");
        vm.StepFrame(); // frame 2: cursor resumes at index 4 -> runs spin[4] and worker[5]
        if (vm.Vars.Get(50) != 88)
            throw new Exception($"frame 2 round-robin must run trailing worker coroutine, got v50={vm.Vars.Get(50)}");
    }

    private static void TestStopAllAnd174Cleanup()
    {
        Console.WriteLine("  Testing StopAll transition reset and op174 entered-set cleanup...");
        var prog = Program.FromSlots(122, "TransitionAnd174", new List<Slot>
        {
            Slot.Command(160, [1, 3]),
            Slot.Command(161),
            Slot.Command(174)
        });
        var vm = new EventVm();
        vm.RegisterProgram(prog);
        vm.CreateInstance(122);
        vm.StepFrame();
        if (vm.EnteredInstanceCount != 0 || vm.Instances.Count != 0
            || vm.TransitionPrepared || vm.TransitionPhase != 0)
            throw new Exception($"op174 must clear entered instances, instances and transition state: entered={vm.EnteredInstanceCount} inst={vm.Instances.Count} prep={vm.TransitionPrepared}");

        vm.ReturnToTitleRequested = false;
        var prep = Program.FromSlots(123, "PrepOnly", new List<Slot>
        {
            Slot.Command(160, [2, 5]),
            Slot.Command(161),
            Slot.Command(180, [10])
        });
        vm.RegisterProgram(prep);
        vm.CreateInstance(123);
        vm.StepFrame();
        if (!vm.TransitionPrepared || vm.TransitionPhase != 1)
            throw new Exception("precondition failed: TransitionPrepared should be true");
        vm.StopAll();
        if (vm.TransitionPrepared || vm.TransitionPhase != 0 || vm.EnteredInstanceCount != 0)
            throw new Exception($"StopAll must reset transition state and entered ids: prep={vm.TransitionPrepared} phase={vm.TransitionPhase}");

        var endGame = Program.FromSlots(126, "TransitionAnd175", new List<Slot>
        {
            Slot.Command(160, [1, 4]),
            Slot.Command(161),
            Slot.Command(175)
        });
        vm.RegisterProgram(endGame);
        vm.CreateInstance(126);
        vm.StepFrame();
        if (!vm.EndGameRequested || vm.TransitionPrepared || vm.TransitionPhase != 0)
            throw new Exception($"op175 must reset transition state: prep={vm.TransitionPrepared} phase={vm.TransitionPhase}");

        vm.EndGameRequested = false;
        var autoKeyHost = new AutoKeyTestHost();
        vm.Host = autoKeyHost;
        var injector = Program.FromSlots(127, "AutoKeyInject", new List<Slot>
        {
            Slot.Command(125, [1]),
            Slot.Command(172)
        });
        var waiter = Program.FromSlots(128, "AutoKeyWait", new List<Slot>
        {
            Slot.Command(123, [2_000_000, 0x10]),
            Slot.Command(180, [10])
        });
        vm.RegisterProgram(injector);
        vm.RegisterProgram(waiter);
        vm.CreateInstance(127);
        var autoInst = vm.CreateInstance(128)!;
        vm.StepFrame();
        if (vm.AutoKeysDeliveredCount != 1)
            throw new Exception($"expected 1 delivered auto-key entry, got {vm.AutoKeysDeliveredCount}");
        vm.StopInstance(autoInst.Id);
        if (vm.AutoKeysDeliveredCount != 0)
            throw new Exception($"StopInstance must clear _autoKeysDelivered, got {vm.AutoKeysDeliveredCount}");
    }

    private sealed class AutoKeyTestHost : NullHost
    {
        public override int KeyCodeAuto(uint mask, uint autoKeys) => autoKeys != 0 ? KeyCodes.CodeOk : 0;
    }

    private static void TestMalformedSlotBoundsAndCallStrings()
    {
        Console.WriteLine("  Testing malformed slot bounds and 210 strCount clamping...");
        var callee = Program.FromSlots(124, "Callee", new List<Slot> { Slot.Command(180, [2]) });
        var malformed = Program.FromSlots(125, "Malformed", new List<Slot>
        {
            Slot.Command(121, [2_000_000]), // short params (<4)
            Slot.Command(111, [3, 2_000_000]), // count=3 with short params
            Slot.Command(499),
            Slot.Command(210, [500_124, (10 << 4)], ["", "s0", "s1", "s2", "s3"]),
            Slot.Command(172)
        });
        var vm = new EventVm();
        vm.RegisterProgram(callee);
        vm.RegisterProgram(malformed);
        vm.CreateInstance(125);
        vm.StepFrame();
        if (vm.Instances.Count < 1)
            throw new Exception("210 should spawn callee without throwing on strCount=10");
        var child = vm.Instances.Find(i => i.EventId == 124)!;
        for (int k = 0; k < 4; k++)
        {
            if (child.Self.GetStr(Magic.SelfArgBase + 5 + k) == null)
                throw new Exception("child self string slot must never be null");
        }
    }

    private sealed class StubVmSession : IVmSession
    {
        public EventVm Vm { get; } = new();
        public MapRuntime? Map { get; set; }
        public int LeaderX { get; set; }
        public int LeaderY { get; set; }
        public int LeaderFacing { get; set; } = 2;
        public uint MovementDeny => Vm.MovementDeny;
        public uint BasicAllDeny => Vm.BasicAllDeny;
        public bool ChoiceAllowsCancel { get; set; } = true;
        public string MapPath(int mapId) => "";
        public bool ActionKeyRequested { get; set; }
        public bool Faulted => Vm.Faulted;
        public string FaultReason => Vm.FaultReason ?? "";
        public int ResetSessionCalls { get; private set; }
        public GameDatabase? Db { get; set; }
        public void StepFrame() => Vm.StepFrame();
        public void StartReservedEvent() { }
        public void StopAll() => Vm.StopAll();
        public void ResetDiagnostics() => Vm.ResetDiagnostics();
        public void ResetSessionState()
        {
            ResetSessionCalls++;
            Vm.Vars.Clear();
            Vm.ResetInputPermissions();
            Db?.ClearOverlays();
        }
        public void ReturnToTitle() { }
        public bool IsWaitingForChoice() => false;
        public bool IsWaitingForMessage() => false;
        public string[] GetCurrentChoices() => Array.Empty<string>();
        public void SelectChoice(int index) { }
        public void CancelChoice() { }
        public void DismissMessage() { }
        public void RestoreDialogue(string? message, string[] choices, bool allowCancel = true) { }
        public DbValue? Database(DbOp op) => Db?.Execute(op);
        public void ApplySavedCells(IReadOnlyList<LegacySaveImporter.DatabaseCell> cells) { }
        public Func<SaveOp, int?>? SaveLoadHandler { get; set; }
    }

    private sealed class StubSessionView : IGameSessionView
    {
        public int HideMessageCalls { get; private set; }
        public int HideChoicesCalls { get; private set; }
        public void SetStatus(string text) { }
        public void SetMapHud(int leaderX, int leaderY, string text) { }
        public void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY) { }
        public void ShowMessage(string text, int face) { }
        public void HideMessage() => HideMessageCalls++;
        public void ShowChoices(string[] options, int selected) { }
        public void HideChoices() => HideChoicesCalls++;
        public void ClearPictures() { }
        public void RestorePictures(IReadOnlyList<SavedPicture> pictures) { }
        public IReadOnlyList<SavedPicture> SnapshotPictures() => Array.Empty<SavedPicture>();
        public void RestoreAudio(IReadOnlyList<SavedAudio> audio) { }
        public IReadOnlyList<SavedAudio> SnapshotAudio() => Array.Empty<SavedAudio>();
        public bool HasMessage => false;
        public string MessageText => "";
    }

    private static void TestGameSessionRestoreAndReset()
    {
        Console.WriteLine("  Testing GameSession RestoreInstances (IsParallel/RetainAfterFinish) and StartTitle ResetSessionState...");
        string tmpDir = System.IO.Path.Combine(System.IO.Path.GetTempPath(), $"lwsmf_session_unit_{Guid.NewGuid():N}");
        System.IO.Directory.CreateDirectory(tmpDir);
        try
        {
            string progJson = """
            {
              "common_events": [
                { "id": 50, "title": "NormalCe", "slots": [{ "op": 180, "params": [10] }, { "op": 172 }] },
                { "id": 51, "title": "ChildCe", "slots": [{ "op": 180, "params": [10] }, { "op": 172 }] },
                { "id": 52, "title": "ParallelCe", "run_condition_op_var_val": [2, 2, 2000000, 1], "slots": [{ "op": 180, "params": [10] }, { "op": 172 }] },
                { "id": 242, "title": "BattleCommandCe", "slots": [{ "op": 180, "params": [10] }, { "op": 172 }] }
              ],
              "maps": [
                { "id": 1, "file": "TitleMap.mps", "name": "Title", "width": 5, "height": 5, "events": [
                  { "id": 2, "page": 0, "trigger": 1, "stats": { "real_instructions": 1 }, "slots": [{ "op": 180, "params": [10] }] }
                ]},
                { "id": 3, "file": "Map003.mps", "name": "NewGame", "width": 5, "height": 5, "events": [
                  { "id": 1, "page": 0, "trigger": 1, "stats": { "real_instructions": 1 }, "slots": [{ "op": 180, "params": [10] }] }
                ]},
                { "id": 10, "file": "SampleMapA.mps", "name": "Room", "width": 5, "height": 5, "events": [
                  { "id": 4, "page": 0, "trigger": 2, "stats": { "real_instructions": 1 }, "slots": [{ "op": 180, "params": [10] }] }
                ]}
              ]
            }
            """;
            string progPath = System.IO.Path.Combine(tmpDir, "programs.json");
            System.IO.File.WriteAllText(progPath, progJson);
            string savePath = System.IO.Path.Combine(tmpDir, "save_01.json");

            var stubView = new StubSessionView();
            var stubVm = new StubVmSession();
            foreach (var p in ProgramLoader.LoadCommonEvents(progPath).Values) stubVm.Vm.RegisterProgram(p);
            var session = new GameSession(stubVm, stubView, progPath, savePath);

            stubVm.Vm.Vars.Set(12, 99);
            if (!session.StartTitle())
                throw new Exception("StartTitle failed on synthetic programs.json");
            if (stubVm.ResetSessionCalls != 1 || stubVm.Vm.Vars.Get(12) != 0)
                throw new Exception($"StartTitle must call ResetSessionState and clear vars (calls={stubVm.ResetSessionCalls}, v12={stubVm.Vm.Vars.Get(12)})");

            // StartNewGameNow (teleport to map 3) must clear op250 DB overlays while preserving TitleMap vars.
            stubVm.Vm.Vars.SetSys(100, 60);
            stubVm.ChoiceAllowsCancel = false;
            session.ShowChoices(new[] { "A", "B" });
            stubVm.ChoiceAllowsCancel = true;
            session.QueueTeleport(-1, 3, 60, 34, 1);
            int hideBefore = stubView.HideChoicesCalls;
            session.AdvanceFrame();
            if (stubVm.ResetSessionCalls != 2 || stubVm.Vm.Vars.GetSys(100) != 60
                || !session.ChoiceAllowsCancel || stubView.HideChoicesCalls <= hideBefore)
                throw new Exception($"StartNewGameNow must reset session overlays/choices while keeping TitleMap vars (calls={stubVm.ResetSessionCalls}, sys100={stubVm.Vm.Vars.GetSys(100)})");

            // Load a slot containing:
            // - map instance (kind="map", page=0) -> RetainAfterFinish must be true
            // - parallel CE with explicit is_parallel=true
            // - legacy CE child (is_parallel=null) whose parent is a parallel map event -> inferred IsParallel=true
            var state = new SaveState
            {
                MapId = 10,
                MapFile = "SampleMapA.mps",
                LeaderX = 1,
                LeaderY = 1,
                MapEvents =
                {
                    new SavedMapEvent { Id = 4, Active = 0, InstanceId = 10, Runs = true, Starts = 1, StartsOnPage = 1 }
                },
                Instances =
                {
                    new SavedInstance { Id = 10, EventId = 4, Kind = "map", Page = 0, Pc = 0, IsParallel = true },
                    new SavedInstance { Id = 11, ParentId = 10, EventId = 51, Kind = "ce", Pc = 0, IsParallel = null },
                    new SavedInstance { Id = 12, EventId = 50, Kind = "ce", Pc = 0, IsParallel = true },
                    new SavedInstance { Id = 13, EventId = 50, Kind = "ce", Pc = 0, IsParallel = false },
                    new SavedInstance { Id = 14, EventId = 242, Kind = "ce", Pc = 0, IsParallel = null },
                    new SavedInstance { Id = 15, ParentId = 14, EventId = 51, Kind = "ce", Pc = 0, IsParallel = null }
                }
            };
            SaveCodec.Write(savePath, state);
            if (!session.LoadGame())
                throw new Exception("GameSession.LoadGame failed on synthetic save");

            var mapInst = stubVm.Vm.FindInstance(10);
            var legacyChild = stubVm.Vm.FindInstance(11);
            var explicitParallel = stubVm.Vm.FindInstance(12);
            var explicitFg = stubVm.Vm.FindInstance(13);
            var gatedParallel = stubVm.Vm.FindInstance(14);
            var gatedChild = stubVm.Vm.FindInstance(15);
            if (mapInst == null || !mapInst.RetainAfterFinish || !mapInst.IsParallel)
                throw new Exception($"restored map instance must have RetainAfterFinish=true and IsParallel=true");
            if (legacyChild == null || !legacyChild.IsParallel)
                throw new Exception("legacy saved CE child under parallel parent must infer IsParallel=true");
            if (explicitParallel == null || !explicitParallel.IsParallel)
                throw new Exception("saved IsParallel=true must round-trip on CE");
            if (explicitFg == null || explicitFg.IsParallel)
                throw new Exception("saved IsParallel=false must stay false on CE");
            if (gatedParallel == null || !gatedParallel.IsParallel || gatedChild == null || !gatedChild.IsParallel)
                throw new Exception("InferLegacyParallel must treat GatedCallOnly (CE242) and its children as parallel");

            SaveState? rebuilt = session.BuildSaveState();
            var rebuilt12 = rebuilt?.Instances.Find(i => i.Id == 12);
            if (rebuilt12?.IsParallel != true)
                throw new Exception("BuildSaveState must persist IsParallel");
        }
        finally
        {
            ProgramLoader.ClearCache();
            try { System.IO.Directory.Delete(tmpDir, true); } catch (System.IO.IOException) { }
        }
    }

    private sealed class DelegateDbHost(Func<DbOp, DbValue?> handler) : NullHost
    {
        public override DbValue? Database(DbOp op) => handler(op);
    }
}
