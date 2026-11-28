using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>
/// Direct checks for the opcode 250 store: exported rows, .project label drift,
/// sentinel rows/columns, overlays and clear semantics.
/// </summary>
public static class DatabaseTest
{
    public static void Run()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string dataDir = Path.GetFullPath(Path.Combine(rootDir, "..", "game_data"));
        string basicDir = Path.GetFullPath(Path.Combine(rootDir, "..", "work", "all", "BasicData"));
        var db = GameDatabase.Load(Path.Combine(dataDir, "database.json"), basicDir);
        Check(db != null, "database.json did not load");
        var store = db!;

        var name = store.Read(ReadOp(DbContainer.CData, 0, 0, 0, Magic.StrBase));
        Check(name is { IsString: true } && name.Str == "勇者先生",
            $"CData(0,0,0) name expected 勇者先生, got {Describe(name)}");

        var hp = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(hp is { IsString: false } && hp.Num == 1,
            $"CData(0,0,5) expected 1, got {Describe(hp)}");

        var resolved = store.ResolveLabels(DbContainer.Data, 18, 22, "システム設定", "装備可能欄数");
        Check(resolved == (17, 22), $"label drift expected (17,22), got {resolved}");
        var slots = store.Read(ReadOp(DbContainer.Data, 18, 0, 22, Magic.VarBase,
            "システム設定", "装備可能欄数"));
        Check(slots is { IsString: false } && slots.Num == 5,
            $"resolved equipment slots expected 5, got {Describe(slots)}");

        var party = store.ResolveLabels(DbContainer.CData, 3, 1, "パーティー情報", "メンバー1");
        Check(party == (6, 1), $"party label expected (6,1), got {party}");
        var leader = store.Read(ReadOp(DbContainer.CData, 3, 0, 1, Magic.VarBase,
            "パーティー情報", "メンバー1"));
        Check(leader is { IsString: false } && leader.Num == 12,
            $"party leader row expected 12, got {Describe(leader)}");

