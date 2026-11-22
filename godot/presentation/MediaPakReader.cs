using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Text;
using Godot;

namespace Lwsmf.Presentation;

/// <summary>
/// Read-only reader for a WOLF media pack (<c>media.pak</c>).
///
/// The on-disk format matches the Rust <c>wolf-format</c> reader:
/// a 32-byte plaintext header, 16-byte-aligned file bodies, and an obfuscated
/// index at the end of the file. Every body and the index are XORed with a
/// fixed offset-indexed splitmix64 stream, and every body carries a zlib CRC-32.
/// Lookup is case-insensitive, with the same five-extension probe used by the
/// Rust asset layer when a name has no extension.
/// </summary>
[GlobalClass]
public partial class MediaPakReader : RefCounted
{
    private const int HeaderSize = 32;
    private const int IndexFixed = 22;
    private const ulong DataAlignment = 16;
    private const int StreamBlockSize = 64;
    private const ulong MaxIndexBytes = 64UL * 1024 * 1024;
    private const ulong MaxEntryBytes = 256UL * 1024 * 1024;
    private const ulong Golden = 0x9E3779B97F4A7C15UL;

    private static readonly byte[] Magic = "WOLFMPK1"u8.ToArray();
    private static readonly byte[] Seed =
    {
        0x9F, 0x2E, 0x4B, 0xD7, 0x18, 0x63, 0xA5, 0x0C,
        0xE4, 0x71, 0x35, 0x8A, 0xC2, 0x5D, 0x90, 0x26,
        0x6B, 0xF3, 0x04, 0xBE, 0x57, 0xA1, 0xD8, 0x39,
        0x82, 0x4C, 0xEF, 0x16, 0x75, 0xB0, 0x2D, 0x9A,
    };
    private static readonly string[] ProbeExtensions = { "png", "jpg", "bmp", "jpeg", "webp" };
    private static readonly uint[] CrcTable = BuildCrcTable();

    private sealed class Entry
    {
        public string Path = "";
        public ulong Offset;
        public ulong Size;
        public uint Crc;
    }

    private readonly object _gate = new();
    private readonly Dictionary<string, Entry> _entries = new(StringComparer.OrdinalIgnoreCase);
    private FileStream? _file;
    private string _packPath = "";
    private string _lastError = "";

    /// <summary>True after a successful <see cref="Open"/>.</summary>
    public bool IsOpen
    {
        get { lock (_gate) return _file != null; }
    }

    /// <summary>Number of indexed files.</summary>
    public int Count
    {
        get { lock (_gate) return _entries.Count; }
    }

    /// <summary>Absolute path of the pack, or empty when not open.</summary>
    public string PackPath
    {
        get { lock (_gate) return _packPath; }
    }

