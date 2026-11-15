# Linux

[Development guides](README.md)

Run commands from the repository root.

Install Rust through Rustup, a C++ compiler, CMake, Clang/libclang, GTK 4.8 or newer development libraries, ALSA development libraries, pkg-config, and OpenSSL development headers (for build-time runtime downloads). Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

The Linux desktop uses GTK with OpenGL 3.3. Under **Settings > Appearance > Titlebar**, choose **Custom** for actual GTK window buttons alongside the application menu, project title and view controls. Choose **Native** for the regular titlebar above the app toolbar. Fresh preferences select Custom; saved choices are retained. Both modes are available on Wayland and X11.

Create a local portable ZIP from the current files without committing:

```sh
sh tools/package_linux.sh debug --local
sh tools/package_linux.sh release --local
```

Omitting the profile selects `debug`; `--local` may appear before or after the
profile. Local archives include icons, runtime requirements and license notices,
plus `DAW/LOCAL-BUILD.txt`. They omit the corresponding-source archive, access
notice and manifest and are for local development. For distribution, omit
`--local`: `sh tools/package_linux.sh release`.

The archive is `target/<profile>/DAW-linux-x64.zip` on x64 or
`DAW-linux-arm64.zip` on ARM64. Both modes use the same output path and replace
it only after packaging succeeds. Extract it and run `DAW/daw-desktop`. Keep
`DAW/LICENSE.txt` and `DAW/licenses` with the executable. The archive preserves
executable permissions and includes the same dependency notices as the other
packages. It does not bundle system libraries: the destination needs compatible
C/C++ runtime, GTK 4.8 or newer, OpenGL 3.3, ALSA, and a desktop portal for dialogs.

Desktop downloads also contain the exact [corresponding source and relinking
materials](README.md#corresponding-source-in-desktop-packages). Distribution packaging requires
a clean committed tracked tree. Keep `SOURCE-ACCESS.txt`, `source-manifest.json`
and `DAW-source.zip` with the binary and licenses; on macOS these are inside
`DAW.app/Contents/Resources`. The source archive includes tracked repository demo
fixtures, but excludes untracked user data and build caches.

## Application launcher

The app uses the shared orange waveform on a rounded graphite background. The ZIP also
includes a desktop entry and icons for a desktop launcher. After extracting it,
run these commands from the extracted `DAW` directory to install for your user:

```sh
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons"
cp daw-desktop "$HOME/.local/bin/"
cp share/applications/org.daw.DAW.desktop "$HOME/.local/share/applications/"
cp -R share/icons/hicolor "$HOME/.local/share/icons/"
```

Ensure `$HOME/.local/bin` is on your command search path. Keep the extracted
package and its source and license materials. The running app embeds its window
icon; desktop-shell launcher integration requires the installed desktop entry
and icons. Some desktops refresh their launcher cache only after signing in again.

## Regenerate the app icon

Edit and save `crates/ui/assets/app-icon/DAW.icon/Assets/Waveform.svg`, which is
also used by macOS and Windows. Install `rsvg-convert` from librsvg and
`glib-compile-resources` from GLib, then run from the repository root:

```sh
python3 tools/export_app_icons.py --platform linux
```

This regenerates the scalable SVG, PNG sizes and embedded icon resource. Rebuild
the app and package to use them. Repeat the launcher installation steps above
to update installed icons. Use `--platform desktop` to regenerate Windows too.

## Saved preferences and window checks

The first GTK startup carries over existing application preferences, including
selected audio devices and titlebar mode. If no previous application preferences
exist, it uses choices from the former GTK trial when available. Older preference
files remain unchanged. Window size, maximized state and fullscreen state are
retained across restarts.

Run the native window check without opening a project or changing preferences:

```sh
cargo run -p daw-desktop -- --smoke-test
```

The Linux host currently has limited screen-reader support for the egui content,
full input-method composition, image clipboard support and additional windows.
GTK window buttons retain their toolkit accessibility. Device playback and
fractional scaling need native verification on the target desktop.

See the [GTK host specification](../../specs/architecture/linux-gtk-titlebar.md)
for implementation details and validation commands.
