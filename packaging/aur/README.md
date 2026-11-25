# AUR package (loner-strike-bin)

`PKGBUILD` for the [AUR](https://aur.archlinux.org): Arch, CachyOS, Manjaro and other Arch-based
systems install Loner-Strike with an AUR helper (`yay -S loner-strike-bin`). It unpacks the
release's `.deb` (byte-exact builds; the AppImage's packager rewrites bundled programs).

Not published yet: it needs a release with the Linux build, and an AUR maintainer account
(aur.archlinux.org, with an SSH key). For each release:

1. Set `pkgver` to the release (without the `v`) and `pkgrel=1`.
2. `sha256sums`: the release `.deb`'s SHA-256 (never `SKIP` when published).
3. `makepkg --printsrcinfo > .SRCINFO`, then test with `makepkg -si` on an Arch system.
4. Commit `PKGBUILD` and `.SRCINFO` to the AUR repo (`ssh://aur@aur.archlinux.org/loner-strike-bin.git`).

In-app updates: the app's updater replaces an AppImage; installed from the AUR, updates come
from the AUR helper instead.
