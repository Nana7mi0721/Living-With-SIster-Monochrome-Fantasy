using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Lwsmf.Core;

/// <summary>
/// Engine database store behind opcode 250: CDataBase, DataBase and SysDatabase.
///
/// Mirrors crates/wolf-game/src/db.rs. The exported rows are the base layer, script
/// writes live in an overlay, and the sentinel rows/columns (-1/-2, 100+n extension
/// slots) are answered here rather than by the VM. database.json lists fields in
/// logical column order, so a column is the index into <c>fields</c>, not into the
/// shorter numeric/text array.
/// </summary>
public sealed class GameDatabase
{
    public const int RowCountQuery = -1;
    public const int RowCountSet = -2;
    public const int FirstEmptyColumn = -1;
    public const int RowClearColumn = -2;
    public const int ExtensionRowBase = 100;

    // crates/wolf-game/src/db.rs: the engine owns the coordinate scale and the
    // system window skin even though CDataBase type 18 rows 97/98 store 0.
    private const int CoordinateScaleType = 18;
    private const int CoordinateScaleXRow = 97;
    private const int CoordinateScaleYRow = 98;
    private const int WindowSkinType = 18;
    private const int WindowSkinRow = 0;
    private const int WindowSkinColumn = 24;
    private const string WindowSkinFile = "SystemFile/WindowBase.png";

    internal sealed class ContainerDto
    {
        [JsonPropertyName("types")] public List<TypeDto> Types { get; set; } = new();
    }

    internal sealed class RootDto
    {
        [JsonPropertyName("cdata")] public ContainerDto? Cdata { get; set; }
        [JsonPropertyName("database")] public ContainerDto? Database { get; set; }
        [JsonPropertyName("system")] public ContainerDto? System { get; set; }
    }

    internal sealed class TypeDto
    {
        [JsonPropertyName("data_id_method")] public int DataIdMethod { get; set; }
        [JsonPropertyName("fields")] public List<FieldDto> Fields { get; set; } = new();
        [JsonPropertyName("row_count")] public int RowCount { get; set; }
        [JsonPropertyName("rows")] public List<RowDto> Rows { get; set; } = new();
    }

    internal sealed class FieldDto
    {
        [JsonPropertyName("index")] public int Index { get; set; }
        [JsonPropertyName("kind")] public string Kind { get; set; } = "";
        [JsonPropertyName("raw")] public int Raw { get; set; }
    }

    internal sealed class RowDto
    {
        [JsonPropertyName("numbers")] public List<int> Numbers { get; set; } = new();
        [JsonPropertyName("strings")] public List<string> Strings { get; set; } = new();
    }

    private sealed class Field
    {
        public bool IsString { get; init; }
        public int Index { get; init; }
    }

    private sealed class Row
    {
        public int[] Numbers { get; init; } = Array.Empty<int>();
        public string[] Strings { get; init; } = Array.Empty<string>();
    }

    private sealed class Table
    {
        public Field[] Fields { get; init; } = Array.Empty<Field>();
        public List<Row> Rows { get; init; } = new();
    }

    private sealed class ProjectType
    {
        public string Name { get; init; } = "";
        public List<string> Fields { get; init; } = new();
    }

    private readonly Dictionary<DbContainer, List<Table>> _tables = new();
    private readonly Dictionary<DbContainer, ProjectType?[]> _projects = new();
    private readonly Dictionary<(DbContainer, int), int> _rowCounts = new();
    private readonly Dictionary<(DbContainer, int, int, int), int> _cells = new();
    private readonly Dictionary<(DbContainer, int, int, int), string> _texts = new();
    private readonly Dictionary<(DbContainer, int, int, int), int> _engineCells = new();
    private readonly Dictionary<(DbContainer, int, int, int), string> _engineTexts = new();

    public string SourcePath { get; }

    /// <summary>Inspect a CDataBase type's field string/numeric flags and base row count for .sav export.</summary>
    public bool TryGetCDataSchema(int dbType, out bool[] fieldIsString, out int baseRowCount)
    {
        if (_tables.TryGetValue(DbContainer.CData, out var tables) && dbType >= 0 && dbType < tables.Count)
        {
            Table table = tables[dbType];
            fieldIsString = new bool[table.Fields.Length];
            for (int i = 0; i < table.Fields.Length; i++)
            {
                fieldIsString[i] = table.Fields[i].IsString;
            }
            baseRowCount = table.Rows.Count;
            return true;
        }
        fieldIsString = Array.Empty<bool>();
        baseRowCount = 0;
        return false;
    }

    private GameDatabase(string sourcePath)
    {
        SourcePath = sourcePath;
        _engineCells[(DbContainer.CData, CoordinateScaleType, CoordinateScaleXRow, 0)] = 32;
        _engineCells[(DbContainer.CData, CoordinateScaleType, CoordinateScaleYRow, 0)] = 24;
        _engineTexts[(DbContainer.Data, WindowSkinType, WindowSkinRow, WindowSkinColumn)] = WindowSkinFile;
    }

