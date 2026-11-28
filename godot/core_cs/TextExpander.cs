using System;
using System.Collections.Generic;
using System.Text;

namespace Lwsmf.Core;

public static class TextExpander
{
    /// <summary>
    /// Expand variable and database escapes. <paramref name="readDb"/> receives the WOLF
    /// database kind (0 = \cdb, 1 = \sdb, 2 = \udb) plus type/row/column and returns the cell
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
        ReadOnlySpan<char> span = text.AsSpan();
        for (int i = span.Length - 1; i >= 0; i--)
        {
            if (span[i] != '\\') continue;

            if (TryParseEscape(span, i, inst, vars, readDb, out int end, out string replacement))
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

    private static bool TryParseEscape(ReadOnlySpan<char> text, int start, Instance inst, Variables vars,
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

        ReadOnlySpan<char> name = text.Slice(start + 1, j - (start + 1));
        if (j >= text.Length || text[j] != '[') return false;

        int close = FindClose(text, j);
        if (close < 0) return false;
        ReadOnlySpan<char> body = text.Slice(j + 1, close - (j + 1));
        end = close + 1;

        int dbKind = name.Equals("udb", StringComparison.OrdinalIgnoreCase) ? 2
            : name.Equals("sdb", StringComparison.OrdinalIgnoreCase) ? 1
            : name.Equals("cdb", StringComparison.OrdinalIgnoreCase) ? 0
            : -1;
        if (dbKind >= 0)
        {
            // A database escape is a cell read, not a colour: [type:row:column], each
            // field itself an expression (docs/engine/TEXT_SPEC.md section 2.7).
            if (readDb == null) return false;
            replacement = ReadDb(body, dbKind, inst, vars, readDb);
            return true;
        }

        if (!int.TryParse(body.Trim(), out int index)) return false;

        if (name.Equals("s", StringComparison.OrdinalIgnoreCase)
            || name.Equals("corgs", StringComparison.OrdinalIgnoreCase))
        {
            replacement = vars.GetStr(index);
            return true;
        }

        if (name.Equals("cself", StringComparison.OrdinalIgnoreCase)
            || name.Equals("self", StringComparison.OrdinalIgnoreCase)
            || name.Equals("cself_s", StringComparison.OrdinalIgnoreCase)
            || name.Equals("self_s", StringComparison.OrdinalIgnoreCase))
        {
            int slot = inst.Program.SelfBase + index;
            string s = inst.Self.GetStr(slot);
            if (string.IsNullOrEmpty(s))
            {
                replacement = (index % 100) is >= 5 and <= 9 ? string.Empty : inst.Self.Get(slot).ToString();
            }
            else
            {
                replacement = s;
            }
            return true;
        }

        if (name.Equals("v", StringComparison.OrdinalIgnoreCase)
            || name.Equals("corgv", StringComparison.OrdinalIgnoreCase))
        {
            replacement = vars.Get(index).ToString();
            return true;
        }

        if (name.Equals("sys", StringComparison.OrdinalIgnoreCase)
            || name.Equals("csys", StringComparison.OrdinalIgnoreCase))
        {
            replacement = vars.GetSys(index).ToString();
            return true;
        }

        return false;
    }

    /// <summary>
    /// Resolve the three colon-separated database coordinates. Split at bracket depth
    /// zero so a nested escape such as \udb[\cself[90]:\cself[10]:8] keeps its fields.
    /// </summary>
    private static string ReadDb(ReadOnlySpan<char> body, int kind, Instance inst, Variables vars,
        Func<int, int, int, int, string> readDb)
    {
        int start0 = 0, len0 = 0;
        int start1 = 0, len1 = 0;
        int start2 = 0, len2 = 0;
        int fieldIndex = 0;
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
                if (fieldIndex == 0) { start0 = from; len0 = i - from; }
                else if (fieldIndex == 1) { start1 = from; len1 = i - from; }
                else if (fieldIndex == 2) { start2 = from; len2 = i - from; }
                fieldIndex++;
                from = i + 1;
            }
        }
        if (fieldIndex == 0) { start0 = from; len0 = body.Length - from; }
        else if (fieldIndex == 1) { start1 = from; len1 = body.Length - from; }
        else if (fieldIndex == 2) { start2 = from; len2 = body.Length - from; }
        int fieldCount = fieldIndex + 1;

        int Coord(ReadOnlySpan<char> slice)
        {
            if (!slice.Contains('\\'))
            {
                return int.TryParse(slice.Trim(), out int direct) ? direct : 0;
            }
            string field = Expand(slice.ToString(), inst, vars, readDb).Trim();
            return int.TryParse(field, out int value) ? value : 0;
        }

        int c0 = fieldCount > 0 ? Coord(body.Slice(start0, len0)) : 0;
        int c1 = fieldCount > 1 ? Coord(body.Slice(start1, len1)) : 0;
        int c2 = fieldCount > 2 ? Coord(body.Slice(start2, len2)) : 0;
        return readDb(kind, c0, c1, c2);
    }

    /// <summary>Matching ']' with bracket depth, skipping escaped characters.</summary>
    private static int FindClose(ReadOnlySpan<char> text, int open)
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
