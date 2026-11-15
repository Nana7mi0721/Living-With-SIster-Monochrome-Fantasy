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

    public static readonly string[] NotRecoverableFields =
    [
        "event_instances",
        "pc",
        "call_stack",
        "wait",
        "audio_phase"
    ];

    public sealed class SaveReader(byte[] buffer, int position = 0)
    {
        public byte[] Buffer { get; } = buffer;
        public int Position { get; set; } = position;

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
            if (count < 0 || Position + count > Buffer.Length)
                throw new EndOfStreamException($"ReadBytes({count}) past EOF at {Position}");
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

        public string ReadMemDataString(int size)
        {
            byte[] raw = ReadMemData(size);
            if (raw.Length == 0) return "";
            int len = raw.Length;
            if (raw[^1] == 0) len--;
            return len > 0 ? Encoding.UTF8.GetString(raw, 0, len) : "";
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
    }

    public sealed class SlotResult
    {
        [JsonPropertyName("index")] public int Index { get; set; }
        [JsonPropertyName("file")] public string File { get; set; } = "";
        [JsonPropertyName("bytes")] public long Bytes { get; set; }
        [JsonPropertyName("checksum_ok")] public bool ChecksumOk { get; set; }
        [JsonPropertyName("recovered")] public RecoveredData Recovered { get; set; } = new();
        [JsonPropertyName("not_recoverable")] public string[] NotRecoverable { get; set; } = NotRecoverableFields;
    }

    public sealed class ImportReport
    {
        [JsonPropertyName("schema")] public string Schema { get; set; } = "wolf.save_import";
        [JsonPropertyName("schema_version")] public int SchemaVersion { get; set; } = 1;
        [JsonPropertyName("slots")] public List<SlotResult> Slots { get; set; } = [];
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

    public static SlotResult ImportFile(string path, int slotIndex)
    {
        byte[] raw = File.ReadAllBytes(path);
        byte[] dec = Decrypt(raw);

        byte storedChecksum = raw[2];
        byte computedChecksum = 0;
        for (int i = CryptStart; i < dec.Length; i++)
        {
            computedChecksum = unchecked((byte)(computedChecksum + dec[i]));
        }
        bool checksumOk = (storedChecksum == computedChecksum);

        var r = new SaveReader(dec, CryptStart + 1);
        string saveName = r.ReadMemDataString(2);
        ushort fileVersion = r.ReadU16();

        // SavePart1
        r.ReadU32(); r.ReadU32(); r.ReadU32();
        uint mapWidth = r.ReadU32();
        uint mapHeight = r.ReadU32();
        uint sentinel = r.ReadU32();
        if (sentinel != uint.MaxValue)
        {
            r.Position -= 4;
            long cells = (long)mapWidth * mapHeight * 3;
            for (long i = 0; i < cells; i++) r.ReadU32();
        }
        string chipImage = r.ReadMemDataString(4);
        for (int i = 0; i < 31; i++) r.ReadMemData(4);
        uint vars2Count = r.ReadU32();
        var switches = new Dictionary<string, bool>();
        for (int i = 0; i < vars2Count; i++)
        {
            uint word = r.ReadU32();
            if (word != 0)
            {
                switches[i.ToString()] = true;
            }
        }

        uint objectCount = r.ReadU32();
        for (int i = 0; i < objectCount; i++)
        {
            SkipObject(r);
        }
        for (int i = 0; i < 11; i++) r.ReadU32();
        uint vars3Count = r.ReadU32();
        for (int i = 0; i < vars3Count; i++) r.ReadU32();

        // SavePart2
        r.ReadU8(); r.ReadU8();
        uint[] vars3_21 = new uint[19];
        for (int i = 0; i < 19; i++) vars3_21[i] = r.ReadU32();
        uint playTimeFrames = vars3_21[0];
        int sys3 = (int)vars3_21[9];
        int sys4 = (int)vars3_21[10];

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
        uint c1 = r.ReadU32(); for (int i = 0; i < c1; i++) r.ReadMemData(4);
        uint c2 = r.ReadU32(); for (int i = 0; i < c2; i++) r.ReadU32();
        for (int i = 0; i < 5; i++) r.ReadU32();
        for (int i = 0; i < 6; i++) r.ReadU32();
        r.ReadU16();
        r.ReadMemData(2); r.ReadMemData(2);
        r.ReadU32();
        for (int i = 0; i < 3; i++) r.ReadU32();
        r.ReadMemData(2); for (int i = 0; i < 3; i++) r.ReadU32();
        r.ReadMemData(2); for (int i = 0; i < 3; i++) r.ReadU32();
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
        uint mds2Count = r.ReadU32();
        for (int i = 0; i < mds2Count; i++) r.ReadMemData(4);

        // SavePart3
        r.ReadU32();
        uint groupCount = r.ReadU32();
        for (int i = 0; i < groupCount; i++)
        {
            uint gc = r.ReadU32();
            for (int j = 0; j < gc; j++)
            {
                byte per = r.ReadU8();
                for (int k = 0; k < per; k++) r.ReadU32();
            }
        }
        uint rowCount = r.ReadU32();
        var normalVars = new Dictionary<string, int>();
        for (int i = 0; i < rowCount; i++)
        {
            uint rc = r.ReadU32();
            for (int j = 0; j < rc; j++)
            {
                int val = r.ReadI32();
                if (i == 0)
                {
                    normalVars[j.ToString()] = val;
                }
            }
        }
        uint mds1Count = r.ReadU32();
        for (int i = 0; i < mds1Count; i++) r.ReadMemData(4);
        uint listCount = r.ReadU32();
        for (int i = 0; i < listCount; i++)
        {
            byte lc = r.ReadU8();
            for (int j = 0; j < lc; j++) r.ReadU32();
        }
        uint slotCount = r.ReadU32();
        for (int i = 0; i < slotCount; i++)
        {
            byte sc = r.ReadU8();
            for (int j = 0; j < sc; j++) r.ReadMemData(4);
        }

        // SavePart4
        SkipObject(r);
        uint p4Count = r.ReadU32();
        for (int i = 0; i < p4Count; i++) SkipObject(r);
        r.ReadU8(); r.ReadU8();
        uint p4Vars2 = r.ReadU32();
        for (int i = 0; i < p4Vars2; i++) r.ReadU32();
        uint p4Vars3 = r.ReadU32();
        for (int i = 0; i < p4Vars3; i++) r.ReadU64();

        // SavePart5 (Picture table)
        ushort p5Var1 = r.ReadU16();
        var pictures = new List<PictureSummary>();
        if ((p5Var1 & 0x8000) == 0)
        {
            for (int i = 0; i < p5Var1; i++)
            {
                uint picId = r.ReadU32();
                r.ReadU8(); r.ReadU8(); r.ReadU16();
                byte isText = r.ReadU8();
                byte isWin = r.ReadU8();
                string picName = r.ReadMemDataString(2);
                for (int k = 0; k < 12; k++) r.ReadU32();
                for (int k = 0; k < 6; k++) r.ReadU32();
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
                uint subCount = r.ReadU32();
                for (int subIdx = 0; subIdx < subCount; subIdx++)
                {
                    uint slen = r.ReadU32();
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
            }
        }

        // VariableDatabase
        r.ReadU8();
        uint typeCount = r.ReadU32();
        var dbOverrides = new List<int>();
        for (int t = 0; t < typeCount; t++)
        {
            int unk = r.ReadI32();
            int cnt = unk;
            if (unk <= -1)
            {
                if (unk <= -2) r.ReadI32();
                uint fc = r.ReadU32();
                cnt = (int)fc;
            }
            uint[] cfg = new uint[cnt > 0 ? cnt : 0];
            for (int k = 0; k < cfg.Length; k++) cfg[k] = r.ReadU32();

            uint typeRowCount = r.ReadU32();
            if (typeRowCount > 0)
            {
                dbOverrides.Add(t);
            }
            for (int row = 0; row < typeRowCount; row++)
            {
                foreach (uint def in cfg)
                {
                    if (def < 2000) r.ReadI32();
                }
                foreach (uint def in cfg)
                {
                    if (def >= 2000) r.ReadMemData(4);
                }
            }
        }

        // SavePart7
        byte p7Var1 = r.ReadU8();
        if (p7Var1 == 1)
        {
            uint p7Count = r.ReadU32();
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
                    ObjectsCount = (int)objectCount
                },
                ChoiceAnchor = [sys3, sys4],
                DatabaseOverrides = dbOverrides
            },
            NotRecoverable = NotRecoverableFields
        };
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

    public static ImportReport ImportAll(string saveDir, string? reportPath = null)
    {
        var report = new ImportReport();
        for (int slot = 1; slot <= 8; slot++)
        {
            string fileName = $"SaveData{slot:D2}.sav";
            string filePath = Path.Combine(saveDir, fileName);
            if (File.Exists(filePath))
            {
                var slotResult = ImportFile(filePath, slot);
                report.Slots.Add(slotResult);
            }
        }

        if (!string.IsNullOrEmpty(reportPath))
        {
            string? dir = Path.GetDirectoryName(reportPath);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

            var options = new JsonSerializerOptions
            {
                WriteIndented = true
            };
            string json = JsonSerializer.Serialize(report, options);
            File.WriteAllText(reportPath, json, Encoding.UTF8);
        }

        return report;
    }
}
