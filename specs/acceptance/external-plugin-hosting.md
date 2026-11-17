# External plugin hosting acceptance

See the [hosting contract and current status](../architecture/external-plugin-hosting.md).
No complete hosting acceptance is claimed. Run every applicable case for VST3 and
CLAP, effects and instruments, on each shipped OS and architecture.

| Gate | Required coverage |
| --- | --- |
| HOST-01 Discovery | Default roots, CLAP_PATH, Unicode paths, bundles, alias/shortcut cycles, native architecture, multi-class factories, duplicate precedence, removed/changed modules, bounded traversal and corrupt caches. |
| HOST-02 Scan recovery | Native crash, timeout, malformed/oversized responses, interrupted coordinator, pending versus attempted quarantine, changed-fingerprint retry, cancellation, atomic cache replacement and no arbitrary installed binaries in normal CI. |
| HOST-03 Lifecycle | Legal actual main-thread and symbolic audio-thread calls, factory/module lifetime, split/combined VST3 controllers, CLAP sleep/restart, partial init failures, port/latency changes and proper-thread retirement. |
| HOST-04 Controls | Stable IDs across reordering/rescan, negative/nonzero plain ranges, nonlinear VST3 conversion, steps/toggles/lists, read-only/hidden/manual-only parameters, thousands of controls, synchronized mapped/native gestures and one Undo transaction. |
| HOST-05 Modulation | Shared LFO/Envelope/Macro/audio-derived providers, additive offsets and shared smoothing, neutral missing/disabled sources, base/effective separation, format capability limits, finite amounts and bounded events. |
| HOST-06 PCM sidechain | Separate follower and raw audio routing, each advertised input, silence on missing inputs, Pre/Post tap semantics, groups/Returns/sends, disabled cycles, unequal input/control latency, bypass and dynamic latency/layout changes. |
| HOST-07 Instruments | Polyphonic chords, overlapping same-key notes, sample-offset terminations, beat retrigger/choke priority, velocity, pause/stop/seek/loops, host output gate and dormant built-in settings. |
| HOST-08 Rendering | Built-in parity, all six project rates and 32–4096 frame buffers, irregular device spans, independent live/export/seek instances, cancellable replay, no foreign clone/double processing, evaluated/consumed clocks and deterministic fixture parity after alignment. |
| HOST-09 State | Legacy/unknown/missing round trips, bounded corrupt state, VST3 component/controller order, no-state CLAP limits, opaque preset Undo, save versus newer revisions, Save As/history-only ownership, copy/restore failures and transactional exports. |
| HOST-10 Native UI | Real editor attachment/floating, multiple windows, resize/DPI/focus/text, timers/FD services, removal during playback, reopen, teardown and installed-app signing/loading policy on macOS, Windows and Linux. |
| HOST-11 Real time | Zero host callback allocation/final-owner destruction, locks, waits, logs, loader/state/editor/text/file operations; full queues, event overflow, nonfinite audio, errors and renderer retirement. Instrument foreign calls separately; Rust allocator guards do not observe all vendor activity. |
| HOST-12 Distribution | Exact locked dependency grants/generated binding provenance, notices in all applicable packages, native helper lookup, device-free/display-free CLI, strict/degraded export behavior, listening and actual device deadline measurements. |

Tests use owned deterministic fixtures. Record executed commands separately from
future tests. Record exact plugin fingerprints, hardware, rate/buffer, native
latency, callback deadlines and remaining platform/device/listening limits.
Do not infer complete native acceptance from egui tests or cross-compilation.
