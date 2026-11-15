"""converter.parse.commonevent 的单元测试。

验证 CommonEvent.dat 的 550 个公共事件、163,828 槽、24,555 空槽、139,273 条真实指令与 47 种 opcode。
"""
from __future__ import annotations

import os
import struct
import tempfile
import unittest

from converter import schema
from converter.parse import commonevent

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DAT_PATH = os.path.join(ROOT, "work", "all", "BasicData", "CommonEvent.dat")
HAVE_DAT = os.path.isfile(DAT_PATH)


class TestCommonEventReal(unittest.TestCase):
    @unittest.skipUnless(HAVE_DAT, "需要 work/all/BasicData/CommonEvent.dat")
    def test_load_real_common_events(self):
        res = commonevent.load(DAT_PATH)
        self.assertEqual(res["version_header"], 0x55)
        self.assertEqual(res["version"], 0xC9)
        self.assertEqual(res["count"], 550)
        self.assertEqual(len(res["events"]), 550)
        self.assertEqual(res["unknown_opcodes"], {})

        counts = schema.count_slots(res["events"])
        self.assertEqual(counts["events"], 550)
        self.assertEqual(counts["slots"], 163828)
        self.assertEqual(counts["blank_slots"], 24555)
        self.assertEqual(counts["real_instructions"], 139273)
        self.assertEqual(counts["distinct_opcodes"], 47)

        # 检查 ID 连续且无空隙 0..=549
        ids = [ev["id"] for ev in res["events"]]
        self.assertEqual(ids, list(range(0, 550)))

        # 验证每个 program 契约
        for ev in res["events"]:
            errs = schema.validate_program(ev)
            self.assertEqual(errs, [])


class TestCommonEventSynthetic(unittest.TestCase):
    def test_magic_error(self):
        bad_data = b"NOTWOLF"
        with tempfile.NamedTemporaryFile(delete=False) as f:
            f.write(bad_data)
            f.flush()
            temp_path = f.name
        try:
            with self.assertRaises(commonevent.CommonEventFormatError):
                commonevent.load(temp_path)
        finally:
            os.unlink(temp_path)


if __name__ == "__main__":
    unittest.main()
