# Common parsing and source preflight for the POSIX package entry points.
package_arguments() {
  package_script="$1"
  shift
  profile=debug
  profile_set=0
  local_build=0
  usage() {
    printf 'Usage: sh tools/%s [debug|release] [--local]\n' "$package_script"
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
}

package_source_revision() {
  if [ "$local_build" = 0 ]; then
    source_revision="$(python3 tools/package_sources.py --revision-only)"
  fi
}

package_local_marker() {
  cat > "$1" <<'NOTE'
Local development build from the current working tree, including uncommitted changes.
No corresponding-source archive is included. This is not a distribution package.
Keep LICENSE.txt and the licenses directory with this app.
NOTE
}
