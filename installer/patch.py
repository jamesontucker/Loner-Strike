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
    """{path: [(old_start, [(op, line)])]} for each file in the diff."""
    files, path, hunk = {}, None, None
    lines = diff.splitlines(keepends=True)
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("+++ "):
            path = line[4:].strip()
            path = path[2:] if path.startswith("b/") else path
            files[path] = []
        elif line.startswith("@@"):
            m = re.match(r"@@ -(\d+)(?:,\d+)? \+\d+(?:,\d+)? @@", line)
            hunk = (int(m.group(1)), [])
            files[path].append(hunk)
        elif hunk is not None and line[:1] in (" ", "-", "+"):
            body = line[1:]
            if i + 1 < len(lines) and lines[i + 1].rstrip("\r\n") == NO_EOL:
                body = body[:-1] if body.endswith("\n") else body
                i += 1
            hunk[1].append((line[0], body))
        i += 1
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
