#!/usr/bin/env bash
# Publishes the ready-made bot navigation pack to Loner-Strike's download mirror (Cloudflare
# R2, dl.lonerstrike.com/navs/): list.json plus <map>.nav and <map>.lsv for each map. The app
# (navpack.rs) checks list.json a few minutes after it starts and every six hours.
# The pack is built by the nav batch tooling (navpack/package.py) into one folder.
#
#   scripts/r2-navs.sh <pack folder>            upload the files, then list.json last
#   scripts/r2-navs.sh <pack folder> --dry-run  only say what it would do
#
# Only files that changed go up (R2's ETag is the MD5 of a file put in one piece). The map files
# go up first and list.json last, so a player never gets a list naming files that
# aren't there yet. Files no longer in the list are deleted afterwards. Short cache times: a
# changed nav keeps its name, so it mustn't be cached for long.
#
# Keys: ~/.config/lonerstrike/r2.env (as scripts/r2-sync.sh).
set -euo pipefail

pack=${1:?Usage: scripts/r2-navs.sh <pack folder> [--dry-run]}
dry=0
[ "${2:-}" = "--dry-run" ] && dry=1
[ -f "$pack/list.json" ] || { echo "No list.json in $pack." >&2; exit 1; }
env_file="$HOME/.config/lonerstrike/r2.env"
[ -f "$env_file" ] || { echo "No keys: make $env_file first (see scripts/r2-sync.sh)." >&2; exit 1; }
# shellcheck disable=SC1090
. "$env_file"
: "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}" "${R2_ENDPOINT:?}" "${R2_BUCKET:?}"
export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY AWS_DEFAULT_REGION=auto
s3() { aws s3api --endpoint-url "$R2_ENDPOINT" "$@"; }

# every file the list names, checked against its sha256 before anything goes up
files=$(python3 - "$pack" <<'PY'
import hashlib, json, os, sys
pack = sys.argv[1]
for m in json.load(open(os.path.join(pack, "list.json")))["maps"]:
    for ext, key in (("nav", "nav_sha256"), ("lsv", "lsv_sha256")):
        name = f"{m['map']}.{ext}"
        data = open(os.path.join(pack, name), "rb").read()
        if hashlib.sha256(data).hexdigest() != m[key]:
            sys.exit(f"{name} doesn't match list.json")
        print(name)
PY
)
count=$(printf '%s\n' "$files" | grep -c .)
echo "$count files for $(python3 -c "import json,sys;print(len(json.load(open(sys.argv[1]))['maps']))" "$pack/list.json") maps"

# what's there now, with each file's MD5 (its ETag)
remote=$(s3 list-objects-v2 --bucket "$R2_BUCKET" --prefix navs/ --query 'Contents[].[Key,ETag]' --output text 2>/dev/null | grep -v '^None$' | tr -d '"' || true)
md5_of() { md5 -q "$1" 2>/dev/null || md5sum "$1" | cut -d' ' -f1; }
put() {  # put <file> <key> <content type> <cache>
  local etag
  etag=$(printf '%s\n' "$remote" | awk -v k="$2" '$1 == k { print $2 }')
  if [ -n "$etag" ] && [ "$etag" = "$(md5_of "$1")" ]; then
    unchanged=$((unchanged + 1)); return
  fi
  if [ "$dry" = 1 ]; then echo "upload   $2"; return; fi
  s3 put-object --bucket "$R2_BUCKET" --key "$2" --body "$1" --content-type "$3" --cache-control "$4" >/dev/null
  echo "uploaded $2"
}
unchanged=0
while read -r name; do
  [ -n "$name" ] && put "$pack/$name" "navs/$name" application/octet-stream "public, max-age=3600"
done <<< "$files"
put "$pack/list.json" "navs/list.json" application/json "public, max-age=300"
echo "$unchanged unchanged, left as they are"

# what's under navs/ but no longer in the list goes
have=$(printf '%s\n' "$remote" | awk '{ print $1 }')
for key in $have; do
  name=${key#navs/}
  [ "$name" = list.json ] && continue
  printf '%s\n' "$files" | grep -qxF "$name" && continue
  echo "delete   $key"
  [ "$dry" = 1 ] || s3 delete-object --bucket "$R2_BUCKET" --key "$key" >/dev/null
done
echo "The nav pack is up: https://dl.lonerstrike.com/navs/list.json"
