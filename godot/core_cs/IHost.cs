using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

public enum PictureAction
{
    Draw = 0,
    Move = 1,
    Erase = 2,
    Hide = 3,
    Set = 4
}

public sealed class PictureCmd
{
    public int Number { get; init; }
    public byte Kind { get; init; }
    public PictureAction Action { get; init; }
    public string File { get; init; } = "";
    public string Text { get; init; } = "";
    public int[] Args { get; init; } = Array.Empty<int>();
    public byte Anchor { get; init; }
    public int[] Animate { get; init; } = Array.Empty<int>();
    /// <summary>Head bits 20..23. Mode 3 means SCALE_Y is independent; otherwise it follows SCALE.</summary>
    public byte ScaleMode { get; init; }
    /// <summary>Head bits 8..11: blend mode (0=normal, 1=add, 2=sub, 3=mul).</summary>
    public byte BlendMode { get; init; }
    /// <summary>Head bit 0x0040_0000: a redraw keeps the running 290 shift.</summary>
    public bool KeepShift { get; init; }
    /// <summary>Head bit 0x0001_0000: X/Y are offsets from the slot's current position.</summary>
    public bool Relative { get; init; }
    /// <summary>Head bit 0x0100_0000: erase/hide carry the far endpoint of a slot range.</summary>
    public bool HasSpan { get; init; }
}

public enum SoundChannel
{
    Bgm,
    Se,
    /// <summary>Background ambience: opcode 140 sound type 1 (`0x10`).</summary>
    Bgs
}

public enum SoundAction
{
    Play,
    Stop
}

public sealed class SoundCmd
{
    public SoundChannel Channel { get; init; }
    public SoundAction Action { get; init; }
    public string File { get; init; } = "";
    public int Volume { get; init; }
    public int Frames { get; init; }
    /// <summary>intArgs[6]: loop point in game frames (BGM/BGS only).</summary>
    public int LoopPoint { get; init; }
    /// <summary>intArgs[5]: playback speed percentage (100 = normal).</summary>
    public int Tempo { get; init; } = 100;
}

public sealed class ChoiceReply
{
    public bool IsCancel { get; init; }
    public int Index { get; init; }

    public static ChoiceReply Select(int index) => new() { Index = index, IsCancel = false };
    public static ChoiceReply Cancel => new() { IsCancel = true };
}

public enum DbContainer
{
    CData = 0,
    Sys = 1,
    Data = 2
}

public enum DbDirection
{
    Write = 0,
    Read = 1
}

public sealed class DbValue
{
    public bool IsString { get; }
    public int Num { get; }
    public string Str { get; }

    public DbValue(int num)
    {
        IsString = false;
        Num = num;
        Str = "";
    }

    public DbValue(string str)
    {
        IsString = true;
        Num = 0;
        Str = str ?? "";
    }
}

public sealed class DbOp
{
    public DbContainer Container { get; init; }
    public DbDirection Direction { get; init; }
    public byte Op { get; init; }
    public int DbType { get; init; }
    public int DataId { get; init; }
    public int ItemId { get; init; }
    public int Flags { get; init; }
    public int? Destination { get; init; }
    public DbValue? Source { get; init; }
    public string? TypeLabel { get; init; }
    public string? FieldLabel { get; init; }
}

public sealed class SaveTarget
{
    public bool IsPath { get; }
    public int Slot { get; }
    public string Path { get; }

    public SaveTarget(int slot)
    {
        IsPath = false;
        Slot = slot;
        Path = "";
    }

    public SaveTarget(string path)
    {
        IsPath = true;
        Slot = 0;
        Path = path ?? "";
    }
}

public sealed class SaveOp
{
    public byte Opcode { get; init; } // 220, 221, 222
    public int Mode { get; init; }     // 0 = save, 1 = load
    public SaveTarget File { get; init; } = new(0);
    public int Destination { get; init; }
    public int Address { get; init; }
    public int Value { get; init; }
    public int Flag { get; init; }
    public int Owner { get; init; }
    /// <summary>Populated by 221 when reading a string cell (e.g., CData string field).</summary>
    public string? LoadedString { get; set; }
}

public interface IHost
{
    void Wait(uint frames);
    void ShowMessage(string text, int? face);
    /// <summary>Show a prompt that stays on screen while the next 102 choice opens.</summary>
    void ShowMessage(string text, int? face, bool keepOpen) => ShowMessage(text, face);
    bool MessageDismissed();
    void CloseMessage();

    void ShowChoices(string[] options);
    /// <summary>102 with bit4 clear exposes a cancel branch; bit4 set must ignore cancel.</summary>
    void ShowChoices(string[] options, bool allowCancel) => ShowChoices(options);
    ChoiceReply? ChoiceMade();

    void CommonEventCall(int target, int[] args, string[] strings);
    void Picture(PictureCmd cmd);
    void Sound(SoundCmd cmd);
    DbValue? Database(DbOp op);