    /// <summary>
    /// Loads game_data/database.json and, when available, the editor labels from the
    /// BasicData .project companions. A missing companion only disables label drift
    /// correction; the numeric table is still usable.
    /// </summary>
    public static GameDatabase? Load(string databasePath, string? basicDataDir = null)
    {
        string? actualPath = JsonArtifactIo.ResolvePath(databasePath);
        if (actualPath == null) return null;
        RootDto? root;
        try
        {
            using Stream stream = JsonArtifactIo.OpenReadStream(actualPath);
            root = JsonSerializer.Deserialize(stream, LwsmfJsonContext.Default.RootDto);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[GameDatabase] failed to read {actualPath}: {ex.Message}");
            return null;
        }
        if (root == null) return null;

        var db = new GameDatabase(actualPath);
        db.AddContainer(DbContainer.CData, root.Cdata);
        db.AddContainer(DbContainer.Data, root.Database);
        db.AddContainer(DbContainer.Sys, root.System);
        if (!string.IsNullOrEmpty(basicDataDir))
        {
            db.LoadProject(DbContainer.CData, Path.Combine(basicDataDir, "CDataBase.project"));
            db.LoadProject(DbContainer.Data, Path.Combine(basicDataDir, "DataBase.project"));
            db.LoadProject(DbContainer.Sys, Path.Combine(basicDataDir, "SysDatabase.project"));
        }
        return db;
    }

    private void AddContainer(DbContainer container, ContainerDto? dto)
    {
        var tables = new List<Table>();
        if (dto != null)
        {
            foreach (var typeDto in dto.Types)
            {
                var fields = new Field[typeDto.Fields.Count];
                for (int i = 0; i < fields.Length; i++)
                {
                    var field = typeDto.Fields[i];
                    fields[i] = new Field { IsString = field.Kind == "str", Index = field.Index };
                }
                var rows = new List<Row>(typeDto.Rows.Count);
                foreach (var rowDto in typeDto.Rows)
                {
                    rows.Add(new Row
                    {
                        Numbers = rowDto.Numbers.ToArray(),
                        Strings = rowDto.Strings.ToArray()
                    });
                }
                tables.Add(new Table { Fields = fields, Rows = rows });
            }
        }
        _tables[container] = tables;
    }

    private void LoadProject(DbContainer container, string path)
    {
        if (!File.Exists(path)) return;
        try
        {
            var parsed = ParseProject(File.ReadAllBytes(path), TableCount(container));
            if (parsed != null) _projects[container] = parsed;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[GameDatabase] ignored project {path}: {ex.Message}");
        }
    }

    private int TableCount(DbContainer container)
        => _tables.TryGetValue(container, out var tables) ? tables.Count : 0;

    /// <summary>Editor type name for a container/type, when the .project was loaded.</summary>
    public string? TypeName(DbContainer container, int dbType)
    {
        if (!_projects.TryGetValue(container, out var project)) return null;
        if (dbType < 0 || dbType >= project.Length) return null;
        return project[dbType]?.Name;
    }

    /// <summary>Number of logical fields of one table, or -1 when it does not exist.</summary>
    public int FieldCount(DbContainer container, int dbType)
    {
        if (!_tables.TryGetValue(container, out var tables)) return -1;
        if (dbType < 0 || dbType >= tables.Count) return -1;
        return tables[dbType].Fields.Length;
    }

    /// <summary>Drop every runtime 250 write (cells, texts, row counts) and return to .dat data.</summary>
    public void ClearOverlays()
    {
        _cells.Clear();
        _texts.Clear();
        _rowCounts.Clear();
    }

    /// <summary>Capture all runtime op250 overlay writes in deterministic order for SaveState persistence.</summary>
    public SavedDbOverlays SnapshotOverlays()
    {
        var snapshot = new SavedDbOverlays();
        foreach (var pair in _rowCounts)
        {
            snapshot.RowCounts.Add(new SavedDbRowCount
            {
                Container = (int)pair.Key.Item1,
                Type = pair.Key.Item2,
                Count = pair.Value
            });
        }
        snapshot.RowCounts.Sort((a, b) =>
        {
            int c = a.Container.CompareTo(b.Container);
            return c != 0 ? c : a.Type.CompareTo(b.Type);
        });

        foreach (var pair in _cells)
        {
            snapshot.Nums.Add(new SavedDbNumCell
            {
                Container = (int)pair.Key.Item1,
                Type = pair.Key.Item2,
                Row = pair.Key.Item3,
                Column = pair.Key.Item4,
                Value = pair.Value
            });
        }
        snapshot.Nums.Sort((a, b) =>
        {
            int c = a.Container.CompareTo(b.Container);
            if (c != 0) return c;
            c = a.Type.CompareTo(b.Type);
            if (c != 0) return c;
            c = a.Row.CompareTo(b.Row);
            return c != 0 ? c : a.Column.CompareTo(b.Column);
        });

        foreach (var pair in _texts)
        {
            snapshot.Texts.Add(new SavedDbTextCell
            {
                Container = (int)pair.Key.Item1,
                Type = pair.Key.Item2,
                Row = pair.Key.Item3,
                Column = pair.Key.Item4,
                Value = pair.Value
            });
        }
        snapshot.Texts.Sort((a, b) =>
        {
            int c = a.Container.CompareTo(b.Container);
            if (c != 0) return c;
            c = a.Type.CompareTo(b.Type);
            if (c != 0) return c;
            c = a.Row.CompareTo(b.Row);
            return c != 0 ? c : a.Column.CompareTo(b.Column);
        });
        return snapshot;
    }

