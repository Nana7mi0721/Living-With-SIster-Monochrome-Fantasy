using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.Text;

namespace Lwsmf.Core;

/// <summary>
/// Binary parser for WOLF RPG Editor <c>.project</c> companion files used to resolve
/// editor type and field labels against exported database tables.
/// </summary>
internal static class WolfProjectParser
{
    private const uint MaxFields = 4096;
    private const uint MaxRows = 100_000;
    private const uint MaxEntries = 100_000;
    private const uint MaxNameBytes = 4096;

    internal sealed class ProjectType
    {
        public string Name { get; init; } = "";
        public List<string> Fields { get; init; } = new();
    }

    internal static ProjectType?[]? Parse(byte[] data, int typeCount)
    {
        if (typeCount < 0 || data.Length < 4 || ReadU32(data, 0) != (uint)typeCount) return null;
        var result = new ProjectType?[typeCount];
        int at = 4;
        for (int i = 0; i < typeCount; i++)
        {
            var parsed = ParseType(data, at);
            if (parsed == null) break;
            result[i] = parsed.Value.Type;
            at = parsed.Value.Next;
        }
        return result;
    }

    private static (ProjectType Type, int Next)? ParseType(byte[] data, int at)
    {
        if (!NameAt(data, at, out string name, out at)) return null;
        if (at + 4 > data.Length) return null;
        uint fieldCount = ReadU32(data, at);
        at += 4;
        if (fieldCount > (uint)(data.Length - at) / 5 || fieldCount > MaxFields) return null;
        var fields = new List<string>((int)fieldCount);
        for (uint i = 0; i < fieldCount; i++)
        {
            if (!NameAt(data, at, out string field, out at)) return null;
            fields.Add(field);
        }

        if (at + 4 > data.Length) return null;
        uint rowCount = ReadU32(data, at);
        at += 4;
        if (rowCount > (uint)(data.Length - at) / 5 || rowCount > MaxRows) return null;
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
        if (entries > (uint)(data.Length - at) / 4 || entries > MaxEntries) return null;
        for (uint i = 0; i < entries; i++)
        {
            if (at + 4 > data.Length) return null;
            uint arguments = ReadU32(data, at);
            at += 4;
            if (arguments > (uint)(data.Length - at) / 5 || arguments > MaxFields) return null;
            for (uint j = 0; j < arguments; j++)
            {
                if (!NameAt(data, at, out _, out at)) return null;
            }
        }

        if (at + 4 > data.Length) return null;
        entries = ReadU32(data, at);
        at += 4;
        if (entries > (uint)(data.Length - at) / 4 || entries > MaxEntries) return null;
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
        if (count > (uint)(data.Length - at) / 5 || count > MaxRows) return false;
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
        if (length == 0 || length > MaxNameBytes || length > (uint)(data.Length - at - 4)) return false;
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
