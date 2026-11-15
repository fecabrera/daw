#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=debug
profile_set=0
local_build=0
usage() {
  echo 'Usage: sh tools/package_linux.sh [debug|release] [--local]'
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
if [ "$(uname -s)" != Linux ]; then
  echo 'Run this packager on Linux.' >&2
  exit 2
fi
if [ "$local_build" = 0 ]; then
  source_revision="$(python3 tools/package_sources.py --revision-only)"
fi
executable="$(python3 tools/build_desktop.py "$profile" linux)"
mkdir -p "$PWD/target/$profile"
# Name the archive for the validated Rust target, including translated hosts.
build_target="$(basename "$(dirname "$(dirname "$executable")")")"
case "$build_target" in
  x86_64-*-linux-*) architecture=x64 ;;
  aarch64-*-linux-*) architecture=arm64 ;;
  *) echo 'Unsupported Linux architecture.' >&2; exit 2 ;;
esac

staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-linux.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT
trap 'exit 1' HUP INT TERM
package="$staging_directory/DAW"
mkdir -p "$package/licenses"
cp "$executable" "$package/daw-desktop"
chmod 755 "$package/daw-desktop"
cp LICENSE "$package/LICENSE.txt"
cp docs/development/linux-runtime.txt "$package/LINUX-REQUIREMENTS.txt"
mkdir -p "$package/share/applications" "$package/share/icons"
cp crates/ui/assets/app-icon/org.daw.DAW.desktop "$package/share/applications/"
cp -R crates/ui/assets/app-icon/hicolor "$package/share/icons/"
cp crates/unit-ui/assets/fonts/OFL.txt "$package/licenses/Outfit-OFL.txt"
cp crates/unit-ui/assets/fonts/Hack-LICENSE.txt "$package/licenses/Hack-LICENSE.txt"
cp crates/ui/assets/icons/LUCIDE-LICENSE.txt "$package/licenses/LUCIDE-LICENSE.txt"
cp crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt "$package/licenses/EGUI-LUCIDE-LICENSE.txt"
for directory in crates/ui/assets/licenses crates/unit-ui/assets/licenses crates/media/assets/licenses \
  crates/oscillator/assets/licenses crates/equalizer/assets/licenses crates/timestretch/licenses crates/separation/assets/licenses; do
  cp "$directory"/*.txt "$package/licenses/"
done
if [ "$local_build" = 1 ]; then
  cat > "$package/LOCAL-BUILD.txt" <<'NOTE'
Local development build from the current working tree, including uncommitted changes.
No corresponding-source archive is included. This is not a distribution package.
Keep LICENSE.txt and the licenses directory with this app.
NOTE
else
  python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
    --destination "$package" --binary "$package/daw-desktop"
fi
(cd "$staging_directory" && zip -qr "$staging_directory/DAW.zip" DAW)
# Publish only a complete archive; failures retain the previous artifact.
artifact="target/$profile/DAW-linux-$architecture.zip"
mv -f "$staging_directory/DAW.zip" "$artifact"
printf '%s\n' "$artifact"
