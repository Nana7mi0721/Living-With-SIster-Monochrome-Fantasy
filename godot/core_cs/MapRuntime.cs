using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>Map event page trigger kinds (crates/wolf-format/src/mps.rs Trigger).</summary>
public enum MapTrigger
{
    ActionKey = 0,
    Autorun = 1,
    Parallel = 2,
    PlayerTouch = 3,
    Unknown = 4
}

public static class MapTriggerExtensions
{
    public static MapTrigger FromByte(int value) => value switch
    {
        0 => MapTrigger.ActionKey,
        1 => MapTrigger.Autorun,
        2 => MapTrigger.Parallel,
        3 => MapTrigger.PlayerTouch,
        _ => MapTrigger.Unknown
    };

    /// <summary>Autorun, parallel and unknown pages start on their own (map_runtime.rs is_automatic).</summary>
    public static bool IsAutomatic(this MapTrigger trigger) =>
        trigger is MapTrigger.Autorun or MapTrigger.Parallel or MapTrigger.Unknown;

    public static bool IsLooping(this MapTrigger trigger) => trigger == MapTrigger.Parallel;
}

/// <summary>One page condition of a map event page.</summary>
public sealed class PageCondition
{
    public bool Enabled { get; set; }
    public int Operator { get; set; }
    public int Variable { get; set; }
    public int Value { get; set; }

    /// <summary>crates/wolf-format/src/mps.rs:527 PageCondition::compare; unknown operators never match.</summary>
    public bool Compare(int left) => Operator switch
    {
        0 => left > Value,
        1 => left >= Value,
        2 => left == Value,
        3 => left <= Value,
        4 => left < Value,
        5 => left != Value,
        _ => false
    };
}

public sealed class MapEventPage
{
    public int Page { get; set; }
    public MapTrigger Trigger { get; set; } = MapTrigger.Unknown;
    public PageCondition[] Conditions { get; set; } = EmptyConditions();
    public Program? Program { get; set; }
    public string Graphic { get; set; } = "";
    public int Direction { get; set; } = 2;
    public int RealInstructions { get; set; }

    /// <summary>Editor features byte: 0x08 slip-through, 0x20 square hit box (mps.rs PageState).</summary>
    public int BehaviorOptions { get; set; }
    public int CollisionWidth { get; set; }
    public int CollisionHeight { get; set; }

    /// <summary>A slip-through event can be walked over (map_runtime.rs skip_through).</summary>
    public bool SkipThrough => (BehaviorOptions & 0x08) != 0;

    private static PageCondition[] EmptyConditions()
    {
        var list = new PageCondition[4];
        for (int i = 0; i < list.Length; i++) list[i] = new PageCondition();
        return list;
    }
}

public sealed class MapEventDef
{
    public int Id { get; set; }
    public string Name { get; set; } = "";
    public int X { get; set; }
    public int Y { get; set; }
    public int PageCount { get; set; } = 1;
    public string Graphic { get; set; } = "";
    public List<MapEventPage> Pages { get; } = new();
}

public sealed class MapDefinition
{
    public int Id { get; set; }
    public string File { get; set; } = "";
    public string Name { get; set; } = "";
    public int Width { get; set; }
    public int Height { get; set; }
    public int TilesetId { get; set; }
    public List<MapEventDef> Events { get; } = new();
}

/// <summary>Runtime state of one map event (map_runtime.rs EventState).</summary>
public sealed class MapEventState
{
    public MapEventDef Def { get; init; } = null!;
    public int? Active { get; set; }
    public int? InstanceId { get; set; }
    public bool Runs { get; set; }
    public int Starts { get; set; }
    public int StartsOnPage { get; set; }
    public SelfVars SelfVars { get; } = new();
}

/// <summary>
/// Map page selection and autoreserved page startup, ported from
/// crates/wolf-game/src/map_runtime.rs (select_page / start_pending / start_trigger_page).
/// </summary>
public sealed class MapRuntime
{
    public MapDefinition Map { get; }
    public int SkipEvent { get; }
    public List<MapEventState> Events { get; } = new();
    public int Starts { get; private set; }

    /// <summary>Tile passability grid for this map; null until the host loads map_tiles.json.</summary>
    public Terrain? Terrain { get; private set; }

    public void SetTerrain(Terrain? terrain) => Terrain = terrain;

