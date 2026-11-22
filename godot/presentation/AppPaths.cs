using System.IO;
using Godot;

namespace Lwsmf.Presentation;

/// <summary>
/// Absolute filesystem roots shared by editor and exported builds.
/// res:// is a real project directory in the editor, but a virtual PCK prefix in
/// exported builds; package data lives beside the executable there.
/// </summary>
public static class AppPaths
{
    public static bool IsAndroid => OS.HasFeature("android");
    public static bool IsIos => OS.HasFeature("ios");

    public static string GodotResourceRoot() => IsAndroid ? "res://" : ResourceRoot();

    public static string ResourceRoot()
    {
        if (IsAndroid)
            return PackageRoot();
        if (OS.HasFeature("template"))
        {
            return Path.GetDirectoryName(OS.GetExecutablePath()) ?? "";
        }
        return ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
    }

    public static string PackageRoot()
    {
        if (IsAndroid)
            return ProjectSettings.GlobalizePath("user://mobile_data");
        // iOS bundles inject game_data/ and work/all/ directly inside Payload/<App>.app/.
        if (IsIos && OS.HasFeature("template"))
            return ResourceRoot();
        return Path.GetFullPath(Path.Combine(ResourceRoot(), ".."));
    }

    public static string GameDataDir()
    {
        return Path.Combine(PackageRoot(), "game_data");
    }
}
