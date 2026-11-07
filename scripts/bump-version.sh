#!/usr/bin/env bash
# bump-version.sh <new-version>
#
# Sets the app's version everywhere it's pinned, commits and tags it (v<version>), so the
# tag and the app always agree. The release workflow refuses a tag that doesn't match
# app/src-tauri/tauri.conf.json, and the in-app updater compares against that version.
#
# Files: app/src-tauri/tauri.conf.json, app/src-tauri/Cargo.toml, app/src-tauri/Cargo.lock
# (Loner-Strike's own entry only), app/ui/package.json and package-lock.json.
#
# Usage: ./scripts/bump-version.sh 0.1.1     (then: git push && git push origin v0.1.1)
set -euo pipefail

if [[ $# -ne 1 || ! "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Usage: $0 <version>, like $0 0.1.1" >&2
  exit 1
fi
NEW="$1"
cd "$(dirname "$0")/.."
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Commit or stash your changes first." >&2
  git status --short >&2
  exit 1
fi
if git rev-parse "v$NEW" >/dev/null 2>&1; then
  echo "The tag v$NEW already exists." >&2
  exit 1
fi
for tool in jq git; do command -v "$tool" >/dev/null || { echo "$tool is needed." >&2; exit 1; }; done

CUR=$(jq -r '.version' app/src-tauri/tauri.conf.json)
echo "Loner-Strike $CUR -> $NEW"

set_json() { local tmp; tmp=$(mktemp); jq --arg v "$NEW" "$2" "$1" > "$tmp" && mv "$tmp" "$1"; }
set_json app/src-tauri/tauri.conf.json '.version = $v'
set_json app/ui/package.json '.version = $v'
[[ -f app/ui/package-lock.json ]] && set_json app/ui/package-lock.json '.version = $v | .packages[""].version = $v'

# Cargo.toml: the [package] version line only
awk -v v="$NEW" 'BEGIN{p=0} /^\[package\]/{p=1} /^\[/{if($0!="[package]")p=0} p && /^version *=/ && !d {print "version = \"" v "\""; d=1; next} {print}' \
  app/src-tauri/Cargo.toml > app/src-tauri/Cargo.toml.tmp && mv app/src-tauri/Cargo.toml.tmp app/src-tauri/Cargo.toml
# Cargo.lock: Loner-Strike's own entry
awk -v v="$NEW" '/^name = "loner-strike"$/{print; getline; print "version = \"" v "\""; next} {print}' \
  app/src-tauri/Cargo.lock > app/src-tauri/Cargo.lock.tmp && mv app/src-tauri/Cargo.lock.tmp app/src-tauri/Cargo.lock

git add app/src-tauri/tauri.conf.json app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock app/ui/package.json app/ui/package-lock.json 2>/dev/null || true
git commit -qm "Version $NEW"
git tag "v$NEW"
echo "Committed and tagged v$NEW. To release: git push && git push origin v$NEW"
