using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

public partial class SaveImportTest : Node
{
    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string saveDir = Path.GetFullPath(Path.Combine(rootDir, "..", "work", "retail_backup", "Save"));
        string reportPath = Path.GetFullPath(Path.Combine(rootDir, "..", "reports", "save_import.json"));

        GD.Print($"Testing LegacySaveImporter on: {saveDir}");
        if (!Directory.Exists(saveDir))
        {
            GD.PrintErr($"Save directory not found: {saveDir}");
            GetTree().Quit(1);
            return;
        }

        var report = LegacySaveImporter.ImportAll(saveDir, reportPath);
        if (report.Slots.Count != 8)
        {
            GD.PrintErr($"Expected 8 slots, got {report.Slots.Count}");
            GetTree().Quit(2);
            return;
        }

        foreach (var slot in report.Slots)
        {
            if (!slot.ChecksumOk)
            {
                GD.PrintErr($"Slot {slot.Index} ({slot.File}) checksum failed!");
                GetTree().Quit(3);
                return;
            }
            if (slot.Recovered.Variables.Count != 800)
            {
                GD.PrintErr($"Slot {slot.Index} expected 800 variables, got {slot.Recovered.Variables.Count}");
                GetTree().Quit(4);
                return;
            }
            GD.Print($"Slot {slot.Index:D2} ({slot.File}): playtime={slot.Recovered.PlayTime.Formatted} pictures={slot.Recovered.Pictures.Count} vars={slot.Recovered.Variables.Count} ok=True");
        }

        if (!TryValidation(saveDir))
        {
            GetTree().Quit(5);
            return;
        }

        if (!TrySaveTransferApis(rootDir, saveDir))
        {
            GetTree().Quit(6);
            return;
        }

        if (!TryRetailExportRoundtrip(rootDir, saveDir))
        {
            GetTree().Quit(7);
            return;
        }

