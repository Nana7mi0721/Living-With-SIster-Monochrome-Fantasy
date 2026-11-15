"""CommonEvent.dat 解析：与 crates/wolf-format/src/commonevent.rs 逐字节等价。

布局（全部小端、无填充）：
    [6]   MAGIC = [0x00, 0x57, 0x00, 0x00, 0x4F, 0x4C]
    [1]   version_header = 0x55
    [3]   MAGIC_TYPE = b"FC\0"
    [1]   version = 0xC9
    [4]   event_count
    [*]   event_count 个事件记录
    [1]   FOOTER = 0x90
    [*]   尾部填充，必须全 0

单事件记录：
    [1]   EVENT_START = 0x8E
    [4]   id
    [1]   raw_condition
    [4]   condition_variable
    [4]   condition_value
    [1]   arg_num_count
    [1]   arg_str_count
    [t]   title
    [4]   line_count
    [*]   line_count 个 Slot
    [5]   UNKNOWN_4 = [0x01, 0x00, 0x00, 0x00, 0x00]
    [t]   memo
    [1]   SEP_1 = 0x8F
    [*]   t_str_array arg_names
    [4]   arg_page_count
    [*]   take(arg_page_count) arg_special_spec
    [4]   string_page_count
    [*]   string_page_count 个 t_str_array arg_string_pages
    [4]   value_page_count
    [*]   value_page_count 个 t_u32_array arg_value_pages
    [*]   t_i32_array default_values
    [1]   SEP_2 = 0x90
    [4]   color
    [*]   100 个 t_str self_variable_names
    [1]   SEP_3 = 0x91
    [5]   UNKNOWN_4 = [0x01, 0x00, 0x00, 0x00, 0x00]
    [1]   SEP_4 = 0x92
    [t]   return_name
    [4]   return_value_id
    [1]   SEP_4 = 0x92
"""
from __future__ import annotations

import os
from collections import Counter

from ..reader import Reader, WolfEof
from ..schema import COMMON_SELF_BASE, program as make_program
from .command import read_slot

MAGIC = b"\x00\x57\x00\x00\x4F\x4C"
MAGIC_TYPE = b"FC\x00"
VERSION_HEADER = 0x55
VERSION = 0xC9
FOOTER = 0x90
EVENT_START = 0x8E
UNKNOWN_4 = b"\x01\x00\x00\x00\x00"
SEP_1 = 0x8F
SEP_2 = 0x90
SEP_3 = 0x91
SEP_4 = 0x92
SELF_VARIABLE_SLOTS = 100

KNOWN_OPCODES = frozenset({
    99, 101, 102, 103, 104, 105, 106, 107, 111, 112, 121, 122, 123, 124, 125,
    126, 130, 140, 150, 151, 160, 161, 162, 170, 171, 172, 174, 175, 176, 179,
    180, 201, 210, 211, 212, 213, 220, 221, 222, 250, 270, 281, 290, 300, 401,
    420, 421, 498, 499, 1000,
})


class CommonEventFormatError(ValueError):
    """CommonEvent.dat 结构损坏。"""


def _expect_bytes(r: Reader, want: bytes, what: str) -> None:
    at = r.pos
    got = r.take(len(want))
    if got != want:
        raise CommonEventFormatError(
            f"{what} @ {at}: 期望 {want!r}，实际 {got!r}"
        )


def _expect_u8(r: Reader, want: int, what: str) -> None:
    at = r.pos
    got = r.u8()
    if got != want:
        raise CommonEventFormatError(
            f"{what} @ {at}: 期望 0x{want:02X}，实际 0x{got:02X}"
        )


def slot_stats(slots: list) -> dict:
    blank = sum(1 for s in slots if not s.get("op"))
    return {
        "slots": len(slots),
        "blank_slots": blank,
        "real_instructions": len(slots) - blank,
    }


