using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace Lwsmf.Core;

public static class ProgramLoader
{
    // ---- shared artifact cache ------------------------------------------
    // Parse programs.json once per path (keyed by size and mtime) into compact
    // C# dictionaries and dispose the JsonDocument immediately so the ~50 MB
    // parse tree does not stay resident.

    private sealed class ParsedMap
    {
        public int Id;
        public string File = "";
        public MapDefinition Definition = null!;
        public List<Program> Events = new();
    }

    private sealed class CachedPrograms
    {
        public long Length;
        public DateTime WriteTimeUtc;
        public Dictionary<int, Program> CommonEvents = new();
        public List<ParsedMap> Maps = new();
    }

    private static readonly Dictionary<string, CachedPrograms> Artifacts =
        new(OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal);
    private static readonly object CacheLock = new();

    public static bool Exists(string jsonPath) => JsonArtifactIo.ResolvePath(jsonPath) != null;

    private static CachedPrograms? LoadArtifact(string jsonPath)
    {
        string? key = JsonArtifactIo.ResolvePath(jsonPath);
        if (key == null) return null;
        var info = new FileInfo(key);
        if (!info.Exists) return null;

        lock (CacheLock)
        {
            if (Artifacts.TryGetValue(key, out var cached))
            {
                if (cached.Length == info.Length && cached.WriteTimeUtc == info.LastWriteTimeUtc)
                {
                    return cached;
                }
                Artifacts.Remove(key);
            }
        }

        var commonEvents = new Dictionary<int, Program>();
        var maps = new List<ParsedMap>();
        using (var stream = JsonArtifactIo.OpenReadStream(key))
        using (var document = JsonDocument.Parse(stream))
        {
            var root = document.RootElement;
            if (root.TryGetProperty("common_events", out var ceList) && ceList.ValueKind == JsonValueKind.Array)
            {
                foreach (var elem in ceList.EnumerateArray())
                {
                    var localElem = elem;
                    var prog = ParseProgram(ref localElem);
                    commonEvents[prog.Id] = prog;
                }
            }
            if (root.TryGetProperty("maps", out var mapList) && mapList.ValueKind == JsonValueKind.Array)
            {
                foreach (var mapElem in mapList.EnumerateArray())
                {
                    maps.Add(ParseMapEntry(mapElem));
                }
            }
        }

        var parsed = new CachedPrograms
        {
            Length = info.Length,
            WriteTimeUtc = info.LastWriteTimeUtc,
            CommonEvents = commonEvents,
            Maps = maps
        };

        lock (CacheLock)
        {
            Artifacts[key] = parsed;
        }
        return parsed;
    }

    /// <summary>Number of parsed artifacts currently held (tests assert the cache is shared).</summary>
    public static int CachedArtifactCount
    {
        get { lock (CacheLock) return Artifacts.Count; }
    }

    /// <summary>Forget parsed artifacts (tests that rewrite programs.json in place).</summary>
    public static void ClearCache()
    {
        lock (CacheLock) Artifacts.Clear();
    }

    public static Slot ParseSlot(ref JsonElement elem)
    {
        int op = elem.GetProperty("op").GetInt32();
        int depth = elem.TryGetProperty("branch_depth", out var dProp) ? dProp.GetInt32() : 0;

        int[] parameters = Array.Empty<int>();
        if (elem.TryGetProperty("params", out var pElem) && pElem.ValueKind == JsonValueKind.Array)
        {
            parameters = new int[pElem.GetArrayLength()];
            int idx = 0;
            foreach (var p in pElem.EnumerateArray())
            {
                parameters[idx++] = ReadInt32(p);
            }
        }

        string[] strings = Array.Empty<string>();
        if (elem.TryGetProperty("strings", out var strElem) && strElem.ValueKind == JsonValueKind.Array)
        {
            strings = new string[strElem.GetArrayLength()];
            int idx = 0;
            foreach (var str in strElem.EnumerateArray())
            {
                strings[idx++] = str.GetString() ?? "";
            }
        }

        MoveRouteData? moveRoute = null;
        if (elem.TryGetProperty("move_route", out var routeElem) && routeElem.ValueKind == JsonValueKind.Object)
        {
            moveRoute = ParseMoveRoute(routeElem);
        }

        return new Slot
        {
            Opcode = op,
            Params = parameters,
            Strings = strings,
            BranchDepth = depth,
            MoveRoute = moveRoute
        };
    }

