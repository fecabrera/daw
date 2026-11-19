#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
. tools/package_desktop_posix.sh
package_arguments package_linux.sh "$@"

if [ "$(uname -s)" != Linux ]; then
  echo 'Run this packager on Linux.' >&2
  exit 2
fi
package_source_revision
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
sh tools/copy_desktop_notices.sh "$package/licenses"
if [ "$local_build" = 1 ]; then
  package_local_marker "$package/LOCAL-BUILD.txt"
else
  python3 tools/package_sources.py --revision "$source_revision" --profile "$profile" \
    --destination "$package" --binary "$package/daw-desktop"
fi
(cd "$staging_directory" && zip -qr "$staging_directory/DAW.zip" DAW)
# Publish only a complete archive; failures retain the previous artifact.
artifact="target/$profile/DAW-linux-$architecture.zip"
mv -f "$staging_directory/DAW.zip" "$artifact"
printf '%s\n' "$artifact"
