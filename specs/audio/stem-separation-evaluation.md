# Stem separation backend evaluation

This is the pre-integration benchmark record. The current workflow is specified in [Stem separation](stem-separation.md).

Date: 2026-10-21. Status: research; no separation feature or dependency is added to the application.

## Decision

Evaluate the current Rust HTDemucs implementation against Charon's Rust/ONNX pipeline before selecting an application backend. Library language alone does not establish separation quality, latency, or distribution rights. The model artifact, numeric precision, windowing, execution provider, and startup state are part of each result.

**The pretrained-model distribution question remains unresolved for both backends.** Do not treat a runtime's permissive license or a converted model's metadata as permission from the original weights' rights holder. Obtain explicit terms for the selected artifact before choosing a bundled model or automatic download service.

**Provisional choice: Charon for a four-stem prototype, with GPU-enabled CoreML on validated Macs and a separate low-memory CPU path, subject to model permission and broader device validation.** The tested Charon CPU and CoreML paths returned valid audio on every local workload. The [CoreML follow-up](stem-separation-coreml-evaluation.md) measures about 4× lower warm end-to-end time than the original CPU baseline, with matching separation scores, but substantial startup costs. Current demucs-rs Metal produced comparable short-clip scores when finite, but intermittently returned non-finite audio with a successful exit code. Do not ship that tested configuration without resolving and reproducing the failure. This is a host/configuration result, not a finding that every Burn backend fails.

## Local results

### Separation quality and correctness

Median whole-signal SDR across five reference excerpts, in dB:

| Configuration | Drums | Bass | Other | Vocals | Five short quality runs |
| --- | ---: | ---: | ---: | ---: | --- |
| demucs-rs Metal | 8.32 | 10.14 | 5.43 | 7.74 | 5/5 finite; exact 300,032-frame stereo output |
| Charon CPU | 8.25 | 10.20 | 5.16 | 7.68 | 5/5 finite; exact 300,032-frame stereo output |
| Charon CoreML follow-up | 8.25 | 10.20 | 5.16 | 7.68 | 5/5 finite; exact 300,032-frame stereo output |

Both separate useful signal from these mixtures. Differences in these five medians are too small and the sample too narrow to select a perceptual-quality winner. The converted artifacts and preprocessing are not numerically identical. No blind listening or full-corpus PyTorch waveform-parity run was performed locally. The CoreML export later passed a checked single-window PyTorch comparison; its same-export CPU control and GPU measurements are recorded in the follow-up.

Reliability is a separate result:

- The first-ever demucs-rs Metal invocation on excerpt 1 produced non-finite samples throughout all four stems, despite exit code 0. A subsequent process on the same file was finite. The five finite rows above do not erase that failure.
- All three Metal runs on the 68.034-second concatenation returned exit code 0 but had non-finite samples at 77.39%, 63.06% and 57.32% of sample positions **in each stem**. The proportion varied across identical invocations. The evaluator did not sanitize or score invalid output.
- All three Charon CPU runs on that concatenation were finite and retained exactly 3,000,320 stereo frames. Their stem-sum residual RMS was about 0.00452; exact reconstruction of the mixture is not established by separation or required by the model.
- One second of digital silence stayed exactly silent, finite, stereo and 44,100 frames on both tested paths.
- A 326,566-frame 48 kHz input returned 326,567 frames at 48 kHz from demucs-rs; this Metal probe also produced non-finite output. Charon returned finite audio at its 44.1 kHz model rate (300,033 frames), rather than the project rate. Both require an explicit host conversion/length policy; these different contracts are not interchangeable.
- The demucs-rs CPU control (Burn NdArray, CLI `--features cpu`) was finite for one short excerpt and the long concatenation, with exact 44.1 kHz frame counts. Its short-excerpt SDR matched the valid Metal result within 0.001 dB per stem. This narrows the failure to the tested Metal/configuration path; the kernel, driver, fusion/autotune setting or other root cause was not isolated. CPU also returned one extra frame at 48 kHz, while remaining finite.

