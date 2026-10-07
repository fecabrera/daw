#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile="${1:-debug}"
case "$profile" in
  debug) cargo build -p daw-desktop ;;
  release) cargo build -p daw-desktop --release ;;
  *) echo "Usage: $0 [debug|release]" >&2; exit 2 ;;
esac
bundle="target/$profile/DAW.app"
mkdir -p "$bundle/Contents/MacOS"
mkdir -p "$bundle/Contents/Resources/licenses"
cp "target/$profile/daw-desktop" "$bundle/Contents/MacOS/daw-desktop"
cp crates/ui/assets/fonts/OFL.txt "$bundle/Contents/Resources/licenses/Outfit-OFL.txt"
rm -f "$bundle/Contents/Resources/licenses/DM-Sans-OFL.txt"
cp crates/ui/assets/fonts/Hack-LICENSE.txt "$bundle/Contents/Resources/licenses/Hack-LICENSE.txt"
cp crates/ui/assets/icons/LUCIDE-LICENSE.txt "$bundle/Contents/Resources/licenses/LUCIDE-LICENSE.txt"
cp crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt "$bundle/Contents/Resources/licenses/EGUI-LUCIDE-LICENSE.txt"
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
echo "$bundle"
