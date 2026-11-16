# Silent stopped MIDI seeks — 2026-11-16

Moving the playhead in Timeline or Editor briefly emitted a note when a MIDI input
was selected. The user identified Auto-filter in the affected track chain.

Live monitoring processes the shared instrument/effect buses while transport is
stopped. Cached seek reconstruction populated those buses and alignment buffers
with arrangement history. This history could escape through live monitoring.
The buffered-note regression reproduced nonzero output with no live key press.

The output worker now clears the returned renderer's arrangement history before
stopped or paused monitoring. Cached checkpoints remain intact. Live reserves
are still prepared before publication, and compatible held live voices retain
the existing callback handoff. Play after a prepared paused seek requests exact
history reconstruction. Ordinary pause/resume retains its immediate path.

## Automated checks

- Output library: 99 passed. Tests cover Auto-filter, buffered master processing,
  stopped/paused seeks, multiple positions, silent output without key presses,
  working live notes afterward, and a fixed stopped playhead. Callback processing
  has no allocation or final-owner destruction.
- A separate Auto-filter playback test checks exact post-seek sample parity,
  the original playback-start position and ordinary pause/resume behavior.
- Full workspace debug and release: 3,533 passed and 22 ignored in each profile.
- Strict workspace Clippy, CLI build and smoke checks, formatting, local links,
  and `git diff --check`: passed.
- Local macOS debug package rebuilt and passed signature verification.

The initial buffered-history regression failed before the reset with a peak of
0.123 and passed after it with zero output. A simple default Auto-filter without
buffered master processing did not reproduce the failure in isolation. The final
regression also covers Auto-filter with a latency-bearing master, and the
playback-parity test covers Auto-filter without that added master stage.

Logs are local ignored `work/midi-seek-*.log` files. No user project, recording or
manifest was modified. Native Timeline/Editor and physical-keyboard confirmation,
plus Windows/Linux native checks, remain open.