### Speed and memory

Serial local runs, after downloads and builds. GiB means bytes divided by 2³⁰.

| Workload/configuration | Process wall time | Peak RSS | Peak memory footprint | Output validity |
| --- | --- | --- | --- | --- |
| First-ever Metal setup + excerpt 1 | 30.25 s | 0.47 GiB | 2.69 GiB | Invalid; all samples non-finite |
| Five short excerpts, demucs-rs Metal, separate processes | Median 2.31 s; range 1.82–3.06 s | See per-run evidence | See per-run evidence | All five finite |
| Five short excerpts, Charon CPU, separate processes | Median 4.25 s; range 4.14–4.33 s | See per-run evidence | See per-run evidence | All five finite |
| 68.034 s, demucs-rs Metal, three fresh processes | 5.73 / 5.59 / 5.71 s | 0.574–0.578 GiB | 2.819–2.886 GiB | Invalid in all three runs |
| 68.034 s, Charon CPU, three fresh processes | 11.17 / 11.09 / 11.18 s | 1.767–1.786 GiB | 1.685–1.704 GiB | Valid in all three runs |
| 68.034 s, demucs-rs CPU control, one process | 152.02 s | 1.475 GiB | 1.426 GiB | Valid |
| 68.034 s, Charon CoreML follow-up, three fresh processes with warm caches | 2.92 / 2.82 / 2.81 s | 1.150–1.154 GiB | 1.627–1.635 GiB | Valid in all three runs |

The valid Charon CPU runs process this artificial workload at about 6.1 times real-time speed, including startup and file output. The single demucs-rs CPU control takes about 2.23 times the audio duration and about 13.6 times Charon’s median wall time. That is a local configuration comparison, not a CPU-backend tuning study; repeat it before setting a performance guarantee. Its short control took 11.41 s; the separate 48 kHz control took 13.03 s. The smaller Metal RSS does **not** establish lower total memory use: its measured physical footprint was larger. RSS and memory footprint are different OS accounting measures and are not interchangeable. The failed long Metal runs cannot support a successful-throughput advantage. No inference ran in the app's audio callback.

Machine-readable baseline measurements are recorded in [local evidence](../validation/stem-separation-2026-10-21.json), unchanged by the [CoreML follow-up](stem-separation-coreml-evaluation.md). The follow-up separates initial compilation, variable fresh-process startup, warm one-shot runs, and resident-model timings; its warm numbers are not first-use latency. Raw logs and WAVs remain in ignored scratch storage. Short correctness probes that overlapped the later CPU build are excluded from timing comparisons.

## Pinned inputs and method

