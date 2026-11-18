using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Full inventory of opcode 150 negative scale commands in game_data/programs.json.
/// The -1000000 sentinel is the Move/field "UNCHANGED" marker, not a mirror; every
/// other negative scale must be an image/window picture. Text slots (kind 2/4) must
/// never carry a real negative scale, because the current presentation mirrors the
/// whole holder and would reverse the glyphs (R7-NEW-03).
/// </summary>
public static class Op150NegativeScaleScanTest
{
    private const int Unchanged = -1_000_000;

    private sealed class Stats
    {
        public int Total;
        public int Sentinel;
        public int Negative;
        public int Draw;
        public int Move;
        public int Text;
        public readonly Dictionary<int, int> ByScale = new();
    }

    public static void Run()
    {
        string path = M13Harness.ProgramsPath();
        using JsonDocument doc = JsonDocument.Parse(File.ReadAllText(path));
        var stats = new Stats();
        var failures = new List<string>();

        if (doc.RootElement.TryGetProperty("common_events", out JsonElement common))
        {
            foreach (JsonElement ev in common.EnumerateArray())
                ScanSlots(ev, "ce", stats, failures);
        }

        if (doc.RootElement.TryGetProperty("maps", out JsonElement maps))
        {
            foreach (JsonElement map in maps.EnumerateArray())
            {
                int mapId = map.TryGetProperty("id", out JsonElement id) ? id.GetInt32() : -1;
                if (!map.TryGetProperty("events", out JsonElement events)) continue;
                foreach (JsonElement ev in events.EnumerateArray())
                    ScanSlots(ev, "map" + mapId, stats, failures);
            }
        }

        Check(stats.Total == 20425, $"op150 total={stats.Total}, expected 20425", failures);
        Check(stats.Sentinel == 474, $"op150 sentinel={stats.Sentinel}, expected 474", failures);
        Check(stats.Negative == 63, $"op150 negative={stats.Negative}, expected 63", failures);
        Check(stats.Draw == 57, $"op150 negative draw={stats.Draw}, expected 57", failures);
        Check(stats.Move == 6, $"op150 negative move={stats.Move}, expected 6", failures);
        Check(stats.Text == 0, $"op150 negative text={stats.Text}, expected 0", failures);
        Check(ScaleCount(stats, -100) == 53, $"scale -100 count={ScaleCount(stats, -100)}, expected 53", failures);
        Check(ScaleCount(stats, -200) == 7, $"scale -200 count={ScaleCount(stats, -200)}, expected 7", failures);
        Check(ScaleCount(stats, -1300) == 3, $"scale -1300 count={ScaleCount(stats, -1300)}, expected 3", failures);

        if (failures.Count > 0)
        {
            foreach (string failure in failures) Console.WriteLine("  OP150_SCAN_FAIL " + failure);
            throw new Exception($"op150 negative scale scan failed: {failures.Count} issue(s)");
        }

        Console.WriteLine($"OP150_NEGATIVE_SCALE_SCAN_OK total={stats.Total} sentinel={stats.Sentinel} negative={stats.Negative} draw={stats.Draw} move={stats.Move} text={stats.Text} m100={ScaleCount(stats, -100)} m200={ScaleCount(stats, -200)} m1300={ScaleCount(stats, -1300)}");
    }

    private static void ScanSlots(JsonElement ev, string scope, Stats stats, List<string> failures)
    {
        if (!ev.TryGetProperty("slots", out JsonElement slots) || slots.ValueKind != JsonValueKind.Array)
            return;
        int eventId = ev.TryGetProperty("id", out JsonElement id) ? id.GetInt32() : -1;
        foreach (JsonElement slot in slots.EnumerateArray())
        {
            if (!slot.TryGetProperty("op", out JsonElement op) || op.GetInt32() != 150)
                continue;
            stats.Total++;
            int[] p = slot.TryGetProperty("params", out JsonElement pars)
                ? pars.EnumerateArray().Select(ReadInt).ToArray()
                : Array.Empty<int>();
            int scale = p.Length > 9 ? p[9] : 100;
            if (scale == Unchanged)
            {
                stats.Sentinel++;
                continue;
            }
            if (scale >= 0) continue;

            stats.Negative++;
            stats.ByScale[scale] = ScaleCount(stats, scale) + 1;
            int head = p.Length > 0 ? p[0] : 0;
            int action = head & 0xF;
            int kind = (head >> 4) & 7;
            int anchor = (head >> 12) & 7;
            if (action == 0) stats.Draw++;
            else if (action == 1) stats.Move++;
            else failures.Add($"{scope} ev{eventId} pc{slot.GetProperty("pc").GetInt32()}: action={action} scale={scale}");

            if (kind == 2 || kind == 4) stats.Text++;
            if (kind != 0 && kind != 3)
                failures.Add($"{scope} ev{eventId} pc{slot.GetProperty("pc").GetInt32()}: kind={kind} scale={scale} (text/window-string would mirror glyphs)");

            string file = "";
            if (slot.TryGetProperty("strings", out JsonElement strings) && strings.ValueKind == JsonValueKind.Array)
                file = strings.EnumerateArray().Select(s => s.GetString() ?? "").FirstOrDefault() ?? "";
            if (action == 0 && kind == 0 && file.Length == 0)
                failures.Add($"{scope} ev{eventId} pc{slot.GetProperty("pc").GetInt32()}: negative file picture without a source");
            _ = anchor;
        }
    }

    private static int ScaleCount(Stats stats, int scale)
        => stats.ByScale.TryGetValue(scale, out int count) ? count : 0;

    private static int ReadInt(JsonElement e)
    {
        if (e.TryGetInt32(out int v)) return v;
        if (e.TryGetInt64(out long l)) return unchecked((int)l);
        return unchecked((int)e.GetDouble());
    }

    private static void Check(bool ok, string message, List<string> failures)
    {
        if (!ok) failures.Add(message);
    }
}
