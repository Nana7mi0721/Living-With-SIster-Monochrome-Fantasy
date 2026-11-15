using System;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>Builds the production core stack (VmController + GameSession) for headless tests.</summary>
public sealed class SessionHarness
{
    public VmController Vm { get; }
    public GameSession Session { get; }
    public TestSessionView View { get; }
    public string DataPath { get; }
    public string SavePath { get; }

    public SessionHarness(Node root, string dataPath, string savePath, TestSessionView? view = null)
    {
        DataPath = dataPath;
        SavePath = savePath;
        View = view ?? new TestSessionView();

        Vm = new VmController { Name = "TestVm", KeyCodeProvider = _ => KeyCodes.CodeOk };
        root.AddChild(Vm);
        Vm.Vm.StrictMode = true;
        if (!Vm.LoadPrograms(dataPath)) throw new InvalidOperationException("programs.json failed to load: " + dataPath);

        Session = new GameSession(Vm, View, dataPath, savePath)
        {
            ScriptedInput = true,
            AutoPickChoice = true
        };
        Vm.MessageRequested += (text, face) => Session.ShowMessage(text, face);
        Vm.MessageClosed += () => Session.NotifyMessageClosed();
        Vm.ChoicesRequested += options => Session.ShowChoices(options);
        Vm.TeleportRequested += (target, map, x, y, options) => Session.QueueTeleport(target, map, x, y, options);

        if (!Session.StartTitle()) throw new InvalidOperationException("title event failed to start");
    }

    public bool RunToBedroom(int maxFrames = 8000)
    {
        int guard = 0;
        while (Session.Phase != "bedroom" && Session.Phase != "failed" && guard++ < maxFrames)
        {
            Session.AdvanceFrame();
        }
        return Session.Phase == "bedroom";
    }
}
