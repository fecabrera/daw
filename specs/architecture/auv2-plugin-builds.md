# Native AUv2 effect exports

## Scope and ownership

All 20 manifest-registered effect products have independent AUv2 export targets.
AUv2 is an export format. It adds no third-party hosting to the DAW.
`crates/plugin-auv2` owns locally generated Apple C declarations, buffer views,
callback containment, the small Cocoa bridge, and independent C ABI probes.
`plugins/drive-auv2/src` is concrete shared adapter source, compiled with each
product's existing plugin/editor dependency aliases. Portable DSP, typed
parameters, state envelopes and shared editor bodies remain their owners.
CLAP/VST3 identities, numeric parameter IDs and serialized state versions do not
change. Manifest schema 2 adds `[auv2]` metadata, with checked four-byte codes,
bundle ID, Cocoa factory class, exported factory symbol and component version.
The manufacturer is `SieD`, the type is `aufx`; per-product subtypes are explicit
in each manifest. These codes are project identities, not a claim of external
registration or trademark clearance.

The initial target is `aarch64-apple-darwin`. The component declares macOS 12.0
as its minimum. Native acceptance currently covers the development machine only;
minimum-OS, Intel, Rosetta and universal builds are not established by compilation.
No external `auv2-sys`, Coupler or CLAP bridge is used.

## Local binding generation

The build uses bindgen 0.72.1 with the exact root symbol inventory in
`crates/plugin-auv2/symbols.txt`. Transitive C dependencies retain their generated
layout, unions, integer discriminants and pointer qualifiers. The wrapper includes
only the public AudioToolbox C interface. It does not generate AppKit or C++ APIs.
Generated Rust, SDK headers and preprocessed SDK text stay outside the repository
and source/plugin archives. SDK headers remain Apple's material under the terms
that accompany the locally obtained developer tools. This does not apply a
separate C++ SDK license to the system SDK.

`approved.json` records the approved SDK/libclang/CLTools tuple, all consumed
header hashes, include-list and symbol-list hashes, and generated declaration
hash. Current tuple: SDK 27.2, CLTools 27.2.0.0.1792721180, Apple clang/libclang
21.0.0 (clang-2100.4.1.1), Rust selected by `rust-toolchain.toml`, deployment 12.0.
The C compiler and loaded libclang must identify the same version. Builds reject
an unapproved inventory. `DAW_AUV2_VERIFY_SDK=1` permits an explicit candidate ABI
verification build; it never updates approval. Packaging rejects verification
candidates. Approval requires review of header/symbol/layout changes and native
checks before replacing the checked inventory.

Every required inventory symbol must occur in the generated output; an unmatched
allowlist entry fails rather than silently omitting an API.

`OUT_DIR/provenance.json` contains hashes and tool identities, with no machine
paths or SDK source. Package `build.json` includes its approved summary. The native
C probe independently measures layout/offsets/constants and invokes a generated
selector type. Rust also invokes an independently compiled C render callback.
C trailing-one arrays are accessed through checked raw byte offsets and copied
headers, never out-of-bounds Rust array references.

Windows/Linux exports are inert libraries. Their workspace builds do not generate
Apple bindings or link Apple frameworks. Building a macOS CLAP/VST3 package alone
does not depend on this crate. No AU dependency enters DAW/CLI/DSP ownership.

## Native lifecycle and supported audio

The stable instance starts with `AudioComponentPlugInInterface`. Open associates
the host handle; Close retires prepared DSP and worker owners on control. Lookup
uses one typed-to-erased selector conversion, checked against generated aliases.
Unknown selectors return null. Reserved is null.

Initialize prepares the existing typed processor, planar input/output storage,
event capacity, ramps and meters before publication. Uninitialize retires them.
Reset clears DSP history and timestamp caching without replacing the processor.
Format, rate, maximum-frame and allocation changes require uninitialization.
Input callbacks and connection descriptors can change after initialization at a
quiescent render boundary, without replacing DSP; the completed-block cache is
invalidated. Input and output allocation flags are independent. With input
allocation disabled, compatible output planes provide storage for the input pull. Invalid configurations retain the
previous configuration. One fail-fast atomic token protects topology and DSP;
overlapping or recursive calls return CannotDoInCurrentContext without waiting.
Host callbacks cannot obtain a second mutable processor reference.

Supported audio is one stereo input/output bus, native noninterleaved f32, at
44.1/48/88.2/96/176.4/192 kHz. The native AU boundary negotiates 1–65536 maximum
frames, default 4096. This is an AU-specific bound for prepared pull/cache storage;
it does not narrow existing CLAP/VST3 setup bounds. Zero/odd/variable blocks are
accepted within the negotiated maximum. Mono, extra buses, sidechain, interleaved,
f64 and integer PCM are rejected. Stereo channel-layout metadata agrees with the
stream format.

Input comes from the registered pull callback or an upstream AudioUnit connection.
One replaces the other on a serialized control call; the host owns the upstream lifetime.
Header copies validate channel count, sizes, nulls and overlapping stereo planes.
Permitted null output planes use prepared unit-owned storage. Input failures return
the upstream status and silence validated output. Quiet blocks still run DSP for
tails/history. Repeated completed requests with the same valid sample timestamp
and frame count return cached output, without advancing DSP/clocks/meters twice.
Reset and time discontinuities invalidate/reconstruct history.

## Parameters, automation and state

AU values use each typed parameter's plain f32 domain. Endpoint conversion clamps
only f32 rounding at the advertised boundary. State keeps the original f64/product
envelope. AU indexed/boolean values are rounded to valid discrete values at the boundary;
the typed product contract retains strict integer validation. Continuous values advertise
ramping. Metadata, value strings and string conversion reuse typed mappings.

