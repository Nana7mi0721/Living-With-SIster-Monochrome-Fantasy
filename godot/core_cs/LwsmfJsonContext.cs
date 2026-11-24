using System.Collections.Generic;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Lwsmf.Core;

[JsonSerializable(typeof(SaveState))]
[JsonSerializable(typeof(IReadOnlyList<SavedPicture>))]
[JsonSerializable(typeof(List<SavedPicture>))]
[JsonSerializable(typeof(IReadOnlyList<SavedAudio>))]
[JsonSerializable(typeof(List<SavedAudio>))]
[JsonSerializable(typeof(GameDatabase.RootDto))]
[JsonSerializable(typeof(LegacySaveImporter.ImportReport))]
[JsonSerializable(typeof(List<string>))]
[JsonSerializable(typeof(string))]
internal sealed partial class LwsmfJsonContext : JsonSerializerContext
{
    internal static LwsmfJsonContext SaveIndented { get; } = new(new JsonSerializerOptions
    {
        WriteIndented = true,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    });

    internal static LwsmfJsonContext Indented { get; } = new(new JsonSerializerOptions
    {
        WriteIndented = true
    });
}
