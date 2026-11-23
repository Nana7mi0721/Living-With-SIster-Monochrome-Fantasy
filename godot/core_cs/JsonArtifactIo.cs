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
        if (File.Exists(brPath)) return brPath;
        if (File.Exists(full)) return full;
        return null;
    }

    public static Stream OpenReadStream(string resolvedPath)
    {
        Stream raw = File.OpenRead(resolvedPath);
        if (!resolvedPath.EndsWith(".br", StringComparison.OrdinalIgnoreCase))
        {
            return raw;
        }
        return new BrotliStream(raw, CompressionMode.Decompress, leaveOpen: false);
    }
}
