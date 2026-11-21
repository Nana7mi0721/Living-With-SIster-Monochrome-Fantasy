using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Lwsmf.Core;

public sealed class EventSelfVars
{
    [JsonPropertyName("id")] public int Id { get; set; }

    // Dense arrays remain for saves written before the sparse format. New saves
    // leave them empty and use the index maps below.
    [JsonPropertyName("nums")] public int[] Nums { get; set; } = Array.Empty<int>();
    [JsonPropertyName("strings")] public string[] Strings { get; set; } = Array.Empty<string>();
    [JsonPropertyName("num_map")] public Dictionary<int, int>? NumMap { get; set; }
    [JsonPropertyName("str_map")] public Dictionary<int, string>? StrMap { get; set; }

    /// <summary>Pack a 1000-slot CSelf bank as sparse index maps.</summary>
    public static EventSelfVars Pack(int id, int[] nums, string[] strings)
    {
        Dictionary<int, int>? numMap = null;
        int numLimit = Math.Min(nums.Length, SelfVars.TotalSlots);
        for (int i = 0; i < numLimit; i++)
        {
            if (nums[i] == 0) continue;
            numMap ??= new Dictionary<int, int>();
            numMap[i] = nums[i];
        }
        Dictionary<int, string>? strMap = null;
        int strLimit = Math.Min(strings.Length, SelfVars.TotalSlots);
        for (int i = 0; i < strLimit; i++)
        {
            if (string.IsNullOrEmpty(strings[i])) continue;
            strMap ??= new Dictionary<int, string>();
            strMap[i] = strings[i];
        }
        return new EventSelfVars { Id = id, NumMap = numMap, StrMap = strMap };
    }

    /// <summary>
    /// Expand to a dense array. Keys outside [0, SelfVars.TotalSlots) are ignored so a
    /// hand-edited save can never allocate an unbounded array or index out of range.
    /// </summary>
    public int[] ExpandNums()
    {
        if (NumMap == null || NumMap.Count == 0) return TrimDense(Nums);
        int length = 0;
        foreach (int key in NumMap.Keys)
        {
            if (key >= 0 && key < SelfVars.TotalSlots) length = Math.Max(length, key + 1);
        }
        var nums = new int[length];
        foreach (var pair in NumMap)
        {
            if (pair.Key >= 0 && pair.Key < SelfVars.TotalSlots) nums[pair.Key] = pair.Value;
        }
        return nums;
    }

    public string[] ExpandStrings()
    {
        if (StrMap == null || StrMap.Count == 0) return TrimDense(Strings);
        int length = 0;
        foreach (int key in StrMap.Keys)
        {
            if (key >= 0 && key < SelfVars.TotalSlots) length = Math.Max(length, key + 1);
        }
        var strings = new string[length];
        Array.Fill(strings, "");
        foreach (var pair in StrMap)
        {
            if (pair.Key >= 0 && pair.Key < SelfVars.TotalSlots) strings[pair.Key] = pair.Value ?? "";
        }
        return strings;
    }

    private static int[] TrimDense(int[]? nums)
    {
        if (nums == null) return Array.Empty<int>();
        return nums.Length <= SelfVars.TotalSlots ? nums : nums[..SelfVars.TotalSlots];
    }

    private static string[] TrimDense(string[]? strings)
    {
        if (strings == null) return Array.Empty<string>();
        if (strings.Length <= SelfVars.TotalSlots) return strings;
        return strings[..SelfVars.TotalSlots];
    }

    /// <summary>Reject banks that would corrupt the 1000-slot arena once expanded.</summary>
    public bool IsValid()
    {
        if (Id < 0) return false;
        if (Nums is { Length: > SelfVars.TotalSlots }) return false;
        if (Strings is { Length: > SelfVars.TotalSlots }) return false;
        foreach (string value in Strings) if (value == null) return false;
        if (NumMap != null)
        {
            foreach (var pair in NumMap)
            {
                if (pair.Key < 0 || pair.Key >= SelfVars.TotalSlots) return false;
            }
        }
        if (StrMap != null)
        {
            foreach (var pair in StrMap)
            {
                if (pair.Key < 0 || pair.Key >= SelfVars.TotalSlots) return false;
                if (pair.Value == null) return false;
            }
        }
        return true;
    }
}

public sealed class SavedLoop
{
    [JsonPropertyName("block")] public int Block { get; set; }
    [JsonPropertyName("remaining")] public long Remaining { get; set; }
}

