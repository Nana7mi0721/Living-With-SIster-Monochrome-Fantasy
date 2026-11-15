"""WOLF 地图文件 .mps 解析：与 crates/wolf-format/src/mps.rs 逐字节等价。

布局（全部小端、无填充）：

    [16]  MAGIC = 10x0x00 + "WOLFM\0"
    [1]   version_header = 0x55
    [3]   MAGIC2 = 0x00 0x00 0x00
    [4]   unknown1 = 100
    [1]   version in {0x65, 0x66}
    [t]   name
    [4]   tileset_index
    [4]   width
    [4]   height
    [4]   event_count
    [4]   first_pixel           0xFFFFFFFF = 无图层数据
    [*]   有图层时：width*height*3 个 u32（3 层，层优先）
    [*]   event_count 个事件记录
    [1]   FOOTER = 0x66
    [*]   尾部填充，必须全 0

事件记录：u8 0x6F、u32 12345、u32 id、t_str name、u32 x、u32 y、u32 page_count、
u32 0、page_count 个页、u8 0x70。

页记录：u8 0x79、u32 graphic_chip_id、t_str graphic、u8 direction、u8 frame、
u8 opacity、u8 display_type、u8 trigger、4xu8 trigger_switches、
4xu32 trigger_variables、4xi32 trigger_values、RouteData、u32 command_count、
command_count 个 Slot、u32 features、u8 shadow_graphic、u8 collision_width、
u8 collision_height、[features > 3 时多 1 字节]、u8 0x7A。

职责边界：本模块只把字节读成结构，不做指令流的预编译。blocks / labels /
stats 里的块级统计归 converter/compile/precompile.py（公共事件与地图页共用同一
份实现，避免两套跳转表算法分叉）。这里通过可选的 compile_fn 注入，缺省时
blocks=[]、labels={}、stats 只给槽级三元组。

页里没有 "priority" 字段：mps.rs 的结构体与那 37 字节条件块都没有它，WOLF 的
显示优先级就是「页码越大越优先」（docs/FORMAT_SPEC.md §5.6.3），所以输出里保留
"priority": None，避免下游误以为漏读。
"""
from __future__ import annotations

import os
from collections import Counter

from ..reader import Reader, WolfEof
from ..schema import MAP_SELF_BASE, program as make_program
from .command import read_route, read_slot

# -- 常量（与 mps.rs 同名同值）--------------------------------------------
MAGIC = b"\x00" * 10 + b"WOLFM\x00"
MAGIC2 = b"\x00\x00\x00"
VERSION_HEADER = 0x55
UNKNOWN1 = 0x64  # 100
VERSIONS = (0x65, 0x66)
EMPTY_TILE = 0xFFFFFFFF
LAYER_COUNT = 3
EVENT_MAGIC = 0x6F
EVENT_MAGIC2 = 12345
EVENT_SEPARATOR = 0
EVENT_FOOTER = 0x70
PAGE_MAGIC = 0x79
PAGE_FOOTER = 0x7A
PAGE_FOOTER_TOLERATED = 0x00
FOOTER = 0x66
CONDITION_SLOTS = 4

#: 页激活条件的比较运算符（trigger_switches[i] 高 4 位）。
CONDITION_OPERATORS = {0: ">", 1: ">=", 2: "==", 3: "<=", 4: "<", 5: "!="}

#: docs/OPCODE_SPEC.md §0 的 50 种 opcode（公共事件 47 + 只在地图的 3）。
#: 只用于「未知 opcode」报告；解析本身对任何 opcode 都照常输出。
KNOWN_OPCODES = frozenset({
    99, 101, 102, 103, 104, 105, 106, 107, 111, 112, 121, 122, 123, 124, 125,
    126, 130, 140, 150, 151, 160, 161, 162, 170, 171, 172, 174, 175, 176, 179,
    180, 201, 210, 211, 212, 213, 220, 221, 222, 250, 270, 281, 290, 300, 401,
    420, 421, 498, 499, 1000,
})


