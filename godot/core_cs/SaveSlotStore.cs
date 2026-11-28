using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace Lwsmf.Core;

/// <summary>
/// Manages native JSON save slots, .bak fallback, in-memory op-221 snapshot cache,
/// HasSave presence cache, and opcode 220/221/222 address routing.
/// </summary>
internal sealed class SaveSlotStore
{
    private const int SaveReadCacheLimit = 32;

    internal sealed class CachedSaveEntry
    {
        public bool Exists;
        public long MainLength;
        public long MainWriteTicks;
        public long BakLength;
        public long BakWriteTicks;
        public SaveState? State;
        public GameDatabase.OverlayIndex? OverlayIndex;
    }

    private static readonly StringComparer PathComparer = OperatingSystem.IsWindows()
        ? StringComparer.OrdinalIgnoreCase
        : StringComparer.Ordinal;

    private static readonly StringComparison PathComparison = OperatingSystem.IsWindows()
        ? StringComparison.OrdinalIgnoreCase
        : StringComparison.Ordinal;

    private readonly IVmSession _vm;
    private readonly IGameSessionView _view;
    private readonly string _savePath;
    private readonly Dictionary<string, CachedSaveEntry> _saveReadCache = new(PathComparer);
    private bool? _hasSaveCache;

    public SaveSlotStore(IVmSession vm, IGameSessionView view, string savePath)
    {
        _vm = vm;
        _view = view;
        _savePath = savePath;
    }

    public string SavePath => _savePath;

    public bool HasSave
    {
        get
        {
            _hasSaveCache ??= _savePath.Length > 0 && SaveCodec.Exists(_savePath);
            return _hasSaveCache.Value;
        }
    }

    public void InvalidateSaveCache(string? path = null)
    {
        if (string.IsNullOrEmpty(path))
        {
            _saveReadCache.Clear();
            _hasSaveCache = null;
            return;
        }
        _saveReadCache.Remove(path);
        if (string.Equals(path, _savePath, PathComparison)
            || string.Equals(path, _savePath + ".bak", PathComparison))
        {
            _hasSaveCache = null;
        }
    }

    public CachedSaveEntry ReadCachedSave(string path)
    {
        TryReadFileStamp(path, out bool mainExists, out long mainLen, out long mainTicks);
        TryReadFileStamp(path + ".bak", out bool bakExists, out long bakLen, out long bakTicks);

        if (_saveReadCache.TryGetValue(path, out CachedSaveEntry? cached)
            && cached.MainLength == mainLen
            && cached.MainWriteTicks == mainTicks
            && cached.BakLength == bakLen
            && cached.BakWriteTicks == bakTicks)
        {
            return cached;
        }

        if (_saveReadCache.Count >= SaveReadCacheLimit)
        {
            _saveReadCache.Clear();
        }

        SaveState? state = null;
        if (mainExists || bakExists)
        {
            try
            {
                state = SaveCodec.Read(path);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                state = null;
            }
        }

        var entry = new CachedSaveEntry
        {
            Exists = state != null,
            MainLength = mainLen,
            MainWriteTicks = mainTicks,
            BakLength = bakLen,
            BakWriteTicks = bakTicks,
            State = state,
            OverlayIndex = state != null ? GameDatabase.OverlayIndex.Build(state.DbOverlays) : null,
        };
        _saveReadCache[path] = entry;
        return entry;
    }

