using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>Origin of the save payload examined by <see cref="SaveAuditReport"/>.</summary>
public enum SaveAuditSource
{
    None = 0,
    NativeMain = 1,
    NativeBackupFallback = 2,
    RetailImport = 3,
}

/// <summary>
/// Read-only diagnostic summary of normalization, schema migration, encoding warnings,
/// and non-mutating lock checks observed during a save load or retail import.
/// </summary>
public sealed class SaveAuditReport
{
    public SaveAuditSource SourceKind { get; set; } = SaveAuditSource.None;
    public int? MigratedFromSchema { get; set; }
    public int StrippedTransientPictures { get; set; }
    public bool StrippedLeakedToiletEye { get; set; }
    public int StrippedTransientSe { get; set; }
    public bool RetailPostLoadTriggerApplied { get; set; }
    public List<string> EncodingWarnings { get; } = new();
    public List<string> SuspiciousLocks { get; } = new();

    /// <summary>Format a single-line summary suitable for diagnostic snapshots.</summary>
    public string FormatSummary()
    {
        string schema = MigratedFromSchema.HasValue
            ? $"{MigratedFromSchema.Value}->{SaveState.SchemaVersionNumber}"
            : "current";
        string locks = SuspiciousLocks.Count > 0 ? string.Join(", ", SuspiciousLocks) : "none";
        string summary =
            $"source={SourceKind} schema={schema} " +
            $"stripped_pics={StrippedTransientPictures} stripped_eye={(StrippedLeakedToiletEye ? "true" : "false")} " +
            $"stripped_se={StrippedTransientSe} post_load_trigger={(RetailPostLoadTriggerApplied ? "true" : "false")} " +
            $"encoding_warnings={EncodingWarnings.Count} locks=[{locks}]";
        if (EncodingWarnings.Count > 0)
        {
            summary += $" warnings=[{string.Join("; ", EncodingWarnings)}]";
        }
        return summary;
    }
}