class MapFormatError(ValueError):
    """.mps 结构损坏。消息里带文件/偏移，便于定位。"""


# -- 小工具 ---------------------------------------------------------------
def _expect_u8(r: Reader, want: int, what: str, file_name: str) -> None:
    at = r.pos
    got = r.u8()
    if got != want:
        raise MapFormatError(
            "%s: %s @ %d: 期望 0x%02X，实际 0x%02X" % (file_name, what, at, want, got)
        )


def _read_conditions(page: dict) -> list:
    """把 4 个条件槽解成可读结构：启用位在低 4 位，运算符在高 4 位。"""
    out = []
    for i in range(CONDITION_SLOTS):
        switch = page["trigger_switches"][i]
        enabled = (switch & 0x0F) == 1
        code = switch >> 4
        out.append({
            "slot": i,
            "enabled": enabled,
            "operator_code": code,
            "operator": CONDITION_OPERATORS.get(code),
            "variable": page["trigger_variables"][i],
            "value": page["trigger_values"][i],
        })
    return out


# -- 页 / 事件 ------------------------------------------------------------
def _read_page(r: Reader, warnings: list, file_name: str, event_id, page_index: int) -> dict:
    _expect_u8(r, PAGE_MAGIC, "page magic", file_name)
    page = {
        "graphic_chip_id": r.u32(),
        "graphic": r.t_str(),
        "direction": r.u8(),
        "frame": r.u8(),
        "opacity": r.u8(),
        "display_type": r.u8(),
        "trigger": r.u8(),
        "trigger_switches": [r.u8() for _ in range(CONDITION_SLOTS)],
        "trigger_variables": [r.u32() for _ in range(CONDITION_SLOTS)],
        "trigger_values": [r.i32() for _ in range(CONDITION_SLOTS)],
    }
    page["route"] = read_route(r)

    command_count = r.u32()
    if command_count > r.remaining() // 3:
        raise MapFormatError(
            "%s: 事件 %s 第 %d 页声明 %d 条指令，但只剩 %d 字节"
            % (file_name, event_id, page_index, command_count, r.remaining())
        )
    page["command_count"] = command_count
    page["slots"] = [read_slot(r, pc) for pc in range(command_count)]

    page["features"] = r.u32()
    page["shadow_graphic"] = r.u8()
    page["collision_width"] = r.u8()
    page["collision_height"] = r.u8()
    if page["features"] > 3:
        # features 的 bit2（page transfer）会多一个字节；本作不出现，按 mps.rs 跳过。
        warnings.append({
            "kind": "page_transfer_byte",
            "file": file_name,
            "event_id": event_id,
            "page": page_index,
            "offset": r.pos,
        })
        r.skip(1)

    at = r.pos
    footer = r.u8()
    if footer != PAGE_FOOTER:
        tolerated = footer == PAGE_FOOTER_TOLERATED and (
            r.at_end() or (r.remaining() >= 1 and r.buf[r.pos] in (EVENT_FOOTER, PAGE_MAGIC))
        )
        if not tolerated:
            raise MapFormatError(
                "%s: 事件 %s 第 %d 页 footer @ %d: 期望 0x%02X，实际 0x%02X"
                % (file_name, event_id, page_index, at, PAGE_FOOTER, footer)
            )
        warnings.append({
            "kind": "tolerated_page_footer",
            "value": footer,
            "expected": PAGE_FOOTER,
            "file": file_name,
            "event_id": event_id,
            "page": page_index,
            "offset": at,
        })
    page["footer"] = footer
    page["footer_offset"] = at
    page["conditions"] = _read_conditions(page)
    return page