    private static void TryReadFileStamp(string filePath, out bool exists, out long length, out long writeTicks)
    {
        try
        {
            var info = new FileInfo(filePath);
            if (info.Exists)
            {
                exists = true;
                length = info.Length;
                writeTicks = info.LastWriteTimeUtc.Ticks;
                return;
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
        exists = false;
        length = -1;
        writeTicks = 0;
    }

    public string SlotSavePath(int slot) => ResolveSaveTarget(new SaveTarget(Math.Max(1, slot)));

    public bool SlotExists(int slot)
    {
        string path = SlotSavePath(slot);
        if (string.Equals(path, _savePath, PathComparison))
        {
            return HasSave;
        }
        return SaveCodec.Exists(path);
    }

    /// <summary>
    /// Map a 220/221/222 file operand onto this port's slots. A numeric slot is one-based
    /// (WOLF names them SaveDataNN); an explicit path keeps only its file name and lives
    /// beside slot 01, so a retail "Save/System.sav" can never write outside the user dir.
    /// </summary>
    public string ResolveSaveTarget(SaveTarget file)
    {
        string dir = Path.GetDirectoryName(_savePath) ?? ".";
        if (file.IsPath)
        {
            string name = Path.GetFileName(file.Path.Replace('\\', '/'));
            if (name.Length == 0) name = "System.sav";
            string targetName = Path.ChangeExtension(name, ".json");
            string candidate = Path.Combine(dir, targetName);
            if (!File.Exists(candidate) && Directory.Exists(dir))
            {
                // Retail scripts mix "save/system.sav" (TitleMap) and "Save/System.sav" (CE131);
                // share one backing file on case-sensitive filesystems without breaking callers
                // that inspect System.json on creation.
                foreach (string existing in Directory.EnumerateFiles(dir, "*.json"))
                {
                    if (string.Equals(Path.GetFileName(existing), targetName, StringComparison.OrdinalIgnoreCase))
                        return existing;
                }
                string bakName = targetName + ".bak";
                foreach (string existingBak in Directory.EnumerateFiles(dir, "*.json.bak"))
                {
                    if (string.Equals(Path.GetFileName(existingBak), bakName, StringComparison.OrdinalIgnoreCase))
                        return existingBak[..^4];
                }
            }
            return candidate;
        }
        int slot = file.Slot <= 1 ? 1 : file.Slot;
        return Path.Combine(dir, $"save_{slot:00}.json");
    }

    public static string FormatSlotLabel(string path)
    {
        string fileName = Path.GetFileName(path);
        if (fileName.StartsWith("save_", StringComparison.OrdinalIgnoreCase)
            && fileName.EndsWith(".json", StringComparison.OrdinalIgnoreCase)
            && int.TryParse(fileName.AsSpan(5, fileName.Length - 10), out int slot))
        {
            return $"slot {slot:00}";
        }
        if (string.Equals(fileName, "quicksave.json", StringComparison.OrdinalIgnoreCase))
        {
            return "quicksave";
        }
        return string.IsNullOrEmpty(fileName) ? "slot" : fileName;
    }

    public SaveState BuildSystemState(int frames)
    {
        var state = new SaveState
        {
            SavedAt = DateTime.UtcNow.ToString("o"),
            PlayFrames = frames,
            Vars = _vm.Vm.Vars.SnapshotNums(),
            StrVars = _vm.Vm.Vars.SnapshotStrings(),
            SysVars = _vm.Vm.Vars.SnapshotSysNums(),
            SysStrVars = _vm.Vm.Vars.SnapshotSysStrings(),
        };
        foreach (var bank in _vm.Vm.CommonSelf) state.CommonSelf.Add(SavedSelf(bank.Key, bank.Value));
        return state;
    }

    public void SaveSystemData(string path, int frames)
    {
        try
        {
            SaveCodec.Write(path, BuildSystemState(frames));
            InvalidateSaveCache(path);
        }
        catch (Exception ex)
        {
            _view.SetStatus("system save failed: " + ex.Message);
        }
    }

    public void LoadSystemData(string path)
    {
        SaveState? state = SaveCodec.Read(path);
        if (state == null)
        {
            _view.SetStatus("system load: no system data at " + Path.GetFileName(path));
            return;
        }
        _vm.Vm.Vars.Restore(state.Vars, state.StrVars, state.SysVars, state.SysStrVars);
        _view.SetStatus("system load: restored " + Path.GetFileName(path));
    }

    public bool WriteSlot(string path, int slot, SaveState state)
    {
        try
        {
            SaveCodec.Write(path, state);
            InvalidateSaveCache(path);
            return true;
        }
        catch (Exception ex)
        {
            _view.SetStatus("save failed: " + ex.Message);
            return false;
        }
    }

    public void SyncSystemSlotMarkers(int clampedSlot)
    {
        string systemPath = ResolveSaveTarget(new SaveTarget("Save/System.sav"));
        try
        {
            SaveState systemState = SaveCodec.Read(systemPath) ?? new SaveState();
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 0, Value = clampedSlot });
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 626, Value = _vm.Vm.Vars.Get(531) });
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + 546, Value = clampedSlot });
            SaveCodec.Write(systemPath, systemState);
            InvalidateSaveCache(systemPath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException
                                   or ArgumentException or NotSupportedException or InvalidOperationException)
        {
            _view.SetStatus("system save failed: " + ex.Message);
        }
    }

    public int? HandleSaveLoad(
        SaveOp op,
        int frames,
        Action<string> queueGameLoad,
        Action<string> queueSystemLoad,
        Func<string, int, bool> saveGameTo)
    {
        string path = ResolveSaveTarget(op.File);
        switch (op.Opcode)
        {
            case 220:
                // A path target is the retail system data (banks only); a slot target is a
                // whole game save. Full loads are queued for the frame boundary.
                if (op.File.IsPath)
                {
                    if (op.Mode == 1) queueSystemLoad(path);
                    else SaveSystemData(path, frames);
                }
                else if (op.Mode == 1)
                {
                    queueGameLoad(path);
                }
                else
                {
                    int slot = Math.Max(1, op.File.Slot);
                    bool ok = saveGameTo(path, slot);
                    if (op.Owner == 435)
                        _view.SetStatus($"[AutoSave] slot={slot} {(ok ? "ok" : "fail")}");
                }
                return null;
            case 221:
            {
                if (op.Address == Magic.SysBase + 24)
                {
                    if (!SaveCodec.Exists(path)) return 0;
                    return ReadCachedSave(path).Exists ? 1 : 0;
                }
                CachedSaveEntry cached = ReadCachedSave(path);
                // A missing tier is the normal "no data here" answer for the existence probes.
                return cached.State == null ? 0 : ReadSaveAddress(cached.State, cached.OverlayIndex, op);
            }
            case 222:
            {
                SaveState state = SaveCodec.Read(path) ?? new SaveState();
                if (WriteSaveAddress(state, op))
                {
                    try
                    {
                        SaveCodec.Write(path, state);
                        InvalidateSaveCache(path);
                    }
                    catch (Exception ex)
                    {
                        _view.SetStatus("save write failed: " + ex.Message);
                    }
                }
                return null;
            }
        }
        return null;
    }

    /// <summary>Read a 221 address: CData cells read target save's overlay via O(1) index; variable banks decode via Magic.</summary>
    private int ReadSaveAddress(SaveState state, GameDatabase.OverlayIndex? overlayIndex, SaveOp op)
    {
        int address = op.Address;
        if (address is >= 1_100_000_000 and < 1_200_000_000)
        {
            int offset = address - 1_100_000_000;
            int dbType = offset / 1_000_000;
            int rem = offset % 1_000_000;
            int row = rem / 100;
            int col = rem % 100;
            var dbOp = new DbOp
            {
                Container = DbContainer.CData,
                Direction = DbDirection.Read,
                DbType = dbType,
                DataId = row,
                ItemId = col,
                Destination = op.Destination
            };
            DbValue? cell = _vm.DatabaseStore != null
                ? _vm.ReadDatabaseWithOverlayIndex(dbOp, overlayIndex)
                : _vm.ReadDatabaseWithOverlay(dbOp, state.DbOverlays);
            if (cell == null) return 0;
            if (cell.IsString)
            {
                op.LoadedString = cell.Str;
                return 0;
            }
            return cell.Num;
        }

        var magic = Magic.Decode(address);
        switch (magic.Kind)
        {
            case MagicKind.Var:
            {
                int index = (int)magic.Index;
                return index >= 0 && index < state.Vars.Count ? state.Vars[index] : 0;
            }
            case MagicKind.StrVar:
            {
                int index = (int)magic.Index;
                op.LoadedString = index >= 0 && index < state.StrVars.Count ? state.StrVars[index] : "";
                return 0;
            }
            case MagicKind.SelfVar:
            {
                EventSelfVars? bank = FindSelfBank(state, op.Owner);
                int index = (int)magic.Index;
                if (magic.IsStringSlot)
                {
                    if (bank == null)
                    {
                        op.LoadedString = "";
                        return 0;
                    }
                    if (bank.StrMap != null && bank.StrMap.TryGetValue(index, out string? sVal))
                    {
                        op.LoadedString = sVal ?? "";
                        return 0;
                    }
                    op.LoadedString = index >= 0 && index < bank.Strings.Length ? (bank.Strings[index] ?? "") : "";
                    return 0;
                }
                if (bank == null) return 0;
                if (bank.NumMap != null && bank.NumMap.TryGetValue(index, out int value)) return value;
                return index >= 0 && index < bank.Nums.Length ? bank.Nums[index] : 0;
            }
            case MagicKind.SysVar:
            {
                if (magic.IsSysStr)
                {
                    int strIdx = (int)(magic.Index - Magic.SysStrOffset);
                    op.LoadedString = strIdx >= 0 && strIdx < state.SysStrVars.Count ? state.SysStrVars[strIdx] : "";
                    return 0;
                }
                int index = (int)magic.Index;
                if (index == 24) return 1;
                if (index == 29) return (int)(state.PlayFrames / 60);
                return index >= 0 && index < state.SysVars.Count ? state.SysVars[index] : 0;
            }
            default:
                return 0;
        }
    }

    /// <summary>Write one 222 value; false means the address is outside every bank.</summary>
    public static bool WriteSaveAddress(SaveState state, SaveOp op)
    {
        var magic = Magic.Decode(op.Address);
        switch (magic.Kind)
        {
            case MagicKind.Var:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.Vars.Count <= index) state.Vars.Add(0);
                state.Vars[index] = op.Value;
                return true;
            }
            case MagicKind.StrVar:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.StrVars.Count <= index) state.StrVars.Add("");
                state.StrVars[index] = op.StringValue ?? "";
                return true;
            }
            case MagicKind.SelfVar:
            {
                int index = (int)magic.Index;
                if (index < 0 || index >= SelfVars.TotalSlots) return false;
                EventSelfVars? bank = FindSelfBank(state, op.Owner);
                if (bank == null)
                {
                    bank = new EventSelfVars { Id = op.Owner };
                    state.SelfVars.Add(bank);
                }
                if (magic.IsStringSlot || op.IsString)
                {
                    bank.StrMap ??= new Dictionary<int, string>();
                    bank.StrMap[index] = op.StringValue ?? "";
                }
                else
                {
                    bank.NumMap ??= new Dictionary<int, int>();
                    bank.NumMap[index] = op.Value;
                }
                return true;
            }
            case MagicKind.SysVar:
            {
                if (magic.IsSysStr)
                {
                    int strIdx = (int)(magic.Index - Magic.SysStrOffset);
                    if (strIdx < 0 || strIdx >= Variables.MaxSlots) return false;
                    while (state.SysStrVars.Count <= strIdx) state.SysStrVars.Add("");
                    state.SysStrVars[strIdx] = op.StringValue ?? "";
                    return true;
                }
                int index = (int)magic.Index;
                if (index == 29)
                {
                    state.PlayFrames = Math.Max(0, op.Value) * 60L;
                    return true;
                }
                if (index < 0 || index >= Variables.MaxSlots) return false;
                while (state.SysVars.Count <= index) state.SysVars.Add(0);
                state.SysVars[index] = op.Value;
                return true;
            }
            default:
                return false;
        }
    }

    private static EventSelfVars? FindSelfBank(SaveState state, int owner)
    {
        foreach (var bank in state.SelfVars) if (bank.Id == owner) return bank;
        foreach (var bank in state.CommonSelf) if (bank.Id == owner) return bank;
        return null;
    }

    /// <summary>Refresh NG+ / clear-unlock variables from System.json and system2.json before New Game.</summary>
    public void SyncPersistentSystemVarsFromDisk()
    {
        string sys1Path = ResolveSaveTarget(new SaveTarget("Save/System.sav"));
        SaveState? sys1 = ReadCachedSave(sys1Path).State;
        if (sys1 != null)
        {
            int[] sys1MergeVars = [300, 80, 81, 82, 83, 84, 85, 86, 87, 324, 325, 326, 327, 328, 503, 546, 550, 626];
            foreach (int v in sys1MergeVars)
            {
                int diskVal = GetVarOrDefault(sys1.Vars, v);
                if (diskVal > _vm.Vm.Vars.Get(v)) _vm.Vm.Vars.Set(v, diskVal);
            }
            int diskVol = GetVarOrDefault(sys1.Vars, 357);
            if (diskVol > 0 && _vm.Vm.Vars.Get(357) <= 0) _vm.Vm.Vars.Set(357, diskVol);
        }

        string sys2Path = ResolveSaveTarget(new SaveTarget("save/system2.sav"));
        SaveState? sys2 = ReadCachedSave(sys2Path).State;
        if (sys2 != null)
        {
            int v503 = GetVarOrDefault(sys2.Vars, 503);
            if (v503 > _vm.Vm.Vars.Get(503)) _vm.Vm.Vars.Set(503, v503);
            int v531 = GetVarOrDefault(sys2.Vars, 531);
            if (v531 > _vm.Vm.Vars.Get(626)) _vm.Vm.Vars.Set(626, v531);
            int v550 = GetVarOrDefault(sys2.Vars, 550);
            if (v550 > _vm.Vm.Vars.Get(550)) _vm.Vm.Vars.Set(550, v550);
        }
    }

    public static EventSelfVars SavedSelf(int id, SelfVars vars)
    {
        return EventSelfVars.Pack(id, vars.SnapshotNums(), vars.SnapshotStrings());
    }

    public static int GetVarOrDefault(IReadOnlyList<int> vars, int index) =>
        index >= 0 && index < vars.Count ? vars[index] : 0;

    public static void MergeMaxSaveVar(SaveState systemState, int varIndex, int candidate)
    {
        if (candidate <= 0) return;
        int current = GetVarOrDefault(systemState.Vars, varIndex);
        if (candidate > current)
        {
            WriteSaveAddress(systemState, new SaveOp { Opcode = 222, Address = Magic.VarBase + varIndex, Value = candidate });
        }
    }

    /// <summary>Validate remake save JSON against the SaveCodec schema and semantic constraints.</summary>
    public static bool ValidateSaveJson(string json, out string error)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            error = "save JSON is empty";
            return false;
        }
        try
        {
            if (new System.Text.UTF8Encoding(false, true).GetByteCount(json) > SaveCodec.MaxSaveBytes)
            {
                error = $"save JSON exceeds {SaveCodec.MaxSaveBytes} bytes";
                return false;
            }
            using JsonDocument doc = JsonDocument.Parse(json);
            if (doc.RootElement.ValueKind != JsonValueKind.Object)
            {
                error = "save JSON root must be an object";
                return false;
            }
            if (!doc.RootElement.TryGetProperty("schema", out JsonElement schemaProp)
                || schemaProp.ValueKind != JsonValueKind.String
                || schemaProp.GetString() != SaveState.SchemaName)
            {
                error = $"missing or invalid save schema (expected {SaveState.SchemaName})";
                return false;
            }
            if (!doc.RootElement.TryGetProperty("schema_version", out JsonElement verProp)
                || verProp.ValueKind != JsonValueKind.Number)
            {
                error = "missing or invalid save schema_version";
                return false;
            }
        }
        catch (Exception ex) when (ex is JsonException or ArgumentException or System.Text.EncoderFallbackException)
        {
            error = "malformed save JSON: " + ex.Message;
            return false;
        }
        if (SaveCodec.FromJson(json) == null)
        {
            error = "save JSON failed schema or semantic validation";
            return false;
        }
        error = "";
        return true;
    }
}
