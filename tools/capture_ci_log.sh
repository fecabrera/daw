#!/usr/bin/env bash
# Mirror command output to the job log and the artifact; keep Bash pipeline status.
set -o pipefail
append=()
if [[ $1 == --append ]]; then
  append=(-a)
  shift
fi
output=$1
shift
"$@" 2>&1 | tee "${append[@]}" "$output"
