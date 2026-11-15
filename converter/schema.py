"""数据契约的唯一权威定义。

对应文档：docs/godot/DATA_CONTRACT.md。本文与文档不一致时以本文为准，并回头修文档。
产物必须可复现：相同输入 -> 逐字节相同的文件，因此产物里不写墙钟时间、不写绝对路径。
"""
from __future__ import annotations

import hashlib
import json
import os

SCHEMA_MANIFEST = "wolf.manifest"
SCHEMA_PROGRAMS = "wolf.programs"
SCHEMA_DATABASE = "wolf.database"
SCHEMA_SAVE_IMPORT = "wolf.save_import"
SCHEMA_MAP_TILES = "wolf.map_tiles"
SCHEMA_VERSION = 1
PRODUCER = {"name": "converter", "version": "0.1.0"}

#: 与 crate 保持同名同义的哨兵值
NO_RETURN_VALUE = 0xFFFFFFFF
COMMON_SELF_BASE = 600
MAP_SELF_BASE = 100

#: FORMAT_SPEC §0.4 解包基准；manifest 必须复现，否则转换失败
BASELINE_SHA256 = {
    "BasicData/Game.dat": "6c8cfdc797f479a380973daa0ced7373efc211bb4e7955ee408d78645234c1c5",
    "BasicData/CommonEvent.dat": "4d1f85cd93f66ce8b1d958097eb67e535a10d8df1f81cf06b0526ec6bedb96cf",
    "BasicData/DataBase.dat": "294d75f57e9c66ebdf77e0579467808fd44e5de8c9a025ba8f638ff0df13a5d1",
}


def envelope(kind: str) -> dict:
    return {"schema": kind, "schema_version": SCHEMA_VERSION, "producer": dict(PRODUCER)}


def canonical_json(obj) -> str:
    return json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2) + "\n"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def write_artifact(obj: dict, path: str, rel_path: str) -> dict:
    """原子写：先写 .tmp 再 replace，避免半截产物被运行时读到。"""
    data = canonical_json(obj).encode("utf-8")
    d = os.path.dirname(os.path.abspath(path))
    os.makedirs(d, exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    return {"path": rel_path, "bytes": len(data), "sha256": sha256_bytes(data)}


# -- 结构 ------------------------------------------------------------------
def slot(pc: int, op: int, params: list, branch_depth: int, strings: list, move_route=None) -> dict:
    return {
        "pc": pc,
        "op": op,
        "params": list(params) if op else [],
        "branch_depth": branch_depth if op else 0,
        "strings": list(strings) if op else [],
        "move_route": move_route if op else None,
    }


def block(head: int, kind: str, cases: list, else_pc, cancel_pc, end_pc: int) -> dict:
    return {
        "head": head,
        "kind": kind,
        "cases": [list(c) for c in cases],
        "else_pc": else_pc,
        "cancel_pc": cancel_pc,
        "end_pc": end_pc,
    }


def program(kind: str, pid: int, title: str, self_base: int, slots: list, blocks: list,
            labels: dict, stats: dict, **extra) -> dict:
    out = {
        "id": pid,
        "kind": kind,
        "title": title,
        "self_base": self_base,
        "slots": slots,
        "blocks": blocks,
        "labels": dict(labels),
        "stats": stats,
    }
    out.update(extra)
    return out


# -- 校验 ------------------------------------------------------------------
def validate_program(p: dict) -> list:
    errs = []
    pid = p.get("id")
    if p.get("kind") not in ("common_event", "map_event"):
        errs.append(f"program {pid}: bad kind {p.get('kind')!r}")
    if p.get("self_base") not in (COMMON_SELF_BASE, MAP_SELF_BASE):
        errs.append(f"program {pid}: bad self_base {p.get('self_base')!r}")
    slots = p.get("slots") or []
    for i, s in enumerate(slots):
        if s.get("pc") != i:
            errs.append(f"program {pid}: slot {i} has pc {s.get('pc')}")
            break
        if s.get("op") == 0 and (s.get("params") or s.get("strings") or s.get("branch_depth")):
            errs.append(f"program {pid}: blank slot {i} is not empty")
            break
    heads = {b.get("head") for b in p.get("blocks") or []}
    errs += [f"program {pid}: block head {h} out of range" for h in heads
             if not isinstance(h, int) or h < 0 or h >= len(slots)]
    return errs


def count_slots(programs: list) -> dict:
    slots = blank = real = 0
    ops = set()
    for p in programs:
        for s in p.get("slots") or []:
            slots += 1
            if s.get("op"):
                real += 1
                ops.add(s["op"])
            else:
                blank += 1
    return {"events": len(programs), "slots": slots, "blank_slots": blank,
            "real_instructions": real, "distinct_opcodes": len(ops)}
