"""Downloads (cached and checked) and unpacking."""

import hashlib
import os
import shutil
import subprocess
import tarfile
import urllib.request
import zipfile

CACHE = os.path.join(os.path.expanduser("~"), ".loner-strike", "downloads")
USER_AGENT = "Loner-Strike-installer/0.1"


class FetchError(Exception):
    pass


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url, sha=None, name=None, quiet=False):
    """Path to a cached copy of url, downloading it if needed. With sha, the
    file must match it exactly (a changed upstream file stops the install)."""
    os.makedirs(CACHE, exist_ok=True)
    name = name or hashlib.sha1(url.encode()).hexdigest()[:16] + "-" + os.path.basename(url.split("?")[0])
    path = os.path.join(CACHE, name)
    if os.path.exists(path) and (not sha or sha256(path) == sha):
        return path
    if not quiet:
        print("    downloading %s" % url)
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    tmp = path + ".part"
    try:
        with urllib.request.urlopen(req, timeout=60) as r, open(tmp, "wb") as f:
            shutil.copyfileobj(r, f)
    except OSError as e:
        raise FetchError("Couldn't download %s (%s)" % (url, e))
    if sha and sha256(tmp) != sha:
        os.remove(tmp)
        raise FetchError("%s isn't the file Loner-Strike was tested with (checksum differs)." % url)
    os.replace(tmp, path)
    return path


def unpack(archive, dest):
    """Unpacks zip, tar(.gz/.xz/.bz2); refuses paths that leave dest and skips links."""
    dest = os.path.abspath(dest)
    os.makedirs(dest, exist_ok=True)

    def safe(name):
        target = os.path.abspath(os.path.join(dest, name))
        if not (target == dest or target.startswith(dest + os.sep)):
            raise FetchError("%s contains an unsafe path: %s" % (archive, name))
        return target

    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as z:
            for m in z.infolist():
                name = m.filename.replace("\\", "/")
                target = safe(name)
                if m.is_dir():
                    os.makedirs(target, exist_ok=True)
                    continue
                os.makedirs(os.path.dirname(target), exist_ok=True)
                with z.open(m) as s, open(target, "wb") as t:
                    shutil.copyfileobj(s, t)
        return dest
    if tarfile.is_tarfile(archive):
        with tarfile.open(archive) as t:
            for m in t.getmembers():
                if not (m.isfile() or m.isdir()):
                    continue          # no links or devices
                target = safe(m.name)
                if m.isdir():
                    os.makedirs(target, exist_ok=True)
                    continue
                os.makedirs(os.path.dirname(target), exist_ok=True)
                with t.extractfile(m) as s, open(target, "wb") as o:
                    shutil.copyfileobj(s, o)
        return dest
    # rar / 7z: bsdtar ships with macOS and Windows 10+
    r = subprocess.run(["tar", "-xf", archive, "-C", dest], capture_output=True, text=True)
    if r.returncode != 0:
        raise FetchError("Couldn't unpack %s: %s" % (archive, r.stderr.strip()[-200:]))
    for d, dirs, files in os.walk(dest):
        for n in dirs + files:
            if os.path.islink(os.path.join(d, n)):
                os.unlink(os.path.join(d, n))
    return dest