| Input | Revision or configuration |
| --- | --- |
| DAW | `57fc4ac445f94435100fdf0882f4700fae8f8b50` |
| demucs-rs | `805d6f72ede20282832ac78c634e8767716c0b21`; `demucs-core` 0.1.0; Burn 0.20.1; locked wgpu 26.0.1 |
| Charon | `bc045cd80f009ea863772c5afef15928101df66f`; charon-audio 0.1.2; locked ort 2.0.0-rc.13 |
| Host | Apple M4 Pro, 14 CPU cores, 48 GB unified memory; macOS 27.0.1; Rust 1.97.0 |
| demucs-rs path | Native release CLI, Metal, standard four-stem HTDemucs; f16-stored weights; default fusion/autotune |
| Charon baseline path | Release `separate` example, CPU, public in-graph HTDemucs export; f32 weights; low-memory preset; shifts disabled |
| Charon follow-up path | CoreML-compatible split export; forced CoreML and same-export CPU control; see the follow-up for exact model/tool hashes and presets |
| Audio | Five test excerpts from the [SiSEC18-MUS seven-second preview archive](https://zenodo.org/records/3270814), decoded once with FFmpeg to stereo float32 WAV at 44.1 kHz |

The excerpts are the first five test files in archive order, selected before seeing separation results: Cristina Vane — So Easy; Louis Cressy Band — Good Time; Secretariat — Borderline; James Elder & Mark M Thompson — The English Actor; Georgia Wonder — Siren. Each decoded stream has 300,032 frames (about 6.803 s). A chunking workload concatenates these five mixtures twice, in that order: 3,000,320 frames, about 68.034 s. Its joins are artificial; it is not a full-song listening test.

The preview stems are AAC-derived references. The dataset restricts use; audio and outputs remain in ignored scratch storage and are not distributed with this report. This is a small engineering sample, not a representative genre benchmark, training experiment, or published MUSDB score.

Measure each backend on identical decoded samples. Use whole-signal SDR, `10 log10(sum(reference²) / sum((estimate-reference)²))`, over both channels, without gain fitting, alignment correction, or distortion filters. Higher is better. Skip a silent reference stem rather than manufacture a score. This metric measures reference-stem error, not agreement between two implementations. It is not BSSEval v4 or a perceptual listening score. Check output length, channel count and finite samples separately; a fast run with invalid audio is not a successful separation.

Timing includes process startup, model loading, decoding, separation and float-WAV writing. Downloads, Rust compilation and metric calculation are excluded. Record the first GPU setup separately. Later processes can reuse OS/file/kernel caches but do not retain a loaded model. CPU versus Metal and f32 versus f16-stored artifacts are different configurations; these results cannot establish which implementation is intrinsically faster. Peak process RSS is not total system memory or a complete GPU allocation measurement. No playback was driven by the evaluator; contention with live DAW playback and UI rendering still needs a dedicated check.

## Library and application fit

| Area | demucs-rs | Charon | Application impact |
| --- | --- | --- | --- |
| Runtime | Rust model implementation on Burn | Rust pipeline, native ONNX Runtime inference | Both can avoid Python at runtime; Charon adds a native inference library to packaging |
| Models | Four-stem, six-stem and fine-tuned HTDemucs variants | Explicit HTDemucs four-stem presets; TIGER music-removal path is a different task | Start any comparison with the same four-stem task; six-stem and fine-tuned quality are not established here |
| CPU/GPU | Generic Burn backend; CLI has a CPU feature and Metal/Vulkan configuration | CPU and an explicit CoreML feature/path | CPU and GPU-enabled CoreML were exercised on macOS; Windows/Linux and other providers remain untested |
| PCM boundary | `Demucs::from_bytes`, `separate_with_listener`, planar channel slices | `Separator::separate_with`, `AudioBuffer`, or `AudioSource`/`StemSink` streaming | Reuse the DAW's decoded PCM; do not adopt either CLI's import/UI/output workflow |
| Progress/cancel | Listener events; cancellation checked after long-input chunks | Shared `Control` and cancel token at model-window boundaries | Neither can interrupt every native/GPU operation; always check cancellation again before publication |
| Long audio | Allocates full-length per-stem stereo accumulators plus overlap weights | Has both full-buffer and bounded-window streaming APIs | Prefer streaming to staged files for long clips; verify the actual sink does not collect the full result in RAM |
| Sample rates | Converts to 44.1 kHz and back; rounding can change frame count | Model-rate output; streaming requires model-rate input | Let shared media conversion own the project-rate boundary and enforce the exact requested output duration |
| Downloads/cache | Separate provider/loader exists; CLI uses the system model/kernel cache | Model path supplied; compiled CoreML cache is keyed by file hash | Own approved model URLs, hashes, cache budget and lifecycle in the app; do not enable implicit downloads during playback |
| Dependency fit | Its enabled GPU path uses wgpu 26; desktop uses wgpu 30 | ort is exactly pinned to a release candidate; native binaries must match its API | Rust-native does not mean shared GPU resources or a smaller bundle; measure package size and device contention |

Use `demucs-core`, not the upstream plugin or complete workspace, for a Rust integration. Its backend features are opt-in; a CPU-only core consumer need not enable wgpu. The measured CLI enables more features and is not a package-size estimate for that consumer.

The demucs-rs cancellation check is in the long-input loop. The short-input fast path does not consult `is_cancelled`; a cancelled short job can finish inference. A host-side publication guard must discard its result. Charon's normal buffer API clones/resamples the full input; select its streaming API deliberately when memory bounds matter. These are source-inspection results, not measured cancellation-latency or streaming-memory claims.

Sources: [demucs-rs core](https://github.com/nikhilunni/demucs-rs/blob/805d6f72ede20282832ac78c634e8767716c0b21/demucs-core/src/lib.rs), [CLI backend configuration](https://github.com/nikhilunni/demucs-rs/blob/805d6f72ede20282832ac78c634e8767716c0b21/demucs-cli/Cargo.toml), [Charon separator](https://github.com/Valkyra-Labs/charon-audio/blob/bc045cd80f009ea863772c5afef15928101df66f/src/separator.rs), [Charon inference options](https://github.com/Valkyra-Labs/charon-audio/blob/bc045cd80f009ea863772c5afef15928101df66f/src/models.rs).

## Published evidence, separate from local measurements

Charon's authors publish a comparison on 50 MUSDB18 test previews. They report median whole-signal SDR of 9.50/9.04/5.19/8.88 dB for drums/bass/other/vocals, matching their PyTorch reference. Their demucs-rs comparison uses **older revision `5d9f61a`**, with 9.33/8.94/5.13/7.48 dB and non-finite output on six tracks. This is a competing project's report, not independent confirmation or a verdict on the current revision.

For a 193-second track on a different M4 Pro configuration, the same report lists Charon CoreML resident jobs at 6.15–6.39 s with 1.5–1.6 GB server RSS, and a one-shot CoreML process at 14.08 s/2.6 GB. Its older demucs-rs Metal run was 13.70 s/0.88 GB but had invalid samples. Invalid-output performance must not be ranked as a working result. Model exports, hardware, and startup state differ from the local measurements.

These claims identify useful validation targets. They do not replace a controlled current-version comparison. The optimized Charon split-transform CPU/CoreML exports are not hosted by Charon; its instructions require a separate Python/PyTorch export step. The CoreML-target export was subsequently generated and benchmarked in the [local follow-up](stem-separation-coreml-evaluation.md); the dedicated CPU-target split export remains unmeasured. The published figures above remain upstream evidence, separate from the follow-up results.

Source: [Charon measurement record at the evaluated revision](https://github.com/Valkyra-Labs/charon-audio/blob/bc045cd80f009ea863772c5afef15928101df66f/docs/MEASUREMENTS.md).

## Code and model licensing

| Artifact | Observed terms | Decision for this app |
| --- | --- | --- |
| [demucs-rs library](https://github.com/nikhilunni/demucs-rs/blob/805d6f72ede20282832ac78c634e8767716c0b21/LICENSE) | Apache-2.0 repository license | Retain license/notices and identify modifications when distributing it; inspect the selected transitive graph before release |
| [Charon library](https://github.com/Valkyra-Labs/charon-audio/blob/bc045cd80f009ea863772c5afef15928101df66f/Cargo.toml) | MIT OR Apache-2.0 | Select and retain the applicable license; audit the ONNX Runtime binary and its notices for each target |
| Original Demucs code | MIT | Code permission does not settle pretrained-weight permission |
| Official HTDemucs weights | Official model card has no license field; a maintainer's 2022 answer says weights are provided for scientific purposes | Exact HTDemucs artifact use and redistribution remain unresolved; that answer predates HTDemucs v4 and is not a complete artifact-specific grant |
| demucs-rs converted weights | `set-soft/audio_separation` model card labels the collection MIT and says its sources claimed MIT | A converter's label does not resolve the original rights-holder conflict |
| Downloadable ONNX weights | `StemSplitio/htdemucs-onnx` labels its conversion MIT | Apply the same unresolved-origin rule; conversion does not itself supply missing rights |
| MUSDB evaluation audio | Educational/non-commercial conditions; individual recordings have their own rights | Keep test data and stems out of tracked fixtures and release packages |

The 84,030,696-byte demucs-rs artifact contains f16 tensors. The 316,446,953-byte ONNX artifact contains the downloadable in-graph model. Download size is not peak RAM. Converting precision or format does not resolve license provenance. A bring-your-own-model flow also does not establish permitted use by itself. This evaluation records the conflict; it does not make a legal determination or claim that every future compatible model has the same restrictions.

Primary evidence: [Demucs code license](https://github.com/facebookresearch/demucs/blob/main/LICENSE), [maintainer answer, issue 327](https://github.com/facebookresearch/demucs/issues/327), [official HTDemucs card](https://huggingface.co/adefossez/HTDemucs/blob/main/README.md), [converted safetensors card](https://huggingface.co/set-soft/audio_separation/blob/main/README.md), [converted ONNX card](https://huggingface.co/StemSplitio/htdemucs-onnx/blob/main/README.md), [dataset terms](https://sigsep.github.io/datasets/musdb.html). Charon also records this conflict in its [model inventory](https://github.com/Valkyra-Labs/charon-audio/blob/bc045cd80f009ea863772c5afef15928101df66f/docs/MODELS.md).

## Proposed integration boundary

This is a proposal for review, not an implemented contract.

1. Keep inference in a portable separation crate, with backend-specific construction behind a small PCM/progress/cancellation boundary. Keep project IDs, dialogs, file placement and audio devices outside it. A backend trait is appropriate because the implementations vary; the rest can use structs/functions.
2. Let `media` own decoding, sample-rate conversion and peaks. Reuse prepared clip/source mapping for a selected range. Decide input semantics before implementation: raw recording versus rendered clip content, including trim/loop/stretch/transpose and clip gain. Avoid silently separating track effects or normalizing each stem independently.
3. Use the existing UI worker/job path and project orchestration. Stage generated audio in app-owned durable storage, validate finite stereo samples and exact output frames, prepare peaks/render data, then publish all new tracks/assets/clips as one history transaction. Retain the original and publish nothing on cancellation/failure. `Session::import_decoded` imports one asset; it needs a shared group transaction for multi-stem publication rather than sequential visible imports.
4. Keep generated stems durable across save/reopen, Save As, undo and redo. Existing source references and seek/stretch caches are not a generated-asset retention policy. Do not delete a file while any live, pending or history session references it.
5. Cache separation results by source-content identity, selected range/input mapping, model hash, backend/revision, precision and processing settings. Keep that cache separate from exact-history seek checkpoints. Budget disk audio, resident models and inference memory independently. Publish through a stale-job/revision check.
6. Start with one separation job at a time. Bound CPU threads, use bounded streaming where available, and unload idle models under memory pressure. Compare playback underruns and UI responsiveness with cache preparation active; two background inference sessions can multiply activation memory and contend with audio work.
7. Reuse existing track groups and import placement for results once the transaction exists. No inference, model load, download, file write or final-owner retirement belongs on the audio callback.

Existing anchors: [worker audio preparation](../../crates/ui/src/audio_edits.rs), [job publication](../../crates/ui/src/lib.rs), [session import](../../crates/project/src/lib.rs), [shared audio buffers/conversion](../../crates/media/src/lib.rs), [history](../editing/undo-redo.md), [performance and exact-history caches](../architecture/performance.md).

## Release gates and remaining work

- Resolve exact model use/redistribution terms and preserve runtime/model provenance and notices. Do not add the downloaded artifacts to the app on the strength of third-party metadata.
- Test the selected backend/provider/precision on a broader independent set: bass-heavy material, dense mixes, vocals, drums, quiet/silent input, mono and asymmetric stereo, very short clips, long clips and all project rates. Include finite-output, exact-length, stereo and seam checks. Run a blind listening comparison; SDR alone does not predict audible artifacts.
- Broaden the completed CoreML startup, fresh-process, same-export CPU and reused-model measurements to more devices/material. Benchmark the dedicated CPU-target split export and repeat demucs-rs CPU across the full test set. Retain model hashes, provider verification, repeated timings, RSS and physical footprint; add live-memory and hardware-level GPU traces where needed.
- Verify progress, cancellation before/during/after inference, failed allocation/model load, corrupt weights, disk exhaustion, device loss and atomic cleanup/publication. Avoid treating CLI success as validated audio.
- Verify offline operation after approved model installation, signed/notarized native-runtime packaging, and CPU fallback on supported Windows/Linux/macOS architectures. No such platform release clearance is established here.

## Reproduction and artifacts

All upstream trees, models, public audio, generated WAVs and raw logs are under ignored `work/stem-evaluation/`. They are local evidence, not required repository inputs. `results/provenance.json` records the initial baseline revisions, model SHA-256/byte counts and its two scratch-only changes: an empty `[workspace]` table isolates Charon from the parent DAW workspace; a `STEM_EVAL_CACHE` override keeps demucs-rs model reads in the evaluator's model directory. Each chosen archive member was checked against its ZIP CRC32 and hashed; the incomplete full-archive download was stopped after the subset was available. No full-archive MD5 verification is claimed. No inference math was patched. The GPU CLI was copied before rebuilding with `--features cpu`, so each recorded run uses the intended backend. demucs-rs retains its normal CubeCL autotune cache policy.

Models (downloaded only for this local evaluation):

- [Converted safetensors](https://huggingface.co/set-soft/audio_separation/resolve/main/Demucs/htdemucs.safetensors), `htdemucs.safetensors`: SHA-256 `8193504cdfb3943adaf039b8acb524a46e87ebf232c383ac7a32c80a6578423e`.
- [In-graph ONNX export](https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/main/htdemucs.onnx), `htdemucs.onnx`: SHA-256 `68d0bf16428ef66e692cdff8a9ccf28f1ef3f69440d57e58605a4cc55fcc5e74`.

Build from the pinned upstream copies using the DAW toolchain and separate Cargo targets/cache:

```sh
cargo build --manifest-path work/stem-evaluation/demucs-rs/Cargo.toml --locked --release -p demucs-cli
cargo build --manifest-path work/stem-evaluation/charon-audio/Cargo.toml --locked --release --example separate
# Save the Metal binary separately before the CPU control build.
cargo build --manifest-path work/stem-evaluation/demucs-rs/Cargo.toml --locked --release -p demucs-cli --features cpu
```

Use the same decoded 44.1 kHz stereo float WAV for both invocations. Point `STEM_EVAL_CACHE` at the scratch model folder for the patched evaluator CLI. Run each command serially with `/usr/bin/time -l`, preserving stdout and stderr:

```sh
demucs mixture.wav --model htdemucs --output demucs-out
separate mixture.wav charon-out htdemucs.onnx --format wav32 --shifts 0 --ep cpu
```

The scratch `evaluate.py` records command exit, process wall time, RSS, frame count, non-finite counts, per-stem SDR, mixture baseline and sum-residual RMS in per-run JSON. It uses NumPy and FFmpeg only for metrics, outside the timed separation process. Raw outputs remain available for a later listening review. The initial sandboxed Charon smoke run could not read `/usr/bin/time` system counters; it is excluded from measured memory results.

The [CoreML follow-up](stem-separation-coreml-evaluation.md#export-and-reproducibility) records its additional observability-only scratch changes, export tools/model hash, provider audit and reproduction commands.
