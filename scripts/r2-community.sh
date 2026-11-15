#!/usr/bin/env bash
# Publishes the community index to Loner-Strike's download mirror (Cloudflare R2,
# dl.lonerstrike.com/community/): index.json and index.json.sha256, as written by
# `services/serverlist/lsmod.py publish`. The app reads index.json (cached 15 minutes).
#
#   scripts/r2-community.sh <folder>            only say what it would do (nothing is uploaded)
#   scripts/r2-community.sh <folder> --upload   check the files, then upload them
#
# Uploading has to be asked for: without --upload this is a dry run. Before anything goes up the
# index is checked: it parses, it's schema 1, it's under the app's 8 MB limit, and its sha256
# matches index.json.sha256. Unchanged files are left alone (R2's ETag is the MD5 of a file put
# in one piece). Short cache time, so a pulled page disappears from apps within minutes.
#
# Keys: ~/.config/lonerstrike/r2.env (as scripts/r2-sync.sh), or R2_ENV=<file>. Never printed.
set -euo pipefail

dir=${1:?Usage: scripts/r2-community.sh <folder> [--upload]}
upload=0
[ "${2:-}" = "--upload" ] && upload=1
[ -f "$dir/index.json" ] && [ -f "$dir/index.json.sha256" ] || { echo "No index.json / index.json.sha256 in $dir." >&2; exit 1; }

python3 -I - "$dir" <<'PY'
import hashlib, json, os, sys
d = sys.argv[1]
data = open(os.path.join(d, "index.json"), "rb").read()
want = open(os.path.join(d, "index.json.sha256")).read().split()[0]
if hashlib.sha256(data).hexdigest() != want:
    sys.exit("index.json doesn't match index.json.sha256")
if len(data) > 8 << 20:
    sys.exit("index.json is over the app's 8 MB limit")
doc = json.loads(data)
if doc.get("schema") != 1 or not isinstance(doc.get("items"), list) or not isinstance(doc.get("revoked"), list):
    sys.exit("index.json isn't a schema 1 index")
print(f"index.json: {len(data)} bytes, {len(doc['items'])} entries, {len(doc['revoked'])} pulled")
PY

if [ "$upload" = 0 ]; then
  echo "would upload community/index.json.sha256 and community/index.json (dry run: add --upload)"
  exit 0
fi

# keys already in the environment (the box's publish timer) or from the file
if [ -z "${R2_ACCESS_KEY_ID:-}" ]; then
  env_file=${R2_ENV:-$HOME/.config/lonerstrike/r2.env}
  [ -f "$env_file" ] || { echo "No keys: make $env_file first (see scripts/r2-sync.sh)." >&2; exit 1; }
  # shellcheck disable=SC1090
  . "$env_file"
fi
: "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}" "${R2_ENDPOINT:?}" "${R2_BUCKET:?}"
export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY AWS_DEFAULT_REGION=auto
s3() { aws s3api --endpoint-url "$R2_ENDPOINT" "$@"; }

md5_of() { md5 -q "$1" 2>/dev/null || md5sum "$1" | cut -d' ' -f1; }
put() {  # put <file> <key> <content type>
  local etag
  etag=$(s3 head-object --bucket "$R2_BUCKET" --key "$2" --query ETag --output text 2>/dev/null | tr -d '"' || true)
  if [ -n "$etag" ] && [ "$etag" = "$(md5_of "$1")" ]; then
    echo "unchanged $2"; return
  fi
  s3 put-object --bucket "$R2_BUCKET" --key "$2" --body "$1" --content-type "$3" --cache-control "public, max-age=300" >/dev/null
  echo "uploaded  $2"
}
# the checksum first: an app that one day checks it never sees a new index with the old sum for long
put "$dir/index.json.sha256" community/index.json.sha256 "text/plain; charset=utf-8"
put "$dir/index.json" community/index.json "application/json; charset=utf-8"
echo "The community index is up: https://dl.lonerstrike.com/community/index.json"
