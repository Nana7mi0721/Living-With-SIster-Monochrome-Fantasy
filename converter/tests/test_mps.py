"""converter.parse.mps 的单元测试。

真实数据对账（9 张 .mps 共 73 事件 / 134 页 / 37,610 槽 / 7,052 空槽 /
30,558 实指令 / 40 种 opcode）来自 docs/BASELINE.json；合成字节流覆盖错误路径。
仓库里没有 work/all/MapData 时真实数据部分自动跳过。

跑法（仓库根目录）：python -m unittest converter.tests.test_mps -v
"""
from __future__ import annotations

import os
import struct
import tempfile
import unittest

from converter import schema
from converter.parse import mps

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MAP_DIR = os.path.join(ROOT, "work", "all", "MapData")
HAVE_MAPS = os.path.isdir(MAP_DIR)

#: docs/BASELINE.json 的 map_events.per_map。
EXPECTED_MAPS = {
    "Dungeon.mps": dict(w=40, h=30, tileset=0, empty=False, events=5, pages=6, commands=79),
    "Map002.mps": dict(w=100, h=30, tileset=4, empty=False, events=5, pages=7, commands=416),
    "Map003.mps": dict(w=70, h=170, tileset=4, empty=False, events=2, pages=2, commands=45),
    "Map024.mps": dict(w=53, h=30, tileset=4, empty=True, events=1, pages=1, commands=3),
    "Map031.mps": dict(w=53, h=30, tileset=0, empty=True, events=1, pages=1, commands=63),
    "SampleMapA.mps": dict(w=64, h=36, tileset=4, empty=False, events=40, pages=93, commands=31083),
    "SampleMapA_1.mps": dict(w=64, h=36, tileset=4, empty=False, events=12, pages=17, commands=5752),
    "SampleMapB.mps": dict(w=30, h=30, tileset=0, empty=False, events=6, pages=6, commands=36),
    "TitleMap.mps": dict(w=53, h=30, tileset=4, empty=False, events=1, pages=1, commands=133),
}

EXPECTED_TOTALS = dict(
    events=73, pages=134, slots=37610, blank=7052, real=30558, opcodes=40
)


# -- 合成字节流 -----------------------------------------------------------
def t_str(value: str) -> bytes:
    raw = value.encode("utf-8") + b"\x00"
    return struct.pack("<I", len(raw)) + raw


def build_route() -> bytes:
    return bytes([0, 0, 0, 0, 0, 0]) + struct.pack("<I", 0)


def build_slot(op: int = 0, params=(), branch: int = 0, strings=(), tail: int = 0) -> bytes:
    if not op:
        return bytes([0, branch, len(strings)]) + b"".join(t_str(s) for s in strings)
    out = bytearray([len(params) + 1])
    out += struct.pack("<I", op)
    for value in params:
        out += struct.pack("<I", value)
    out += bytes([branch, len(strings)])
    for s in strings:
        out += t_str(s)
    out.append(tail)
    return bytes(out)


def build_page(slots=(), graphic: str = "", features: int = 0,
               footer: int = mps.PAGE_FOOTER) -> bytes:
    out = bytearray([mps.PAGE_MAGIC])
    out += struct.pack("<I", 0)
    out += t_str(graphic)
    out += bytes([2, 0, 255, 0, 0])          # direction/frame/opacity/display/trigger
    out += bytes([0, 0, 0, 0])               # trigger switches
    out += struct.pack("<4I", 0, 0, 0, 0)    # trigger variables
    out += struct.pack("<4i", 0, 0, 0, 0)    # trigger values
    out += build_route()
    out += struct.pack("<I", len(slots))
    for item in slots:
        out += item
    out += struct.pack("<I", features)
    out += bytes([0, 1, 1])                  # shadow, collision w, collision h
    out.append(footer)
    return bytes(out)


