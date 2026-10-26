# All effect plugin builds

Status: development previews. Native host and release acceptance remain open.

## Products and ownership

All fourteen registered effect units have a `plugins/<unit>/unit.toml` manifest and independent `daw-<unit>-plugin`, `daw-<unit>-editor`, `daw-<unit>-clap`, and `daw-<unit>-vst3` packages. The [user plugin table](../../docs/effects/plugins.md) defines product names, slugs, and exposed controls. The original four products retain their public identities and binary state formats.

The ten added plugin models map explicit parameter IDs to portable settings. IDs start at 100 (Bypass), remain contiguous, and follow each checked manifest. Never regenerate class IDs or reorder IDs after publication. Identity/default-state fixtures freeze each contract. State uses an eight-byte product magic, two little-endian u32 versions, and one little-endian f64 per parameter in ID order. Reject foreign magic, future versions, invalid values, truncation, and trailing data before publication.

The original Drive CLAP/VST3 adapters compile against each concrete product through dependency aliases. Shared bounded state exchange, binary I/O, normalized mappings, gestures, native window lifetime, and format boundary handling remain in their existing owners. No plugin depends on DAW engine, project, device, or application UI crates. Unit-owned editor bodies compose shared `unit-ui` knobs, strips, footers, colors, and meter painters. DAW shells and modulation remain independent.

DSP preparation allocates delay storage, settings, meters, and fixed queues before activation. Parameter automation updates prepared settings and retargets the portable DSP without heap work, locks, I/O, serialization, SDK calls, or final-owner retirement. Portable units retain their own bounds and ramps. OTT applies upper-crossover precedence with at least a 1 Hz gap; Stereo Spread uses its existing constrained-frequency helper. Host base values remain unchanged when effective crossover bounds constrain the DSP.

## Timing, latency, and monitoring

Both formats support one stereo f32 input/output bus and six processing rates. Host timing reaches Auto-filter and Chorus: CLAP block transport and timestamped transport events, and VST3 block context tempo/sample position. The clock starts at frame zero and 120 BPM. VST3 offline processing uses the supplied sample position even without a Playing flag. Stopped live transport or missing timing advances the local counter and retains the last valid tempo; invalid tempo is ignored and negative positions clamp to zero. CLAP tempo updates follow block snapshots and explicit transport events; intra-block `tempo_inc` ramps remain outside this preview. No MIDI, sidechain, or DAW modulation routes are advertised.

Limiter latency is `ceil(sample_rate / 100) + 32` frames in either mode, including bypass. Saturator latency remains 32; other products report zero. Host queries use the selected rate, including before first processing. New products report a conservative four-second tail, including the maximum Chorus feedback delay; the original four retain one second. Quiet input must not cause the adapters to sleep before their portable histories advance.

All added products include standard output current/held sample-peak telemetry. Compressor/Limiter also attach their portable reduction peak meters; OTT attaches its signed band meter. Portable `UnitMeter`/`LevelMonitor` owns enable clearing and publication. Shared gain-monitor painters now live in `unit-ui`; the DAW reexports them and retains its existing tests.

CLAP shares prepared atomics with the editor. VST3 exchanges copied scalar snapshots only on the control thread. The original output-only products retain two wire values (`current`, `peak`); new products carry thirteen values: output current/hold, reduction current/hold, three signed band currents, three reduction holds, and three boost holds. Extra attributes use `meter.<index>`. Reset masks select independent held readings. Meter messages, appearance, and resets never enter parameter/state payloads.

## Multiband EQ prepared bank

The plugin prepares 64 stable band slots, each with Enabled, Mode, Frequency, Gain, and Q parameters, plus Bypass and output Level (322 parameters). The first slot starts enabled. Slot identities are deterministic non-nil UUIDs 1–64 internal to the prepared bank; no DAW project identity is serialized. Disabled slots retain their parameters and suspend only after their 5 ms mode-weight fade reaches zero. Ordinary DAW bands retain warm filter histories and variable topology.

`MultibandEq::retarget` changes only same-ID/same-order prepared settings. Route/topology changes still require worker preparation and `inherit`. Reject mismatched topologies before changing any band. The plugin retains its settings vector through processing; automation must not reconstruct it. Editor and CLAP gesture queues scale to the fixed parameter count while remaining bounded. The editor uses a fixed viewport with horizontal scrolling.

## Packaging, CI, and checks

The manifest-discovered packager requires no product allowlist. `--units all` now selects fourteen units. Test fixture libraries cover all products on each supported archive layout and retain transactional publication after later-unit/signing failures.

The plugin workflow runs fourteen products across three platforms (42 jobs), with both formats, typed model/editor and ABI tests, packaging, separate artifacts, checksums, and logs. Existing library-path environment variables retain `DRIVE_*` names for shared host tests.

ABI tests cover native metadata, latency at all rates, every parameter, choices, dense automation, legal in-place/irregular buffers, zero-frame flushes, state corruption and active recall. Product tests freeze identities and default state, validate mappings/text conversions, compare portable DSP and wrappers, and check bypass meter clearing. Platform-native editor/device and external-host checks remain separate from automated tests. Record executed checks and remaining limits in [validation history](../validation/README.md).
