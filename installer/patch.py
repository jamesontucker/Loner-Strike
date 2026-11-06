"""Unified diffs: making them (for maintainers) and applying them (at install time).

Used for mods whose upstream code has no license: the repo ships only
Loner-Strike's changes, and the installer applies them to the original files
it downloads. Line endings are kept exactly (CZ files often use CRLF).
"""

import difflib
import re

NO_EOL = "\\ No newline at end of file"


def _lines(text):
    return text.splitlines(keepends=True)


def make(old, new, path):
    """A unified diff turning old into new, for the file at path."""
    out = []
    for line in difflib.unified_diff(_lines(old), _lines(new), "a/" + path, "b/" + path, n=3):
        if line.startswith(("---", "+++", "@@")):
            out.append(line.rstrip("\r\n") + "\n")
        elif line.endswith("\n"):
            out.append(line)
        else:                       # last line of a file without a newline
            out.append(line + "\n" + NO_EOL + "\n")
    return "".join(out)


class PatchError(Exception):
    pass


def parse(diff):
    """{path: [(old_start, [(op, line)])]} for each file in the diff.

    Each hunk's header says how many old and new lines it has, so hunks are
    read by count: a removed line that happens to start with "--" can't be
    mistaken for the next file's header.
    """
    files, path = {}, None
    lines = diff.splitlines(keepends=True)
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("+++ "):
            path = line[4:].strip()
            path = path[2:] if path.startswith("b/") else path
            files.setdefault(path, [])
            i += 1
            continue
        m = re.match(r"@@ -(\d+)(?:,(\d+))? \+\d+(?:,(\d+))? @@", line)
        if not m:
            i += 1
            continue
        old_left = int(m.group(2) if m.group(2) is not None else 1)
        new_left = int(m.group(3) if m.group(3) is not None else 1)
        body = []
        i += 1
        while i < len(lines) and (old_left > 0 or new_left > 0):
            op, text = lines[i][:1], lines[i][1:]
            if op not in (" ", "-", "+"):
                raise PatchError("malformed patch near: %r" % lines[i][:60])
            if i + 1 < len(lines) and lines[i + 1].rstrip("\r\n") == NO_EOL:
                text = text[:-1] if text.endswith("\n") else text
                i += 1
            body.append((op, text))
            old_left -= op != "+"
            new_left -= op != "-"
            i += 1
        files[path].append((int(m.group(1)), body))
    return files


def apply(text, hunks):
    """Applies one file's hunks; each must match exactly (searching nearby if lines moved)."""
    src = _lines(text)
    out, pos = [], 0
    for start, body in hunks:
        old = [l for op, l in body if op != "+"]
        new = [l for op, l in body if op != "-"]
        want = max(start - 1, 0)
        at = None
        for delta in range(0, len(src) + 1):
            for cand in (want + delta, want - delta):
                if pos <= cand <= len(src) - len(old) and src[cand:cand + len(old)] == old:
                    at = cand
                    break
            if at is not None:
                break
        if at is None:
            raise PatchError("a change near line %d doesn't fit the downloaded file" % start)
        out += src[pos:at] + new
        pos = at + len(old)
    return "".join(out + src[pos:])
