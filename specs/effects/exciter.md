# Multiband Exciter

Implemented design, 2026-10-30. User controls are in the [guide](../../docs/effects/exciter.md).

## Ownership and scope

`daw.exciter`, state version 1, is a built-in effect under Distortion. Its registration in `effects_catalog::ENTRIES` supplies additions and group drops. Use the existing track, group, Master, regular layer and Output effect chains, transactions, selection, compact/collapse, copy/transfer, history and missing-effect fallback.

- `unit-core::ExciterSettings`, `ExciterBand` and `ExciterMode` own validated saved data.
- `crates/exciter` owns portable stereo processing and shared typed modulation. It depends only on portable unit, filter and saturation libraries.
- `unit-ui::exciter` owns the headerless body. `ui::exciter_panel` adds the DAW shell, target/source adapters and transient Solo state.
- Engine effects own construction, snapshots, fixed latency, prepared inheritance and meter/control binding. Output owns live audition handles.

No external dependency or version upgrade is required. Independent CLAP/VST3 identities, fixed-slot parameter banks and products are outside this implementation. Future editors must reuse the shared body.

## Settings and topology

Defaults are four enabled Soft bands, crossovers at 150 Hz, 2 kHz and 6 kHz, Drive 0 dB, Mix 0%, band and output Level 0 dB, global Dry/Wet 100%, enabled/expanded, and closed modulation.

Each band has a non-nil stable UUID, power, Mode, upper boundary, Drive, Mix and Level. Bands are contiguous in frequency order. All but the last band's upper bound are shared crossovers. The final stored upper value is the 20 kHz sentinel; actual processing extends its high branch to Nyquist. Crossovers span 20–20,000 Hz with at least 1 Hz separation. Drive spans −36…+36 dB, Mix and global Dry/Wet span 0…1, and Level uses the existing exact-silence…+12 dB scale. Reject nonfinite values, empty lists, duplicate identities, unordered boundaries and invalid routes transactionally.

There is no fixed band-count cap. Plus splits the widest logarithmic interval that has room for a boundary. Preserve the original identity and routes; the new right band gets a new UUID and inherits its control values. Removing a band extends the lower neighbor through its former range, or extends the next band downwards when removing the first. Keep one band. Remove only the removed band's routes. Effect duplication assigns fresh effect, band and route identities and remaps targeted routes. Presentation and transient Solo do not change DSP settings.

## Layout

Use the common lane shell, 90-point strips, margins, full-height dividers, shared knob base, canonical Small dropdown menus and standard output meter/held peak.

1. Ranges: vertically ordered crossover knobs, with scrolling for larger banks and a Ranges footer. Clamp UI edits between neighbors before publishing settings. Frequency is logarithmic; the selector has no modulation.
2. Each band: Mode dropdown, then 42-point Drive, Mix and Level knobs. The footer contains the shared Power/label pair (`B1`, etc.) beside the shared 22-point `S` toggle. Footer tooltips identify the full range. Retain compact Power/Solo controls. Right-click a knob or Power for the standard Remove band menu; disable removal of the final band. Do not attach a context popup with the ComboBox's popup ID to Mode.
3. Plus: shared add strip.
4. Out: shared Level and Dry/Wet knobs, footer and the standard linked-stereo rail with held peak/reset. Retain the meter's bypass masking and lifetime contract.

Body rendering and control order stay in unit-ui. The host supplies modulation markers, effective snapshots, live Solo selections and output readings. Retain standard numeric drag, Shift sensitivity, keyboard input, reset and accessibility.

## DSP and alignment

Reuse the original OTT fourth-order Linkwitz–Riley splitter, extracted without arithmetic changes into `daw-filter::crossover::LinkwitzRileySplit`. Its FunDSP nodes remain private. Sequentially split the remaining high signal. Apply each later split's all-pass sum to earlier low branches; this aligns the phase of all bands for arbitrary band counts. Recombined neutral bands have a flat magnitude response, not a sample-identical response to the original.

Crossover values use logarithmic 5 ms interpolation with one clock for the entire ordered vector. Restart that common clock on any changed boundary; independently restarting boundary ramps can cross neighboring frequencies during rapid edits. Preserve current values and filter history on compatible updates. Crossover topology is prepared on workers.

Reuse Saturator's 129-tap 4× polyphase interpolation/decimation and static Analog Clip/Hard Curve functions through their portable APIs. Each band oversamples the nonlinear residual `shape(driven) / drive - original`. This retains unity small-signal gain instead of carrying the input drive gain to the output. Keep the band's linear dry path delayed by the same 32 project frames. Mode weights crossfade over 5 ms and are normalized during rapid retargeting. Evaluate active/crossfading curves; keep oversampling histories and band clocks running while bypassed.

