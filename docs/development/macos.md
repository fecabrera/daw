# macOS

[Development guides](README.md)

Run commands from the repository root.

## Prerequisites

Install Rust through Rustup and the native C++ development tools with Clang/libclang. Install CMake for the bundled Opus decoder (for example,
`brew install cmake`).

## Build and run from source

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

## Build and run the app bundle

Create a local app bundle from the current files without committing:

```sh
sh tools/package_macos.sh debug --local
sh tools/package_macos.sh release --local
```

Open `target/debug/DAW.app` or `target/release/DAW.app`, then select a project
folder with File > Open. Omitting the profile selects `debug`; `--local` may
appear before or after the profile. Both bundles are ad-hoc signed after assembly.
A failed build, signing step or bundle assembly leaves the previous app and
source receipts in place.

Local bundles include icons and license notices, plus `LOCAL-BUILD.txt` in
`Contents/Resources`. They omit the corresponding-source archive and manifest
and are for local development. To create a distribution bundle, omit `--local`:
`sh tools/package_macos.sh release`. Distribution packaging requires a clean
committed tracked tree and includes the matching source materials.

The bundle uses a layered Icon Composer icon with an orange split waveform
and native graphite background. macOS controls its shape, size and appearance.
Direct Cargo launches use an Apple-rendered static version with native padding.

## Package a disk image

Create a disk image with `sh tools/package_macos_dmg.sh release`. Use `debug` for
a debug build. The image is `target/<profile>/DAW-macos-arm64.dmg` on Apple Silicon
or `DAW-macos-x64.dmg` on Intel. Open the image and drag DAW to Applications. The
app includes the project license and dependency notices in `Contents/Resources`.

The image is unsigned. The app has an ad-hoc signature, but no Developer ID
signature or Apple notarization. This checks bundle integrity; it does not make
a browser download trusted by Gatekeeper. For a CI build you trust, copy the app
to Applications, try to open it, then use System Settings > Privacy & Security >
Open Anyway if macOS offers it. See [Apple’s guidance](https://support.apple.com/en-lamr/102445).
Do not use this override for a package whose integrity cannot be established.

Stem separation requires Apple Silicon on macOS. Intel builds omit that backend and disable its controls; the other app features remain available.

## Distribution verification

Desktop downloads also contain the exact [corresponding source and relinking
materials](README.md#corresponding-source-in-desktop-packages). Distribution packaging requires
a clean committed tracked tree. Keep `SOURCE-ACCESS.txt`, `source-manifest.json`
and `DAW-source.zip` with the binary and licenses. See the linked source guide
for their locations in macOS packages. The source archive includes tracked repository demo
fixtures, but excludes untracked user data and build caches.

## Regenerate the app icon

Install full Xcode with Icon Composer; Command Line Tools alone are insufficient
for icon regeneration. Run these steps from the repository root:

1. Open `crates/ui/assets/app-icon/DAW.icon` in Icon Composer, edit it, and save.
2. Regenerate the macOS exports:

   ```sh
   python3 tools/export_app_icons.py --platform macos
   ```

   This updates `Assets.car`, `DAW.icns`, and `macos.png` in
   `crates/ui/assets/app-icon`. Windows/Linux icons are unchanged.
3. Review the source and generated assets. Local builds can use uncommitted
   changes. Distribution packages require a clean committed tracked tree.
4. Rebuild the app bundle:

   ```sh
   sh tools/package_macos.sh debug --local
   ```

   Use `release` for an optimized build.
5. Quit the running app and reopen `target/debug/DAW.app`, or
   `target/release/DAW.app` for a release build, to load the updated icon.

To check the static Cargo-launch icon without creating a bundle, run
`cargo run -p daw-desktop` after regeneration. The layered icon requires the
rebuilt app bundle.
