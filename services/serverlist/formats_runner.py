#!/usr/bin/env python3
"""Checks every file in one package, as a child process of the list service (community.py).

  python3 -I formats_runner.py <package file> <its name on GameBanana> <seconds>

Runs with its own limits (CPU time, address space, lower priority) so a hostile package can't
starve or crash the service; the service also kills it at a wall-clock limit. Each file in a zip
is streamed to its own temporary file (never whole into memory) and checked by formats.py.
Prints one line of JSON: {"ok": true, "files": n, "kinds": {...}} or {"ok": false, "error": "..."}.
"""
import hashlib
import json
import os
import struct
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
    sections = {}
    refs = set()
    files = {}  # models/ and sound/ files: sha256 (creature files are compared with their owner's)
    npc = []  # NPC-editor: the pack's NPC files, each named by the package's own maps or graphs
    texts = []  # (lower-cased) entity lumps, .map files and graphs, to find them in
    text_bytes = [0]

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
            data = fh.read(formats.CREATURES_MAX)
            creatures.extend(formats.creatures_ini_types(data))
            sections.update(formats.creatures_ini_sections(data))
            refs.update(formats.creatures_ini_refs(data))
            npc.extend(formats.creatures_ini_npc(data) or [])
        low = fname.replace("\\", "/").lower()
        if low.endswith((".bsp", ".map", ".graph.json")) and text_bytes[0] < 96 << 20:
            fh.seek(0)
            got = b""
            if low.endswith(".bsp"):
                head = fh.read(12)
                if len(head) == 12:
                    off, ln = struct.unpack("<II", head[4:12])
                    if ln <= 8 << 20 and off + ln <= size:
                        fh.seek(off)
                        got = fh.read(ln)
            elif size <= 64 << 20:
                got = fh.read(size)
            if got:
                texts.append(got.lower())
                text_bytes[0] += len(got)
        if low.startswith(("models/", "sprites/", "sound/")) and len(files) < 2048:
            fh.seek(0)
            h = hashlib.sha256()
            while True:
                b = fh.read(1 << 20)
                if not b:
                    break
                h.update(b)
            files[low] = h.hexdigest()

    if not name.lower().endswith(".zip"):
        with open(path, "rb") as fh:
            one(name, fh, os.path.getsize(path))
    else:
        with tempfile.TemporaryDirectory(prefix="ls-part-") as tmp, zipfile.ZipFile(path) as z:
            said = sum(i.file_size for i in z.infolist())  # [Caps]
            if said > TOTAL_MAX:
                raise Refused(f"The package unpacks to {said >> 20} MB; the most is {TOTAL_MAX >> 20} MB.")
            seen = set()
            for i in z.infolist():
                # one entry per name however it's spelled (case, \\, ./, //): a second would replace the first
                key = "/".join(x for x in i.filename.replace("\\", "/").lower().split("/") if x not in ("", "."))
                if key in seen:
                    raise Refused(f"The package has {i.filename} twice.")
                seen.add(key)
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
    # an NPC file the package's own maps and graphs don't name isn't one of its NPCs'
    for p in npc:
        forms = [p.encode()] + ([p[len("sound/"):].encode()] if p.startswith("sound/") else [])
        if not any(f in t for t in texts for f in forms):
            raise Refused(f"{p} isn't used by any of the package's NPCs. Make it again with Publish.")
    refs.update(npc)
    if npc:
        out["npcFiles"] = sorted(set(npc))
    if creatures or npc:
        out["creatures"] = creatures
        out["creatureSections"] = sections
        out["creatureRefs"] = sorted(refs)
        out["creatureFiles"] = files
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
