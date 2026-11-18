"""命令行入口：python -m converter <stage> [--root ...] [--out ...]"""
from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile

from . import schema


def _sha(path: str) -> str:
    return schema.sha256_file(path)


def _kind_of(name: str) -> str:
    ext = os.path.splitext(name)[1].lower()
    if ext in (".png", ".jpg", ".jpeg", ".webp", ".bmp"):
        return "image"
    if ext in (".ogg", ".wav", ".mp3", ".m4a"):
        return "audio"
    if ext in (".ttf", ".otf", ".ttc"):
        return "font"
    return "other"


#: `all` 阶段的固定执行顺序；新增阶段时只改这里和 STAGES。
ALL_STAGES = ("manifest", "programs", "database", "map-tiles", "validate")

#: 完整转换实际读取的 BasicData 文件；缺任何一个都在阶段 0 直接报错（exit 2）。
REQUIRED_BASICDATA = (
    "BasicData/Game.dat",
    "BasicData/CommonEvent.dat",
    "BasicData/DataBase.dat",
    "BasicData/CDataBase.dat",
    "BasicData/SysDatabase.dat",
    "BasicData/TileSetData.dat",
    "BasicData/MapTree.dat",
)


def _writable_problem(path: str) -> str | None:
    """返回目录不可写的原因；可写返回 None。会创建缺失目录。"""
    try:
        os.makedirs(path, exist_ok=True)
        fd, probe = tempfile.mkstemp(prefix=".convert-write-", dir=path)
        os.close(fd)
        os.unlink(probe)
    except OSError as exc:
        return str(exc)
    return None


def _preflight_all(args) -> str | None:
    """stage all 的完整性与可写性检查；返回错误文本或 None。"""
    root = os.path.abspath(args.root)
    if not os.path.isdir(root):
        return f"root directory not found: {args.root}"
    missing = [rel for rel in REQUIRED_BASICDATA
               if not os.path.isfile(os.path.join(root, *rel.split("/")))]
    if missing:
        return "missing required BasicData file(s) under {}: {}".format(
            root, ", ".join(missing))
    for label, path in (("out", args.out), ("reports", args.reports)):
        problem = _writable_problem(path)
        if problem:
            return f"{label} directory not writable: {path}: {problem}"
    return None


def cmd_manifest(args) -> int:
    root = args.root
    if not os.path.isdir(root):
        print(f"root not found: {root}", file=sys.stderr)
        return 2

    files = []
    for sub, exts in (("BasicData", (".dat",)), ("MapData", (".mps",))):
        d = os.path.join(root, sub)
        if not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            if os.path.splitext(name)[1].lower() in exts:
                rel = sub + "/" + name
                p = os.path.join(root, rel)
                files.append({"path": rel, "bytes": os.path.getsize(p), "sha256": _sha(p)})

    bad = []
    seen = {f["path"]: f["sha256"] for f in files}
    for rel, want in schema.BASELINE_SHA256.items():
        got = seen.get(rel)
        if got is None:
            bad.append(f"{rel}: missing")
        elif got != want:
            bad.append(f"{rel}: sha256 {got} != baseline {want}")
    if bad:
        for b in bad:
            print("baseline mismatch: " + b, file=sys.stderr)
        return 1

    assets = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            if rel in seen:
                continue
            kind = _kind_of(name)
            assets.append({"logical": rel, "source": rel, "kind": kind,
                           "bytes": os.path.getsize(full), "sha256": _sha(full)})

    counts = None
    programs_path = os.path.join(args.out, args.programs_name)
    if os.path.exists(programs_path):
        with open(programs_path, "r", encoding="utf-8") as f:
            prog = json.load(f)
        ce_progs = prog.get("common_events") or []
        maps = prog.get("maps") or []
        map_progs = [ev for m in maps for ev in m.get("events") or []]
        ce = schema.count_slots(ce_progs)
        me = schema.count_slots(map_progs)
        ops = {s["op"] for p in ce_progs + map_progs for s in p.get("slots") or [] if s.get("op")}
        counts = {
            "common_events": ce["events"], "map_events": me["events"], "maps": len(maps),
            "slots": ce["slots"] + me["slots"],
            "blank_slots": ce["blank_slots"] + me["blank_slots"],
            "real_instructions": ce["real_instructions"] + me["real_instructions"],
            "distinct_opcodes": len(ops),
        }

    artifacts = {}
    for key, name in (("programs", args.programs_name), ("database", args.database_name)):
        p = os.path.join(args.out, name)
        if os.path.exists(p):
            artifacts[key] = {"path": name, "bytes": os.path.getsize(p), "sha256": _sha(p)}

    manifest = schema.envelope(schema.SCHEMA_MANIFEST)
    manifest.update({
        "game": {"id": "lwsmf", "title": "Living With Sister Monochrome Fantasy",
                 "appid": 2429860, "version": None},
        "source": {"files": files},
        "artifacts": artifacts,
        "counts": counts,
        "assets": assets,
        "asset_omissions": [],
    })
    rel = "manifest.json"
    info = schema.write_artifact(manifest, os.path.join(args.out, rel), rel)
    print(json.dumps({"written": info, "files": len(files), "assets": len(assets),
                      "counts": counts}, ensure_ascii=False))
    return 0