def _read_event(r: Reader, warnings: list, file_name: str) -> dict:
    _expect_u8(r, EVENT_MAGIC, "event magic", file_name)
    at = r.pos
    magic2 = r.u32()
    if magic2 != EVENT_MAGIC2:
        raise MapFormatError(
            "%s: event magic2 @ %d: 期望 %d，实际 %d" % (file_name, at, EVENT_MAGIC2, magic2)
        )
    event = {"id": r.u32(), "name": r.t_str(), "x": r.u32(), "y": r.u32()}
    page_count = r.u32()
    at = r.pos
    separator = r.u32()
    if separator != EVENT_SEPARATOR:
        raise MapFormatError(
            "%s: 事件 %s separator @ %d: 期望 0，实际 %d"
            % (file_name, event["id"], at, separator)
        )
    if page_count > r.remaining() // 8:
        raise MapFormatError(
            "%s: 事件 %s 声明 %d 页，但只剩 %d 字节"
            % (file_name, event["id"], page_count, r.remaining())
        )
    event["page_count"] = page_count
    event["pages"] = [
        _read_page(r, warnings, file_name, event["id"], i) for i in range(page_count)
    ]
    _expect_u8(r, EVENT_FOOTER, "事件 %s footer" % event["id"], file_name)
    return event


# -- 文件 ----------------------------------------------------------------
def parse_map(data: bytes, file_name: str = "") -> dict:
    """把一个 .mps 的字节读成结构（不含 program 包装）。

    解析必须刚好走到文件末尾：footer 之后允许补零，出现非零字节即报错。
    """
    r = Reader(data)
    warnings: list = []
    try:
        magic = r.take(16)
        if magic != MAGIC:
            raise MapFormatError("%s: 文件头不是 WOLFM 地图 magic（@0）" % file_name)
        _expect_u8(r, VERSION_HEADER, "version_header", file_name)
        if r.take(3) != MAGIC2:
            raise MapFormatError("%s: magic2 不是 3 个 0x00（@17）" % file_name)
        unknown1 = r.u32()
        if unknown1 != UNKNOWN1:
            raise MapFormatError(
                "%s: unknown1 @20: 期望 %d，实际 %d" % (file_name, UNKNOWN1, unknown1)
            )
        version = r.u8()
        if version not in VERSIONS:
            raise MapFormatError("%s: 不支持的版本 0x%02X（@24）" % (file_name, version))

        out = {
            "file": file_name,
            "version_header": VERSION_HEADER,
            "version": version,
            "name": r.t_str(),
            "tileset_id": r.u32(),
            "width": r.u32(),
            "height": r.u32(),
        }
        event_count = r.u32()
        if event_count > r.remaining() // 8:
            raise MapFormatError(
                "%s: 头部声明 %d 个事件，但只剩 %d 字节"
                % (file_name, event_count, r.remaining())
            )
        out["event_count"] = event_count

        first_pixel = r.u32()
        out["has_tile_data"] = first_pixel != EMPTY_TILE
        if out["has_tile_data"]:
            pixels = out["width"] * out["height"] * LAYER_COUNT * 4
            needed = pixels - 4  # 首字已经读过
            if needed > r.remaining():
                raise MapFormatError(
                    "%s: 图层需要 %d 字节，文件只剩 %d 字节"
                    % (file_name, pixels, r.remaining())
                )
            # Layer-major tile words: whole layer0, then layer1, then layer2
            # (docs/FORMAT_SPEC.md §5.3). Terrain and the map layer consume these.
            count = pixels // 4
            words = [first_pixel]
            words.extend(r.u32() for _ in range(count - 1))
            stride = out["width"] * out["height"]
            out["layers"] = [words[i * stride:(i + 1) * stride] for i in range(LAYER_COUNT)]

        events = []
        while not r.at_end() and r.buf[r.pos] == EVENT_MAGIC:
            events.append(_read_event(r, warnings, file_name))
        if len(events) != event_count:
            raise MapFormatError(
                "%s: 头部声明 %d 个事件，实际读到 %d 个（@ %d）"
                % (file_name, event_count, len(events), r.pos)
            )
        out["events"] = events

        footer_at = r.pos
        _expect_u8(r, FOOTER, "file footer", file_name)
        out["footer"] = FOOTER
        out["footer_offset"] = footer_at
        tail_start = r.pos
        while not r.at_end():
            if r.u8() != 0:
                raise MapFormatError(
                    "%s: footer 之后 %d 处出现非零尾字节" % (file_name, r.pos - 1)
                )
        out["trailing_bytes"] = r.pos - tail_start
        out["size"] = len(data)
    except WolfEof as exc:
        raise MapFormatError("%s: 意外读到文件末尾（%s）" % (file_name, exc)) from exc

    warnings.extend({"kind": "utf8_decode", **w} for w in r.warnings)
    out["warnings"] = warnings
    return out