| Mode | Transfer/character |
| --- | --- |
| Soft | Existing Saturator Analog Clip rounded knee, divided by linear drive. |
| Tube | `(tanh(x + 0.35) - tanh(0.35)) / (1 - tanh(0.35)^2)`, divided by drive; `x` is the driven sample. |
| Tape | `tanh(x)` divided by drive; smooth its nonlinear residual with an 8 kHz one-pole at the oversampled rate. |
| Hard | Existing Saturator Hard Curve at ±1.2, divided by drive. |

Remove DC from the blended nonlinear residual with a 10 Hz high-pass at the oversampled rate. Mode names describe the implemented character; no particular analogue circuit or tape machine is modeled. Added harmonics are not filtered back into the original band's upper range.

For each band, form `dry + power × ((level − 1) × dry + level × mix × residual)`, where gains are linear and all paths are aligned. Sum the bands. Global Dry/Wet interpolates between their phase-matched dry sum and the processed sum; Out Level follows that interpolation. Header bypass interpolates to the original signal delayed by 32 frames. Report 32 frames at every rate, in all power/mix states. Use the existing downstream-clock, layer/track/Main/Master alignment, priming and live/export paths.

Base values, power, Mode and Solo audibility use shared 5 ms ramps; route offsets use the shared 1 ms smoother. Sanitize nonfinite input to zero. Bound crossover-processing input at ±1e12 to keep state finite on extreme host samples; preserve full finite f32 headroom on the original bypass path. Accumulate residuals and conversion in f64 and clamp finite final conversion. Exact bypass endpoints avoid cancellation and preserve original finite samples.

## Modulation and publication

Reuse `P::SATURATOR` parameter identities and native adapters for targeted band Drive, Level and Mix; reuse Level/DryWet at unit scope for Out. `valid_mixed_modulations`, `RoutedDestination`, typed shared inputs and the common route resolver own validation, source binding, sums, scope, smoothing and route inheritance. Consumers do not look up providers or implement routing loops. Mode, crossover and power are not modulation destinations.

Effect-band telemetry is keyed by `(effect UUID, band UUID)` and reports Drive dB, Level dB and Mix in that order. Unit telemetry reports Out Level dB and Dry/Wet. Share source discovery, layer groups, amount mapping, markers and connection strips. Unassigned or disabled routes stay neutral. Copies and provider transfers use the existing core remapping paths.

Prepare vectors, filters, compensation banks, route storage, meters and audition flags outside the callback. Processing, reset and compatible inheritance allocate, free and block nothing. Removed owners remain in the retired processor. Memory accounting includes dynamic band and route storage and the triangular crossover-compensation bank. Live playback and export use the same processor.

## Live-only Solo

Solo is not part of ExciterSettings, the project, Undo/Redo or export state. The UI retains `(effect, band)` selections for the current project and prunes removed identities. Output prepares one atomic flag per live band, retains matching flags across plan updates and binds them through the engine's existing track/layer/Main/Master traversal.

If any flag is set, fade unsoloed bands out over 5 ms. Apply the mask to both dry and processed contributions, including when global Dry/Wet is zero. Several Solo bands sum normally. An off band still auditions its original range. Whole-unit bypass ignores audition while retaining the flags.

DSP snapshots detach Solo flags and restore neutral band audibility. Worker checkpoints and ordinary offline renderers have no audition bindings; export therefore cannot inherit UI Solo. Clear UI state on project identity changes. Solo edits do not publish a new project or renderer graph. Old handles stay owned until processor retirement. Audio reads only atomics and ramps.

## Validation

Cover state/version round trips, invalid atomic edits, band/route identity after add/remove/copy/transfer, save/reopen/Save As and byte-identical float export. At all six project rates verify flat recombination, independent channels, latency/bypass, distinct modes, even harmonics/DC removal, finite extremes and folded-harmonic suppression. Check band Level/global Mix/Out Level order, rapid ordered crossover edits, source scope, processed snapshots, compatible inheritance, route retargeting and zero callback allocation/retirement.

Exercise live/offline, block and seek parity; all effect placements; Solo dry isolation, multiple Solo and snapshot/export exclusion; output meter binding and bypass lifecycle. UI tests cover menus, band removal, transient Solo, dirty state, compact layout, knob order/geometry, shared footers and meter placement in Dark/Light/Mist at 1×/1.5×. Retain OTT/Saturator and all existing editor parity tests after shared code extraction. Record executed checks and remaining native listening/device/platform/performance limits in [validation history](../validation/README.md).
