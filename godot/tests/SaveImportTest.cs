using System;
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

        GD.Print($"SAVE_IMPORT_OK 8/8 retail saves verified and exported to {reportPath}");
        GetTree().Quit(0);
    }
}
