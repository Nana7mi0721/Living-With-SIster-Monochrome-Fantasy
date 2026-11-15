"""指令流读取：与 crates/wolf-format/src/command.rs 逐字节等价。

一个槽（slot）的布局：
    u8  param_count          0 = 空槽
    [u32 opcode, (param_count-1)*4 参数]   仅 param_count > 0
    u8  branch_depth
    u8  string_count
    t_str[string_count]
    [u8 tail_kind, RouteData]              仅 param_count > 0
"""
from __future__ import annotations

from ..schema import slot as make_slot
from ..reader import Reader

TAIL_NONE = 0
TAIL_MOVE_ROUTE = 1


def read_route(r: Reader) -> dict:
    route = {
        "animation_frequency": r.u8(),
        "move_speed": r.u8(),
        "move_frequency": r.u8(),
        "move_route_mode": r.u8(),
        "behavior_options": r.u8(),
        "route_options": r.u8(),
    }
    count = r.u32()
    steps = []
    for _ in range(count):
        kind = r.u8()
        n32 = r.u8()
        u32_args = r.u32_array(n32)
        n8 = r.u8()
        u8_args = list(r.take(n8))
        steps.append({"kind": kind, "u32_args": u32_args, "u8_args": u8_args})
    route["commands"] = steps
    return route


def read_slot(r: Reader, pc: int) -> dict:
    param_count = r.u8()
    if param_count == 0:
        r.u8()  # branch_depth：空槽也带
        n = r.u8()
        for _ in range(n):
            r.t_str_bytes()
        return make_slot(pc, 0, [], 0, [], None)

    op = r.u32()
    params = r.u32_array(param_count - 1)
    branch_depth = r.u8()
    string_count = r.u8()
    strings = [r.t_str() for _ in range(string_count)]
    tail = r.u8()
    if tail == TAIL_NONE:
        route = None
    elif tail == TAIL_MOVE_ROUTE:
        route = read_route(r)
    else:
        raise ValueError(f"unknown tail kind {tail} at pc {pc} (only 0 and 1 exist)")
    return make_slot(pc, op, params, branch_depth, strings, route)


def param_status(params: list) -> dict:
    """210/211/300 的 param_status 位域：params[1]（紧随第一个 u32）。"""
    word = params[1] if len(params) > 1 else 0
    return {
        "number_count": word & 0xF,
        "string_count": (word >> 4) & 0xF,
        "strings_enabled": [(word >> (12 + i)) & 1 for i in range(5)],
        "return_value_enabled": (word >> 24) & 1,
    }
