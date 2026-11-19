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
        TestOpcode1000ProFeatureWait();
        TestMaxCallDepth();
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
        Console.WriteLine("  Testing int.MinValue / -1 safety...");
        int binary = EventVm.BinaryOp(0x3, int.MinValue, -1);
        if (binary != int.MinValue)
            throw new Exception($"BinaryOp min/-1 should wrap, got {binary}");
        int compound = EventVm.Compound(0x400, int.MinValue, -1);
        if (compound != int.MinValue)
            throw new Exception($"Compound min/-1 should wrap, got {compound}");
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

        // No 0x04 literal flag: the operand is still evaluated as Var 0 (9).
        vm.Vars.Set(0, 9);
        vm.ExecuteSetVariable([2_000_002, 2_000_000, 0, 0x0004], inst);
        if (vm.Vars.Get(2) != 9)
            throw new Exception($"TC-121 literal flag must be ignored, got {vm.Vars.Get(2)}");

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
            throw new Exception("600094 from caller 270 must resolve to 264");
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
    }

    private static void TestOpcode1000ProFeatureWait()
    {
        Console.WriteLine("  Testing Opcode 1000 (ProFeature wait)...");
        var slots = new List<Slot>
        {
            Slot.Command(1000, [5 << 16, 32]),
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
        vm.StepFrame();
        if (vm.Vars.Get(0) != 0)
            throw new Exception("1000 resumed before its wait elapsed");
        vm.StepFrame();
        if (vm.Vars.Get(0) != 7)
            throw new Exception($"1000 did not resume, Var 0={vm.Vars.Get(0)}");
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
}