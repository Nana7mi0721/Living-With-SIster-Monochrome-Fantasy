using System;
using System.Collections.Generic;
using System.Text.Json.Nodes;

namespace Lwsmf.Core;

public sealed class TraceStep
{
    public int Step { get; set; }
    public ulong Frame { get; set; }
    public int EventId { get; set; }
    public int Pc { get; set; }
    public int Op { get; set; }
    public Dictionary<int, int> Vars { get; } = new();
    public Dictionary<int, string> StrVars { get; } = new();
    public List<JsonObject> Effects { get; } = new();
}

public sealed class TraceRecorderHost : IHost
{
    public List<JsonObject> PendingEffects { get; } = new();
    public ChoiceReply CurrentChoiceReply { get; set; } = ChoiceReply.Select(0);

    /// <summary>Optional deterministic menu driver; null keeps the fixed reply (oracle mode).</summary>
    public Func<string[], ChoiceReply>? ChoiceSelector { get; set; }

    /// <summary>Optional database provider for battle/event tests that need real CData reads.</summary>
    public Func<DbOp, DbValue?>? DatabaseProvider { get; set; }

    // Waiting is scheduling, not an effect: the Rust trace does not record it, so
    // neither does this host (the two step traces must agree literally).
    public void Wait(uint frames) { }

    public void Picture(PictureCmd cmd)
    {
        var argsArr = new JsonArray();
        foreach (int a in cmd.Args)
        {
            argsArr.Add(a);
        }

        string actionStr = cmd.Action switch
        {
            PictureAction.Draw => "draw",
            PictureAction.Move => "move",
            PictureAction.Erase => "erase",
            PictureAction.Hide => "hide",
            _ => "set"
        };

        var eff = new JsonObject
        {
            ["kind"] = "picture",
            ["action"] = actionStr,
            ["slot"] = cmd.Number,
            ["file"] = cmd.File,
            ["args"] = argsArr
        };
        PendingEffects.Add(eff);
    }

    public void Sound(SoundCmd cmd)
    {
        var eff = new JsonObject
        {
            ["kind"] = "sound",
            ["action"] = cmd.Action == SoundAction.Play ? "play" : "stop",
            ["type"] = cmd.Channel switch
            {
                SoundChannel.Bgm => "bgm",
                SoundChannel.Bgs => "bgs",
                _ => "se"
            },
            ["file"] = cmd.File,
            ["vol"] = cmd.Volume,
            ["frames"] = cmd.Frames
        };
        PendingEffects.Add(eff);
    }

    public void ShowChoices(string[] options)
    {
        if (ChoiceSelector != null) CurrentChoiceReply = ChoiceSelector(options);
        var optsArr = new JsonArray();
        foreach (var opt in options)
        {
            optsArr.Add(opt);
        }

        var eff = new JsonObject
        {
            ["kind"] = "choice",
            ["options"] = optsArr,
            ["selected"] = CurrentChoiceReply.IsCancel ? -1 : CurrentChoiceReply.Index
        };
        PendingEffects.Add(eff);
    }

    public ChoiceReply? ChoiceMade() => CurrentChoiceReply;

    public void Teleport(int target, int map, int x, int y, int options)
    {
        var eff = new JsonObject
        {
            ["kind"] = "teleport",
            ["map_id"] = map,
            ["x"] = x,
            ["y"] = y,
            ["direction"] = options
        };
        PendingEffects.Add(eff);
    }

    public DbValue? Database(DbOp op)
    {
        var eff = new JsonObject
        {
            ["kind"] = "database",
            ["type"] = op.DbType,
            ["data"] = op.DataId,
            ["item"] = op.ItemId,
            ["value"] = op.Source?.Num ?? 0
        };
        PendingEffects.Add(eff);
        return DatabaseProvider?.Invoke(op);
    }

    public void CommonEventCall(int target, int[] args, string[] strings)
    {
        var argsArr = new JsonArray();
        foreach (int a in args)
        {
            argsArr.Add(a);
        }

        var eff = new JsonObject
        {
            ["kind"] = "call",
            ["target_event_id"] = target,
            ["args"] = argsArr
        };
        PendingEffects.Add(eff);
    }

    public void Effect(int kind, int duration, int[] args)
    {
        var argsArr = new JsonArray();
        foreach (int a in args)
        {
            argsArr.Add(a);
        }

        var eff = new JsonObject
        {
            ["kind"] = "effect",
            ["type"] = kind,
            ["duration"] = duration,
            ["args"] = argsArr
        };
        PendingEffects.Add(eff);
    }

    public void Reserve(int target) { }

    public void ShowMessage(string text, int? face) { }
    public bool MessageDismissed() => true;
    public void CloseMessage() { }
    public void Transition(int kind, int flags) { }
    public int? SaveLoad(SaveOp op) => null;
    public int? PictureState(int number, int field) => null;
    public int? CharacterState(int target, int field) => null;
    public int? EngineState(int field) => null;
    public int KeyCode(uint mask) => 0;
    public void Log(string message) { }
}
