# Stem separation

## Accepted behavior

Timeline's audio-clip context menu contains **Stem-separation**. It acts on the clicked clip, independently of a multi-clip selection. Require a saved project and a configured local CPU model. Do not bundle, fetch, or redistribute weights. Store machine-local model paths in application Settings, outside the project manifest.

The arranged clip is the input: resolve the shared prepared buffer for the saved stretch engine/revision and transpose, then map trim and repeat coordinates through `Clip::source_frame`. Apply the shared loop-seam fade. Exclude outer fades from the input because normal stem playback supplies them. Do not bake clip gain, track/group/Master mix settings, effects, or modulation. Copy clip gain to each resulting stem.

Publish a new processing group and four ordinary audio tracks, in Drums/Bass/Other/Vocals order, at the end of the track list. Start each stem at the original clip start and retain its exact project-frame duration. Retain the original clip's ID and all settings; set only its saved `muted` field. Use one history publication. Generated tracks use normal track mix defaults and shared group processing.

## Stages and ownership

| Stage | Owner | Contract |
| --- | --- | --- |
| Menu and configuration | `daw-ui` | Existing file-action/job/dialog guards; local ONNX pickers; no inference in painting |
| Snapshot and scheduling | Existing UI `run_job` | Clone Session through shared immutable audio owners; one offline job; keep playback/session on failure |
| Input preparation | `daw-project` | Shared clip-buffer lookup and frame mapping; shared loop fade; cancellable shared resampling to stereo 44.1 kHz |
| Model and separation | `daw-separation` | Pinned Charon revision; private backend types; stream source and output callbacks; finite stereo PCM |
| Encoding | Shared `daw-media::WavEncoder` | Float32 WAV; four fixed safe names; ordered blocks; validate exact model-rate output length |
| Project-rate preparation | `daw-project` + media | Decode/resample through shared media code; permit at most two rounding frames, pad/truncate to exact duration; encode project-rate Float32 and prepare peaks |
| Import and transaction | `Session::import_decoded`, core edits | New stable IDs, one new track per stem; `GroupTracks` and `SetClipMuted`; validate before publication |
| Publication | UI worker polling | Recheck cancellation, retain current navigation, replace session once and call shared history/render synchronization |
| Status | Shared `job_report` | Same dialog, progress, diagnostics, and completion rules as export; cancel requests an atomic flag |

The separation crate has no UI/device dependency. Dependencies flow project → separation → media. Decoding, model construction, inference, WAV I/O, peaks, and resampling stay on the existing worker. Nothing is added to the audio callback.

Charon runs windows in its existing serial stream pipeline. The adapter retains only one output block per write, not four whole model-rate output arrays. The arranged input and resampled input still need memory; imported project-rate buffers and peaks use the normal shared media path. There is no streaming disk-backed project audio or fixed job-memory guarantee.

## Backends

Pin Charon to `bc045cd80f009ea863772c5afef15928101df66f` (0.1.2 source version) in Cargo and retain the lockfile. Default features are disabled; enable the ONNX backend and enable CoreML only on macOS. The default HTDemucs CPU preset uses its in-graph model and low-memory configuration. The optional macOS preset uses the split-transform ONNX model with CoreML CPUAndGPU compute units. CoreML is an execution provider, not a promise that every operation runs on a GPU. ONNX Runtime can assign unsupported nodes to CPU.

On a CoreML load, inference, or output-validation error, discard its writers and retry using the separate CPU model and fresh files. Retain a warning. Do not retry a cancellation. Both paths validate stem identities/order, channel count, rate, finite samples, and exact model-rate duration. Whole-process termination or native out-of-memory abort cannot be recovered in-process.

Compiled CoreML data lives under the user cache directory's `DAW/Charon/CoreML/ort-1.28` subtree, keyed by Charon's model-content hash. It is separate from project stems, model locations, and seek caches. Models are loaded per job; a resident model service is not implemented. Cold load can dominate elapsed time. See the [CPU evaluation](stem-separation-evaluation.md) and [CoreML evaluation](stem-separation-coreml-evaluation.md); their timing evidence is not a feature-level latency guarantee.

The shared report progress bar fills the available dialog content width. Retain the dialog's existing left and right padding for separation and export reports.

## Files, failure, and persistence

Reserve a portable `stems/<clip name>/` directory with `create_dir`, adding a numeric suffix on collision. Never overwrite a pre-existing output directory or source file. Only the job's reserved directory can be removed on failure. An unpublished `PreparedSeparation` owns that directory and removes it on drop, including when the worker receiver disappears. Successful UI publication transfers file ownership to the session. Undo retains those files for Redo; no orphan-file cleaner is introduced. History restoration retargets generated sources by stable asset ID to the latest saved stem locations, so Save As followed by Undo/Redo retains the copied files.

Generated assets use `Source.kind = "generated"` and project-relative paths. Core validation accepts them under the normal audio-source contract. Save As copies generated assets into reserved directories in the destination project's `stems` tree before publishing the manifest; failed saves remove only newly reserved copies. External recordings remain referenced through existing save behavior. Existing files are never replaced by generated copies. Missing generated sources fail Save As without replacing the manifest.

`Clip.muted` defaults to false for legacy manifests. `SetClipMuted` is a core transaction, undoable and shared by the context-menu Enable/Disable action. Audio random-access and scheduled rendering skip disabled clips. MIDI scheduling excludes them from events and LFO note history. Mute enters source/history fingerprints so renderer/cache inheritance cannot retain stale active schedules. Split pieces preserve the flag. Shared clip painting desaturates the resolved clip body toward grey, retains a small hint of its inherited or overridden color, and adjusts it against the theme panel. Shared palette construction derives opaque header/channel fills and readable waveform, note, and text colors from that subdued body. It does not reduce painter opacity or dim the selection outline.

Cancellation checks input preparation, resampling, streaming reads/writes, model-window callbacks, post-inference preparation, and UI publication. Model construction and a native inference call are not interruptible. Cancel leaves the report visible until the worker acknowledges it; failed/cancelled reports require Close. The shared UI modal guards block edits and project replacement while the job/report owns the operation.

## Licenses and validation

Models are user-provided. No rights to distribute pretrained weights are assumed. Keep Charon, ONNX Runtime, and new Rust dependency notices in `crates/separation/assets/licenses`; all three platform packaging paths include them. The runtime binaries are build dependencies, distinct from model weights.

Automated regressions use a deterministic injected separation function to test arrangement/import ownership without requiring models. An ignored opt-in project test exercises real local models and WAV input with `DAW_STEM_CPU_MODEL`, optional `DAW_STEM_COREML_MODEL`, and `DAW_STEM_INPUT`. Never commit weights or test recordings. Executed checks belong in the [validation record](../validation/README.md).
