# Audio sidechain modulation plan

Status: implemented; native acceptance remains open. The user requested integration into the
existing modulation controls: select a track, group or Return in Source, then
configure how its audio maps to normalized modulation values from 0 to 1.
This document retains the design requirements. The implementation contract below
resolves the initial bounds, defaults, timing and ownership decisions.

See the [production workflow roadmap](../architecture/production-workflows-roadmap.md)
and the implemented [shared modulation contract](contract.md).

## Source picker and connection controls

Use this top-level order, with exactly one divider between each section:

```text
None
--------------------
<Modulation sources>
--------------------
<Sidechain sources>
```

The angle-bracketed lines describe sections, not literal menu labels or extra
submenu levels. Keep existing Output, LFO, Envelope and other registered provider
choices in the modulation section. Put Tracks, Groups and Returns submenus in
the sidechain section. Reuse the canonical Small modulation dropdown style and
shared unpadded dividers. List eligible project sources by name and stable ID;
renaming/reordering must not change a connection's source. Show unavailable
choices with a reason where practical. Reuse the same eligibility calculation
for picker availability and core route validation.

An audio source reveals sidechain settings in its existing modulation connection
strip. Ordinary provider selections retain their current presentation. Preserve
the signed Amount control, its destination-specific units and bounds, power,
selection and strip-reordering behavior. The normalized source can drive any
compatible modulation destination, including gain, filter cutoff or pitch.

Audio-to-control settings:

| Control | Purpose |
| --- | --- |
| Tap | Explicit signal location, Pre-fader or Post-fader after source effects; group/Return and mute/solo semantics are defined below. |
| Channel | Linked stereo, Left, Right, Mid or Side; normalization and linked detection are defined below. |
| Frequency range | Optional high-pass/low-pass filtering of the detector signal; does not filter the audible source. |
| Detector | Peak or RMS (root mean square); rectify/measure audio rather than using the signed waveform as a level. |
| Input range | Lower and upper levels in dBFS (decibels relative to digital full scale), mapped to 0 and 1. Enforce ordered finite limits. |
| Attack / Release | Response times in milliseconds; bounds and defaults are defined below. |
| Mapping curve | Linear amplitude or linear dB; transfer functions and processing order are defined below. |
| Invert | Reverse the mapped value within 0–1; retain independent signed Amount. |
| Readout | Detected input level and final mapped control value, from actual processing telemetry. |

The transfer function clamps to 0 below the lower input bound and 1 above the
upper bound before optional inversion. Use the finite silence floor, filter/detector state, RMS window and
envelope timing defined below; smoothing precedes mapping. Do not silently apply inconsistent detector and
modulation smoothing. Delay hold, transient extraction, multiband followers and
user-drawn transfer curves until the initial mapping is stable.

Example: choose a kick track as Source on a bass Gain connection. Map a useful
kick level range to 0–1, then use a negative Gain amount for ducking. Invert is
available for an opposite control shape; it is not required for basic ducking.
The input-range settings adjust the modulation response without changing the
source's audible level. Amount retains the destination's native units.

## Shared architecture and scope

Represent detector/mapping settings with versioned stable identities through
unit-core's shared provider/settings metadata, publication and route contract.
Keep portable detection/mapping DSP independent of project, device and UI code.
The engine supplies prepared audio taps and advances each configured provider
once per processing frame. Fan out cached values to consumers; identical
configurations may share work while distinct configurations retain independent
state. The saved schema uses one independent follower per route, as defined below.

Treat audio-derived modulation as a shared Continuous provider. It has no note
gate or authority to release a note. Apply existing per-note/shared destination
scope rules through the common resolver, including instrument-level sources
where supported. Extending cross-track scope requires an explicit shared
contract change; do not bypass current same-instrument validation in the UI.

Reuse source metadata/discovery, normalization, enable lifecycle, typed
`process_modulation`, additive offset smoothing, destination bounds and prepared
telemetry. Never create a unit-specific routing loop or cross-track lookup in
Compressor, Filter or another consumer. Audio-derived modulation is distinct
from adding an external audio detector input to a particular dynamics unit;
that processing mode needs its own specification if requested later.

## Audio graph, clocks and latency

Use the tap points defined below for regular tracks, nested groups and Returns, including
source faders/effects, mute, Solo, disabled processing and send eligibility.
Keep existing send routing semantics intact. Selection must not create an audible
send or duplicate the source into the destination's audio input.

