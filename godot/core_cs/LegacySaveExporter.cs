using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;

namespace Lwsmf.Core;

/// <summary>
/// Serializes a <see cref="SaveState"/> into a retail WOLF RPG Editor v2.20+ (<c>0x008C</c>)
/// encrypted <c>.sav</c> binary compatible with <c>crates/wolf-save</c> and original <c>Game.exe</c>.
/// </summary>
public static class LegacySaveExporter
{
    private const int HeaderLen = 0x14;
    private const byte StartByte = 0x19;
    private const byte EndByte = 0x19;
    private const byte Utf8Flag = 0x55;
    private const ushort FileVersion = 0x008C;
    private const int NormalVarCount = 800;
    private const int EventGroups = 550;
    private const int SelfNumCount = 100;
    private const int SelfStrCount = 5;

    private static readonly uint[] Part1TailVars = [0, 0, 0, 0, 100, 100, 0, 0, 0, 0, 100];

    /// <summary>Encode a slot <see cref="SaveState"/> into encrypted retail <c>.sav</c> bytes.</summary>
    public static byte[] ExportSaveState(SaveState state, GameDatabase? db = null, string title = "LWSMF Save")
    {
        ArgumentNullException.ThrowIfNull(state);
        byte[] plain = BuildPlaintext(state, db, title, includePictures: true, includeVarDb: true);
        return LegacySaveImporter.Decrypt(plain);
    }

    /// <summary>Encode a system <see cref="SaveState"/> into encrypted retail <c>System.sav</c> bytes.</summary>
    public static byte[] ExportSystemState(SaveState systemState, string title = "LWSMF System")
    {
        ArgumentNullException.ThrowIfNull(systemState);
        byte[] plain = BuildPlaintext(systemState, null, title, includePictures: false, includeVarDb: false);
        return LegacySaveImporter.Decrypt(plain);
    }

    /// <summary>Atomically write encrypted <c>.sav</c> bytes via temporary file replacement.</summary>
    public static void WriteAtomic(string path, byte[] data)
    {
        string? dir = Path.GetDirectoryName(Path.GetFullPath(path));
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);
        string temp = path + ".tmp";
        try
        {
            using (var stream = new FileStream(temp, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                stream.Write(data, 0, data.Length);
                stream.Flush(true);
            }
            File.Move(temp, path, overwrite: true);
        }
        catch
        {
            if (File.Exists(temp))
            {
                try { File.Delete(temp); } catch { }
            }
            throw;
        }
    }

    /// <summary>
    /// Encode a text picture string back into WOLF's SavePart5_1 <c>md1</c> style bytecode
    /// (<c>\E</c>, <c>\f[n]</c>, <c>\c[n]</c>) with trailing NUL byte.
    /// </summary>
    public static byte[] EncodePictureText(string text)
    {
        using var ms = new MemoryStream(Math.Max(16, text.Length * 2));
        int i = 0;
        while (i < text.Length)
        {
            if (text[i] == '\\' && TryEncodeEscape(text, i, out int consumed, out byte[]? escapeBytes))
            {
                ms.Write(escapeBytes!, 0, escapeBytes!.Length);
                i += consumed;
                continue;
            }
            int charLen = char.IsHighSurrogate(text[i]) && i + 1 < text.Length && char.IsLowSurrogate(text[i + 1]) ? 2 : 1;
            byte[] utf8 = Encoding.UTF8.GetBytes(text.AsSpan(i, charLen).ToString());
            ms.Write(utf8, 0, utf8.Length);
            i += charLen;
        }
        ms.WriteByte(0);
        return ms.ToArray();
    }

    private static bool TryEncodeEscape(string text, int index, out int consumed, out byte[]? bytes)
    {
        consumed = 0;
        bytes = null;
        if (index + 1 >= text.Length) return false;
        char kind = text[index + 1];
        if (kind == 'E')
        {
            consumed = 2;
            bytes = [1, 2, 5];
            return true;
        }
        if ((kind == 'f' || kind == 'c') && index + 2 < text.Length && text[index + 2] == '[')
        {
            int end = index + 3;
            int value = 0;
            while (end < text.Length && text[end] is >= '0' and <= '9')
            {
                value = value * 10 + (text[end] - '0');
                if (value > 1000) return false;
                end++;
            }
            if (end == index + 3 || end >= text.Length || text[end] != ']')
                return false;
            int code = value + 14;
            if (code is < 1 or > 255)
                return false;
            consumed = (end + 1) - index;
            bytes = kind == 'f'
                ? [1, 5, 3, 0x0E, (byte)code, 1]
                : [1, 6, 2, 0x0E, 0x0E, (byte)code, 1];
            return true;
        }
        return false;
    }

