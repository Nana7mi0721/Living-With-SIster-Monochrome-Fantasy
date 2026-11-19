using System;
using System.Collections.Generic;
using Godot;

namespace Lwsmf.Core;

public class GDScriptHost : IHost
{
    public void Wait(uint frames) => GD.Print($"Host.Wait({frames})");
    public void ShowMessage(string text, int? face) => GD.Print($"Host.ShowMessage({text}, face={face})");
    public bool MessageDismissed() => true;
    public void CloseMessage() => GD.Print("Host.CloseMessage()");
    public void ShowChoices(string[] options) => GD.Print($"Host.ShowChoices([{string.Join(",", options)}])");
    public ChoiceReply? ChoiceMade() => null;
    public void CommonEventCall(int target, int[] args, string[] strings) => GD.Print($"Host.CommonEventCall({target})");
    public void Picture(PictureCmd cmd) => GD.Print($"Host.Picture({cmd.File}, {cmd.Action})");
    public void Sound(SoundCmd cmd) => GD.Print($"Host.Sound({cmd.File}, {cmd.Action})");
    public DbValue? Database(DbOp op) => null;
    public void Teleport(int target, int map, int x, int y, int options) => GD.Print($"Host.Teleport(map={map}, pos={x},{y})");
    public void Transition(int kind, int flags) => GD.Print($"Host.Transition({kind})");
    public int? SaveLoad(SaveOp op) => null;
    public int? PictureState(int number, int field) => null;
    public int? CharacterState(int target, int field) => null;
    public int? EngineState(int field) => 0;
    public int KeyCode(uint mask) => 0;
    public void Effect(int kind, int duration, int[] args) => GD.Print($"Host.Effect(kind={kind}, duration={duration})");
    public void Reserve(int target) => GD.Print($"Host.Reserve({target})");
    public void Reserve(int target, int[] args, string[] strings) => Reserve(target);
    public void Log(string message) => GD.Print($"[Log] {message}");
}
