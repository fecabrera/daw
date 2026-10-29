# macOS

[Development guides](README.md)

Run commands from the repository root.

Install Rust through Rustup and the native C++ development tools with Clang/libclang. Build and run with:

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

Create an app bundle with `sh tools/package_macos.sh debug`. Open `target/debug/DAW.app`, then select a project folder with File > Open. Use `release` instead of `debug` for an optimized build. The local bundle is unsigned.

Create a disk image with `sh tools/package_macos_dmg.sh release`. Use `debug` for a debug build. The image is `target/<profile>/DAW-macos-arm64.dmg` on Apple Silicon or `DAW-macos-x64.dmg` on Intel. Open the image and drag DAW to Applications. The app includes the project license and dependency notices in `Contents/Resources`. The image is unsigned and the app is not notarized.

Stem separation requires Apple Silicon on macOS. Intel builds omit that backend and disable its controls; the other app features remain available.
