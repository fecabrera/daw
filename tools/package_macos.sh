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
metadata_artifact="target/$profile/DAW-source-metadata"
staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-app.XXXXXX")"
app_published=0
metadata_published=0
publish_complete=0
cleanup() {
  status=$?
  trap - EXIT
  rollback_failed=0
  if [ "$publish_complete" = 0 ]; then
    if [ "$app_published" = 1 ]; then rm -rf "$artifact" || rollback_failed=1; fi
    if [ "$metadata_published" = 1 ]; then rm -rf "$metadata_artifact" || rollback_failed=1; fi
    if [ -e "$staging_directory/previous.app" ]; then
      if [ -e "$artifact" ]; then rollback_failed=1
      else mv "$staging_directory/previous.app" "$artifact" || rollback_failed=1; fi
    fi
    if [ -e "$staging_directory/previous-metadata" ]; then
      if [ -e "$metadata_artifact" ]; then rollback_failed=1
      else mv "$staging_directory/previous-metadata" "$metadata_artifact" || rollback_failed=1; fi
    fi
  fi
  if [ "$rollback_failed" = 1 ]; then
    echo "Cannot restore the previous package; retained backups in $staging_directory" >&2
    exit 1
  fi
  rm -rf "$staging_directory"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
bundle="$staging_directory/DAW.app"
receipts="$staging_directory/DAW-source-metadata"
mkdir -p "$bundle/Contents/MacOS"
mkdir -p "$bundle/Contents/Resources/licenses"
cp crates/ui/assets/app-icon/DAW.icns "$bundle/Contents/Resources/DAW.icns"
cp crates/ui/assets/app-icon/Assets.car "$bundle/Contents/Resources/Assets.car"
# Publish a fresh inode so a running DAW instance can keep its mapped executable.
temporary_executable="$(mktemp "$bundle/Contents/MacOS/.daw-desktop.XXXXXX")"
cp "$executable" "$temporary_executable"
chmod 755 "$temporary_executable"
mv -f "$temporary_executable" "$bundle/Contents/MacOS/daw-desktop"
# Remove the obsolete helper when rebuilding an existing local bundle.
rm -f "$bundle/Contents/MacOS/daw-plugin-scan"
cp LICENSE "$bundle/Contents/Resources/LICENSE.txt"
rm -f "$bundle/Contents/Resources/licenses/DM-Sans-OFL.txt"
sh tools/copy_desktop_notices.sh "$bundle/Contents/Resources/licenses"
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
    --destination "$bundle/Contents/Resources" --binary "$bundle/Contents/MacOS/daw-desktop" \
    --receipt-destination "$receipts"
fi
# Sign the complete bundle, including icons and source materials. The linker's
# executable-only signature does not seal an application bundle's resources.
codesign --force --sign - --timestamp=none "$bundle"
codesign --verify --deep --strict --verbose=2 "$bundle"
if [ "$local_build" = 0 ]; then
  # Signing changes the executable. Keep its final checksum outside the seal.
  python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
    --destination "$bundle/Contents/Resources" --binary "$bundle/Contents/MacOS/daw-desktop" \
    --receipt-destination "$receipts" --refresh-receipt
fi
# Publish the signed app and its matching receipt as one rollback transaction.
if [ -e "$artifact" ]; then mv "$artifact" "$staging_directory/previous.app"; fi
if [ -e "$metadata_artifact" ]; then mv "$metadata_artifact" "$staging_directory/previous-metadata"; fi
app_published=1
mv "$bundle" "$artifact"
if [ -d "$receipts" ]; then
  metadata_published=1
  mv "$receipts" "$metadata_artifact"
fi
publish_complete=1
echo "$artifact"