Combine audio bus dependencies, sends and sidechain control dependencies when
validating and preparing execution order. Reject zero-delay cycles, including
indirect paths through groups, Returns and modulated provider parameters. Count
disabled and zero-amount assignments so enabling them cannot reveal a hidden
cycle. Self-taps are eligible only when the selected upstream processing stage
can be proven acyclic; otherwise reject them. Do not introduce an undocumented
one-block delay to make invalid routes appear valid.

Prepare graph order, source slots, detector buffers, route resolution and delay
alignment on workers/control threads. Align the detected source time with the
destination's processing time, including effects with latency. Define how
independent delay paths affect compensation. Keep callback processing bounded,
allocation-free and nonblocking. Retire final owners outside the callback.

Playback and export must run the same taps, detector, transfer functions and
modulation evaluator. Specify reset/history behavior for seek, loops, transport
and compatible plan replacement; do not read UI meter values as modulation.

## Persistence and failure behavior

Commit source and mapping edits through core validation and shared history.
Persist stable source IDs and settings; retain source selection across rename,
reorder, save/reopen and Save As. Copies/transfers and source deletion follow the identity and Undo rules below. Retain unresolved source identities
when opening a project with unavailable sources and expose a clear status.

Unavailable, removed or invalid providers contribute neutral modulation through
the shared presence/enable mechanism. Neutral means zero destination offset,
including when Invert would otherwise map a silent signal to 1. Genuine source
silence still follows the selected mapping. Reject invalid edits atomically;
failed preparation retains the previous graph and playback. Existing projects
without sidechain routes retain their current behavior.

## Delivery and acceptance

1. Freeze signal taps, mapping equations, defaults/bounds, scope extension,
   cycle rules, time alignment and saved ownership in the shared contracts.
2. Implement portable detector/mapping reference tests and provider publication.
3. Add core persistence, route validation and worker-prepared engine integration.
4. Extend the shared source dropdown and conditional connection controls.
5. Close automated and native acceptance gates before marking the feature shipped.

Required automated coverage includes known Peak/RMS signals, silence/nonfinite
input, channel cancellation, filter bounds, ordered input levels, curve/invert
endpoints, attack/release response at every project rate, block-size independence,
and signed destination amounts. Cover tracks/groups/Returns, indirect cycles,
disabled routes, latency-bearing effects, mute/Solo, missing sources, per-note
consumers, seek/loop/export parity, compatible history, copies/deletion/Undo,
legacy migration and callback allocation checks. Test UI section order and both dividers, picker eligibility,
conditional controls, cancellation and unchanged project state during browsing.

Native checks cover source selection and mapping in every theme, readable
telemetry, predictable ducking with a kick/bass example, and aligned playback
and export on macOS, Windows and Linux. Record limits and completed evidence in
[validation history](../validation/README.md).

## Implementation contract — 2026-11-07

Each `Modulation` can save either its existing `source` or an optional version-1
`AudioSidechain`, never both. Legacy routes omit the new field and keep their
behavior. A sidechain stores its stable source-track UUID and detector settings.
Its provider identity is `{ layer: source_track, unit: route_id }`. Each route
owns an independent follower; no settings-based deduplication occurs. Source
settings discovery uses `SidechainProviderSettings`. `Modulation::effective_source`
feeds the existing cached source-slot resolver. Shared `ModulationOutput` publishes
unipolar `Continuous` values; all destinations retain `process_modulation`, typed
scope checks, signed Amount and the common offset smoother.

Portable settings, detection and mapping live in `unit-core::audio_sidechain`.
Default tap is Pre-fader; default Channel is linked Stereo; default Detector is
Peak. Both taps follow the complete source effects. Pre-fader precedes the
source gain/pan/mute/solo gate. Post-fader applies that gate and the same current
gain/pan coefficients. Group input already includes gated child outputs; Return
input follows existing send eligibility. Source selection creates no audible send.
Master is a destination only. Self-taps are conservatively unavailable.