def build_event(eid: int = 1, name: str = "ev", x: int = 0, y: int = 0,
                pages=()) -> bytes:
    out = bytearray([mps.EVENT_MAGIC])
    out += struct.pack("<I", mps.EVENT_MAGIC2)
    out += struct.pack("<I", eid)
    out += t_str(name)
    out += struct.pack("<II", x, y)
    out += struct.pack("<I", len(pages))
    out += struct.pack("<I", 0)
    for page in pages:
        out += page
    out.append(mps.EVENT_FOOTER)
    return bytes(out)


def build_map(events=(), width: int = 1, height: int = 1, tiles: bool = False,
              version: int = 0x66, trailing: bytes = b"", magic: bytes = None,
              event_count: int = None) -> bytes:
    out = bytearray(magic if magic is not None else mps.MAGIC)
    out.append(mps.VERSION_HEADER)
    out += mps.MAGIC2
    out += struct.pack("<I", mps.UNKNOWN1)
    out.append(version)
    out += t_str("テスト")
    out += struct.pack("<I", 4)
    out += struct.pack("<II", width, height)
    out += struct.pack("<I", len(events) if event_count is None else event_count)
    if tiles:
        out += struct.pack("<I", 0)
        out += b"\x00" * (width * height * 3 * 4 - 4)
    else:
        out += struct.pack("<I", mps.EMPTY_TILE)
    for event in events:
        out += event
    out.append(mps.FOOTER)
    out += trailing
    return bytes(out)


class HeaderTest(unittest.TestCase):
    def test_minimal_map_roundtrip(self):
        parsed = mps.parse_map(build_map(), "t.mps")
        self.assertEqual(parsed["version"], 0x66)
        self.assertEqual(parsed["name"], "テスト")
        self.assertEqual(parsed["tileset_id"], 4)
        self.assertEqual((parsed["width"], parsed["height"]), (1, 1))
        self.assertEqual(parsed["event_count"], 0)
        self.assertEqual(parsed["events"], [])
        self.assertFalse(parsed["has_tile_data"])
        self.assertEqual(parsed["footer"], 0x66)
        self.assertEqual(parsed["trailing_bytes"], 0)

    def test_tile_data_is_skipped_over(self):
        data = build_map(width=3, height=2, tiles=True)
        parsed = mps.parse_map(data, "tiles.mps")
        self.assertTrue(parsed["has_tile_data"])
        self.assertEqual(parsed["size"], len(data))
        self.assertEqual(parsed["footer_offset"], len(data) - 1)

    def test_trailing_zeroes_are_allowed(self):
        parsed = mps.parse_map(build_map(trailing=b"\x00" * 3), "t.mps")
        self.assertEqual(parsed["trailing_bytes"], 3)

    def test_non_zero_trailing_byte_is_rejected(self):
        with self.assertRaises(mps.MapFormatError) as ctx:
            mps.parse_map(build_map(trailing=b"\x00\x07"), "t.mps")
        self.assertIn("非零尾字节", str(ctx.exception))

    def test_bad_magic(self):
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(build_map(magic=b"\x00" * 16), "t.mps")

    def test_bad_unknown1_and_version(self):
        good = bytearray(build_map())
        bad = bytearray(good)
        bad[20:24] = struct.pack("<I", 7)
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(bytes(bad), "t.mps")
        bad = bytearray(good)
        bad[24] = 0x64
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(bytes(bad), "t.mps")

    def test_truncated_header(self):
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(build_map()[:20], "t.mps")


