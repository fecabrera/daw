# All effect plugin builds

Status: development previews. Native host and release acceptance remain open.

## Products and ownership

The registered standalone effect products each have a `plugins/<unit>/unit.toml` manifest and independent `daw-<unit>-plugin`, `daw-<unit>-editor`, `daw-<unit>-clap`, and `daw-<unit>-vst3` packages. The [user plugin table](../../docs/effects/plugins.md) defines product names, slugs, and exposed controls. The original four products retain their public identities and binary state formats. The later [Multiband Exciter](../effects/exciter.md) is built-in only and does not add a product to the packager or CI matrix.

The later plugin models map explicit parameter IDs to portable settings. IDs start at 100 (Bypass), remain contiguous, and follow each checked manifest. Never regenerate class IDs or reorder IDs after publication. Identity/default-state fixtures freeze each contract. State uses an eight-byte product magic, two little-endian u32 versions, and one little-endian f64 per parameter in ID order. Reject foreign magic, future versions, invalid values, truncation, and trailing data before publication.

The original Drive CLAP/VST3 adapters compile against each concrete product through dependency aliases. Shared bounded state exchange, binary I/O, normalized mappings, gestures, native window lifetime, and format boundary handling remain in their existing owners. No plugin depends on DAW engine, project, device, or application UI crates. All registered plugin editors and their built-in counterparts call the same effect body renderers in `unit-ui`. DAW shells, editing transactions and modulation discovery remain in `ui`; SDK automation bindings remain in each editor crate.

DSP preparation allocates delay storage, settings, meters, and fixed queues before activation. Parameter automation updates prepared settings and retargets the portable DSP without heap work, locks, I/O, serialization, SDK calls, or final-owner retirement. Portable units retain their own bounds and ramps. OTT applies upper-crossover precedence with at least a 1 Hz gap; Stereo Spread uses its existing constrained-frequency helper. Host base values remain unchanged when effective crossover bounds constrain the DSP.

### Built-in editor parity

`unit-ui` owns effect control order, knob specifications, value labels, choice diagrams, strip geometry, footers and meter placement. Both hosts call these renderers. Do not copy a built-in body into a plugin adapter or add plugin-only controls or meters. The DAW adds its unit header and modulation connections outside the portable body.

The accepted output-meter exception uses the shared `HIDE_OUTPUT_LEVELS_TAG`
presentation policy: DAW shells suppress horizontal output rails and provide
stereo gap meters; standalone editors retain their existing output rails.
Per-band level, gain-reduction and signed gain-change meters remain shared.
Paired body comparisons use the default portable presentation; separate DAW
tests cover this host policy without changing strip or control geometry.

The shared layout uses 90-point single strips and 161-point two-column strips, with the same equal column gaps, panel margin and control height in both hosts. Fixed editor bounds contain every strip and response. Multiband EQ includes the shared three-row Response graph before its band strips. Its default viewport is 488×218 and scrolls horizontally when the visible bank exceeds the viewport.

Plugin adapters use `control_capture::scope` to collect actual widget responses and map their rectangles to frozen parameter IDs. The collection is scoped to UI painting and releases response ownership on return or unwinding. It is never accessed by the audio callback. Captured meter clicks send reset requests, not parameter edits.

All registered editors have paired tessellated rendering comparisons against their portable built-in bodies in Dark, Light and Mist, at 1×, 1.5× and 2× scale, with defaults and two changed parameter snapshots. Compare clips, textures, triangles, vertex positions and colors. An active gesture must also send its End when host automation hides that control. Reuse the bounded enqueue/reservation path, retain the active flag on a full queue, and retry on the next frame. Host mode changes, release and focus loss must not leave an edit open. Separate gesture checks verify parameter identity and balanced host gestures, including both rate modes; Saturator dropdowns have dedicated selection checks. Geometry checks cover control containment and contiguous strip order. Native host attachment and display scaling still require platform checks.

## Timing, latency, and monitoring

Both formats support one stereo f32 input/output bus and six processing rates. Host timing reaches Auto-filter and Chorus: CLAP block transport and timestamped transport events, and VST3 block context tempo/sample position. The clock starts at frame zero and 120 BPM. VST3 offline processing uses the supplied sample position even without a Playing flag. Stopped live transport or missing timing advances the local counter and retains the last valid tempo; invalid tempo is ignored and negative positions clamp to zero. CLAP tempo updates follow block snapshots and explicit transport events; intra-block `tempo_inc` ramps remain outside this preview. No MIDI, sidechain, or DAW modulation routes are advertised.

Limiter latency is `ceil(sample_rate / 100) + 32` frames in either mode, including bypass. Saturator latency remains 32; Harmonic Lift reports 49 frames in all quality/mix/bypass states; other products report zero. Host queries use the selected rate, including before first processing.

Tail declarations use each product's fixed host bound. The original four products retain one second. Other products report four seconds, including the maximum Chorus feedback delay, with these exceptions: [Plate Reverb reports 45 seconds and Ambient Reverb reports 180 seconds](../effects/reverbs.md#implementation-and-provenance); [Loudness Meter reports zero](../effects/loudness-meter.md). Reverb host bounds remain at their product maxima so automation cannot shorten an active host tail. Quiet input must not cause the adapters to sleep before their portable histories advance.

