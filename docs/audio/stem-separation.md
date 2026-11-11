# Stem separation

Split an audio clip into four stems (Drums, Bass, Other, and Vocals) or six stems (adding Guitar and Piano). Separation uses local HTDemucs model files. The app does not include or download model weights. Use model files that you have permission to use.

## Set up models

1. Open Settings and find **Stem Separation**.
2. On Apple Silicon macOS, choose a **CoreML model**, a **CPU model**, or both. Windows and Linux use a **CPU model**.
3. Wait for validation to finish. Separation becomes available when at least one supported model passes.

Select either a four-stem or six-stem model in each existing model picker. The CPU picker accepts waveform HTDemucs ONNX models, including `htdemucs.onnx` and `htdemucs_6s.onnx`. The CoreML picker requires the split-transform HTDemucs ONNX export for Charon. Validation detects whether the model produces four or six stems; there is no separate stem-count setting. A `.mlmodel` or `.mlpackage` is not supported.

Selection loads the model and runs a short stereo test. A failed check rejects the new path and retains a different, previously accepted model. If the failed path was already selected, it is cleared. Cancel also retains the previous model and waits for the current native operation to return. Saved paths are checked again when the app starts; failed checks remove the invalid selection. Model validation does not change the project.

After validation, small text below each selected model shows its stem count, stem names, stereo/44.1 kHz audio format, and file size in MB. File size refers to the selected ONNX file, not runtime memory use. These details come from the successful validation and supported model format; they are checked again at startup. Clearing a model removes its details. A failed or cancelled replacement retains the previous model’s details.

With both models validated, macOS uses CoreML first and keeps CPU as fallback. The CoreML model determines the normal stem count. Clear its selection to run the CPU model directly. A CoreML model works without a separate CPU model. If CoreML fails and no CPU model is selected, separation reports the error and leaves the arrangement unchanged. CoreML can use both the CPU and GPU. Stem separation is unavailable on Intel macOS; the rest of the app remains available, and separation controls are disabled.

Model loading needs substantial memory. CoreML can take tens of seconds to compile on first use. Later loads can also take time. Compiled CoreML files use a separate user cache and can occupy more than 1 GB. Model files stay where you selected them.

## Separate a clip

1. In Timeline, select an audio clip and choose **Clip > Stem separation**, or right-click the clip and choose **Stem separation**.
2. Keep the status dialog open while the job runs. It shows progress, the stem count and names, output audio quality, errors, and warnings. Choose **Cancel** in the dialog or status bar to stop the job.
3. Close the finished report.

The dialog and status bar show the same phase: **Preparing audio and loading model…**, **Separating audio…**, then **Preparing stem tracks…**. Cancellation changes both to **Cancelling…** while the job stops. Windows and Linux load the CPU model. On Apple Silicon macOS, a selected CoreML model can also need time to compile.

Separation uses the clicked clip's trim, repeats, stretch, and transpose. It excludes track, group, and Master processing. Each stem keeps the clip gain and saved fades. Ordinary fades apply once during playback. Saved crossfades stay inactive on the new tracks until an eligible neighboring clip touches the edge; otherwise playback uses the normal boundary fade. The source file stays unchanged.

A new group appears at the end of the track list with one audio track per stem. The selected model determines whether four or six tracks are created. All resulting clips start at the original clip's position and have its duration. The original clip stays in place, disabled with a neutral header and muted foreground colors. Its translucent body shows the grid underneath. Right-click it and choose **Enable clip** to hear it again. Disable or mute the stems first if you do not want both versions to play.

You can separate clips before saving the project. Unsaved projects use temporary storage. The status dialog shows output details instead of a destination folder. The first save copies protected stems into group folders under `stems/` inside the project folder. Saved projects write new separations under `stems/<clip name>/`. An existing folder is retained; a number is added to the new folder name. Long folder names are shortened to fit the filesystem limit. Save the project to retain the new arrangement. Save As copies generated stems to the new project folder.

Temporary stems remain available to the project and Undo/Redo until saved or discarded. Closing an unsaved project discards them. Temporary storage is not crash recovery; save the project to keep the stems.

Undo restores the arrangement. It retains generated files so Redo can restore the stems. After Save As copies the stems, Undo/Redo uses those copies, including when you save with the separation undone and then redo it. Failure or cancellation leaves the arrangement unchanged and removes unpublished stem files. Cancellation waits for the current model operation to return; it is not immediate during model loading or inference.

Save and Save As also copy stems retained only by Undo/Redo. Redo can restore them without access to the old project folder or temporary files. Edits made while Save runs remain marked as unsaved.

Below the status and progress bar, the report shows three lines: the separator and stem count; the stem names; and the output format. For example:

```text
Charon · HTDemucs · 6 stems
Drums, Bass, Other, Vocals, Guitar, Piano
WAV · IEEE float 32-bit · Stereo · 48 kHz
```

The format line uses the project sample rate. It describes the output format, not a guarantee of separation quality. The finished report uses the stem count and names from the model that succeeded.

Large audio files use temporary disk storage and bounded playback buffers. The combined stems can exceed 2 GiB without loading every sample into memory. Keep enough free space on the system temporary disk and the project disk. Playback can pause briefly while a disk read catches up; it retains its position. A disk read failure stops playback and reports an error. Saving retains the stem WAV files; temporary playback data is rebuilt when the project opens.

Separation can leave bleed or artifacts. It processes one clip at a time and still needs memory for model processing and the arranged input. Very long clips can exceed these remaining limits. Stretching and Sampler preparation also retain their memory limits. When a validated CPU model is also selected, a CoreML error triggers a fresh CPU attempt and appears as a warning in the finished report. The finished report lists the stems from the model that succeeded, including when CPU fallback uses a different stem count. If CPU processing also fails, the report shows the error.
