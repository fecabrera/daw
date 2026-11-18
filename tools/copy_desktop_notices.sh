#!/bin/sh
# Stage the common notices from the repository root.
set -eu
notice_directory="$1"
while IFS='|' read -r notice_source notice_name; do
  if [ -n "$notice_name" ]; then
    cp "$notice_source" "$notice_directory/$notice_name"
  else
    cp "$notice_source"/*.txt "$notice_directory/"
  fi
done < tools/desktop_notices.txt
