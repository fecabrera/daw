#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Linux ]; then
  echo 'Run this packager on Linux.' >&2
  exit 2
fi
source_revision="$(python3 tools/package_sources.py --revision-only)"
profile="${1:-debug}"
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
cp crates/unit-ui/assets/fonts/OFL.txt "$package/licenses/Outfit-OFL.txt"
cp crates/unit-ui/assets/fonts/Hack-LICENSE.txt "$package/licenses/Hack-LICENSE.txt"
cp crates/ui/assets/icons/LUCIDE-LICENSE.txt "$package/licenses/LUCIDE-LICENSE.txt"
cp crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt "$package/licenses/EGUI-LUCIDE-LICENSE.txt"
for directory in crates/ui/assets/licenses crates/unit-ui/assets/licenses crates/media/assets/licenses \
  crates/oscillator/assets/licenses crates/equalizer/assets/licenses crates/timestretch/licenses crates/separation/assets/licenses; do
  cp "$directory"/*.txt "$package/licenses/"
done
python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
  --destination "$package" --binary "$package/daw-desktop"
(cd "$staging_directory" && zip -qr "$staging_directory/DAW.zip" DAW)
# Publish only a complete archive; failures retain the previous artifact.
artifact="target/$profile/DAW-linux-$architecture.zip"
mv -f "$staging_directory/DAW.zip" "$artifact"
printf '%s\n' "$artifact"
