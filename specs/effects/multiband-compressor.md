# Multiband Compressor

For controls and use, see the [user guide](../../docs/effects/multiband-compressor.md).

## Layout and settings

Register `daw.multiband-compressor`, state version 1, under Dynamics. Reuse the
track, group, layer, Main and Master effect chains, catalog, unit shell, history,
worker preparation and shared modulation contract. Defaults are three enabled
bands, split at 200 Hz and 2 kHz, with unit power on, Level 0 dB and Dry/Wet 100%.

The strip order is Split, variable band strips, Plus, Out, then open modulation
connections. Split uses shared frequency knobs for adjacent boundaries, with
vertical scrolling when needed. Boundaries span 20 Hz–20 kHz with a minimum 1 Hz
gap. The outer bands reach DC and Nyquist. The last saved upper bound is the
20 kHz sentinel; it does not cut the top range at 20 kHz.

Each band uses the shared 161-point two-column geometry and four compact control
rows: Threshold/Ratio, Attack/Release, Knee/Makeup, Peak–RMS/gain reduction. The
bottom footer contains band power, its label and the shared S button. A standard
level rail sits above the controls. Out contains Level and Dry/Wet with the shared
output meter. Compact mode retains power, labels and S. Both built-in and plugin
editors call the same `unit-ui` body.

Compression ranges and defaults match [Compressor](compressor.md). Makeup uses the
shared silence…+12 dB gain control and defaults to 0 dB. No automatic makeup,
lookahead or external sidechain is added. Band power bypasses compression and
Makeup while retaining the range's audio. Split frequencies and Peak/RMS are
manual controls. The six band compression controls and global Level/DryWet use
shared targeted destinations, route validation, smoothing and processed snapshots.

Band UUIDs own controls, routes and monitoring. Plus splits the widest logarithmic
range and retains its settings on both halves; the new half receives a fresh ID.
Remove merges into an adjacent range, removes only the removed band's routes,
and retains at least one band. Built-in bands have no fixed count limit.
Release-only strip dragging stores a separate presentation permutation. It does
not move frequency ranges or change audio, identities, routes or detector history.

## Processing

`daw-multiband-compressor` owns portable stereo processing. It composes the shared
24 dB/oct Linkwitz–Riley split with independent instances of `daw-compressor`.
Lower branches receive the later crossover all-pass responses before compression,
so adjacent ranges recombine with flat magnitude at unity settings. Each range
uses linked-stereo feed-forward downward compression with the existing detector,
knee, attack and release behavior. No new DSP backend or external dependency is
introduced.

Dry/Wet blends the phase-matched uncompressed range sum with the compressed sum.
Level follows the blend. Whole-unit bypass returns the original unscaled input.
Reported alignment latency is zero; crossover phase is not a linear-phase delay.
Plugins declare the shared conservative four-second tail bound for crossover
histories. DAW rendering retains the existing arranged-duration policy for filters.
Control and power ramps last 5 ms at all six project rates; crossover frequencies
interpolate in log space. Compatible updates retain histories by band UUID and
crossover topology. Construction and topology preparation run on workers. Sample
processing, reset and compatible retargeting do not allocate, lock, perform I/O or
retire final owners. Storage and CPU increase with the band count; phase
compensation uses quadratic crossover storage and work.

## Temporary Solo

S auditions the processed frequency range. Multiple S buttons combine their
adjacent ranges. With band power off, S auditions the uncompressed range.
Audition excludes unselected ranges and the full-range dry signal, even with
Dry/Wet at zero. Solo masks and the audition mix use 5 ms transitions. Detectors
and the full crossover bank continue running.

Reuse the live output's prepared `(effect UUID, band UUID)` atomic controls across
all buses. Solo changes neither project settings, dirty state, history nor routes.
Project replacement, band removal and unit power off clear the relevant flags.
Offline DAW processors never bind these controls. Clones detach them. Plugin Solo
is outside parameters and serialized state. VST3 sends bounded control-thread
messages; CLAP shares prepared atomics. VST3 offline mode and CLAP's render
extension mask audition immediately. Hosts must declare offline mode to receive
that behavior; real-time recording captures the monitored output.

## Standalone products

Independent CLAP and VST3 products reuse the shared format adapters and editor
body. The plugin prepares 64 stable automation slots outside activation. Each
slot exposes Present, Enabled, Frequency, Threshold, Ratio, Attack, Release, Knee,
Makeup and Detection; Bypass, Level and DryWet are global, for 643 parameters.
Default presence is slots 1–3. Removing/reusing a slot retains its published ID;
no parameter IDs are generated during audio processing. Inactive slot settings
remain in presets. Frequency order determines processing ranges; a separate
64-slot permutation controls visual order. Crossing host frequency targets sorts
active slots, then constrains effective boundaries to ordered 1 Hz gaps. A host
can deactivate every slot; the empty bank passes input through Out and Plus can
create a new range.

State is bounded, versioned and validated before publication. Per-slot updates
retarget only that compressor; frequency/presence changes rebuild a bounded index
bank without allocation. Public identities and parameter IDs are frozen in the
checked product manifest. The manifest-discovered packager and the native plugin
CI matrix include this product. Native external-host and device acceptance remain
separate from headless tests.

## Band context menu and held level peaks

The shared band body attaches the canonical context menu to its strip, controls,
power, Solo and level rail. Actions are Enable band/Disable band, checked Solo,
and Remove band. Retain the one-band removal guard and disabled/focus/reorder
editing guards. Power and removal follow the existing settings transaction or
plugin parameter bindings; Solo remains live-only.

Each band level rail reuses the standard held sample-peak marker, tooltip,
accessibility and independent click reset. Capture every processed sample in the
prepared band `UnitMeter<BandMeter<3>>`, including while hidden. Keep disabled-band
fills secondary grey; whole-unit off clears and masks current and held values.
Preserve the separate gain-reduction hold and final Out peak when resetting a
band level peak. Meter clicks must not start strip dragging or edit the project.

CLAP uses shared peak handles. VST3 appends 64 held level values to its existing
205-value telemetry for 269 values total. Bounded per-band reset messages select
reduction or level holds independently. These monitoring fields do not change
saved state, parameter IDs, audio processing or latency.
