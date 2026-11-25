using System;
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

    private static bool IsLeakedToiletEyePicture(SavedPicture? pic) =>
        pic != null
        && pic.Number == -6
        && pic.File.Contains("dlc_imouto01_eye", StringComparison.OrdinalIgnoreCase);

    /// <summary>True when picture -6 carries the toilet DLC eye without its parent CG body (#-8).</summary>
    public static bool ShouldStripLeakedToiletEye(IReadOnlyList<SavedPicture> pictures)
    {
        bool hasEye = false;
        bool hasBody = false;
        for (int i = 0; i < pictures.Count; i++)
        {
            var pic = pictures[i];
            if (pic == null) continue;
            if (pic.Number == -8 && pic.File.Contains("dlc_imouto01_", StringComparison.OrdinalIgnoreCase))
            {
                hasBody = true;
                break;
            }
            if (IsLeakedToiletEyePicture(pic))
            {
                hasEye = true;
            }
        }
        return hasEye && !hasBody;
    }

    /// <summary>Filter transient overlay slots and leaked retail toilet-eye (#-6) frames before display.</summary>
    public static List<SavedPicture> SanitizeRestoredPictures(IReadOnlyList<SavedPicture> pictures)
    {
        bool stripEye = ShouldStripLeakedToiletEye(pictures);
        var result = new List<SavedPicture>(pictures.Count);
        for (int i = 0; i < pictures.Count; i++)
        {
            var pic = pictures[i];
            if (pic == null || IsTransientUiPicture(pic.Number)) continue;
            if (stripEye && IsLeakedToiletEyePicture(pic)) continue;
            result.Add(pic);
        }
        return result;
    }

    /// <summary>
    /// Retail manual saves record var[302] == 1 (Save-UI lock) and retail AutoSaves record
    /// var[302] == 0 with var[68] == 1 &amp;&amp; var[9] == 1; CE131 pc=261 patches var[302] = 2 before
    /// loading so SampleMapA Ev3 ("ロード後の処理") clears stale state and restores bedroom clicks.
    /// Imported retail saves have no coroutine stack to resume Ev15 pc=18, so normalize var[302] = 2.
    /// </summary>
    public static bool NeedsRetailPostLoadTrigger(IReadOnlyList<int>? vars)
    {
        if (vars == null || vars.Count <= 302) return false;
        int v302 = vars[302];
        if (v302 == 1) return true;
        if (v302 == 0)
        {
            int v68 = vars.Count > 68 ? vars[68] : 0;
            int v9 = vars.Count > 9 ? vars[9] : 0;
            if (v68 != 0 && v9 != 0) return true;
        }
        return false;
    }

    /// <summary>
    /// Apply retail post-load var[302] = 2 trigger to a raw variable array and pre-clear
    /// var[68]/var[9] (which SampleMapA Ev3 p0 pc=24..25 unconditionally clears when v65 == 0)
    /// so MapRuntime's multi-autorun dispatch cannot start Ev4 p0 before Ev3 finishes.
    /// </summary>
    public static void ApplyRetailPostLoadVars(List<int>? vars)
    {
        if (vars == null || !NeedsRetailPostLoadTrigger(vars)) return;
        vars[302] = 2;
        int v65 = vars.Count > 65 ? vars[65] : 0;
        if (v65 == 0)
        {
            if (vars.Count > 68) vars[68] = 0;
            if (vars.Count > 9) vars[9] = 0;
        }
    }

    /// <summary>Apply retail post-load var[302] = 2 trigger when loading a coroutine-less SampleMapA save.</summary>
    public static void ApplyRetailPostLoadTriggerIfNeeded(SaveState state)
    {
        bool isSampleMapA = state.MapId == 1005
            || string.Equals(state.MapFile, "SampleMapA.mps", StringComparison.OrdinalIgnoreCase);
        if (!isSampleMapA || state.Instances.Count > 0) return;
        ApplyRetailPostLoadVars(state.Vars);
    }

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
        if (ShouldStripLeakedToiletEye(state.Pictures))
        {
            state.Pictures.RemoveAll(IsLeakedToiletEyePicture);
        }
        state.Audio.RemoveAll(a => a != null && a.Channel == 2); // CHANNEL_SE is transient.
        return state;
    }
}
