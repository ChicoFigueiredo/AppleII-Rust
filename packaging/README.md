# Packaging

- `make_icons.py <AppleWin/resource>`: regenerates `icons/` (PNG 16..512, `.ico`, `.icns`) from AppleWin's `APPLEWIN.ICO`/`DISK.ICO`. The originals only have 16/32 px frames, so larger sizes are nearest-neighbour upscales.
- `package.sh <linux|windows|macos> <version> <bindir>`: builds `dist/applewin-<ver>-<platform>.{tar.gz,zip}`; used by `.github/workflows/release.yml` and GitLab CI.
- `windows/applewin.manifest`: PerMonitorV2 DPI awareness, long paths; embedded by `crates/applewin/build.rs` together with the icon and version info.
- `linux/`: `.desktop`, shared-mime-info XML, `install.sh` (user-level install). No AppImage: the binary dlopens Vulkan/Wayland/X11/ALSA, which AppImage bundling handles badly.
- `macos/Info.plist.in`: bundle `io.github.chicofigueiredo.appleii-rust` with disk-image document types. The app is ad-hoc signed (`codesign -s -`), not notarized: first launch needs right-click > Open.

Windows release builds use the GUI subsystem (no console window); `wincon.rs` attaches to the parent console so `--help`/`--dump-text` still print from cmd/PowerShell, but the shell prompt does not wait for the program.

Cutting a release: `git tag v0.1.0 && git push origin v0.1.0`. Dry run: Actions > Release > Run workflow (artifacts only, no GitHub release).
