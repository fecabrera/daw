# Plugin development tools

For current product builds and packaging, see [README: Plugins](../../README.md#plugins). This reference retains Drive-specific contract, validator, feasibility, and native-editor commands. The validator results below are recorded phase baselines, not new validation runs. These tools do not install plugins. See the [plugin architecture](unit-plugin-builds.md) for requirements and the [validation history](../validation/README.md) for executed checks.

## Shared Drive contract

The original Drive metadata, parameter, and state contract can be built and tested without a plugin SDK or audio device:

```sh
cargo build -p daw-drive-plugin --locked
cargo test -p daw-drive-plugin --locked
```

These commands build only the shared contract. See the [Phase 1 contract](drive-plugin-phase1.md) for its scope.

## Drive CLAP processor checks

A separate CLAP development target shares built-in Drive DSP and its two-strip editor. This is not a release package. Native execution has been checked on macOS arm64 only. See [development controls and limits](../../docs/effects/drive-plugin.md).

Build the library with the pinned Rust toolchain:

```sh
cargo build -p daw-drive-clap --release --locked
```

Run the development artifact checks with Python 3.10 or later and clap-validator **0.4.1**. The [Phase 0 tools](#feasibility-tools) build the pinned validator, or build that validator directly from revision `152b9823e992d782c5c1fd33bca0295478b919aa` of the official [clap-validator repository](https://github.com/free-audio/clap-validator). It needs only its normal Rust/native platform build prerequisites; no VST3 SDK is required for this target.

```sh
python3 tools/check_drive_clap.py --validator work/drive-plugin-phase0/clap-validator/target/release/clap-validator
```

The checker uses its own target directory and places the native development artifact at `work/drive-plugin-phase2/Drive.clap`. It tests both an instrumented library and the normal artifact, then measures callback budgets. Cached Cargo dependencies are used by default; add `--fetch` for missing locked downloads. No plugin is installed. On macOS the output is a bundle; Windows/Linux development layout is supported by the tool but native checks remain open.

The Phase 2 validator record reports **32 success, 11 skipped, 1 failed**: its rate sweep requires unsupported 8 kHz activation. The complete result is retained. A successful checker exit means the scoped checks completed with that known out-of-contract failure, not an unconditional validator pass. `--strict-validator` exits 2 for any validator failure. Missing/partial evidence and any other validator failure fail the checker. See [Phase 2 evidence and remaining gates](drive-plugin-phase2.md).

## Drive VST3 processor checks

The separate VST3 development target shares the same Drive DSP, parameters and base settings as CLAP. It provides a processor, edit controller, and shared two-strip editor.

```sh
cargo build -p daw-drive-vst3 --release --locked
python3 tools/check_drive_vst3.py --validator work/drive-plugin-phase0/vst3-build/bin/Release/validator
```

Use Python 3.10 or later and the pinned Steinberg SDK validator reporting **3.8.1**. The [Phase 0 tools](#feasibility-tools) build it from SDK revision `3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96` and its committed submodules. Building the Rust plugin itself uses the locked Rust bindings and does not require a local SDK/C++ installation. Building the validator requires the native CMake/Ninja/compiler setup described in the feasibility section; full Xcode is needed for that SDK build on macOS.

The checker builds in isolated scratch space and writes `work/drive-plugin-phase3/Drive.vst3`, without installing it. It checks instrumented and normal loaded binaries, CPU budgets, build separation and the full validator summary. Cached dependencies are used by default; `--fetch` permits missing locked downloads. Native execution has been checked on macOS arm64 only; Windows/Linux bundle layouts exist in the tool but their native acceptance remains open.

The Phase 3 SDK validator record reports **47 passed, 0 failed**. It reports unsupported-rate diagnostics while accepting clean refusal of rates outside the six-rate policy. This does not resolve the CLAP validator's different rate-sweep expectation. Full release acceptance still requires native accessibility, external-host/platform checks, and production packaging. See [Phase 3 implementation and evidence](drive-plugin-phase3.md).

## Feasibility tools

The isolated [Phase 0 experiment](drive-plugin-phase0.md) currently runs on macOS arm64. It requires the pinned Rust toolchain, Python 3.10 or later, CMake 3.25 or later, Ninja, and full Xcode with its macOS SDK. The runner uses `DEVELOPER_DIR` when set, or `/Applications/Xcode.app/Contents/Developer` when present; it does not change the system's selected developer directory.

```sh
python3 tools/probe_drive_plugins.py --fetch --gui
```

`--fetch` downloads exact pinned sources and locked dependencies. Omit it for a cached run. `--gui` briefly opens temporary native probe windows; omit it for headless checks. Results and private test bundles go under `work/drive-plugin-phase0/`. These are disposable gain probes, not Drive products; the command does not install plugins or open user projects. A completed diagnostic run can reject the candidate framework. Read `candidate_gates` in `report.json`; `--require-framework-contract` returns a failing exit status for unmet production gates.

## Native editor checks

The macOS native editor harness uses the pinned SDK checkouts prepared by the feasibility tools. It builds both development targets and opens temporary host windows while processing. It does not install plugins:

```sh
python3 tools/check_drive_editor.py
```

The harness needs full Xcode and a graphical login session. Results go under `work/drive-plugin-phase4/`. See [Phase 4 editor implementation](drive-plugin-phase4.md) for the checks and remaining platform/accessibility limits.
