using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Lwsmf.Core;

public sealed class LegacySaveImporter
{
    private const int CryptStart = 0x14;
    private const byte StartByte = 0x19;
    private const byte EndByte = 0x19;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private static readonly Encoding ShiftJisEncoding = InitShiftJis();

    private static Encoding InitShiftJis()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        return Encoding.GetEncoding(932);
    }

    public static readonly string[] NotRecoverableFields =
    [
        "event_instances",
        "pc",
        "call_stack",
        "wait",
        "audio_phase"
    ];

    public sealed class SaveReader(byte[] buffer, int position = 0, List<string>? warnings = null, bool isUtf8 = true)
    {
        public byte[] Buffer { get; } = buffer;
        public int Position { get; set; } = position;
        public List<string> Warnings { get; } = warnings ?? [];
        public bool IsUtf8 { get; } = isUtf8;

        public int Remaining => Buffer.Length - Position;

        public byte ReadU8()
        {
            if (Position >= Buffer.Length)
                throw new EndOfStreamException($"ReadU8 past EOF at {Position}");
            return Buffer[Position++];
        }

        public ushort ReadU16()
        {
            if (Position + 2 > Buffer.Length)
                throw new EndOfStreamException($"ReadU16 past EOF at {Position}");
            ushort v = (ushort)(Buffer[Position] | (Buffer[Position + 1] << 8));
            Position += 2;
            return v;
        }

        public uint ReadU32()
        {
            if (Position + 4 > Buffer.Length)
                throw new EndOfStreamException($"ReadU32 past EOF at {Position}");
            uint v = (uint)(Buffer[Position] |
                           (Buffer[Position + 1] << 8) |
                           (Buffer[Position + 2] << 16) |
                           (Buffer[Position + 3] << 24));
            Position += 4;
            return v;
        }

        public int ReadI32() => (int)ReadU32();

        public ulong ReadU64()
        {
            ulong lo = ReadU32();
            ulong hi = ReadU32();
            return lo | (hi << 32);
        }

        public byte[] ReadBytes(int count)
        {
            if (count < 0 || count > Buffer.Length - Position)
                throw new InvalidDataException($"ReadBytes({count}) out of bounds at {Position} (remaining {Buffer.Length - Position})");
            byte[] dest = new byte[count];
            Array.Copy(Buffer, Position, dest, 0, count);
            Position += count;
            return dest;
        }

        public byte[] ReadMemData(int size)
        {
            int len = size switch
            {
                1 => ReadU8(),
                2 => ReadU16(),
                4 => (int)ReadU32(),
                _ => throw new ArgumentException($"Invalid size {size}")
            };
            return ReadBytes(len);
        }

        public string ReadMemDataString(int size, string field = "") =>
            ReadMemDataString(size, field, out _);

        public string ReadMemDataString(int size, string field, out byte[] raw)
        {
            int at = Position;
            raw = ReadMemData(size);
            if (raw.Length == 0) return "";
            int len = raw.Length;
            if (raw[^1] == 0) len--;
            if (len <= 0) return "";
            if (!IsUtf8)
            {
                return ShiftJisEncoding.GetString(raw, 0, len);
            }
            try
            {
                return StrictUtf8.GetString(raw, 0, len);
            }
            catch (DecoderFallbackException)
            {
                Warnings.Add($"{field} at 0x{at:X4}: invalid UTF-8, replacement applied");
                return Encoding.UTF8.GetString(raw, 0, len);
            }
        }
    }

    public sealed class PictureSummary
    {
        [JsonPropertyName("id")] public int Id { get; set; }
        [JsonPropertyName("is_text")] public bool IsText { get; set; }
        [JsonPropertyName("is_window")] public bool IsWindow { get; set; }
        [JsonPropertyName("name")] public string Name { get; set; } = "";
    }

    public sealed class PlayTimeSummary
    {
        [JsonPropertyName("frames")] public uint Frames { get; set; }
        [JsonPropertyName("seconds")] public uint Seconds { get; set; }
        [JsonPropertyName("formatted")] public string Formatted { get; set; } = "";
    }

    public sealed class MapSummary
    {
        [JsonPropertyName("width")] public uint Width { get; set; }
        [JsonPropertyName("height")] public uint Height { get; set; }
        [JsonPropertyName("chip_image")] public string ChipImage { get; set; } = "";
        [JsonPropertyName("objects_count")] public int ObjectsCount { get; set; }
    }

    public sealed class RecoveredData
    {
        [JsonPropertyName("variables")] public Dictionary<string, int> Variables { get; set; } = [];
        [JsonPropertyName("switches")] public Dictionary<string, bool> Switches { get; set; } = [];
        [JsonPropertyName("pictures")] public List<PictureSummary> Pictures { get; set; } = [];
        [JsonPropertyName("play_time")] public PlayTimeSummary PlayTime { get; set; } = new();
        [JsonPropertyName("map")] public MapSummary Map { get; set; } = new();
        [JsonPropertyName("choice_anchor")] public int[] ChoiceAnchor { get; set; } = [512, 240];
        [JsonPropertyName("database_overrides")] public List<int> DatabaseOverrides { get; set; } = [];
        [JsonIgnore] public int[] SysVars { get; set; } = Array.Empty<int>();
        [JsonIgnore] public List<string> StringVars { get; set; } = [];
        [JsonIgnore] public Dictionary<int, Dictionary<int, int[]>> MapEventSelfVars { get; set; } = [];
        [JsonIgnore] public Dictionary<int, int> DatabaseRowCounts { get; set; } = [];
        [JsonIgnore] public List<DatabaseCell> DatabaseCells { get; set; } = [];
        [JsonIgnore] public List<DatabaseTextCell> DatabaseTextCells { get; set; } = [];
        /// <summary>Event id -> CSelf0..99 numbers (SavePart3 list table).</summary>
        [JsonIgnore] public Dictionary<int, int[]> CommonSelfNumbers { get; set; } = [];
        /// <summary>Event id -> CSelf5..9 strings (SavePart3 slot table).</summary>
        [JsonIgnore] public Dictionary<int, string[]> CommonSelfStrings { get; set; } = [];
        /// <summary>Complete SavePart5 picture records for a real load (not the summary view).</summary>
        [JsonIgnore] public List<SavedPicture> PictureStates { get; set; } = [];
        [JsonIgnore] public string BgmFile { get; set; } = "";
        [JsonIgnore] public int BgmVolume { get; set; } = 100;
        [JsonIgnore] public int BgmTempo { get; set; } = 100;
        [JsonIgnore] public int BgmLoopPoint { get; set; } = 0;
        [JsonIgnore] public string BgsFile { get; set; } = "";
        [JsonIgnore] public int BgsVolume { get; set; } = 100;
        [JsonIgnore] public int BgsTempo { get; set; } = 100;
        [JsonIgnore] public int BgsLoopPoint { get; set; } = 0;
    }

    public sealed class DatabaseCell
    {
        public int Type { get; set; }
        public int Row { get; set; }
        public int Column { get; set; }
        public int Value { get; set; }
    }

    public sealed class DatabaseTextCell
    {
        public int Type { get; set; }
        public int Row { get; set; }
        public int Column { get; set; }
        public string Value { get; set; } = "";
    }

    public sealed class SlotResult
    {
        [JsonPropertyName("index")] public int Index { get; set; }
        [JsonPropertyName("file")] public string File { get; set; } = "";
        [JsonPropertyName("bytes")] public long Bytes { get; set; }
        [JsonPropertyName("checksum_ok")] public bool ChecksumOk { get; set; }
        [JsonPropertyName("encoding_warnings")] public List<string> EncodingWarnings { get; set; } = [];
        [JsonPropertyName("recovered")] public RecoveredData Recovered { get; set; } = new();
        [JsonPropertyName("not_recoverable")] public string[] NotRecoverable { get; set; } = NotRecoverableFields;
    }

    public sealed class ImportReport
    {
        [JsonPropertyName("schema")] public string Schema { get; set; } = "wolf.save_import";
        [JsonPropertyName("schema_version")] public int SchemaVersion { get; set; } = 1;
        [JsonPropertyName("slots")] public List<SlotResult> Slots { get; set; } = [];
        [JsonPropertyName("errors")] public List<string> Errors { get; set; } = [];
    }

    public static byte[] Decrypt(byte[] encrypted)
    {
        if (encrypted.Length <= CryptStart)
            throw new InvalidDataException($"Save file too small: {encrypted.Length} bytes");

        byte[] buf = (byte[])encrypted.Clone();
        byte[] seeds = [buf[0], buf[3], buf[9]];
        int[] steps = [1, 2, 5];

        for (int pass = 0; pass < 3; pass++)
        {
            uint state = seeds[pass];
            int step = steps[pass];
            int at = CryptStart;
            while (at < buf.Length)
            {
                state = unchecked(state * 214013u + 2531011u);
                uint rand = (state >> 16) & 0x7FFFu;
                buf[at] ^= (byte)((rand >> 12) & 0xFFu);
                at += step;
            }
        }
        return buf;
    }

    public static SlotResult ImportFile(string path, int slotIndex, bool rejectBadChecksum = false)
    {
        var info = new FileInfo(path);
        if (info.Exists && info.Length > SaveCodec.MaxSaveBytes)
        {
            throw new InvalidDataException($"Save file too large: {info.Length} bytes (max {SaveCodec.MaxSaveBytes})");
        }
        byte[] raw = File.ReadAllBytes(path);
        if (raw.Length > SaveCodec.MaxSaveBytes)
        {
            throw new InvalidDataException($"Save file too large: {raw.Length} bytes (max {SaveCodec.MaxSaveBytes})");
        }
        byte[] dec = Decrypt(raw);

        if (dec[CryptStart] != StartByte)
        {
            throw new InvalidDataException($"Expected start byte 0x19 at 0x{CryptStart:X}, got 0x{dec[CryptStart]:X2}");
        }

        byte storedChecksum = raw[2];
        byte computedChecksum = 0;
        for (int i = CryptStart; i < dec.Length; i++)
        {
            computedChecksum = unchecked((byte)(computedChecksum + dec[i]));
        }
        bool checksumOk = (storedChecksum == computedChecksum);
        if (rejectBadChecksum && !checksumOk)
        {
            throw new InvalidDataException($"Checksum mismatch: stored 0x{storedChecksum:X2}, computed 0x{computedChecksum:X2}");
        }

        var warnings = new List<string>();
        bool isUtf8 = raw[6] == 0x55;
        var r = new SaveReader(dec, CryptStart + 1, warnings, isUtf8);
        string saveName = r.ReadMemDataString(2, "save_name");
        ushort fileVersion = r.ReadU16();

        // SavePart1
        r.ReadU32(); r.ReadU32(); r.ReadU32();
        uint mapWidth = r.ReadU32();
        uint mapHeight = r.ReadU32();
        uint sentinel = r.ReadU32();
        if (sentinel != uint.MaxValue)
        {
            r.Position -= 4;
            ulong cells;
            try
            {
                cells = checked((ulong)mapWidth * (ulong)mapHeight * 3UL);
            }
            catch (OverflowException ex)
            {
                throw new InvalidDataException("Map dimensions overflow cell count", ex);
            }
            if (cells > (ulong)r.Remaining / 4UL)
                throw new InvalidDataException($"Map cell count {cells} exceeds remaining {r.Remaining} bytes");
            for (ulong i = 0; i < cells; i++) r.ReadU32();
        }
        string chipImage = r.ReadMemDataString(4, "chip_image");
        for (int i = 0; i < 31; i++) r.ReadMemData(4);
        int vars2Count = SignedCount(r.ReadU32());
        var switches = new Dictionary<string, bool>();
        for (int i = 0; i < vars2Count; i++)
        {
            uint word = r.ReadU32();
            if (word != 0)
            {
                switches[i.ToString()] = true;
            }
        }

        int objectCount = SignedCount(r.ReadU32());
        for (int i = 0; i < objectCount; i++)
        {
            SkipObject(r);
        }
        for (int i = 0; i < 11; i++) r.ReadU32();
        int vars3Count = SignedCount(r.ReadU32());
        for (int i = 0; i < vars3Count; i++) r.ReadU32();

        // SavePart2
        r.ReadU8(); r.ReadU8();
        uint[] vars3_21 = new uint[19];
        for (int i = 0; i < 19; i++) vars3_21[i] = r.ReadU32();
        uint playTimeFrames = vars3_21[0];
        var sysVars = new int[13];
        for (int i = 0; i < 13; i++) sysVars[i] = unchecked((int)vars3_21[6 + i]);
        int sys3 = sysVars[WolfIds.SysChoiceAnchorX];
        int sys4 = sysVars[WolfIds.SysChoiceAnchorY];

        for (int i = 0; i < 3; i++) r.ReadU16();
        r.ReadU32();
        for (int i = 0; i < 3; i++) r.ReadU16();
        for (int i = 0; i < 9; i++) r.ReadU32();
        r.ReadU8(); r.ReadU16();
        r.ReadU32(); r.ReadU32();
        for (int i = 0; i < 4; i++) r.ReadU16();
        for (int i = 0; i < 3; i++) r.ReadU32();
        r.ReadU8(); r.ReadU32(); r.ReadU32(); r.ReadU32();

        // block 100
        r.ReadU32(); r.ReadU32(); r.ReadU8();
        int c1 = SignedCount(r.ReadU32()); for (int i = 0; i < c1; i++) r.ReadMemData(4);
        int c2 = SignedCount(r.ReadU32()); for (int i = 0; i < c2; i++) r.ReadU32();
        for (int i = 0; i < 5; i++) r.ReadU32();
        for (int i = 0; i < 6; i++) r.ReadU32();
        r.ReadU16();
        r.ReadMemData(2); r.ReadMemData(2);
        r.ReadU32();
        for (int i = 0; i < 3; i++) r.ReadU32();
        string bgmPath1 = r.ReadMemDataString(2, "bgm_path1");
        int bgmVol = Math.Clamp(r.ReadI32(), 0, 100);
        int bgmTempo = r.ReadI32();
        int bgmLoop = Math.Max(0, r.ReadI32());
        if (bgmTempo <= 0) bgmTempo = 100;
        if (bgmVol == 0 && !string.IsNullOrEmpty(bgmPath1)) bgmVol = 100;
        string bgmPath2 = r.ReadMemDataString(2, "bgm_path2");
        int bgsVol = Math.Clamp(r.ReadI32(), 0, 100);
        int bgsTempo = r.ReadI32();
        int bgsLoop = Math.Max(0, r.ReadI32());
        if (bgsTempo <= 0) bgsTempo = 100;
        if (bgsVol == 0 && !string.IsNullOrEmpty(bgmPath2)) bgsVol = 100;
        r.ReadMemData(2); r.ReadMemData(2); r.ReadU8();
        r.ReadU8();
        for (int i = 0; i < 3; i++) r.ReadMemData(2);
        for (int i = 0; i < 5; i++) r.ReadU32();
        r.ReadU32(); r.ReadU32();
        r.ReadU32(); r.ReadU32();
        for (int i = 0; i < 4; i++) r.ReadU32();
        r.ReadU8(); r.ReadU8();
        for (int i = 0; i < 4; i++) r.ReadU32();
        for (int i = 0; i < 24; i++) r.ReadU32();
        for (int i = 0; i < 4; i++) r.ReadU32();
        r.ReadMemData(4);
        int mds2Count = SignedCount(r.ReadU32());
        for (int i = 0; i < mds2Count; i++) r.ReadMemData(4);

        // SavePart3
        r.ReadU32();
        int groupCount = SignedCount(r.ReadU32());
        var mapEventSelfVars = new Dictionary<int, Dictionary<int, int[]>>();
        for (int i = 0; i < groupCount; i++)
        {
            int gc = SignedCount(r.ReadU32());
            Dictionary<int, int[]>? byEvent = null;
            for (int j = 0; j < gc; j++)
            {
                byte per = r.ReadU8();
                int[]? words = null;
                bool any = false;
                for (int k = 0; k < per; k++)
                {
                    int v = r.ReadI32();
                    if (v != 0)
                    {
                        words ??= new int[per];
                        words[k] = v;
                        any = true;
                    }
                }
                if (any && words != null)
                {
                    byEvent ??= new Dictionary<int, int[]>();
                    byEvent[j] = words;
                }
            }
            if (byEvent != null)
            {
                mapEventSelfVars[i] = byEvent;
            }
        }
        int rowCount = SignedCount(r.ReadU32());
        var normalVars = new Dictionary<string, int>();
        for (int i = 0; i < rowCount; i++)
        {
            int rc = SignedCount(r.ReadU32());
            for (int j = 0; j < rc; j++)
            {
                int val = r.ReadI32();
                if (i == 0)
                {
                    normalVars[j.ToString()] = val;
                }
            }
        }
        int mds1Count = SignedCount(r.ReadU32());
        var stringVars = new List<string>(mds1Count);
        for (int i = 0; i < mds1Count; i++)
        {
            stringVars.Add(r.ReadMemDataString(4, $"str_var[{i}]"));
        }
        int listCount = SignedCount(r.ReadU32());
        var commonSelfNumbers = new Dictionary<int, int[]>();
        for (int i = 0; i < listCount; i++)
        {
            byte lc = r.ReadU8();
            var words = new int[lc];
            for (int j = 0; j < lc; j++) words[j] = r.ReadI32();
            commonSelfNumbers[i] = words;
        }
        int slotCount = SignedCount(r.ReadU32());
        var commonSelfStrings = new Dictionary<int, string[]>();
        for (int i = 0; i < slotCount; i++)
        {
            byte sc = r.ReadU8();
            var strs = new string[sc];
            for (int j = 0; j < sc; j++) strs[j] = r.ReadMemDataString(4, "common_self");
            commonSelfStrings[i] = strs;
        }

        // SavePart4
        SkipObject(r);
        int p4Count = SignedCount(r.ReadU32());
        for (int i = 0; i < p4Count; i++) SkipObject(r);
        r.ReadU8(); r.ReadU8();
        int p4Vars2 = SignedCount(r.ReadU32());
        for (int i = 0; i < p4Vars2; i++) r.ReadU32();
        int p4Vars3 = SignedCount(r.ReadU32());
        for (int i = 0; i < p4Vars3; i++) r.ReadU64();

        // SavePart5 (Picture table)
        ushort p5Var1 = r.ReadU16();
        var pictures = new List<PictureSummary>();
        var pictureStates = new List<SavedPicture>();
        if ((p5Var1 & 0x8000) == 0)
        {
            for (int i = 0; i < p5Var1; i++)
            {
                uint picId = r.ReadU32();
                r.ReadU8(); r.ReadU8();
                ushort anchor = r.ReadU16();
                byte isText = r.ReadU8();
                byte isWin = r.ReadU8();
                string picName = r.ReadMemDataString(2, "picture_name", out byte[] picBytes);
                if (isText != 1 && !IsSafeAssetPath(picName))
                {
                    throw new InvalidDataException($"Unsafe picture asset path: '{picName}'");
                }
                var vars7_18 = new int[12];
                for (int k = 0; k < 12; k++) vars7_18[k] = r.ReadI32();
                // SavePart5_1.vals1[0..2]: RGB tone delta in [-255..+255] around neutral 100.
                int toneDeltaR = r.ReadI32();
                int toneDeltaG = r.ReadI32();
                int toneDeltaB = r.ReadI32();
                for (int k = 3; k < 6; k++) r.ReadU32();
                for (int k = 0; k < 18; k++) r.ReadU32();
                r.ReadU8(); r.ReadU8(); r.ReadU8();
                for (int k = 0; k < 4; k++) r.ReadU32();
                for (int k = 0; k < 4; k++) r.ReadU32();
                for (int k = 0; k < 4; k++) r.ReadU32();
                for (int k = 0; k < 5; k++) r.ReadU32();
                for (int k = 0; k < 6; k++) r.ReadU32();
                for (int k = 0; k < 6; k++) r.ReadU32();
                for (int k = 0; k < 3; k++) r.ReadU32();
                for (int k = 0; k < 21; k++) r.ReadU32();
                for (int k = 0; k < 5; k++) r.ReadU32();
                int subCount = SignedCount(r.ReadU32());
                for (int subIdx = 0; subIdx < subCount; subIdx++)
                {
                    int slen = SignedCount(r.ReadU32());
                    for (int w = 0; w < slen; w++) r.ReadU32();
                }
                for (int k = 0; k < 7; k++) r.ReadU32();

                pictures.Add(new PictureSummary
                {
                    Id = (int)picId,
                    IsText = isText == 1,
                    IsWindow = isWin == 1,
                    Name = picName
                });

                // PictureStage layout: args[1]=w/cols, args[2]=h/rows, args[3]=pattern(1-based),
                // args[4]=opacity, args[5]=x, args[6]=y, args[7]=scale, args[8]=rotation(0 on load),
                // args[10]=PIC_FLAG_TONE_MASK (0x02000000 when tone is non-neutral),
                // args[13..15]=RGB tone (0..200, 100 neutral), args[16]=vscale (only when scale_mode=3).
                // SavePart5 vars7_18 order: (x,y,w,h,pattern,opacity,blend,-1,fade,scale,vscale,0).
                int scaleX = vars7_18[9];
                int scaleY = vars7_18[10];
                int blend = vars7_18[6];
                bool independentScale = scaleY != 0 && scaleY != scaleX;
                int toneR = Math.Clamp((int)Math.Round(100.0 + toneDeltaR * 100.0 / 255.0, MidpointRounding.AwayFromZero), 0, 200);
                int toneG = Math.Clamp((int)Math.Round(100.0 + toneDeltaG * 100.0 / 255.0, MidpointRounding.AwayFromZero), 0, 200);
                int toneB = Math.Clamp((int)Math.Round(100.0 + toneDeltaB * 100.0 / 255.0, MidpointRounding.AwayFromZero), 0, 200);
                bool hasTone = toneDeltaR != 0 || toneDeltaG != 0 || toneDeltaB != 0;
                int toneFlags = hasTone ? 0x0200_0000 : 0;
                List<int> picArgs = (independentScale || hasTone)
                    ? [0, vars7_18[2], vars7_18[3], vars7_18[4], vars7_18[5], vars7_18[0], vars7_18[1], scaleX, 0, 0, toneFlags, 0, 0, toneR, toneG, toneB, independentScale ? scaleY : scaleX]
                    : [0, vars7_18[2], vars7_18[3], vars7_18[4], vars7_18[5], vars7_18[0], vars7_18[1], scaleX, 0];
                Dictionary<string, int>? picFlags = null;
                if (blend != 0)
                {
                    picFlags ??= new Dictionary<string, int>();
                    picFlags["blend"] = blend;
                }
                if (independentScale)
                {
                    picFlags ??= new Dictionary<string, int>();
                    picFlags["scale_mode"] = 3;
                }
                pictureStates.Add(new SavedPicture
                {
                    Number = (int)picId,
                    Kind = isText == 1 ? 2 : isWin == 1 ? 4 : 0,
                    Action = 0,
                    File = isText == 1 ? "" : picName,
                    Text = isText == 1 ? DecodePictureText(picBytes, warnings, $"picture[{picId}] text", isUtf8) : "",
                    Anchor = anchor,
                    Args = picArgs,
                    Flags = picFlags
                });
            }
        }

        // VariableDatabase
        r.ReadU8();
        uint typeCount = r.ReadU32();
        if ((ulong)typeCount * 8UL > (ulong)r.Remaining)
            throw new InvalidDataException($"VariableDatabase type count {typeCount} exceeds remaining {r.Remaining} bytes");
        var dbOverrides = new List<int>();
        var dbRowCounts = new Dictionary<int, int>();
        var dbCells = new List<DatabaseCell>();
        var dbTextCells = new List<DatabaseTextCell>();
        for (uint t = 0; t < typeCount; t++)
        {
            int unk = r.ReadI32();
            int cnt = unk;
            if (unk <= -1)
            {
                if (unk <= -2) r.ReadI32();
                uint fc = r.ReadU32();
                if (fc > int.MaxValue)
                    throw new InvalidDataException($"VariableDatabase field count {fc} exceeds int.MaxValue");
                cnt = (int)fc;
            }
            if (cnt > 0 && ((long)cnt * 4 > r.Remaining || cnt > 1_000_000))
                throw new InvalidDataException($"VariableDatabase field count {cnt} exceeds bounds (remaining {r.Remaining})");
            uint[] cfg = new uint[cnt > 0 ? cnt : 0];
            for (int k = 0; k < cfg.Length; k++) cfg[k] = r.ReadU32();

            uint typeRowCount = r.ReadU32();
            if (typeRowCount <= int.MaxValue)
            {
                dbRowCounts[(int)t] = (int)typeRowCount;
            }
            if (cfg.Length == 0)
            {
                if (typeRowCount > 100_000)
                    throw new InvalidDataException($"VariableDatabase type {t} has {typeRowCount} rows with zero fields");
                if (typeRowCount > 0) dbOverrides.Add((int)t);
                continue;
            }
            if ((ulong)typeRowCount > (ulong)r.Remaining / ((ulong)cfg.Length * 4UL))
                throw new InvalidDataException($"VariableDatabase type {t} row count {typeRowCount} exceeds remaining {r.Remaining} bytes");
            if (typeRowCount > 0)
            {
                dbOverrides.Add((int)t);
            }
            for (uint row = 0; row < typeRowCount; row++)
            {
                for (int k = 0; k < cfg.Length; k++)
                {
                    if (cfg[k] < 2000)
                        dbCells.Add(new DatabaseCell { Type = (int)t, Row = (int)row, Column = k, Value = r.ReadI32() });
                }
                for (int k = 0; k < cfg.Length; k++)
                {
                    if (cfg[k] >= 2000)
                    {
                        string textVal = r.ReadMemDataString(4, $"db[{t}:{row}:{k}]");
                        dbTextCells.Add(new DatabaseTextCell { Type = (int)t, Row = (int)row, Column = k, Value = textVal });
                    }
                }
            }
        }

        // SavePart7
        byte p7Var1 = r.ReadU8();
        if (p7Var1 == 1)
        {
            int p7Count = SignedCount(r.ReadU32());
            for (int i = 0; i < p7Count; i++)
            {
                byte tag = r.ReadU8();
                if (tag < 0xFA) r.ReadU8();
                else r.ReadU32();
            }
        }

        byte finalEndByte = r.ReadU8();
        if (finalEndByte != EndByte)
        {
            throw new InvalidDataException($"Expected final byte 0x19, got 0x{finalEndByte:X2}");
        }

        uint secs = playTimeFrames / 60;
        uint s = secs % 60;
        uint m = (secs / 60) % 60;
        uint h = secs / 3600;

        return new SlotResult
        {
            Index = slotIndex,
            File = Path.GetFileName(path),
            Bytes = raw.Length,
            ChecksumOk = checksumOk,
            EncodingWarnings = warnings,
            Recovered = new RecoveredData
            {
                Variables = normalVars,
                Switches = switches,
                Pictures = pictures,
                PlayTime = new PlayTimeSummary
                {
                    Frames = playTimeFrames,
                    Seconds = secs,
                    Formatted = $"{h:D2}:{m:D2}:{s:D2}"
                },
                Map = new MapSummary
                {
                    Width = mapWidth,
                    Height = mapHeight,
                    ChipImage = chipImage,
                    ObjectsCount = objectCount
                },
                ChoiceAnchor = [sys3, sys4],
                SysVars = sysVars,
                StringVars = stringVars,
                MapEventSelfVars = mapEventSelfVars,
                DatabaseOverrides = dbOverrides,
                DatabaseRowCounts = dbRowCounts,
                DatabaseCells = dbCells,
                DatabaseTextCells = dbTextCells,
                CommonSelfNumbers = commonSelfNumbers,
                CommonSelfStrings = commonSelfStrings,
                PictureStates = pictureStates,
                BgmFile = bgmPath1,
                BgmVolume = bgmVol,
                BgmTempo = bgmTempo,
                BgmLoopPoint = bgmLoop,
                BgsFile = bgmPath2,
                BgsVolume = bgsVol,
                BgsTempo = bgsTempo,
                BgsLoopPoint = bgsLoop
            },
            NotRecoverable = NotRecoverableFields
        };
    }

    private static int SignedCount(uint raw) => (int)raw > 0 ? (int)raw : 0;

    /// <summary>Validate a retail .sav file without altering live session state.</summary>
    public static bool ValidateFile(string path, out string error)
    {
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
        {
            error = "retail save file not found";
            return false;
        }
        try
        {
            _ = ImportFile(path, 0, rejectBadChecksum: true);
            error = "";
            return true;
        }
        catch (Exception ex) when (ex is InvalidDataException or EndOfStreamException or IOException or UnauthorizedAccessException)
        {
            error = ex.Message;
            return false;
        }
    }

    internal static string DecodePictureText(byte[] bytes, List<string>? warnings = null,
        string field = "picture_text", bool isUtf8 = true)
    {
        var sb = new StringBuilder();
        int i = 0;
        bool warned = false;
        while (i < bytes.Length && bytes[i] != 0)
        {
            if (bytes[i] != 1)
            {
                int len;
                if (!isUtf8)
                {
                    byte b = bytes[i];
                    bool sjisLead = (b is >= 0x81 and <= 0x9F) || (b is >= 0xE0 and <= 0xFC);
                    len = sjisLead && i + 1 < bytes.Length && bytes[i + 1] != 0 ? 2 : 1;
                    sb.Append(ShiftJisEncoding.GetString(bytes, i, len));
                    i += len;
                    continue;
                }
                len = CharLen(bytes[i]);
                if (i + len > bytes.Length) len = bytes.Length - i;
                for (int k = 1; k < len; k++)
                {
                    if ((bytes[i + k] & 0xC0) != 0x80)
                    {
                        len = k;
                        break;
                    }
                }
                if (AppendDecodedText(sb, bytes, i, len, !warned ? warnings : null, field))
                {
                    warned = true;
                }
                i += len;
                continue;
            }
            if (i + 2 > bytes.Length) break;
            int total = bytes[i + 1];
            if (total < 2 || i + 1 + total > bytes.Length)
            {
                i += 2;
                continue;
            }
            int payload = i + 2;
            switch (total)
            {
                case 2 when bytes[payload] == 5:
                    sb.Append("\\E");
                    break;
                case 4:
                {
                    int size = bytes[payload] - 10;
                    if (size is >= 1 and <= 96) sb.Append("\\f[").Append(size).Append(']');
                    break;
                }
                case 5:
                {
                    int size = bytes[payload + 2] - 14;
                    if (size is >= 0 and <= 96) sb.Append("\\f[").Append(size).Append(']');
                    break;
                }
                case 6 when bytes[payload] == 2:
                {
                    int index = bytes[payload + 3] - 14;
                    if (index is >= 0 and <= 30) sb.Append("\\c[").Append(index).Append(']');
                    break;
                }
            }
            i += 1 + total;
        }
        return sb.ToString();
    }

    private static bool AppendDecodedText(StringBuilder sb, byte[] bytes, int offset, int length,
        List<string>? warnings, string field)
    {
        try
        {
            sb.Append(StrictUtf8.GetString(bytes, offset, length));
            return false;
        }
        catch (DecoderFallbackException)
        {
            warnings?.Add($"{field} at 0x{offset:X4}: invalid UTF-8, replacement applied");
            sb.Append(Encoding.UTF8.GetString(bytes, offset, length));
            return true;
        }
    }

    private static int CharLen(byte first) => first switch
    {
        <= 0x7f => 1,
        >= 0xc2 and <= 0xdf => 2,
        >= 0xe0 and <= 0xef => 3,
        >= 0xf0 and <= 0xf4 => 4,
        _ => 1
    };

    private static bool IsSafeAssetPath(string path)
    {
        if (string.IsNullOrEmpty(path)) return true;
        if (Path.IsPathRooted(path) || path[0] == '/' || path[0] == '\\') return false;
        if (path.Contains(':')) return false;
        foreach (string segment in path.Split('/', '\\'))
        {
            if (segment == "..") return false;
        }
        return true;
    }

    private static void SkipObject(SaveReader r)
    {
        r.ReadU32(); r.ReadU32(); r.ReadU32(); r.ReadU32(); r.ReadU8(); r.ReadU32();
        r.ReadMemData(2);
        for (int i = 0; i < 4; i++) r.ReadU16();
        r.ReadU8(); r.ReadU8();
        for (int s = 0; s < 2; s++)
        {
            for (int i = 0; i < 6; i++) r.ReadU8();
            uint c = r.ReadU32();
            for (int i = 0; i < c; i++)
            {
                r.ReadU8();
                byte n1 = r.ReadU8(); for (int j = 0; j < n1; j++) r.ReadU32();
                byte n2 = r.ReadU8(); for (int j = 0; j < n2; j++) r.ReadU8();
            }
        }
        for (int i = 0; i < 6; i++) r.ReadU16();
        for (int i = 0; i < 3; i++) r.ReadU8();
        uint c1 = r.ReadU32();
        if (c1 < 0x80000000)
        {
            for (int i = 0; i < c1; i++) r.ReadU32();
        }
        r.ReadU8(); r.ReadU8();
        for (int i = 0; i < 4; i++) r.ReadU32();
        r.ReadU32();
        for (int i = 0; i < 3; i++) r.ReadU32();
        ushort raw = r.ReadU16();
        int count = raw < 0x8000 ? raw : 0;
        for (int i = 0; i < count; i++) r.ReadMemData(2);
        r.ReadU32(); r.ReadU8(); r.ReadU32(); r.ReadU32();
        r.ReadU32(); r.ReadU32();
    }

    public static ImportReport ImportAll(string saveDir, string? reportPath = null, bool rejectBadChecksum = false)
    {
        var report = new ImportReport();
        for (int slot = 1; slot <= 8; slot++)
        {
            string fileName = $"SaveData{slot:D2}.sav";
            string filePath = Path.Combine(saveDir, fileName);
            if (!File.Exists(filePath)) continue;
            try
            {
                var slotResult = ImportFile(filePath, slot, rejectBadChecksum);
                report.Slots.Add(slotResult);
            }
            catch (Exception ex) when (ex is InvalidDataException or EndOfStreamException or IOException or UnauthorizedAccessException)
            {
                string message = $"slot {slot:D2} ({fileName}): {ex.Message}";
                Console.Error.WriteLine($"[LegacySaveImporter] {message}");
                report.Errors.Add(message);
            }
        }

        if (!string.IsNullOrEmpty(reportPath))
        {
            string? dir = Path.GetDirectoryName(reportPath);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

            string json = JsonSerializer.Serialize(report, LwsmfJsonContext.Indented.ImportReport);
            WriteTextAtomic(reportPath, json);

            var warnings = new List<string>();
            foreach (var slot in report.Slots) warnings.AddRange(slot.EncodingWarnings);
            if (warnings.Count > 0)
            {
                string warningPath = Path.Combine(dir ?? ".", "encoding_warnings.json");
                WriteTextAtomic(warningPath, JsonSerializer.Serialize(warnings, LwsmfJsonContext.Indented.ListString));
            }
        }

        return report;
    }

    private static void WriteTextAtomic(string path, string content)
    {
        string temp = path + ".tmp";
        try
        {
            using (var stream = new FileStream(temp, FileMode.Create, FileAccess.Write, FileShare.None))
            using (var writer = new StreamWriter(stream, Encoding.UTF8))
            {
                writer.Write(content);
                writer.Flush();
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
}