/// <summary>One live EventVm coroutine: enough to resume at the same pc/wait without replay.</summary>
public sealed class SavedInstance
{
    [JsonPropertyName("id")] public int Id { get; set; }
    [JsonPropertyName("parent_id")] public int? ParentId { get; set; }
    [JsonPropertyName("event_id")] public int EventId { get; set; }
    [JsonPropertyName("pc")] public int Pc { get; set; }
    [JsonPropertyName("return_to")] public int? ReturnTo { get; set; }
    [JsonPropertyName("kind")] public string Kind { get; set; } = "ce";
    [JsonPropertyName("page")] public int Page { get; set; } = -1;
    [JsonPropertyName("wait_kind")] public int WaitKind { get; set; }
    [JsonPropertyName("wait_frames")] public uint WaitFrames { get; set; }
    [JsonPropertyName("wait_block")] public int WaitBlock { get; set; }
    [JsonPropertyName("wait_child")] public int WaitChild { get; set; }
    [JsonPropertyName("loops")] public List<SavedLoop> Loops { get; set; } = new();
    [JsonPropertyName("self")] public EventSelfVars Self { get; set; } = new();
}

/// <summary>MapRuntime per-event state: active page, live instance, counters and self variables.</summary>
public sealed class SavedMapEvent
{
    [JsonPropertyName("id")] public int Id { get; set; }
    [JsonPropertyName("active")] public int? Active { get; set; }
    [JsonPropertyName("instance_id")] public int? InstanceId { get; set; }
    [JsonPropertyName("runs")] public bool Runs { get; set; }
    [JsonPropertyName("starts")] public int Starts { get; set; }
    [JsonPropertyName("starts_on_page")] public int StartsOnPage { get; set; }
    [JsonPropertyName("self")] public EventSelfVars Self { get; set; } = new();
}

/// <summary>One live picture slot: enough to rebuild the layer after a load.</summary>
public sealed class SavedPicture
{
    [JsonPropertyName("number")] public int Number { get; set; }
    [JsonPropertyName("kind")] public int Kind { get; set; }
    [JsonPropertyName("action")] public int Action { get; set; }
    [JsonPropertyName("file")] public string File { get; set; } = "";
    [JsonPropertyName("text")] public string Text { get; set; } = "";
    [JsonPropertyName("args")] public List<int> Args { get; set; } = new();
    [JsonPropertyName("anchor")] public int Anchor { get; set; }
}

/// <summary>One audio track playing when the slot was written (BGM/BGS/SE).</summary>
public sealed class SavedAudio
{
    [JsonPropertyName("channel")] public int Channel { get; set; }
    [JsonPropertyName("file")] public string File { get; set; } = "";
    [JsonPropertyName("volume")] public int Volume { get; set; }
    [JsonPropertyName("loop")] public bool Loop { get; set; }
    [JsonPropertyName("loop_point")] public int LoopPoint { get; set; }
    [JsonPropertyName("tempo")] public int Tempo { get; set; } = 100;
    [JsonPropertyName("position_ms")] public int PositionMs { get; set; }
}

public sealed class SavedDialogue
{
    [JsonPropertyName("message")] public string? Message { get; set; }
    [JsonPropertyName("choices")] public List<string> Choices { get; set; } = new();
}

/// <summary>
/// Remake save slot. The retail .sav format is handled by <see cref="LegacySaveImporter"/>
/// (read-only); this format is what the remake writes under user://.
/// </summary>
public sealed class SaveState
{
    public const string SchemaName = "lwsmf.save";
    public const int SchemaVersionNumber = 1;