    /// <summary>True when an active, non-slip-through event page sits on this tile (map_runtime.rs event_occupies).</summary>
    public bool EventOccupies(int x, int y)
    {
        foreach (var state in Events)
        {
            if (state.Active is not int index) continue;
            if (index < 0 || index >= state.Def.Pages.Count) continue;
            if (state.Def.Pages[index].SkipThrough) continue;
            if (state.Def.X == x && state.Def.Y == y) return true;
        }
        return false;
    }

    /// <summary>Leader may enter this tile: inside the map, clear of events, and not solid terrain.</summary>
    public bool CanEnter(int x, int y)
    {
        if (x < 0 || y < 0 || x >= Map.Width || y >= Map.Height) return false;
        if (EventOccupies(x, y)) return false;
        return Terrain == null || !Terrain.IsSolidCell(x, y);
    }

    /// <summary>Leader may step into this tile from a direction (0 up, 1 right, 2 left, 3 down).</summary>
    public bool CanEnter(int x, int y, int step)
    {
        if (x < 0 || y < 0 || x >= Map.Width || y >= Map.Height) return false;
        if (EventOccupies(x, y)) return false;
        return Terrain == null || !Terrain.IsBlocked(x, y, step);
    }

    /// <summary>
    /// map_runtime.rs:965 page_contact_box: base hit box one tile wide by half a tile high
    /// (square option: full tile), extended by collision width/height in half-tile units.
    /// </summary>
    public static (int MinX, int MinY, int MaxX, int MaxY) ContactBox(MapEventState state, MapEventPage page)
    {
        const int TilePx = 16;
        const int ExtensionUnitPx = 8;
        bool square = (page.BehaviorOptions & 0x20) != 0;
        int baseHalfW = TilePx / 2;
        int baseHalfH = square ? TilePx / 2 : TilePx / 4;
        int halfW = baseHalfW + page.CollisionWidth * ExtensionUnitPx;
        int halfH = baseHalfH + page.CollisionHeight * ExtensionUnitPx;
        int centerX = state.Def.X * TilePx + TilePx / 2;
        int centerY = state.Def.Y * TilePx + TilePx / 2;
        int minX = FloorDiv(centerX - halfW, TilePx);
        int maxX = FloorDiv(centerX + halfW - 1, TilePx);
        int minY = FloorDiv(centerY - halfH, TilePx);
        int maxY = FloorDiv(centerY + halfH - 1, TilePx);
        return (minX, minY, maxX, maxY);
    }

    private static int FloorDiv(int numerator, int divisor)
    {
        int q = numerator / divisor;
        int r = numerator % divisor;
        if (r != 0 && (r < 0) != (divisor < 0)) q--;
        return q;
    }

    /// <summary>
    /// map_runtime.rs:1002 try_contact_triggers: player-touch pages fire when the step's
    /// attempted destination enters the contact box from outside. Blocked steps still fire.
    /// </summary>
    public List<(MapEventState State, int Page)> ContactTriggers(int fromX, int fromY, int toX, int toY)
    {
        var list = new List<(MapEventState, int)>();
        foreach (var state in Events)
        {
            if (state.Active is not int index || index < 0 || index >= state.Def.Pages.Count) continue;
            var page = state.Def.Pages[index];
            if (page.Trigger != MapTrigger.PlayerTouch) continue;
            var box = ContactBox(state, page);
            bool InBox(int x, int y) => x >= box.MinX && x <= box.MaxX && y >= box.MinY && y <= box.MaxY;
            if (InBox(toX, toY) && !InBox(fromX, fromY)) list.Add((state, index));
        }
        list.Sort((a, b) => a.Item1.Def.Id.CompareTo(b.Item1.Def.Id));
        return list;
    }

    public MapRuntime(MapDefinition map, int skipEvent = -1)
    {
        Map = map;
        SkipEvent = skipEvent;
        foreach (var def in map.Events)
        {
            var state = new MapEventState
            {
                Def = def,
                // collect_except: a page without instructions is not a runnable program.
                Runs = def.Id != skipEvent && def.Pages.Exists(p => p.Program != null)
            };
            Events.Add(state);
        }
    }

    public MapEventState? Find(int eventId)
    {
        foreach (var state in Events)
        {
            if (state.Def.Id == eventId) return state;
        }
        return null;
    }

    public int AutorunCount => CountBy(MapTrigger.Autorun);
    public int ParallelCount => CountBy(MapTrigger.Parallel);