        GD.Print($"SAVE_IMPORT_OK 8/8 retail saves verified and exported to {reportPath}");
        GetTree().Quit(0);
    }

    private static bool TryValidation(string saveDir)
    {
        string sample = Path.Combine(saveDir, "SaveData01.sav");
        byte[] original = File.ReadAllBytes(sample);
        string temp = Path.Combine(Path.GetTempPath(), $"lwsmf_save_validation_{System.Environment.ProcessId}.tmp");

        byte[] badChecksum = (byte[])original.Clone();
        badChecksum[2] ^= 0xFF;
        File.WriteAllBytes(temp, badChecksum);
        try
        {
            var relaxed = LegacySaveImporter.ImportFile(temp, 1);
            if (relaxed.ChecksumOk)
            {
                GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: tampered checksum reported ok");
                return false;
            }

            bool strictThrew = false;
            try { _ = LegacySaveImporter.ImportFile(temp, 1, true); }
            catch (InvalidDataException) { strictThrew = true; }
            if (!strictThrew)
            {
                GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: strict import accepted bad checksum");
                return false;
            }
        }
        finally
        {
            File.Delete(temp);
        }

        byte[] badStart = (byte[])original.Clone();
        badStart[0x14] ^= 1;
        File.WriteAllBytes(temp, badStart);
        try
        {
            bool startThrew = false;
            try { _ = LegacySaveImporter.ImportFile(temp, 1); }
            catch (InvalidDataException) { startThrew = true; }
            if (!startThrew)
            {
                GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: tampered start byte accepted");
                return false;
            }
        }
        finally
        {
            File.Delete(temp);
        }

        var warnings = new List<string>();
        var valid = new LegacySaveImporter.SaveReader(new byte[] { 4, 0, 0xE3, 0x81, 0x82, 0 }, 0, warnings);
        if (valid.ReadMemDataString(2, "name") != "あ" || warnings.Count != 0)
        {
            GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: valid UTF-8 mis-decoded");
            return false;
        }

        var invalid = new LegacySaveImporter.SaveReader(new byte[] { 2, 0, 0xFF, 0xFE }, 0, warnings);
        string replaced = invalid.ReadMemDataString(2, "name");
        if (warnings.Count != 1 || replaced.Length == 0)
        {
            GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: invalid UTF-8 warnings={warnings.Count} text={replaced.Length}");
            return false;
        }

        var pictureWarnings = new List<string>();
        string pictureText = LegacySaveImporter.DecodePictureText(
            new byte[] { 0xFF, 0xFE, 0 }, pictureWarnings, "picture_test");
        if (!pictureText.Contains('\uFFFD') || pictureWarnings.Count != 1
            || !pictureWarnings[0].Contains("invalid UTF-8"))
        {
            GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: picture UTF-8 warnings={pictureWarnings.Count} text={pictureText.Length}");
            return false;
        }

        // Invalid UTF-8 lead byte must advance by 1 so following ASCII/control bytes survive.
        string preservedAscii = LegacySaveImporter.DecodePictureText(
            new byte[] { 0xFF, (byte)'A', 0 }, new List<string>(), "picture_step");
        if (preservedAscii != "\uFFFDA")
        {
            GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: invalid UTF-8 lead byte swallowed trailing char: '{preservedAscii}'");
            return false;
        }

        // A truncated multibyte lead must not swallow the next valid character.
        string truncatedLead = LegacySaveImporter.DecodePictureText(
            new byte[] { 0xE3, 0xE3, 0x81, 0x82, 0 }, new List<string>(), "picture_lead");
        if (truncatedLead != "\uFFFD\u3042")
        {
            GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: truncated multibyte lead swallowed next character: '{truncatedLead}'");
            return false;
        }

        // ReadBytes integer-overflow guard: Position + count overflow must throw InvalidDataException.
        var overflowReader = new LegacySaveImporter.SaveReader(new byte[16], 8);
        bool readBytesThrew = false;
        try { overflowReader.ReadBytes(int.MaxValue); }
        catch (InvalidDataException) { readBytesThrew = true; }
        if (!readBytesThrew)
        {
            GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: ReadBytes accepted overflowing count");
            return false;
        }

        // Crafted .sav guards: map cell overflow, VariableDatabase fc/zero-field loop, unsafe picName, oversized file.
        byte[] BuildEncryptedSave(byte[] plainPayload)
        {
            byte[] buf = new byte[0x15 + plainPayload.Length];
            buf[0] = 7; buf[3] = 11; buf[9] = 13;
            buf[0x14] = 0x19;
            Array.Copy(plainPayload, 0, buf, 0x15, plainPayload.Length);
            byte sum = 0;
            for (int i = 0x14; i < buf.Length; i++) sum = unchecked((byte)(sum + buf[i]));
            buf[2] = sum;
            byte[] seeds = [buf[0], buf[3], buf[9]];
            int[] steps = [1, 2, 5];
            for (int pass = 0; pass < 3; pass++)
            {
                uint st = seeds[pass];
                for (int at = 0x14; at < buf.Length; at += steps[pass])
                {
                    st = unchecked(st * 214013u + 2531011u);
                    buf[at] ^= (byte)(((st >> 16) & 0x7FFFu) >> 12);
                }
            }
            return buf;
        }

        bool ExpectInvalidSav(byte[] plainPayload, string label)
        {
            string craftPath = Path.Combine(Path.GetTempPath(), $"lwsmf_craft_{Guid.NewGuid():N}.sav");
            try
            {
                File.WriteAllBytes(craftPath, BuildEncryptedSave(plainPayload));
                try
                {
                    _ = LegacySaveImporter.ImportFile(craftPath, 1);
                    GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: {label} was not rejected");
                    return false;
                }
                catch (InvalidDataException)
                {
                    return true;
                }
            }
            finally
            {
                if (File.Exists(craftPath)) File.Delete(craftPath);
            }
        }

        // Map dimension multiplication overflow: width=0xFFFFFFFF, height=0xFFFFFFFF, sentinel=0
        using (var ms = new MemoryStream())
        using (var bw = new BinaryWriter(ms))
        {
            bw.Write((ushort)0); // save_name len 0
            bw.Write((ushort)1); // version
            bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(uint.MaxValue); // mapWidth
            bw.Write(uint.MaxValue); // mapHeight
            bw.Write(0u); // sentinel != MaxValue
            if (!ExpectInvalidSav(ms.ToArray(), "map dimension overflow")) return false;
        }

        // Oversized file guard (> SaveCodec.MaxSaveBytes)
        string bigPath = Path.Combine(Path.GetTempPath(), $"lwsmf_big_{Guid.NewGuid():N}.sav");
        try
        {
            using (var fs = new FileStream(bigPath, FileMode.Create, System.IO.FileAccess.Write))
            {
                fs.SetLength(SaveCodec.MaxSaveBytes + 1);
            }
            bool bigThrew = false;
            try { _ = LegacySaveImporter.ImportFile(bigPath, 1); }
            catch (InvalidDataException) { bigThrew = true; }
            if (!bigThrew)
            {
                GD.PrintErr("SAVE_IMPORT_VALIDATION_FAILED: oversized save file was accepted");
                return false;
            }
        }
        finally
        {
            if (File.Exists(bigPath)) File.Delete(bigPath);
        }

        // VariableDatabase fc >= 0x80000000 and cfg.Length==0 with typeRowCount=0xFFFFFFFF:
        // build a minimal valid prefix up to VariableDatabase by splicing SavePart7 of a decrypted retail save?
        // Instead craft a minimal save stream with 0-count sections up to VariableDatabase.
        byte[] BuildSaveUpToVarDb(int unk, uint? fc, uint typeRowCount)
        {
            using var ms = new MemoryStream();
            using var bw = new BinaryWriter(ms);
            bw.Write((ushort)0); bw.Write((ushort)1); // save_name + version
            bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write(0u); bw.Write(uint.MaxValue); // map sentinel
            bw.Write(0u); // chip_image
            for (int i = 0; i < 31; i++) bw.Write(0u);
            bw.Write(0u); // vars2Count
            bw.Write(0u); // objectCount
            for (int i = 0; i < 11; i++) bw.Write(0u);
            bw.Write(0u); // vars3Count
            // SavePart2
            bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 19; i++) bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            for (int i = 0; i < 9; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write((ushort)0);
            bw.Write(0u); bw.Write(0u);
            for (int i = 0; i < 4; i++) bw.Write((ushort)0);
            for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            // block 100
            bw.Write(0u); bw.Write(0u); bw.Write((byte)0);
            bw.Write(0u); bw.Write(0u);
            for (int i = 0; i < 11; i++) bw.Write(0u);
            bw.Write((ushort)0);
            bw.Write((ushort)0); bw.Write((ushort)0);
            bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); bw.Write((ushort)0); bw.Write((byte)0);
            bw.Write((byte)0);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            for (int i = 0; i < 9; i++) bw.Write(0u);
            for (int i = 0; i < 4; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 32; i++) bw.Write(0u);
            bw.Write(0u); // readmemdata(4)
            bw.Write(0u); // mds2Count
            // SavePart3
            bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            // SavePart4 (SkipObject)
            bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write((byte)0); bw.Write(0u);
            bw.Write((ushort)0);
            for (int i = 0; i < 4; i++) bw.Write((ushort)0);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int s = 0; s < 2; s++) { for (int i = 0; i < 6; i++) bw.Write((byte)0); bw.Write(0u); }
            for (int i = 0; i < 6; i++) bw.Write((ushort)0);
            for (int i = 0; i < 3; i++) bw.Write((byte)0);
            bw.Write(0u);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 8; i++) bw.Write(0u);
            bw.Write((ushort)0);
            bw.Write(0u); bw.Write((byte)0); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); // p4Count
            bw.Write((byte)0); bw.Write((byte)0);
            bw.Write(0u); bw.Write(0u);
            // SavePart5
            bw.Write((ushort)0);
            // VariableDatabase
            bw.Write((byte)0);
            bw.Write(1u); // typeCount=1
            bw.Write(unk);
            if (fc.HasValue) bw.Write(fc.Value);
            bw.Write(typeRowCount);
            return ms.ToArray();
        }

        if (!ExpectInvalidSav(BuildSaveUpToVarDb(-1, 0x8000_0001u, 1u), "VariableDatabase fc > int.MaxValue")) return false;
        if (!ExpectInvalidSav(BuildSaveUpToVarDb(-1, 10_000u, 1u), "VariableDatabase fc > remaining")) return false;
        if (!ExpectInvalidSav(BuildSaveUpToVarDb(0, null, uint.MaxValue), "VariableDatabase zero-field infinite row loop")) return false;

        // Unsafe picName path traversal in SavePart5 (isText=0, picName="../evil.png")
        byte[] BuildSaveWithUnsafePicName(string picName)
        {
            using var ms = new MemoryStream();
            using var bw = new BinaryWriter(ms);
            bw.Write((ushort)0); bw.Write((ushort)1);
            bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write(0u); bw.Write(uint.MaxValue);
            bw.Write(0u);
            for (int i = 0; i < 31; i++) bw.Write(0u);
            bw.Write(0u); bw.Write(0u);
            for (int i = 0; i < 11; i++) bw.Write(0u);
            bw.Write(0u);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 19; i++) bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            for (int i = 0; i < 9; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write((ushort)0);
            bw.Write(0u); bw.Write(0u);
            for (int i = 0; i < 4; i++) bw.Write((ushort)0);
            for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write(0u); bw.Write((byte)0);
            bw.Write(0u); bw.Write(0u);
            for (int i = 0; i < 11; i++) bw.Write(0u);
            bw.Write((ushort)0); bw.Write((ushort)0); bw.Write((ushort)0);
            bw.Write(0u);
            for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); for (int i = 0; i < 3; i++) bw.Write(0u);
            bw.Write((ushort)0); bw.Write((ushort)0); bw.Write((byte)0);
            bw.Write((byte)0);
            for (int i = 0; i < 3; i++) bw.Write((ushort)0);
            for (int i = 0; i < 9; i++) bw.Write(0u);
            for (int i = 0; i < 4; i++) bw.Write(0u);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 32; i++) bw.Write(0u);
            bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write((byte)0); bw.Write(0u);
            bw.Write((ushort)0);
            for (int i = 0; i < 4; i++) bw.Write((ushort)0);
            bw.Write((byte)0); bw.Write((byte)0);
            for (int s = 0; s < 2; s++) { for (int i = 0; i < 6; i++) bw.Write((byte)0); bw.Write(0u); }
            for (int i = 0; i < 6; i++) bw.Write((ushort)0);
            for (int i = 0; i < 3; i++) bw.Write((byte)0);
            bw.Write(0u); bw.Write((byte)0); bw.Write((byte)0);
            for (int i = 0; i < 8; i++) bw.Write(0u);
            bw.Write((ushort)0);
            bw.Write(0u); bw.Write((byte)0); bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u);
            bw.Write(0u); bw.Write((byte)0); bw.Write((byte)0); bw.Write(0u); bw.Write(0u);
            // SavePart5: 1 picture entry
            bw.Write((ushort)1);
            bw.Write(1u); // picId
            bw.Write((byte)0); bw.Write((byte)0);
            bw.Write((ushort)0); // anchor
            bw.Write((byte)0); // isText = 0
            bw.Write((byte)0); // isWin = 0
            byte[] nameBytes = System.Text.Encoding.UTF8.GetBytes(picName);
            bw.Write((ushort)nameBytes.Length);
            bw.Write(nameBytes);
            return ms.ToArray();
        }
        if (!ExpectInvalidSav(BuildSaveWithUnsafePicName("../evil.png"), "unsafe picName ..")) return false;
        if (!ExpectInvalidSav(BuildSaveWithUnsafePicName("/etc/passwd"), "unsafe rooted picName")) return false;

        // ImportAll per-slot resilience & atomic report write
        string batchDir = Path.Combine(Path.GetTempPath(), $"lwsmf_import_batch_{Guid.NewGuid():N}");
        Directory.CreateDirectory(batchDir);
        try
        {
            File.Copy(sample, Path.Combine(batchDir, "SaveData01.sav"));
            File.WriteAllBytes(Path.Combine(batchDir, "SaveData02.sav"), new byte[] { 1, 2, 3 });
            string batchReport = Path.Combine(batchDir, "report.json");
            var batch = LegacySaveImporter.ImportAll(batchDir, batchReport);
            if (batch.Slots.Count != 1 || batch.Errors.Count != 1 || !File.Exists(batchReport) || File.Exists(batchReport + ".tmp"))
            {
                GD.PrintErr($"SAVE_IMPORT_VALIDATION_FAILED: batch isolation slots={batch.Slots.Count} errors={batch.Errors.Count}");
                return false;
            }
        }
        finally
        {
            try { Directory.Delete(batchDir, true); } catch (IOException) { }
        }

        GD.Print("SAVE_IMPORT_VALIDATION_OK checksum=strict start=strict utf8=warn-once picture-utf8=warn-once");
        return true;
    }

    private bool TrySaveTransferApis(string rootDir, string saveDir)
    {
        var validState = new SaveState { MapId = 1005, MapFile = "SampleMapA.mps" };
        string validJson = SaveCodec.ToJson(validState);
        if (!GameSession.ValidateSaveJson(validJson, out string validErr) || validErr.Length != 0)
        {
            GD.PrintErr($"SAVE_TRANSFER_FAILED: valid JSON rejected: {validErr}");
            return false;
        }

        foreach (string bad in new[] { "", "{bad", "{}", "{\"schema\":\"lwsmf.other\",\"schema_version\":1}" })
        {
            if (GameSession.ValidateSaveJson(bad, out string badErr) || string.IsNullOrEmpty(badErr))
            {
                GD.PrintErr($"SAVE_TRANSFER_FAILED: malformed JSON accepted: {bad}");
                return false;
            }
        }

        string sampleSav = Path.Combine(saveDir, "SaveData01.sav");
        string dataPath = Path.GetFullPath(Path.Combine(rootDir, "..", "game_data", "programs.json"));
        string transferDir = Path.Combine(Path.GetTempPath(), $"lwsmf_transfer_{Guid.NewGuid():N}");
        Directory.CreateDirectory(transferDir);

        var controllerUi = new Godot.Control();
        AddChild(controllerUi);
        var controllerVm = new VmController { Name = "TransferCtrlVm" };
        AddChild(controllerVm);
        controllerVm.LoadPrograms(dataPath);
        var controller = new GameSessionController();
        AddChild(controller);

        try
        {
            string slot1Path = Path.Combine(transferDir, "save_01.json");
            var harness = new SessionHarness(this, dataPath, slot1Path);
            if (!harness.RunToBedroom(4000))
            {
                GD.PrintErr($"SAVE_TRANSFER_FAILED: could not reach bedroom ({harness.Session.Phase})");
                return false;
            }

            harness.Vm.Vm.Vars.Set(500, 12345);
            int beforeFrames = harness.Session.Frames;
            string beforePhase = harness.Session.Phase;
            int beforeMapId = harness.Session.MapData?.Id ?? -1;

            if (!harness.Session.ImportRetailSaveToSlot(sampleSav, 2, out string importErr) || importErr.Length != 0)
            {
                GD.PrintErr($"SAVE_TRANSFER_FAILED: ImportRetailSaveToSlot failed: {importErr}");
                return false;
            }

            if (harness.Session.Phase != beforePhase
                || harness.Session.Frames != beforeFrames
                || (harness.Session.MapData?.Id ?? -1) != beforeMapId
                || harness.Vm.Vm.Vars.Get(500) != 12345)
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: ImportRetailSaveToSlot mutated active live session");
                return false;
            }

            string slot2Path = Path.Combine(transferDir, "save_02.json");
            if (!File.Exists(slot2Path))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: save_02.json was not written");
                return false;
            }

            SaveState? imported = SaveCodec.Read(slot2Path);
            if (imported == null || imported.Vars.Count != WolfIds.VarPollMax || imported.PlayFrames <= 0
                || imported.MapId != 1005 || imported.CommonSelf.Count == 0
                || !imported.MapEvents.Exists(e => e.Id == 0 && !e.Runs)
                || !imported.Pictures.Exists(p => p.Number < 0))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: imported save_02.json missing expected state");
                return false;
            }
            var rawSlot = LegacySaveImporter.ImportFile(sampleSav, 1);
            if (rawSlot.Recovered.DatabaseCells.Count > 0)
            {
                if (imported.DbOverlays == null || imported.DbOverlays.Nums.Count == 0
                    || imported.DbOverlays.Nums.Count >= rawSlot.Recovered.DatabaseCells.Count)
                {
                    GD.PrintErr($"SAVE_TRANSFER_FAILED: expected sparse diffed DbOverlays (0 < n < {rawSlot.Recovered.DatabaseCells.Count}), got {imported.DbOverlays?.Nums.Count ?? 0}");
                    return false;
                }
                // Spot-check that sparse DbOverlays still reproduces every cell value via ReadWithOverlay.
                if (harness.Vm.DatabaseStore != null)
                {
                    var idx = GameDatabase.OverlayIndex.Build(imported.DbOverlays);
                    foreach (var cell in rawSlot.Recovered.DatabaseCells)
                    {
                        var readVal = harness.Vm.DatabaseStore.ReadWithOverlayIndex(new DbOp
                        {
                            Container = DbContainer.CData,
                            Direction = DbDirection.Read,
                            DbType = cell.Type,
                            DataId = cell.Row,
                            ItemId = cell.Column
                        }, idx);
                        if (readVal == null || readVal.IsString || readVal.Num != cell.Value)
                        {
                            GD.PrintErr($"SAVE_TRANSFER_FAILED: sparse overlay mismatch at [{cell.Type}:{cell.Row}:{cell.Column}] expected {cell.Value}, got {readVal?.Num}");
                            return false;
                        }
                    }
                }
                if (!string.IsNullOrEmpty(harness.Session.LastImportNotice))
                {
                    GD.PrintErr($"SAVE_TRANSFER_FAILED: unexpected LastImportNotice when DbOverlays persisted: {harness.Session.LastImportNotice}");
                    return false;
                }
            }

            if (harness.Session.ImportRetailSaveToSlot(sampleSav, 0, out string badSlotErr) || string.IsNullOrEmpty(badSlotErr))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: slot 0 should be rejected");
                return false;
            }
            if (harness.Session.ImportRetailSaveToSlot(sampleSav, 100, out string highSlotErr) || string.IsNullOrEmpty(highSlotErr))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: slot 100 should be rejected");
                return false;
            }

            controller.Setup(controllerVm, controllerUi, slot1Path);
            if (controller.ValidateSaveJson(validJson) != "")
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: controller.ValidateSaveJson rejected valid JSON");
                return false;
            }
            if (string.IsNullOrEmpty(controller.ValidateSaveJson("{bad")))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: controller.ValidateSaveJson accepted malformed JSON");
                return false;
            }
            if (!controller.ImportRetailSaveToSlot(sampleSav, 3) || controller.LastError.Length != 0)
            {
                GD.PrintErr($"SAVE_TRANSFER_FAILED: controller.ImportRetailSaveToSlot failed: {controller.LastError}");
                return false;
            }
            if (!File.Exists(Path.Combine(transferDir, "save_03.json")))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: save_03.json was not written by controller");
                return false;
            }
            if (controller.ImportRetailSaveToSlot(Path.Combine(transferDir, "missing.sav"), 4) || string.IsNullOrEmpty(controller.LastError))
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: missing .sav should fail and populate LastError");
                return false;
            }

            if (!harness.Session.LoadGameFrom(slot2Path) || harness.Session.Phase != "bedroom")
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: imported save_02.json failed to load into live session");
                return false;
            }

            harness.Session.ShowMessage("stale", 0);
            harness.Session.ShowChoices(new[] { "stale" });
            if (!harness.Session.LoadRetailSave(sampleSav, 0)
                || !harness.Session.ChoiceAllowsCancel
                || harness.Session.MessageOpen || harness.Session.HasChoices
                || harness.View.HasMessage || harness.View.HasChoices)
            {
                GD.PrintErr("SAVE_TRANSFER_FAILED: LoadRetailSave did not clear stale choice/popup state");
                return false;
            }
        }
        finally
        {
            controller.QueueFree();
            controllerVm.QueueFree();
            controllerUi.QueueFree();
            try { Directory.Delete(transferDir, true); } catch (IOException) { }
        }

        GD.Print("SAVE_TRANSFER_API_OK");
        return true;
    }

    private bool TryRetailExportRoundtrip(string rootDir, string saveDir)
    {
        // 1. Shift-JIS (raw[6] != 0x55) decoding verification in SaveReader & DecodePictureText.
        System.Text.Encoding.RegisterProvider(System.Text.CodePagesEncodingProvider.Instance);
        var sjis = System.Text.Encoding.GetEncoding(932);
        byte[] sjisBytes = sjis.GetBytes("妹相随");
        byte[] memBuf = new byte[2 + sjisBytes.Length + 1];
        memBuf[0] = (byte)(sjisBytes.Length + 1);
        memBuf[1] = 0;
        Array.Copy(sjisBytes, 0, memBuf, 2, sjisBytes.Length);
        var sjisWarnings = new List<string>();
        var sjisReader = new LegacySaveImporter.SaveReader(memBuf, 0, sjisWarnings, isUtf8: false);
        string decodedSjis = sjisReader.ReadMemDataString(2, "sjis_test");
        if (decodedSjis != "妹相随" || sjisWarnings.Count != 0)
        {
            GD.PrintErr($"RETAIL_EXPORT_FAILED: Shift-JIS decode mismatch got '{decodedSjis}' warnings={sjisWarnings.Count}");
            return false;
        }

        // 2. EncodePictureText <-> DecodePictureText roundtrip for \E, \f[n], \c[n].
        string styled = "\\E\\c[1]周六\\c[0] \\f[24]大字";
        byte[] encodedText = LegacySaveExporter.EncodePictureText(styled);
        string decodedText = LegacySaveImporter.DecodePictureText(encodedText);
        if (decodedText != styled)
        {
            GD.PrintErr($"RETAIL_EXPORT_FAILED: picture text roundtrip mismatch want='{styled}' got='{decodedText}'");
            return false;
        }

        // 3. Full SaveState -> LegacySaveExporter -> LegacySaveImporter roundtrip (Vars, CommonSelf, Pictures, DbOverlays, Audio, PlayFrames, ChoiceAnchor).
        string tempDir = Path.Combine(Path.GetTempPath(), $"lwsmf_retail_rt_{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);
        string dataPath = Path.GetFullPath(Path.Combine(rootDir, "..", "game_data", "programs.json"));
        try
        {
            var state = new SaveState
            {
                PlayFrames = 123456,
                MapId = 1005,
                MapFile = "SampleMapA.mps",
                Vars = new List<int>(new int[800]),
                StrVars = ["周六", "", "save/system.sav"],
                SysVars = [0, 0, 0, 480, 260, 0, 0, 0, 20],
                CommonSelf =
                [
                    new EventSelfVars
                    {
                        Id = 12,
                        NumMap = new Dictionary<int, int> { [600] = 777, [604] = -42 },
                        StrMap = new Dictionary<int, string> { [605] = "你好世界", [609] = "tail" }
                    }
                ],
                Pictures =
                [
                    new SavedPicture
                    {
                        Number = 50,
                        Kind = 0,
                        File = "cg/bg_01.png",
                        Anchor = 1,
                        Args = [0, 2, 2, 3, 200, 100, 200, 100, 0, 0, 0, 0, 0, 0, 0, 0, 75],
                        Flags = new Dictionary<string, int> { ["blend"] = 1, ["scale_mode"] = 3 }
                    },
                    new SavedPicture { Number = 51, Kind = 2, Text = styled, Anchor = 0, Args = [0, 320, 64, 1, 255, 20, 30, 100, 0] },
                    new SavedPicture { Number = 12050, Kind = 0, File = "SystemFile/WindowBase.png", Anchor = 0, Args = [0, 1, 1, 1, 255, 0, 0, 100, 0] }
                ],
                Audio =
                [
                    new SavedAudio { Channel = 0, File = "BGM/test.ogg", Volume = 80, Loop = true, Tempo = 105 },
                    new SavedAudio { Channel = 1, File = "BGM/rain.ogg", Volume = 40, Loop = true, Tempo = 95 }
                ],
                DbOverlays = new SavedDbOverlays
                {
                    Nums = [new SavedDbNumCell { Container = (int)DbContainer.CData, Type = 2, Row = 1, Column = 0, Value = 9876 }],
                    Texts = [new SavedDbTextCell { Container = (int)DbContainer.CData, Type = 2, Row = 1, Column = 1, Value = "自定义名称" }]
                }
            };
            state.Vars[0] = 3;
            state.Vars[160] = 25;
            state.Vars[531] = 4;
            state.Vars[550] = 22;
            state.Vars[799] = -99;

            string savPath = Path.Combine(tempDir, "SaveData01.sav");
            byte[] encoded = LegacySaveExporter.ExportSaveState(state, null, "Roundtrip Test");
            LegacySaveExporter.WriteAtomic(savPath, encoded);

            if (!GameSession.ValidateRetailSave(savPath, out string valErr))
            {
                GD.PrintErr($"RETAIL_EXPORT_FAILED: exported .sav failed ValidateRetailSave: {valErr}");
                return false;
            }
            var parsed = LegacySaveImporter.ImportFile(savPath, 1, rejectBadChecksum: true);
            var rec = parsed.Recovered;
            if (!parsed.ChecksumOk
                || rec.PlayTime.Frames != 123456
                || rec.ChoiceAnchor[0] != 480 || rec.ChoiceAnchor[1] != 260
                || rec.SysVars.Length < 9 || rec.SysVars[8] != 20
                || rec.StringVars.Count != 3 || rec.StringVars[0] != "周六" || rec.StringVars[2] != "save/system.sav"
                || rec.Variables["0"] != 3 || rec.Variables["160"] != 25 || rec.Variables["550"] != 22 || rec.Variables["799"] != -99
                || rec.CommonSelfNumbers[12][0] != 777 || rec.CommonSelfNumbers[12][4] != -42
                || rec.CommonSelfStrings[12][0] != "你好世界" || rec.CommonSelfStrings[12][4] != "tail"
                || rec.PictureStates.Count != 2
                || rec.PictureStates[0].File != "cg/bg_01.png"
                || rec.PictureStates[0].Args.Count < 17 || rec.PictureStates[0].Args[3] != 3 || rec.PictureStates[0].Args[16] != 75
                || !(rec.PictureStates[0].Flags is { } p0Flags
                    && p0Flags.TryGetValue("blend", out int p0Blend) && p0Blend == 1
                    && p0Flags.TryGetValue("scale_mode", out int p0ScaleMode) && p0ScaleMode == 3)
                || rec.PictureStates[1].Text != styled || rec.PictureStates[1].Args.Count != 9
                || rec.BgmFile != "BGM/test.ogg" || rec.BgmVolume != 80 || rec.BgmTempo != 105
                || rec.BgsFile != "BGM/rain.ogg" || rec.BgsVolume != 40 || rec.BgsTempo != 95
                || !rec.DatabaseCells.Exists(c => c.Type == 2 && c.Row == 1 && c.Column == 0 && c.Value == 9876)
                || !rec.DatabaseTextCells.Exists(c => c.Type == 2 && c.Row == 1 && c.Column == 1 && c.Value == "自定义名称"))
            {
                GD.PrintErr("RETAIL_EXPORT_FAILED: SaveState -> .sav -> ImportFile roundtrip data mismatch");
                return false;
            }

            // 4. Session ImportRetailSystemSave + ImportRetailSaveToSlot System.json sync + ExportAllToRetailSavDir.
            string slot1Path = Path.Combine(tempDir, "saves", "save_01.json");
            Directory.CreateDirectory(Path.GetDirectoryName(slot1Path)!);
            var harness = new SessionHarness(this, dataPath, slot1Path);

            string retailSysSav = Path.Combine(saveDir, "System.sav");
            if (File.Exists(retailSysSav))
            {
                if (!harness.Session.ImportRetailSystemSave(retailSysSav, out string sysErr))
                {
                    GD.PrintErr($"RETAIL_EXPORT_FAILED: ImportRetailSystemSave failed on retail System.sav: {sysErr}");
                    return false;
                }
                if (!harness.Session.ImportRetailSystemSaveTo(retailSysSav, "system2.sav", out string sys2Err)
                    || !File.Exists(Path.Combine(Path.GetDirectoryName(slot1Path)!, "system2.json")))
                {
                    GD.PrintErr($"RETAIL_EXPORT_FAILED: ImportRetailSystemSaveTo(system2.sav) failed: {sys2Err}");
                    return false;
                }
            }

            if (!harness.Session.ImportRetailSaveToSlot(savPath, 4, out string slotErr))
            {
                GD.PrintErr($"RETAIL_EXPORT_FAILED: ImportRetailSaveToSlot failed on exported .sav: {slotErr}");
                return false;
            }
            string sysJsonPath = Path.Combine(Path.GetDirectoryName(slot1Path)!, "System.json");
            SaveState? sysAfter = SaveCodec.Read(sysJsonPath);
            if (sysAfter == null || sysAfter.Vars.Count <= 626
                || sysAfter.Vars[0] != 4 || sysAfter.Vars[546] != 4 || sysAfter.Vars[626] != 4
                || sysAfter.Vars[550] < 22)
            {
                GD.PrintErr("RETAIL_EXPORT_FAILED: ImportRetailSaveToSlot did not sync System.json slot markers & NG+ var550");
                return false;
            }

            string outDir = Path.Combine(tempDir, "exported");
            if (!harness.Session.ExportAllToRetailSavDir(outDir, true, out int expCount, out string expErr)
                || expCount < 2
                || !File.Exists(Path.Combine(outDir, "SaveData04.sav"))
                || !File.Exists(Path.Combine(outDir, "System.sav"))
                || (File.Exists(Path.Combine(Path.GetDirectoryName(slot1Path)!, "system2.json"))
                    && !File.Exists(Path.Combine(outDir, "system2.sav"))))
            {
                GD.PrintErr($"RETAIL_EXPORT_FAILED: ExportAllToRetailSavDir failed count={expCount} err={expErr}");
                return false;
            }
            bool okSav = GameSession.ValidateRetailSave(Path.Combine(outDir, "SaveData04.sav"), out string e1);
            bool okSys = GameSession.ValidateRetailSave(Path.Combine(outDir, "System.sav"), out string e2);
            if (!okSav || !okSys)
            {
                GD.PrintErr($"RETAIL_EXPORT_FAILED: batch exported .sav failed validation: {e1} / {e2}");
                return false;
            }

            // Optional external probe output for Rust wolf-save cross-check when LWsmf_EXPORT_PROBE is set.
            string? probeOut = System.Environment.GetEnvironmentVariable("LWsmf_EXPORT_PROBE");
            if (!string.IsNullOrWhiteSpace(probeOut))
            {
                LegacySaveExporter.WriteAtomic(probeOut, encoded);
            }
        }
        finally
        {
            try { Directory.Delete(tempDir, true); } catch (IOException) { }
        }

        GD.Print("RETAIL_EXPORT_ROUNDTRIP_OK");
        return true;
    }
}
