# External VST3 and CLAP hosting

Status: active hosting is integrated in the desktop, project model and shared
renderer. Both formats support effects and polyphonic layer instruments, mapped
controls, native editors, shared modulation and separate PCM sidechain inputs.
See the [integration validation record](../validation/external-plugin-hosting-integration-2026-11-17.md)
for executed checks and native platform limits. This is an in-process host.

## Required delivery

The user requested the recommended in-process hosting architecture on
2026-11-17. This extends the earlier built-in-only runtime scope in
[modular audio units](modular-audio-units.md). Existing standalone plugin export
products remain separate. AU hosting and cross-architecture bridging are outside
this request.

| ID | Required behavior | Current implementation |
| --- | --- | --- |
| H01 | VST3 and CLAP effects and instruments on macOS, Windows and Linux. | Native lifecycle adapters and shared block graph; one instance per effect or polyphonic source. Owned fixtures exercise both formats. |
| H02 | Automatic default-location discovery, cache and quarantine. | App Settings > Plugins, shared machine-local additional folders, desktop startup refresh, retry/rescan/cancel, native aliases/shortcuts and embedded isolated scanner. |
| H03 | Existing unit controls and stable parameter targets. | Dynamic native metadata, plain CLAP/normalized VST3 domains, native text formatting, steps, read-only/hidden flags, search/pagination and UUID targets. Parameter reordering uses native IDs. |
| H04 | Native editors synchronized with mapped controls. | Cocoa/HWND/X11 parents, CLAP GUI and VST3 view attachment, main-thread pumping, focus/resize/scale, gestures and worker-safe retirement. |
| H05 | Shared parameter modulation and audio-derived providers. | Typed hosted destinations use the common resolver, scope validation, additive smoothing and telemetry. CLAP native deltas or absolute values; VST3 normalized queues. |
| H06 | Actual cross-track PCM auxiliary inputs. | Independent saved routes, pre/post taps, channel selection/gain, shared DAG/cycle checks, stage-aware delay and zero disconnected inputs. |
| H07 | Sources, live/offline/CLI, transport, state and missing data. | Exclusive layer source, block graph, independent export/seek/bounce instances, immutable state blobs/history receipts, strict and explicit degraded CLI policy. |
| H08 | Bounded callback and proper-thread ownership. | Prepared buffers/events, exclusive processing lease, no foreign cloning, callback heap guards, atomic base/effective separation and broker-owned retirement. |
| H09 | Acceptance and distribution. | Owned native fixture checks and CI commands, macOS native editor check, Windows/Linux cross-checks, helper packaging/signing and dependency notices. Native device/listening and full Windows/Linux editor acceptance remain open. |

## Implemented ownership

- `unit-core/hosted.rs` and `hosted_settings.rs` own identities, value domains,
  saved settings, stable scalar targets, state references and PCM route metadata.
- `core` owns inventory, source exclusivity, transactions, copy validation and the
  combined source-first routing DAG. Disabled PCM edges still reject cycles.
- `plugin-host` owns discovery, isolated scanning, native module/instance/control
  lifetimes, SDK boundaries, state streams, event queues and main-thread brokerage.
- `engine` owns block scheduling around the existing built-in sample kernels,
  shared modulation, stage-aligned audio/control inputs, transport and graph handoff.
- `project` owns immutable state banks, save/open/export and revision-checked
  history receipts. State is content-addressed under `assets/plugin-state`.
- `ui` uses existing unit shells, controls, selection and modulation strips.
  External effects and instruments live in VST3/CLAP folders in their respective
  catalogs. App Settings owns scan folders and management.
  `apps/desktop/hosting` owns native editor parents and services SDK callbacks.
- `apps/plugin-scan` enumerates descriptors in a disposable process. `apps/cli`
  exposes catalog management and device-free rendering through the shared engine.

Native factory interfaces are reused through the existing narrow binding crates.
VST3 module entry receives the actual CFBundleRef on macOS and the shared-object
handle on Linux. COM factory references are released before module exit/unload.
CLAP entry init/deinit calls are paired, including descriptor-inspection errors.
Unknown role metadata remains Unknown; do not guess that every class is an effect.

## Discovery and failure policy