    /// <summary>Last human-readable failure, cleared by a successful read.</summary>
    public string LastError
    {
        get { lock (_gate) return _lastError; }
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            Close();
        }
        base.Dispose(disposing);
    }

    /// <summary>Opens a pack and loads only its header and index.</summary>
    public bool Open(string path)
    {
        lock (_gate)
        {
            CloseLocked();
            _lastError = "";
            if (string.IsNullOrWhiteSpace(path))
            {
                _lastError = "media pack path is empty";
                return false;
            }

            FileStream? stream = null;
            bool opened = false;
            try
            {
                stream = new FileStream(path, FileMode.Open, System.IO.FileAccess.Read, System.IO.FileShare.Read);
                long fileLength = stream.Length;
                if (fileLength < HeaderSize)
                {
                    _lastError = $"media pack is {fileLength} bytes, shorter than the {HeaderSize}-byte header";
                    return false;
                }

                byte[] header = new byte[HeaderSize];
                stream.ReadExactly(header);
                if (!header.AsSpan(0, 8).SequenceEqual(Magic))
                {
                    _lastError = "media pack has bad magic";
                    return false;
                }
                ushort version = BinaryPrimitives.ReadUInt16LittleEndian(header.AsSpan(8, 2));
                if (version != 1)
                {
                    _lastError = $"unsupported media pack version {version}";
                    return false;
                }
                ushort flags = BinaryPrimitives.ReadUInt16LittleEndian(header.AsSpan(10, 2));
                if (flags != 0)
                {
                    _lastError = $"unsupported media pack header flags 0x{flags:X4}";
                    return false;
                }

                uint count = BinaryPrimitives.ReadUInt32LittleEndian(header.AsSpan(12, 4));
                ulong indexOffset = BinaryPrimitives.ReadUInt64LittleEndian(header.AsSpan(16, 8));
                ulong indexSize = BinaryPrimitives.ReadUInt64LittleEndian(header.AsSpan(24, 8));
                if (indexOffset < HeaderSize || (indexOffset & (DataAlignment - 1)) != 0)
                {
                    _lastError = $"media pack index offset {indexOffset} is invalid";
                    return false;
                }
                if (indexOffset > (ulong)fileLength || indexSize > (ulong)fileLength - indexOffset)
                {
                    _lastError = "media pack index range is outside the file";
                    return false;
                }
                if (indexOffset + indexSize != (ulong)fileLength)
                {
                    _lastError = "media pack index does not end at end of file";
                    return false;
                }
                if (indexSize > MaxIndexBytes || count > indexSize / IndexFixed)
                {
                    _lastError = "media pack index is too large or internally inconsistent";
                    return false;
                }

                byte[] index = new byte[(int)indexSize];
                stream.Position = (long)indexOffset;
                stream.ReadExactly(index);
                XorBytes(index, indexOffset);
                if (!ParseIndex(index, count, indexOffset, out string parseError))
                {
                    _lastError = parseError;
                    return false;
                }

                _file = stream;
                _packPath = path;
                opened = true;
                return true;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException or ArgumentException)
            {
                _lastError = error.Message;
                return false;
            }
            finally
            {
                if (!opened)
                {
                    _entries.Clear();
                    _packPath = "";
                    _file = null;
                    stream?.Dispose();
                }
            }
        }
    }

    /// <summary>Closes the file handle and drops the index.</summary>
    public void Close()
    {
        lock (_gate)
        {
            CloseLocked();
        }
    }

    private void CloseLocked()
    {
        _file?.Dispose();
        _file = null;
        _entries.Clear();
        _packPath = "";
    }

    /// <summary>Canonical paths in index order.</summary>
    public string[] Paths()
    {
        lock (_gate)
        {
            string[] paths = new string[_entries.Count];
            int index = 0;
            foreach (Entry entry in _entries.Values)
            {
                paths[index++] = entry.Path;
            }
            Array.Sort(paths, StringComparer.Ordinal);
            return paths;
        }
    }

    /// <summary>True when the pack contains the name (case-insensitive).</summary>
    public bool Has(string name)
    {
        lock (_gate)
        {
            return TryFind(name, out _);
        }
    }

    /// <summary>
    /// Reads one file by path. Returns null when absent or damaged; a damaged body
    /// sets <see cref="LastError"/> instead of being returned silently.
    /// </summary>
    public byte[]? Read(string name)
    {
        lock (_gate)
        {
            if (_file == null)
            {
                _lastError = "media pack is not open";
                return null;
            }
            if (!TryFind(name, out Entry? entry) || entry == null)
            {
                _lastError = "";
                return null;
            }
            if (entry.Size > MaxEntryBytes || entry.Size > (ulong)_file.Length)
            {
                _lastError = $"{entry.Path} exceeds maximum readable entry size";
                return null;
            }

            try
            {
                byte[] bytes = new byte[(int)entry.Size];
                _file.Position = (long)entry.Offset;
                _file.ReadExactly(bytes);
                XorBytes(bytes, entry.Offset);
                uint actual = Crc32(bytes);
                if (actual != entry.Crc)
                {
                    _lastError = $"media pack CRC mismatch for {entry.Path}: expected {entry.Crc:X8}, got {actual:X8}";
                    return null;
                }
                _lastError = "";
                return bytes;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                _lastError = error.Message;
                return null;
            }
        }
    }

    private bool ParseIndex(byte[] index, uint count, ulong indexOffset, out string error)
    {
        error = "";
        _entries.Clear();
        int cursor = 0;
        for (uint number = 0; number < count; number++)
        {
            if (index.Length - cursor < IndexFixed)
            {
                error = $"media pack entry {number} is truncated";
                return false;
            }
            int pathLength = BinaryPrimitives.ReadUInt16LittleEndian(index.AsSpan(cursor, 2));
            ulong offset = BinaryPrimitives.ReadUInt64LittleEndian(index.AsSpan(cursor + 2, 8));
            ulong size = BinaryPrimitives.ReadUInt64LittleEndian(index.AsSpan(cursor + 10, 8));
            uint crc = BinaryPrimitives.ReadUInt32LittleEndian(index.AsSpan(cursor + 18, 4));
            cursor += IndexFixed;
            if (pathLength < 0 || cursor > index.Length - pathLength)
            {
                error = $"media pack entry {number} has a truncated path";
                return false;
            }
            string path = Encoding.UTF8.GetString(index, cursor, pathLength);
            cursor += pathLength;
            if (!IsValidPath(path))
            {
                error = $"media pack entry {number} has an invalid path";
                return false;
            }
            if (_entries.ContainsKey(path))
            {
                error = $"media pack has a duplicate path: {path}";
                return false;
            }
            if (offset < HeaderSize || (offset & (DataAlignment - 1)) != 0)
            {
                error = $"media pack entry {path} has an invalid offset";
                return false;
            }
            if (size > MaxEntryBytes || size > indexOffset || offset > indexOffset - size)
            {
                error = $"media pack entry {path} has invalid size or extends past the index";
                return false;
            }
            _entries[path] = new Entry { Path = path, Offset = offset, Size = size, Crc = crc };
        }
        if (cursor != index.Length)
        {
            error = $"media pack index has {index.Length - cursor} trailing byte(s)";
            return false;
        }
        return true;
    }

    private bool TryFind(string name, out Entry? entry)
    {
        entry = null;
        if (string.IsNullOrWhiteSpace(name))
        {
            return false;
        }
        string lookup = name.Trim();
        if (lookup.Contains('\\'))
        {
            lookup = lookup.Replace('\\', '/');
        }
        if (_entries.TryGetValue(lookup, out entry))
        {
            return true;
        }
        if (HasExtension(lookup))
        {
            return false;
        }
        foreach (string extension in ProbeExtensions)
        {
            if (_entries.TryGetValue(lookup + "." + extension, out entry))
            {
                return true;
            }
        }
        return false;
    }

    private static bool HasExtension(string path)
    {
        int slash = path.LastIndexOf('/');
        int dot = path.LastIndexOf('.');
        return dot > slash && dot >= 0;
    }

    private static bool IsValidPath(string path)
    {
        if (path.Length == 0 || path.Length > ushort.MaxValue)
        {
            return false;
        }
        if (path.Contains('\0') || path.Contains('\\') || path.Contains(':') || path.StartsWith('/'))
        {
            return false;
        }
        foreach (string segment in path.Split('/'))
        {
            if (segment.Length == 0 || segment == "." || segment == "..")
            {
                return false;
            }
        }
        return true;
    }

    private static uint Crc32(byte[] data)
    {
        uint crc = 0xFFFF_FFFF;
        foreach (byte value in data)
        {
            crc = CrcTable[(crc ^ value) & 0xFF] ^ (crc >> 8);
        }
        return crc ^ 0xFFFF_FFFF;
    }

    private static uint[] BuildCrcTable()
    {
        uint[] table = new uint[256];
        for (uint index = 0; index < table.Length; index++)
        {
            uint value = index;
            for (int bit = 0; bit < 8; bit++)
            {
                value = (value & 1) != 0 ? (value >> 1) ^ 0xEDB8_8320 : value >> 1;
            }
            table[index] = value;
        }
        return table;
    }

    private static void XorBytes(byte[] bytes, ulong offset)
    {
        int done = 0;
        ulong currentBlock = ulong.MaxValue;
        byte[]? stream = null;
        while (done < bytes.Length)
        {
            ulong absolute = unchecked(offset + (ulong)done);
            ulong block = absolute / StreamBlockSize;
            int within = (int)(absolute % StreamBlockSize);
            if (block != currentBlock)
            {
                stream = StreamBlock(block);
                currentBlock = block;
            }
            int take = Math.Min(StreamBlockSize - within, bytes.Length - done);
            for (int index = 0; index < take; index++)
            {
                bytes[done + index] ^= stream![within + index];
            }
            done += take;
        }
    }

    private static byte[] StreamBlock(ulong block)
    {
        ulong state = unchecked(block * Golden);
        for (int offset = 0; offset < Seed.Length; offset += 8)
        {
            ulong seedWord = BinaryPrimitives.ReadUInt64LittleEndian(Seed.AsSpan(offset, 8));
            state = unchecked((state + seedWord) * Golden);
            state ^= state >> 29;
        }
        byte[] output = new byte[StreamBlockSize];
        for (int offset = 0; offset < StreamBlockSize; offset += 8)
        {
            state = unchecked(state + Golden);
            ulong value = state;
            value = unchecked((value ^ (value >> 30)) * 0xBF58_476D_1CE4_E5B9UL);
            value = unchecked((value ^ (value >> 27)) * 0x94D0_49BB_1331_11EBUL);
            value ^= value >> 31;
            BinaryPrimitives.WriteUInt64LittleEndian(output.AsSpan(offset, 8), value);
        }
        return output;
    }
}
