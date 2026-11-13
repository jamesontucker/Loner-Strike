"""Size caps on what the community service unpacks (docs/notes/Caps.md)."""
import os
import tempfile
import unittest
import zipfile

import formats_runner


class TotalUnpacked(unittest.TestCase):
    def zip_of(self, tmp, parts):
        path = os.path.join(tmp, "pack.zip")
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
            for name, data in parts:
                z.writestr(name, data)
        return path

    def test_many_small_entries_over_the_total_are_refused(self):
        old = formats_runner.TOTAL_MAX
        formats_runner.TOTAL_MAX = 3 << 20
        try:
            with tempfile.TemporaryDirectory() as tmp:
                # each entry is far under ENTRY_MAX; together they pass the total
                path = self.zip_of(tmp, [(f"maps/x{i}.txt", b"\0" * (1 << 20)) for i in range(4)])
                with self.assertRaises(formats_runner.Refused) as e:
                    formats_runner.run(path, "pack.zip", 30)
                self.assertIn("unpacks to 4 MB; the most is 3 MB", str(e.exception))
        finally:
            formats_runner.TOTAL_MAX = old

    def test_a_normal_package_under_the_total_passes_the_cap(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.zip_of(tmp, [("readme.txt", b"hello\n")])
            out = formats_runner.run(path, "pack.zip", 30)
            self.assertTrue(out["ok"])


class PackageCheckCap(unittest.TestCase):
    def test_real_mappacks_fit(self):
        import community
        # the biggest real CS 1.6 item found on GameBanana: a 132.7 MB multi-map pack
        self.assertGreaterEqual(community.CHECK_MAX, int(132.7 * (1 << 20)))
        self.assertEqual(community.CHECK_MAX, 256 << 20)
        # never more than one package's files are checked at a time (temp disk: package + one entry)
        self.assertLessEqual(community.CHECK_ENTRY_MAX, 256 << 20)


if __name__ == "__main__":
    unittest.main()