    private static byte[] BuildPlaintext(SaveState state, GameDatabase? db, string title,
        bool includePictures, bool includeVarDb)
    {
        using var ms = new MemoryStream(64 * 1024);
        using var bw = new BinaryWriter(ms, Encoding.UTF8, leaveOpen: true);

        // 20-byte header: LCG seeds at [0/3/9], UTF-8 flag 0x55 at [6], checksum patched at [2].
        var header = new byte[HeaderLen];
        header[0] = 0x3A;
        header[3] = 0x71;
        header[6] = Utf8Flag;
        header[9] = 0xC5;
        bw.Write(header);

        bw.Write(StartByte);
        WriteMemDataString(bw, string.IsNullOrEmpty(title) ? "LWSMF Save" : title, 2, alwaysNul: true);
        bw.Write(FileVersion);

        WritePart1(bw);
        WritePart2(bw, state);
        WritePart3(bw, state);
        WritePart4(bw);
        WritePart5(bw, includePictures ? state.Pictures : null);
        WriteVarDb(bw, includeVarDb ? state.DbOverlays : null, db);

        // SavePart7 gate byte (0 = empty) + trailing EndByte.
        bw.Write((byte)0);
        bw.Write(EndByte);
        bw.Flush();

        byte[] plain = ms.ToArray();
        byte checksum = 0;
        for (int i = HeaderLen; i < plain.Length; i++)
        {
            checksum = unchecked((byte)(checksum + plain[i]));
        }
        plain[2] = checksum;
        return plain;
    }

    private static void WritePart1(BinaryWriter bw)
    {
        bw.Write(1u);
        bw.Write(0u);
        bw.Write(0u);
        bw.Write(64u);
        bw.Write(36u);
        bw.Write(uint.MaxValue); // No map snapshot sentinel.
        bw.Write(0u); // Empty chip_image MemData<u32>.
        for (int i = 0; i < 31; i++) bw.Write(0u); // 31 empty auto_tiles.
        bw.Write(192u);
        for (int i = 0; i < 192; i++) bw.Write(0u); // 192-word switch table.
        bw.Write(0u); // 0 objects.
        for (int i = 0; i < Part1TailVars.Length; i++) bw.Write(Part1TailVars[i]);
        bw.Write(0u); // 0 vars3.
    }