        RunVmDatabase(store);

        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 1, 4));
        var added = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(added is { IsString: false } && added.Num == 5,
            $"numeric add expected 5, got {Describe(added)}");

        store.Execute(WriteOp(DbContainer.CData, 0, 0, 0, 0, 99));
        var guarded = store.Read(ReadOp(DbContainer.CData, 0, 0, 0, Magic.StrBase));
        Check(guarded is { IsString: true } && guarded.Str == "勇者先生",
            $"numeric write must not overwrite text, got {Describe(guarded)}");

        store.Execute(WriteOp(DbContainer.CData, 0, 0, 0, 0, "テスト"));
        var written = store.Read(ReadOp(DbContainer.CData, 0, 0, 0, Magic.StrBase));
        Check(written is { IsString: true } && written.Str == "テスト",
            $"text write expected テスト, got {Describe(written)}");

        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 0, "8"));
        var parsed = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(parsed is { IsString: false } && parsed.Num == 8,
            $"text-to-number write expected 8, got {Describe(parsed)}");

        Check(store.RowCount(DbContainer.CData, 0) == 17,
            $"CData type 0 expected 17 rows, got {store.RowCount(DbContainer.CData, 0)}");
        store.Execute(WriteOp(DbContainer.CData, 0, -2, 0, 0, 2));
        Check(store.RowCount(DbContainer.CData, 0) == 2,
            $"row-count write expected 2, got {store.RowCount(DbContainer.CData, 0)}");
        var beyond = store.Read(ReadOp(DbContainer.CData, 0, 2, 5, Magic.VarBase));
        Check(beyond is { IsString: false } && beyond.Num == 0,
            $"row 2 past the runtime count expected 0, got {Describe(beyond)}");
        var count = store.Read(ReadOp(DbContainer.CData, 0, -1, 0, Magic.VarBase));
        Check(count is { IsString: false } && count.Num == 2,
            $"row-count query expected 2, got {Describe(count)}");

        store.Execute(WriteOp(DbContainer.CData, 0, 0, -2, 0, 0));
        var clearedText = store.Read(ReadOp(DbContainer.CData, 0, 0, 0, Magic.StrBase));
        var clearedNum = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(clearedText is { IsString: true } && clearedText.Str.Length == 0,
            $"cleared row text expected empty, got {Describe(clearedText)}");
        Check(clearedNum is { IsString: false } && clearedNum.Num == 0,
            $"cleared row number expected 0, got {Describe(clearedNum)}");
        var first = store.Read(ReadOp(DbContainer.CData, 0, 0, -1, Magic.VarBase));
        Check(first is { IsString: false } && first.Num == 0,
            $"first empty row expected 0, got {Describe(first)}");

        var extension = store.Read(ReadOp(DbContainer.CData, 0, 100, 0, Magic.StrBase));
        Check(extension is { IsString: true } && extension.Str.Length == 0,
            $"extension row text expected empty, got {Describe(extension)}");

        // Division/modulo edge cases: /0 and %0 preserve current; int.MinValue / -1 and % -1 do not throw.
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 0, 42));
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 4, 0));
        var divZero = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(divZero is { IsString: false } && divZero.Num == 42,
            $"DB division by zero must preserve current 42, got {Describe(divZero)}");
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 5, 0));
        var modZero = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(modZero is { IsString: false } && modZero.Num == 42,
            $"DB modulo by zero must preserve current 42, got {Describe(modZero)}");
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 0, int.MinValue));
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 4, -1));
        var divMin = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(divMin is { IsString: false } && divMin.Num == int.MinValue,
            $"DB int.MinValue / -1 must wrap cleanly, got {Describe(divMin)}");
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 0, int.MinValue));
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 5, -1));
        var modMin = store.Read(ReadOp(DbContainer.CData, 0, 0, 5, Magic.VarBase));
        Check(modMin is { IsString: false } && modMin.Num == 0,
            $"DB int.MinValue % -1 must return 0, got {Describe(modMin)}");

        // ClearRow with negative row must be rejected without writing negative-row keys.
        store.Execute(WriteOp(DbContainer.CData, 0, -3, -2, 0, 0));
        var negCleared = store.Read(ReadOp(DbContainer.CData, 0, -3, 0, Magic.VarBase));
        Check(negCleared == null, $"negative row ClearRow must not write overlay, got {Describe(negCleared)}");

        // TryReadInt fast path must match Read() on numeric, sentinel, and string cells.
        Check(store.TryReadInt(DbContainer.CData, 0, 0, 5, out int fastVal) && fastVal == 0,
            $"TryReadInt numeric cell expected 0, got {fastVal}");
        store.Execute(WriteOp(DbContainer.CData, 0, 0, 5, 0, 42));
        Check(store.TryReadInt(DbContainer.CData, 0, 0, 5, out int fastValUpdated) && fastValUpdated == 42,
            $"TryReadInt updated numeric cell expected 42, got {fastValUpdated}");
        Check(store.TryReadInt(DbContainer.CData, 0, -1, 0, out int fastCount) && fastCount == 2,
            $"TryReadInt RowCountQuery expected 2, got {fastCount}");
        Check(store.TryReadInt(DbContainer.CData, 0, 100, 0, out int fastExt) && fastExt == 0,
            $"TryReadInt extension cell expected 0, got {fastExt}");
        Check(!store.TryReadInt(DbContainer.CData, 0, 0, 0, out _),
            "TryReadInt must return false for string cell");

        // ReadWithOverlay must match ReadWithOverlayIndex wantsText preference.
        var dualOverlays = new SavedDbOverlays
        {
            Nums = [new SavedDbNumCell { Container = (int)DbContainer.CData, Type = 0, Row = 1, Column = 5, Value = 123 }],
            Texts = [new SavedDbTextCell { Container = (int)DbContainer.CData, Type = 0, Row = 1, Column = 5, Value = "txt" }]
        };
        var ovNum = store.ReadWithOverlay(ReadOp(DbContainer.CData, 0, 1, 5, Magic.VarBase), dualOverlays);
        var ovText = store.ReadWithOverlay(ReadOp(DbContainer.CData, 0, 1, 5, Magic.StrBase), dualOverlays);
        Check(ovNum is { IsString: false } && ovNum.Num == 123,
            $"ReadWithOverlay(!wantsText) expected 123, got {Describe(ovNum)}");
        Check(ovText is { IsString: true } && ovText.Str == "txt",
            $"ReadWithOverlay(wantsText) expected txt, got {Describe(ovText)}");

        // WolfProjectParser must reject truncated/crafted counts without OOM or long loops.
        byte[] craftedProj = new byte[32];
        System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(craftedProj.AsSpan(0, 4), 1u); // typeCount = 1
        System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(craftedProj.AsSpan(4, 4), 2u); // name len 2
        craftedProj[8] = (byte)'A'; craftedProj[9] = 0;
        System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(craftedProj.AsSpan(10, 4), 0x7FFF_FFFFu); // fieldCount
        var parsedProj = WolfProjectParser.Parse(craftedProj, 1);
        Check(parsedProj != null && parsedProj.Length == 1 && parsedProj[0] == null,
            "WolfProjectParser must reject oversized fieldCount safely");

        // Case-insensitive .project filename fallback in GameDatabase.Load.
        string ciBasicDir = Path.Combine(Path.GetTempPath(), "lwsmf_ci_proj_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(ciBasicDir);
        try
        {
            string srcProj = Path.Combine(basicDir, "DataBase.project");
            if (File.Exists(srcProj))
            {
                File.Copy(srcProj, Path.Combine(ciBasicDir, "database.project"));
                var ciDb = GameDatabase.Load(Path.Combine(dataDir, "database.json"), ciBasicDir);
                Check(ciDb != null && ciDb.ResolveLabels(DbContainer.Data, 18, 22, "システム設定", "装備可能欄数") == (17, 22),
                    "GameDatabase.Load must resolve lowercase database.project fallback");
            }
        }
        finally
        {
            try { Directory.Delete(ciBasicDir, true); } catch (IOException) { }
        }

        // TextExpander ReadOnlySpan<char> path checks (direct coords, nested coords, untouched colour escapes).
        var expVm = new EventVm();
        var expProg = Program.FromSlots(999, "ExpTest", new List<Slot>());
        expVm.RegisterProgram(expProg);
        var expInst = expVm.CreateInstance(999)!;
        expInst.Self.Set(expProg.SelfBase + 0, 2);
        expVm.Vars.Set(1, 3);
        expVm.Vars.Set(5, 77);
        expVm.Vars.SetStr(2, "name");
        string expanded = TextExpander.Expand(
            "\\E\\f[20]\\v[5]-\\s[2]-\\udb[\\cself[0]:\\v[1]:0]",
            expInst,
            expVm.Vars,
            (kind, t, r, c) => $"{kind}:{t}:{r}:{c}");
        Check(expanded == "\\E\\f[20]77-name-2:2:3:0",
            $"TextExpander span path mismatch: '{expanded}'");

        var vm = new Lwsmf.Presentation.VmController();
        try
        {
            Check(vm.LoadPrograms(Path.Combine(dataDir, "programs.json")),
                "VmController.LoadPrograms failed for the database test");
            var wired = vm.Database(ReadOp(DbContainer.CData, 0, 0, 0, Magic.StrBase));
            Check(wired is { IsString: true } && wired.Str.Length > 0,
                $"VmController.Database did not reach the store, got {Describe(wired)}");
        }
        finally
        {
            vm.Free();
        }

        GD.Print("DATABASE_TEST_OK name/labels/sentinels/overlay/vm-250");
    }

    private static void RunVmDatabase(GameDatabase store)
    {
        var slots = new List<Slot>
        {
            Slot.Command(250, [0, 0, 5, 0x1000, Magic.VarBase]),
            Slot.Command(250, [0, 0, 0, 0x1000, Magic.StrBase]),
            Slot.Command(250, [18, 0, 22, 0x1200, 2_000_001], ["", "システム設定", "", "装備可能欄数"]),
            Slot.Command(250, [0, 5, 0, 0], ["テスト"]),
            Slot.Command(250, [0, 5, 0, 0x1000, Magic.StrBase + 1]),
            Slot.Command(121, [2_000_002, 7, 0, 0]),
            Slot.Command(250, [0, 5, 5, 0, 2_000_002]),
            Slot.Command(250, [0, 5, 5, 0x1000, 2_000_003]),
            Slot.Command(499)
        };
        var host = new StoreHost { Store = store };
        var vm = new EventVm { Host = host };
        vm.RegisterProgram(Program.FromSlots(300, "DbVmTest", slots));
        var inst = vm.CreateInstance(300);
        Check(inst != null, "opcode 250 test instance was not created");
        int budget = 100;
        var signal = vm.StepInstance(inst!, ref budget);
        Check(signal == Lwsmf.Core.Signal.Finished, $"opcode 250 test expected Finished, got {signal}");
        Check(vm.Vars.Get(0) == 1, $"opcode 250 numeric read expected 1, got {vm.Vars.Get(0)}");
        Check(vm.Vars.GetStr(0) == "勇者先生",
            $"opcode 250 string read expected 勇者先生, got {vm.Vars.GetStr(0)}");
        Check(vm.Vars.Get(1) == 5, $"opcode 250 label read expected 5, got {vm.Vars.Get(1)}");
        Check(vm.Vars.GetStr(1) == "テスト",
            $"opcode 250 string write expected テスト, got {vm.Vars.GetStr(1)}");
        Check(vm.Vars.Get(3) == 7, $"opcode 250 numeric write expected 7, got {vm.Vars.Get(3)}");
    }

    private sealed class StoreHost : RecordingHost
    {
        public GameDatabase? Store;
        public override DbValue? Database(DbOp op) => Store?.Execute(op);
    }

    private static DbOp ReadOp(DbContainer container, int type, int row, int column, int destination,
        string? typeLabel = null, string? fieldLabel = null) => new()
    {
        Container = container,
        Direction = DbDirection.Read,
        Op = 0,
        DbType = type,
        DataId = row,
        ItemId = column,
        Destination = destination,
        TypeLabel = typeLabel,
        FieldLabel = fieldLabel
    };

    private static DbOp WriteOp(DbContainer container, int type, int row, int column, byte op, int value) => new()
    {
        Container = container,
        Direction = DbDirection.Write,
        Op = op,
        DbType = type,
        DataId = row,
        ItemId = column,
        Source = new DbValue(value)
    };

    private static DbOp WriteOp(DbContainer container, int type, int row, int column, byte op, string value) => new()
    {
        Container = container,
        Direction = DbDirection.Write,
        Op = op,
        DbType = type,
        DataId = row,
        ItemId = column,
        Source = new DbValue(value)
    };

    private static string Describe(DbValue? value)
        => value == null ? "null" : value.IsString ? $"\"{value.Str}\"" : value.Num.ToString();

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception($"DATABASE_TEST_FAILED: {message}");
    }
}
