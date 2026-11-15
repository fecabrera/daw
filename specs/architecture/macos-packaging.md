# macOS application packaging

`tools/package_macos.sh [debug|release] [--local]` builds the native desktop
executable with `tools/build_desktop.py` and assembles `target/<profile>/DAW.app`.
The default profile is debug. Accept `--local` before or after the profile;
reject unknown arguments and multiple profiles with exit code 2 before building.

Distribution mode is the default. It checks the clean committed tracked tree
before compilation and includes the corresponding-source archive, manifest and
access notice through `tools/package_sources.py`. Keep `DAW-source.zip` and a
stable source-access note in Resources. Store the exact signed-executable receipt
and manifest in sibling `DAW-source-metadata`, with paths relative to that directory.
The internal note records the source revision and archive checksum. The external
manifest also records the final executable checksum; putting that checksum inside
the signed bundle would create a circular resource seal. The DMG packager uses
this distribution mode. There is no local DMG option.

Local mode builds the current working tree without checking Git state or vendoring sources.
It includes the same executable, native icon catalog, ICNS fallback, plist,
project license and dependency notices. It adds `LOCAL-BUILD.txt` in Resources
and omits `DAW-source.zip`, `source-manifest.json` and `SOURCE-ACCESS.txt`.
No commit, checkout, index change or user-data collection is performed.

Both modes assemble a fresh staging bundle. After all resources are installed,
apply an ad-hoc signature with `codesign --force --sign - --timestamp=none`, then
verify the complete bundle with `--deep --strict`. Refresh only the external
receipt after signing, checking the source revision and archive checksum again.
Do not modify sealed resources afterward. The linker’s executable-only signature
is insufficient for a complete app bundle. No Developer ID or notarization is supplied.

Build, assembly, source-package and signing failures retain the previous app and
receipts. Publish the complete app and companion receipts with rollback of both
on replacement failure. Retain backups and report their path if rollback fails.
Fresh assembly also prevents stale distribution metadata or local-build labels when switching modes. The executable
is copied to a fresh inode so an existing process can retain its mapped binary.

Packaging fixtures cover default distribution contents, local mode on a dirty
tree and without Git history, both argument orders, default debug profile,
native target selection, icon/license retention, omitted source metadata,
invalid arguments and failure preservation. Native application launch and
Finder/Dock presentation remain separate checks.

The DMG contains the signed app, companion receipts and Applications link. Verify
the copied app before creating the image, and verify the image checksum before
publishing it. A failed image step retains the previous DMG. Ad-hoc signing checks
integrity; trusted browser distribution still requires Developer ID signing and
Apple notarization. Native fixtures check the resource seal, exact final hashes,
copy preservation, tamper rejection and rollback after receipt publication failure.
