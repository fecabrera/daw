# Drive plugin Phase 0: adapter and host feasibility

Status: local macOS arm64 feasibility experiment implemented. Narrow format adapters are selected for the production implementation. Drive plugin processors and the shared two-strip product editor are not implemented. Windows/Linux and external-DAW checks remain open.

This record implements [Phase 0](unit-plugin-builds.md#phase-0--adapter-and-host-feasibility) of the separate plugin build plan. The original built-in `daw.drive` remains unchanged. Sources and fixtures are development tools; generated libraries, SDK checkouts, measurements, and logs stay under ignored `work/drive-plugin-phase0/`.

## Decision

Use narrow CLAP and VST3 adapters over the existing portable Drive DSP, with a shared editor built on the version-compatible egui window backend. Do not adopt nice-plug 0.4.2 unchanged as the production runtime.

The maintained framework builds both formats and proves native editor feasibility. However, two independent checks fail the accepted production contract even though the standard validators pass:

1. A VST3 queue with points `(0, 1.0)` and `(31, 0.0)` produces steps, rather than the linear values between those points. Framework smoothing is disabled in the probe. The host compares every sample to the VST3 queue contract, so this is independent of Drive's own five-millisecond ramps.
2. A legal queue containing 2,048 distinct sample-offset points in a 2,048-frame block causes one Rust allocation and one deallocation during the framework's processing callback. Host queue construction is outside the measured region. The framework reserves 1,024 input events and permits its event vector to grow when full.

A larger event reserve does not establish a general no-allocation contract. Enabling framework smoothing also does not reconstruct the VST3 automation curve before the portable DSP's existing ramp. Replacing those paths would require maintaining a framework fork. The first Drive effect has seven known parameters and no note input, so direct format adapters are the selected bounded implementation path.

This is a dependency and implementation decision, not evidence that the future narrow adapters already pass their production tests. They must pass the protocol, state, allocation, editor, and host gates in Phases 2–5.

## Compared stacks and exact versions

The isolated [tool manifest](../../tools/drive_plugin_phase0/Cargo.toml) and [lockfile](../../tools/drive_plugin_phase0/Cargo.lock) pin the executable experiment. These dependencies are not added to the DAW workspace.

| Component | Version / revision | Finding and role |
| --- | --- | --- |
| nice-plug | 0.4.2; published source revision `91ba75d20c43ca0cd21afa802d0ae8c0511f48ed` | Maintained dual-format candidate; ISC. Builds with the pinned compiler and passes standard validators. Rejected unchanged for the two callback/automation gaps above. |
| nice-plug-core | 0.4.2; same published revision | ISC; candidate parameter/lifecycle contract. Kept only in the feasibility tool. |
| nice-plug-egui | 0.5.0; published revision `ce793d8275aebf7589e35fb5fc1e1e3e6a016940` | ISC; version compatible with the DAW's egui. This adapter is used for the editor feasibility probe, not selected for production. |
| nice-plug-derive | 0.1.2; published revision `ba4eb8aca372be239d25cabf8b4bdfe6bbce911c` | ISC; exact upstream license text retained with the tool. |
| egui | 0.36.2 | MIT OR Apache-2.0; one resolved version shared by the prototype editor dependencies. No DAW upgrade is required. |
| egui-baseview / baseview | 0.7.2 / 0.3.7 | MIT OR Apache-2.0; OpenGL native-child-window backend verified through the candidate's editor adapter. Selected as the feasibility baseline for a thin editor-window adapter. The production adapter and full Drive body remain Phase 4 work. |
| clap-sys | 0.5.0 | MIT/Apache-2.0; selected narrow CLAP bindings, also exercised underneath the dual-format probe. |
| vst3 | 0.3.0 | MIT OR Apache-2.0; selected narrow Rust VST3 bindings, also exercised underneath the probe. These are not the older GPL-licensed `vst3-sys` bindings. |
| CLAP headers | `a47f6badb49d948fd009998f28309cdab78979c9` | Official protocol headers used by the independent native host. |
| CLAP validator | 0.4.1; `152b9823e992d782c5c1fd33bca0295478b919aa` | Built from the pinned upstream source with Rust 1.97. |
| Steinberg VST3 SDK / validator | checkout reports 3.8.1; `3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96` | Built from source with pinned SDK submodule gitlinks. This identifies the tested checkout, not a floating SDK release requirement. |

The original NIH-plug project directs framework users to its maintained successor. That successor is now nice-plug at `codeberg.org/RustAudio/nice-plug`. The inspected nice-plug packages use the permissively licensed `vst3` bindings; the original NIH-plug README's older VST3 dependency terms must not be applied to this different dependency graph. The current nice-plug-egui 0.6.0 source requires egui 0.37; the probe deliberately uses 0.5.0 and resolves egui 0.36.2. No framework, SDK, or dependency upgrade is made to the built-in DAW.

### License and notice evidence

The macOS dependency audit traverses the actual reachable Cargo graph rather than all unused package metadata. It finds 175 packages, all with license declarations. The machine-readable `dependency-inventory.json`, resolved manifest graph, and available license/notice texts are generated in scratch space. The graph contains no DAW desktop, CLI, project, engine, output, media/separation, CPAL, rfd, or eframe dependency.

The graph includes permissive license alternatives, font terms, and Unicode data terms. In particular, the prototype's default egui fonts declare OFL and Ubuntu-font terms in addition to the code license. The runner retains nested font text notices, including `Hack-Regular.txt`, `OFL.txt`, `UFL.txt`, and the emoji font MIT notice. The production editor must use the existing approved Outfit/Lucide assets and retain their notices when they are extracted.

Four published nice-plug tarballs omit their workspace's ISC text. The tool retains that text from each package's exact `.cargo_vcs_info.json` source revision in [license files and provenance](../../tools/drive_plugin_phase0/licenses/sources.json). Remaining registry text-coverage gaps are listed in generated `notice-text-gaps.json`; an SPDX declaration alone is not treated as a complete copyright notice. These local feasibility bundles are not public distribution artifacts. Phase 5 must collect and review complete notices for the selected production dependency graph. Production will not include the rejected nice-plug runtime simply because this diagnostic tool uses it.

No upstream framework examples or SDK implementation files are copied into tracked plugin source. The native host compiles pinned SDK interface/IID helper files from scratch checkouts. Keep the SDK source notices with those checkouts and with any later redistributed tools.

## Reproducible probes

Sources:

- [Python experiment runner](../../tools/probe_drive_plugins.py).
- [Disposable gain plugin](../../tools/drive_plugin_phase0/src/lib.rs), exported as CLAP and VST3 from the same test binary. Its name and reserved `example.invalid` identity explicitly distinguish it from Drive and from future public identities.
- [Independent macOS ABI host](../../tools/drive_plugin_phase0/native_host.mm), using CLAP C headers and VST3 C++ interfaces to load the compiled libraries.
- [Direct Drive baseline and tail survey](../../tools/drive_plugin_phase0/src/baseline.rs).
- [Pinned protocol sources](../../tools/drive_plugin_phase0/sources.json), [quantitative budgets](../../tools/drive_plugin_phase0/budgets.json), and [diagnostic/scratch-safety tests](../../tools/test_probe_drive_plugins.py).

Platform prerequisites and commands are in [README](../../README.md#drive-plugin-feasibility-tools). The runner downloads only with explicit `--fetch`, uses the isolated lockfile, verifies SDK revisions/submodule gitlinks, and keeps outputs in an owned subdirectory of ignored `work/`. It refuses an unmarked nonempty directory and paths outside that scratch tree. It never installs a plugin or opens a user project.

Outputs include private `.clap`/`.vst3` bundles, direct-DSP measurements, standard-validator results, independent-host observations, dependency/notice inventories, compiler/hardware metadata, and `report.json`. `--gui` briefly opens temporary host windows and verifies native child-view attachment/removal. The default run does not initialize a plugin editor. This is a native window test harness, not a general DAW or an editor visual/accessibility acceptance test.

A successful runner exit means the diagnostic experiment completed. It does not mean the tested framework satisfies the production contract. `report.json` carries separate production gates; `--require-framework-contract` exits 2 if either of those gates fails. Unit tests ensure absent measurements, a curve mismatch, or either allocation/deallocation cannot be reported as production success.

## Observed local results

The [sanitized results snapshot](../validation/drive-plugin-phase0.json) records one local run without absolute source paths, hostnames, or private projects. Hardware: Apple M5, 10 logical CPUs, 16 GiB memory; native arm64. The locked experiment uses Rust 1.97.0 and a release build with thin LTO and one codegen unit. The snapshot records the exact OS/compiler strings.

| Check | Observed result | Scope |
| --- | --- | --- |
| CLAP standard validator | 34 success, 10 skipped, no failures | Skips are the validator's unsupported/optional checks; not all CLAP features are implemented. |
| VST3 standard validator | 47 passed, 0 failed | Applies to the disposable gain probe, not Drive. |
| CLAP independent ABI host | Discovery, init/activate/start/process/reset/stop/deactivate/destroy; exact stereo pass-through; gain change at sample 17; changing block length | Host-owned planar buffers; no audio device or user project. |
| VST3 independent ABI host | Factory, component/controller, stereo negotiation, setup/activation/processing/termination; pass-through and sample-17 jump; changing block length | Uses separate SDK C++ interfaces rather than the Rust wrapper's own test abstractions. |
| VST3 linear curve | Failed additional production gate | Probe outputs steps between queue points; standard validator success does not waive this failure. |
| VST3 2,048-point automation | 1 Rust allocation and 1 Rust deallocation inside processing | Counts the loaded plugin's Rust allocator, including framework code. It is not a complete native/system-allocation profiler. |
| Native CLAP editor | Three create/embed/show/hide/destroy cycles passed | Cocoa NSView child, egui 0.36.2 and OpenGL backend. |
| Native VST3 editor | Three attach/remove cycles passed | Cocoa NSView child through `IPlugView`; same editor/backend. |
| Production editor | Not tested or implemented | Probe contains a clearly labeled test gain slider, not the user's required two-strip Drive body. |
| External DAW session | Not run | Native harness and validators are not substituted for a real external DAW session. |

The library is Mach-O arm64. Inspection shows system AppKit/Foundation/OpenGL/CoreFoundation/runtime dependencies, with no DAW dynamic library dependency. The prototype's local Mach-O install name and development bundle metadata are not a portable signed release packaging claim.

### Direct DSP baseline and predeclared budgets

The benchmark measures the existing Drive processor directly, with precomputed stereo input, 1,000 warm-up blocks, and 10,000 measured blocks. Color uses Mid hump + bass cut; other controls use the existing defaults. No signal generation, allocation, editor work, or host queues are inside the timed block. This is a local baseline, not a universal CPU-cost claim and not a measurement of a Drive plugin wrapper.

| Frames / instances at 48 kHz | Direct median | Direct p99 | Future plugin median ceiling |
| --- | --- | --- | --- |
| 32 / 1 | 0.708 µs | 0.875 µs | 1.8496 µs |
| 32 / 8 | 9.708 µs | 12.083 µs | 19.6496 µs |
| 512 / 1 | 12.458 µs | 15.458 µs | 15.9496 µs |
| 512 / 8 | 164.375 µs | 188.875 µs | 205.25 µs |

The checked-in budget is fixed before candidate performance comparison: median no greater than `1.20 × direct median + 1 µs × instance count` per block; p99 below 50% of the callback period on the same machine/configuration, with the direct baseline also fitting that callback budget. The periods at 48 kHz are approximately 666.67 µs for 32 frames and 10,666.67 µs for 512 frames. Zero adapter latency frames and zero callback allocation/final-owner deallocation remain hard gates.

No candidate Drive-export performance percentage is claimed: the current exported candidate is a different, simpler gain processor. Re-run the direct and real Drive-export measurements on the same build/compiler/hardware with equivalent reconstructed parameter streams in Phases 2/3. Investigate a failed budget; do not silently relax it. Record additional static/automation and target-platform measurements before release.

### Tail policy

Adopt a conservative fixed one-second tail for the first Drive plugin implementation, zero reported processing latency, and no extra block of adapter buffering. Report the tail through each SDK and keep processing residuals when the host requests it. Do not advertise immediate sleep solely because a block has silent input. Parameter changes wake/continue processing according to the host contract. Hard bypass/suspension remains the host's decision.

The survey processes an impulse `[4, -2]` through 108 settings combinations at each of the six rates: all four Color choices, minimum/default/maximum Color frequency, minimum/neutral/maximum Tone, and zero/default/maximum Gain, with maximum Color strength and +12 dB output. It follows each impulse with two seconds of silence and checks finite output and the final frame exceeding −120 dBFS absolute per-channel amplitude.

| Rate | Last observed sample above threshold |
| --- | --- |
| 44.1 kHz | 30.227 ms |
| 48 kHz | 30.125 ms |
| 88.2 kHz | 29.603 ms |
| 96 kHz | 29.510 ms |
| 176.4 kHz | 27.976 ms |
| 192 kHz | 27.776 ms |

The one-second policy includes a large margin over this corpus. The survey is not a proof for arbitrary preceding audio, parameter-history changes, every finite input magnitude, or the effects of a host suspending processing. Exported Drive tests must additionally cover representative sustained/high-level input histories, parameter/bypass transitions, reset, and SDK tail/sleep behavior before shipping. The built-in DAW's arranged-duration export and continuous processing rules remain unchanged.

## Production implementation map

| Operation | Narrow CLAP adapter | Narrow VST3 adapter | Shared ownership / remaining work |
| --- | --- | --- | --- |
| Discovery and identity | Entry/factory/descriptor | Factory and stable processor/controller class IDs | Manifest/identity freeze in Phase 1; current probe IDs are disposable. |
| Prepare / retire | Init, activate, deactivate, destroy on prescribed threads | Initialize, setupProcessing, setActive, terminate and reference-counted ownership | Prepare `Drive` and bounded storage outside processing; Phase 2/3 production implementation. |
| Audio | Planar `process` buffers, process status, events | `ProcessData` buffers, bus/sample negotiation and silence flags | Direct shared `Drive` sample calls inside each format's block; zero extra block latency. |
| Base automation | Walk ordered timed events without materializing an unbounded copy | Use prepared cursors for the known parameters; reconstruct normalized piecewise-linear curves, including implicit previous-block values | Convert through shared parameter mappings and retarget existing DSP ramps once; do not copy the rejected step/vector path. |
| Editor gestures | Value/gesture events and host requests | Controller begin/perform/end edit with processor synchronization | Bounded UI handoff with latest-value/gesture retry semantics; no DAW history dependency. |
| State | Prescribed main-thread stream calls and dirty notification | Component/controller state streams and coherent recall | Phase 1 validated bounded envelope and stopped/active handoff; no plugin-owned callback mutex. |
| Bypass | Standard bypass parameter metadata and value events | Bypass-flagged parameter and queues | Shared `DriveSettings.enabled` conversion; parameter 100 remains proposed until Phase 1 freeze. |
| Delay / residuals | Latency and tail extensions; correct process status | getLatencySamples / getTailSamples | Zero frames latency, one-second tail policy; exported DSP verification still required. |
| Window | GUI extension with platform parent handles | `IPlugView` and `IPlugFrame` | Planned `crates/plugin-window` around egui-baseview, with common `crates/unit-ui` controls/Drive body. Native backend feasibility demonstrated on macOS; production adapters and other OS checks remain. |
| Realtime / offline | Render/lifecycle negotiation without changing DSP | Processing mode/setup contracts without changing DSP | No hidden quality switch; device-free processing. |

Keep `crates/plugin-clap`, `crates/plugin-vst3`, and `crates/plugin-window` above portable DSP and below the independent export targets in dependency direction. Existing built-in construction stays in the engine. `unit-ui` must not depend on the DAW engine/project/output. The external format adapters must implement their own host contracts; they must not become a second modulation resolver or duplicate the Drive algorithm.

## Platforms, hosts, and remaining gates

| Target | Phase 0 evidence | Remaining requirement |
| --- | --- | --- |
| macOS arm64 | Pinned compiler, both plugin ABIs, both validators, native editor-child lifecycle, direct DSP survey/baseline | Actual Drive adapters/body, editor interaction/scaling while processing, complete release notices/bundles, and external DAW sessions. |
| macOS x86_64 | Not built or tested | Native compile/SDK/editor checks before advertising this architecture. |
| Windows x64 | SDK/backend source support inspected; no native execution | Native MSVC/Windows SDK, validators, HWND embedding/scaling and real-host checks. |
| Linux x86_64 | X11 backend/dependencies inspected; no native execution | Native C/C++/X11 dependencies, validators, embedding/scaling and real-host checks. The selected backend does not establish native Wayland support; do not advertise it. XWayland requires a host-provided X11 parent and its own checks. |

Name REAPER as the first planned external host for both formats on each advertised OS. Record its actual version when the sessions are run. No supported external CLAP/VST3 DAW was available in this local environment; the installed Logic application is not a substitute for these format checks. Do not infer Windows/Linux or real-DAW compatibility from the local native harness. These gaps block the corresponding release claims, not continued Phase 1 parameter/state design.

Phase 0 establishes the local dependency decision, reproducible prototypes, tail policy, quantitative baseline/budgets, and a concrete lifecycle/editor map. Cross-platform feasibility stays open until native runners execute the relevant checks. Do not mark the first Drive plugin delivery complete from this record.

## Primary sources and provenance

- [nice-plug repository](https://codeberg.org/RustAudio/nice-plug), published Cargo packages and their `.cargo_vcs_info.json` revisions above; the [isolated lockfile](../../tools/drive_plugin_phase0/Cargo.lock) records registry checksums. The inspected VST3 wrapper queues point events and grows `process_events` under `permit_alloc`; the independent host checks both behaviors directly.
- [CLAP protocol](https://github.com/free-audio/clap/tree/a47f6badb49d948fd009998f28309cdab78979c9), including lifecycle, GUI, state, parameters, and tail interfaces; [validator 0.4.1](https://github.com/free-audio/clap-validator/tree/152b9823e992d782c5c1fd33bca0295478b919aa).
- [VST3 SDK checkout](https://github.com/steinbergmedia/vst3sdk/tree/3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96) and its committed submodules. `pluginterfaces/vst/ivstparameterchanges.h` defines linear queue sections and implicit previous-block points; [Steinberg parameter/automation documentation](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Parameters%2BAutomation/Index.html) describes the surrounding host/controller contract.
- [clap-sys](https://github.com/micahrj/clap-sys) and [vst3-rs](https://github.com/coupler-rs/vst3-rs) are the selected bindings; [egui-baseview](https://codeberg.org/RustAudio/egui-baseview) is the selected window-backend baseline. Selection does not waive future pin/license/target verification when production dependencies are added.
