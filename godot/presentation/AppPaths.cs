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
    public static string ResourceRoot()
    {
        if (OS.HasFeature("template"))
        {
            return Path.GetDirectoryName(OS.GetExecutablePath()) ?? "";
        }
        return ProjectSettings.GlobalizePath("res://").TrimEnd('/', '\\');
    }

    public static string PackageRoot()
    {
        return Path.GetFullPath(Path.Combine(ResourceRoot(), ".."));
    }

    public static string GameDataDir()
    {
        return Path.Combine(PackageRoot(), "game_data");
    }
}