    /// <summary>
    /// Database cell text for \cdb/\sdb/\udb escapes. This is a read for display, not a
    /// script database op: recorder hosts must forward it without appending a trace effect
    /// (the rust oracle expands text outside the VM, so a logged read would diverge).
    /// </summary>
    string DbText(int kind, int dbType, int row, int column) => "";
    void Teleport(int target, int map, int x, int y, int options);
    void Transition(int kind, int flags);

    int? SaveLoad(SaveOp op);
    int? PictureState(int number, int field);
    int? CharacterState(int target, int field);
    int? EngineState(int field);
    int KeyCode(uint mask);

    /// <summary>290 Effect: <paramref name="kind"/> is the high nibble of arg0.</summary>
    void Effect(int kind, int duration, int[] args);

    /// <summary>211 Reserve: preload a common event with its call arguments without calling it.</summary>
    void Reserve(int target, int[] args, string[] strings) { }

    /// <summary>201 SetMoveRoute: apply (or queue) a route on <paramref name="target"/>.</summary>
    void MoveRoute(int target, MoveRouteData route) { }

    /// <summary>270 Party: add/remove/reorder party members. Arguments stay raw words.</summary>
    void Party(int[] args) { }

    /// <summary>281 Scroll: camera scroll request. Arguments stay raw words.</summary>
    void Scroll(int[] args) { }

    /// <summary>151 ChangeColor: whole-screen colour multiplier, <paramref name="frames"/> ramp.</summary>
    void ChangeColor(int color, int frames) { }

    /// <summary>107 ClearDebugMessage.</summary>
    void ClearDebug() { }

    /// <summary>174 ReturnToTitle.</summary>
    void ReturnToTitle() { }

    /// <summary>175 EndGame.</summary>
    void EndGame() { }

    /// <summary>1000 ProFeature: engine-specific request, <paramref name="milliseconds"/> is the hint.</summary>
    void ProFeature(int kind, int milliseconds, string path) { ProFeature(kind, milliseconds); }

    /// <summary>Legacy two-argument form kept for hosts that only need the kind and timing.</summary>
    void ProFeature(int kind, int milliseconds) { }

    /// <summary>
    /// 123 key poll with the auto-input mask of opcode 125. A host that owns a real
    /// keyboard ORs <paramref name="autoKeys"/> into its held-key state; the default
    /// ignores it, like a headless host.
    /// </summary>
    int KeyCodeAuto(uint mask, uint autoKeys) => KeyCode(mask);

    /// <summary>A foreground event is about to execute its first slot.</summary>
    void EventEntered(int eventId, bool isRoot) { }

    /// <summary>WOLF opcode 122 &lt;&lt;GET_FILE_EXIST&gt;&gt; runtime existence check.</summary>
    bool FileExists(string path) => false;

    void Log(string message);
}

public class NullHost : IHost
{
    public virtual void Wait(uint frames) { }
    public virtual void ShowMessage(string text, int? face) { }
    public virtual void ShowMessage(string text, int? face, bool keepOpen) => ShowMessage(text, face);
    public virtual bool MessageDismissed() => true;
    public virtual void CloseMessage() { }
    public virtual void ShowChoices(string[] options) { }
    public virtual void ShowChoices(string[] options, bool allowCancel) => ShowChoices(options);
    public virtual ChoiceReply? ChoiceMade() => ChoiceReply.Select(0);
    public virtual void CommonEventCall(int target, int[] args, string[] strings) { }
    public virtual void Picture(PictureCmd cmd) { }
    public virtual void Sound(SoundCmd cmd) { }
    public virtual DbValue? Database(DbOp op) => null;
    public virtual void Teleport(int target, int map, int x, int y, int options) { }
    public virtual void Transition(int kind, int flags) { }
    public virtual int? SaveLoad(SaveOp op) => null;
    public virtual int? PictureState(int number, int field) => null;
    public virtual int? CharacterState(int target, int field) => null;
    public virtual int? EngineState(int field) => null;
    public virtual void MoveRoute(int target, MoveRouteData route) { }
    public virtual void Party(int[] args) { }
    public virtual void Scroll(int[] args) { }
    public virtual void ChangeColor(int color, int frames) { }
    public virtual void ClearDebug() { }
    public virtual void ReturnToTitle() { }
    public virtual void EndGame() { }
    public virtual void ProFeature(int kind, int milliseconds) { }
    public virtual void ProFeature(int kind, int milliseconds, string path) => ProFeature(kind, milliseconds);
    public virtual int KeyCodeAuto(uint mask, uint autoKeys) => KeyCode(mask);
    public virtual void EventEntered(int eventId, bool isRoot) { }
    public virtual bool FileExists(string path) => false;
    public virtual int KeyCode(uint mask) => 0;
    public virtual void Effect(int kind, int duration, int[] args) { }
    public virtual void Reserve(int target) { }
    public virtual void Reserve(int target, int[] args, string[] strings) => Reserve(target);
    public virtual void Log(string message) { }
}