    /// <summary>
    /// Parse the WolfTL move-route tail carried by opcode 201. Command arguments are
    /// kept split the way the container stores them (u32 then u8); the interpreter
    /// only hands the whole route to the host.
    /// </summary>
    private static MoveRouteData ParseMoveRoute(JsonElement elem)
    {
        var commands = new List<RouteCommand>();
        if (elem.TryGetProperty("commands", out var cmdElem) && cmdElem.ValueKind == JsonValueKind.Array)
        {
            foreach (var c in cmdElem.EnumerateArray())
            {
                var u32 = new List<int>();
                if (c.TryGetProperty("u32_args", out var u32Elem) && u32Elem.ValueKind == JsonValueKind.Array)
                {
                    foreach (var value in u32Elem.EnumerateArray()) u32.Add(ReadInt32(value));
                }
                var u8 = new List<int>();
                if (c.TryGetProperty("u8_args", out var u8Elem) && u8Elem.ValueKind == JsonValueKind.Array)
                {
                    foreach (var value in u8Elem.EnumerateArray()) u8.Add(ReadInt32(value));
                }
                commands.Add(new RouteCommand
                {
                    Kind = c.TryGetProperty("kind", out var kindElem) ? ReadInt32(kindElem) : 0,
                    U32Args = u32.ToArray(),
                    U8Args = u8.ToArray()
                });
            }
        }

        return new MoveRouteData
        {
            AnimationFrequency = ReadRouteInt(elem, "animation_frequency"),
            BehaviorOptions = ReadRouteInt(elem, "behavior_options"),
            MoveFrequency = ReadRouteInt(elem, "move_frequency"),
            MoveRouteMode = ReadRouteInt(elem, "move_route_mode"),
            MoveSpeed = ReadRouteInt(elem, "move_speed"),
            RouteOptions = ReadRouteInt(elem, "route_options"),
            Commands = commands
        };
    }

    private static int ReadRouteInt(JsonElement elem, string name)
    {
        return elem.TryGetProperty(name, out var value) ? ReadInt32(value) : 0;
    }

    /// <summary>
    /// Reads one operand. The artifact stores operands as raw u32 bit patterns, so values
    /// above int.MaxValue (NO_RETURN_VALUE = 0xFFFFFFFF, magic with the high bit set) only
    /// fit after an unchecked narrowing read.
    /// </summary>
    private static int ReadInt32(JsonElement elem)
    {
        if (elem.ValueKind != JsonValueKind.Number) return 0;
        if (elem.TryGetInt32(out int v)) return v;
        if (elem.TryGetInt64(out long l)) return unchecked((int)l);
        return unchecked((int)(long)elem.GetDouble());
    }

    public static Program ParseProgram(ref JsonElement elem)
    {
        int id = elem.GetProperty("id").GetInt32();
        string title = elem.TryGetProperty("title", out var titleProp) ? titleProp.GetString() ?? "" : "";
        uint retValId = elem.TryGetProperty("return_value_id", out var retProp) ? retProp.GetUInt32() : Program.NoReturnValue;
        int selfBase = elem.TryGetProperty("self_base", out var selfProp) ? selfProp.GetInt32() : Program.CommonSelfBase;

        var slots = new List<Slot>();
        if (elem.TryGetProperty("slots", out var slotsElem) && slotsElem.ValueKind == JsonValueKind.Array)
        {
            foreach (var s in slotsElem.EnumerateArray())
            {
                var localSlot = s;
                slots.Add(ParseSlot(ref localSlot));
            }
        }

        var prog = Program.FromSlots(id, title, slots);
        prog.ReturnValueId = retValId;
        prog.SelfBase = selfBase;
        if (elem.TryGetProperty("run_condition_op_var_val", out var condition)
            && condition.ValueKind == JsonValueKind.Array
            && condition.GetArrayLength() >= 4)
        {
            prog.RunCondition = ReadInt32(condition[0]);
            prog.ConditionOperator = ReadInt32(condition[1]);
            prog.ConditionVariable = ReadInt32(condition[2]);
            prog.ConditionValue = ReadInt32(condition[3]);
        }
        return prog;
    }

