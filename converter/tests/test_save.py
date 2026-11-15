"""converter.export.save 的单元测试。"""
from __future__ import annotations

import os
import unittest

from converter.export import save

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SAVE_DIR = os.path.join(ROOT, "work", "retail_backup", "Save")
HAVE_SAVES = os.path.isdir(SAVE_DIR)


class TestSaveImport(unittest.TestCase):
    @unittest.skipUnless(HAVE_SAVES, "需要 work/retail_backup/Save")
    def test_import_all_retail_saves(self):
        for slot in range(1, 9):
            p = os.path.join(SAVE_DIR, f"SaveData{slot:02d}.sav")
            res = save.import_save_file(p, slot)
            self.assertEqual(res["index"], slot)
            self.assertTrue(res["checksum_ok"])
            self.assertEqual(len(res["recovered"]["variables"]), 800)
            self.assertGreater(len(res["recovered"]["pictures"]), 0)
            self.assertGreater(res["recovered"]["play_time"]["frames"], 0)
            self.assertEqual(res["not_recoverable"], save.NOT_RECOVERABLE)


if __name__ == "__main__":
    unittest.main()
