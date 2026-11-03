# Noise source

For user controls, see [Noise](../../docs/instruments/noise.md).

## Model and ownership

`unit-core::NoiseSettings` is an optional regular-layer source with a stable UUID,
version 1, enabled/collapsed flags, Type, Color, Width, Level and shared modulation
routes/presentation. Defaults are White, Color 0, Width 0, Level −12 dB, enabled and
expanded. Color is −1..1, Width 0..1 and Level −∞..+12 dB through the standard
silent-gain sentinel. Reject nonfinite/out-of-range settings, invalid versions,
nil/duplicate source identities, invalid routes and source conflicts atomically.
Older manifests without Noise deserialize to no Noise source. Save the existing
instrument version; this is an optional source field, not a reinterpretation.

A layer has Oscillator, Sampler/Granulizer or Noise, never more than one. Catalog
registration uses `daw.noise` in Sources, with no Main/provider-lane constructor.
Adding Noise clears the existing source and its routes. Source copies get fresh
source/route identities; moves retain identity. Shared layer-copy/reference-remap
and transactional component-transfer APIs apply. Keep Noise pinned first and
reject transfers to occupied source layers or Output/modulation lanes. Audio-file
drops still replace Noise with a prepared Sampler through the shared worker path.

## Portable generation

`crates/noise` owns the allocation-free stereo generator. Its only dependency is
`unit-core`; no project, media, UI or device dependency is permitted. The host owns
note scheduling. Seed two xorshift64* streams from stable source and note identities.
Do not use a global random generator, system time, random allocation or MIDI pitch.
Repeat render/export/seek with the same identities reproduces the signal; distinct
notes and copied sources use independent streams.

White is uniform bipolar noise. Pink uses eleven parallel, octave-spaced one-pole
low-pass sections with inverse-square-root frequency weights. Their stationary
covariance determines the reference RMS normalization. Brown uses the 20 Hz leaky
integrator, normalized by its stationary variance; it approximates a −6 dB/octave
slope above that corner without an unbounded random walk. All three endpoint types
match White's reference RMS at each project rate; this is not perceptual matching.
This implementation uses independently written filter equations and statistical
normalization. It imports no third-party noise implementation.

Color splits at 1 kHz and applies complementary low/high gains of ±6 dB at each
endpoint. Width keeps the left stream and blends correlated left with independent
right through a cosine/sine pair, preserving reference channel power. Zero Width
is exact mono; full Width uses independent stereo. Clamp final effective controls
in portable DSP. Level modulation cannot raise an exactly silent base gain.

Use five-millisecond base-control/power ramps and Type crossfades. Retain random,
filter and route/smoother state on compatible live updates. A new note resets its
stream from note identity and takes the current settings. Continue source state
while its voice runs; the shared Amplitude stage owns note gate/release. Noise
introduces no new release owner or one-shot mode. Keep state fixed-size and include
route/storage sizes in the existing renderer memory accounting.

## Shared host integration

Noise implements `StereoSourceUnit` and `ModulationDestination`, while settings
implement `ModulationDestinationSettings`. Color, Width and Level are per-note
additive destinations through the shared resolver, source-index cache, route
validation, one-millisecond offset smoother and typed parameter entry point. Amounts
are ±100% for Color/Width and ±24 dB for Level. Type/power remain discrete. Reuse
shared source selectors, connection strips, assignment/range indicators and amount
knob mappings. Publish processed voice extrema through `ModulationTarget::Noise`:
Color min/max, Width min/max, Level dB min/max. Telemetry is display-only.

Tap the stereo voice sum after Noise controls and velocity, before Amplitude,
clip/layer/track processing. Use `SourceOutputTarget` with Noise UUID, existing
prepared `LevelMonitor<3>`/`UnitMeter` handles, source-output binding/retirement and
the shared level/held-peak painter. Disabled units mask and clear readings and stop
publication. Compatible edits retain monitor history; snapshots detach handles.
No callback allocation, locks, file I/O or final-owner retirement is permitted.
MIDI and normalized beat-subtrack rendering share this path with offline export.

## Two-strip presentation

Reuse the common unit shell, header power/collapse, selection, context menu,
component drag, knob rows, modulation and footer helpers. Each strip is 90 points,
with existing unit padding, one full-height divider and no added gap. Keep shared
expanded/compact heights. Noise has Type dropdown, Color and spectral preview in
its three rows, with Noise footer. Out has Level, Width and one empty row, with
Out footer and the standard top-margin meter. Use Small popup text and canonical
`unit-ui::menus::Dropdown` layout. Reset Color/Width to zero and Level to −12 dB.

The bounded analytic spectral drawing shows approximate Type/base-Color balance
on a logarithmic 20 Hz–20 kHz axis, anchored at 1 kHz on a fixed relative scale.
It is not live analysis and excludes gain/modulation/effects. Do not clock audio
DSP from UI painting. Grey the preview while off. Compact layers/unit collapse hide
controls/preview and retain the common footer/header behavior. Meter/knob/selector
and preview hits must not start unit drags. Removal retains other layer components.

## Acceptance

Cover settings migration/roundtrip, conflict validation, copies/moves and route
references; deterministic independent streams, stereo correlation, spectral order,
reference RMS and finite output at all six rates; modulation, exact silence, power
and compatible updates; live/export/seek equality, beat-subtrack rendering, passive
meter placement and zero callback allocations; save/reopen and float WAV export;
and layout, controls, power, compact/collapsed states across themes. Record executed
checks and native/device/platform limits in [validation](../validation/README.md).