    private static ParsedMap ParseMapEntry(JsonElement mapElem)
    {
        var map = new MapDefinition
        {
            Id = mapElem.TryGetProperty("id", out var mid) ? mid.GetInt32() : 0,
            File = mapElem.TryGetProperty("file", out var mfile) ? mfile.GetString() ?? "" : "",
            Name = mapElem.TryGetProperty("name", out var mname) ? mname.GetString() ?? "" : "",
            Width = mapElem.TryGetProperty("width", out var mw) ? mw.GetInt32() : 0,
            Height = mapElem.TryGetProperty("height", out var mh) ? mh.GetInt32() : 0,
            TilesetId = mapElem.TryGetProperty("tileset_id", out var mts) ? mts.GetInt32() : 0
        };
        var rawEvents = new List<Program>();

        if (!mapElem.TryGetProperty("events", out var events) || events.ValueKind != JsonValueKind.Array)
        {
            return new ParsedMap { Id = map.Id, File = map.File, Definition = map, Events = rawEvents };
        }

        var byId = new Dictionary<int, MapEventDef>();
        foreach (var elem in events.EnumerateArray())
        {
            var localElem = elem;
            Program rawProgram = ParseProgram(ref localElem);
            rawEvents.Add(rawProgram);

            int eventId = elem.TryGetProperty("id", out var eid) ? eid.GetInt32() : 0;
            if (!byId.TryGetValue(eventId, out var def))
            {
                def = new MapEventDef
                {
                    Id = eventId,
                    Name = Join(elem, "title", "event_name"),
                    X = elem.TryGetProperty("x", out var ex) ? ex.GetInt32() : 0,
                    Y = elem.TryGetProperty("y", out var ey) ? ey.GetInt32() : 0,
                    PageCount = elem.TryGetProperty("page_count", out var epc) ? epc.GetInt32() : 1,
                    Graphic = elem.TryGetProperty("graphic", out var eg) ? eg.GetString() ?? "" : ""
                };
                byId[eventId] = def;
                map.Events.Add(def);
            }

            int pageIndex = elem.TryGetProperty("page", out var ep) ? ep.GetInt32() : 0;
            if (pageIndex < 0) continue;
            while (def.Pages.Count <= pageIndex) def.Pages.Add(new MapEventPage());

            var page = new MapEventPage
            {
                Page = pageIndex,
                Trigger = MapTriggerExtensions.FromByte(
                    elem.TryGetProperty("trigger", out var trig) ? trig.GetInt32() : 0),
                Direction = elem.TryGetProperty("direction", out var dir) ? dir.GetInt32() : 2,
                Graphic = elem.TryGetProperty("graphic", out var pg) ? pg.GetString() ?? "" : "",
                BehaviorOptions = elem.TryGetProperty("features", out var feat) ? feat.GetInt32() : 0,
                CollisionWidth = elem.TryGetProperty("collision_width", out var cw) ? cw.GetInt32() : 0,
                CollisionHeight = elem.TryGetProperty("collision_height", out var chh) ? chh.GetInt32() : 0
            };

            int real = 0;
            if (elem.TryGetProperty("stats", out var stats) && stats.ValueKind == JsonValueKind.Object &&
                stats.TryGetProperty("real_instructions", out var ri))
            {
                real = ri.GetInt32();
            }
            page.RealInstructions = real;

            if (elem.TryGetProperty("trigger_conditions", out var conditions) &&
                conditions.ValueKind == JsonValueKind.Array)
            {
                int index = 0;
                foreach (var c in conditions.EnumerateArray())
                {
                    if (index >= page.Conditions.Length) break;
                    page.Conditions[index] = new PageCondition
                    {
                        Enabled = c.TryGetProperty("enabled", out var en) && en.ValueKind == JsonValueKind.True,
                        Operator = c.TryGetProperty("operator_code", out var op) ? op.GetInt32() : 0,
                        Variable = c.TryGetProperty("variable", out var v) ? ReadInt32(v) : 0,
                        Value = c.TryGetProperty("value", out var val) ? ReadInt32(val) : 0
                    };
                    index++;
                }
            }

            // An instruction-less page is not a runnable program (map_runtime.rs collect_except).
            if (real > 0)
            {
                page.Program = rawProgram;
            }

            def.Pages[pageIndex] = page;
        }

        foreach (var def in map.Events)
        {
            while (def.Pages.Count < def.PageCount) def.Pages.Add(new MapEventPage());
        }

        return new ParsedMap { Id = map.Id, File = map.File, Definition = map, Events = rawEvents };
    }

