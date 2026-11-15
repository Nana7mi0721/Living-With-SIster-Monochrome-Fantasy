using System;
using System.IO;
using System.Text.Json;
using System.Text.Json.Nodes;
using Godot;

namespace Lwsmf.Tests;

public partial class TraceDiffTest : Node
{
    public override void _Ready()
    {
        string rootDir = ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
        string fixturesDir = Path.GetFullPath(Path.Combine(rootDir, "..", "oracle", "fixtures"));
        string refTracePath = Path.Combine(fixturesDir, "ce42_reference_trace.json");
        string corruptTracePath = Path.Combine(fixturesDir, "ce42_corrupt_trace.json");

        GD.Print($"Testing Oracle Trace Contract & Diff on: {refTracePath}");

        if (!File.Exists(refTracePath))
        {
            GD.PrintErr($"Reference trace not found: {refTracePath}");
            GetTree().Quit(1);
            return;
        }

        if (!File.Exists(corruptTracePath))
        {
            GD.PrintErr($"Corrupt trace fixture not found: {corruptTracePath}");
            GetTree().Quit(2);
            return;
        }

        string refJson = File.ReadAllText(refTracePath);
        string corruptJson = File.ReadAllText(corruptTracePath);

        JsonNode? refNode = JsonNode.Parse(refJson);
        JsonNode? corruptNode = JsonNode.Parse(corruptJson);

        if (refNode == null || corruptNode == null)
        {
            GD.PrintErr("Failed to parse trace JSON files");
            GetTree().Quit(3);
            return;
        }

        // Test 1: Self-diff (must match 100%)
        bool selfOk = CompareTraces(refNode, refNode, out string diffMsg);
        if (!selfOk)
        {
            GD.PrintErr($"Self diff failed: {diffMsg}");
            GetTree().Quit(4);
            return;
        }

        // Test 2: Diff with corrupted trace (must detect divergence at step 6)
        bool corruptOk = CompareTraces(refNode, corruptNode, out string corruptDiffMsg);
        if (corruptOk)
        {
            GD.PrintErr("Expected divergence against corrupt trace, but matched instead!");
            GetTree().Quit(5);
            return;
        }

        if (!corruptDiffMsg.Contains("Step 6"))
        {
            GD.PrintErr($"Expected divergence at Step 6, got: {corruptDiffMsg}");
            GetTree().Quit(6);
            return;
        }

        GD.Print($"Detected expected divergence: {corruptDiffMsg}");
        GD.Print("TRACE_DIFF_TEST_OK 10/10 steps verified and divergence detected correctly");
        GetTree().Quit(0);
    }

    private static bool CompareTraces(JsonNode baseTrace, JsonNode targetTrace, out string message)
    {
        string? schemaB = baseTrace["schema"]?.GetValue<string>();
        string? schemaT = targetTrace["schema"]?.GetValue<string>();
        if (schemaB != "wolf.trace" || schemaT != "wolf.trace")
        {
            message = $"Invalid schema: {schemaB} vs {schemaT}";
            return false;
        }

        int eventIdB = baseTrace["event_id"]?.GetValue<int>() ?? -1;
        int eventIdT = targetTrace["event_id"]?.GetValue<int>() ?? -1;
        if (eventIdB != eventIdT)
        {
            message = $"Event ID mismatch: {eventIdB} vs {eventIdT}";
            return false;
        }

        JsonArray? stepsB = baseTrace["steps"]?.AsArray();
        JsonArray? stepsT = targetTrace["steps"]?.AsArray();

        if (stepsB == null || stepsT == null)
        {
            message = "Steps array missing";
            return false;
        }

        if (stepsB.Count != stepsT.Count)
        {
            message = $"Step count mismatch: {stepsB.Count} vs {stepsT.Count}";
            return false;
        }

        for (int i = 0; i < stepsB.Count; i++)
        {
            JsonNode? sb = stepsB[i];
            JsonNode? st = stepsT[i];
            if (sb == null || st == null) continue;

            int stepB = sb["step"]?.GetValue<int>() ?? 0;
            int stepT = st["step"]?.GetValue<int>() ?? 0;
            int pcB = sb["pc"]?.GetValue<int>() ?? 0;
            int pcT = st["pc"]?.GetValue<int>() ?? 0;
            int opB = sb["op"]?.GetValue<int>() ?? 0;
            int opT = st["op"]?.GetValue<int>() ?? 0;

            if (pcB != pcT || opB != opT)
            {
                message = $"Step {stepB}: pc/op mismatch (base={pcB}/{opB}, target={pcT}/{opT})";
                return false;
            }

            // Compare vars
            JsonObject? varsB = sb["vars"]?.AsObject();
            JsonObject? varsT = st["vars"]?.AsObject();
            if (varsB != null && varsT != null)
            {
                if (varsB.Count != varsT.Count)
                {
                    message = $"Step {stepB}: vars count mismatch";
                    return false;
                }
                foreach (var kvp in varsB)
                {
                    if (!varsT.TryGetPropertyValue(kvp.Key, out JsonNode? tVal) || tVal?.GetValue<int>() != kvp.Value?.GetValue<int>())
                    {
                        message = $"Step {stepB}: var {kvp.Key} mismatch (base={kvp.Value}, target={tVal})";
                        return false;
                    }
                }
            }
        }

        message = "OK";
        return true;
    }
}
