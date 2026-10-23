# Charon CoreML benchmark

This is the pre-integration benchmark record. The current workflow is specified in [Stem separation](stem-separation.md).

Date: 2026-10-21. Follow-up to the [backend evaluation](stem-separation-evaluation.md). This is research on one Mac, not an implemented app feature or release acceptance.

## Result

**Use Charon's GPU-enabled CoreML path for the next macOS prototype, with a separate low-memory CPU configuration.** All measured CoreML outputs were finite and retained the expected stereo frames. Separation scores matched the CPU baseline at two decimal places. Once prepared, CoreML was faster on this workload; its model setup still has a substantial cold-start cost.

The [model-license question](stem-separation-evaluation.md#code-and-model-licensing) remains unresolved. Exporting the checkpoint locally does not grant distribution rights. This follow-up changes the performance evidence, not the licensing decision.

## Scope and verification

The same pinned Charon revision, host and decoded audio are used as in the original evaluation: `bc045cd80f009ea863772c5afef15928101df66f`, Apple M4 Pro with 14 CPU cores and 48 GB unified memory, macOS 27.0.1. All five mixtures and reference stems match the original SHA-256 values. The long input remains their two-pass concatenation: 3,000,320 stereo frames at 44.1 kHz, about 68.034 seconds.

Three configurations are kept separate:

| Name | Model and processing path | Purpose |
| --- | --- | --- |
| Original CPU | Downloadable in-graph HTDemucs; existing low-memory CPU preset | Compare with the recorded app-candidate baseline |
| Same-export CPU | Newly exported CoreML-compatible split graph, forced CPU; shared split preset | Control for export/host-transform changes when comparing providers |
| CoreML | The identical split graph and preset, forced CoreML; CPUAndGPU compute units | Measure the GPU-enabled provider with host STFT/iSTFT in Rust |

The split preset uses Extended graph optimization, memory-pattern planning off, CPU arena on and shifts off. CoreML uses MLProgram and static input shapes. It permits CPU use; it is not a GPU-only execution contract. No automatic whole-session CPU fallback is requested by the evaluator.

A separate ONNX Runtime profile verifies one CoreML partition containing all **1,982 optimized graph nodes**. The exported graph has 2,055 nodes before native optimization. Recorded model-kernel events use `CoreMLExecutionProvider`, with no CPU-provider model-kernel events. This verifies graph assignment, not hardware execution of each internal CoreML operation. Host decoding, FFTs, overlap-add, resampling and writes remain CPU work. No hardware GPU-utilization trace was captured.

## Quality and correctness

Median whole-signal SDR over the same five excerpts, in dB; higher is better:

| Configuration | Drums | Bass | Other | Vocals |
| --- | ---: | ---: | ---: | ---: |
| Original CPU | 8.25 | 10.20 | 5.16 | 7.68 |
| Same-export CPU | 8.25 | 10.20 | 5.16 | 7.68 |
| CoreML | 8.25 | 10.20 | 5.16 | 7.68 |

The largest absolute change in an excerpt/stem's SDR was below **0.0005 dB** against either CPU reference. Direct waveform comparison across all five excerpts and the first long run found a maximum sample difference below **8.56 × 10⁻⁵**, and at least **70.37 dB agreement** in every compared stem/run. Agreement measures numerical similarity to CPU, not separation accuracy. Output is not bit-identical.

Both providers passed all five excerpt runs and three fresh-process long runs. The nine reused-model separations also passed finite-sample, four-stem, stereo and exact-frame assertions outside their timers. No invalid output was removed from this follow-up's tables.

CoreML also retained exact digital silence for the one-second probe. For the 326,566-frame 48 kHz probe it returned finite stereo audio at 44.1 kHz with 300,033 frames, matching Charon's model-rate output contract. The app still needs shared project-rate conversion and explicit final-frame enforcement.

The metric remains the original whole-signal stereo SDR without fitting or alignment correction. Five AAC-derived excerpts and an artificial concatenation do not establish a perceptual winner or general genre performance. No blind listening test was performed.

## Fresh-process speed and memory

Each row includes model loading, decode, separation and four float32 WAV writes. Downloads, export, compilation and NumPy/FFmpeg metric calculation are excluded. Processes run serially. GiB means bytes divided by 2³⁰.

For the 68.034-second input:

| Configuration | Three process times | Median | Peak RSS range | Peak memory footprint range |
| --- | --- | ---: | --- | --- |
| Original CPU, recorded baseline | 11.17 / 11.09 / 11.18 s | 11.17 s | 1.767–1.786 GiB | 1.685–1.704 GiB |
| Same-export CPU | 6.81 / 6.85 / 6.88 s | 6.85 s | 3.460–3.465 GiB | 3.448–3.453 GiB |
| CoreML, warm caches | 2.92 / 2.82 / 2.81 s | 2.82 s | 1.150–1.154 GiB | 1.627–1.635 GiB |

CoreML is about **4.0× faster end to end than the original CPU baseline**, and **2.4× faster than CPU on the same export**. Part of the improvement over the original baseline comes from moving the transforms outside the model, not solely from the execution provider. The same-export CPU graph is a control; it is not Charon's dedicated CPU-target split export, which remains unmeasured.

Peak RSS is lower on the warm CoreML path, but its physical footprint is only slightly lower than the original CPU path. Do not interpret the RSS difference as an equal reduction in total GPU/system memory. The same-export CPU fallback nearly doubles RSS versus the original low-memory CPU configuration.

For the five short excerpts, CoreML process times were **25.97 / 1.08 / 1.11 / 1.19 / 1.15 seconds**, in input order. The median is 1.15 s, but the first outlier is retained. Same-export CPU times were 1.58 / 1.54 / 1.50 / 1.55 / 1.55 s; the recorded original CPU median was 4.25 s. A warm-cache short-clip figure must not be presented as first-use latency.

## Reused-model speed and memory

The same process decodes once, loads once and separates the long input three times through `Separator::separate`. These separation timers exclude decode, model load, WAV writing and the subsequent output checks. They are not directly interchangeable with the fresh-process totals above.

| Configuration | Model load | Three separation times | Median separation | Process peak RSS | Process peak footprint |
| --- | ---: | --- | ---: | ---: | ---: |
| Original CPU | 2.84 s | 8.22 / 8.13 / 8.10 s | 8.13 s | 1.771 GiB | 1.689 GiB |
| Same-export CPU | 0.18 s | 6.46 / 6.29 / 6.21 s | 6.29 s | 3.452 GiB | 3.439 GiB |
| CoreML | 0.85 s | 2.23 / 1.98 / 1.98 s | 1.98 s | 1.149 GiB | 1.624 GiB |

CoreML's median separation is about **4.1× faster than original CPU** and **3.2× faster than same-export CPU**. Model reuse avoids the load cost and can suit repeated clip operations. Peak memory is process-lifetime accounting across load and all three jobs, not a separate live measurement of each job's retained model. Unload policy still needs a live-memory and playback-contention test.

## Startup and disk cache

The first profile used an empty Charon compiled-model cache for the new model hash. Model load/compilation took **25.26 s**, and its first short separation took **5.33 s**. Total process time was 31.04 s without WAV output; peak RSS was 2.837 GiB and peak footprint 2.140 GiB.

The next fresh one-shot process still took 25.97 s despite cached files being present. Later short processes took 1.08–1.19 s. OS-internal caches were not purged or controlled, and the cause of that second slow initialization was not isolated. Cache-file existence alone does not establish warm startup. Do not promise that compilation is paid exactly once or that every subsequent launch loads in 0.85 s.

After testing, `du` reported about **1.67 GiB** for Charon's hash-keyed compiled-model cache, in addition to the 172,152,058-byte ONNX file. This is the local filesystem's reported usage, not a universal cache-size bound. It needs a separate disk budget from audio-result and exact-history seek caches.

## Export and reproducibility

The original CPU [measurement record](../validation/stem-separation-2026-10-21.json) is unchanged. The [CoreML follow-up record](../validation/stem-separation-coreml-2026-10-21.json) contains model/tool versions and hashes, per-run values, waveform comparisons, resident profiles and provider-assignment evidence.

An isolated Python environment runs the pinned Charon export script, with its checks enabled. Versions include Demucs 4.1.0, PyTorch 2.14.0, ONNX 1.23.2, onnxscript 0.7.2 and Python ONNX Runtime 1.31.0. The Rust evaluator uses ort 2.0.0-rc.13 and its downloaded native ONNX Runtime 1.28.0. Python is used for export, not native separation.

The official checkpoint is `adefossez/HTDemucs`, revision `cbc8a9b1a87023b7fd74e7b3412e6321c0eab003`, artifact `955717e8.safetensors`, SHA-256 `d9fa14133cfcc034a6758923bb3a8ca9f8dfd0b582134643bbf83f72c17576dd`. Exported graph SHA-256: `af57ac5ce9e8399415fb754faa11f438e5a6ffee7ec6870309ede64abf01a3c6`; 172,152,058 bytes; opset 18; 2,055 nodes and 757 initializers.

The export's checked single-window maximum differences were 3.129 × 10⁻⁷ for the patched split model versus PyTorch, and 6.551 × 10⁻⁵ for ONNX CPU plus host iSTFT versus PyTorch, both below its 10⁻³ gate. This is an export check, not a full-corpus PyTorch comparison. The file differs from the upstream example hash; the local structure, checks and hash are recorded instead of claiming byte identity.

From the existing scratch checkout and export environment:

```sh
TORCH_HOME="$PWD/work/stem-evaluation/torch-cache" \
HF_HOME="$PWD/work/stem-evaluation/hf-cache" OMP_NUM_THREADS=8 \
work/stem-evaluation/export-venv/bin/python \
  work/stem-evaluation/charon-audio/tools/export/export_htdemucs.py \
  work/stem-evaluation/models/htdemucs_split_coreml_reproduction.onnx --target coreml

CARGO_HOME="$PWD/work/stem-evaluation/cargo-home" \
CARGO_TARGET_DIR="$PWD/work/stem-evaluation/target-charon" \
cargo build --manifest-path work/stem-evaluation/charon-audio/Cargo.toml \
  --locked --release --features coreml --example separate --example profile
```

Build/export precede timed runs. Use isolated model/Cargo/Python caches as in the raw provenance. The export example uses a new output path to preserve the evaluated artifact. Record its hash and use that new path in subsequent separation commands. Model files, compiled caches and research audio remain under ignored `work/stem-evaluation/`; none is included in the repository or a release package.

Fresh-process command, wrapped in `/usr/bin/time -l` for memory and process time:

```sh
work/stem-evaluation/target-charon/release/examples/separate \
  mixture.wav coreml-output htdemucs_split_coreml.onnx \
  --format wav32 --shifts 0 --split --ep coreml
```

The same-export CPU control changes only `--ep coreml` to `--ep cpu`. Resident profiling uses `examples/profile INPUT MODEL split=1 ep=coreml shifts=0 runs=3`; omit `split=1` and use the original model for the original CPU path.

Scratch-only observability changes print the configured provider, add finite/stereo/frame assertions outside resident timers, and enable ORT profiling through `STEM_EVAL_ORT_PROFILE` only for the separate audit. Normal timed runs have profiling disabled. No inference math or app source is changed.

## App implications and limits

- Prepare approved models on a worker with visible load/compile progress. Retain the current session and playback until successful result publication. Use a publication-time cancel/revision check.
- Keep one loaded model for repeated jobs when the memory budget permits. Measure actual live memory before defining an idle eviction policy; these are peak counters.
- Use provider-specific model/configuration selection. The measured same-export CPU fallback costs about 3.46 GiB RSS; the original low-memory CPU path costs about 1.77 GiB. Do not silently assume the GPU export is also the lowest-memory CPU fallback.
- Give converted models, compiled CoreML artifacts, generated stems and seek checkpoints separate budgets and identities. Invalidate compiled artifacts by model hash and relevant runtime/backend version.
- Retain the original worker/publication, duration, finite-output and model-license gates. Broader material, blind listening, more Macs, memory pressure, cancellation/device failure, signed packaging and live playback/UI contention remain unverified.