    private static ParsedMap? FindMap(CachedPrograms artifact, string mapFile, int? mapId)
    {
        foreach (var entry in artifact.Maps)
        {
            bool matches = false;
            if (mapId.HasValue)
            {
                matches = entry.Id == mapId.Value;
            }

            if (!matches && !string.IsNullOrEmpty(mapFile) && !string.IsNullOrEmpty(entry.File))
            {
                string want = mapFile.Replace('\\', '/');
                string have = entry.File.Replace('\\', '/');
                matches = string.Equals(have, want, StringComparison.OrdinalIgnoreCase)
                          || have.EndsWith("/" + want, StringComparison.OrdinalIgnoreCase)
                          || string.Equals(Path.GetFileName(have), Path.GetFileName(want), StringComparison.OrdinalIgnoreCase);
            }
            if (matches) return entry;
        }
        return null;
    }

    /// <summary>Parse every event page of one map out of the programs artifact.</summary>
    /// <remarks>
    /// Map programs are not registered in the common-event set: the original runs a map
    /// page as its own coroutine (an Instance owns its Program), and event ids in a map
    /// collide with common-event ids.
    /// </remarks>
    public static List<Program> LoadMapEvents(string jsonPath, string mapFile, int? mapId = null)
    {
        var artifact = LoadArtifact(jsonPath);
        if (artifact == null) return new List<Program>();
        var entry = FindMap(artifact, mapFile, mapId);
        return entry != null ? new List<Program>(entry.Events) : new List<Program>();
    }

    /// <summary>
    /// Load a whole map with its event pages (trigger, conditions, program). Mirrors the
    /// page grouping converter/parse/mps.py:302-348 performs: every page of an event is a
    /// separate entry in the artifact and is regrouped here by event id.
    /// </summary>
    public static MapDefinition? LoadMap(string jsonPath, string mapFile, int? mapId = null)
    {
        var artifact = LoadArtifact(jsonPath);
        if (artifact == null) return null;
        return FindMap(artifact, mapFile, mapId)?.Definition;
    }

    private static string Join(JsonElement elem, string first, string second)
    {
        if (elem.TryGetProperty(first, out var a) && a.ValueKind == JsonValueKind.String) return a.GetString() ?? "";
        if (elem.TryGetProperty(second, out var b) && b.ValueKind == JsonValueKind.String) return b.GetString() ?? "";
        return "";
    }

    /// <summary>First page of one map event, or null.</summary>
    public static Program? LoadMapEvent(string jsonPath, string mapFile, int eventId)
    {
        foreach (var prog in LoadMapEvents(jsonPath, mapFile))
        {
            if (prog.Id == eventId) return prog;
        }
        return null;
    }

    public static Dictionary<int, Program> LoadCommonEvents(string jsonPath)
    {
        var artifact = LoadArtifact(jsonPath);
        return artifact != null
            ? new Dictionary<int, Program>(artifact.CommonEvents)
            : new Dictionary<int, Program>();
    }

    public static Program? LoadSingleCommonEvent(string jsonPath, int targetId)
    {
        var artifact = LoadArtifact(jsonPath);
        if (artifact == null) return null;
        return artifact.CommonEvents.TryGetValue(targetId, out var prog) ? prog : null;
    }
}
