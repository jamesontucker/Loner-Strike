#!/usr/bin/env python3
"""Puts a release's installers on Polar: the Mac .dmg and the Windows setup .exe are uploaded,
and the downloads benefit is changed to offer only those two (the files it had before are
taken off it and deleted).

  POLAR_ACCESS_TOKEN=... scripts/polar-release.py <folder with the release's files> <tag>

The benefit: POLAR_BENEFIT_ID if set, otherwise the one downloads benefit whose description
mentions Loner-Strike. The token needs benefits:read, benefits:write, files:read and
files:write (an Organization Access Token).
"""
import base64
import glob
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request

API = os.environ.get("POLAR_API", "https://api.polar.sh")
CHUNK = 10_000_000          # bytes per uploaded part
# (Polar's firewall turns away Python's own User-Agent)
UA = "Loner-Strike-release/1.0 (+https://github.com/jamesontucker/Loner-Strike)"
INSTALLERS = [("*_universal.dmg", "application/x-apple-diskimage"), ("*_x64-setup.exe", "application/x-msdownload")]


def fail(msg):
    print(f"::error::{msg}")
    sys.exit(1)


def call(method, path, body=None):
    req = urllib.request.Request(API + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                 headers={"Authorization": "Bearer " + os.environ["POLAR_ACCESS_TOKEN"],
                                          "Content-Type": "application/json", "Accept": "application/json", "User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            text = r.read().decode()
            return json.loads(text) if text else None
    except urllib.error.HTTPError as e:
        fail(f"Polar said {e.code} to {method} {path}: {e.read().decode()[:400]}")


def b64sha(data):
    return base64.b64encode(hashlib.sha256(data).digest()).decode()


def upload(path, mime, version):
    data = open(path, "rb").read()
    parts = []
    for n, start in enumerate(range(0, len(data), CHUNK), 1):
        end = min(start + CHUNK, len(data))
        parts.append({"number": n, "chunk_start": start, "chunk_end": end, "checksum_sha256_base64": b64sha(data[start:end])})
    f = call("POST", "/v1/files/", {"name": os.path.basename(path), "mime_type": mime, "size": len(data), "service": "downloadable",
                                    "version": version, "checksum_sha256_base64": b64sha(data), "upload": {"parts": parts}})
    done = []
    for p in f["upload"]["parts"]:
        req = urllib.request.Request(p["url"], method="PUT", data=data[p["chunk_start"]:p["chunk_end"]], headers=p.get("headers") or {})
        for attempt in range(4):
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    etag = r.headers.get("ETag")
                break
            except (urllib.error.URLError, TimeoutError) as e:
                if attempt == 3:
                    fail(f"Couldn't upload part {p['number']} of {os.path.basename(path)} ({e}).")
        if not etag:
            fail(f"Polar's storage didn't confirm part {p['number']} of {os.path.basename(path)}.")
        done.append({"number": p["number"], "checksum_etag": etag, "checksum_sha256_base64": p.get("checksum_sha256_base64")})
    call("POST", f"/v1/files/{f['id']}/uploaded", {"id": f["upload"]["id"], "path": f["upload"]["path"], "parts": done})
    print(f"uploaded {os.path.basename(path)} ({len(data)} bytes)")
    return f["id"]


def benefit():
    bid = os.environ.get("POLAR_BENEFIT_ID", "").strip()
    if bid:
        return call("GET", f"/v1/benefits/{bid}")
    found = [b for b in call("GET", "/v1/benefits/?type=downloadables&limit=100")["items"]
             if "loner-strike" in (b.get("description") or "").lower()]
    if len(found) != 1:
        fail(f"Found {len(found)} downloads benefits mentioning Loner-Strike: set the repository variable POLAR_BENEFIT_ID to the right one.")
    return found[0]


def main():
    folder, tag = sys.argv[1], sys.argv[2]
    if not os.environ.get("POLAR_ACCESS_TOKEN"):
        fail("POLAR_ACCESS_TOKEN isn't set.")
    files = []
    for pattern, mime in INSTALLERS:
        hits = glob.glob(os.path.join(folder, pattern))
        if len(hits) != 1:
            fail(f"Expected one {pattern} in the release, found {len(hits)}.")
        files.append((hits[0], mime))
    b = benefit()
    old = list(b["properties"].get("files") or [])
    new = [upload(path, mime, tag.lstrip("v")) for path, mime in files]
    call("PATCH", f"/v1/benefits/{b['id']}", {"type": "downloadables", "properties": {"files": new, "archived": {}}})
    print(f"benefit {b['description']!r} now offers {tag}'s installers")
    # the old ones: off the benefit already, and gone from Polar (a failure here is only noted)
    for fid in old:
        if fid in new:
            continue
        req = urllib.request.Request(f"{API}/v1/files/{fid}", method="DELETE", headers={"Authorization": "Bearer " + os.environ["POLAR_ACCESS_TOKEN"], "User-Agent": UA})
        try:
            urllib.request.urlopen(req, timeout=60).close()
            print(f"deleted old file {fid}")
        except urllib.error.HTTPError as e:
            print(f"::warning::Couldn't delete old file {fid} ({e.code}); it's no longer offered.")


if __name__ == "__main__":
    main()
