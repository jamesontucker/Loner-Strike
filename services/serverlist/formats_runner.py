#!/usr/bin/env python3
"""Checks every file in one package, as a child process of the list service (community.py).

  python3 -I formats_runner.py <package file> <its name on GameBanana> <seconds>

Runs with its own limits (CPU time, address space, lower priority) so a hostile package can't
starve or crash the service; the service also kills it at a wall-clock limit. Each file in a zip
is streamed to its own temporary file (never whole into memory) and checked by formats.py.
Prints one line of JSON: {"ok": true, "files": n, "kinds": {...}} or {"ok": false, "error": "..."}.
"""
import json
import os
import sys
import tempfile
import time
import zipfile

ENTRY_MAX = 256 << 20     # biggest single file inside a package
# [Caps] everything in one package together, unpacked (zip bombs: many entries that each stay
# under ENTRY_MAX). A 128 MB map pack unpacks to a few hundred MB; 1 GB matches the app's cap.
TOTAL_MAX = 1 << 30
CPU_SECONDS = 120
ADDRESS_SPACE = 256 << 20


def limit():
    try:
        import resource
        resource.setrlimit(resource.RLIMIT_CPU, (CPU_SECONDS, CPU_SECONDS + 5))
        resource.setrlimit(resource.RLIMIT_AS, (ADDRESS_SPACE, ADDRESS_SPACE))
    except (ImportError, ValueError, OSError):
        pass  # (macOS doesn't take RLIMIT_AS; the service's wall-clock kill still applies)
    try:
        os.nice(10)
    except OSError:
        pass


def run(path, name, seconds):
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import formats
    formats.set_deadline(time.monotonic() + seconds)
    kinds = {}
    creatures = []

    def one(fname, fh, size):
        try:
            k = formats.check(fname, fh, size)
        except formats.FormatError as e:
            raise Refused(f"{fname} didn't pass the checks: {e}.")
        if k:
            kinds[k] = kinds.get(k, 0) + 1
        if k == "creatures":
            # the types it defines (the service checks the listing against them)
            fh.seek(0)
            creatures.extend(formats.creatures_ini_types(fh.read(formats.CREATURES_MAX)))

    if not name.lower().endswith(".zip"):
        with open(path, "rb") as fh:
            one(name, fh, os.path.getsize(path))
    else:
        with tempfile.TemporaryDirectory(prefix="ls-part-") as tmp, zipfile.ZipFile(path) as z:
            said = sum(i.file_size for i in z.infolist())  # [Caps]
            if said > TOTAL_MAX:
                raise Refused(f"The package unpacks to {said >> 20} MB; the most is {TOTAL_MAX >> 20} MB.")
            for i in z.infolist():
                if i.is_dir():
                    continue
                if i.file_size > ENTRY_MAX:
                    raise Refused(f"{i.filename} is too big to check ({i.file_size >> 20} MB).")
                part = os.path.join(tmp, "part")
                got = 0
                with z.open(i) as src, open(part, "wb") as dst:
                    while True:
                        formats.tick()
                        chunk = src.read(1 << 20)
                        if not chunk:
                            break
                        got += len(chunk)
                        if got > i.file_size:  # more than it said: refuse (no zip bombs)
                            raise Refused(f"{i.filename} unpacks to more than the zip says.")
                        dst.write(chunk)
                with open(part, "rb") as fh:
                    one(i.filename, fh, got)
                os.remove(part)
    out = {"ok": True, "files": sum(kinds.values()), "kinds": kinds}
    if creatures:
        out["creatures"] = creatures
    return out


class Refused(Exception):
    pass


def main(argv):
    limit()
    try:
        out = run(argv[0], argv[1], float(argv[2]))
    except Refused as e:
        out = {"ok": False, "error": str(e)}
    except MemoryError:
        out = {"ok": False, "error": "The package needs too much memory to check."}
    except Exception as e:  # formats.FormatError from the deadline, damaged zips, anything else
        msg = str(e) if type(e).__name__ == "FormatError" else "The package couldn't be checked. Upload the zip the app made, unchanged."
        out = {"ok": False, "error": msg}
    sys.stdout.write(json.dumps(out) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