    private int CountBy(MapTrigger trigger)
    {
        int n = 0;
        foreach (var state in Events)
        {
            if (state.Active is int index && index >= 0 && index < state.Def.Pages.Count &&
                state.Def.Pages[index].Trigger == trigger)
            {
                n++;
            }
        }
        return n;
    }

    /// <summary>map_runtime.rs:1402 — the last page whose conditions all hold wins.</summary>
    public int? SelectPage(MapEventState state, EventVm vm)
    {
        var selfVars = SelfVarsFor(state, vm);
        for (int i = state.Def.Pages.Count - 1; i >= 0; i--)
        {
            var page = state.Def.Pages[i];
            if (page == null) continue;
            bool ok = true;
            foreach (var condition in page.Conditions)
            {
                if (!condition.Enabled) continue;
                if (!condition.Compare(ConditionValue(condition.Variable, selfVars, vm.Vars)))
                {
                    ok = false;
                    break;
                }
            }
            if (ok) return i;
        }
        return null;
    }

    private static int ConditionValue(int raw, SelfVars selfVars, Variables vars)
    {
        var magic = Magic.Decode(raw);
        return magic.Kind switch
        {
            MagicKind.Var => vars.Get((int)magic.Index),
            MagicKind.SysVar => vars.GetSys((int)magic.Index),
            MagicKind.SelfVar => selfVars.Get((int)magic.Index),
            _ => raw
        };
    }

    private static SelfVars SelfVarsFor(MapEventState state, EventVm vm)
    {
        if (state.InstanceId is int id && vm.FindInstance(id) is Instance live)
        {
            return live.Self;
        }
        return state.SelfVars;
    }

    /// <summary>map_runtime.rs:1224 — reap finished pages, swap pages, start automatic pages.</summary>
    public int StartPending(EventVm vm)
    {
        int started = 0;
        foreach (var state in Events)
        {
            if (state.InstanceId is int runningId)
            {
                var live = vm.FindInstance(runningId);
                if (live == null || live.Pc >= live.Program.Len)
                {
                    if (live != null)
                    {
                        state.SelfVars.CopyFrom(live.Self);
                        vm.StopInstance(live.Id);
                    }
                    state.InstanceId = null;
                }
            }

            int? want = SelectPage(state, vm);
            if (state.Active != want)
            {
                var live = state.InstanceId is int id ? vm.FindInstance(id) : null;

                // Do not cancel an in-flight autorun coroutine or a child-waiting page
                // while its own script clears the condition variable before finishing.
                if (want == null && live != null && (!live.IsParallel || live.Wait.Kind == WaitStateKind.Child || live.JustResumedFromChild)) continue;

                if (live != null)
                {
                    state.SelfVars.CopyFrom(live.Self);
                    vm.StopInstance(live.Id);
                    state.InstanceId = null;
                }
                state.Active = want;
                // A new page owns a fresh autorun budget (map_runtime.rs:1281). Without
                // this reset a page that once ran can never restart after switching back.
                state.StartsOnPage = 0;
            }

            if (!state.Runs || state.Active is not int pageIndex) continue;
            if (pageIndex < 0 || pageIndex >= state.Def.Pages.Count) continue;

            var page = state.Def.Pages[pageIndex];
            if (page.Program == null || !page.Trigger.IsAutomatic()) continue;
            if (state.InstanceId is int existing && vm.FindInstance(existing) != null) continue;
            // Autorun runs once per activation; only parallel pages restart on their
            // own (map_runtime.rs:1310). Without this guard every finished autorun
            // is recreated on the next StartPending, which replay never does.
            if (state.StartsOnPage > 0 && !page.Trigger.IsLooping()) continue;

            var inst = vm.CreateInstance(page.Program);
            if (inst == null) continue;
            inst.RetainAfterFinish = true;
            inst.IsParallel = page.Trigger == MapTrigger.Parallel;
            inst.Self.CopyFrom(state.SelfVars);
            state.InstanceId = inst.Id;
            state.Starts++;
            state.StartsOnPage++;
            Starts++;
            started++;
        }
        return started;
    }