def slot_stats(slots: list) -> dict:
    """槽级对账三元组（块级统计由 converter/compile/precompile.py 提供）。"""
    blank = sum(1 for s in slots if not s.get("op"))
    return {
        "slots": len(slots),
        "blank_slots": blank,
        "real_instructions": len(slots) - blank,
    }


def to_programs(parsed: dict, map_id: int, compile_fn=None) -> list:
    """把 parse_map 的结果摊平成 program 列表：一个事件页 = 一个 program。

    compile_fn(slots) 若给出，返回值需含 blocks / labels / stats 三个键；缺省
    （None）时 blocks=[]、labels={}、stats 只有槽级三元组。
    """
    programs = []
    for event in parsed["events"]:
        for index, page in enumerate(event["pages"]):
            slots = page["slots"]
            if compile_fn is None:
                blocks, labels, stats = [], {}, slot_stats(slots)
            else:
                built = compile_fn(slots)
                blocks = built.get("blocks") or []
                labels = built.get("labels") or {}
                stats = built.get("stats") or slot_stats(slots)
            programs.append(make_program(
                "map_event",
                event["id"],
                event["name"],
                MAP_SELF_BASE,
                slots,
                blocks,
                labels,
                stats,
                page=index,
                page_count=event["page_count"],
                event_name=event["name"],
                x=event["x"],
                y=event["y"],
                priority=None,
                graphic=page["graphic"],
                graphic_chip_id=page["graphic_chip_id"],
                direction=page["direction"],
                frame=page["frame"],
                opacity=page["opacity"],
                display_type=page["display_type"],
                trigger=page["trigger"],
                trigger_conditions=page["conditions"],
                shadow_graphic=page["shadow_graphic"],
                collision_width=page["collision_width"],
                collision_height=page["collision_height"],
                features=page["features"],
                initial_move_route=page["route"],
            ))
    return programs


def load_map(path: str, map_id: int, compile_fn=None) -> dict:
    """读一个 .mps，返回 export/programs.py 需要的地图 dict。

    键：id / file / name / width / height / tileset_id / event_count /
    page_count / events / warnings / unknown_opcodes（外加 version、
    has_tile_data、size、trailing_bytes、footer_offset）。
    """
    file_name = os.path.basename(path)
    with open(path, "rb") as f:
        data = f.read()
    parsed = parse_map(data, file_name)
    programs = to_programs(parsed, map_id, compile_fn)

    unknown: Counter = Counter()
    for program in programs:
        for slot in program["slots"]:
            op = slot["op"]
            if op and op not in KNOWN_OPCODES:
                unknown[op] += 1
    warnings = list(parsed["warnings"])
    for op, count in sorted(unknown.items()):
        warnings.append({
            "kind": "unknown_opcode",
            "file": file_name,
            "op": op,
            "count": count,
        })

    return {
        "id": map_id,
        "file": file_name,
        "name": parsed["name"],
        "version": parsed["version"],
        "width": parsed["width"],
        "height": parsed["height"],
        "tileset_id": parsed["tileset_id"],
        "has_tile_data": parsed["has_tile_data"],
        "layers": parsed.get("layers", []),
        "size": parsed["size"],
        "trailing_bytes": parsed["trailing_bytes"],
        "footer_offset": parsed["footer_offset"],
        "event_count": parsed["event_count"],
        "page_count": sum(len(e["pages"]) for e in parsed["events"]),
        "events": programs,
        "unknown_opcodes": {str(op): count for op, count in sorted(unknown.items())},
        "warnings": warnings,
    }
