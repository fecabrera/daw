#!/usr/bin/env bash
# Run from the workspace root on the Linux CI runner.
printf '\n%s\n' "$1"
df -h -- / . "$RUNNER_TEMP" || true

for directory in target/debug "${CARGO_HOME:-$HOME/.cargo}" "$RUNNER_TEMP"; do
  if [[ -d "$directory" ]]; then
    du -sh -- "$directory" || true
  fi
done