    [JsonPropertyName("schema")] public string Schema { get; set; } = SchemaName;
    [JsonPropertyName("schema_version")] public int SchemaVersion { get; set; } = SchemaVersionNumber;
    [JsonPropertyName("saved_at")] public string SavedAt { get; set; } = "";
    [JsonPropertyName("play_frames")] public long PlayFrames { get; set; }
    [JsonPropertyName("map_id")] public int MapId { get; set; }
    [JsonPropertyName("map_file")] public string MapFile { get; set; } = "";
    [JsonPropertyName("leader_x")] public int LeaderX { get; set; }
    [JsonPropertyName("leader_y")] public int LeaderY { get; set; }
    [JsonPropertyName("facing")] public int Facing { get; set; } = 2;
    [JsonPropertyName("vars")] public List<int> Vars { get; set; } = new();
    [JsonPropertyName("str_vars")] public List<string> StrVars { get; set; } = new();
    [JsonPropertyName("sys_vars")] public List<int> SysVars { get; set; } = new();
    [JsonPropertyName("sys_str_vars")] public List<string> SysStrVars { get; set; } = new();
    [JsonPropertyName("self_vars")] public List<EventSelfVars> SelfVars { get; set; } = new();
    [JsonPropertyName("common_self")] public List<EventSelfVars> CommonSelf { get; set; } = new();
    [JsonPropertyName("rng_state")] public uint RngState { get; set; }
    [JsonPropertyName("rng_event_state")] public uint RngEventState { get; set; }
    [JsonPropertyName("rng_event_seed")] public uint RngEventSeed { get; set; }
    [JsonPropertyName("rng_draws")] public ulong RngDraws { get; set; }
    [JsonPropertyName("map_events")] public List<SavedMapEvent> MapEvents { get; set; } = new();
    [JsonPropertyName("instances")] public List<SavedInstance> Instances { get; set; } = new();
    [JsonPropertyName("pictures")] public List<SavedPicture> Pictures { get; set; } = new();
    [JsonPropertyName("audio")] public List<SavedAudio> Audio { get; set; } = new();
    [JsonPropertyName("dialogue")] public SavedDialogue? Dialogue { get; set; }
}

/// <summary>Which file supplied a decoded slot: the main slot or its .bak rotation.</summary>
public enum SaveSlotSource
{
    None = 0,
    Main = 1,
    Backup = 2,
}

/// <summary>
/// Observable result of <see cref="SaveCodec.ReadWithSource"/>: the payload plus the file
/// it came from. The status line must be able to say "backup" when the main slot was
/// corrupt or missing.
/// </summary>
public readonly struct SaveReadResult
{
    public SaveReadResult(SaveState? state, SaveSlotSource source)
    {
        State = state;
        Source = source;
    }

    public SaveState? State { get; }
    public SaveSlotSource Source { get; }
    public bool Ok => State != null;
    public string SourceName => SaveCodec.SourceName(Source);
}

public static class SaveCodec
{
    /// <summary>Hard ceiling for one slot; user:// files above it are not parsed.</summary>
    public const int MaxSaveBytes = 8 * 1024 * 1024;
    /// <summary>Live coroutine ceiling (EventVm.MaxInstances); the loader must not build more.</summary>
    public const int MaxInstances = 4096;
    private const int MaxPc = 10_000_000;
    private const int MaxDialogue = 1_000_000;
    /// <summary>The retail screen never has more live picture slots than this.</summary>
    public const int MaxPictures = 256;
    /// <summary>Exactly one BGM, one BGS and one SE track can be live.</summary>
    public const int MaxAudio = 3;

    private static readonly JsonSerializerOptions Options = new()
    {
        WriteIndented = true,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };

    // Invalid UTF-8 must fail the read instead of silently decoding replacement chars.
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static string ToJson(SaveState state) => JsonSerializer.Serialize(state, Options);

    public static SaveState? FromJson(string json)
    {
        try
        {
            var state = JsonSerializer.Deserialize<SaveState>(json, Options);
            if (state == null) return null;
            if (state.Schema != SaveState.SchemaName) return null;
            if (state.SchemaVersion < 1 || state.SchemaVersion > SaveState.SchemaVersionNumber) return null;
            Normalize(state);
            return Validate(state) ? state : null;
        }
        catch (JsonException)
        {
            return null;
        }
        catch (NotSupportedException)
        {
            return null;
        }
    }

    private static void Normalize(SaveState state)
    {
        state.Vars ??= new List<int>();
        state.StrVars ??= new List<string>();
        state.SysVars ??= new List<int>();
        state.SysStrVars ??= new List<string>();
        state.SelfVars ??= new List<EventSelfVars>();
        state.CommonSelf ??= new List<EventSelfVars>();
        state.MapEvents ??= new List<SavedMapEvent>();
        state.Instances ??= new List<SavedInstance>();
        state.Pictures ??= new List<SavedPicture>();
        state.Audio ??= new List<SavedAudio>();
        foreach (var audio in state.Audio) audio.File ??= "";
        foreach (var pic in state.Pictures)
        {
            pic.File ??= "";
            pic.Text ??= "";
            pic.Args ??= new List<int>();
        }
        state.MapFile ??= "";
        state.SavedAt ??= "";
        if (state.Dialogue != null) state.Dialogue.Choices ??= new List<string>();
        foreach (var inst in state.Instances)
        {
            inst.Loops ??= new List<SavedLoop>();
            inst.Self ??= new EventSelfVars();
            inst.Kind ??= "ce";
        }
        foreach (var ev in state.MapEvents) ev.Self ??= new EventSelfVars();
    }

