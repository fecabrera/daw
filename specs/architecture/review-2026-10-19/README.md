# Architecture review — 2026-10-19

Status: **ready for owner review**. Recommendations remain proposals unless a dated follow-up records approval and implementation.

Reviewed baseline: `2dbb347` (`perf: reduce cold seek preparation cost`). The working tree was clean at the start. The initial review added documentation only. It covers all 27 workspace packages and the current application pipelines at the subsystem and workflow level. It is not a line-by-line proof of every DSP algorithm, a security audit, or a new native performance benchmark.

## Assessment

The application has useful boundaries: portable units, a transactional project model, worker-side media preparation, a shared live/export engine, and a device adapter. Stable IDs and shared modulation, controls, selection, and drag helpers support reuse across track, layer, and Main units. Preserve these boundaries.

The main weaknesses are in lifecycle coordination and growth: old pattern assets accumulate, large snapshots and caches lack a common memory policy, audio publication depends on UI polling, and complete plans are rebuilt for a broad set of changes. A concrete latency-clock omission also shows that registering an effect does not yet cover every host integration point.

## Read in this order

| Document | Contents |
| --- | --- |
| [Architecture map](architecture.md) | Crates, dependencies, state ownership, threads, clocks, extension boundaries |
| [Audio and modulation](audio.md) | Preparation, note/voice processing, every unit family, bus routing, latency, device output, seek and activation |
| [Preparation and caches](preparation-and-caches.md) | Decode, stretching, Sampler, cache keys, retention, invalidation, resource admission |
| [UI and editing](ui-and-editing.md) | Frame stages, focus, interaction, previews, transactions, history, painting and telemetry |
| [Workflow catalog](workflows.md) | User actions traced through stages, publication, cancellation and errors |
| [Findings and options](findings.md) | Evidence, consequences, suggestions, pros, cons, impact and acceptance checks |
| [Coverage and validation](validation.md) | What was inspected, existing evidence, gaps, proposed measurement matrix |
| [Package inventory](inventory.md) | Manifest-derived direct dependencies for all workspace packages |

## First decisions

| Order | Proposed action | Reason | Finding |
| --- | --- | --- | --- |
| 1 | Completed: correct effect-clock latency through one common latency query | Saturator delays audio by 32 frames, but the next effect's clock is not adjusted | [F01](findings.md#f01-effect-clock-latency-is-incomplete) |
| 2 | Add reachability-based asset cleanup | Every changed MIDI pattern appends an asset; obsolete versions remain in the current project | [F02](findings.md#f02-unused-assets-accumulate) |
| 3 | Measure and remove UI dependence from output publication/retirement | A stalled frame can delay a ready seek or edit and retain old renderers | [F03](findings.md#f03-audio-handoff-depends-on-ui-progress) |
| 4 | Implemented: byte limits and categorized memory estimates; native peak-memory validation remains | PCM admission excludes DSP snapshots, metadata, native allocations and GPU storage | [F06](findings.md#f06-memory-limits-do-not-cover-total-retention) |
| 5 | Accepted: exact seek history with idle coverage and strict reuse; other transport/export-tail decisions remain open | Exact history, quick seeking, LFO phase continuity and final effect tails have different costs | [F05](findings.md#f05-cold-history-replay-can-still-take-a-long-time), [F15](findings.md#f15-transport-history-semantics-are-not-uniform), [F16](findings.md#f16-export-stops-at-the-arrangement-end) |
| 6 | Strengthen file publication before treating the format as durable storage | Windows replacement and no-overwrite export have gaps under crashes or concurrent writers | [F07](findings.md#f07-save-publication-is-not-fully-crash-durable), [F08](findings.md#f08-export-no-overwrite-has-a-check-then-rename-race) |

The user approved F01, F05 and F06 after this review. The follow-ups preserve exact seek history, add background coverage and byte limits, and permit memory pressure to shorten undo history with a visible notice. Their implementation and remaining measurement limits are linked from the findings. Other changes remain proposals. Review priorities and the product decisions first; each recommendation includes a check that would establish completion.

## Evidence labels

- **Code:** behavior observed in the reviewed implementation. Source links identify the relevant module; symbol names identify the exact stage.
- **Historical measurement:** a previously recorded result, with its original workload and limits. It was not rerun for this review.
- **Risk:** a consequence inferred from code, not a reproduced user failure.
- **Decision:** an observable behavior or tradeoff that needs an explicit product choice.

P1 means address before scaling or relying on the affected guarantee. P2 means planned improvement. P3 means a lower-priority capability or assurance gap. Priorities do not imply that every project has the problem. Effort estimates are relative: S = focused change; M = several modules and regression coverage; L = architecture work across ownership boundaries.

## Coverage overview

| Pipeline | Entry → result | Detailed map |
| --- | --- | --- |
| Desktop startup | arguments/preferences → native adapters → egui → staged project open | [Architecture](architecture.md), [W01](workflows.md#w01-start-open-new-and-close) |
| CLI | clap → session load/validation → optional shared offline export → exit status | [W13](workflows.md#w13-headless-cli-and-stretch-tools) |
| UI | native events → focus/guards → controls/preview → transaction → repaint | [UI stages](ui-and-editing.md#frame-stages) |
| Editing | staged model → full validation → optional media preparation → session/history → plan | [Commit path](ui-and-editing.md#commit-and-history-path) |
| Playback | plan → schedules/DSP/delays → worker preparation → queue → callback → device | [Audio stages](audio.md) |
| Modulation | provider graph → ordered publication → indexed routes → smoothing → parameter bounds | [Modulation](audio.md#modulation-pipeline) |
| Seek/live updates | intent/revision → inheritance or replay/cache → exact activation → retirement | [Transport](audio.md#transport-and-plan-activation) |
| Import/preview | external path → decode/rate conversion/peaks → placement → atomic session | [W02](workflows.md#w02-import-or-drop-a-wav) |
| Stretch/Sampler | range/ratio/pitches → resource admission → transform → peaks/bank → publication | [Preparation](preparation-and-caches.md), [W04](workflows.md#w04-stretch-one-or-more-clips), [W08](workflows.md#w08-assign-and-edit-a-sampler) |
| Save/history | snapshot → paths/JSON → temporary file → publication; independent undo snapshots | [W10](workflows.md#w10-save-save-as-and-unsaved-changes), [W11](workflows.md#w11-undo-and-redo) |
| Export | validated snapshot → shared renderer → rate/codec → finalize → publish | [W12](workflows.md#w12-export-a-mix) |
| Diagnostics/delivery | atomic counters/tests → reports → platform build/package/licenses | [Validation](validation.md) |

Beat tracks, general audio graph editing, live recording, automation lanes and external plugin hosting are not current pipelines. Beat design is explicitly pending. The current graph machinery is the modulation dependency graph plus fixed audio buses and serial effect chains.
