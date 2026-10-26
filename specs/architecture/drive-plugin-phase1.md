# Drive plugin Phase 1: identity, parameters, and state

Status: shared contract implemented. [Phase 2](drive-plugin-phase2.md) now adds the headless CLAP processor; [Phase 3](drive-plugin-phase3.md) adds the headless VST3 processor/controller; the plugin editor remains later work. Built-in DSP, catalog construction, and DAW project serialization retain their existing behavior.

This implements [Phase 1](unit-plugin-builds.md#phase-1--identity-manifest-parameters-and-state). The new [daw-drive-plugin crate](../../crates/drive-plugin/src/lib.rs) owns SDK-independent metadata, typed parameter conversion, and validated copyable state. Phase 1 initially depends only on `daw-unit-core` at runtime; Phase 2 adds the portable `daw-drive` processor dependency; Phase 3 adds format-neutral `daw-plugin-support` and shares state publication through `StateExchange`. It has no application, DSP algorithm, plugin SDK, window, or device dependency. The future format adapters must consume this contract rather than duplicate it.

## Frozen identity and versions

The user selected vendor **Sie**, product **Drive**, and the proposed CLAP ID. The following production identities are assigned once in the [build manifest](../../plugins/drive/unit.toml), independently of the disposable Phase 0 probe:

| Field | Frozen value |
| --- | --- |
| CLAP ID | `io.github.fecabrera.daw.drive` |
| VST3 processor class ID | `f399b92b5d264c7d98c6de3c2654d534` |
| VST3 controller class ID | `c212153f328b4d87a659bb681c55f768` |
| Provenance | original `daw.drive`; excludes `daw.drive-v2` |
| Initial plugin version | `0.1.0`; independently versioned from the DAW |
| Manifest schema / plugin state schema / Drive settings version | `1` / `1` / `1` |

Class IDs are stored as canonical 16-byte UUID order. The VST3 adapter must apply the pinned SDK's platform TUID encoding, including Windows byte order, and test both processor and controller IDs. Do not generate IDs per build or publish the probe IDs as Drive.

Compatible releases keep names, IDs, mappings, and state semantics. Increment plugin patch/minor versions for compatible fixes/additions and update the manifest and crate version together. A state layout change needs its own schema version and tested migration. A manifest-only schema change need not change saved state. A deliberately incompatible product needs new identities and an explicit migration policy. There is no promise that hosts offer version selection for plugins with the same identity.

## Checked manifest and panel bindings

`unit.toml` is compile-time data. `build.rs` parses it with the already locked `toml_edit` parser and generates identity, parameter order, and typed two-strip bindings. Parsing is a build dependency, not a runtime loader. The build rejects unknown fields, unsupported schemas/type IDs, duplicate or unknown parameter IDs, missing parameters/controls, range/default/scale differences, invalid class IDs, and unsupported control/diagram/enablement references.

`Parameter::info` takes defaults from the existing `DriveSettings`. Drive range constants remain in `unit-core` and are used by its validation and the parameter model. Manifest numeric defaults are checked at the portable settings' `f32` precision; for example, manifest `0.2` resolves to the existing `f32` Gain default promoted to `f64`. Typed metadata is authoritative for host defaults. No DSP range or default is silently changed to match a manifest.

The panel has Drive: Volume/Gain/Tone and Color: Shape/Strength/Frequency. These are typed bindings, not widget implementations. Only Shape uses the `drive_color` diagram identifier. Strength and Frequency depend on Shape being nonzero for editor interaction; host automation and state still retain/update them while Off. Bypass is host-visible and absent from the six-control body. The existing DAW panel is not replaced in this phase. Shared body extraction and native interaction remain Phase 4.

## Parameter meaning and precision

IDs 100–106 are frozen in the [plan's parameter table](unit-plugin-builds.md#manifest-identity-and-parameter-contract) and `Parameter`. IDs are not enumeration positions. CLAP receives validated plain `f64`; VST3 conversion uses normalized `f64`. Nonfinite, out-of-range, unknown, and nonintegral plain discrete values return errors without editing current state. Normalized discrete values select the nearest choice; exact half-step ties round upward. Boolean bypass 1 maps to `DriveSettings.enabled = false`.

Pure [gain helpers](../../crates/unit-core/src/gain.rs) now own the shared fader and knob scales, dB parser, and dB readout. DAW knobs/faders delegate to them. The gain knob retains unity at 0.5, the existing scale knots, an amplitude tail below −60 dB, and exact silence only at normalized zero. Frequency uses `ln(hz / 250) / ln(2000 / 250)`. Continuous unit-interval parameters use their plain values directly; Color's frozen choice order is Off, Mid scoop, Mid hump, Mid hump + bass cut.

Only the finite −1000 dB sentinel formats as minus infinity. A supported finite value such as −950 dB retains its numeric readout, state value, and nonzero host normalization even when its DSP `f32` amplitude underflows to zero. The DSP arithmetic and silence policy do not change. Very small positive normalized values below the supported finite tail resolve to the next representable `f64` above the sentinel. Conversion to portable `f32` settings prevents finite values from rounding into the sentinel. Float precision limits prevent promising a bijection for every normalized subnormal value.

Tests require gain/frequency plain round trips within `1e-10` for the fixture corpus and normalized round trips within `1e-13` over a 10,001-point ordinary curve sweep. The low-tail fixture additionally checks relative normalized error below `1e-12`, so a false zero cannot pass an absolute tolerance. Portable rounding can differ by at most 0.000062 dB across the Volume range, 0.000123 Hz across the Frequency range, and `6e-8` for unit-interval controls; preserve the original host `f64` values in state. No conversion tolerance is a license to change the Drive DSP or add smoothing.

Host text uses one decimal dB, whole percentages, whole Hz, choice names, and On/Off bypass. Within 0.1 dB above the silence sentinel, retain sufficient digits to prevent text from rounding into silence. Text is a rounded readout, not a lossless state representation. Percent text with or without `%` is interpreted in percent units (`20` and `20%` both mean 0.2). Frequency accepts Hz, kHz, or bare Hz numbers. Volume accepts bare dB, a dB suffix, and explicit negative infinity spellings. Invalid units, nonfinite numbers, and values outside the parameter's range fail.

## State envelope and failure rules

Version 1 is exactly 72 bytes, with little-endian numeric fields:

| Offset / bytes | Content |
| --- | --- |
| 0 / 8 | ASCII `DAWDRIVE` product tag |
| 8 / 4 | Plugin state schema: unsigned integer 1 |
| 12 / 4 | Portable Drive settings version: unsigned integer 1 |
| 16 / 56 | Seven IEEE 754 binary64 plain values, ordered by IDs 100–106 |

There are no editor preferences yet. Later optional presentation preferences require an explicit schema/migration design. There are no DAW paths, UUIDs, routes, collapsed state, theme-store references, or DSP history in this envelope. Converting existing settings intentionally projects base parameters and creates default DAW presentation/empty routes on output; it does not modify the original settings.

`decode` validates the complete candidate before returning it. `restore` replaces a snapshot only after successful decoding. Bad length/tag, trailing bytes, unsupported versions, invalid choices, nonfinite values, and invalid ranges retain the caller's old state. Schema 0 is not an earlier public format and is rejected, as are future schemas. Schema 1 is the first supported fixture; no fictitious earlier-state migration is claimed.

Control-thread `read_from` consumes at most 73 bytes and 146 read attempts, including interrupted calls. `write_to` completes short writes within 144 attempts and rejects zero/error writes. Each format adapter must bound its stream reader to its state payload and map SDK errors/EOF correctly. These routines never run in processing or UI painting. The codec uses fixed arrays and performs no project or file I/O of its own.

`PluginState` is a private-field, validated `Copy` value. It provides a coherent local snapshot, not cross-thread synchronization. Phases 2/3 must implement stopped/active publication and coherent processor/controller recall on SDK-permitted threads, without callback locks or serialization. Define restore-versus-automation ordering and verify bounded handoff/queue-full behavior in those adapter tests. This phase does not claim those future lifecycle gates pass.

## Compatibility fixtures and checks

Synthetic [fixtures](../../crates/drive-plugin/tests/fixtures) freeze identity, state bytes, Color Off retained settings, and plain/normalized/text conversions. Integration tests cover invalid manifests, endpoints, enum thresholds, curve monotonicity, low finite gains, default parity, bypass, instance isolation, state validation/atomic failure, truncated/oversized streams, short writes/reads, interruptions, and I/O failures. No private audio or host projects are used.

Run `cargo test -p daw-drive-plugin -p daw-unit-core --offline --locked` with cached dependencies. `cargo build -p daw-drive-plugin --offline --locked` builds this shared library contract, not a `.clap`/`.vst3` artifact. Native format validation remains Phase 2/3; custom plugin editor validation remains Phase 4. See [validation history](../validation/README.md) for executed workspace/native checks and limits.
