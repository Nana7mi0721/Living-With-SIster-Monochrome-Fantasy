#!/usr/bin/env python3
r"""Build the runtime text-overlay table for the Rust engine.

Reads the per-line A/B sheet (work/translation/final/wolf_text_AB.jsonl) and writes
work/translation/final/wolf_override.tsv: event<TAB>pc<TAB>text, consumed by
wolf_game::translation::TextOverlay (env WOLF_TEXT_OVERLAY).

Rows where the fan translation (bimeng) won are written.  MV/JRPG-Maker escape
codes are translated onto WOLF's own syntax before the row is accepted:

  \N<name> / \N[name]  -> dropped (the speaker is a separate 101 field)
  \C[n]                -> \c[n]   (MV writes the colour code upper-case)
  \V[n]                -> WOLF variable, or the official line's own code when
                          that line names the same value (e.g. \cself[8])
  \c[n]                -> \c[n]; when the official line colours that span with a
                          different palette index, the official index wins
  \{  / \}             -> the official line's font-size escape (\f[32]) / dropped
  \_                   -> a half-width space (and \_C[n] is repaired to \c[n])
  \.  / \^             -> kept (WOLF supports both pauses)

The official string's leading style prefix (\E outline and any \f[n] size) is
prepended so the translated line keeps the original weight and font size.  A row
whose remaining escapes cannot be mapped onto WOLF's grammar keeps the shipped
official string instead.
"""
import json, os, re

BS = chr(92)
SRC = "work/translation/final/wolf_text_AB.jsonl"
OUT = "work/translation/final/wolf_override.tsv"

# Escape codes the WOLF tokeniser understands (crates/wolf-format/src/messagetext.rs).
WOLF_CODES = {
    "E", "e", "c", "C", "f", "f+", "v", "V", "self", "cself", "sys", "sysS",
    "s", "udb", "cdb", "sdb", "xy", "xyN", "wE", "cE", "cS", "cR", "fR",
    "m", "font", "-", "space", "sp", "bx", "mx", "my", "ax", "ay",
    "i", "iS", "isize", "r",
}


def strip_names(s):
    out = []
    i = 0
    n = len(s)
    while i < n:
        if s[i] == BS and i + 1 < n and s[i + 1] == "N":
            nxt = s[i + 2] if i + 2 < n else ""
            if nxt == "<":
                j = s.find(">", i + 3)
                i = j + 1 if j != -1 else n
                continue
            if nxt == "[":
                j = s.find("]", i + 3)
                i = j + 1 if j != -1 else n
                continue
        out.append(s[i])
        i += 1
    return "".join(out)


CODE_NAMES = sorted({c.rstrip("+") for c in WOLF_CODES if c != "f+"}, key=len, reverse=True)


def is_alpha(c):
    return ("A" <= c <= "Z") or ("a" <= c <= "z")


def scan(s):
    """Return [(start, end, literal, kind, arg)] for first-level escapes."""
    out = []
    i = 0
    n = len(s)
    while i < n:
        if s[i] != BS:
            i += 1
            continue
        if i + 1 >= n:
            out.append((i, n, BS, "bs", ""))
            i = n
            continue
        c = s[i + 1]
        if c in ".!^" + BS:
            out.append((i, i + 2, s[i:i + 2], {".": "dot", "!": "bang", "^": "caret", BS: "bs"}[c], ""))
            i += 2
            continue
        if c in "{}_":
            out.append((i, i + 2, s[i:i + 2], {"{": "fontup", "}": "fontdown", "_": "halfspace"}[c], ""))
            i += 2
            continue
        if is_alpha(c):
            j = None
            for cand in CODE_NAMES:
                if s.startswith(cand, i + 1):
                    j = i + 1 + len(cand)
                    break
            if j is None:
                # A single unknown ASCII escape letter; keep it visible as "other"
                # instead of swallowing the literal text that follows it.
                j = i + 2
            name = s[i + 1:j]
            if j < n and s[j] == "+":
                j += 1
                name += "+"
            if name in ("v", "V") and j < n and s[j].isdigit():
                name += s[j]
                j += 1
            if j < n and s[j] == "[":
                depth = 1
                k = j + 1
                while k < n and depth:
                    if s[k] == "[":
                        depth += 1
                    elif s[k] == "]":
                        depth -= 1
                    k += 1
                lit = s[i:k]
                i = k
            else:
                lit = s[i:j]
                i = j
            out.append((i - len(lit), i, lit, classify(name, lit), bracket(lit)))
            continue
        out.append((i, i + 2, s[i:i + 2], "other", ""))
        i += 2
    return out


def bracket(lit):
    p = lit.find("[")
    return lit[p + 1:-1] if p != -1 and lit.endswith("]") else ""


def classify(name, lit):
    low = name.lower()
    if low == "c":
        return "color"
    if low == "v":
        return "var"
    if low in ("f", "f+"):
        return "font"
    if low == "e":
        return "outline"
    return "other"


FONT_RE = re.compile(r"^" + BS + r"f\+?\[(-?\d+)\]$")


def font_literal(lit):
    m = FONT_RE.match(lit)
    if not m:
        return lit
    v = int(m.group(1))
    return BS + "f[%d]" % max(1, v)


