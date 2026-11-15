#!/usr/bin/env bash
# Uploads one Loner-Strike server's fast download files to the download mirror (Cloudflare R2,
# dl.lonerstrike.com/fastdl/<server id>/), so players get its custom maps, models and sounds over
# HTTP. The app prepares the files first (the server's Rotation & extras > Fast downloads >
# Prepare files): only the server's custom files, never the game's own, copied into
#   ~/Library/Application Support/Loner-Strike/fastdl/<server id>/   (files/ + manifest.json)
# This uploads what changed since last time (by SHA-256), deletes what the server doesn't have
# any more, and writes the mirror's manifest.json last (with each file's ETag, for the app's
# "Check fast downloads").
#
#   scripts/r2-fastdl.sh <server id>             upload what changed
#   scripts/r2-fastdl.sh <server id> --dry-run   only say what it would do
#   scripts/r2-fastdl.sh <server id> --bzip2     also upload a .bz2 of each file (see below)
#   scripts/r2-fastdl.sh <server id> --from DIR  a prepared folder somewhere else
#   scripts/r2-fastdl.sh <server id> --to-dir DIR
#       write to a local folder laid out like the mirror instead of R2 (a fake mirror for tests:
#       serve DIR with any web server and point LS_FASTDL_MIRROR at <url>/fastdl). No keys used.
#
# .bz2: Source engine games fetch "<file>.bz2" first; GoldSrc's own downloader is not known to,
# so the plain file always goes up too and --bzip2 only adds the compressed copy.
#
# Keys: ~/.config/lonerstrike/r2.env (R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_ENDPOINT,
# R2_BUCKET), readable only by you. Never in a repo, never on a server machine.
set -euo pipefail

id="${1:-}"
shift || true
case "$id" in
  ""|-*) echo "Usage: scripts/r2-fastdl.sh <server id> [--dry-run] [--bzip2] [--from DIR] [--to-dir DIR]" >&2; exit 2 ;;
esac
[[ "$id" =~ ^[A-Za-z0-9-]{1,48}$ ]] || { echo "That isn't a server id (letters, digits and -)." >&2; exit 2; }

dry=0 bz=0 from="" todir=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1 ;;
    --bzip2) bz=1 ;;
    --from) from="${2:?--from needs a folder}"; shift ;;
    --to-dir) todir="${2:?--to-dir needs a folder}"; shift ;;
    *) echo "Unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
[ -n "$from" ] || from="$HOME/Library/Application Support/Loner-Strike/fastdl/$id"
[ -f "$from/manifest.json" ] || { echo "Nothing prepared in $from: use Prepare files in the app first." >&2; exit 1; }

if [ -z "$todir" ]; then
  env_file="$HOME/.config/lonerstrike/r2.env"
  [ -f "$env_file" ] || { echo "No keys: make $env_file first (see the comment at the top)." >&2; exit 1; }
  # shellcheck disable=SC1090
  . "$env_file"
  : "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}" "${R2_ENDPOINT:?}" "${R2_BUCKET:?}"
  export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY AWS_DEFAULT_REGION=auto
  command -v aws >/dev/null || { echo "Needs the AWS CLI (brew install awscli)." >&2; exit 1; }
fi

export FD_ID="$id" FD_FROM="$from" FD_TODIR="$todir" FD_DRY="$dry" FD_BZ="$bz"
exec python3 -I - <<'PY'
import hashlib, json, os, re, stat, subprocess, sys, tempfile, bz2, shutil

sid, src, todir = os.environ["FD_ID"], os.environ["FD_FROM"], os.environ["FD_TODIR"]
dry, want_bz = os.environ["FD_DRY"] == "1", os.environ["FD_BZ"] == "1"
prefix = f"fastdl/{sid}/"
files_dir = os.path.join(src, "files")

# what may go to the public mirror: the same rule as the app (fastdl.rs safe_path + downloadable)
SAFE = re.compile(r"^[A-Za-z0-9_\-.+!()/]{1,200}$")
EXT = {"bsp", "res", "wad", "wav", "mp3", "mdl", "spr", "tga", "bmp", "txt"}
DIRS = ("maps/", "sound/", "models/", "sprites/", "gfx/", "overviews/")

def allowed(rel):
    if not SAFE.match(rel) or rel.startswith("/") or any(p in ("", ".", "..") for p in rel.split("/")):
        return False
    ext = rel.rsplit(".", 1)[-1].lower()
    return ext in EXT and (rel.lower().startswith(DIRS) or (ext == "wad" and "/" not in rel))

def fail(msg):
    print(msg, file=sys.stderr)
    sys.exit(1)

def open_plain(rel):
    """The prepared file, opened folder by folder without following a link."""
    d = os.open(files_dir, os.O_RDONLY | os.O_DIRECTORY)
    parts = rel.split("/")
    try:
        for p in parts[:-1]:
            n = os.open(p, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=d)
            os.close(d)
            d = n
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=d)
    finally:
        os.close(d)
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        os.close(fd)
        raise OSError("not a plain file")
    return os.fdopen(fd, "rb")

man = json.load(open(os.path.join(src, "manifest.json")))
if man.get("server") != sid:
    fail(f"{src} was prepared for {man.get('server')!r}, not {sid}.")
local = {}
for f in man.get("files", []):
    rel = f.get("path", "")
    if not allowed(rel):
        fail(f"{rel!r} isn't a file players download, so nothing was uploaded.")
    local[rel] = f

