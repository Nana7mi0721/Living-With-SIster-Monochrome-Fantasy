using System;

namespace Lwsmf.Core;

/// <summary>Process-scoped debug and RNG environment flags cached once at startup.</summary>
public static class CoreRuntimeConfig
{
    /// <summary>True when LWsmf_TRACE_AUTO=1 enables auto-event and CommonSelf trace output.</summary>
    public static bool TraceAutoEvents { get; } =
        string.Equals(Environment.GetEnvironmentVariable("LWsmf_TRACE_AUTO"), "1", StringComparison.Ordinal);

    /// <summary>True when WOLF_LIVE_RNG is set to any value other than "0".</summary>
    public static bool LiveRng { get; } = ReadLiveRng();

    private static bool ReadLiveRng()
    {
        string? live = Environment.GetEnvironmentVariable("WOLF_LIVE_RNG");
        return !string.IsNullOrEmpty(live) && live != "0";
    }
}
