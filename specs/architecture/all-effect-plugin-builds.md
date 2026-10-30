# All effect plugin builds

Status: development previews. Native host and release acceptance remain open.

## Products and ownership

The fourteen standalone effect products each have a `plugins/<unit>/unit.toml` manifest and independent `daw-<unit>-plugin`, `daw-<unit>-editor`, `daw-<unit>-clap`, and `daw-<unit>-vst3` packages. The [user plugin table](../../docs/effects/plugins.md) defines product names, slugs, and exposed controls. The original four products retain their public identities and binary state formats. The later [Multiband Exciter](../effects/exciter.md) is built-in only and does not add a product to the packager or CI matrix.

The ten added plugin models map explicit parameter IDs to portable settings. IDs start at 100 (Bypass), remain contiguous, and follow each checked manifest. Never regenerate class IDs or reorder IDs after publication. Identity/default-state fixtures freeze each contract. State uses an eight-byte product magic, two little-endian u32 versions, and one little-endian f64 per parameter in ID order. Reject foreign magic, future versions, invalid values, truncation, and trailing data before publication.

The original Drive CLAP/VST3 adapters compile against each concrete product through dependency aliases. Shared bounded state exchange, binary I/O, normalized mappings, gestures, native window lifetime, and format boundary handling remain in their existing owners. No plugin depends on DAW engine, project, device, or application UI crates. All fourteen plugin editors and their built-in counterparts call the same effect body renderers in `unit-ui`. DAW shells, editing transactions and modulation discovery remain in `ui`; SDK automation bindings remain in each editor crate.

DSP preparation allocates delay storage, settings, meters, and fixed queues before activation. Parameter automation updates prepared settings and retargets the portable DSP without heap work, locks, I/O, serialization, SDK calls, or final-owner retirement. Portable units retain their own bounds and ramps. OTT applies upper-crossover precedence with at least a 1 Hz gap; Stereo Spread uses its existing constrained-frequency helper. Host base values remain unchanged when effective crossover bounds constrain the DSP.

### Built-in editor parity

`unit-ui` owns effect control order, knob specifications, value labels, choice diagrams, strip geometry, footers and meter placement. Both hosts call these renderers. Do not copy a built-in body into a plugin adapter or add plugin-only controls or meters. The DAW adds its unit header and modulation connections outside the portable body.

The shared layout uses 90-point single strips and 161-point two-column strips, with the same equal column gaps, panel margin and control height in both hosts. Fixed editor bounds contain every strip and response. Multiband EQ scrolls horizontally when its visible band bank exceeds the viewport.

Plugin adapters use `control_capture::scope` to collect actual widget responses and map their rectangles to frozen parameter IDs. The collection is scoped to UI painting and releases response ownership on return or unwinding. It is never accessed by the audio callback. Captured meter clicks send reset requests, not parameter edits.

All fourteen editors have paired tessellated rendering comparisons against their portable built-in bodies in Dark, Light and Mist, at 1×, 1.5× and 2× scale, with defaults and two changed parameter snapshots. Compare clips, textures, triangles, vertex positions and colors. Separate gesture checks verify parameter identity and balanced host gestures, including both rate modes; Saturator dropdowns have dedicated selection checks. Geometry checks cover control containment and contiguous strip order. Native host attachment and display scaling still require platform checks.

## Timing, latency, and monitoring

Both formats support one stereo f32 input/output bus and six processing rates. Host timing reaches Auto-filter and Chorus: CLAP block transport and timestamped transport events, and VST3 block context tempo/sample position. The clock starts at frame zero and 120 BPM. VST3 offline processing uses the supplied sample position even without a Playing flag. Stopped live transport or missing timing advances the local counter and retains the last valid tempo; invalid tempo is ignored and negative positions clamp to zero. CLAP tempo updates follow block snapshots and explicit transport events; intra-block `tempo_inc` ramps remain outside this preview. No MIDI, sidechain, or DAW modulation routes are advertised.

Limiter latency is `ceil(sample_rate / 100) + 32` frames in either mode, including bypass. Saturator latency remains 32; other products report zero. Host queries use the selected rate, including before first processing. New products report a conservative four-second tail, including the maximum Chorus feedback delay; the original four retain one second. Quiet input must not cause the adapters to sleep before their portable histories advance.

All added products include standard output current/held sample-peak telemetry. Compressor/Limiter also attach their portable reduction peak meters; OTT attaches its signed band meter. Portable `UnitMeter`/`LevelMonitor` owns enable clearing and publication. The shared body places only the meters present in its built-in counterpart. Equalizer uses per-band levels; Multiband EQ uses per-band levels and an output rail. Direction Mixer and Stereo Spread retain their built-in controls without an extra output rail. Shared meter painters live in `unit-ui`.

CLAP shares prepared atomics with the editor. VST3 exchanges copied scalar snapshots only on the control thread. The original output-only products retain two wire values (`current`, `peak`); nine of the new products carry thirteen values: output current/hold, reduction current/hold, three signed band currents, three reduction holds, and three boost holds. Multiband EQ appends 64 linear band readings for 77 values total. Equalizer uses the existing three band-current fields for its linear band readings. Extra attributes use `meter.<index>`. Reset masks select independent held readings. Meter messages, appearance, and resets never enter parameter/state payloads.

## Multiband EQ prepared bank

The plugin prepares 64 stable band slots, each with Enabled, Mode, Frequency, Gain, and Q parameters, plus Bypass and output Level (322 parameters). The first slot starts enabled. Slot identities are deterministic non-nil UUIDs 1–64 internal to the prepared bank; no DAW project identity is serialized. Disabled slots retain their parameters and suspend only after their 5 ms mode-weight fade reaches zero. Ordinary DAW bands retain warm filter histories and variable topology.

`MultibandEq::retarget` changes only same-ID/same-order prepared settings. Route/topology changes still require worker preparation and `inherit`. Reject mismatched topologies before changing any band. The plugin retains its settings vector through processing; automation must not reconstruct it. Editor and CLAP gesture queues scale to the fixed parameter count while remaining bounded. The editor starts with one band, the shared Add strip and Output. Add reveals and enables the next hidden slot; power bypass retains the visible strip. Remove resets and hides that slot while preserving its stable identity. Enabled or modified slots become visible after state recall. The visible-slot mask is instance UI state, separate from parameters and serialized presets. The bank scrolls horizontally as needed, and Add is disabled at 64 visible slots. These presentation rules do not change the fixed automation bank. Tests cover Add, power, Remove and identity preservation.

The fixed band order is a current implementation limit under the [variable-strip reordering requirement](modular-audio-units.md#variable-strip-reordering). Standalone band reordering remains pending. Its implementation must preserve the published slot parameter IDs and bindings while saving the chosen order separately; it must not renumber automation parameters by display position.

## Packaging, CI, and checks

The manifest-discovered packager requires no product allowlist. `--units all` now selects fourteen units. Test fixture libraries cover all products on each supported archive layout and retain transactional publication after later-unit/signing failures.

The plugin workflow runs fourteen products across three platforms (42 jobs), with both formats, typed model/editor and ABI tests, packaging, separate artifacts, checksums, and logs. Existing library-path environment variables retain `DRIVE_*` names for shared host tests.

ABI tests cover native metadata, latency at all rates, every parameter, choices, dense automation, legal in-place/irregular buffers, zero-frame flushes, state corruption and active recall. Product tests freeze identities and default state, validate mappings/text conversions, compare portable DSP and wrappers, and check bypass meter clearing. Platform-native editor/device and external-host checks remain separate from automated tests. Record executed checks and remaining limits in [validation history](../validation/README.md).
