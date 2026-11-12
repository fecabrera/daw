#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
source_revision="$(python3 tools/package_sources.py --revision-only)"
profile="${1:-debug}"
executable="$(python3 tools/build_desktop.py "$profile" macos)"
mkdir -p "$PWD/target/$profile"
artifact="target/$profile/DAW.app"
staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-app.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT
trap 'exit 1' HUP INT TERM
bundle="$staging_directory/DAW.app"
mkdir -p "$bundle/Contents/MacOS"
mkdir -p "$bundle/Contents/Resources/licenses"
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
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
  --destination "$bundle/Contents/Resources" --binary "$bundle/Contents/MacOS/daw-desktop"
# A failed source/vendor step must not replace the previous complete application.
if [ -e "$artifact" ]; then mv "$artifact" "$staging_directory/previous.app"; fi
if ! mv "$bundle" "$artifact"; then
  if [ -e "$staging_directory/previous.app" ]; then mv "$staging_directory/previous.app" "$artifact"; fi
  exit 1
fi
echo "$artifact"
