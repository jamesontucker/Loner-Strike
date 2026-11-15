#!/usr/bin/env python3
# Tests for scripts/community-index-check.py (the publish workflow's checks), with indexes the
# list service really builds:  cd services/serverlist && python3 -m unittest test_publish_check
import hashlib
import importlib.util
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import community as cm  # noqa: E402
from test_community import MAP, body, make, map_item  # noqa: E402

spec = importlib.util.spec_from_file_location("check", os.path.join(HERE, "..", "..", "scripts", "community-index-check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def index_with(n_listed):
    """An index the list service builds, with n listed maps."""
    c, _ = make()
    for i in range(1, n_listed + 1):
        page = json.loads(json.dumps(MAP))
        page["_idRow"] = i
        c.get.pages[("Mod", i)] = page
        c.submit(body(map_item(gb=i)), f"ip{i}")
    return cm.build_index(c.store)


class Check(unittest.TestCase):
    def run_check(self, data, sha_text=None, header=None, live=None, force=False):
        d = tempfile.mkdtemp()
        paths = [os.path.join(d, n) for n in ("index.json", "index.json.sha256", "live.json")]
        sha = hashlib.sha256(data).hexdigest()
        open(paths[0], "wb").write(data)
        open(paths[1], "w").write(sha_text if sha_text is not None else f"{sha}  index.json\n")
        args = [paths[0], paths[1], header if header is not None else sha]
        if live is not None:
            open(paths[2], "wb").write(live)
            args.append(paths[2])
        return check.main(args + (["--force"] if force else []))

    def test_a_real_index_passes(self):
        data, _ = index_with(3)
        self.assertEqual(self.run_check(data), 0)
        self.assertEqual(self.run_check(data, live=b""), 0, "nothing live yet")
        empty, _ = index_with(0)
        self.assertEqual(self.run_check(empty, live=empty), 3, "the same as live")

    def test_refusals(self):
        data, sha = index_with(2)
        self.assertEqual(self.run_check(data, sha_text="0" * 64), 1)
        self.assertEqual(self.run_check(data, header="0" * 64), 1)
        for bad in (b"<html>", b"[]", json.dumps({"schema": 2, "items": [], "revoked": []}).encode(),
                    json.dumps({"schema": 1, "items": [{"kind": "map"}], "revoked": []}).encode(),
                    json.dumps({"schema": 1, "items": [], "revoked": ["mod:x"]}).encode(),
                    json.dumps({"schema": 1, "items": [], "revoked": []}).encode() + b" " * (8 << 20)):
            with self.subTest(bad[:30]):
                self.assertEqual(self.run_check(bad), 1)

    def test_many_disappearing_at_once(self):
        ten, _ = index_with(10)
        six, _ = index_with(6)
        nine, _ = index_with(9)
        two, _ = index_with(2)
        one, _ = index_with(1)
        self.assertEqual(self.run_check(six, live=ten), 1, "4 of 10 at once")
        self.assertEqual(self.run_check(six, live=ten, force=True), 0, "unless forced by hand")
        self.assertEqual(self.run_check(nine, live=ten), 0, "one goes through")
        self.assertEqual(self.run_check(one, live=two), 0, "one of two")
        self.assertEqual(self.run_check(ten, live=six), 0, "more is fine")


if __name__ == "__main__":
    unittest.main()