def repair_body(body):
    # A half-width backslash lost in the MV source: \_C[2] / \_c[1] -> \C[2].
    body = body.replace(BS + "_C[", BS + "C[").replace(BS + "_c[", BS + "c[")
    # A colour code missing its backslash at the start of a line: c[1] -> \c[1].
    body = re.sub(r"(^|\n)([cC])\[(\d+)\]", lambda m: m.group(1) + BS + m.group(2) + "[" + m.group(3) + "]", body)
    return body


def prefix_of(official):
    """Leading outline/font style tokens of the official line, in order."""
    toks = scan(official)
    prefix = []
    cursor = 0
    for start, end, lit, kind, _arg in toks:
        if start != cursor:
            break
        if kind in ("outline", "font"):
            prefix.append(font_literal(lit) if kind == "font" else lit)
            cursor = end
        else:
            break
    return "".join(prefix), toks


def map_body(body, official):
    """Map MV escapes in body onto WOLF syntax using the official line as guide.

    Returns (text, ok).  ok is False when an unmappable escape survives.
    """
    # Alignment tags lay out a title/subtitle split that the MV sheet often
    # collapses into prose; replacing such a slot would drop the title, so keep
    # the shipped official string for these few rows.
    if re.search(r"<[LCR]>", official):
        return body, False
    body = repair_body(body)
    prefix, off_toks = prefix_of(official)
    # Skip the official prefix tokens when building the queues.
    off_used = 0
    while off_used < len(off_toks) and off_toks[off_used][3] in ("outline", "font") and off_toks[off_used][0] == (0 if off_used == 0 else off_toks[off_used - 1][1]):
        off_used += 1
    queues = {}
    for _s, _e, lit, kind, _arg in off_toks[off_used:]:
        queues.setdefault(kind, []).append(lit)

    def take(kind):
        q = queues.get(kind) or []
        return q.pop(0) if q else None

    toks = scan(body)
    parts = []
    cursor = 0
    for start, end, lit, kind, arg in toks:
        parts.append(body[cursor:start])
        cursor = end
        if kind == "outline":
            continue  # the official prefix already carries it
        if kind == "color":
            official_lit = take("color")
            if official_lit is not None:
                parts.append(official_lit)
            else:
                try:
                    idx = int(arg)
                except ValueError:
                    idx = 99
                if 0 <= idx <= 15:
                    parts.append(BS + "c[%d]" % idx)
        elif kind == "var":
            official_lit = take("var")
            if official_lit is not None:
                parts.append(official_lit)
            else:
                parts.append(BS + "v[" + arg + "]")
        elif kind == "fontup":
            official_lit = take("font")
            parts.append(font_literal(official_lit) if official_lit else "")
        elif kind == "fontdown":
            continue
        elif kind == "halfspace":
            parts.append(" ")
        elif kind in ("dot", "caret", "bang"):
            parts.append(lit)
        elif kind == "bs":
            parts.append(BS)
        elif kind == "other":
            # Keep only codes WOLF understands, and rewrite their name verbatim.
            continue
        else:
            continue
    parts.append(body[cursor:])
    text = prefix + "".join(parts)
    # Reject if an unknown escape slipped through (it would render literally).
    for _s, _e, lit, kind, _arg in scan(text):
        if kind == "other" or (kind == "var" and False):
            return text, False
        if lit.startswith(BS) and not lit.startswith(BS + ".") and not lit.startswith(BS + "^") and not lit.startswith(BS + "!"):
            name = lit[1:]
            for cut in ("[", "+"):
                p = name.find(cut)
                if p != -1:
                    name = name[:p]
                    break
            if name not in WOLF_CODES:
                return text, False
    return text, True


def esc(text):
    return text.replace(BS, BS + BS).replace(chr(10), BS + "n").replace(chr(9), BS + "t").replace(chr(13), "")


def main():
    rows = [json.loads(l) for l in open(SRC, encoding="utf-8") if l.strip()]
    records = []
    skipped = {"not_bimeng": 0, "no_text": 0, "unmappable": 0, "empty": 0}
    mapped = 0
    examples = []
    for r in rows:
        if r.get("source") != "bimeng":
            skipped["not_bimeng"] += 1
            continue
        body = strip_names(r.get("final_zh") or "")
        if not body:
            skipped["no_text"] += 1
            continue
        text, ok = map_body(body, r.get("official_zh") or "")
        if not ok:
            skipped["unmappable"] += 1
            continue
        if BS in body:
            mapped += 1
            if len(examples) < 8:
                examples.append((r.get("event"), r.get("index"), r.get("official_zh"), text))
        if not text.strip():
            skipped["empty"] += 1
            continue
        records.append((r.get("event"), r.get("index"), text))

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline=chr(10)) as f:
        f.write("# event\tpc\ttext  -- generated by gen_override.py; do not edit by hand" + chr(10))
        for event, index, text in records:
            f.write("%s\t%s\t%s%s" % (event, index, esc(text), chr(10)))

    events = len({e for e, _, _ in records})
    print("wrote", OUT)
    print("records:", len(records), "events:", events, "of", len(rows), "text rows")
    print("escaped rows now mapped:", mapped)
    print("skipped:", skipped)
    for event, index, official, text in examples:
        print("  mapped %s/%s OFF %r -> %r" % (event, index, official[:60], text[:80]))
    for event, index, text in records[:5]:
        print("  sample %s/%s %r" % (event, index, text))


if __name__ == "__main__":
    main()
