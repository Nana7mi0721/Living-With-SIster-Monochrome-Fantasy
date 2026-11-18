using System;
using System.Text;

namespace Lwsmf.Core;

public static class TextExpander
{
    public static string Expand(string text, Instance inst, Variables vars)
    {
        if (string.IsNullOrEmpty(text) || !text.Contains('\\'))
            return text;

        string current = text;
        for (int pass = 0; pass < 64; pass++)
        {
            if (!TryExpandOnce(current, inst, vars, out string next))
                break;
            current = next;
        }
        return current;
    }

    private static bool TryExpandOnce(string text, Instance inst, Variables vars, out string expanded)
    {
        for (int i = text.Length - 1; i >= 0; i--)
        {
            if (text[i] != '\\') continue;

            if (TryParseEscape(text, i, inst, vars, out int end, out string replacement))
            {
                var sb = new StringBuilder(text.Length + replacement.Length);
                sb.Append(text, 0, i);
                sb.Append(replacement);
                sb.Append(text, end, text.Length - end);
                expanded = sb.ToString();
                return true;
            }
        }

        expanded = text;
        return false;
    }

    private static bool TryParseEscape(string text, int start, Instance inst, Variables vars, out int end, out string replacement)
    {
        end = start;
        replacement = "";

        int j = start + 1;
        while (j < text.Length && char.IsAsciiLetter(text[j]))
        {
            j++;
        }
        if (j == start + 1) return false;

        string name = text.Substring(start + 1, j - (start + 1));
        if (j >= text.Length || text[j] != '[') return false;

        int close = text.IndexOf(']', j + 1);
        if (close < 0) return false;

        string digits = text.Substring(j + 1, close - (j + 1)).Trim();
        if (!int.TryParse(digits, out int index)) return false;

        end = close + 1;

        switch (name.ToLowerInvariant())
        {
            case "s" or "corgs":
                replacement = vars.GetStr(index);
                return true;

            case "cself" or "self" or "cself_s" or "self_s":
            {
                string s = inst.Self.GetStr(inst.Program.SelfBase + index);
                replacement = string.IsNullOrEmpty(s) ? inst.Self.Get(inst.Program.SelfBase + index).ToString() : s;
                return true;
            }

            case "v" or "corgv":
                replacement = vars.Get(index).ToString();
                return true;

            case "sys" or "csys":
                replacement = vars.GetSys(index).ToString();
                return true;

            default:
                return false;
        }
    }
}
