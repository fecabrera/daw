# Real-time and background work

The audio callback must remain nonblocking and must not allocate or perform file I/O.

- Do not decode audio, build peaks, stretch clips, serialize manifests, show dialogs, log, acquire locks, or wait on channels in the callback.
- Prepare renderers, buffers, meter handles, and queue capacity on a control or worker thread. Retire old renderers and final audio owners outside the callback.
- Use the existing bounded `rtrb` queues and atomic status values. Handle queue-full and device-failure paths without blocking the callback.
- All current and future unit meters must reuse `unit-core::UnitMeter` and `meters::unit_reading` for the [shared bypass lifecycle](../../../specs/architecture/modular-audio-units.md#effect-meter-lifecycle). Disabled units clear all their meters, show empty rails and zero numeric readings, and stop updates until re-enabled. Mask queued or stale readings while the unit is off. Retain handles until processor retirement; preserve audio ramps, detector history, and latency.
- Run import, source preparation, project I/O, stretching, and export through the existing worker/job paths. Keep expensive work out of egui painting and input handlers.
- Stage changes and publish them after successful work. Retain current project state on failure or cancellation. Route errors and warnings through the existing UI/CLI reporting paths.