class PageTest(unittest.TestCase):
    def test_slot_fields(self):
        page = build_page([build_slot(101, [7, 9]), build_slot()])
        parsed = mps.parse_map(build_map([build_event(pages=[page])]), "t.mps")
        slots = parsed["events"][0]["pages"][0]["slots"]
        self.assertEqual(slots[0]["op"], 101)
        self.assertEqual(slots[0]["params"], [7, 9])
        self.assertEqual(slots[1]["op"], 0)
        self.assertEqual(mps.slot_stats(slots)["blank_slots"], 1)

    def test_command_count_mismatch_is_rejected(self):
        page = bytearray(build_page([build_slot(101, [1])]))
        # command_count 就在 RouteData 之后；把 1 改成 999。
        offset = page.index(struct.pack("<I", 1), 60)
        page[offset:offset + 4] = struct.pack("<I", 999)
        with self.assertRaises(mps.MapFormatError) as ctx:
            mps.parse_map(build_map([build_event(pages=[bytes(page)])]), "t.mps")
        self.assertIn("条指令", str(ctx.exception))

    def test_tolerated_zero_page_footer_before_event_footer(self):
        data = build_map([build_event(pages=[build_page(footer=0x00)])])
        parsed = mps.parse_map(data, "t.mps")
        kinds = [w["kind"] for w in parsed["warnings"]]
        self.assertIn("tolerated_page_footer", kinds)

    def test_zero_page_footer_before_next_page_is_tolerated(self):
        pages = [build_page(footer=0x00), build_page()]
        parsed = mps.parse_map(build_map([build_event(pages=pages)]), "t.mps")
        self.assertEqual(len(parsed["events"][0]["pages"]), 2)

    def test_zero_page_footer_followed_by_junk_is_rejected(self):
        page = build_page(footer=0x00)
        event = bytearray(build_event(pages=[page]))
        event[-1] = 0x11  # 事件 footer 位置放垃圾
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(build_map([bytes(event)]), "t.mps")

    def test_event_count_mismatch_is_rejected(self):
        # 头部声明 2 个事件但只给 1 个
        data = build_map([build_event(pages=[build_page()])], event_count=2)
        with self.assertRaises(mps.MapFormatError):
            mps.parse_map(data, "t.mps")


class ProgramTest(unittest.TestCase):
    def test_program_shape_without_compiler(self):
        page = build_page([build_slot(150, [3])], graphic="chip")
        parsed = mps.parse_map(build_map([build_event(eid=5, name="とびら", pages=[page])]), "t.mps")
        programs = mps.to_programs(parsed, 42)
        self.assertEqual(len(programs), 1)
        program = programs[0]
        self.assertEqual(program["kind"], "map_event")
        self.assertEqual(program["id"], 5)
        self.assertEqual(program["title"], "とびら")
        self.assertEqual(program["self_base"], 100)
        self.assertEqual(program["blocks"], [])
        self.assertEqual(program["labels"], {})
        self.assertEqual(program["graphic"], "chip")
        self.assertEqual(program["page"], 0)
        self.assertEqual(program["priority"], None)
        self.assertEqual(program["stats"]["slots"], 1)
        schema.validate_program(program)

    def test_compile_fn_is_used_when_given(self):
        page = build_page([build_slot(101, [1])])
        parsed = mps.parse_map(build_map([build_event(pages=[page])]), "t.mps")

        def fake(slots):
            return {"blocks": [{"id": 0, "head": 0, "end": len(slots)}],
                    "labels": {"L0": 0},
                    "stats": {"slots": len(slots), "blank_slots": 0, "real_instructions": 1}}

        program = mps.to_programs(parsed, 1, fake)[0]
        self.assertEqual(program["labels"], {"L0": 0})
        self.assertEqual(program["stats"]["real_instructions"], 1)

    def test_unknown_opcode_is_recorded(self):
        page = build_page([build_slot(7777, [1])])
        data = build_map([build_event(pages=[page])])
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "Map777.mps")
            with open(path, "wb") as handle:
                handle.write(data)
            result = mps.load_map(path, 777)
        self.assertEqual(result["unknown_opcodes"], {"7777": 1})
        kinds = [w["kind"] for w in result["warnings"]]
        self.assertIn("unknown_opcode", kinds)

    def test_known_opcode_is_not_flagged(self):
        page = build_page([build_slot(101, [1])])
        data = build_map([build_event(pages=[page])])
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "Map101.mps")
            with open(path, "wb") as handle:
                handle.write(data)
            result = mps.load_map(path, 101)
        self.assertEqual(result["unknown_opcodes"], {})