def cmd_all(args) -> int:
    """按 manifest -> programs -> database -> map-tiles -> validate 顺序执行完整转换。"""
    problem = _preflight_all(args)
    if problem:
        print(f"integrity check failed: {problem}", file=sys.stderr)
        return 2

    total = len(ALL_STAGES)
    for index, name in enumerate(ALL_STAGES, 1):
        print(f"[{index}/{total}] {name} ...", flush=True)
        code = STAGES[name](args)
        if code != 0:
            print(f"[{index}/{total}] {name} FAILED (exit {code})", file=sys.stderr,
                  flush=True)
            return code if code > 0 else 1
        print(f"[{index}/{total}] {name} OK", flush=True)
    print(f"CONVERT_ALL_OK root={args.root} out={args.out} reports={args.reports}",
          flush=True)
    return 0


def build_parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--root", default="work/all", help="解包后的 Data 目录树")
    common.add_argument("--out", default="game_data", help="game_data 输出目录")
    common.add_argument("--reports", default="reports", help="对账报告输出目录")
    common.add_argument("--programs-name", default="programs.json")
    common.add_argument("--database-name", default="database.json")
    common.add_argument("--map-tiles-name", default="map_tiles.json")

    ap = argparse.ArgumentParser(prog="python -m converter")
    sub = ap.add_subparsers(dest="stage", required=True)
    sub.add_parser("all", parents=[common],
                   help="按 manifest -> programs -> database -> map-tiles -> validate 顺序执行完整转换")
    sub.add_parser("manifest", parents=[common], help="生成 manifest.json（含 sha256 基准校验）")
    sub.add_parser("programs", parents=[common], help="导出 programs.json")
    sub.add_parser("database", parents=[common], help="导出 database.json")
    sub.add_parser("save", parents=[common], help="导出 save_import.json（读 .sav）")
    sub.add_parser("map-tiles", parents=[common], help="导出 map_tiles.json（三层图块 + tileset 通行度）")
    sub.add_parser("validate", parents=[common], help="对账并写 reports/reconcile.json")
    return ap


def cmd_programs(args) -> int:
    from .export import programs

    return programs.run(args)


def cmd_validate(args) -> int:
    from . import validate

    return validate.run(args)


def cmd_database(args) -> int:
    from .export import database

    return database.run(args)


def cmd_map_tiles(args) -> int:
    from .export import map_tiles

    return map_tiles.run(args)


def cmd_save(args) -> int:
    from .export import save

    return save.run(args)


STAGES = {"all": cmd_all, "manifest": cmd_manifest, "programs": cmd_programs,
          "database": cmd_database, "save": cmd_save,
          "map-tiles": cmd_map_tiles, "validate": cmd_validate}


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    fn = STAGES.get(args.stage)
    if fn is None:
        print(f"stage {args.stage!r} not implemented yet (M0 skeleton)", file=sys.stderr)
        return 3
    return fn(args)
