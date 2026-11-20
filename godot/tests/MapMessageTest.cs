using System;
using System.Linq;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// Drives the core session into SampleMapA (artifact 1005), arms event 4 page 4 (parallel
/// 会話メニュー処理: 63=1, 62=0, 464=0, 60=0) and checks that a map event page really opens
/// the message/choice windows through the GameSession view.
/// </summary>
public partial class MapMessageTest : Node
{
    private SessionHarness _harness = null!;
    private int _failures;
    private int _checks;

    public override void _Ready()
    {
        string dataPath = System.IO.Path.Combine(
            ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\'), "..", "game_data", "programs.json");
        string savePath = System.IO.Path.Combine(
            System.IO.Path.GetTempPath(), "lwsmf-mapmsg-" + Guid.NewGuid().ToString("N") + ".json");
        try
        {
            _harness = new SessionHarness(this, dataPath, savePath);
        }
        catch (Exception ex)
        {
            Check(false, "core session setup failed: " + ex.Message);
            Finish(0);
            return;
        }

        CallDeferred(nameof(Run));
    }

    private void Check(bool ok, string label)
    {
        _checks++;
        if (ok) return;
        _failures++;
        GD.PrintErr($"[MapMessageTest] FAIL {label}");
    }

    private void Run()
    {
        GameSession session = _harness.Session;
        VmController vm = _harness.Vm;
        TestSessionView view = _harness.View;

        int frames = 0;
        while (session.Phase != "bedroom" && frames < 4000)
        {
            session.AdvanceFrame();
            frames++;
        }
        Check(session.Phase == "bedroom", $"reached bedroom (phase={session.Phase} frames={frames})");
        if (session.Phase != "bedroom")
        {
            Finish(frames);
            return;
        }

        // Stop auto-dismissing so the first map-authored message stays on screen.
        session.ScriptedInput = false;

        // SampleMapA event 4 page 4 gate: 2000063=1, 2000062=0, 2000464=0, 2000060=0.
        vm.SetVar(63, 1);
        vm.SetVar(62, 0);
        vm.SetVar(464, 0);
        vm.SetVar(60, 0);

        int wait = 0;
        while (wait < 3000 && !vm.IsWaitingForMessage() && !view.HasChoices)
        {
            session.AdvanceFrame();
            wait++;
        }

        Check(vm.IsWaitingForMessage() || view.HasChoices,
            $"map event opened a message/choice (waitingMessage={vm.IsWaitingForMessage()} choices={view.HasChoices})");
        Check(view.HasMessage || view.HasChoices,
            $"session view shows the window (hasMessage={view.HasMessage} hasChoices={view.HasChoices})");
        if (view.HasMessage)
        {
            Check(view.MessageText.Length > 0, "message text is not empty");
            Check(!view.MessageText.Contains("\\E"), "message control codes are stripped");
        }

        // \cdb/\udb are database cell reads, not colours (docs/engine/TEXT_SPEC.md
        // section 2.7). The probe callback pins the parse; the live VM pins the host hook.
        Instance? probeInst = vm.Vm.Instances.FirstOrDefault();
        Check(probeInst != null, "a live instance exists for the expander probe");
        if (probeInst != null)
        {
            vm.SetVar(160, 7);
            string probe = TextExpander.Expand("\\udb[28:\\v[160]:0]", probeInst, vm.Vm.Vars,
                (kind, type, row, column) => $"{kind}/{type}/{row}/{column}");
            Check(probe == "2/28/7/0", $"nested db escape resolves (got '{probe}')");
            string cdb = TextExpander.Expand("\\cdb[0:12:8]", probeInst, vm.Vm.Vars,
                (kind, type, row, column) => $"{kind}/{type}/{row}/{column}");
            Check(cdb == "0/0/12/8", $"cdb escape resolves (got '{cdb}')");
            string live = vm.Vm.Expand("\\cdb[0:12:8]", probeInst);
            Check(!live.Contains("\\cdb"), $"live expander consumes cdb (got '{live}')");
        }

        Check(session.MapRuntime != null, "map runtime exists");
        var event4 = session.MapRuntime?.Events.FirstOrDefault(e => e.Def.Id == 4 && e.InstanceId > 0);
        Check(event4 != null, "event 4 is in the runtime");
        Check(event4?.Active == 4, $"event 4 selected page 4 (active={event4?.Active})");
        Check(event4?.InstanceId != null, "event 4 page instance is live");

        // Walk the dialogue the page started (it calls a common event first): dismiss messages
        // until the page's own choice menu appears, which proves choices also cross the view bridge.
        bool sawChoice = false;
        string[] labels = Array.Empty<string>();
        int steps = 0;
        while (steps < 1500 && !sawChoice)
        {
            if (view.HasChoices && vm.GetCurrentChoices().Length > 0)
            {
                sawChoice = true;
                labels = vm.GetCurrentChoices();
                break;
            }
            if (vm.IsWaitingForMessage()) session.DismissMessage();
            session.AdvanceFrame();
            steps++;
        }
        Check(sawChoice, $"map event reached a choice menu (steps={steps})");
        Check(labels.Length > 0, $"choice menu has options (count={labels.Length})");
        if (sawChoice)
        {
            Check(view.HasChoices, "session view shows the choice box");
            vm.SelectChoice(0);
        }

        GD.Print($"[MapMessageTest] frames={frames} wait={wait} message='{Trim(view.MessageText)}' " +
                 $"choiceSteps={steps} choices=[{string.Join(" | ", labels)}] event4Instance={event4?.InstanceId}");
        Finish(frames);
    }

    private static string Trim(string text)
    {
        string flat = text.Replace("\n", " / ");
        return flat.Length <= 60 ? flat : flat.Substring(0, 60) + "…";
    }

    private void Finish(int frames)
    {
        Check(_harness.Vm.Vm.UnhandledOpcodeCount == 0,
            $"chain saw {_harness.Vm.Vm.UnhandledOpcodeCount} unhandled opcodes");
        Check(!_harness.Vm.Vm.Faulted, "chain faulted the VM (" + _harness.Vm.Vm.FaultReason + ")");
        if (_failures == 0)
        {
            GD.Print($"MAP_MESSAGE_TEST_OK {_checks}/{_checks} frames={frames}");
        }
        else
        {
            GD.Print($"MAP_MESSAGE_TEST_FAIL {_checks - _failures}/{_checks}");
        }
        GetTree().Quit();
    }
}
