using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace Lwsmf.Core;

/// <summary>One tileset row from game_data/map_tiles.json.</summary>
public sealed class MapTileset
{
    public int Index { get; init; }
    public string Name { get; init; } = "";
    public string BaseImage { get; init; } = "";
    public List<string> Autotiles { get; init; } = new();
    public uint[] Passability { get; init; } = Array.Empty<uint>();
}

/// <summary>One map's three tile layers from game_data/map_tiles.json.</summary>
public sealed class MapTileEntry
{
    public int Id { get; init; }
    public string File { get; init; } = "";
    public int Width { get; init; }
    public int Height { get; init; }
    public int TilesetId { get; init; }
    public bool HasTileData { get; init; }

    /// <summary>Three u32 layers, layer-major, width*height words each (missing layers are empty).</summary>
    public List<uint[]> Layers { get; init; } = new();
}

/// <summary>
/// Reads game_data/map_tiles.json (converter export/map_tiles.py) and builds a
/// <see cref="Terrain"/> for a loaded map, since programs.json carries no tile words.
/// </summary>
public sealed class MapTileData
{
    private sealed class Cached
    {
        public long Length { get; init; }
        public DateTime WriteTimeUtc { get; init; }
        public MapTileData Data { get; init; } = null!;
    }

    private static readonly Dictionary<string, Cached> Cache = new(StringComparer.OrdinalIgnoreCase);

    public List<MapTileset> Tilesets { get; } = new();
    public List<MapTileEntry> Maps { get; } = new();

    /// <summary>Artifact path next to the programs.json the session already loads.</summary>
    public static string Alongside(string programsPath) =>
        Path.Combine(Path.GetDirectoryName(Path.GetFullPath(programsPath)) ?? ".", "map_tiles.json");

    /// <summary>Load and cache by full path + length + mtime. Returns null when missing or malformed.</summary>
    public static MapTileData? Load(string path)
    {
        try
        {
            string? key = JsonArtifactIo.ResolvePath(path);
            if (key == null) return null;
            var info = new FileInfo(key);
            if (!info.Exists) return null;
            if (Cache.TryGetValue(key, out Cached? cached) &&
                cached.Length == info.Length && cached.WriteTimeUtc == info.LastWriteTimeUtc)
            {
                return cached.Data;
            }

            using Stream stream = JsonArtifactIo.OpenReadStream(key);
            using var doc = JsonDocument.Parse(stream);
            JsonElement root = doc.RootElement;
            var data = new MapTileData();
            foreach (JsonElement element in root.GetProperty("tilesets").EnumerateArray())
            {
                var autotiles = new List<string>();
                if (element.TryGetProperty("autotiles", out JsonElement auto))
                {
                    foreach (JsonElement item in auto.EnumerateArray()) autotiles.Add(item.GetString() ?? "");
                }
                data.Tilesets.Add(new MapTileset
                {
                    Index = element.GetProperty("index").GetInt32(),
                    Name = element.TryGetProperty("name", out JsonElement name) ? name.GetString() ?? "" : "",
                    BaseImage = element.TryGetProperty("base_image", out JsonElement image) ? image.GetString() ?? "" : "",
                    Autotiles = autotiles,
                    Passability = DecodeU32(element.GetProperty("passability").GetString() ?? "")
                });
            }

            foreach (JsonElement element in root.GetProperty("maps").EnumerateArray())
            {
                var layers = new List<uint[]>();
                if (element.TryGetProperty("layers", out JsonElement layerArray))
                {
                    foreach (JsonElement layer in layerArray.EnumerateArray())
                    {
                        layers.Add(DecodeU32(layer.GetString() ?? ""));
                    }
                }
                data.Maps.Add(new MapTileEntry
                {
                    Id = element.GetProperty("id").GetInt32(),
                    File = element.TryGetProperty("file", out JsonElement file) ? file.GetString() ?? "" : "",
                    Width = element.GetProperty("width").GetInt32(),
                    Height = element.GetProperty("height").GetInt32(),
                    TilesetId = element.GetProperty("tileset_id").GetInt32(),
                    HasTileData = element.TryGetProperty("has_tile_data", out JsonElement has) && has.GetBoolean(),
                    Layers = layers
                });
            }

            Cache[key] = new Cached
            {
                Length = info.Length,
                WriteTimeUtc = info.LastWriteTimeUtc,
                Data = data
            };
            return data;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException or FormatException or InvalidOperationException)
        {
            Console.Error.WriteLine($"[MapTileData] failed to load {path}: {ex.Message}");
            return null;
        }
    }

    public static MapTileData? LoadAlongside(string programsPath) =>
        string.IsNullOrEmpty(programsPath) ? null : Load(Alongside(programsPath));

    public MapTileEntry? Find(int mapId)
    {
        foreach (MapTileEntry entry in Maps)
        {
            if (entry.Id == mapId) return entry;
        }
        return null;
    }

    /// <summary>Terrain for a loaded map, or null when the artifact has no tile data for it.</summary>
    public Terrain? BuildTerrain(MapDefinition map)
    {
        MapTileEntry? entry = Find(map.Id);
        if (entry == null || entry.Layers.Count == 0) return null;
        MapTileset? tileset = null;
        foreach (MapTileset candidate in Tilesets)
        {
            if (candidate.Index == entry.TilesetId)
            {
                tileset = candidate;
                break;
            }
        }
        if (tileset == null) return null;
        int width = map.Width > 0 ? map.Width : entry.Width;
        int height = map.Height > 0 ? map.Height : entry.Height;
        if (width <= 0 || height <= 0) return null;
        return Terrain.FromLayers(width, height, entry.Layers, tileset.Passability);
    }

    public static int CachedArtifactCount => Cache.Count;

    public static void ClearCache() => Cache.Clear();

    private static uint[] DecodeU32(string base64)
    {
        if (base64.Length == 0) return Array.Empty<uint>();
        byte[] bytes = Convert.FromBase64String(base64);
        var values = new uint[bytes.Length / 4];
        for (int i = 0; i < values.Length; i++)
        {
            values[i] = BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(i * 4, 4));
        }
        return values;
    }
}
