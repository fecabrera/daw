# Linux

[Development guides](README.md)

Run commands from the repository root.

Install Rust through Rustup, a C++ compiler, CMake, Clang/libclang, GTK 4.8 or newer development libraries, ALSA development libraries, pkg-config, and OpenSSL development headers (for build-time runtime downloads). Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

The Linux desktop uses GTK with OpenGL 3.3. Under **Settings > Appearance > Titlebar**, choose **Custom** for actual GTK window buttons alongside the application menu, project title and view controls. Choose **Native** for the regular titlebar above the app toolbar. Fresh preferences select Custom; saved choices are retained. Both modes are available on Wayland and X11.

Create a portable ZIP with `sh tools/package_linux.sh release`. Use `debug` for a debug build. The archive is `target/<profile>/DAW-linux-x64.zip` on x64 or `DAW-linux-arm64.zip` on ARM64. Extract it and run `DAW/daw-desktop`. Keep `DAW/LICENSE.txt` and `DAW/licenses` with the executable. The archive preserves executable permissions and includes the same dependency notices as the other packages. It does not bundle system libraries: the destination needs compatible C/C++ runtime, GTK 4.8 or newer, OpenGL 3.3, ALSA, and a desktop portal for dialogs.

Desktop downloads also contain the exact [corresponding source and relinking
materials](README.md#corresponding-source-in-desktop-packages). Packaging requires
a clean committed tracked tree. Keep `SOURCE-ACCESS.txt`, `source-manifest.json`
and `DAW-source.zip` with the binary and licenses; on macOS these are inside
`DAW.app/Contents/Resources`. The source archive includes tracked repository demo
fixtures, but excludes untracked user data and build caches.

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