Default roots follow [CLAP entry documentation](https://github.com/free-audio/clap/blob/main/include/clap/entry.h)
and [VST3 locations](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Locations%2BFormat/Plugin%2BLocations.html).
The rendered VST3 table includes Linux lib64 roots in addition to lib roots.

| Platform | VST3 priority | CLAP priority |
| --- | --- | --- |
| macOS | User Library, system Library, Network Library, application Contents/VST3. | User Library, system Library, then CLAP_PATH. |
| Windows | User Programs/Common/VST3, native Common Files/VST3, application VST3. | User Programs/Common/CLAP, native Common Files/CLAP, then CLAP_PATH. |
| Linux | ~/.vst3, /usr/lib64/vst3, /usr/lib/vst3, /usr/local/lib64/vst3, /usr/local/lib/vst3, application vst3. | ~/.clap, /usr/lib/clap, then CLAP_PATH. |

Windows uses Known Folder APIs. CLAP_PATH uses the platform path-list separator;
empty elements do not search the current directory. CLAP user-first duplicate
precedence is host policy. VST3 first processor-UID precedence follows its format.
Shadowed and missing entries retain metadata. A changed module fingerprint retries
automatically; an unchanged failure requires explicit retry.

Traversal stops at plugin bundles. Canonical paths prevent symlink cycles.
Resource hashing includes bundle file names, types, lengths and content; links
outside a resolved bundle are rejected. Native ELF/PE/Mach-O headers are checked
before a child loads a candidate. No architecture bridge is implied.

| Resource | Current bound |
| --- | --- |
| Directory traversal | 100,000 entries and 64 levels. |
| Bundle fingerprint | 512 MiB, 100,000 resolved entries and 64 levels. |
| Classes in one module | 4,096. |
| Descriptor strings | 4,096 bytes; CLAP feature lists have at most 128 entries. |
| Scan response | 8 MiB. |
| Registry | 32 MiB and 16,384 module records. |
| CLI scan timeout | 10 seconds per candidate. |

The registry journals Scanning immediately before each foreign initializer.
Pending modules remain Pending. Loading a journal after interruption quarantines
only attempted modules. Native crashes and timeouts fail one candidate; other
candidates continue. Cancellation does not mark a module as defective. Registry
files publish by temporary-file replacement. Corrupt caches produce a diagnostic
and can be rebuilt. These are crash-recovery controls, not a security sandbox.

The CLI scans synchronously and has no editor or audio-device requirement.
The desktop and CLI launch their own executable with an internal scanner argument.
The child dispatches inspection on its main thread before UI, graphics, device or
application startup. No separate scanner installation is required. The optional
`daw-plugin-scan` executable remains available for custom hosts and diagnostics.

## Processing and state contracts

### Processor ownership and seeks

Native handles remain in the main-thread broker. A unique processing lease is
Send, never Clone or Sync. Dropping the lease returns ownership; the broker
retires the instance, editor, COM objects and module. Preparation, export, seeks
and bounce each obtain independent instances. Native graph checkpoints and
parallel warming are disabled. Exact seeks replay on a cancellable worker.
Fast seeks prepare fresh processors at the destination.

### Fast edits during playback

In Fast Seek mode, a single hosted effect insertion during playback can
install at the next callback boundary. The worker prepares the candidate and
checks the edit against the active plan. The callback requires the original
processing publication and compatible source, routing and native input state.
It transfers held voices, existing effect DSP, native leases, parameter
smoothers, delay lines and already evaluated audio. The worker uses measured
plug-in latency to prepare the new compensation lines. After draining the old
output, the block renderer feeds any added frames from the next live input
clock while it holds the audible position. This fill is split into ordinary
processing quanta. Only the inserted effect starts fresh. A stale candidate
or incompatible route follows normal preparation. Exact mode uses normal
reconstruction for hosted effect additions.

Fast duplication can insert multiple copies in one publication, including
mixed native and built-in groups. The copies recall the saved plug-in
configuration under new unit IDs. The callback retains the original native
leases and their queued output and starts the copies with fresh signal history.
Native copies still require recallable plug-in state. Exact retains normal
preparation.

Fast removal waits for the end of the current evaluated block. It transfers
the surviving leases, note identities, effect state and compatible input
history. If the last hosted effect leaves, the sample renderer resumes from
the same source cursor. When compensation shrinks, the audible clock moves
ahead by the decrease. Exact uses normal reconstruction for removals.

Fast moves and reorders retain native leases by unit ID across chain owners.
The callback finishes the already evaluated block before applying new
compensation. Existing auxiliary audio and modulation delay histories must
fit the new routing. Other changes use normal worker preparation.

### Audio buses and modulation routes

Native processors use bounded stereo/mono buses, one output bus, up to 32 input
buses and at most one note input. Before setup/activation, VST3 negotiation
requests mono for existing mono buses and stereo for other buses. This follows the
[VST3 bus arrangement sequence](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Workflow%2BDiagrams/Bus%2BArrangement%2BSetting%2BSequence.html).
Re-enumerate actual buses after negotiation, including a rejected request; size
buffers only from those validated results. A plugin may use a supported fallback.
Layouts that remain unsupported fail during preparation with direction, index,
name and channel count. Controller base synchronization skips unchanged values;
processor base restoration still uses the normal input parameter queue.
Every missing/unconnected auxiliary input is zeroed. VST3 input keys include a
layout signature; incompatible ports stay disconnected. Native latency is measured
at the project rate before preparing graph delays. Bypass retains that latency
and uses a 5 ms wet/dry ramp. Native source bypass produces silence.

The callback publishes providers once per sample and uses the common modulation
resolver. Ordinary shared controls are delayed to the destination input clock;
audio-derived controls use the shared follower's existing stage alignment.
Parameters and notes share a bounded event budget; large parameter sets reduce
the graph quantum. Equal-offset events retain insertion order. VST3 MIDI mapping
covers advertised controllers, pitch bend and pressure. Editor/output telemetry
is separate from authored manual bases and never creates modulation feedback.

### State publication and recovery

CLAP state is opaque. VST3 component and controller state remain separate.
Each stream is bounded to 16 MiB and verified by hash/length. Exact captures
suspend at a processing boundary, remove transient modulation, restore manual
bases, flush controls, capture state, then resume. Receipts apply only to the
same settings and revision, including matching Undo/Redo owners. Save and export
wait for those receipts. History owns immutable blobs; snapshots prune unreferenced
owners. Save As publishes retained state before the manifest. Corrupt files and
symlinks below the state folder fail without replacing the current session.

Plugins without a state extension reopen at their defaults. Their saved mapped
values are diagnostic, not a replacement state format. Exact copying is rejected
until a current opaque capture exists. Unknown future payloads round-trip without
reinterpretation. Missing plugins retain settings, targets, ports and state.
Desktop missing units remain visible and use delayed dry effects or silent sources.
Strict CLI rendering fails for missing/quarantined plugins and unsupported saved versions. Only
`--allow-missing-plugins` enables delayed dry effects and silent instruments, with
a warning for each unit. Processing/state failures of available plugins never
silently enter degraded rendering.

### Editor transactions and restart

Native editor and mapped parameter gestures share project edits and history groups.
Opaque state changes request an exact capture. Restart/latency notifications
suspend the old instance and prepare a new one after capture. Parameter reordering
retains targets by ID. An incompatible parameter definition/layout reports an
error rather than guessing a new mapping. Reload opens the last captured state
in a fresh instance. A processing fault ramps to dry/silence and prevents publishing
an offline export.

## Native acceptance and limits

Run the [acceptance matrix](../acceptance/external-plugin-hosting.md) for each
shipped OS and architecture. Cross-compilation checks types and platform branches;
it does not prove native editor behavior, device deadlines or vendor compatibility.
Linux editors use negotiated X11/XWayland. Unsupported native window APIs report
an error. Actual Windows/Linux editor operation, installed-app loading policy,
listening and hardware callback deadlines require native release checks.

The host runs active plugins in process. A vendor crash or hang can terminate or
block the application. Scan isolation is crash recovery, not active-DSP isolation
or a security sandbox. AU hosting and cross-architecture bridging remain outside
this feature. No third-party plugin is redistributed. Binding provenance and the
full distribution obligation graph require their existing release review.

## Controller recall and mapped control layout — 2026-11-17

VST3 processor state restore remains mandatory when a saved state exists.
Controller component-state synchronization accepts `kNotImplemented`; other
controller errors remain failures. Each restore call receives its own stream
reader. This handles controllers that share processor state or retain the SDK's
[default unimplemented synchronization method](https://github.com/steinbergmedia/vst3_public_sdk/blob/master/source/vst/vsteditcontroller.cpp).
Native activation failures include the module path and actual format so failures
in another installed instance are identifiable during full renderer preparation.

Mapped controls reuse shared 90-point strip surfaces, three 52-point named
selector rows and the semibold strip footer. They use zero extra horizontal gap
between strip slots. Partial parameter groups reserve their empty rows. Sidechain
source, tap/channel choices, gain and route power follow the same geometry.
Long selector names truncate and retain their full hover text.

### Header actions

Regular-size hosted units place two momentary icon buttons below the header's
Power control. Lucide Maximize2 opens the editor and RefreshCcw reloads captured
state. Both use the active modulation dot surface and centered 8-point artwork.
Use a 7-point gap between adjacent dots to match the visible inset from the
header top to Power. Give each button a 22-by-17-point hit area. The shared
shell reserves their slots above the
vertical unit title and excludes them from collapse and drag handling. They
remain available when the regular-size unit is collapsed and follow availability,
UI enable and focus guards. Compact lane presentation keeps its existing hidden
editor/reload controls. Search and page controls remain in the body.
