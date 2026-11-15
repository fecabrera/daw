# macOS application packaging

`tools/package_macos.sh [debug|release] [--local]` builds the native desktop
executable with `tools/build_desktop.py` and assembles `target/<profile>/DAW.app`.
The default profile is debug. Accept `--local` before or after the profile;
reject unknown arguments and multiple profiles with exit code 2 before building.

Distribution mode is the default. It checks the clean committed tracked tree
before compilation and includes the corresponding-source archive, manifest and
access notice through `tools/package_sources.py`. The DMG packager continues to
use this mode. There is no local DMG option.

Local mode builds the current working tree without checking Git state or vendoring sources.
It includes the same executable, native icon catalog, ICNS fallback, plist,
project license and dependency notices. It adds `LOCAL-BUILD.txt` in Resources
and omits `DAW-source.zip`, `source-manifest.json` and `SOURCE-ACCESS.txt`.
No commit, checkout, index change or user-data collection is performed.

Both modes assemble a fresh staging bundle. Build, assembly and source-package
failures retain the previous app. Publish only the complete bundle; restore
the previous app if its replacement fails. Fresh assembly also prevents stale
distribution metadata or local-build labels when switching modes. The executable
is copied to a fresh inode so an existing process can retain its mapped binary.

Packaging fixtures cover default distribution contents, local mode on a dirty
tree and without Git history, both argument orders, default debug profile,
native target selection, icon/license retention, omitted source metadata,
invalid arguments and failure preservation. Native application launch and
Finder/Dock presentation remain separate checks.