Zero-offset SetParameter publishes an atomic requested scalar with a fail-fast
control token. Nonzero offsets use the same scheduled-event path as
ScheduleParameters. Each call preflights its entire batch before appending. The
prepared limit is 65536 event records. Invalid IDs/scopes/values, discrete ramps,
capacity overflow and contention return errors with no partial acceptance.
Signed ramp starts support a ramp that began before this buffer. Equal-offset
order follows call order. Sorting allocates no scratch. Per-sample interpolation
uses the declared plain domain, followed by existing portable smoothing.

Scheduled events apply only to the current render. The host resubmits continuing
ramps in subsequent buffers. A zero-frame render applies offset-zero events.
Pre-render notification callbacks can schedule the current block through separate
bounded storage. Recursive render fails immediately. Only active ramps are
visited per sample; large idle parameter banks do not add a full bank scan for
every sample.

Requested atomics, serialized control recall, and audio publication are separate.
Existing StateExchange remains single-writer on each side; Snapshot is not made
multi-writer. Recall applies before timed events. Audio consumes once and never
waits. Product ClassInfo stores the unchanged binary envelope in CFData with AU
property-list metadata. Invalid/foreign/future/truncated envelopes retain prior
state. Default factory preset recalls the typed default. Persistent strip order
uses the existing product envelope; appearance, visibility, meters, temporary
Solo, Hold and Reset stay instance-only.

Supported properties cover format/rate/channel layout, buses/names, allocation,
input callback/connection, parameter metadata/text, ClassInfo/presets, host timing,
bypass, latency/tail, maximum frames, quality/offline indication, CocoaUI and last
render error. Host timing fields are independently nullable. Missing position is
not an invented stopped state. Existing rate-aware latency/tail helpers remain
authoritative, including Loudness worker ownership. MIDI, legacy Component Manager,
Carbon, FastDispatch, complex/multiple render and AUv3 are not advertised.

## Editor and telemetry

Each binary statically declares a unique AUCocoaUIBase factory and fresh NSView containers.
CocoaUI returns owned URL/class references. Main-thread containers retain themselves through callbacks, so a host can close a
view during a notification without retiring its Rust context during that call.
A fail-fast host-call lease prevents native instance retirement during view
notifications; reentrant gesture ends wait for a safe control point.
Main-thread containers reuse
`plugin-window` and each existing editor. Timers use a non-owning proxy and retire
before the child window and parent view. A view retains a shared inert model after
audio Close; it cannot dereference a retired instance. Headless preparation does
not create a GPU/window. Begin/Value/End messages notify AU host listeners;
presentation edits preserve the existing state envelope. Meters and band audition
use the existing atomic handles and lifecycle. Offline rendering suppresses Solo;
real-time recording can hear it, as in other formats.

Objective-C classes and Rust panic hooks can outlive instances. The boundary pins
its image before publishing an instance or installing a hook. Static classes
remain discoverable through the component NSBundle; runtime-created classes
cannot satisfy that lookup. Image code/immutable class metadata stays
loaded until process exit; processors, analysis workers and windows still retire
with their owners. Callback-scoped quiet panic handling is installed off render,
forwards unrelated panics, supports nested callbacks, catches unwinds and retains
exceptional payloads. A render panic silences validated output, latches a fault
and requires uninitialize/initialize. Unexpected panics can allocate at their
origin; this is containment, not a hard-real-time guarantee for arbitrary failure.
Normal and expected-error render/schedule/reset paths allocate, free, lock, log,
wait and perform file I/O zero times.

## Release linker output

The pinned Rust compiler can implicitly strip Mach-O debug data and misalign the
LINKEDIT string pool; macOS 27 dyld rejects affected dylibs even when codesign
verification passes. The release profile explicitly sets `strip = "none"` to
preserve the linker's output. This also keeps existing CLAP/VST3 release exports
loadable on the newer macOS worker. It changes no DSP settings or optimization.
See the [upstream Rust defect](https://github.com/rust-lang/rust/issues/157750).
The packaged-export checker rejects misaligned string tables independently of
the current host loader and then performs actual dlopen/AudioComponent tests.

## Packaging and CI

`package_plugins.py --api` accepts `clap,vst3,auv2,all`. Omission/all means all
three on macOS and CLAP/VST3 on Windows/Linux. Explicit AU on another platform
fails before build/output creation, including `all,auv2`. The source build names
selected declared packages. AU assembly produces `.component`, full
AudioComponents plist identities/factory/version, notices, relocated binary,
resource signing and verification before hashing and ZIP. No unverified
sandboxSafe claim is added. Debug adds `-debug` before extensions and shares
identities with release; the profiles replace each other when installed.

Checksum namespaces are relative to the platform's complete API set. All selected
products stage before publication. A journal restores overwritten bytes and
removes newly published files on a later publication failure; unselected outputs
remain unchanged. This is operation-level rollback, not a crash/power-loss atomic
transaction.

macOS workflow workers use `xcode-27` and select the approved CLTools installation.
The [published runner inventory](https://github.com/actions/runner-images/blob/main/images/macos/xcode-27-arm64-Readme.md)
lists CLTools 27.2.0.0.1792721180 and SDK 27.2. Actual generated hashes still gate
builds; a runner label alone is not provenance approval. All 20 products retain
Windows/Linux CLAP/VST3 gates and gain macOS AU debug/release ABI tests and preview
uploads. Registry tests catch missing metadata/targets/workflow entries.

See [acceptance gates](../acceptance/auv2-plugin-builds.md) and
[executed checks](../validation/README.md).
