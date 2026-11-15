#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=debug
profile_set=0
local_build=0
usage() {
  echo 'Usage: sh tools/package_macos.sh [debug|release] [--local]'
}
for argument in "$@"; do
  case "$argument" in
    debug|release)
      if [ "$profile_set" = 1 ]; then usage >&2; exit 2; fi
      profile="$argument"
      profile_set=1
      ;;
    --local) local_build=1 ;;
    --help|-h) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done
if [ "$local_build" = 0 ]; then
  source_revision="$(python3 tools/package_sources.py --revision-only)"
fi
executable="$(python3 tools/build_desktop.py "$profile" macos)"
mkdir -p "$PWD/target/$profile"
artifact="target/$profile/DAW.app"
staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-app.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT
trap 'exit 1' HUP INT TERM
bundle="$staging_directory/DAW.app"
mkdir -p "$bundle/Contents/MacOS"
mkdir -p "$bundle/Contents/Resources/licenses"
cp crates/ui/assets/app-icon/DAW.icns "$bundle/Contents/Resources/DAW.icns"
cp crates/ui/assets/app-icon/Assets.car "$bundle/Contents/Resources/Assets.car"
# Publish a fresh inode so a running DAW instance can keep its mapped executable.
temporary_executable="$(mktemp "$bundle/Contents/MacOS/.daw-desktop.XXXXXX")"
cp "$executable" "$temporary_executable"
chmod 755 "$temporary_executable"
mv -f "$temporary_executable" "$bundle/Contents/MacOS/daw-desktop"
cp LICENSE "$bundle/Contents/Resources/LICENSE.txt"
cp crates/unit-ui/assets/fonts/OFL.txt "$bundle/Contents/Resources/licenses/Outfit-OFL.txt"
rm -f "$bundle/Contents/Resources/licenses/DM-Sans-OFL.txt"
cp crates/unit-ui/assets/fonts/Hack-LICENSE.txt "$bundle/Contents/Resources/licenses/Hack-LICENSE.txt"
cp crates/ui/assets/icons/LUCIDE-LICENSE.txt "$bundle/Contents/Resources/licenses/LUCIDE-LICENSE.txt"
cp crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt "$bundle/Contents/Resources/licenses/EGUI-LUCIDE-LICENSE.txt"
cp crates/ui/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/unit-ui/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/separation/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/media/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/timestretch/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/oscillator/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cp crates/equalizer/assets/licenses/*.txt "$bundle/Contents/Resources/licenses/"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>daw-desktop</string>
<key>CFBundleIdentifier</key><string>dev.daw.prototype</string>
<key>CFBundleName</key><string>DAW</string>
<key>CFBundleIconFile</key><string>DAW.icns</string>
<key>CFBundleIconName</key><string>DAW</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
if [ "$local_build" = 1 ]; then
  cat > "$bundle/Contents/Resources/LOCAL-BUILD.txt" <<'NOTE'
Local development build from the current working tree, including uncommitted changes.
No corresponding-source archive is included. This is not a distribution package.
Keep LICENSE.txt and the licenses directory with this app.
NOTE
else
  python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
    --destination "$bundle/Contents/Resources" --binary "$bundle/Contents/MacOS/daw-desktop"
fi
# Failed builds, assembly and source/vendor steps retain the previous app.
if [ -e "$artifact" ]; then mv "$artifact" "$staging_directory/previous.app"; fi
if ! mv "$bundle" "$artifact"; then
  if [ -e "$staging_directory/previous.app" ]; then mv "$staging_directory/previous.app" "$artifact"; fi
  exit 1
fi
echo "$artifact"
