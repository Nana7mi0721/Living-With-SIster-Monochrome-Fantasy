#!/usr/bin/env python3
"""Audit the translation rows that fell back to the shipped official Chinese.
Reads the pipeline outputs and writes:
  official_fallback_audit.tsv  one row per event that still has official text
  official_only.tsv            every official-fallback text row
The point is to show WHERE the 4350 official rows are and WHY, so the next
translation pass can target them instead of re-auditing the whole corpus.
"""
import csv, json, re, sys

AB = "work/translation/final/wolf_text_AB.jsonl"
SUM = "work/translation/final/summary_by_event.tsv"
AUDIT = "work/translation/final/official_fallback_audit.tsv"
ONLY = "work/translation/final/official_only.tsv"
KANA = re.compile(r"[\u3040-\u30ff]")
ALBUM = re.compile(r"アルバム|相册|相冊|album|Album")

def main():
    summaries = {}
    with open(SUM, encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            summaries[int(row["event"])] = row

    rows = [json.loads(line) for line in open(AB, encoding="utf-8") if line.strip()]
    official = [r for r in rows if r.get("source") == "official"]
    by_event = {}
    for r in official:
        by_event.setdefault(int(r["event"]), []).append(r)

    audit = []
    for event in sorted(by_event):
        rs = by_event[event]
        s = summaries.get(event, {})
        title = s.get("title", "")
        method = s.get("method", "")
        reason = s.get("reason", "")
        if method == "mismatch" or reason == "mv_id_collision":
            category = "mv_id_collision"
        elif reason == "dlc_absent_in_mv":
            category = "dlc_absent"
        elif reason == "mv_empty_title":
            category = "mv_no_text"
        elif ALBUM.search(title):
            category = "album_absent"
        elif reason == "ok":
            category = "aligned_unmatched_lines"
        else:
            category = reason or "unknown"
        kana = sum(1 for r in rs if KANA.search(r.get("official_zh") or ""))
        audit.append({
            "event": event,
            "title": title,
            "method": method,
            "category": category,
            "official_lines": len(rs),
            "kana_lines": kana,
            "avg_sim": s.get("avg_sim", ""),
            "sample_official": (rs[0].get("official_zh") or "").replace("\n", "\\n")[:120],
        })

    with open(AUDIT, "w", encoding="utf-8", newline="\n") as f:
        cols = ["event", "title", "method", "category", "official_lines", "kana_lines", "avg_sim", "sample_official"]
        f.write("\t".join(cols) + "\n")
        for a in audit:
            f.write("\t".join(str(a[c]).replace("\t", " ") for c in cols) + "\n")

    with open(ONLY, "w", encoding="utf-8", newline="\n") as f:
        f.write("event\tpc\tofficial_zh\n")
        for r in official:
            text = (r.get("official_zh") or "").replace("\t", " ").replace("\n", "\\n")
            f.write("%s\t%s\t%s\n" % (r["event"], r["index"], text))

    from collections import Counter
    cats = Counter(a["category"] for a in audit)
    print("official rows:", len(official), "events:", len(audit))
    print("categories:", dict(cats))
    print("kana rows:", sum(1 for r in official if KANA.search(r.get("official_zh") or "")))
    print("top events:")
    for a in sorted(audit, key=lambda x: -x["official_lines"])[:15]:
        print("  CE%-4s %-4s %-24s %4d lines, kana %d, %s" % (
            a["event"], a["category"], a["title"][:24], a["official_lines"], a["kana_lines"], a["method"]))
    print("wrote", AUDIT, "and", ONLY)

if __name__ == "__main__":
    main()
