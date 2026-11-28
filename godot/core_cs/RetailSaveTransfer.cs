using System;
using System.Collections.Generic;
using System.IO;

namespace Lwsmf.Core;

/// <summary>
/// Bidirectional conversion between native JSON <see cref="SaveState"/> slots and retail
/// WOLF RPG <c>.sav</c> / <c>System.sav</c> binaries without mutating the live VM session.
/// </summary>
internal sealed class RetailSaveTransfer
{
    private readonly IVmSession _vm;
    private readonly string _dataPath;
    private readonly SaveSlotStore _saveStore;

    public RetailSaveTransfer(IVmSession vm, string dataPath, SaveSlotStore saveStore)
    {
        _vm = vm;
        _dataPath = dataPath;
        _saveStore = saveStore;
    }

    public string LastImportNotice { get; private set; } = "";

    public bool ImportRetailSaveToSlot(string savPath, int slot, out SaveAuditReport? auditResult, out string error)
    {
        LastImportNotice = "";
        auditResult = null;
        if (slot < 1 || slot > 99)
        {
            error = $"save slot {slot} out of range (must be 1..99)";
            return false;
        }
        if (string.IsNullOrWhiteSpace(savPath) || !File.Exists(savPath))
        {
            error = $"retail save file not found: {savPath}";
            return false;
        }

        LegacySaveImporter.SlotResult imported;
        try
        {
            imported = LegacySaveImporter.ImportFile(savPath, Math.Max(0, slot - 1), rejectBadChecksum: true);
        }
        catch (Exception ex)
        {
            error = "retail import failed: " + ex.Message;
            return false;
        }

        if (!imported.ChecksumOk)
        {
            error = "retail import failed: checksum mismatch";
            return false;
        }

        LegacySaveImporter.RecoveredData data = imported.Recovered;
        List<int> numbers = BuildRetailVarsList(data);
        List<int> sysVars = BuildRetailSysVarsList(data);

        string mapPath = _vm.MapPath(1);
        MapDefinition? map1 = mapPath.Length > 0
            ? ProgramLoader.LoadMap(_dataPath, mapPath, null)
            : ProgramLoader.LoadMap(_dataPath, "SampleMapA.mps", 1005);

        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = data.PlayTime.Frames,
            MapId = map1?.Id ?? 1005,
            MapFile = map1?.File ?? "SampleMapA.mps",
            LeaderX = 0,
            LeaderY = 0,
            Facing = 2,
            Vars = numbers,
            StrVars = new List<string>(data.StringVars),
            SysVars = sysVars,
        };

        foreach (KeyValuePair<int, SelfVars> bank in BuildCommonSelfFromRetail(data))
        {
            state.CommonSelf.Add(SaveSlotStore.SavedSelf(bank.Key, bank.Value));
        }

        // Preserve retail map-1 event self-vars (including Ev3 one-time migration flags)
        // while keeping new-game setup event 0 disabled so load never resets the day.
        ApplyRetailMapEventSelfVars(state, data);

        var audit = new SaveAuditReport { SourceKind = SaveAuditSource.RetailImport };
        audit.EncodingWarnings.AddRange(imported.EncodingWarnings);

        List<SavedPicture> sanitizedPics = SaveMigrator.SanitizeRestoredPictures(data.PictureStates, audit);
        var picByNum = new Dictionary<int, SavedPicture>(sanitizedPics.Count);
        foreach (SavedPicture pic in sanitizedPics)
        {
            picByNum[pic.Number] = pic;
        }
        foreach (SavedPicture pic in picByNum.Values)
        {
            if (state.Pictures.Count >= SaveCodec.MaxPictures) break;
            state.Pictures.Add(pic);
        }

        state.DbOverlays = BuildRetailDbOverlays(data);
        AddRetailAudio(state.Audio, data);

        SaveMigrator.Normalize(state, audit);
        SaveMigrator.ApplyRetailPostLoadTriggerIfNeeded(state, audit);
        SaveMigrator.InspectSuspiciousLocks(state.Vars, audit);

        if (!SaveSlotStore.ValidateSaveJson(SaveCodec.ToJson(state), out string validationError))
        {
            error = "converted save failed validation: " + validationError;
            return false;
        }