Filter defaults off, with 30 Hz high-pass and 20 kHz low-pass. Each cutoff accepts
20–20,000 Hz, with strict ordering. Each stage is two cascaded one-pole filters;
coefficients cap cutoffs at 0.45 times the project rate. Sanitize nonfinite audio
to zero and bound finite detector samples to ±1e12 before squaring. Stereo Peak
is `max(abs(L), abs(R))`; Stereo RMS energy is `(L² + R²)/2`. Left/Right select
one channel. Mid is `(L+R)/2`; Side is `(L-R)/2`. RMS uses a 10 ms exponential
energy average, followed by square root. Peak uses rectified level directly.

Attack and Release are one-pole envelope time constants, default 5/120 ms,
bounded 0–10,000 ms; zero is immediate. Choose Attack while the measured level
rises, Release while it falls. Smoothing precedes mapping. Default input range
is −36/−6 dBFS; ordered finite endpoints accept −120 to +24 dBFS. dB mapping
uses a −120 dBFS floor and `(level_db-lower)/(upper-lower)`. Amplitude mapping
uses `(level-10^(lower/20))/(10^(upper/20)-10^(lower/20))`. Clamp to 0–1 before
optional inversion. Amplitude endpoints that round to the same floating-point
level form a finite step instead of dividing by zero. Invert defaults off. Silence is a real signal and follows
the mapping; a missing source instead publishes zero contribution weight.

Core `sidechain_order` combines every assigned control dependency with group
and send dependencies. Include disabled/zero-amount/retained assignments.
`sidechain_source_available` tests reachability through the same edges for the
picker and transfer eligibility. Core transactions reject cycles atomically.
The renderer compiles beats before constructing this graph. Each source bus
runs before its consumers; detectors advance once per processing frame and
publish prepared values before providers, voices and effects consume them.

Prepare stage offsets from preceding layer, Output and track effects. Each
track's ingress latency is the maximum of its audio ingress and each controlling
source latency minus the receiving stage offset, bounded below by zero. Route
delay is `destination_ingress + stage_offset - source_output_latency`. Delay
storage, order, source indices and publication arrays are prepared off callback.
Generated/audio track clocks are delayed by ingress; sibling and Return alignment
uses the combined graph. Master uses aligned track ingress plus its effect-stage
offset. Playback, export, streaming readiness and transport preparation use these
same clocks. Sidechain projects use serial graph replay rather than independent
parallel track jobs. Checkpoint accounting includes followers, delays and caches.

Source presence, topology and alignment changes require prepared replacement
history. Compatible route/settings changes preserve detector state, prepared
delay storage and publication ramps. Detector topology changes reset that
follower; changed mapping or response times retain its compatible history. Seek
preparation replays the combined graph; looping retains detector history across
wraps. Each processing node releases note gates when its own ingress-delayed
clock wraps; the global feed clock does not release delayed nodes early. Transport pauses freeze the graph after the ordinary output fade.

Source removal retains its UUID; reopening, renaming, reordering, Save As and Undo
do not retarget it. Track, layer and effect copies get fresh sidechain route IDs
and retain original source-track assignments, including group copies. Transfers
retain assignments and pass final graph validation. Missing sources have neutral
weight even when Invert maps silence to one. Unknown sidechain schema versions
fail settings validation rather than being reinterpreted.

UI appends Tracks, Groups and Returns after the existing provider groups, with
exactly two section dividers. It uses the common Small dropdown layout. Selecting
an audio source keeps the standard 90-point connection strip and its existing
destination-matched Amount knob. It appends Detector, Mapping and Response strips,
each using `unit_columns_width(2)`, `unit_columns` and `unit_strip_with_width`,
as Chorus Motion does: 161-point width, 52-point knob columns and 19-point knob gaps,
no internal divider, standard padding, full-height outer dividers, one centered
footer, and the existing expanded/compact lane height. Strip reordering includes
the complete connection and settings as one stable route. Telemetry is actual
renderer Input dBFS and mapped value through the existing atomic snapshot path.

Selectors use each column's padded 70-point field, matching Source/Destination,
while knob centers and 52-point row allocations remain fixed. Input/Value use
the shared numeric font on one line; longer dBFS readings cannot move the footer.
Anchor all three footers to the shared strip body height, including when Mapping
readouts use less space than the standard knob rows.
UI drags keep one 0.1 dB or 0.1 Hz input step between paired limits. Clamp after the
knob's floating-point conversion. Do not shrink the gap on successive frames.
Retain narrower valid saved ranges without changing them during painting; an
edit cannot narrow them further. Bound no-ops must not reach validation as errors.
