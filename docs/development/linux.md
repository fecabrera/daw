# Linux

[Development guides](README.md)

Run commands from the repository root.

Install Rust through Rustup, a C++ compiler, CMake, Clang/libclang, ALSA development libraries, pkg-config, and OpenSSL development headers (for build-time runtime downloads). Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

On Wayland, the app uses an Adwaita-style titlebar when the desktop does not provide window decorations. X11 uses the window manager's decorations.

Create a portable ZIP with `sh tools/package_linux.sh release`. Use `debug` for a debug build. The archive is `target/<profile>/DAW-linux-x64.zip` on x64 or `DAW-linux-arm64.zip` on ARM64. Extract it and run `DAW/daw-desktop`. Keep `DAW/LICENSE.txt` and `DAW/licenses` with the executable. The archive preserves executable permissions and includes the same dependency notices as the other packages. It does not bundle system libraries: the destination needs compatible C/C++ runtime, ALSA, graphics libraries, and a desktop portal for dialogs.

Desktop downloads also contain the exact [corresponding source and relinking
materials](README.md#corresponding-source-in-desktop-packages). Packaging requires
a clean committed tracked tree. Keep `SOURCE-ACCESS.txt`, `source-manifest.json`
and `DAW-source.zip` with the binary and licenses; on macOS these are inside
`DAW.app/Contents/Resources`. The source archive includes tracked repository demo
fixtures, but excludes untracked user data and build caches.