def s3(*args, capture=True):
    r = subprocess.run(["aws", "s3api", "--endpoint-url", os.environ["R2_ENDPOINT"], *args], capture_output=capture, text=True)
    if r.returncode != 0:
        fail(f"R2 said no ({' '.join(args[:1])}): {r.stderr.strip()[:300]}")
    return r.stdout

# what's on the mirror now
up = {}
if todir:
    p = os.path.join(todir, prefix, "manifest.json")
    if os.path.isfile(p):
        up = {f["path"]: f for f in json.load(open(p)).get("files", [])}
    have = set()
    base = os.path.join(todir, prefix)
    for root, dirs, names in os.walk(base):
        for n in names:
            have.add(os.path.relpath(os.path.join(root, n), base).replace(os.sep, "/"))
else:
    tmp = tempfile.NamedTemporaryFile(delete=False)
    tmp.close()
    r = subprocess.run(["aws", "s3api", "--endpoint-url", os.environ["R2_ENDPOINT"], "get-object", "--bucket", os.environ["R2_BUCKET"], "--key", prefix + "manifest.json", tmp.name], capture_output=True, text=True)
    if r.returncode == 0:
        try:
            up = {f["path"]: f for f in json.load(open(tmp.name)).get("files", [])}
        except ValueError:
            up = {}
    os.unlink(tmp.name)
    have = set()
    token = None
    while True:
        args = ["list-objects-v2", "--bucket", os.environ["R2_BUCKET"], "--prefix", prefix, "--output", "json"]
        if token:
            args += ["--continuation-token", token]
        out = json.loads(s3(*args) or "{}")
        have |= {o["Key"][len(prefix):] for o in out.get("Contents", [])}
        token = out.get("NextContinuationToken")
        if not token:
            break

def put(key_rel, path, ctype, cache):
    """Uploads one file; its ETag."""
    if todir:
        dst = os.path.join(todir, prefix, key_rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(path, dst)
        return hashlib.md5(open(dst, "rb").read()).hexdigest()
    out = s3("put-object", "--bucket", os.environ["R2_BUCKET"], "--key", prefix + key_rel, "--body", path,
             "--content-type", ctype, "--cache-control", cache, "--output", "json")
    return json.loads(out).get("ETag", "").strip('"')

CACHE = "public, max-age=300"
new, sent, sent_bytes = [], 0, 0
with tempfile.TemporaryDirectory() as work:
    for rel in sorted(local):
        f = local[rel]
        prev = up.get(rel)
        changed = not prev or prev.get("sha256") != f["sha256"] or not prev.get("etag") or rel not in have
        needs_bz = want_bz and (changed or not prev.get("bz2") or rel + ".bz2" not in have)
        entry = {"path": rel, "size": f["size"], "sha256": f["sha256"], "etag": (prev or {}).get("etag", "")}
        if want_bz or (prev or {}).get("bz2"):
            entry["bz2"] = True
        if not changed and not needs_bz:
            print(f"same     {rel}")
            new.append(entry)
            continue
        # the very bytes the app prepared: re-hashed here, read without following a link
        tmp = os.path.join(work, "f")
        try:
            with open_plain(rel) as fh, open(tmp, "wb") as out:
                h = hashlib.sha256()
                for b in iter(lambda: fh.read(1 << 16), b""):
                    h.update(b)
                    out.write(b)
        except OSError:
            fail(f"{rel} isn't a plain prepared file (a link?), so nothing more was uploaded. Prepare the files again.")
        if h.hexdigest() != f["sha256"] or os.path.getsize(tmp) != f["size"]:
            fail(f"{rel} changed since it was prepared. Prepare the files again.")
        if changed:
            print(f"upload   {rel} ({f['size'] // 1024} KB)")
            if not dry:
                entry["etag"] = put(rel, tmp, "application/octet-stream", CACHE)
            sent += 1
            sent_bytes += f["size"]
        if needs_bz:
            print(f"upload   {rel}.bz2")
            if not dry:
                with open(tmp, "rb") as i, bz2.open(tmp + ".bz2", "wb", compresslevel=9) as o:
                    shutil.copyfileobj(i, o)
                put(rel + ".bz2", tmp + ".bz2", "application/octet-stream", CACHE)
        new.append(entry)

# what the server doesn't have any more (only inside this server's own folder)
keep = {e["path"] for e in new} | {e["path"] + ".bz2" for e in new if e.get("bz2")} | {"manifest.json"}
for rel in sorted(have - keep):
    print(f"delete   {rel}")
    if dry:
        continue
    if todir:
        os.unlink(os.path.join(todir, prefix, rel))
    else:
        s3("delete-object", "--bucket", os.environ["R2_BUCKET"], "--key", prefix + rel)

# the manifest last: the app's check reads it (never cached for long)
if not dry:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as t:
        json.dump({"server": sid, "game": man.get("game", ""), "made": man.get("made", 0), "files": new}, t, indent=1)
    if todir:
        shutil.copyfile(t.name, os.path.join(todir, prefix, "manifest.json"))
    else:
        s3("put-object", "--bucket", os.environ["R2_BUCKET"], "--key", prefix + "manifest.json", "--body", t.name,
           "--content-type", "application/json", "--cache-control", "no-cache", "--output", "json")
    os.unlink(t.name)
print(f"{'Would upload' if dry else 'Uploaded'} {sent} of {len(new)} files ({sent_bytes / 1048576:.1f} MB). "
      f"Players download from https://dl.lonerstrike.com/{prefix.rstrip('/')}" + (" (a dry run: nothing changed)" if dry else ""))
PY
