import datetime, os, sqlite3, tempfile, unittest
import backup


class Backup(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        db = sqlite3.connect(os.path.join(self.d, "community.db"))
        db.execute("PRAGMA journal_mode=WAL")
        db.execute("CREATE TABLE t(x)")
        db.execute("INSERT INTO t VALUES (42)")
        db.commit()
        self.live = db  # stays open, like the running service
        open(os.path.join(self.d, "list.json"), "w").write("{}")

    def tearDown(self):
        self.live.close()

    def test_consistent_copy_while_open_and_old_ones_pruned(self):
        out = os.path.join(self.d, "backups")
        os.makedirs(out)
        for name in ("community-2026-10-01.db", "list-2026-10-01.json", "community-2026-11-01.db", "notes.txt"):
            open(os.path.join(out, name), "w").close()
        made, removed = backup.backup(self.d, today=datetime.date(2026, 11, 13))
        self.assertEqual(sorted(os.path.basename(p) for p in made), ["community-2026-11-13.db", "list-2026-11-13.json"])
        self.assertEqual(sorted(removed), ["community-2026-10-01.db", "list-2026-10-01.json"])
        self.assertTrue(os.path.exists(os.path.join(out, "community-2026-11-01.db")), "inside 14 days: kept")
        self.assertTrue(os.path.exists(os.path.join(out, "notes.txt")), "other files untouched")
        copy = sqlite3.connect(made[0])
        self.assertEqual(copy.execute("SELECT x FROM t").fetchone()[0], 42)
        copy.close()
        self.assertEqual(oct(os.stat(made[0]).st_mode & 0o777), "0o600")
        self.assertFalse(any(n.endswith(".part") for n in os.listdir(out)))

    def test_same_day_twice_replaces(self):
        day = datetime.date(2026, 11, 13)
        backup.backup(self.d, today=day)
        self.live.execute("INSERT INTO t VALUES (7)")
        self.live.commit()
        made, _ = backup.backup(self.d, today=day)
        copy = sqlite3.connect(made[0])
        self.assertEqual(copy.execute("SELECT count(*) FROM t").fetchone()[0], 2)
        copy.close()

    def test_the_roles_database_too(self):
        r = sqlite3.connect(os.path.join(self.d, "roles.db"))
        r.execute("CREATE TABLE links (steamid TEXT)")
        r.execute("INSERT INTO links VALUES ('STEAM_0:1:1')")
        r.commit()
        out = os.path.join(self.d, "backups")
        os.makedirs(out, exist_ok=True)
        open(os.path.join(out, "roles-2026-10-01.db"), "w").close()
        made, removed = backup.backup(self.d, today=datetime.date(2026, 11, 13))
        self.assertIn("roles-2026-11-13.db", [os.path.basename(p) for p in made])
        self.assertIn("roles-2026-10-01.db", removed)
        copy = sqlite3.connect(os.path.join(out, "roles-2026-11-13.db"))
        self.assertEqual(copy.execute("SELECT steamid FROM links").fetchone()[0], "STEAM_0:1:1")
        copy.close()
        r.close()

    def test_nothing_yet(self):
        empty = tempfile.mkdtemp()
        self.assertEqual(backup.backup(empty), ([], []))


if __name__ == "__main__":
    unittest.main()
