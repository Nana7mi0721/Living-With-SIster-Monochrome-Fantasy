"""导出 save_import.json（读原版 8 个零售存档）。"""
from __future__ import annotations

import glob
import json
import os
import struct

from .. import schema

NOT_RECOVERABLE = [
    "event_instances",
    "pc",
    "call_stack",
    "wait",
    "audio_phase",
]


def decrypt(data: bytes) -> bytes:
    buf = bytearray(data)
    seeds = [buf[0], buf[3], buf[9]]
    for pass_idx, step in enumerate([1, 2, 5]):
        state = seeds[pass_idx]
        at = 0x14
        while at < len(buf):
            state = (state * 214013 + 2531011) & 0xFFFFFFFF
            rand = (state >> 16) & 0x7FFF
            buf[at] ^= (rand >> 12) & 0xFF
            at += step
    return bytes(buf)


class Reader:
    def __init__(self, buf: bytes, pos: int = 0):
        self.buf = buf
        self.pos = pos

    def u8(self) -> int:
        v = self.buf[self.pos]; self.pos += 1; return v

    def u16(self) -> int:
        v = struct.unpack("<H", self.buf[self.pos:self.pos + 2])[0]; self.pos += 2; return v

    def u32(self) -> int:
        v = struct.unpack("<I", self.buf[self.pos:self.pos + 4])[0]; self.pos += 4; return v

    def i32(self) -> int:
        v = struct.unpack("<i", self.buf[self.pos:self.pos + 4])[0]; self.pos += 4; return v

    def u64(self) -> int:
        v = struct.unpack("<Q", self.buf[self.pos:self.pos + 8])[0]; self.pos += 8; return v

    def mem_data(self, size: int) -> bytes:
        n = self.u8() if size == 1 else (self.u16() if size == 2 else self.u32())
        data = self.buf[self.pos:self.pos + n]
        self.pos += n
        return data

    def mem_data_str(self, size: int) -> str:
        b = self.mem_data(size)
        if not b:
            return ""
        if b[-1:] == b"\x00":
            b = b[:-1]
        return b.decode("utf-8", "replace")


def skip_object(r: Reader):
    r.u32(); r.u32(); r.u32(); r.u32(); r.u8(); r.u32()
    r.mem_data(2)
    for _ in range(4): r.u16()
    r.u8(); r.u8()
    for _ in range(2):
        for _ in range(6): r.u8()
        c = r.u32()
        for _ in range(c):
            r.u8()
            n1 = r.u8(); [r.u32() for _ in range(n1)]
            n2 = r.u8(); [r.u8() for _ in range(n2)]
    for _ in range(6): r.u16()
    for _ in range(3): r.u8()
    c = r.u32()
    if c < 0x80000000:
        for _ in range(c): r.u32()
    r.u8(); r.u8()
    for _ in range(4): r.u32()
    r.u32()
    for _ in range(3): r.u32()
    raw = r.u16()
    c = raw if raw < 0x8000 else 0
    for _ in range(c): r.mem_data(2)
    r.u32(); r.u8(); r.u32(); r.u32()
    r.u32(); r.u32()


