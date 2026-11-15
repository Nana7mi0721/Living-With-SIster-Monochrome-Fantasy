using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using Lwsmf.Core;

namespace Lwsmf.Tests;

/// <summary>Headless IGameSessionView used by the core session tests after the legacy screen retired.</summary>
public sealed class TestSessionView : IGameSessionView
{
    public string Status { get; private set; } = "";
    public string Hud { get; private set; } = "";
    public MapDefinition? Map { get; private set; }
    public MapRuntime? Runtime { get; private set; }
    public int HudX { get; private set; }
    public int HudY { get; private set; }

    public bool HasMessage { get; private set; }
    public string MessageText { get; private set; } = "";
    public bool HasChoices { get; private set; }
    public string[] Choices { get; private set; } = Array.Empty<string>();
    public int Selected { get; private set; }

    public List<SavedPicture> Pictures { get; } = new();
    public List<SavedAudio> Audio { get; } = new();

    public void SetStatus(string text) => Status = text;

    public void SetMapHud(int leaderX, int leaderY, string text)
    {
        HudX = leaderX;
        HudY = leaderY;
        Hud = text;
    }

    public void ShowMap(MapDefinition map, MapRuntime runtime, int leaderX, int leaderY)
    {
        Map = map;
        Runtime = runtime;
        HudX = leaderX;
        HudY = leaderY;
    }

    public void ShowMessage(string text, int face)
    {
        MessageText = CleanMessage(text);
        HasMessage = true;
    }

    /// <summary>
    /// WOLF message strings carry inline control codes (E, c[n], \\n, sp[n], ^, ...).
    /// Copied from the retired ScreenHostView so headless sessions show the same text.
    /// </summary>
    private static string CleanMessage(string text)
    {
        if (string.IsNullOrEmpty(text) || !text.Contains('\\')) return text;
        var sb = new StringBuilder(text.Length);
        for (int i = 0; i < text.Length; i++)
        {
            char ch = text[i];
            if (ch != '\\' || i + 1 >= text.Length)
            {
                sb.Append(ch);
                continue;
            }

            char next = text[i + 1];
            if (next == 'n')
            {
                sb.Append('\n');
                i++;
                continue;
            }

            if (char.IsAsciiLetter(next))
            {
                int j = i + 1;
                while (j < text.Length && char.IsAsciiLetter(text[j])) j++;
                if (j < text.Length && text[j] == '[')
                {
                    int close = text.IndexOf(']', j + 1);
                    if (close >= 0) j = close + 1;
                }
                i = j - 1;
                continue;
            }

            if (next is '^' or '.' or '!' or '>' or '<' or '|')
            {
                i++;
                continue;
            }

            sb.Append(ch);
        }
        return sb.ToString();
    }

    public void HideMessage()
    {
        HasMessage = false;
        MessageText = "";
    }

    public void ShowChoices(string[] options, int selected)
    {
        Choices = options;
        Selected = selected;
        HasChoices = true;
    }

    public void HideChoices()
    {
        HasChoices = false;
        Choices = Array.Empty<string>();
        Selected = 0;
    }

    public void ClearPictures() => Pictures.Clear();

    public void RestorePictures(IReadOnlyList<SavedPicture> pictures)
    {
        Pictures.Clear();
        Pictures.AddRange(pictures);
    }

    public IReadOnlyList<SavedPicture> SnapshotPictures() => Pictures.ToList();

    public List<SavedPicture> PictureSnapshots() => Pictures.ToList();

    public void RestoreAudio(IReadOnlyList<SavedAudio> audio)
    {
        Audio.Clear();
        Audio.AddRange(audio);
    }

    public IReadOnlyList<SavedAudio> SnapshotAudio() => Audio.ToList();

    /// <summary>Same slot semantics as the retired screen: draw replaces, erase drops.</summary>
    public void DrawPicture(PictureCmd cmd)
    {
        if (cmd.Action is PictureAction.Erase or PictureAction.Hide)
        {
            Pictures.RemoveAll(p => p.Number == cmd.Number);
            return;
        }

        SavedPicture saved = Pictures.FirstOrDefault(p => p.Number == cmd.Number) ?? new SavedPicture { Number = cmd.Number };
        saved.Kind = cmd.Kind;
        saved.Action = (int)cmd.Action;
        saved.File = cmd.File;
        saved.Text = cmd.Text;
        saved.Args = cmd.Args.ToList();
        saved.Anchor = cmd.Anchor;
        if (!Pictures.Contains(saved)) Pictures.Add(saved);
        Pictures.Sort((a, b) => a.Number.CompareTo(b.Number));
    }
}