        string dir = Path.GetDirectoryName(_saveStore.SavePath) ?? ".";
        int normalizedSlot = slot <= 1 ? 1 : slot;
        string targetPath = Path.Combine(dir, $"save_{normalizedSlot:00}.json");
        try
        {
            SaveCodec.Write(targetPath, state);
            _saveStore.InvalidateSaveCache(targetPath);
            // Sync System.json slot markers & monotonic progression (NG+ var550 / clear flags)
            // without downgrading existing System.sav data.
            string systemPath = _saveStore.ResolveSaveTarget(new SaveTarget("Save/System.sav"));
            SaveState systemState = SaveCodec.Read(systemPath) ?? new SaveState();
            SaveSlotStore.WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 0, Value = normalizedSlot });
            SaveSlotStore.WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 546, Value = normalizedSlot });
            SaveSlotStore.MergeMaxSaveVar(systemState, 626, SaveSlotStore.GetVarOrDefault(state.Vars, 531));
            SaveSlotStore.MergeMaxSaveVar(systemState, 550, SaveSlotStore.GetVarOrDefault(state.Vars, 550));
            SaveSlotStore.MergeMaxSaveVar(systemState, 503, SaveSlotStore.GetVarOrDefault(state.Vars, 503));
            SaveSlotStore.MergeMaxSaveVar(systemState, 300, SaveSlotStore.GetVarOrDefault(state.Vars, 300));
            for (int v = 80; v <= 87; v++) SaveSlotStore.MergeMaxSaveVar(systemState, v, SaveSlotStore.GetVarOrDefault(state.Vars, v));
            for (int v = 324; v <= 328; v++) SaveSlotStore.MergeMaxSaveVar(systemState, v, SaveSlotStore.GetVarOrDefault(state.Vars, v));
            SaveCodec.Write(systemPath, systemState);
            _saveStore.InvalidateSaveCache(systemPath);
        }
        catch (Exception ex)
        {
            error = "save write failed: " + ex.Message;
            return false;
        }

        auditResult = audit;
        error = "";
        return true;
    }

    public static bool ValidateRetailSave(string savPath, out string error) =>
        LegacySaveImporter.ValidateFile(savPath, out error);

    public bool ImportRetailSystemSaveTo(string savPath, string targetFileName, out string error)
    {
        if (string.IsNullOrWhiteSpace(savPath) || !File.Exists(savPath))
        {
            error = "retail system save file not found";
            return false;
        }
        string cleanLeaf = Path.GetFileName((targetFileName ?? "").Replace('\\', '/'));
        string saveTarget = cleanLeaf.Equals("system2.sav", StringComparison.OrdinalIgnoreCase)
            || cleanLeaf.Equals("system2.json", StringComparison.OrdinalIgnoreCase)
            ? "save/system2.sav"
            : "Save/System.sav";

        LegacySaveImporter.SlotResult parsed;
        try
        {
            parsed = LegacySaveImporter.ImportFile(savPath, 0, rejectBadChecksum: true);
        }
        catch (Exception ex) when (ex is InvalidDataException or EndOfStreamException or IOException or UnauthorizedAccessException)
        {
            error = "retail system save import failed: " + ex.Message;
            return false;
        }

        var data = parsed.Recovered;
        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = data.PlayTime.Frames,
            Vars = BuildRetailVarsList(data),
            StrVars = new List<string>(data.StringVars),
            SysVars = BuildRetailSysVarsList(data),
            DbOverlays = BuildRetailDbOverlays(data),
        };
        foreach (KeyValuePair<int, SelfVars> bank in BuildCommonSelfFromRetail(data))
        {
            state.CommonSelf.Add(SaveSlotStore.SavedSelf(bank.Key, bank.Value));
        }

        SaveMigrator.Normalize(state);
        if (!SaveSlotStore.ValidateSaveJson(SaveCodec.ToJson(state), out string validationError))
        {
            error = "converted system save failed validation: " + validationError;
            return false;
        }

        string systemPath = _saveStore.ResolveSaveTarget(new SaveTarget(saveTarget));
        try
        {
            SaveCodec.Write(systemPath, state);
            _saveStore.InvalidateSaveCache(systemPath);
        }
        catch (Exception ex)
        {
            error = "system save write failed: " + ex.Message;
            return false;
        }

        error = "";
        return true;
    }

    public bool ExportSlotToRetailSav(int slot, string destPath, out string error)
    {
        if (slot < 1 || slot > 99)
        {
            error = $"slot {slot} out of range (1..99)";
            return false;
        }
        if (string.IsNullOrWhiteSpace(destPath))
        {
            error = "destination path is empty";
            return false;
        }
        string srcPath = _saveStore.SlotSavePath(slot);
        SaveState? state = SaveCodec.Read(srcPath);
        if (state == null)
        {
            error = $"save slot {slot:00} not found or unreadable";
            return false;
        }
        try
        {
            byte[] encoded = LegacySaveExporter.ExportSaveState(state, _vm.DatabaseStore, $"LWSMF Save {slot:00}");
            LegacySaveExporter.WriteAtomic(destPath, encoded);
            error = "";
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException)
        {
            error = "retail export failed: " + ex.Message;
            return false;
        }
    }

    public bool ExportSystemTargetToRetailSav(string saveTarget, string destPath, string label, out string error)
    {
        if (string.IsNullOrWhiteSpace(destPath))
        {
            error = "destination path is empty";
            return false;
        }
        string systemPath = _saveStore.ResolveSaveTarget(new SaveTarget(saveTarget));
        SaveState? state = SaveCodec.Read(systemPath);
        if (state == null)
        {
            error = $"{Path.GetFileName(systemPath)} not found or unreadable";
            return false;
        }
        try
        {
            byte[] encoded = LegacySaveExporter.ExportSystemState(state, label);
            LegacySaveExporter.WriteAtomic(destPath, encoded);
            error = "";
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException)
        {
            error = "retail system export failed: " + ex.Message;
            return false;
        }
    }

    public bool ExportAllToRetailSavDir(string destDir, bool includeThumbnails, out int exportedCount, out string error)
    {
        exportedCount = 0;
        if (string.IsNullOrWhiteSpace(destDir))
        {
            error = "destination directory is empty";
            return false;
        }
        string saveDir = Path.GetDirectoryName(_saveStore.SavePath) ?? ".";
        if (!Directory.Exists(saveDir))
        {
            error = "save directory does not exist";
            return false;
        }

        try
        {
            Directory.CreateDirectory(destDir);
            for (int slot = 1; slot <= 99; slot++)
            {
                string srcPath = _saveStore.SlotSavePath(slot);
                if (!SaveCodec.Exists(srcPath)) continue;
                string outSav = Path.Combine(destDir, $"SaveData{slot:00}.sav");
                if (!ExportSlotToRetailSav(slot, outSav, out string slotErr))
                {
                    error = slotErr;
                    return false;
                }
                exportedCount++;
                if (includeThumbnails)
                {
                    string thumbName = $"save{slot}.jpg";
                    string? thumbSrc = ResolveExistingFileCaseInsensitive(saveDir, thumbName);
                    if (thumbSrc != null)
                    {
                        File.Copy(thumbSrc, Path.Combine(destDir, thumbName), overwrite: true);
                        exportedCount++;
                    }
                }
            }

            foreach (var (sysTarget, outLeaf, label) in new[]
            {
                ("Save/System.sav", "System.sav", "LWSMF System"),
                ("save/system2.sav", "system2.sav", "LWSMF System2")
            })
            {
                string systemPath = _saveStore.ResolveSaveTarget(new SaveTarget(sysTarget));
                if (SaveCodec.Exists(systemPath)
                    && ExportSystemTargetToRetailSav(sysTarget, Path.Combine(destDir, outLeaf), label, out _))
                {
                    exportedCount++;
                }
            }

            if (exportedCount == 0)
            {
                error = "no save slots found to export";
                return false;
            }
            error = "";
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            error = "batch retail export failed: " + ex.Message;
            return false;
        }
    }

    private static string? ResolveExistingFileCaseInsensitive(string dir, string fileName)
    {
        string exact = Path.Combine(dir, fileName);
        if (File.Exists(exact)) return exact;
        try
        {
            foreach (string existing in Directory.EnumerateFiles(dir))
            {
                if (string.Equals(Path.GetFileName(existing), fileName, StringComparison.OrdinalIgnoreCase))
                    return existing;
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
        {
        }
        return null;
    }

    public static Dictionary<int, SelfVars> BuildCommonSelfFromRetail(LegacySaveImporter.RecoveredData data)
    {
        var result = new Dictionary<int, SelfVars>();
        foreach (KeyValuePair<int, int[]> pair in data.CommonSelfNumbers)
        {
            var bank = new SelfVars();
            bool any = false;
            for (int i = 0; i < pair.Value.Length; i++)
            {
                if (pair.Value[i] == 0) continue;
                bank.Set(600 + i, pair.Value[i]);
                any = true;
            }
            if (any) result[pair.Key] = bank;
        }
        foreach (KeyValuePair<int, string[]> pair in data.CommonSelfStrings)
        {
            SelfVars? bank = result.GetValueOrDefault(pair.Key);
            for (int i = 0; i < pair.Value.Length; i++)
            {
                if (string.IsNullOrEmpty(pair.Value[i])) continue;
                if (bank == null)
                {
                    bank = new SelfVars();
                    result[pair.Key] = bank;
                }
                bank.SetStr(605 + i, pair.Value[i]);
            }
        }
        return result;
    }

    public static List<int> BuildRetailVarsList(LegacySaveImporter.RecoveredData data)
    {
        int count = WolfIds.VarPollMax;
        foreach (string key in data.Variables.Keys)
        {
            if (int.TryParse(key, out int idx) && idx >= count && idx < Variables.MaxSlots)
                count = idx + 1;
        }
        var numbers = new List<int>(count);
        for (int i = 0; i < count; i++) numbers.Add(0);
        foreach (KeyValuePair<string, int> pair in data.Variables)
        {
            if (int.TryParse(pair.Key, out int index) && index >= 0 && index < numbers.Count)
            {
                numbers[index] = pair.Value;
            }
        }
        return numbers;
    }

    public static List<int> BuildRetailSysVarsList(LegacySaveImporter.RecoveredData data)
    {
        if (data.SysVars is { Length: > 0 })
        {
            return new List<int>(data.SysVars);
        }
        var sysVars = new List<int>();
        if (data.ChoiceAnchor is { Length: >= 2 })
        {
            int needed = Math.Max(WolfIds.SysChoiceAnchorX, WolfIds.SysChoiceAnchorY) + 1;
            while (sysVars.Count < needed) sysVars.Add(0);
            sysVars[WolfIds.SysChoiceAnchorX] = data.ChoiceAnchor[0];
            sysVars[WolfIds.SysChoiceAnchorY] = data.ChoiceAnchor[1];
        }
        return sysVars;
    }

    public static Dictionary<int, SelfVars>? BuildRetailMapSelfVars(LegacySaveImporter.RecoveredData data)
    {
        if (!data.MapEventSelfVars.TryGetValue(1, out Dictionary<int, int[]>? map1Events) || map1Events.Count == 0)
            return null;
        var result = new Dictionary<int, SelfVars>(map1Events.Count);
        foreach (var pair in map1Events)
        {
            var sv = new SelfVars();
            for (int k = 0; k < pair.Value.Length; k++)
            {
                if (pair.Value[k] != 0) sv.Set(100 + k, pair.Value[k]);
            }
            result[pair.Key] = sv;
        }
        return result;
    }

    private static void ApplyRetailMapEventSelfVars(SaveState state, LegacySaveImporter.RecoveredData data)
    {
        Dictionary<int, SelfVars>? mapSelf = BuildRetailMapSelfVars(data);
        if (mapSelf != null)
        {
            foreach (var pair in mapSelf)
            {
                state.SelfVars.Add(SaveSlotStore.SavedSelf(pair.Key, pair.Value));
            }
        }
        EventSelfVars ev0Self = mapSelf != null && mapSelf.TryGetValue(0, out SelfVars? sv0)
            ? SaveSlotStore.SavedSelf(0, sv0)
            : new EventSelfVars { Id = 0 };
        state.MapEvents.Add(new SavedMapEvent
        {
            Id = 0,
            Active = null,
            InstanceId = null,
            Runs = false,
            Starts = 1,
            StartsOnPage = 1,
            Self = ev0Self,
        });
    }

    private SavedDbOverlays? BuildRetailDbOverlays(LegacySaveImporter.RecoveredData data)
    {
        if (data.DatabaseRowCounts.Count == 0 && data.DatabaseCells.Count == 0 && data.DatabaseTextCells.Count == 0)
            return null;
        if (_vm.DatabaseStore != null)
        {
            return _vm.DatabaseStore.BuildSparseOverlays(data.DatabaseRowCounts, data.DatabaseCells, data.DatabaseTextCells);
        }
        var overlays = new SavedDbOverlays();
        foreach (var rc in data.DatabaseRowCounts)
        {
            if (rc.Key < 0 || rc.Value < 0) continue;
            overlays.RowCounts.Add(new SavedDbRowCount { Container = (int)DbContainer.CData, Type = rc.Key, Count = rc.Value });
        }
        foreach (var cell in data.DatabaseCells)
        {
            if (cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            overlays.Nums.Add(new SavedDbNumCell
            {
                Container = (int)DbContainer.CData,
                Type = cell.Type,
                Row = cell.Row,
                Column = cell.Column,
                Value = cell.Value
            });
        }
        foreach (var cell in data.DatabaseTextCells)
        {
            if (cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            overlays.Texts.Add(new SavedDbTextCell
            {
                Container = (int)DbContainer.CData,
                Type = cell.Type,
                Row = cell.Row,
                Column = cell.Column,
                Value = cell.Value
            });
        }
        return overlays;
    }

    public static void AddRetailAudio(List<SavedAudio> target, LegacySaveImporter.RecoveredData data)
    {
        if (!string.IsNullOrEmpty(data.BgmFile))
        {
            target.Add(new SavedAudio
            {
                Channel = 0,
                File = data.BgmFile,
                Volume = data.BgmVolume,
                Loop = true,
                LoopPoint = data.BgmLoopPoint,
                Tempo = data.BgmTempo
            });
        }
        if (!string.IsNullOrEmpty(data.BgsFile))
        {
            target.Add(new SavedAudio
            {
                Channel = 1,
                File = data.BgsFile,
                Volume = data.BgsVolume,
                Loop = true,
                LoopPoint = data.BgsLoopPoint,
                Tempo = data.BgsTempo
            });
        }
    }
}
