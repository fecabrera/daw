# Audio sidechain modulation plan

Status: planned high-priority feature. The user requested integration into the
existing modulation controls: select a track, group or Return in Source, then
configure how its audio maps to normalized modulation values from 0 to 1.
This document records the design direction and proposed initial scope. Resolve
bounds, defaults and timing decisions before implementing the model or DSP.

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

Proposed audio-to-control settings:

| Control | Purpose |
| --- | --- |
| Tap | Explicit signal location, initially Pre-fader or Post-fader after source effects; define exact group/Return and mute/solo semantics before coding. |
| Channel | Linked stereo, Left, Right, Mid or Side; define channel normalization and linked detection mathematically. |
| Frequency range | Optional high-pass/low-pass filtering of the detector signal; does not filter the audible source. |
| Detector | Peak or RMS (root mean square); rectify/measure audio rather than using the signed waveform as a level. |
| Input range | Lower and upper levels in dBFS (decibels relative to digital full scale), mapped to 0 and 1. Enforce ordered finite limits. |
| Attack / Release | Response times in milliseconds; publish their supported bounds and defaults in the topic contract. |
| Mapping curve | Initially linear amplitude or linear dB; define the exact transfer function and processing order. |
| Invert | Reverse the mapped value within 0–1; retain independent signed Amount. |
| Readout | Detected input level and final mapped control value, from actual processing telemetry. |

The transfer function clamps to 0 below the lower input bound and 1 above the
upper bound before optional inversion. Define a finite silence floor for the
dB mapping, filter/detector state, RMS window, envelope timing, and whether
smoothing precedes mapping. Do not silently apply inconsistent detector and
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
state. Decide ownership/deduplication before freezing the saved schema.

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

Specify tap points for regular tracks, nested groups and Returns, including
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
reorder, save/reopen and Save As. Define route remapping for copies/transfers and
source deletion, including Undo restoration. Retain unresolved source identities
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