    private static void WritePart2(BinaryWriter bw, SaveState state)
    {
        bw.Write((byte)0);
        bw.Write((byte)0);

        uint[] vars3_21 = new uint[19];
        vars3_21[0] = state.PlayFrames > 0 ? (uint)Math.Min(state.PlayFrames, uint.MaxValue) : 0u;
        int sys3 = state.SysVars != null && state.SysVars.Count > WolfIds.SysChoiceAnchorX
            ? state.SysVars[WolfIds.SysChoiceAnchorX] : 512;
        int sys4 = state.SysVars != null && state.SysVars.Count > WolfIds.SysChoiceAnchorY
            ? state.SysVars[WolfIds.SysChoiceAnchorY] : 240;
        vars3_21[9] = unchecked((uint)sys3);
        vars3_21[10] = unchecked((uint)sys4);
        for (int i = 0; i < 19; i++) bw.Write(vars3_21[i]);

        for (int i = 0; i < 3; i++) bw.Write((ushort)0); // vars22_24
        bw.Write(0u); // var25
        for (int i = 0; i < 3; i++) bw.Write((ushort)0); // vars26_28
        for (int i = 0; i < 9; i++) bw.Write(0u); // vars29_37
        bw.Write((byte)0); // var38
        bw.Write((ushort)0); // var39
        bw.Write(0u); bw.Write(0u); // vars40_41
        for (int i = 0; i < 4; i++) bw.Write((ushort)0); // vars42_45
        for (int i = 0; i < 3; i++) bw.Write(0u); // vars46_48
        bw.Write((byte)0); // var49
        bw.Write(0u); // var51 (var50 omitted at file_version > 96)
        bw.Write(0u); bw.Write(0u); // vars52_53

        // block100
        bw.Write(0u); bw.Write(0u); bw.Write((byte)0);
        bw.Write(0u); // vars1 count
        bw.Write(0u); // vars2 count
        for (int i = 0; i < 5; i++) bw.Write(0u); // vars59_63

        for (int i = 0; i < 6; i++) bw.Write(0u); // vars64_69
        bw.Write((ushort)0); // var70
        bw.Write((ushort)0); bw.Write((ushort)0); // md1, md2
        bw.Write(0u); // var71
        for (int i = 0; i < 3; i++) bw.Write(0u); // vars72_74

        // bgm (file_version >= 108): channel 0 BGM and channel 1 BGS.
        SavedAudio? bgm = state.Audio?.FirstOrDefault(a => a != null && a.Channel == 0 && !string.IsNullOrEmpty(a.File));
        SavedAudio? bgs = state.Audio?.FirstOrDefault(a => a != null && a.Channel == 1 && !string.IsNullOrEmpty(a.File));
        WriteAudioSlot(bw, bgm);
        WriteAudioSlot(bw, bgs);

        // block109
        bw.Write((ushort)0); bw.Write((ushort)0); bw.Write((byte)0);
        bw.Write((byte)0); // var82
        for (int i = 0; i < 3; i++) bw.Write((ushort)0); // mds7_9
        for (int i = 0; i < 5; i++) bw.Write(0u); // var83..87
        bw.Write(0u); bw.Write(0u); // vars88_89
        bw.Write(0u); bw.Write(0u); // var90, var91
        for (int i = 0; i < 4; i++) bw.Write(0u); // vars92_95
        bw.Write((byte)0); bw.Write((byte)0); // var96, var97
        // block137
        for (int i = 0; i < 4; i++) bw.Write(0u);
        for (int i = 0; i < 24; i++) bw.Write(0u);
        // block8a
        for (int i = 0; i < 4; i++) bw.Write(0u);
        bw.Write(0u); // md10
        bw.Write(0u); // mds2 count
    }

    private static void WriteAudioSlot(BinaryWriter bw, SavedAudio? audio)
    {
        if (audio == null || string.IsNullOrEmpty(audio.File))
        {
            bw.Write((ushort)0);
            bw.Write(0u); bw.Write(0u); bw.Write(0u);
            return;
        }
        WriteMemDataString(bw, audio.File, 2, alwaysNul: true);
        bw.Write((uint)Math.Clamp(audio.Volume, 0, 100));
        bw.Write((uint)Math.Clamp(audio.Tempo <= 0 ? 100 : audio.Tempo, 1, 1000));
        bw.Write((uint)Math.Max(0, audio.LoopPoint));
    }

    private static void WritePart3(BinaryWriter bw, SaveState state)
    {
        bw.Write(0u); // var1
        bw.Write(0u); // groupCount

        // Row 0: 800 normal variables.
        bw.Write(1u);
        bw.Write((uint)NormalVarCount);
        var vars = state.Vars;
        for (int i = 0; i < NormalVarCount; i++)
        {
            bw.Write(vars != null && i < vars.Count ? vars[i] : 0);
        }

        bw.Write(0u); // mds1Count

        // Index CommonSelf banks by event id.
        var byEvent = new Dictionary<int, EventSelfVars>();
        if (state.CommonSelf != null)
        {
            foreach (var bank in state.CommonSelf)
            {
                if (bank != null && bank.Id >= 0) byEvent[bank.Id] = bank;
            }
        }

        // var5: 550 groups x 100 CSelf numeric slots (indices 600..699).
        bw.Write((uint)EventGroups);
        for (int ev = 0; ev < EventGroups; ev++)
        {
            bw.Write((byte)SelfNumCount);
            EventSelfVars? bank = byEvent.GetValueOrDefault(ev);
            int[]? dense = bank != null && (bank.NumMap == null || bank.NumMap.Count == 0) ? bank.Nums : null;
            for (int k = 0; k < SelfNumCount; k++)
            {
                int slot = 600 + k;
                int val = 0;
                if (bank != null)
                {
                    if (bank.NumMap != null && bank.NumMap.TryGetValue(slot, out int mapped)) val = mapped;
                    else if (dense != null && slot < dense.Length) val = dense[slot];
                }
                bw.Write(val);
            }
        }

        // var6: 550 groups x 5 CSelf string slots (indices 605..609). Must never be 0.
        bw.Write((uint)EventGroups);
        for (int ev = 0; ev < EventGroups; ev++)
        {
            bw.Write((byte)SelfStrCount);
            EventSelfVars? bank = byEvent.GetValueOrDefault(ev);
            string[]? dense = bank != null && (bank.StrMap == null || bank.StrMap.Count == 0) ? bank.Strings : null;
            for (int k = 0; k < SelfStrCount; k++)
            {
                int slot = 605 + k;
                string str = "";
                if (bank != null)
                {
                    if (bank.StrMap != null && bank.StrMap.TryGetValue(slot, out string? mapped)) str = mapped ?? "";
                    else if (dense != null && slot < dense.Length) str = dense[slot] ?? "";
                }
                WriteMemDataString(bw, str, 4, alwaysNul: true);
            }
        }
    }

