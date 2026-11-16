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

    private sealed class ContainerDto
    {
        [JsonPropertyName("types")] public List<TypeDto> Types { get; set; } = new();
    }

    private sealed class RootDto
    {
        [JsonPropertyName("cdata")] public ContainerDto? Cdata { get; set; }
        [JsonPropertyName("database")] public ContainerDto? Database { get; set; }
        [JsonPropertyName("system")] public ContainerDto? System { get; set; }
    }

    private sealed class TypeDto
    {
        [JsonPropertyName("data_id_method")] public int DataIdMethod { get; set; }
        [JsonPropertyName("fields")] public List<FieldDto> Fields { get; set; } = new();
        [JsonPropertyName("row_count")] public int RowCount { get; set; }
        [JsonPropertyName("rows")] public List<RowDto> Rows { get; set; } = new();
    }

    private sealed class FieldDto
    {
        [JsonPropertyName("index")] public int Index { get; set; }
        [JsonPropertyName("kind")] public string Kind { get; set; } = "";
        [JsonPropertyName("raw")] public int Raw { get; set; }
    }

    private sealed class RowDto
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
        if (!File.Exists(databasePath)) return null;
        RootDto? root;
        try
        {
            root = JsonSerializer.Deserialize<RootDto>(File.ReadAllText(databasePath, Encoding.UTF8));
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[GameDatabase] failed to read {databasePath}: {ex.Message}");
            return null;
        }
        if (root == null) return null;

        var db = new GameDatabase(Path.GetFullPath(databasePath));
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
            if (TryTableText(container, dbType, row, column, out text)) return new DbValue(text);
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
        4 when operand != 0 => current / operand,
        5 when operand != 0 => current % operand,
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

    private int FirstEmptyRow(DbContainer container, int dbType)
    {
        int count = RowCount(container, dbType);
        if (count <= 0) return 0;
        int fieldCount = FieldCount(container, dbType);
        if (fieldCount < 0) return 0;
        var fields = _tables[container][dbType].Fields;
        for (int row = 0; row < count; row++)
        {
            bool empty = true;
            for (int column = 0; column < fieldCount; column++)
            {
                if (fields[column].IsString)
                {
                    string text = _texts.TryGetValue((container, dbType, row, column), out var overlay)
                        ? overlay
                        : TableText(container, dbType, row, column);
                    if (text.Length != 0) { empty = false; break; }
                }
                else
                {
                    int number = _cells.TryGetValue((container, dbType, row, column), out var overlay)
                        ? overlay
                        : TableNum(container, dbType, row, column);
                    if (number != 0) { empty = false; break; }
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