def import_save_file(path: str, slot_index: int) -> dict:
    raw = open(path, "rb").read()
    dec = decrypt(raw)

    stored_chk = raw[2]
    computed_chk = sum(dec[0x14:]) & 0xFF
    checksum_ok = (stored_chk == computed_chk)

    r = Reader(dec, 0x15)
    _save_name = r.mem_data_str(2)
    _fv = r.u16()

    # Part1
    r.u32(); r.u32(); r.u32()
    mw, mh = r.u32(), r.u32()
    sent = r.u32()
    if sent != 0xFFFFFFFF:
        r.pos -= 4
        cells = mw * mh * 3
        for _ in range(cells): r.u32()
    chip_image = r.mem_data_str(4)
    for _ in range(31): r.mem_data(4)
    vars2_c = r.u32()
    switches = {}
    for i in range(vars2_c):
        val = r.u32()
        if val != 0:
            switches[str(i)] = True

    obj_c = r.u32()
    for _ in range(obj_c): skip_object(r)
    for _ in range(11): r.u32()
    vars3_c = r.u32()
    for _ in range(vars3_c): r.u32()

    # Part2
    r.u8(); r.u8()
    vars3_21 = [r.u32() for _ in range(19)]
    play_time_frames = vars3_21[0]
    sys3 = vars3_21[9]
    sys4 = vars3_21[10]

    for _ in range(3): r.u16()
    r.u32()
    for _ in range(3): r.u16()
    for _ in range(9): r.u32()
    r.u8(); r.u16()
    r.u32(); r.u32()
    for _ in range(4): r.u16()
    for _ in range(3): r.u32()
    r.u8(); r.u32(); r.u32(); r.u32()

    r.u32(); r.u32(); r.u8()
    c = r.u32(); [r.mem_data(4) for _ in range(c)]
    c = r.u32(); [r.u32() for _ in range(c)]
    for _ in range(5): r.u32()
    for _ in range(6): r.u32()
    r.u16()
    r.mem_data(2); r.mem_data(2)
    r.u32()
    for _ in range(3): r.u32()
    r.mem_data(2); [r.u32() for _ in range(3)]
    r.mem_data(2); [r.u32() for _ in range(3)]
    r.mem_data(2); r.mem_data(2); r.u8()
    r.u8()
    for _ in range(3): r.mem_data(2)
    for _ in range(5): r.u32()
    r.u32(); r.u32()
    r.u32(); r.u32()
    for _ in range(4): r.u32()
    r.u8(); r.u8()
    for _ in range(4): r.u32()
    for _ in range(24): r.u32()
    for _ in range(4): r.u32()
    r.mem_data(4)
    c = r.u32(); [r.mem_data(4) for _ in range(c)]

    # Part3
    r.u32()
    v2 = r.u32()
    for _ in range(v2):
        gc = r.u32()
        for _ in range(gc):
            per = r.u8(); [r.u32() for _ in range(per)]
    v3 = r.u32()
    normal_vars = {}
    for row_idx in range(v3):
        rc = r.u32()
        words = [r.i32() for _ in range(rc)]
        if row_idx == 0:
            normal_vars = {str(j): words[j] for j in range(len(words))}
    v4 = r.u32(); [r.mem_data(4) for _ in range(v4)]
    v5 = r.u32()
    for _ in range(v5):
        lc = r.u8(); [r.u32() for _ in range(lc)]
    v6 = r.u32()
    for _ in range(v6):
        sc = r.u8(); [r.mem_data(4) for _ in range(sc)]

    # Part4
    skip_object(r)
    p4_c = r.u32(); [skip_object(r) for _ in range(p4_c)]
    r.u8(); r.u8()
    p4_v2 = r.u32(); [r.u32() for _ in range(p4_v2)]
    p4_v3 = r.u32(); [r.u64() for _ in range(p4_v3)]

    # Part5
    p5_v1 = r.u16()
    pics = []
    if (p5_v1 & 0x8000) == 0:
        for _ in range(p5_v1):
            pic_id = r.u32()
            r.u8(); r.u8(); r.u16()
            is_text = r.u8()
            is_win = r.u8()
            pic_name = r.mem_data_str(2)
            for _ in range(12): r.u32()
            for _ in range(6): r.u32()
            for _ in range(18): r.u32()
            r.u8(); r.u8(); r.u8()
            for _ in range(4): r.u32()
            for _ in range(4): r.u32()
            for _ in range(4): r.u32()
            for _ in range(5): r.u32()
            for _ in range(6): r.u32()
            for _ in range(6): r.u32()
            for _ in range(3): r.u32()
            for _ in range(21): r.u32()
            for _ in range(5): r.u32()
            sub_count = r.u32()
            for _ in range(sub_count):
                slen = r.u32(); [r.u32() for _ in range(slen)]
            for _ in range(7): r.u32()
            pics.append({
                "id": pic_id if pic_id < 0x80000000 else pic_id - 0x100000000,
                "is_text": is_text == 1,
                "is_window": is_win == 1,
                "name": pic_name,
            })

    # VarDB
    r.u8()
    t_count = r.u32()
    db_overrides = []
    for t in range(t_count):
        unk = r.i32()
        dis = 0; fc = 0; cnt = unk
        if unk <= -1:
            if unk <= -2: dis = r.i32()
            fc = r.u32()
            cnt = fc
        cfg = [r.u32() for _ in range(cnt)] if cnt > 0 else []
        row_count = r.u32()
        if row_count > 0:
            db_overrides.append(t)
        for _ in range(row_count):
            for def_val in cfg:
                if def_val < 2000: r.i32()
            for def_val in cfg:
                if def_val >= 2000: r.mem_data(4)

    # Part7
    p7_v1 = r.u8()
    if p7_v1 == 1:
        c = r.u32()
        for _ in range(c):
            tag = r.u8()
            if tag < 0xFA: r.u8()
            else: r.u32()

    end_byte = r.u8()
    if end_byte != 0x19:
        raise ValueError(f"Bad end byte 0x{end_byte:02X}")

    secs = play_time_frames // 60
    s = secs % 60
    m = (secs // 60) % 60
    h = secs // 3600

    return {
        "index": slot_index,
        "file": os.path.basename(path),
        "bytes": len(raw),
        "checksum_ok": checksum_ok,
        "recovered": {
            "variables": normal_vars,
            "switches": switches,
            "pictures": pics,
            "play_time": {
                "frames": play_time_frames,
                "seconds": secs,
                "formatted": f"{h:02d}:{m:02d}:{s:02d}",
            },
            "map": {
                "width": mw,
                "height": mh,
                "chip_image": chip_image,
                "objects_count": obj_c,
            },
            "choice_anchor": [sys3, sys4],
            "database_overrides": db_overrides,
        },
        "not_recoverable": NOT_RECOVERABLE,
    }


def run(args) -> int:
    save_dir = os.path.join("work", "retail_backup", "Save")
    slots = []
    for slot in range(1, 9):
        p = os.path.join(save_dir, f"SaveData{slot:02d}.sav")
        if os.path.isfile(p):
            slots.append(import_save_file(p, slot))

    report = {
        "schema": "wolf.save_import",
        "schema_version": 1,
        "slots": slots,
    }
    os.makedirs(args.reports, exist_ok=True)
    report_path = os.path.join(args.reports, "save_import.json")
    info = schema.write_artifact(report, report_path, "save_import.json")
    print(json.dumps({"written": info, "slots": len(slots)}, ensure_ascii=False))
    return 0