    private static void WritePart4(BinaryWriter bw)
    {
        WriteEmptyObject(bw);
        bw.Write(0u); // objects count
        bw.Write((byte)0); bw.Write((byte)0); // var2, var3
        bw.Write(0u); // vars2 count
        bw.Write(0u); // vars3 count
    }

    private static void WriteEmptyObject(BinaryWriter bw)
    {
        bw.Write(0u); bw.Write(0u); bw.Write(0u); bw.Write(0u); // var1..4
        bw.Write((byte)0); // var5
        bw.Write(0u); // var6
        bw.Write((ushort)0); // md1
        for (int i = 0; i < 4; i++) bw.Write((ushort)0); // var7..10
        bw.Write((byte)0); bw.Write((byte)0); // var11, var12
        for (int s = 0; s < 2; s++) // sp1, sp2
        {
            for (int i = 0; i < 6; i++) bw.Write((byte)0);
            bw.Write(0u);
        }
        for (int i = 0; i < 6; i++) bw.Write((ushort)0); // var13..18
        for (int i = 0; i < 3; i++) bw.Write((byte)0); // var19..21
        bw.Write(0u); // vars1 count
        bw.Write((byte)0); bw.Write((byte)0); // var23, var24
        for (int i = 0; i < 4; i++) bw.Write(0u); // var25..28
        bw.Write(0u); // var29
        for (int i = 0; i < 3; i++) bw.Write(0u); // var30..32
        bw.Write((ushort)0); // mds1 count
        bw.Write(0u); // var34
        bw.Write((byte)0); // var35
        bw.Write(0u); bw.Write(0u); // var36, var37
        bw.Write(0u); // var38
        bw.Write(0u); // var39
    }

    private static void WritePart5(BinaryWriter bw, List<SavedPicture>? pictures)
    {
        var filtered = new List<SavedPicture>();
        if (pictures != null)
        {
            foreach (var pic in pictures)
            {
                if (pic == null || SaveMigrator.IsTransientUiPicture(pic.Number)) continue;
                filtered.Add(pic);
                if (filtered.Count >= 0x7FFF) break;
            }
        }

        bw.Write((ushort)filtered.Count);
        foreach (var pic in filtered)
        {
            bw.Write(unchecked((uint)pic.Number));
            bw.Write((byte)0);
            bw.Write((byte)0);
            bw.Write((ushort)Math.Clamp(pic.Anchor, 0, ushort.MaxValue));
            byte isText = pic.Kind == 2 ? (byte)1 : (byte)0;
            byte isWin = pic.Kind == 4 ? (byte)1 : (byte)0;
            bw.Write(isText);
            bw.Write(isWin);

            byte[] md1 = isText == 1
                ? EncodePictureText(pic.Text ?? "")
                : AppendNul(Encoding.UTF8.GetBytes(pic.File ?? ""));
            WriteMemDataBytes(bw, md1, 2);

            int Arg(int idx, int fallback) => pic.Args != null && idx < pic.Args.Count ? pic.Args[idx] : fallback;
            int scale = Arg(7, 100);
            int vscale = pic.Args != null && pic.Args.Count > 10 && pic.Args[10] != 0 ? pic.Args[10] : scale;

            // Reverse mapping of LegacySaveImporter: vars7_18 = (x,y,w,h,pattern,opacity,blend,-1,fade,scale,vscale,0).
            bw.Write(Arg(5, 0));
            bw.Write(Arg(6, 0));
            bw.Write(Math.Max(0, Arg(1, 1)));
            bw.Write(Math.Max(0, Arg(2, 1)));
            bw.Write(Arg(9, 0));
            bw.Write(Arg(4, 255));
            bw.Write(Arg(3, 0));
            bw.Write(unchecked((int)uint.MaxValue));
            bw.Write(Arg(8, 0));
            bw.Write(scale);
            bw.Write(vscale);
            bw.Write(0);

            for (int k = 0; k < 6; k++) bw.Write(0u);  // vals1
            for (int k = 0; k < 18; k++) bw.Write(0u); // vars19_36
            bw.Write((byte)0); bw.Write((byte)0); bw.Write((byte)0); // var37..39
            for (int k = 0; k < 4; k++) bw.Write(0u);  // vals2
            for (int k = 0; k < 4; k++) bw.Write(0u);  // vals3
            for (int k = 0; k < 4; k++) bw.Write(0u);  // vars40_43
            for (int k = 0; k < 5; k++) bw.Write(0u);  // vars44_48
            for (int k = 0; k < 6; k++) bw.Write(0u);  // vars49_54
            for (int k = 0; k < 6; k++) bw.Write(0u);  // vars55_60
            for (int k = 0; k < 3; k++) bw.Write(0u);  // vars61_63
            for (int k = 0; k < 21; k++) bw.Write(0u); // vars64_84
            for (int k = 0; k < 5; k++) bw.Write(0u);  // vars85_89
            bw.Write(0u);                              // var90 (sub_tables count)
            for (int k = 0; k < 7; k++) bw.Write(0u);  // vars91_97
        }
    }

