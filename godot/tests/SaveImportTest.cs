using System;
using System.Collections.Generic;
using System.IO;
using Godot;
using Lwsmf.Core;

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

        GD.Print("SAVE_IMPORT_VALIDATION_OK checksum=strict start=strict utf8=warn-once picture-utf8=warn-once");
        return true;
    }
}
