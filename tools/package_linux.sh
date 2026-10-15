#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Linux ]; then
  echo 'Run this packager on Linux.' >&2
  exit 2
fi
profile="${1:-debug}"
case "$profile" in
  debug) cargo build --locked -p daw-desktop ;;
  release) cargo build --locked -p daw-desktop --release ;;
  *) echo "Usage: $0 [debug|release]" >&2; exit 2 ;;
esac
case "$(uname -m)" in
  x86_64) architecture=x64 ;;
  aarch64|arm64) architecture=arm64 ;;
  *) echo 'Unsupported Linux architecture.' >&2; exit 2 ;;
esac

staging_directory="$(mktemp -d "$PWD/target/$profile/.daw-linux.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT
trap 'exit 1' HUP INT TERM
package="$staging_directory/DAW"
mkdir -p "$package/licenses"
cp "target/$profile/daw-desktop" "$package/daw-desktop"
chmod 755 "$package/daw-desktop"
cp LICENSE "$package/LICENSE.txt"
cp crates/ui/assets/fonts/OFL.txt "$package/licenses/Outfit-OFL.txt"
cp crates/ui/assets/fonts/Hack-LICENSE.txt "$package/licenses/Hack-LICENSE.txt"
cp crates/ui/assets/icons/LUCIDE-LICENSE.txt "$package/licenses/LUCIDE-LICENSE.txt"
cp crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt "$package/licenses/EGUI-LUCIDE-LICENSE.txt"
for directory in crates/ui/assets/licenses crates/media/assets/licenses \
  crates/oscillator/assets/licenses crates/equalizer/assets/licenses crates/timestretch/licenses; do
  cp "$directory"/*.txt "$package/licenses/"
done
(cd "$staging_directory" && zip -qr "$staging_directory/DAW.zip" DAW)
# Publish only a complete archive; failures retain the previous artifact.
artifact="target/$profile/DAW-linux-$architecture.zip"
mv -f "$staging_directory/DAW.zip" "$artifact"
printf '%s\n' "$artifact"
