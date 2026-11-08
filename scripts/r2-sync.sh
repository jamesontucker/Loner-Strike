#!/usr/bin/env bash
# Keeps Loner-Strike's download mirror (Cloudflare R2, dl.lonerstrike.com) in step with the
# downloads the app pins: Metamod-P, AMX Mod X, our ReGameDLL and ReHLDS, ReAPI,
# relocalizebugfix (Windows and Linux). Each missing one is fetched from its original,
# checked against its pinned SHA-256 and uploaded; once all of them are there, files no
# longer pinned are deleted. The app tries the mirror first and the original after it.
#
#   scripts/r2-sync.sh            upload what's missing, then delete what's no longer pinned
#   scripts/r2-sync.sh --dry-run  only say what it would do
#
# Keys: ~/.config/lonerstrike/r2.env (R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_ENDPOINT,
# R2_BUCKET), readable only by you. Never in a repo.
set -euo pipefail

dry=0
[ "${1:-}" = "--dry-run" ] && dry=1
here=$(cd "$(dirname "$0")/.." && pwd)
env_file="$HOME/.config/lonerstrike/r2.env"
[ -f "$env_file" ] || { echo "No keys: make $env_file first (see the comment at the top)." >&2; exit 1; }
# shellcheck disable=SC1090
. "$env_file"
: "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}" "${R2_ENDPOINT:?}" "${R2_BUCKET:?}"
export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY AWS_DEFAULT_REGION=auto
s3() { aws s3api --endpoint-url "$R2_ENDPOINT" "$@"; }

# what the app pins: "url sha256" per line, from foundation.rs and the Linux setup's list
pins=$(python3 - "$here/app/src-tauri/src" <<'PY'
import re, sys
src = sys.argv[1]
found = open(f"{src}/setup/foundation.rs").read()
consts = {}
for name, body in re.findall(r'pub const (\w+): Pinned = Pinned \{(.*?)\};', found, re.S):
    url = re.search(r'url: "([^"]+)"', body).group(1)
    sha = re.search(r'sha256: "([0-9a-f]{64})"', body).group(1)
    consts[name] = (url, sha)
    print(url, sha)
remote = open(f"{src}/remote.rs").read()
env = re.search(r'FOUNDATION_ENV: &\[\(&str, &str\)\] = &\[(.*?)\];', remote, re.S).group(1)
pairs = dict(re.findall(r'\("(\w+)", "([^"]+)"\)', env))
for k, url in pairs.items():
    if k.endswith("_URL"):
        print(url, pairs[k[:-4] + "_SHA"])
PY
)
[ -n "$pins" ] || { echo "Couldn't read the pinned downloads." >&2; exit 1; }

have=$(s3 list-objects-v2 --bucket "$R2_BUCKET" --query 'Contents[].Key' --output text 2>/dev/null | tr '\t' '\n' | grep -v '^None$' || true)
work=$(mktemp -d)
trap 'rm -f "$work"/*; rmdir "$work"' EXIT
wanted=()
while read -r url sha; do
  name=${url##*/}
  wanted+=("$name")
  if printf '%s\n' "$have" | grep -qxF "$name"; then
    echo "have     $name"
    continue
  fi
  echo "upload   $name"
  [ "$dry" = 1 ] && continue
  curl -fsSL --retry 8 --retry-delay 8 --retry-all-errors "$url" -o "$work/$name"
  got=$(shasum -a 256 "$work/$name" | cut -d' ' -f1)
  [ "$got" = "$sha" ] || { echo "$name isn't the pinned file ($got), so it wasn't uploaded." >&2; exit 1; }
  s3 put-object --bucket "$R2_BUCKET" --key "$name" --body "$work/$name" \
    --content-type application/octet-stream --cache-control "public, max-age=31536000, immutable" >/dev/null
  rm -f "$work/$name"
done <<< "$pins"

# everything pinned is there now: what isn't pinned any more goes (only the downloads at the
# top: folders like app/, the app's own releases, are kept by their own workflow)
for key in $have; do
  case "$key" in */*) continue ;; esac
  printf '%s\n' "${wanted[@]}" | grep -qxF "$key" && continue
  echo "delete   $key"
  [ "$dry" = 1 ] || s3 delete-object --bucket "$R2_BUCKET" --key "$key" >/dev/null
done
echo "The mirror matches the pinned downloads."