    /// <summary>
    /// Semantic gate that runs before any state reaches the runtime: a malformed slot
    /// must fall back to .bak, never expand keys or index past a 1000-slot bank.
    /// </summary>
    private static bool Validate(SaveState state)
    {
        if (!ValidVars(state.Vars, state.StrVars)) return false;
        if (!ValidVars(state.SysVars, state.SysStrVars)) return false;
        if (!ValidBanks(state.SelfVars) || !ValidBanks(state.CommonSelf)) return false;
        if (state.Instances.Count > MaxInstances) return false;
        if (state.MapEvents.Count > MaxInstances) return false;
        if (state.MapFile.Length > 260 || state.SavedAt.Length > 64) return false;
        if (!IsSafeAssetPath(state.MapFile)) return false;

        var ids = new HashSet<int>();
        foreach (var inst in state.Instances)
        {
            if (!ValidInstance(inst)) return false;
            if (!ids.Add(inst.Id)) return false;
        }

        foreach (var inst in state.Instances)
        {
            int depth = 0;
            var cursor = inst;
            while (cursor.ParentId is int parentId)
            {
                if (parentId == inst.Id) return false;
                if (++depth > MaxInstances) return false;
                cursor = state.Instances.Find(x => x.Id == parentId);
                if (cursor == null) return false;
            }
        }

        var eventIds = new HashSet<int>();
        foreach (var ev in state.MapEvents)
        {
            if (ev.Id < 0 || !eventIds.Add(ev.Id)) return false;
            if (ev.Active is int active && active < -1) return false;
            if (ev.InstanceId is int iid && iid <= 0) return false;
            if (ev.Starts < 0 || ev.StartsOnPage < 0) return false;
            if (!ev.Self.IsValid()) return false;
        }

        if (state.Pictures.Count > MaxPictures) return false;
        var slots = new HashSet<int>();
        foreach (var pic in state.Pictures)
        {
            if (!slots.Add(pic.Number)) return false;
            if (pic.Kind is < 0 or > 255) return false;
            if (pic.Action is < 0 or > 4) return false;
            if (pic.File.Length > 260 || pic.Text.Length > MaxDialogue) return false;
            if (!IsSafeAssetPath(pic.File)) return false;
            if (pic.Args.Count > 32) return false;
            if (pic.Anchor is < 0 or > 255) return false;
        }

        if (state.Audio.Count > MaxAudio) return false;
        var audioChannels = new HashSet<int>();
        foreach (var audio in state.Audio)
        {
            if (audio.Channel is < 0 or > 2 || !audioChannels.Add(audio.Channel)) return false;
            if (audio.File.Length > 260 || !IsSafeAssetPath(audio.File)) return false;
            if (audio.Volume is < 0 or > 200) return false;
            if (audio.LoopPoint < 0 || audio.LoopPoint > 86_400 * 60 || audio.Tempo is < 25 or > 400) return false;
            if (audio.PositionMs < 0 || audio.PositionMs > 86_400_000) return false;
        }

        if (state.Dialogue != null)
        {
            if (state.Dialogue.Message is { Length: > MaxDialogue }) return false;
            if (state.Dialogue.Choices.Count > 64) return false;
            foreach (string choice in state.Dialogue.Choices)
            {
                if (choice == null || choice.Length > MaxDialogue) return false;
            }
        }
        return true;
    }

    /// A picture file is a logical asset name resolved under work/all, never a path that
    /// can climb out of it or address another drive.
    private static bool IsSafeAssetPath(string path)
    {
        if (string.IsNullOrEmpty(path)) return true;
        if (path[0] == '/' || path[0] == '\\') return false;
        if (path.Contains(':')) return false;
        foreach (string segment in path.Split('/', '\\'))
        {
            if (segment == "..") return false;
        }
        return true;
    }

    private static bool ValidVars(List<int> nums, List<string> strings)
    {
        if (nums.Count > Variables.MaxSlots || strings.Count > Variables.MaxSlots) return false;
        foreach (string value in strings) if (value == null) return false;
        return true;
    }