def parse_event(r: Reader, compile_fn=None) -> dict:
    _expect_u8(r, EVENT_START, "common event start marker")
    eid = r.u32()
    raw_condition = r.u8()
    condition_variable = r.u32()
    condition_value = r.u32()
    arg_num_count = r.u8()
    arg_str_count = r.u8()
    title = r.t_str()

    line_count = r.u32()
    if line_count > r.remaining() // 3:
        raise CommonEventFormatError(
            f"event {eid} declares {line_count} lines but only {r.remaining()} bytes remain"
        )
    slots = [read_slot(r, pc) for pc in range(line_count)]

    _expect_bytes(r, UNKNOWN_4, "common event unknown4")
    memo = r.t_str()

    _expect_u8(r, SEP_1, "common event separator 1")
    arg_names = r.t_str_array()

    arg_page_count = r.u32()
    arg_special_spec = list(r.take(arg_page_count))

    string_page_count = r.u32()
    arg_string_pages = [r.t_str_array() for _ in range(string_page_count)]

    value_page_count = r.u32()
    arg_value_pages = [r.t_u32_array() for _ in range(value_page_count)]

    default_values = r.t_i32_array()

    _expect_u8(r, SEP_2, "common event separator 2")
    color = r.u32()
    self_variable_names = [r.t_str() for _ in range(SELF_VARIABLE_SLOTS)]

    _expect_u8(r, SEP_3, "common event separator 3")
    _expect_bytes(r, UNKNOWN_4, "common event unknown5")

    _expect_u8(r, SEP_4, "common event separator 4")
    return_name = r.t_str()
    return_value_id = r.u32()
    _expect_u8(r, SEP_4, "common event separator 5")

    if compile_fn is None:
        blocks, labels, stats = [], {}, slot_stats(slots)
    else:
        built = compile_fn(slots)
        blocks = built.get("blocks") or []
        labels = built.get("labels") or {}
        stats = built.get("stats") or slot_stats(slots)

    run_condition = raw_condition & 0x0F
    condition_operator = (raw_condition >> 4) & 0x0F

    return make_program(
        "common_event",
        eid,
        title,
        COMMON_SELF_BASE,
        slots,
        blocks,
        labels,
        stats,
        memo=memo,
        return_name=return_name,
        return_value_id=return_value_id,
        run_condition_op_var_val=[run_condition, condition_operator, condition_variable, condition_value],
        raw_condition=raw_condition,
        args={
            "num": arg_num_count,
            "str": arg_str_count,
            "names": arg_names,
            "defaults": default_values,
            "special_spec": arg_special_spec,
            "string_pages": arg_string_pages,
            "value_pages": arg_value_pages,
        },
        color=color,
        self_variable_names=self_variable_names,
    )


def load(path: str, compile_fn=None) -> dict:
    with open(path, "rb") as f:
        data = f.read()

    r = Reader(data)
    _expect_bytes(r, MAGIC, "CommonEvent.dat magic")
    version_header = r.u8()
    if version_header != VERSION_HEADER:
        raise CommonEventFormatError(f"bad version header: 0x{version_header:02X}")
    _expect_bytes(r, MAGIC_TYPE, "CommonEvent.dat type suffix")
    version = r.u8()
    if version != VERSION:
        raise CommonEventFormatError(f"bad version: 0x{version:02X}")

    count = r.u32()
    if count > r.remaining() // 8:
        raise CommonEventFormatError(f"{count} events do not fit in {r.remaining()} bytes")

    events = []
    for _ in range(count):
        events.append(parse_event(r, compile_fn=compile_fn))

    footer = r.u8()
    if footer != FOOTER:
        raise CommonEventFormatError(f"bad footer: 0x{footer:02X}")

    while not r.at_end():
        b = r.u8()
        if b != 0:
            raise CommonEventFormatError(f"trailing non-zero byte 0x{b:02X} at {r.pos - 1}")

    warnings = [{"kind": "utf8_decode", **w} for w in r.warnings]

    ops_counter = Counter()
    for ev in events:
        for s in ev["slots"]:
            op = s.get("op")
            if op:
                ops_counter[op] += 1

    unknown = {op: cnt for op, cnt in ops_counter.items() if op not in KNOWN_OPCODES}

    return {
        "version_header": version_header,
        "version": version,
        "count": count,
        "events": events,
        "warnings": warnings,
        "unknown_opcodes": unknown,
    }
