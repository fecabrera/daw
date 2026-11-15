#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'Run this packager on macOS.' >&2
  exit 2
fi
profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *) echo "Usage: $0 [debug|release]" >&2; exit 2 ;;
esac
case "$(uname -m)" in
  arm64) architecture=arm64 ;;
  x86_64) architecture=x64 ;;
  *) echo 'Unsupported macOS architecture.' >&2; exit 2 ;;
esac

sh tools/package_macos.sh "$profile"
staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-dmg.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT
trap 'exit 1' HUP INT TERM
mkdir "$staging_directory/image"
cp -R "target/$profile/DAW.app" "$staging_directory/image/DAW.app"
cp -R "target/$profile/DAW-source-metadata" "$staging_directory/image/DAW-source-metadata"
codesign --verify --deep --strict --verbose=2 "$staging_directory/image/DAW.app"
ln -s /Applications "$staging_directory/image/Applications"
# Hosted macOS workers can temporarily hold disk-image resources busy.
# Retry only that error; retain diagnostics and never reuse a partial image.
attempt=1
while :; do
  if LC_ALL=C hdiutil create -volname DAW -srcfolder "$staging_directory/image" \
    -format UDZO "$staging_directory/DAW.dmg" 2>"$staging_directory/create.log"; then
    cat "$staging_directory/create.log" >&2
    break
  else
    status=$?
  fi
  cat "$staging_directory/create.log" >&2
  if [ "$attempt" -ge 5 ] || ! grep -Fq 'Resource busy' "$staging_directory/create.log"; then
    exit "$status"
  fi
  rm -f "$staging_directory/DAW.dmg"
  delay=$((attempt * 2))
  echo "Disk image resources busy; retrying in $delay seconds (attempt $((attempt + 1))/5)." >&2
  sleep "$delay"
  attempt=$((attempt + 1))
done
hdiutil verify "$staging_directory/DAW.dmg"
# Publish only a complete disk image; failures retain the previous artifact.
artifact="target/$profile/DAW-macos-$architecture.dmg"
mv -f "$staging_directory/DAW.dmg" "$artifact"
printf '%s\n' "$artifact"
