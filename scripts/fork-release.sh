#!/usr/bin/env bash
# Publishes a release of one of Loner-Strike's forks (ReGameDLL_CS, ReHLDS) from a green
# build of its loner-strike branch, laid out like upstream's releases (bin/win32,
# bin/linux32), and prints the URL and SHA-256 for the app to pin.
#
#   scripts/fork-release.sh regamedll 5.30.0.814-ls1
#   scripts/fork-release.sh rehlds 3.15.0.896-ls1
#
# The build is the newest successful run on the branch (its commit is the release's target).
set -euo pipefail

kind=${1:?regamedll or rehlds}
version=${2:?version, like 5.30.0.814-ls1}
case "$kind" in
  regamedll) repo=jamesontucker/ReGameDLL_CS; name=regamedll ;;
  rehlds)    repo=jamesontucker/ReHLDS; name=rehlds ;;
  *) echo "regamedll or rehlds" >&2; exit 2 ;;
esac

run=$(gh run list -R "$repo" -b loner-strike --status success -L 1 --json databaseId,headSha -q '.[0].databaseId')
sha=$(gh run list -R "$repo" -b loner-strike --status success -L 1 --json headSha -q '.[0].headSha')
[ -n "$run" ] || { echo "no successful build on loner-strike" >&2; exit 1; }

work=$(mktemp -d)
gh run download "$run" -R "$repo" -n win32 -D "$work/win32"
gh run download "$run" -R "$repo" -n linux32 -D "$work/linux32"
mkdir -p "$work/pkg"
cp -R "$work/win32/bin" "$work/pkg/"
cp -R "$work/linux32/bin/." "$work/pkg/bin/"
# ReGameDLL's releases also carry its dist files (game.cfg, game_init.cfg, delta.lst) beside
# the game DLL, which the build itself doesn't output
if [ "$kind" = regamedll ]; then
  for f in delta.lst game.cfg game_init.cfg; do
    for plat in win32 linux32; do
      gh api "repos/$repo/contents/dist/$f?ref=$sha" -H "Accept: application/vnd.github.raw" > "$work/pkg/bin/$plat/cstrike/$f"
    done
  done
fi
zip="$work/$name-bin-$version.zip"
(cd "$work/pkg" && zip -qr -X "$zip" bin)

if gh release view "$version" -R "$repo" >/dev/null 2>&1; then
  gh release upload "$version" "$zip" -R "$repo" --clobber
else
gh release create "$version" "$zip" -R "$repo" --target "$sha" \
  --title "Loner-Strike $name $version" \
  --notes "Built from the loner-strike branch at $sha (CI run $run), for Loner-Strike. Unsigned. Upstream: https://github.com/rehlds"
fi

echo "url: https://github.com/$repo/releases/download/$version/$(basename "$zip")"
echo "sha256: $(shasum -a 256 "$zip" | cut -d' ' -f1)"
echo "next: pin it in app/src-tauri/src/setup/foundation.rs, then scripts/r2-sync.sh (puts it on"
echo "the download mirror and removes the release it replaces)"
