using System;
using System.Collections.Generic;
using System.Text;

namespace Lwsmf.Core;

public static class TextExpander
{
    /// <summary>
    /// Expand variable and database escapes. <paramref name="readDb"/> receives the WOLF
    /// database kind (0 = \cdb/\sdb, 2 = \udb) plus type/row/column and returns the cell
    /// text; without it the database escapes stay verbatim for a caller that cannot read cells.
    /// </summary>
    public static string Expand(string text, Instance inst, Variables vars,
        Func<int, int, int, int, string>? readDb = null)
    {
        if (string.IsNullOrEmpty(text) || !text.Contains('\\'))
            return text;

        string current = text;
        for (int pass = 0; pass < 64; pass++)
        {
            if (!TryExpandOnce(current, inst, vars, readDb, out string next))
                break;
            current = next;
        }
        return current;
    }

    private static bool TryExpandOnce(string text, Instance inst, Variables vars,
        Func<int, int, int, int, string>? readDb, out string expanded)
    {
        for (int i = text.Length - 1; i >= 0; i--)
        {
            if (text[i] != '\\') continue;

            if (TryParseEscape(text, i, inst, vars, readDb, out int end, out string replacement))
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

    private static bool TryParseEscape(string text, int start, Instance inst, Variables vars,
        Func<int, int, int, int, string>? readDb, out int end, out string replacement)
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

        int close = FindClose(text, j);
        if (close < 0) return false;
        string body = text.Substring(j + 1, close - (j + 1));
        end = close + 1;

        string lower = name.ToLowerInvariant();
        if (lower is "cdb" or "sdb" or "udb")
        {
            // A database escape is a cell read, not a colour: [type:row:column], each
            // field itself an expression (docs/engine/TEXT_SPEC.md section 2.7).
            if (readDb == null) return false;
            int kind = lower == "udb" ? 2 : 0;
            replacement = ReadDb(body, kind, inst, vars, readDb);
            return true;
        }

        if (!int.TryParse(body.Trim(), out int index)) return false;

        switch (lower)
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

    /// <summary>
    /// Resolve the three colon-separated database coordinates. Split at bracket depth
    /// zero so a nested escape such as \udb[\cself[90]:\cself[10]:8] keeps its fields.
    /// </summary>
    private static string ReadDb(string body, int kind, Instance inst, Variables vars,
        Func<int, int, int, int, string> readDb)
    {
        var fields = new List<string>();
        int depth = 0;
        int from = 0;
        for (int i = 0; i < body.Length; i++)
        {
            char c = body[i];
            if (c == '\\')
            {
                i++;
                continue;
            }
            if (c == '[') depth++;
            else if (c == ']') depth--;
            else if (c == ':' && depth == 0)
            {
                fields.Add(body.Substring(from, i - from));
                from = i + 1;
            }
        }
        fields.Add(body.Substring(from));

        int Coord(int slot)
        {
            if (slot >= fields.Count) return 0;
            string field = Expand(fields[slot], inst, vars, readDb).Trim();
            return int.TryParse(field, out int value) ? value : 0;
        }

        return readDb(kind, Coord(0), Coord(1), Coord(2));
    }

    /// <summary>Matching ']' with bracket depth, skipping escaped characters.</summary>
    private static int FindClose(string text, int open)
    {
        int depth = 0;
        for (int i = open; i < text.Length; i++)
        {
            char c = text[i];
            if (c == '\\')
            {
                i++;
                continue;
            }
            if (c == '[') depth++;
            else if (c == ']')
            {
                depth--;
                if (depth == 0) return i;
            }
        }
        return -1;
    }
}
