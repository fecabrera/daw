# Desktop application icons

The app uses the approved V4 split waveform: eight orange capsule bars with
offset left and right halves. Assets belong to `crates/ui/assets/app-icon`.
The original raster concepts were generated with the built-in image tool.
The macOS product now uses an editable native Icon Composer document with a
vector waveform and a native graphite background, as requested. No third-party
logo or icon pack is used.

## Platform variants

| Platform | Appearance | Integration |
| --- | --- | --- |
| macOS bundle | Native graphite background and orange glass waveform | `DAW.icon` compiled to `Assets.car` and `DAW.icns`; plist declares `CFBundleIconName=DAW` and the fallback icon file |
| Direct macOS Cargo launch | Apple-rendered static version with native padding | Embedded `macos.png` supplied to eframe because no bundle catalog exists |
| Windows | Shared orange waveform on rounded graphite tile | Multi-resolution `DAW.ico` embedded with Windows SDK `rc.exe`; `windows.png` supplied to eframe |
| Linux | Same shared waveform and rounded graphite tile | Scalable SVG and eight PNG sizes embedded in a GTK GResource, plus packaged hicolor icons and matching desktop entry |

The CLI and effect plugins do not use the application icon. Icon lookup and
image decoding run during startup, outside audio processing.

## Native macOS ownership and size

`DAW.icon/icon.json` defines the graphite fill, waveform group, native shadow and
translucency. `DAW.icon/Assets/Waveform.svg` defines the eight capsule shapes.
The layers contain no baked corner mask, outer padding, rim or glass lighting.
Apple's compiler and renderer supply those effects and the platform geometry.
This follows [Apple's Icon Composer workflow](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)
and [app-icon guidance](https://developer.apple.com/design/human-interface-guidelines/app-icons).

When the main bundle declares `CFBundleIconName`, the desktop passes an explicitly
empty icon to eframe. eframe treats this value as a request to leave the native
application icon alone. This preserves the layered catalog and system appearance.
Leaving the viewport icon unset would allow eframe's default icon; assigning a
PNG would replace the layered icon with a static image. Direct Cargo launches
have no catalog, so they receive the Apple-rendered static fallback instead.

The fallback PNG is decoded from Apple's compiled ICNS before resizing. It
retains Apple's optical padding; it is not generated from an unpadded Composer
canvas preview. Keep native rounding and scaling with the renderer rather than
manually changing the Dock size.

## Source artwork and exports

All platforms share `DAW.icon/Assets/Waveform.svg` as their waveform source.
macOS additionally uses the gradient, glass properties and layer transforms in
`DAW.icon/icon.json`. Windows/Linux compose the unmodified waveform SVG over
`desktop-background.svg`, which preserves their rounded graphite tile. The
outer margins remain transparent. Linux installs this self-contained composite
as its scalable icon; PNG fallbacks and the Windows ICO use the same composition.
Keep source documents and exports in version control; remove obsolete raster
masters and duplicate exports.

```sh
python3 tools/export_app_icons.py --platform macos
python3 tools/export_app_icons.py --platform windows
python3 tools/export_app_icons.py --platform linux
```

Omitting `--platform` exports all platforms; `--platform desktop` exports only
Windows and Linux. macOS export requires full Xcode with Icon
Composer and uses `actool` plus `sips`. Tool lookup respects `DEVELOPER_DIR`, then
checks the selected developer directory and the standard Xcode installation.
Windows/Linux raster export requires `rsvg-convert` from librsvg. Linux also
requires `glib-compile-resources` from GLib for its embedded resource. Ordinary builds
use committed exports and need no Composer, graphics session or image service.
Regenerate macOS assets on a native machine with graphics-service access.

The macOS packager copies `Assets.car` and `DAW.icns` into Resources. The Linux
ZIP includes its desktop entry and hicolor files under `share`. Windows resource
compilation failure stops the build; Linux resource-registration failure logs a
warning and retains the platform fallback. Embedded viewport PNGs must decode
successfully at startup.

## Verification

Packaging fixtures check the macOS plist keys and catalog/icon bytes, every
Linux icon size, the launcher icon name and transactional package publication.
Inspect native Composer rendering, catalog contents and fallback optical bounds.
Check the running bundle after the first frame so a runtime override cannot go
unnoticed. Finder/Dock, Windows Explorer/taskbar and Linux shell appearance
checks remain distinct from file, compilation and packaging checks.
