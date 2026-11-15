using System;
using System.Collections.Generic;
using System.Linq;

namespace Lwsmf.Core;

/// <summary>
/// Host that records every side effect instead of presenting it. Headless tests read the
/// recorded list; the VM keeps running as if a user dismissed every message at once.
/// </summary>
public class RecordingHost : NullHost
{
    public readonly List<PictureCmd> Pictures = new();
    public readonly List<SoundCmd> Sounds = new();
    public readonly List<EffectCall> Effects = new();
    public readonly List<CallRecord> Calls = new();
    public readonly List<TeleportRecord> Teleports = new();
    public readonly List<string> Logs = new();
    public readonly List<int[]> PartyCalls = new();
    public readonly List<(int Target, MoveRouteData Route)> MoveRoutes = new();
    public readonly List<int[]> Scrolls = new();
    public readonly List<(int Color, int Frames)> Colors = new();
    public readonly List<(int Kind, int Milliseconds)> ProFeatures = new();
    public int ClearDebugCount;
    public int ReturnToTitleCount;
    public int EndGameCount;
    public uint LastKeyMask;
    public uint LastAutoKeys;

    public int CurrentPc;
    public ulong Frame;

    public readonly struct EffectCall
    {
        public readonly int Kind;
        public readonly int Duration;
        public readonly int[] Args;
        public EffectCall(int kind, int duration, int[] args)
        {
            Kind = kind;
            Duration = duration;
            Args = args;
        }
        public override string ToString() => $"effect kind={Kind} duration={Duration} args=[{string.Join(",", Args)}]";
    }

    public readonly struct CallRecord
    {
        public readonly int Target;
        public readonly int[] Args;
        public readonly string[] Strings;
        public CallRecord(int target, int[] args, string[] strings)
        {
            Target = target;
            Args = args;
            Strings = strings;
        }
        public override string ToString() => $"call target={Target} args=[{string.Join(",", Args)}]";
    }

    public readonly struct TeleportRecord
    {
        public readonly int Target;
        public readonly int Map;
        public readonly int X;
        public readonly int Y;
        public readonly int Options;
        public TeleportRecord(int target, int map, int x, int y, int options)
        {
            Target = target;
            Map = map;
            X = x;
            Y = y;
            Options = options;
        }
        public override string ToString() => $"teleport target={Target} map={Map} x={X} y={Y} options={Options}";
    }

    public override void Picture(PictureCmd cmd) => Pictures.Add(cmd);
    public override void Sound(SoundCmd cmd) => Sounds.Add(cmd);
    public override void Effect(int kind, int duration, int[] args) => Effects.Add(new EffectCall(kind, duration, args));
    public override void CommonEventCall(int target, int[] args, string[] strings) => Calls.Add(new CallRecord(target, args, strings));
    public override void Teleport(int target, int map, int x, int y, int options) => Teleports.Add(new TeleportRecord(target, map, x, y, options));
    public override void Log(string message) => Logs.Add(message);

    public override void MoveRoute(int target, MoveRouteData route) => MoveRoutes.Add((target, route));
    public override void Party(int[] args) => PartyCalls.Add(args);
    public override void Scroll(int[] args) => Scrolls.Add(args);
    public override void ChangeColor(int color, int frames) => Colors.Add((color, frames));
    public override void ClearDebug() => ClearDebugCount++;
    public override void ReturnToTitle() => ReturnToTitleCount++;
    public override void EndGame() => EndGameCount++;
    public override void ProFeature(int kind, int milliseconds) => ProFeatures.Add((kind, milliseconds));

    public override int KeyCodeAuto(uint mask, uint autoKeys)
    {
        LastKeyMask = mask;
        LastAutoKeys = autoKeys;
        return KeyCode(mask);
    }

    public bool Drew(string file) => Pictures.Any(p => p.File == file);
    public bool Played(string file) => Sounds.Any(s => s.File == file);
    public bool Called(int target) => Calls.Any(c => c.Target == target);

    public IEnumerable<string> Describe()
    {
        foreach (var p in Pictures) yield return $"picture slot={p.Number} action={p.Action} file={p.File}";
        foreach (var s in Sounds) yield return $"sound {s.Channel} {s.Action} file={s.File}";
        foreach (var e in Effects) yield return e.ToString();
        foreach (var c in Calls) yield return c.ToString();
        foreach (var t in Teleports) yield return t.ToString();
        foreach (var l in Logs) yield return "log " + l;
    }
}