All added products include standard output current/held sample-peak telemetry. Compressor/Limiter also attach their portable reduction peak meters; OTT attaches its signed band meter. Portable `UnitMeter`/`LevelMonitor` owns enable clearing and publication. The shared body places only the meters present in its built-in counterpart. Equalizer uses per-band levels; Multiband EQ uses per-band levels and an output rail. Direction Mixer and Stereo Spread retain their built-in controls without an extra output rail. Shared meter painters live in `unit-ui`.

CLAP shares prepared atomics with the editor. VST3 exchanges copied scalar snapshots only on the control thread. The original output-only products retain two wire values (`current`, `peak`); nine of the new products carry thirteen values: output current/hold, reduction current/hold, three signed band currents, three reduction holds, and three boost holds. Multiband EQ appends 64 linear band readings and 64 held sample peaks for 141 values, followed by sample rate, 512 spectrum magnitudes and coherent-frame metadata for 668 values total, with independent per-band peak reset requests. See [spectrum wire compatibility](#spectrum-wire-compatibility). Equalizer uses the existing three band-current fields for its linear band readings. Extra attributes use `meter.<index>`. Reset masks select independent held readings. Meter messages, appearance, and resets never enter parameter/state payloads.

## Multiband EQ prepared bank

The plugin prepares 64 stable band slots, each with Enabled, Mode, Frequency, Gain, and Q parameters, plus Bypass and output Level (322 parameters). The first slot starts enabled. Slot identities are deterministic non-nil UUIDs 1–64 internal to the prepared bank; no DAW project identity is serialized. Disabled slots retain their parameters and suspend only after their 5 ms mode-weight fade reaches zero. Ordinary DAW bands retain warm filter histories and variable topology.

`MultibandEq::retarget` changes only same-ID/same-order prepared settings. Route/topology changes still require worker preparation and `inherit`. Reject mismatched topologies before changing any band. The plugin retains its settings vector through processing; automation must not reconstruct it. Editor and CLAP gesture queues scale to the fixed parameter count while remaining bounded. The editor starts with one band, the shared Add strip and Output. Add reveals and enables the next hidden slot; power bypass retains the visible strip. Remove resets and hides that slot while preserving its stable identity. Enabled or modified slots become visible after state recall. The visible-slot mask is instance UI state, separate from parameters and serialized presets. The bank scrolls horizontally as needed, and Add is disabled at 64 visible slots. These presentation rules do not change the fixed automation bank. Tests cover Add, power, Remove and identity preservation.

Single-field automation resolves its canonical slot in constant time and calls
`MultibandEq::retarget_prepared_band` for discrete changes and
`retarget_prepared_band_continuous` for Gain/Frequency/Q changes. This entry point checks the slot index and
stable UUID before changing any state. Output Level updates only the output ramp;
Bypass updates the global ramp and all meter enable states. The full validated
same-topology `retarget` path remains available for preset recall. Both paths use
the same band ramps, including multiple targets at one sample timestamp; neither
changes processing order or creates bands. Tests compare the per-field paths
against full retargeting at all six rates, including reset, same-sample transient
targets, meters, presentation order and callback heap operations. The explicit
release cost regression compares 64-frequency and 322-field automation against
the unchanged full-bank reference using warmed, paired, alternated total-time
measurements and a seven-run median. Retain the less-than-50% total-time gate;
this is a relative regression check, not a device deadline.

The checked Multiband EQ manifest declares state version 2. Build validation emits `MANIFEST_STATE_VERSION`; its encoder exports and uses that approved value. A wire-header contract test compares the manifest value and actual encoded envelope. Version-1 decoding remains supported.

Standalone Multiband EQ uses the shared release-only strip gesture lifecycle. Its version-2 state appends a validated 64-slot permutation; version-1 state loads with the original order. Published automation IDs and their slot bindings remain fixed. UI controls resolve slots by stable UUID, and the prepared DSP retains canonical slot order. Shared state exchanges publish the complete bounded state, including presentation fields. CLAP consumes order from its bounded editor queue on the control thread, saves it immediately without audio-event delivery, and marks non-parameter state dirty after releasing queue borrows. Order does not enter DSP processing. VST3 stores the order in controller state and calls the host dirty-state hook. The owning host controls native state undo; native undo/recall acceptance is not established by headless tests. The visible-slot mask remains instance-only UI state as described above.

## Packaging, CI, and checks

The manifest-discovered packager requires no product allowlist. `--units all` selects every manifest-registered product. Test fixture libraries cover all products on each supported archive layout and retain transactional publication after later-unit/signing failures.

The plugin workflow runs registered products across three platforms (one job per product and platform), with both formats, typed model/editor and ABI tests, packaging, separate artifacts, checksums, and logs. Existing library-path environment variables retain `DRIVE_*` names for shared host tests.

ABI tests cover native metadata, latency at all rates, every parameter, choices, dense automation, legal in-place/irregular buffers, zero-frame flushes, state corruption and active recall. Product tests freeze identities and default state, validate mappings/text conversions, compare portable DSP and wrappers, and check bypass meter clearing. Platform-native editor/device and external-host checks remain separate from automated tests. Record executed checks and remaining limits in [validation history](../validation/README.md).

## Runtime notice inventory — 2026-11-02

`tools/runtime_notices.json` records exact text hashes for the confirmed tracing,
tracing-subscriber, libloading, ashpd, alsa-sys and ICU4X notice omissions. Their
texts live in `crates/ui/assets/licenses`, already copied by all desktop and plugin
packagers. Archive tests compare the staged bytes with this inventory for each
format and target. `.gitattributes` disables Git text conversion for all files in
`crates/ui/assets/licenses`, preserving upstream LF and CRLF bytes under Windows
checkout settings. Do not normalize notice files or regenerate inventory hashes
to match checkout conversion. A temporary-index regression exercises
`core.autocrlf=true` and `core.eol=crlf` without creating a commit.
Notices can be included conservatively on targets that do not
link a component. This inventory closes those omissions; it is not a complete
license or corresponding-source audit of every dependency and deliverable.

## Loudness Meter

The stereo analyzer adds an independent typed contract, shared editor and CLAP/VST3
products. Both editors use the 720-point shared history body. The product has four
discrete parameters, zero audio latency/tail, bounded worker analysis and transient
Hold/Reset actions outside automation. Its extended control-thread telemetry and
measurement limits are defined in the [meter specification](../effects/loudness-meter.md).

## Runtime notice closure correction — 2026-11-03

The runtime inventory now retains all 36 selected normal-runtime omissions from
the six-target/per-product audit, plus the shared legacy objc2 MIT grant containing
Steven Sheldon’s copyright. Exact upstream bytes are retained in the common UI
notice directory; existing desktop and plugin copy rules include them. Unused
target notices may be bundled conservatively. Build/proc-macro-only and currently
unpackaged tool candidates were excluded from this correction.

Desktop staging and Linux archives, as well as both plugin-format archives across
the supported package targets, verify the inventory hashes. The inventory is a
bounded regression record; dependency/feature changes still require a fresh
license-closure assessment. Corresponding-source/relink release obligations remain
separate from notice retention.

## Supplemental source grants — 2026-11-04

The shared notice inventory retains the exact Valuable 0.1.1 MIT grant from upstream
`9efc29b6e58cef28f6566a47aa7e142a55fead77`. Its registry source payload omits the grant.
The source packager checks package name/version, registry checksum, VCS revision,
VCS subpath and notice SHA-256 before copying the supplemental text. It writes
`SOURCE-LICENSES/index.json` and the unmodified grant into the source archive.
Common desktop/plugin notice inputs retain the same grant conservatively. These
checks cover source-only optional packages too; metadata resolution alone does
not prove notice completeness. They do not modify the pinned native ONNX runtime.

## Multiband Compressor and band audition

[Multiband Compressor](../effects/multiband-compressor.md) adds an independent
643-parameter, 64-slot adjacent-range product with zero alignment latency and
a conservative four-second plugin tail bound. It shares the built-in body, portable Compressor DSP and LR4
crossovers. Its state retains inactive slots and a visual permutation.

Multiband EQ and Multiband Compressor share temporary S controls. Solo flags are
outside parameters, presets and state publication. VST3 transports per-slot
index/boolean messages on the control thread; CLAP shares prepared atomics.
VST3 offline processing and the CLAP render extension gate audition out of audio.
Native host validation is still separate from automated adapter/editor tests.

## Shimmer Reverb

[Shimmer Reverb](../effects/shimmer-reverb.md) adds a checked 18-parameter product,
a 611-point shared body, independent CLAP/VST3 identities and version 1 bounded
state. Both formats receive transport tempo for synced pre-delay. It reports zero
alignment latency and a fixed 120-second tail budget. The original portable DSP
uses bounded pitch regeneration and a contractive eight-line late tank.

## Spectrum wire compatibility

Multiband EQ schema 2 uses `daw.output-meter.request.v2` and
`daw.output-meter.reading.v2`, with a required `value-count` integer of 668.
The controller tries v2 first and falls back to v1 when unsupported. V1 requests
receive only 206 exposed attributes and zero spectrum. New controllers accept
the original 142-value scalar/rate prefix, mask legacy spectrum and mark it
unavailable. Other products retain their existing v1 messages and payload sizes.
Shared adapter code must not require EQ-only symbols from other product crates.

The [effect contract](../effects/multiband-eq.md#response-graph-and-live-spectrum)
defines all 668 offsets. Four exact unsigned 16-bit words encode each u64 clock
or generation. Reject unsupported schemas, lengths, flags, rates, nonfinite
values, invalid words and impossible ready clocks before applying the frame.
Stopped transport masks magnitudes/readiness/clocks while retaining scalar held
peaks. CLAP uses the same coherent portable frame directly; transient telemetry
never changes parameter IDs, product identities or persistent state bytes.
