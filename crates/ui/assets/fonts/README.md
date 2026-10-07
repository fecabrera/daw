# Outfit

The UI embeds the unmodified upright variable font from [Google Fonts](https://github.com/google/fonts/tree/main/ofl/outfit). Regular text uses the `wght` axis at 400. The semibold family uses the same bytes at 600. The font's default weight is 100, so both UI weights are set explicitly.

- Font: [Outfit[wght].ttf](https://raw.githubusercontent.com/google/fonts/main/ofl/outfit/Outfit%5Bwght%5D.ttf), saved locally as Outfit.ttf.
- License and copyright: [OFL.txt](OFL.txt), included unchanged.
- Download date: 2026-10-07.
- Font SHA-256: `fc7287273e66929776e2ba54f144fe699080bec29f61bf649d70d871468aeade`.

The font bytes are compiled into the desktop executable. egui's default fonts remain available as symbol fallbacks. The CLI does not include this UI asset.

The transport time display uses Outfit at 13 points. All characters use semibold weight. Digits use the default text color and are centered in equal-width slots sized for the widest semibold digit. The `h`, `m`, `s`, and `.` characters use a slightly darker gray and retain their normal widths. egui also bundles Hack as its default monospace font; its license is included as `Hack-LICENSE.txt` and copied into the macOS bundle.
