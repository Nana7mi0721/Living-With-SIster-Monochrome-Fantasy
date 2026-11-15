"""导出 database.json（Game.dat + 三个数据库 + tileset + map tree）。"""
from __future__ import annotations

import json
import os

from .. import schema
from ..parse import database as db


def build(root: str) -> tuple:
    data = db.load_database(root)
    warnings = data.pop("warnings", [])
    doc = schema.envelope(schema.SCHEMA_DATABASE)
    doc.update(data)
    return doc, warnings


def run(args) -> int:
    doc, warnings = build(args.root)
    info = schema.write_artifact(doc, os.path.join(args.out, args.database_name), args.database_name)
    if warnings:
        os.makedirs(args.reports, exist_ok=True)
        schema.write_artifact({"schema": "wolf.warnings", "schema_version": 1,
                               "source": "database", "items": warnings},
                              os.path.join(args.reports, "database_warnings.json"),
                              "database_warnings.json")
    print(json.dumps({"written": info, "warnings": len(warnings)}, ensure_ascii=False))
    return 0
