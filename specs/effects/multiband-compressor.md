# Multiband Compressor

For controls and use, see the [user guide](../../docs/effects/multiband-compressor.md).

## Layout and settings

Register `daw.multiband-compressor`, state version 1, under Dynamics. Reuse the
track, group, layer, Main and Master effect chains, catalog, unit shell, history,
worker preparation and shared modulation contract. Defaults are three enabled
bands, split at 200 Hz and 2 kHz, with unit power on, Level 0 dB and Dry/Wet 100%.

The expanded strip order is Response, Split, variable band strips, Plus, Out, then open modulation
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
prepared band `LevelMonitor<3>`, which composes `UnitMeter<BandMeter<3>>`, including
while hidden. Publish linked/L/R level with instant attack and the shared 300 ms
release. Capture held peaks from raw samples, never from decaying display values.
Keep gain-reduction publication separate and unchanged. Compatible inheritance
retains level release history. Worker binding uses `attach_peak_prepared` and
must not zero or overwrite the active producer's band readings before activation.
Reset clears current release and unit power off
clears current/held levels. Do not add UI-side smoothing. Keep disabled-band
fills secondary grey; whole-unit off clears and masks current and held values.
Preserve the separate gain-reduction hold and final Out peak when resetting a
band level peak. Meter clicks must not start strip dragging or edit the project.

CLAP uses shared peak handles. VST3 appends 64 held level values to its existing
205-value telemetry for 269 values total. Bounded per-band reset messages select
reduction or level holds independently. These monitoring fields do not change
saved state, parameter IDs, audio processing or latency.

## Live output spectrum and crossover markers

Reuse `daw-spectrum::SpectrumAnalyzer` and its coherent telemetry suffix with
Multiband EQ. The portable unit samples the final Out signal, including Dry/Wet,
Level and live Solo, without entering PCM. Reuse `UnitMeter<SpectrumMeter>` for
prepared attachment, activation, power clearing, reset, compatible history/handle
retargeting and detached clones. Count prepared heap storage. Analysis runs only
with an enabled bound meter. Preparation and retirement remain off the callback;
processing, reset and inheritance cannot allocate, lock, perform I/O or retire
final owners. No new parameter, unit-state or project schema is introduced.

The shared `unit-ui::spectrum_graph` paints the same 270-point section, 172-point
chart, frequency grid, secondary-color filled contour and Response footer as EQ.
The Response footer uses the same 18-point footer row and bottom spacing as
Split, band and Out strips, independently of chart height.
The compressor amplitude axis is −90…0 dBFS. Use the common 512-cell analyzer,
35 ms display following, 300 ms peak release and 16 ms repaint behavior defined
by the [EQ spectrum contract](multiband-eq.md#response-graph-and-live-spectrum).
Draw draggable vertical crossover lines at each ordered upper boundary except
the final 20 kHz sentinel. Use the same 30% secondary gray as unselected
threshold segments. Only the selected enabled crossover uses the 65% accent
color. Keep one graph selection, identified by stable band UUID and control kind
(crossover or threshold), in UI state. Retain it after release and discard it if
that control is removed. Resolve all interactions before painting to avoid
stale highlights when changing selections. Stable band UUIDs own gestures. Horizontal movement
changes logarithmic frequency, clamped to 20 Hz and the adjacent boundaries with
a minimum 1 Hz gap. Shift reduces sensitivity tenfold; modifier-only and press
frames cannot move a line. Divide overlapping hit areas at the midpoint between
lines so a neighboring crossover cannot steal a press. Split shows the draft
value while dragging. Publish
one DAW edit on release and retain original project/history data while held.
Reuse the EQ graph's coalesced worker preview state for audible temporary plans,
including target validation and restoration after cancellation. Escape, focus
loss, disabled/compact UI, skipped frames or invalidated target/order cancels.
External edits retain their current values. Plugin bodies publish bounded live
Frequency/Threshold gestures with balanced Begin/Value/End, including cancellation rollback
and queue retries; map marker responses to the same fixed slots as Split knobs.
Draw a horizontal threshold segment across each band's displayed frequency
range. Vertical movement maps linearly to dBFS and clamps to −60…0. Use the same
gesture lifecycle, Shift sensitivity, worker previews and release-only DAW edit
as crossovers. The Threshold knob shows the held draft. Map plugin responses to
the existing band Threshold parameter. Keep crossover hits available where lines
intersect. Segment order follows frequency order and preserves stable band IDs;
unselected and muted band segments use the EQ band-line secondary color at
30% intensity. Each segment has a centered, unnumbered EQ-style circular handle
with the shared panel fill, 5.5-point radius and 1.25-point outline. Selection
belongs to UI state by stable band ID and persists after release without editing
settings. Selected handles use a 6.5-point outline radius and the accent color;
the selected enabled segment retains its 65% accent intensity. Reuse EQ's handle
painter and keep crossover hit areas available beside the handle. Threshold lines show configured
settings, independently of the processed-output spectrum.
No new automation IDs or saved fields are introduced. Unit off mutes markers and clears spectrum. Compact/collapsed views
clear graph state and reserve no graph section. Preserve band section identities,
control bindings, drag helpers and presentation order. Marker order follows
frequency order, independently of strip presentation. The shared standalone body
adds 270 points to its viewport width (990 total), retaining its existing height.

DAW discovery/binding includes compressor effects at every existing scope through
the same prepared handle map as EQ. Reuse output transport masking and consumer
caches. CLAP shares the prepared spectrum handle and clears it at processing stop.
VST3 negotiates the existing schema-2 IDs with exact count 796. Keep scalar/band
indices 0–268 unchanged; rate is 269, bins 270–781, schema 782, readiness 783,
generation 784–787, upper clock 788–791 and legacy bass clock 792–795. Use shared
`daw-spectrum::telemetry` validation before any receiver mutation. Retain complete
269-value v1 fallback scalar/band messages and mask unavailable spectrum. Stopped
messages clear bins/readiness/clocks while retaining held peaks. Preserve other
products' v1 layouts and EQ's 668-value v2 layout.
