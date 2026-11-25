# AUR package (loner-strike-bin)

`PKGBUILD` for the [AUR](https://aur.archlinux.org): Arch, CachyOS, Manjaro and other Arch-based
systems install Loner-Strike with an AUR helper (`yay -S loner-strike-bin`). It unpacks the
release's `.deb` (byte-exact builds; the AppImage's packager rewrites bundled programs).

Not published yet: it needs a release with the Linux build, and an AUR maintainer account
(aur.archlinux.org, with an SSH key). For each release:

1. Set `pkgver` to the release (without the `v`) and `pkgrel=1`.
2. `sha256sums`: the release `.deb`'s SHA-256 (`sha256sum Loner-Strike_<ver>_amd64.deb`). The
   placeholder in the file isn't a valid sum on purpose, so `makepkg` fails until it's set.
3. `makepkg --printsrcinfo > .SRCINFO`, then test with `makepkg -si` on an Arch system.
4. Commit `PKGBUILD` and `.SRCINFO` to the AUR repo (`ssh://aur@aur.archlinux.org/loner-strike-bin.git`).

In-app updates: the app's updater handles the AppImage, .deb and .rpm; an AUR install (no
dpkg on Arch) is told to update with its AUR helper (app_api.rs, linux_package_update).
