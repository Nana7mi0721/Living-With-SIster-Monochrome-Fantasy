using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace Lwsmf.Core;

public static class ProgramLoader
{
    // ---- shared artifact cache (M1-P2-02) -------------------------------
    // Every loader below used to re-open and re-parse the whole programs.json (550 common
    // events + 73 map events, several MB). Keep one parsed document per path, keyed by
    // size and mtime so a regenerated artifact invalidates itself. Callers must not
    // dispose the document they get back.

    private sealed class CachedDocument
    {
        public long Length;
        public DateTime WriteTimeUtc;
        public JsonDocument Document = null!;
    }

    private static readonly Dictionary<string, CachedDocument> Documents = new();

    private static JsonDocument? OpenDocument(string jsonPath)
    {
        string key = Path.GetFullPath(jsonPath);
        var info = new FileInfo(key);
        if (!info.Exists) return null;

        if (Documents.TryGetValue(key, out var cached) &&
            cached.Length == info.Length && cached.WriteTimeUtc == info.LastWriteTimeUtc)
        {
            return cached.Document;
        }

        JsonDocument document;
        using (var stream = File.OpenRead(key))
        {
            document = JsonDocument.Parse(stream);
        }

        Documents[key] = new CachedDocument
        {
            Length = info.Length,
            WriteTimeUtc = info.LastWriteTimeUtc,
            Document = document
        };
        return document;
    }

    /// <summary>Number of parsed artifacts currently held (tests assert the cache is shared).</summary>
    public static int CachedArtifactCount => Documents.Count;

    /// <summary>Forget parsed artifacts (tests that rewrite programs.json in place).</summary>
    public static void ClearCache()
    {
        foreach (var cached in Documents.Values) cached.Document.Dispose();
        Documents.Clear();
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
        return prog;
    }

    /// <summary>Parse every event page of one map out of the programs artifact.</summary>
    /// <remarks>
    /// Map programs are not registered in the common-event set: the original runs a map
    /// page as its own coroutine (an Instance owns its Program), and event ids in a map
    /// collide with common-event ids.
    /// </remarks>
    public static List<Program> LoadMapEvents(string jsonPath, string mapFile, int? mapId = null)
    {
        var list = new List<Program>();
        var doc = OpenDocument(jsonPath);
        if (doc == null) return list;

        if (!doc.RootElement.TryGetProperty("maps", out var maps) || maps.ValueKind != JsonValueKind.Array)
            return list;

        foreach (var mapElem in maps.EnumerateArray())
        {
            bool matches = false;
            if (mapId.HasValue)
            {
                matches = mapElem.TryGetProperty("id", out var idProp) && idProp.GetInt32() == mapId.Value;
            }

            if (!matches && !string.IsNullOrEmpty(mapFile) &&
                mapElem.TryGetProperty("file", out var fileProp) && fileProp.GetString() is string artFile)
            {
                string want = mapFile.Replace('\\', '/');
                string have = artFile.Replace('\\', '/');
                matches = have == want
                          || have.EndsWith("/" + want, StringComparison.Ordinal)
                          || Path.GetFileName(have) == Path.GetFileName(want);
            }
            if (!matches) continue;

            if (!mapElem.TryGetProperty("events", out var events) || events.ValueKind != JsonValueKind.Array)
                continue;
            foreach (var elem in events.EnumerateArray())
            {
                var localElem = elem;
                list.Add(ParseProgram(ref localElem));
            }
            break;
        }

        return list;
    }

    /// <summary>
    /// Load a whole map with its event pages (trigger, conditions, program). Mirrors the
    /// page grouping converter/parse/mps.py:302-348 performs: every page of an event is a
    /// separate entry in the artifact and is regrouped here by event id.
    /// </summary>
    public static MapDefinition? LoadMap(string jsonPath, string mapFile, int? mapId = null)
    {
        var doc = OpenDocument(jsonPath);
        if (doc == null) return null;

        if (!doc.RootElement.TryGetProperty("maps", out var maps) || maps.ValueKind != JsonValueKind.Array)
            return null;

        foreach (var mapElem in maps.EnumerateArray())
        {
            bool matches = false;
            if (mapId.HasValue)
            {
                matches = mapElem.TryGetProperty("id", out var idProp) && idProp.GetInt32() == mapId.Value;
            }

            if (!matches && !string.IsNullOrEmpty(mapFile) &&
                mapElem.TryGetProperty("file", out var fileProp) && fileProp.GetString() is string artFile)
            {
                string want = mapFile.Replace('\\', '/');
                string have = artFile.Replace('\\', '/');
                matches = have == want
                          || have.EndsWith("/" + want, StringComparison.Ordinal)
                          || Path.GetFileName(have) == Path.GetFileName(want);
            }
            if (!matches) continue;

            var map = new MapDefinition
            {
                Id = mapElem.TryGetProperty("id", out var mid) ? mid.GetInt32() : 0,
                File = mapElem.TryGetProperty("file", out var mfile) ? mfile.GetString() ?? "" : "",
                Name = mapElem.TryGetProperty("name", out var mname) ? mname.GetString() ?? "" : "",
                Width = mapElem.TryGetProperty("width", out var mw) ? mw.GetInt32() : 0,
                Height = mapElem.TryGetProperty("height", out var mh) ? mh.GetInt32() : 0,
                TilesetId = mapElem.TryGetProperty("tileset_id", out var mts) ? mts.GetInt32() : 0
            };

            if (!mapElem.TryGetProperty("events", out var events) || events.ValueKind != JsonValueKind.Array)
                return map;

            var byId = new Dictionary<int, MapEventDef>();
            foreach (var elem in events.EnumerateArray())
            {
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
                    var localElem = elem;
                    page.Program = ParseProgram(ref localElem);
                }

                def.Pages[pageIndex] = page;
            }

            foreach (var def in map.Events)
            {
                while (def.Pages.Count < def.PageCount) def.Pages.Add(new MapEventPage());
            }

            return map;
        }

        return null;
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
        var dict = new Dictionary<int, Program>();
        var doc = OpenDocument(jsonPath);
        if (doc == null) return dict;

        var root = doc.RootElement;

        if (root.TryGetProperty("common_events", out var ceList) && ceList.ValueKind == JsonValueKind.Array)
        {
            foreach (var elem in ceList.EnumerateArray())
            {
                var localElem = elem;
                var prog = ParseProgram(ref localElem);
                dict[prog.Id] = prog;
            }
        }

        return dict;
    }

    public static Program? LoadSingleCommonEvent(string jsonPath, int targetId)
    {
        if (!File.Exists(jsonPath)) return null;

        var doc = OpenDocument(jsonPath);
        if (doc == null) return null;
        var root = doc.RootElement;

        if (root.TryGetProperty("common_events", out var ceList) && ceList.ValueKind == JsonValueKind.Array)
        {
            foreach (var elem in ceList.EnumerateArray())
            {
                if (elem.TryGetProperty("id", out var idProp) && idProp.GetInt32() == targetId)
                {
                    var localElem = elem;
                    return ParseProgram(ref localElem);
                }
            }
        }

        return null;
    }
}
