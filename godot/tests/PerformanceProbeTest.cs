using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using Godot;
using Lwsmf.Core;
using Lwsmf.Presentation;

namespace Lwsmf.Tests;

/// <summary>
/// M5-C performance baseline probe (core side).
///
/// Runs the production core stack through SessionHarness: TitleMap event 2 -> new game
/// -> SampleMapA (artifact 1005) bedroom. Measures the startup wall clock, then the
/// per-frame cost of 600 subsequent deterministic frames. Structural assertions are
/// always enforced (600 frames, map 1005, no Faulted/unhandled opcode). Time thresholds
/// are opt-in through LWsmf_PERF_STRICT=1 so slow CI machines do not fail the probe.
/// </summary>
public partial class PerformanceProbeTest : Node
{
    private const int MeasureFrames = 600;

    public override void _Ready()
    {
        string repo = Path.GetFullPath(Path.Combine(
            ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\'), ".."));
        string dataPath = Path.Combine(repo, "game_data", "programs.json");
        string savePath = Path.Combine(repo, "work", "out", "performance_probe_save.json");
        string reportPath = Path.Combine(repo, "work", "out", "performance_core_report.json");
        var failures = new List<string>();

        try
        {
            string? reportDir = Path.GetDirectoryName(reportPath);
            if (!string.IsNullOrEmpty(reportDir)) Directory.CreateDirectory(reportDir);

            // ---- startup: construct the real title->newgame->bedroom chain ----
            long startupStart = Stopwatch.GetTimestamp();
            var harness = new SessionHarness(this, dataPath, savePath);
            bool reachedBedroom = harness.RunToBedroom(8000);
            double startupMs = ElapsedMs(startupStart);

            if (!reachedBedroom)
                failures.Add($"bedroom not reached: phase={harness.Session.Phase} frames={harness.Session.Frames}");
            int startupFrames = harness.Session.Frames;
            int startupMap = harness.Session.MapData?.Id ?? -1;
            if (startupMap != 1005)
                failures.Add($"startup map={startupMap}, expected 1005");
            if (harness.Vm.Vm.Faulted)
                failures.Add($"faulted during startup: {harness.Vm.Vm.FaultReason}");
            if (harness.Vm.Vm.UnhandledOpcodeCount != 0)
                failures.Add($"unhandled opcodes during startup: {harness.Vm.Vm.UnhandledOpcodeCount}");

            // ---- 600-frame pacing: measure every frame boundary ----
            int framesAtStart = harness.Session.Frames;
            var samples = new double[MeasureFrames];
            long allocationsBefore = GC.GetTotalAllocatedBytes(true);
            long frameStart = Stopwatch.GetTimestamp();
            for (int i = 0; i < MeasureFrames; i++)
            {
                harness.Session.AdvanceFrame();
                long now = Stopwatch.GetTimestamp();
                samples[i] = (now - frameStart) * 1000.0 / Stopwatch.Frequency;
                frameStart = now;
            }
            long allocationsAfter = GC.GetTotalAllocatedBytes(true);

            int measuredFrames = samples.Length;
            int measuredMap = harness.Session.MapData?.Id ?? -1;
            int sessionFrames = harness.Session.Frames;

            // ---- structural assertions (always) ----
            if (measuredFrames != MeasureFrames)
                failures.Add($"measured frames={measuredFrames}, expected {MeasureFrames}");
            if (sessionFrames != framesAtStart + MeasureFrames)
                failures.Add($"session frames={sessionFrames}, expected {framesAtStart + MeasureFrames}");
            if (measuredMap != 1005)
                failures.Add($"post-frame map={measuredMap}, expected 1005");
            if (harness.Vm.Vm.Faulted)
                failures.Add($"faulted during 600 frames: {harness.Vm.Vm.FaultReason}");
            if (harness.Vm.Vm.UnhandledOpcodeCount != 0)
                failures.Add($"unhandled opcodes during 600 frames: {harness.Vm.Vm.UnhandledOpcodeCount}");

            // ---- statistics ----
            Array.Sort(samples);
            double avgMs = samples.Average();
            double p50Ms = Percentile(samples, 50);
            double p95Ms = Percentile(samples, 95);
            double maxMs = samples[samples.Length - 1];
            double gcMb = Math.Max(0, allocationsAfter - allocationsBefore) / (1024.0 * 1024.0);
            double staticMb = Convert.ToDouble(OS.GetStaticMemoryUsage()) / (1024.0 * 1024.0);
            double staticPeakMb = Convert.ToDouble(OS.GetStaticMemoryPeakUsage()) / (1024.0 * 1024.0);
            bool strict = string.Equals(System.Environment.GetEnvironmentVariable("LWsmf_PERF_STRICT"), "1", StringComparison.Ordinal);

            GD.Print($"[PerformanceProbeTest] startup_frames={startupFrames} session_frames={sessionFrames} "
                + $"static_peak_mb={staticPeakMb.ToString("F3", CultureInfo.InvariantCulture)} strict={strict}");

            if (strict)
            {
                if (avgMs > 16.7) failures.Add($"strict avg_ms={avgMs:F3} > 16.7");
                if (p95Ms > 33.4) failures.Add($"strict p95_ms={p95Ms:F3} > 33.4");
                if (staticMb > 512.0) failures.Add($"strict static_mb={staticMb:F3} > 512");
            }

            WriteReport(reportPath, new
            {
                schema = "lwsmf.performance.core",
                schema_version = 1,
                startup_ms = Round(startupMs),
                startup_frames = startupFrames,
                frames = measuredFrames,
                session_frames = sessionFrames,
                map = measuredMap,
                avg_ms = Round(avgMs),
                p50_ms = Round(p50Ms),
                p95_ms = Round(p95Ms),
                max_ms = Round(maxMs),
                gc_mb = Round(gcMb),
                static_mb = Round(staticMb),
                static_peak_mb = Round(staticPeakMb),
                strict,
                frame_ms = samples.Select(Round).ToArray(),
            });

            if (failures.Count > 0)
            {
                GD.PrintErr($"[PerformanceProbeTest] PERFORMANCE_CORE_FAILED count={failures.Count}");
                foreach (string failure in failures) GD.PrintErr("  - " + failure);
                GetTree().Quit(1);
                return;
            }

            GD.Print(string.Format(CultureInfo.InvariantCulture,
                "PERFORMANCE_CORE_OK startup_ms={0:F3} frames={1} avg_ms={2:F4} p50_ms={3:F4} "
                + "p95_ms={4:F4} max_ms={5:F4} gc_mb={6:F3} static_mb={7:F3}",
                startupMs, measuredFrames, avgMs, p50Ms, p95Ms, maxMs, gcMb, staticMb));
            GetTree().Quit();
        }
        catch (Exception ex)
        {
            GD.PrintErr($"[PerformanceProbeTest] PERFORMANCE_CORE_FAILED exception={ex.GetType().Name}: {ex.Message}");
            GD.PrintErr(ex.StackTrace);
            GetTree().Quit(1);
        }
    }

    private static double ElapsedMs(long startTimestamp)
        => (Stopwatch.GetTimestamp() - startTimestamp) * 1000.0 / Stopwatch.Frequency;

    private static double Percentile(double[] sorted, double percentile)
    {
        if (sorted.Length == 0) return 0.0;
        double rank = (percentile / 100.0) * (sorted.Length - 1);
        int low = (int)Math.Floor(rank);
        int high = (int)Math.Ceiling(rank);
        if (low == high) return sorted[low];
        return sorted[low] + (sorted[high] - sorted[low]) * (rank - low);
    }

    private static double Round(double value) => Math.Round(value, 4, MidpointRounding.AwayFromZero);

    private static void WriteReport(string path, object payload)
    {
        string json = System.Text.Json.JsonSerializer.Serialize(payload, new System.Text.Json.JsonSerializerOptions
        {
            WriteIndented = true,
        });
        File.WriteAllText(path, json + System.Environment.NewLine, new UTF8Encoding(false));
    }
}