    /// <summary>Replace all runtime op250 overlays with a loaded save's sparse overlay layer, pruning cells matching base data.</summary>
    public void RestoreOverlays(SavedDbOverlays? overlays)
    {
        ClearOverlays();
        if (overlays == null) return;
        foreach (var rc in overlays.RowCounts)
        {
            if (rc.Container is < 0 or > 2 || rc.Type < 0 || rc.Count < 0) continue;
            var container = (DbContainer)rc.Container;
            if (rc.Count == BaseRowCount(container, rc.Type)) continue;
            _rowCounts[(container, rc.Type)] = rc.Count;
        }
        foreach (var cell in overlays.Nums)
        {
            if (cell.Container is < 0 or > 2 || cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            var container = (DbContainer)cell.Container;
            if (TryTableNum(container, cell.Type, cell.Row, cell.Column, out int baseNum) && baseNum == cell.Value)
            {
                continue;
            }
            var key = (container, cell.Type, cell.Row, cell.Column);
            _cells[key] = cell.Value;
            _texts.Remove(key);
        }
        foreach (var cell in overlays.Texts)
        {
            if (cell.Container is < 0 or > 2 || cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            var container = (DbContainer)cell.Container;
            string textVal = cell.Value ?? "";
            if (TryTableText(container, cell.Type, cell.Row, cell.Column, out string baseText)
                && string.Equals(baseText, textVal, StringComparison.Ordinal))
            {
                continue;
            }
            var key = (container, cell.Type, cell.Row, cell.Column);
            _texts[key] = textVal;
            _cells.Remove(key);
        }
    }

    /// <summary>
    /// Diff a retail VariableDatabase dump against the loaded base CDataBase tables and emit
    /// only modified row counts and cells, keeping imported save JSON compact (~50 KB vs ~2 MB).
    /// </summary>
    public SavedDbOverlays? BuildSparseOverlays(
        IReadOnlyDictionary<int, int>? rowCounts,
        IReadOnlyList<LegacySaveImporter.DatabaseCell> numCells,
        IReadOnlyList<LegacySaveImporter.DatabaseTextCell> textCells)
    {
        var overlays = new SavedDbOverlays();
        if (rowCounts != null)
        {
            foreach (var pair in rowCounts)
            {
                if (pair.Key < 0 || pair.Value < 0) continue;
                if (pair.Value == BaseRowCount(DbContainer.CData, pair.Key)) continue;
                overlays.RowCounts.Add(new SavedDbRowCount
                {
                    Container = (int)DbContainer.CData,
                    Type = pair.Key,
                    Count = pair.Value
                });
            }
            overlays.RowCounts.Sort((a, b) => a.Type.CompareTo(b.Type));
        }

        foreach (var cell in numCells)
        {
            if (cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            if (TryTableNum(DbContainer.CData, cell.Type, cell.Row, cell.Column, out int baseNum))
            {
                if (baseNum == cell.Value) continue;
            }
            else if (cell.Value == 0)
            {
                continue;
            }
            overlays.Nums.Add(new SavedDbNumCell
            {
                Container = (int)DbContainer.CData,
                Type = cell.Type,
                Row = cell.Row,
                Column = cell.Column,
                Value = cell.Value
            });
        }

        foreach (var cell in textCells)
        {
            if (cell.Type < 0 || cell.Row < 0 || cell.Column < 0) continue;
            string val = cell.Value ?? "";
            if (TryTableText(DbContainer.CData, cell.Type, cell.Row, cell.Column, out string baseText))
            {
                if (string.Equals(baseText, val, StringComparison.Ordinal)) continue;
            }
            else if (val.Length == 0)
            {
                continue;
            }
            overlays.Texts.Add(new SavedDbTextCell
            {
                Container = (int)DbContainer.CData,
                Type = cell.Type,
                Row = cell.Row,
                Column = cell.Column,
                Value = val
            });
        }

        if (overlays.RowCounts.Count == 0 && overlays.Nums.Count == 0 && overlays.Texts.Count == 0)
            return null;
        return overlays;
    }

    /// <summary>O(1) dictionary index built once per cached SaveState for op 221 CData probes.</summary>
    public sealed class OverlayIndex
    {
        public Dictionary<(DbContainer, int), int> RowCounts { get; } = new();
        public Dictionary<(DbContainer, int, int, int), int> Nums { get; } = new();
        public Dictionary<(DbContainer, int, int, int), string> Texts { get; } = new();

        public static OverlayIndex? Build(SavedDbOverlays? overlays)
        {
            if (overlays == null) return null;
            if ((overlays.RowCounts == null || overlays.RowCounts.Count == 0)
                && (overlays.Nums == null || overlays.Nums.Count == 0)
                && (overlays.Texts == null || overlays.Texts.Count == 0))
            {
                return null;
            }
            var idx = new OverlayIndex();
            if (overlays.RowCounts != null)
            {
                foreach (var rc in overlays.RowCounts)
                {
                    if (rc.Container is < 0 or > 2 || rc.Type < 0 || rc.Count < 0) continue;
                    idx.RowCounts[((DbContainer)rc.Container, rc.Type)] = rc.Count;
                }
            }
            if (overlays.Nums != null)
            {
                foreach (var c in overlays.Nums)
                {
                    if (c.Container is < 0 or > 2 || c.Type < 0 || c.Row < 0 || c.Column < 0) continue;
                    idx.Nums[((DbContainer)c.Container, c.Type, c.Row, c.Column)] = c.Value;
                }
            }
            if (overlays.Texts != null)
            {
                foreach (var c in overlays.Texts)
                {
                    if (c.Container is < 0 or > 2 || c.Type < 0 || c.Row < 0 || c.Column < 0) continue;
                    idx.Texts[((DbContainer)c.Container, c.Type, c.Row, c.Column)] = c.Value ?? "";
                }
            }
            return idx;
        }
    }

    /// <summary>O(1) variant of <see cref="ReadWithOverlay"/> using a pre-indexed <see cref="OverlayIndex"/>.</summary>
    public DbValue? ReadWithOverlayIndex(DbOp op, OverlayIndex? index)
    {
        var container = op.Container;
        var (dbType, column) = ResolveLabels(container, op.DbType, op.ItemId, op.TypeLabel, op.FieldLabel);
        int row = op.DataId;

        int overlayRowCount = 0;
        bool hasOverlayRowCount = index != null && index.RowCounts.TryGetValue((container, dbType), out overlayRowCount);
        int effectiveRowCount = hasOverlayRowCount ? overlayRowCount : BaseRowCount(container, dbType);
        if (row == RowCountQuery) return new DbValue(effectiveRowCount);
        if (column == FirstEmptyColumn)
        {
            return new DbValue(FirstEmptyRowCore(
                container,
                dbType,
                effectiveRowCount,
                (r, c) => index != null && index.Texts.TryGetValue((container, dbType, r, c), out string? ov) ? ov : TableText(container, dbType, r, c),
                (r, c) => index != null && index.Nums.TryGetValue((container, dbType, r, c), out int ov) ? ov : TableNum(container, dbType, r, c)));
        }
        bool wantsText = IsTextDestination(op.Destination);
        if (column < 0) return EmptyLike(wantsText);

        var key = (container, dbType, row, column);
        if (!wantsText)
        {
            if ((index != null && index.Nums.TryGetValue(key, out int number))
                || _engineCells.TryGetValue(key, out number))
            {
                return new DbValue(number);
            }
            if (index != null && index.Texts.TryGetValue(key, out string? overlayText))
            {
                return new DbValue(overlayText);
            }
            if (_engineTexts.TryGetValue(key, out string? engineText))
            {
                return new DbValue(engineText);
            }
        }
        else
        {
            if (index != null && index.Texts.TryGetValue(key, out string? overlayText))
            {
                return new DbValue(overlayText);
            }
            if (_engineTexts.TryGetValue(key, out string? engineText))
            {
                return new DbValue(engineText);
            }
            if ((index != null && index.Nums.TryGetValue(key, out int number))
                || _engineCells.TryGetValue(key, out number))
            {
                return new DbValue(number);
            }
        }

        bool hasTable = _tables.TryGetValue(container, out var tables)
            && dbType >= 0 && dbType < tables.Count;
        if (hasTable && row >= 0 && row < effectiveRowCount)
        {
            if (TryTableNum(container, dbType, row, column, out int tableNum)) return new DbValue(tableNum);
            if (TryTableText(container, dbType, row, column, out string tableText)) return new DbValue(tableText);
        }
        if (row >= ExtensionRowBase) return EmptyLike(wantsText);
        if (row >= 0 && hasOverlayRowCount) return EmptyLike(wantsText);
        int fieldCount = FieldCount(container, dbType);
        if (fieldCount >= 0 && column >= fieldCount) return EmptyLike(wantsText);
        return null;
    }

    /// <summary>Apply retail VariableDatabase row counts differing from the base .dat table.</summary>
    public void ApplySavedRowCounts(IReadOnlyDictionary<int, int> rowCounts)
    {
        foreach (var pair in rowCounts)
        {
            if (pair.Key < 0 || pair.Value < 0) continue;
            if (pair.Value == BaseRowCount(DbContainer.CData, pair.Key))
                _rowCounts.Remove((DbContainer.CData, pair.Key));
            else
                _rowCounts[(DbContainer.CData, pair.Key)] = pair.Value;
        }
    }

    /// <summary>
    /// Evaluate a database read against the base .dat tables plus an explicit save's
    /// overlay layer, completely isolated from the live runtime overlay.
    /// </summary>
    public DbValue? ReadWithOverlay(DbOp op, SavedDbOverlays? overlays)
    {
        var container = op.Container;
        var (dbType, column) = ResolveLabels(container, op.DbType, op.ItemId, op.TypeLabel, op.FieldLabel);
        int row = op.DataId;

        bool hasOverlayRowCount = TryFindOverlayRowCount(overlays, container, dbType, out int overlayRowCount);
        int effectiveRowCount = hasOverlayRowCount ? overlayRowCount : BaseRowCount(container, dbType);
        if (row == RowCountQuery) return new DbValue(effectiveRowCount);
        if (column == FirstEmptyColumn) return new DbValue(FirstEmptyRowWithOverlay(container, dbType, effectiveRowCount, overlays));
        bool wantsText = IsTextDestination(op.Destination);
        if (column < 0) return EmptyLike(wantsText);

        if (TryFindOverlayNum(overlays, container, dbType, row, column, out int number)
            || _engineCells.TryGetValue((container, dbType, row, column), out number))
        {
            return new DbValue(number);
        }
        if (TryFindOverlayText(overlays, container, dbType, row, column, out string overlayText))
        {
            return new DbValue(overlayText);
        }
        if (_engineTexts.TryGetValue((container, dbType, row, column), out string? engineText))
        {
            return new DbValue(engineText);
        }

        bool hasTable = _tables.TryGetValue(container, out var tables)
            && dbType >= 0 && dbType < tables.Count;
        if (hasTable && row >= 0 && row < effectiveRowCount)
        {
            if (TryTableNum(container, dbType, row, column, out number)) return new DbValue(number);
            if (TryTableText(container, dbType, row, column, out string tableText)) return new DbValue(tableText);
        }
        if (row >= ExtensionRowBase) return EmptyLike(wantsText);
        if (row >= 0 && hasOverlayRowCount) return EmptyLike(wantsText);
        int fieldCount = FieldCount(container, dbType);
        if (fieldCount >= 0 && column >= fieldCount) return EmptyLike(wantsText);
        return null;
    }

    private int BaseRowCount(DbContainer container, int dbType)
    {
        if (!_tables.TryGetValue(container, out var tables)) return 0;
        return dbType >= 0 && dbType < tables.Count ? tables[dbType].Rows.Count : 0;
    }

    private static bool TryFindOverlayRowCount(SavedDbOverlays? overlays, DbContainer container, int dbType, out int count)
    {
        if (overlays?.RowCounts != null)
        {
            int c = (int)container;
            for (int i = overlays.RowCounts.Count - 1; i >= 0; i--)
            {
                var rc = overlays.RowCounts[i];
                if (rc.Container == c && rc.Type == dbType)
                {
                    count = rc.Count;
                    return true;
                }
            }
        }
        count = 0;
        return false;
    }

    private static bool TryFindOverlayNum(SavedDbOverlays? overlays, DbContainer container, int dbType, int row, int column, out int value)
    {
        if (overlays?.Nums != null)
        {
            int c = (int)container;
            for (int i = overlays.Nums.Count - 1; i >= 0; i--)
            {
                var cell = overlays.Nums[i];
                if (cell.Container == c && cell.Type == dbType && cell.Row == row && cell.Column == column)
                {
                    value = cell.Value;
                    return true;
                }
            }
        }
        value = 0;
        return false;
    }

    private static bool TryFindOverlayText(SavedDbOverlays? overlays, DbContainer container, int dbType, int row, int column, out string value)
    {
        if (overlays?.Texts != null)
        {
            int c = (int)container;
            for (int i = overlays.Texts.Count - 1; i >= 0; i--)
            {
                var cell = overlays.Texts[i];
                if (cell.Container == c && cell.Type == dbType && cell.Row == row && cell.Column == column)
                {
                    value = cell.Value ?? "";
                    return true;
                }
            }
        }
        value = "";
        return false;
    }

    private int FirstEmptyRowWithOverlay(DbContainer container, int dbType, int count, SavedDbOverlays? overlays) =>
        FirstEmptyRowCore(
            container,
            dbType,
            count,
            (row, column) => TryFindOverlayText(overlays, container, dbType, row, column, out string overlay)
                ? overlay
                : TableText(container, dbType, row, column),
            (row, column) => TryFindOverlayNum(overlays, container, dbType, row, column, out int overlay)
                ? overlay
                : TableNum(container, dbType, row, column));

    /// <summary>Current row count, including a runtime 250 row-count write.</summary>
    public int RowCount(DbContainer container, int dbType)
    {
        if (_rowCounts.TryGetValue((container, dbType), out int count)) return count;
        if (!_tables.TryGetValue(container, out var tables)) return 0;
        return dbType >= 0 && dbType < tables.Count ? tables[dbType].Rows.Count : 0;
    }

    /// <summary>
    /// Resolves the editor labels a 250 command carries onto the table/column the
    /// .dat file uses. SysDatabase is left numeric, as in db.rs.
    /// </summary>
    public (int Type, int Column) ResolveLabels(
        DbContainer container, int dbType, int column, string? typeLabel, string? fieldLabel)
    {
        if (container == DbContainer.Sys) return (dbType, column);
        if (!_tables.TryGetValue(container, out var tables)) return (dbType, column);
        if (!_projects.TryGetValue(container, out var project)) return (dbType, column);

        string wantedType = NormaliseLabel(typeLabel);
        if (wantedType.Length > 0)
        {
            for (int i = 0; i < project.Length && i < tables.Count; i++)
            {
                if (NormaliseLabel(project[i]?.Name) == wantedType)
                {
                    dbType = i;
                    break;
                }
            }
        }

        string wantedField = NormaliseLabel(fieldLabel);
        if (wantedField.Length == 0) return (dbType, column);
        if (dbType < 0 || dbType >= tables.Count || dbType >= project.Length) return (dbType, column);
        var record = project[dbType];
        if (record == null || record.Fields.Count != tables[dbType].Fields.Length) return (dbType, column);
        for (int i = 0; i < record.Fields.Count; i++)
        {
            if (NormaliseLabel(record.Fields[i]) == wantedField) return (dbType, i);
        }
        return (dbType, column);
    }

    /// <summary>Handles one decoded 250 command.</summary>
    public DbValue? Execute(DbOp op)
    {
        if (op.Direction == DbDirection.Read) return Read(op);
        Write(op);
        return null;
    }

    public DbValue? Read(DbOp op)
    {
        var container = op.Container;
        var (dbType, column) = ResolveLabels(container, op.DbType, op.ItemId, op.TypeLabel, op.FieldLabel);
        int row = op.DataId;
        if (row == RowCountQuery) return new DbValue(RowCount(container, dbType));
        if (column == FirstEmptyColumn) return new DbValue(FirstEmptyRow(container, dbType));
        bool wantsText = IsTextDestination(op.Destination);
        if (column < 0) return EmptyLike(wantsText);

        if (TryEngineNum(container, dbType, row, column, out int number)) return new DbValue(number);
        if (TryEngineText(container, dbType, row, column, out string text)) return new DbValue(text);

        bool hasTable = _tables.TryGetValue(container, out var tables)
            && dbType >= 0 && dbType < tables.Count;
        if (hasTable && RowExists(container, dbType, row))
        {
            if (TryTableNum(container, dbType, row, column, out number)) return new DbValue(number);
            if (TryTableText(container, dbType, row, column, out string tableText)) return new DbValue(tableText);
        }
        if (row >= ExtensionRowBase) return EmptyLike(wantsText);
        if (row >= 0 && _rowCounts.ContainsKey((container, dbType))) return EmptyLike(wantsText);
        int fieldCount = FieldCount(container, dbType);
        if (fieldCount >= 0 && column >= fieldCount) return EmptyLike(wantsText);
        return null;
    }

    private void Write(DbOp op)
    {
        var container = op.Container;
        var (dbType, column) = ResolveLabels(container, op.DbType, op.ItemId, op.TypeLabel, op.FieldLabel);
        int row = op.DataId;
        if (row == RowCountSet)
        {
            if (op.Source is { IsString: false }) SetRowCount(container, dbType, op.Source.Num);
            return;
        }
        if (column == RowClearColumn)
        {
            ClearRow(container, dbType, row);
            return;
        }
        if (row < 0 || column < 0 || op.Source == null) return;
        if (op.Source.IsString)
        {
            SetText(container, dbType, row, column, op.Source.Str);
            return;
        }

        int current = 0;
        if (TryEngineNum(container, dbType, row, column, out int overlay))
        {
            current = overlay;
        }
        else if (RowExists(container, dbType, row)
                 && TryTableNum(container, dbType, row, column, out int fileValue))
        {
            current = fileValue;
        }
        SetNum(container, dbType, row, column, CombineCell(op.Op, current, op.Source.Num));
    }

    private static int CombineCell(byte op, int current, int operand) => op switch
    {
        1 => unchecked(current + operand),
        2 => unchecked(current - operand),
        3 => unchecked(current * operand),
        4 => operand == 0 ? current : (current == int.MinValue && operand == -1 ? int.MinValue : current / operand),
        5 => operand == 0 ? current : (current == int.MinValue && operand == -1 ? 0 : current % operand),
        6 => Math.Max(current, operand),
        7 => Math.Min(current, operand),
        _ => operand
    };

    private void SetRowCount(DbContainer container, int dbType, int count)
    {
        count = Math.Max(0, count);
        _rowCounts[(container, dbType)] = count;
        foreach (var key in new List<(DbContainer, int, int, int)>(_cells.Keys))
        {
            if (key.Item1 == container && key.Item2 == dbType && key.Item3 >= count && key.Item3 >= 0)
                _cells.Remove(key);
        }
        foreach (var key in new List<(DbContainer, int, int, int)>(_texts.Keys))
        {
            if (key.Item1 == container && key.Item2 == dbType && key.Item3 >= count && key.Item3 >= 0)
                _texts.Remove(key);
        }
    }

    private bool ClearRow(DbContainer container, int dbType, int row)
    {
        if (row < 0) return false;
        if (!_tables.TryGetValue(container, out var tables)) return false;
        if (dbType < 0 || dbType >= tables.Count) return false;
        var table = tables[dbType];
        for (int column = 0; column < table.Fields.Length; column++)
        {
            _cells.Remove((container, dbType, row, column));
            _texts.Remove((container, dbType, row, column));
        }
        for (int column = 0; column < table.Fields.Length; column++)
        {
            if (table.Fields[column].IsString) _texts[(container, dbType, row, column)] = "";
            else _cells[(container, dbType, row, column)] = 0;
        }
        return true;
    }

    private bool SetNum(DbContainer container, int dbType, int row, int column, int value)
    {
        if (column >= 0 && column < FieldCount(container, dbType))
        {
            // A numeric write never overwrites a text cell that already has text,
            // matching Databases::set_num in db.rs.
            var field = _tables[container][dbType].Fields[column];
            if (field.IsString)
            {
                string text = _texts.TryGetValue((container, dbType, row, column), out string? overlay)
                    ? overlay
                    : TableText(container, dbType, row, column);
                if (text.Length > 0) return false;
            }
        }
        _cells[(container, dbType, row, column)] = value;
        _texts.Remove((container, dbType, row, column));
        return true;
    }

    private void SetText(DbContainer container, int dbType, int row, int column, string value)
    {
        if (column >= 0 && column < FieldCount(container, dbType))
        {
            var field = _tables[container][dbType].Fields[column];
            if (!field.IsString)
            {
                if (int.TryParse(value, System.Globalization.NumberStyles.Integer,
                        System.Globalization.CultureInfo.InvariantCulture, out int parsed))
                {
                    SetNum(container, dbType, row, column, parsed);
                }
                return;
            }
        }
        _cells.Remove((container, dbType, row, column));
        _texts[(container, dbType, row, column)] = value;
    }

    private int FirstEmptyRow(DbContainer container, int dbType) =>
        FirstEmptyRowCore(
            container,
            dbType,
            RowCount(container, dbType),
            (row, column) => _texts.TryGetValue((container, dbType, row, column), out var overlay)
                ? overlay
                : TableText(container, dbType, row, column),
            (row, column) => _cells.TryGetValue((container, dbType, row, column), out var overlay)
                ? overlay
                : TableNum(container, dbType, row, column));

    private int FirstEmptyRowCore(
        DbContainer container,
        int dbType,
        int count,
        Func<int, int, string> readText,
        Func<int, int, int> readNum)
    {
        if (count <= 0) return 0;
        int fieldCount = FieldCount(container, dbType);
        if (fieldCount <= 0) return 0;
        var fields = _tables[container][dbType].Fields;
        for (int row = 0; row < count; row++)
        {
            bool empty = true;
            for (int column = 0; column < fieldCount; column++)
            {
                if (fields[column].IsString)
                {
                    if (readText(row, column).Length != 0) { empty = false; break; }
                }
                else
                {
                    if (readNum(row, column) != 0) { empty = false; break; }
                }
            }
            if (empty) return row;
        }
        return count;
    }

    private bool TryEngineNum(DbContainer container, int dbType, int row, int column, out int value)
    {
        if (_cells.TryGetValue((container, dbType, row, column), out value)) return true;
        return _engineCells.TryGetValue((container, dbType, row, column), out value);
    }

    private bool TryEngineText(DbContainer container, int dbType, int row, int column, out string value)
    {
        if (_texts.TryGetValue((container, dbType, row, column), out string? text))
        {
            value = text;
            return true;
        }
        if (_engineTexts.TryGetValue((container, dbType, row, column), out string? engine))
        {
            value = engine;
            return true;
        }
        value = "";
        return false;
    }

    private bool TryTableNum(DbContainer container, int dbType, int row, int column, out int value)
    {
        value = 0;
        if (!_tables.TryGetValue(container, out var tables)) return false;
        if (dbType < 0 || dbType >= tables.Count) return false;
        var table = tables[dbType];
        if (row < 0 || row >= table.Rows.Count || column < 0 || column >= table.Fields.Length) return false;
        var field = table.Fields[column];
        if (field.IsString || field.Index < 0 || field.Index >= table.Rows[row].Numbers.Length) return false;
        value = table.Rows[row].Numbers[field.Index];
        return true;
    }

    private bool TryTableText(DbContainer container, int dbType, int row, int column, out string value)
    {
        value = "";
        if (!_tables.TryGetValue(container, out var tables)) return false;
        if (dbType < 0 || dbType >= tables.Count) return false;
        var table = tables[dbType];
        if (row < 0 || row >= table.Rows.Count || column < 0 || column >= table.Fields.Length) return false;
        var field = table.Fields[column];
        if (!field.IsString || field.Index < 0 || field.Index >= table.Rows[row].Strings.Length) return false;
        value = table.Rows[row].Strings[field.Index] ?? "";
        return true;
    }

    private string TableText(DbContainer container, int dbType, int row, int column)
        => TryTableText(container, dbType, row, column, out string text) ? text : "";

    /// <summary>Engine cells first, then the .dat table, like <see cref="Read"/>.</summary>
    public string ReadText(DbContainer container, int dbType, int row, int column)
    {
        if (TryEngineText(container, dbType, row, column, out string text)) return text;
        return TableText(container, dbType, row, column);
    }

    /// <summary>
    /// Test/data precondition from crates/wolf-game/src/battle_seed.rs: a fresh game never
    /// writes the party/enemy counts, leader command slots or stats the battle events read.
    /// Never called by GameSession; tests and fixture builders opt in explicitly.
    /// </summary>
    public int SeedBattleCells(int partyCount = 1, int enemyCount = 1)
    {
        int written = 0;
        void Put(int dbType, int row, int column, int value)
        {
            if (SetNum(DbContainer.CData, dbType, row, column, value)) written++;
        }

        Put(18, 4, 0, partyCount);
        Put(18, 69, 0, enemyCount);
        int[] commandSlots = [3, 4, 6, 7, 0, 0, 0, 0];
        for (int i = 0; i < commandSlots.Length; i++) Put(0, 12, 47 + i, commandSlots[i]);
        int[] leaderStats = [100, 100, 50, 50, 20, 15, 10, 30, 30, 20, 10, 10];
        int[] leaderColumns = [20, 21, 22, 23, 24, 25, 26, 27, 28, 9, 10, 4];
        for (int i = 0; i < leaderStats.Length; i++) Put(0, 12, leaderColumns[i], leaderStats[i]);
        Put(7, 0, 0, 1);
        Put(8, 0, 0, 1);
        Put(8, 1, 0, 1);
        Put(8, 2, 0, 1);
        return written;
    }

    /// <summary>Overlay the numeric cells carried by a save's variable database.</summary>
    public int ApplySavedCells(IEnumerable<LegacySaveImporter.DatabaseCell> cells)
    {
        int written = 0;
        foreach (var cell in cells)
        {
            if (SetNum(DbContainer.CData, cell.Type, cell.Row, cell.Column, cell.Value)) written++;
        }
        return written;
    }

    /// <summary>Overlay the text cells carried by a save's variable database.</summary>
    public int ApplySavedTextCells(IEnumerable<LegacySaveImporter.DatabaseTextCell> cells)
    {
        int written = 0;
        foreach (var cell in cells)
        {
            SetText(DbContainer.CData, cell.Type, cell.Row, cell.Column, cell.Value);
            written++;
        }
        return written;
    }

    /// <summary>Command-window labels (CData type 19 column 0) used by battle seed tests.</summary>
    public int SeedBattleMenuText()
    {
        string[] labels = ["蓄力", "防御", "强斩", "反击", "普通攻击", "普通攻击", "普通攻击", "普通攻击"];
        for (int row = 0; row < labels.Length; row++)
            SetText(DbContainer.CData, 19, row, 0, labels[row]);
        return labels.Length;
    }

    private int TableNum(DbContainer container, int dbType, int row, int column)
        => TryTableNum(container, dbType, row, column, out int number) ? number : 0;

    private bool RowExists(DbContainer container, int dbType, int row)
        => row >= 0 && row < RowCount(container, dbType);

    private static bool IsTextDestination(int? destination)
        => destination is >= Magic.StrBase and < Magic.StrBase + 1_000_000;

    private static DbValue EmptyLike(bool wantsText) => wantsText ? new DbValue("") : new DbValue(0);

    private static string NormaliseLabel(string? label)
    {
        if (string.IsNullOrEmpty(label)) return "";
        return label.TrimEnd('\0')
            .Replace('—', '-')
            .Replace('―', '-')
            .Replace('　', ' ')
            .Trim();
    }

    private static ProjectType?[]? ParseProject(byte[] data, int typeCount)
    {
        if (data.Length < 4 || ReadU32(data, 0) != (uint)typeCount) return null;
        var result = new ProjectType?[typeCount];
        int at = 4;
        for (int i = 0; i < typeCount; i++)
        {
            var parsed = ParseProjectType(data, at);
            if (parsed == null) break;
            result[i] = parsed.Value.Type;
            at = parsed.Value.Next;
        }
        return result;
    }

    private static (ProjectType Type, int Next)? ParseProjectType(byte[] data, int at)
    {
        if (!NameAt(data, at, out string name, out at)) return null;
        if (at + 4 > data.Length) return null;
        uint fieldCount = ReadU32(data, at);
        at += 4;
        if (fieldCount > (uint)data.Length / 5) return null;
        var fields = new List<string>((int)fieldCount);
        for (uint i = 0; i < fieldCount; i++)
        {
            if (!NameAt(data, at, out string field, out at)) return null;
            fields.Add(field);
        }
        if (at + 4 > data.Length) return null;
        uint rowCount = ReadU32(data, at);
        at += 4;
        if (rowCount > (uint)data.Length / 5) return null;
        for (uint i = 0; i < rowCount; i++)
        {
            if (!NameAt(data, at, out _, out at)) return null;
        }
        if (!NameAt(data, at, out _, out at)) return null;
        if (at + 4 > data.Length) return null;
        uint typeBytes = ReadU32(data, at);
        at += 4;
        if (typeBytes > (uint)(data.Length - at)) return null;
        at += (int)typeBytes;

        if (!SkipNameBlock(data, ref at)) return null;
        if (at + 4 > data.Length) return null;
        uint entries = ReadU32(data, at);
        at += 4;
        for (uint i = 0; i < entries; i++)
        {
            if (at + 4 > data.Length) return null;
            uint arguments = ReadU32(data, at);
            at += 4;
            for (uint j = 0; j < arguments; j++)
            {
                if (!NameAt(data, at, out _, out at)) return null;
            }
        }
        if (at + 4 > data.Length) return null;
        entries = ReadU32(data, at);
        at += 4;
        for (uint i = 0; i < entries; i++)
        {
            if (at + 4 > data.Length) return null;
            uint arguments = ReadU32(data, at);
            at += 4;
            long bytes = (long)arguments * 4;
            if (bytes > data.Length - at) return null;
            at += (int)bytes;
        }
        if (at + 4 > data.Length) return null;
        uint defaults = ReadU32(data, at);
        at += 4;
        long defaultBytes = (long)defaults * 4;
        if (defaultBytes > data.Length - at) return null;
        at += (int)defaultBytes;
        return (new ProjectType { Name = name, Fields = fields }, at);
    }

    private static bool SkipNameBlock(byte[] data, ref int at)
    {
        if (at + 4 > data.Length) return false;
        uint count = ReadU32(data, at);
        at += 4;
        for (uint i = 0; i < count; i++)
        {
            if (!NameAt(data, at, out _, out at)) return false;
        }
        return true;
    }

    private static bool NameAt(byte[] data, int at, out string name, out int next)
    {
        name = "";
        next = at;
        if (at < 0 || at + 4 > data.Length) return false;
        uint length = ReadU32(data, at);
        if (length == 0 || length > 4096 || length > (uint)(data.Length - at - 4)) return false;
        int start = at + 4;
        int count = (int)length;
        if (data[start + count - 1] != 0) return false;
        name = Encoding.UTF8.GetString(data, start, count - 1);
        next = start + count;
        return true;
    }

    private static uint ReadU32(byte[] data, int at)
        => BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(at, 4));
}
