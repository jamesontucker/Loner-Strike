# 7-Zip for Linux

`7zzs` is the official static build of 7-Zip 26.05 for Linux x64, from
[ip7z/7zip](https://github.com/ip7z/7zip/releases/tag/26.05) (`7z2605-linux-x64.tar.xz`,
sha256 `22cf82fcc6eb7474f1d5bb4f02c0f8ebd573a718089d6833dc30f8350a3b232e`), unmodified.
Linux's GNU tar can't open RAR or 7z files (the Mac's and Windows' bsdtar can), so archive.rs
unpacks those map downloads with it, as the Windows build does with its bundled 7z.exe.

sha256 of `7zzs`: `8952c73646dc8d4abfb31e48a949ba0ac15cb2437da9e4430886ddb6e12b383b`

7-Zip is under the GNU LGPL, with the unRAR restriction for its RAR code (License.txt).