    private static bool ValidBanks(List<EventSelfVars> banks)
    {
        var ids = new HashSet<int>();
        foreach (var bank in banks)
        {
            if (bank == null || !bank.IsValid() || !ids.Add(bank.Id)) return false;
        }
        return true;
    }

    private static bool ValidInstance(SavedInstance inst)
    {
        if (inst.Id <= 0 || inst.EventId < 0) return false;
        if (inst.Pc < 0 || inst.Pc > MaxPc) return false;
        if (inst.WaitKind is < 0 or > 4) return false;
        if (inst.WaitBlock < 0 || inst.WaitBlock > MaxPc) return false;
        if (inst.WaitChild < 0) return false;
        if (inst.Page < -1) return false;
        if (inst.Kind is not ("ce" or "map")) return false;
        if (inst.Loops.Count > 64) return false;
        foreach (var loop in inst.Loops)
        {
            if (loop == null || loop.Block < 0) return false;
        }
        return inst.Self != null && inst.Self.IsValid();
    }

    public static void Write(string path, SaveState state)
    {
        string? dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        // Rotate a known-good slot to .bak. A corrupt main file must never overwrite
        // the last good backup; the backup itself is written through a fsynced temp.
        if (File.Exists(path))
        {
            string? current = TryReadText(path);
            if (current != null && FromJson(current) != null)
            {
                try
                {
                    WriteRaw(path + ".bak", current);
                }
                catch (IOException)
                {
                    // Backup is best effort; the atomic replace below still protects slot 01.
                }
            }
        }

        WriteRaw(path, ToJson(state));
    }

    /// <summary>Write through path.tmp, fsync, then replace in one move.</summary>
    private static void WriteRaw(string path, string json)
    {
        string temp = path + ".tmp";
        try
        {
            using (var stream = new FileStream(temp, FileMode.Create, FileAccess.Write, FileShare.None))
            using (var writer = new StreamWriter(stream, new UTF8Encoding(false)))
            {
                writer.Write(json);
                writer.Flush();
                stream.Flush(true);
            }
            File.Move(temp, path, true);
        }
        catch
        {
            if (File.Exists(temp))
            {
                try { File.Delete(temp); } catch { }
            }
            throw;
        }
    }

    /// <summary>
    /// Read the slot and report which file supplied it. A corrupt or missing main slot
    /// falls back to the .bak rotation; <see cref="Read"/> stays the state-only API.
    /// </summary>
    public static SaveReadResult ReadWithSource(string path)
    {
        SaveState? main = ReadOne(path);
        if (main != null) return new SaveReadResult(main, SaveSlotSource.Main);
        SaveState? backup = ReadOne(path + ".bak");
        return backup != null
            ? new SaveReadResult(backup, SaveSlotSource.Backup)
            : new SaveReadResult(null, SaveSlotSource.None);
    }

    public static SaveState? Read(string path) => ReadWithSource(path).State;

    /// <summary>Stable lower-case label used by F5/F9 status lines and fault hints.</summary>
    public static string SourceName(SaveSlotSource source) => source switch
    {
        SaveSlotSource.Main => "main",
        SaveSlotSource.Backup => "backup",
        _ => "none",
    };

    /// <summary>True when a slot or its backup exists (title menu shows SAVE).</summary>
    public static bool Exists(string path)
    {
        return File.Exists(path) || File.Exists(path + ".bak");
    }

    private static SaveState? ReadOne(string path)
    {
        string? text = TryReadText(path);
        return text == null ? null : FromJson(text);
    }

    private static string? TryReadText(string path)
    {
        try
        {
            var info = new FileInfo(path);
            if (!info.Exists || info.Length > MaxSaveBytes) return null;
            return File.ReadAllText(path, StrictUtf8);
        }
        catch (IOException)
        {
            return null;
        }
        catch (UnauthorizedAccessException)
        {
            return null;
        }
        catch (DecoderFallbackException)
        {
            return null;
        }
        catch (NotSupportedException)
        {
            return null;
        }
    }

    public static byte[] ToBytes(SaveState state) => new UTF8Encoding(false).GetBytes(ToJson(state));

    public static SaveState? FromBytes(byte[] bytes)
    {
        try
        {
            return FromJson(StrictUtf8.GetString(bytes));
        }
        catch (DecoderFallbackException)
        {
            return null;
        }
    }
}
