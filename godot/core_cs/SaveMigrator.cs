using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>
/// Single normalization and schema-migration pipeline invoked after decoding any SaveState
/// (main slot, .bak backup, or legacy import cleanup).
/// </summary>
public static class SaveMigrator
{
    public const int UiPictureFirst = 12000;
    public const int UiPictureLast = 12599;
    public const int UniversalWindowFirst = 15000;
    public const int UniversalWindowLast = 15999;
    public const int PopupWindowFirst = 16000;
    public const int PopupWindowLast = 16999;

    /// <summary>True for transient overlay picture slots that must never survive save/load.</summary>
    public static bool IsTransientUiPicture(int number) =>
        (number >= UiPictureFirst && number <= UiPictureLast)
        || (number >= UniversalWindowFirst && number <= UniversalWindowLast)
        || (number >= PopupWindowFirst && number <= PopupWindowLast);

    /// <summary>
    /// Normalize a decoded save payload in place so legacy/older-build residues (transient UI
    /// boxes, negative counters, null collections, transient SE audio) are repaired in one place.
    /// </summary>
    public static SaveState Normalize(SaveState state)
    {
        if (state.SchemaVersion < 1)
        {
            state.SchemaVersion = SaveState.SchemaVersionNumber;
        }
        if (state.SchemaVersion == 1)
        {
            state.SchemaVersion = SaveState.SchemaVersionNumber;
            state.DbOverlays ??= new SavedDbOverlays();
        }
        if (state.PlayFrames < 0) state.PlayFrames = 0;
        if (state.MapId < 0) state.MapId = 0;

        state.Vars ??= new List<int>();
        state.StrVars ??= new List<string>();
        state.SysVars ??= new List<int>();
        state.SysStrVars ??= new List<string>();
        state.SelfVars ??= new List<EventSelfVars>();
        state.CommonSelf ??= new List<EventSelfVars>();
        state.MapEvents ??= new List<SavedMapEvent>();
        state.Instances ??= new List<SavedInstance>();
        state.Pictures ??= new List<SavedPicture>();
        state.Audio ??= new List<SavedAudio>();
        if (state.DbOverlays != null)
        {
            state.DbOverlays.RowCounts ??= new List<SavedDbRowCount>();
            state.DbOverlays.Nums ??= new List<SavedDbNumCell>();
            state.DbOverlays.Texts ??= new List<SavedDbTextCell>();
        }

        state.Pictures.RemoveAll(p => p != null && IsTransientUiPicture(p.Number));
        state.Audio.RemoveAll(a => a != null && a.Channel == 2); // CHANNEL_SE is transient.
        return state;
    }
}