    /// <summary>map_runtime.rs:707 — start the active page on demand (action key / touch).</summary>
    public bool StartTriggerPage(int eventId, int pageIndex, EventVm vm)
    {
        var state = Find(eventId);
        if (state == null) return false;
        if (!state.Runs || state.Active != pageIndex) return false;
        if (state.InstanceId is int running && vm.FindInstance(running) != null) return false;
        if (pageIndex < 0 || pageIndex >= state.Def.Pages.Count) return false;

        var page = state.Def.Pages[pageIndex];
        if (page.Program == null) return false;

        var inst = vm.CreateInstance(page.Program);
        if (inst == null) return false;
        inst.RetainAfterFinish = true;
        inst.IsParallel = page.Trigger == MapTrigger.Parallel;
        inst.Self.CopyFrom(state.SelfVars);
        state.InstanceId = inst.Id;
        state.Starts++;
        state.StartsOnPage++;
        Starts++;
        return true;
    }

    /// <summary>
    /// map_runtime.rs:660 — pages that react to a key press / player touch at the leader's
    /// tile, sorted by event id (action key first). Events without a graphic must be stood on
    /// (map_runtime.rs:755 player_contacts).
    /// </summary>
    /// <summary>Copy every event's self variables (save slots need the page state they carry).</summary>
    public Dictionary<int, SelfVars> SnapshotSelfVars()
    {
        var snapshot = new Dictionary<int, SelfVars>();
        foreach (var state in Events) snapshot[state.Def.Id] = state.SelfVars.Clone();
        return snapshot;
    }

    public void RestoreSelfVars(Dictionary<int, SelfVars> snapshot)
    {
        foreach (var state in Events)
        {
            if (snapshot.TryGetValue(state.Def.Id, out var vars)) state.SelfVars.CopyFrom(vars);
        }
    }

    /// <summary>Snapshot page selection, counters and self variables (live instance wins over the cache).</summary>
    public List<SavedMapEvent> SnapshotStates(EventVm vm)
    {
        var saved = new List<SavedMapEvent>();
        foreach (var state in Events)
        {
            SelfVars self = SelfVarsFor(state, vm);
            saved.Add(new SavedMapEvent
            {
                Id = state.Def.Id,
                Active = state.Active,
                InstanceId = state.InstanceId,
                Runs = state.Runs,
                Starts = state.Starts,
                StartsOnPage = state.StartsOnPage,
                Self = EventSelfVars.Pack(state.Def.Id, self.SnapshotNums(), self.SnapshotStrings())
            });
        }
        return saved;
    }

    public void RestoreStates(List<SavedMapEvent> saved)
    {
        var byId = new Dictionary<int, SavedMapEvent>();
        foreach (var item in saved) byId[item.Id] = item;
        foreach (var state in Events)
        {
            if (!byId.TryGetValue(state.Def.Id, out var item)) continue;
            state.Active = item.Active;
            state.InstanceId = item.InstanceId;
            state.Runs = item.Runs;
            state.Starts = item.Starts;
            state.StartsOnPage = item.StartsOnPage;
            state.SelfVars.Restore(item.Self);
        }
    }

    public List<(MapEventState State, int Page)> TriggerCandidates(
        bool actionPressed, bool touch, int leaderX, int leaderY, int facing)
    {
        var list = new List<(MapEventState, int)>();
        foreach (var state in Events)
        {
            if (!state.Runs || state.Active is not int pageIndex) continue;
            if (pageIndex < 0 || pageIndex >= state.Def.Pages.Count) continue;
            var page = state.Def.Pages[pageIndex];
            if (page.Program == null) continue;

            bool wantsAction = actionPressed && page.Trigger == MapTrigger.ActionKey;
            bool wantsTouch = touch && page.Trigger == MapTrigger.PlayerTouch;
            if (!wantsAction && !wantsTouch) continue;
            if (!PlayerContacts(state, page, leaderX, leaderY, facing)) continue;

            list.Add((state, pageIndex));
        }
        list.Sort((a, b) => a.Item1.Def.Id.CompareTo(b.Item1.Def.Id));
        return list;
    }

    private static bool PlayerContacts(MapEventState state, MapEventPage page, int leaderX, int leaderY, int facing)
    {
        var def = state.Def;
        if (leaderX == def.X && leaderY == def.Y) return true;
        if (string.IsNullOrEmpty(page.Graphic)) return false; // must stand on it

        (int dx, int dy) = facing switch
        {
            2 => (0, 1),
            4 => (-1, 0),
            6 => (1, 0),
            8 => (0, -1),
            _ => (0, 0)
        };
        return leaderX + dx == def.X && leaderY + dy == def.Y;
    }
}
