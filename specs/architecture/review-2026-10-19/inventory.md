# Package inventory

[Review index](README.md) · Baseline `2dbb347`

Derived from the root workspace and each member Cargo manifest. This table lists normal direct workspace dependencies; third-party libraries and test-only dependencies are separate. Source counts include tests inside `src/`; they exclude integration tests, assets and generated/build output. Counts indicate scope, not complexity or performance.

| Package | Direct workspace dependencies | Rust files in src | Lines including tests |
| --- | --- | ---: | ---: |
| [daw-saturator](../../../crates/saturator/Cargo.toml) | `daw-unit-core` | 3 | 630 |
| [daw-bitcrusher](../../../crates/bitcrusher/Cargo.toml) | `daw-unit-core` | 2 | 515 |
| [daw-stereo-spread](../../../crates/stereo-spread/Cargo.toml) | `daw-unit-core` | 2 | 538 |
| [daw-direction-mixer](../../../crates/direction-mixer/Cargo.toml) | `daw-unit-core` | 2 | 455 |
| [daw-sampler](../../../crates/sampler/Cargo.toml) | `daw-unit-core` | 2 | 390 |
| [daw-macro](../../../crates/macro/Cargo.toml) | `daw-unit-core` | 2 | 219 |
| [daw-lfo](../../../crates/lfo/Cargo.toml) | `daw-unit-core` | 2 | 752 |
| [daw-amplitude](../../../crates/amplitude/Cargo.toml) | `daw-unit-core` | 1 | 150 |
| [timestretch](../../../crates/timestretch/Cargo.toml) | None | 4 | 1,073 |
| [timestretch-cli](../../../crates/timestretch-cli/Cargo.toml) | `timestretch` | 4 | 928 |
| [daw-unit-core](../../../crates/unit-core/Cargo.toml) | None | 27 | 6,236 |
| [daw-oscillator](../../../crates/oscillator/Cargo.toml) | `daw-unit-core` | 1 | 365 |
| [daw-envelope](../../../crates/envelope/Cargo.toml) | `daw-unit-core` | 4 | 851 |
| [daw-equalizer](../../../crates/equalizer/Cargo.toml) | `daw-unit-core` | 4 | 1,262 |
| [daw-drive](../../../crates/drive/Cargo.toml) | `daw-unit-core` | 2 | 575 |
| [daw-auto-filter](../../../crates/auto-filter/Cargo.toml) | `daw-lfo`, `daw-unit-core` | 1 | 707 |
| [daw-compressor](../../../crates/compressor/Cargo.toml) | `daw-unit-core` | 4 | 722 |
| [daw-limiter](../../../crates/limiter/Cargo.toml) | `daw-unit-core` | 5 | 925 |
| [daw-ott](../../../crates/ott/Cargo.toml) | `daw-unit-core` | 4 | 953 |
| [daw-core](../../../crates/core/Cargo.toml) | `daw-unit-core` | 16 | 6,393 |
| [daw-media](../../../crates/media/Cargo.toml) | `daw-core`, `timestretch` | 6 | 1,400 |
| [daw-engine](../../../crates/engine/Cargo.toml) | `daw-amplitude`, `daw-auto-filter`, `daw-bitcrusher`, `daw-compressor`, `daw-core`, `daw-direction-mixer`, `daw-drive`, `daw-envelope`, `daw-equalizer`, `daw-lfo`, `daw-limiter`, `daw-macro`, `daw-media`, `daw-oscillator`, `daw-ott`, `daw-sampler`, `daw-saturator`, `daw-stereo-spread`, `daw-unit-core` | 10 | 4,862 |
| [daw-project](../../../crates/project/Cargo.toml) | `daw-core`, `daw-engine`, `daw-media` | 3 | 1,082 |
| [daw-output](../../../crates/output/Cargo.toml) | `daw-core`, `daw-engine`, `daw-media` | 5 | 2,857 |
| [daw-ui](../../../crates/ui/Cargo.toml) | `daw-core`, `daw-engine`, `daw-media`, `daw-output`, `daw-project`, `daw-unit-core` | 98 | 60,584 |
| [daw-desktop](../../../apps/desktop/Cargo.toml) | `daw-ui` | 7 | 1,238 |
| [daw-cli](../../../apps/cli/Cargo.toml) | `daw-core`, `daw-media`, `daw-project` | 1 | 177 |

Total: 27 packages; 222 Rust files under package `src/` directories.

## External boundaries

| Boundary | Libraries/adapters | Runtime ownership |
| --- | --- | --- |
| Window and GPU | egui/eframe; wgpu on macOS/Linux, Glow on Windows | Desktop/UI; absent from CLI dependency path |
| OS integration | Native menu/titlebar/file-drag adapters; rfd dialogs | Desktop/UI thread rules |
| Device audio | CPAL and Rubato device-rate conversion | Output; no device initialization in Engine/CLI |
| DSP | FunDSP behind several portable units; RustFFT in independent Timestretch | Concrete implementations stay private to units |
| Decode/encode | Symphonia, Hound, Rubato, LAME wrapper | Media/worker or CLI |
| Native stretch | Rubber Band public API wrapper | Media/backend; separately licensed; not in portable Timestretch dependencies |
| Data | Serde/JSON, UUID | Settings, model, persistence |
| Messaging | rtrb, atomics, standard threads/channels | Callback queues versus worker/control channels |

Dependency versions are defined in [Cargo.toml](../../../Cargo.toml), per-package manifests and [Cargo.lock](../../../Cargo.lock); the compiler is pinned in [rust-toolchain.toml](../../../rust-toolchain.toml). This review does not assess external package vulnerabilities or audit third-party implementation source.

## Platform delivery

| Target | Native boundary | Delivery source |
| --- | --- | --- |
| macOS | CoreAudio via CPAL; native menu/titlebar/file dragging; wgpu | [App bundle](../../../tools/package_macos.sh), [DMG](../../../tools/package_macos_dmg.sh) |
| Windows | WASAPI via CPAL; native caption/input/file dragging; Glow | [Executable/package](../../../tools/package_windows.ps1) |
| Linux | ALSA via CPAL; desktop file-drag adapter; wgpu; portal dialogs | [ZIP/package](../../../tools/package_linux.sh) |

The [CI matrix](../../../.github/workflows/build-and-test.yml) invokes the [platform workflow](../../../.github/workflows/platform.yml) independently. Each platform tests before its own packaging job. Build/test logs and dependency/license notices are deliverables. Workflow presence is not evidence that a particular CI run or physical device test passed.
