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

    public static string ResourceRoot()
    {
        if (IsAndroid)
            return "res://";
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
        return Path.GetFullPath(Path.Combine(ResourceRoot(), ".."));
    }

    public static string GameDataDir()
    {
        return Path.Combine(PackageRoot(), "game_data");
    }
}
