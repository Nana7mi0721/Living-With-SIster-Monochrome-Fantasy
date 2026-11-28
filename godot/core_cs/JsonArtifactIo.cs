using System;
using System.IO;
using System.IO.Compression;

namespace Lwsmf.Core;

/// <summary>
/// Resolves game_data JSON artifacts with transparent `.json.br` preference.
/// When a Brotli-compressed sibling exists, streams decompression directly without
/// landing an uncompressed copy on disk.
/// </summary>
internal static class JsonArtifactIo
{
    public static string? ResolvePath(string? jsonPath)
    {
        if (string.IsNullOrWhiteSpace(jsonPath)) return null;
        string full = Path.GetFullPath(jsonPath);
        string brPath = full.EndsWith(".br", StringComparison.OrdinalIgnoreCase)
            ? full
            : full + ".br";
        bool hasFull = File.Exists(full);
        bool hasBr = File.Exists(brPath);
        if (hasFull && hasBr && !string.Equals(full, brPath, StringComparison.OrdinalIgnoreCase))
        {
            return File.GetLastWriteTimeUtc(full) > File.GetLastWriteTimeUtc(brPath)
                ? full
                : brPath;
        }
        if (hasBr) return brPath;
        if (hasFull) return full;
        return null;
    }

    public static Stream OpenReadStream(string resolvedPath)
    {
        Stream raw = File.OpenRead(resolvedPath);
        if (!resolvedPath.EndsWith(".br", StringComparison.OrdinalIgnoreCase))
        {
            return raw;
        }
        try
        {
            return new BrotliStream(raw, CompressionMode.Decompress, leaveOpen: false);
        }
        catch
        {
            raw.Dispose();
            throw;
        }
    }
}