    private static void WriteVarDb(BinaryWriter bw, SavedDbOverlays? overlays, GameDatabase? db)
    {
        bw.Write((byte)0x6F);
        if (overlays == null)
        {
            bw.Write(0u);
            return;
        }

        var numsByType = new Dictionary<int, Dictionary<(int Row, int Col), int>>();
        var textsByType = new Dictionary<int, Dictionary<(int Row, int Col), string>>();
        var rowCountsByType = new Dictionary<int, int>();
        int maxType = -1;

        foreach (var cell in overlays.Nums)
        {
            if (cell.Container != (int)DbContainer.CData
                || cell.Type is < 0 or > 4096
                || cell.Row is < 0 or > 100_000
                || cell.Column is < 0 or > 4096)
            {
                continue;
            }
            if (!numsByType.TryGetValue(cell.Type, out var map))
            {
                map = new Dictionary<(int, int), int>();
                numsByType[cell.Type] = map;
            }
            map[(cell.Row, cell.Column)] = cell.Value;
            if (cell.Type > maxType) maxType = cell.Type;
        }
        foreach (var cell in overlays.Texts)
        {
            if (cell.Container != (int)DbContainer.CData
                || cell.Type is < 0 or > 4096
                || cell.Row is < 0 or > 100_000
                || cell.Column is < 0 or > 4096)
            {
                continue;
            }
            if (!textsByType.TryGetValue(cell.Type, out var map))
            {
                map = new Dictionary<(int, int), string>();
                textsByType[cell.Type] = map;
            }
            map[(cell.Row, cell.Column)] = cell.Value ?? "";
            if (cell.Type > maxType) maxType = cell.Type;
        }
        foreach (var rc in overlays.RowCounts)
        {
            if (rc.Container != (int)DbContainer.CData
                || rc.Type is < 0 or > 4096
                || rc.Count is < 0 or > 100_000)
            {
                continue;
            }
            rowCountsByType[rc.Type] = rc.Count;
            if (rc.Type > maxType) maxType = rc.Type;
        }

        if (maxType < 0)
        {
            bw.Write(0u);
            return;
        }

        int typeCount = maxType + 1;
        bw.Write((uint)typeCount);

        for (int t = 0; t < typeCount; t++)
        {
            numsByType.TryGetValue(t, out var tNums);
            textsByType.TryGetValue(t, out var tTexts);
            bool hasRowCount = rowCountsByType.TryGetValue(t, out int explicitRows);
            bool touched = (tNums is { Count: > 0 }) || (tTexts is { Count: > 0 }) || hasRowCount;
            if (!touched)
            {
                bw.Write(-2);
                bw.Write(0);
                bw.Write(0u);
                bw.Write(0u);
                continue;
            }

            bool[] schemaStrings = Array.Empty<bool>();
            int baseRows = 0;
            if (db != null)
            {
                db.TryGetCDataSchema(t, out schemaStrings, out baseRows);
            }

            int maxCol = schemaStrings.Length - 1;
            int maxRow = (hasRowCount ? explicitRows : baseRows) - 1;
            var textCols = new HashSet<int>();
            if (tNums != null)
            {
                foreach (var key in tNums.Keys)
                {
                    if (key.Col > maxCol) maxCol = key.Col;
                    if (key.Row > maxRow) maxRow = key.Row;
                }
            }
            if (tTexts != null)
            {
                foreach (var key in tTexts.Keys)
                {
                    if (key.Col > maxCol) maxCol = key.Col;
                    if (key.Row > maxRow) maxRow = key.Row;
                    textCols.Add(key.Col);
                }
            }

            int fieldCount = Math.Max(0, maxCol + 1);
            int rowCount = Math.Max(0, maxRow + 1);
            uint[] cfg = new uint[fieldCount];
            for (int k = 0; k < fieldCount; k++)
            {
                bool isStr = k < schemaStrings.Length ? schemaStrings[k] : textCols.Contains(k);
                cfg[k] = isStr ? (uint)(2000 + k) : (uint)k;
            }

            bw.Write(-2);
            bw.Write(0);
            bw.Write((uint)fieldCount);
            for (int k = 0; k < fieldCount; k++) bw.Write(cfg[k]);
            bw.Write((uint)rowCount);

            for (int row = 0; row < rowCount; row++)
            {
                // First pass: all numeric fields (< 2000).
                for (int k = 0; k < fieldCount; k++)
                {
                    if (cfg[k] >= 2000) continue;
                    int val;
                    if (tNums != null && tNums.TryGetValue((row, k), out int overNum))
                    {
                        val = overNum;
                    }
                    else if (db != null)
                    {
                        var baseVal = db.ReadWithOverlay(new DbOp
                        {
                            Container = DbContainer.CData,
                            Direction = DbDirection.Read,
                            DbType = t,
                            DataId = row,
                            ItemId = k
                        }, null);
                        val = baseVal?.Num ?? 0;
                    }
                    else
                    {
                        val = 0;
                    }
                    bw.Write(val);
                }
                // Second pass: all textual fields (>= 2000).
                for (int k = 0; k < fieldCount; k++)
                {
                    if (cfg[k] < 2000) continue;
                    string str;
                    if (tTexts != null && tTexts.TryGetValue((row, k), out string? overStr))
                    {
                        str = overStr ?? "";
                    }
                    else if (db != null)
                    {
                        var baseVal = db.ReadWithOverlay(new DbOp
                        {
                            Container = DbContainer.CData,
                            Direction = DbDirection.Read,
                            DbType = t,
                            DataId = row,
                            ItemId = k,
                            Destination = Magic.StrBase
                        }, null);
                        str = baseVal?.Str ?? "";
                    }
                    else
                    {
                        str = "";
                    }
                    WriteMemDataString(bw, str, 4, alwaysNul: true);
                }
            }
        }
    }

    private static byte[] AppendNul(byte[] utf8)
    {
        byte[] result = new byte[utf8.Length + 1];
        Buffer.BlockCopy(utf8, 0, result, 0, utf8.Length);
        return result;
    }

    private static void WriteMemDataString(BinaryWriter bw, string value, int sizeWidth, bool alwaysNul)
    {
        byte[] utf8 = string.IsNullOrEmpty(value) ? Array.Empty<byte>() : Encoding.UTF8.GetBytes(value);
        byte[] payload = alwaysNul ? AppendNul(utf8) : utf8;
        WriteMemDataBytes(bw, payload, sizeWidth);
    }

    private static void WriteMemDataBytes(BinaryWriter bw, byte[] payload, int sizeWidth)
    {
        switch (sizeWidth)
        {
            case 2:
                if (payload.Length > ushort.MaxValue)
                    throw new InvalidDataException($"MemData<u16> payload too large: {payload.Length}");
                bw.Write((ushort)payload.Length);
                break;
            case 4:
                bw.Write((uint)payload.Length);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(sizeWidth));
        }
        if (payload.Length > 0) bw.Write(payload);
    }
}