@unittest.skipUnless(HAVE_MAPS, "work/all/MapData 不存在")
class RealMapsTest(unittest.TestCase):
    """按 docs/BASELINE.json 逐图对账。"""

    @classmethod
    def setUpClass(cls):
        cls.maps = {}
        for name in sorted(EXPECTED_MAPS):
            path = os.path.join(MAP_DIR, name)
            cls.maps[name] = mps.load_map(path, 0)
            cls.parsed = getattr(cls, "parsed", {})
            with open(path, "rb") as handle:
                cls.parsed[name] = mps.parse_map(handle.read(), name)

    def test_per_map_headers(self):
        for name, want in sorted(EXPECTED_MAPS.items()):
            with self.subTest(map=name):
                got = self.maps[name]
                self.assertEqual(got["width"], want["w"])
                self.assertEqual(got["height"], want["h"])
                self.assertEqual(got["tileset_id"], want["tileset"])
                self.assertEqual(got["has_tile_data"], not want["empty"])
                self.assertEqual(got["event_count"], want["events"])

    def test_per_map_pages(self):
        for name, want in sorted(EXPECTED_MAPS.items()):
            with self.subTest(map=name):
                got = self.maps[name]
                self.assertEqual(got["page_count"], want["pages"])
                self.assertEqual(len({p["id"] for p in got["events"]}), want["events"])

    def test_per_map_command_totals(self):
        for name, want in sorted(EXPECTED_MAPS.items()):
            with self.subTest(map=name):
                total = sum(len(p["slots"]) for p in self.maps[name]["events"])
                self.assertEqual(total, want["commands"])

    def test_file_is_fully_consumed(self):
        for name in sorted(EXPECTED_MAPS):
            with self.subTest(map=name):
                parsed = self.parsed[name]
                self.assertEqual(parsed["footer"], 0x66)
                self.assertEqual(parsed["trailing_bytes"], 0)
                self.assertEqual(parsed["footer_offset"] + 1, parsed["size"])

    def test_page_footers_are_0x7a_or_tolerated_zero(self):
        for name in sorted(EXPECTED_MAPS):
            with self.subTest(map=name):
                zeroes = 0
                for event in self.parsed[name]["events"]:
                    for page in event["pages"]:
                        self.assertIn(page["footer"], (0x7A, 0x00))
                        zeroes += page["footer"] == 0x00
                tolerated = sum(1 for w in self.maps[name]["warnings"]
                                if w["kind"] == "tolerated_page_footer")
                self.assertEqual(zeroes, tolerated)

    def test_totals_match_baseline(self):
        programs = [p for m in self.maps.values() for p in m["events"]]
        stats = schema.count_slots(programs)
        ops = {s["op"] for p in programs for s in p["slots"] if s["op"]}
        # count_slots 按 program 计，一个事件页一个 program，所以这里是页数。
        self.assertEqual(stats["events"], EXPECTED_TOTALS["pages"])
        self.assertEqual(sum(m["event_count"] for m in self.maps.values()),
                         EXPECTED_TOTALS["events"])
        self.assertEqual(sum(m["page_count"] for m in self.maps.values()),
                         EXPECTED_TOTALS["pages"])
        self.assertEqual(stats["slots"], EXPECTED_TOTALS["slots"])
        self.assertEqual(stats["blank_slots"], EXPECTED_TOTALS["blank"])
        self.assertEqual(stats["real_instructions"], EXPECTED_TOTALS["real"])
        self.assertEqual(len(ops), EXPECTED_TOTALS["opcodes"])

    def test_no_unknown_opcodes_and_all_programs_valid(self):
        for name, got in sorted(self.maps.items()):
            with self.subTest(map=name):
                self.assertEqual(got["unknown_opcodes"], {})
                for program in got["events"]:
                    schema.validate_program(program)
                    self.assertEqual(program["title"], program["event_name"])
                    self.assertEqual(program["self_base"], 100)


if __name__ == "__main__":
    unittest.main()
