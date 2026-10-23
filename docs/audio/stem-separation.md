# Stem separation

Split an audio clip into Drums, Bass, Other, and Vocals. Separation uses local HTDemucs model files. The app does not include or download model weights. Use model files that you have permission to use.

## Set up models

1. Open Settings and find **Stem Separation**.
2. On Apple Silicon macOS, choose a **CoreML model**, a **CPU model**, or both. Windows and Linux use a **CPU model**.
3. Wait for validation to finish. Separation becomes available when at least one supported model passes.

The CPU model must be Charon's four-stem, in-graph HTDemucs ONNX model. The CoreML model must be the split-transform HTDemucs ONNX export for Charon. A `.mlmodel` or `.mlpackage` is not supported.

Selection loads the model and runs a short stereo test. A failed check rejects the new path and retains a different, previously accepted model. If the failed path was already selected, it is cleared. Cancel also retains the previous model and waits for the current native operation to return. Saved paths are checked again when the app starts; failed checks remove the invalid selection. Model validation does not change the project.

With both models validated, macOS uses CoreML first and keeps CPU as fallback. A CoreML model works without a separate CPU model. If CoreML fails and no CPU model is selected, separation reports the error and leaves the arrangement unchanged. CoreML can use both the CPU and GPU. Stem separation is unavailable on Intel macOS; the rest of the app remains available, and separation controls are disabled.

Model loading needs substantial memory. CoreML can take tens of seconds to compile on first use. Later loads can also take time. Compiled CoreML files use a separate user cache and can occupy more than 1 GB. Model files stay where you selected them.

## Separate a clip

1. In Timeline, right-click an audio clip and choose **Stem-separation**.
2. Keep the status dialog open while the job runs. It shows progress, errors, and warnings. Choose **Cancel** in the dialog or status bar to stop the job.
3. Close the finished report.

The status shows **Preparing audio and loading model…** before separation progress starts. Windows and Linux load the CPU model. On Apple Silicon macOS, a selected CoreML model can also need time to compile.

Separation uses the clicked clip's trim, repeats, stretch, and transpose. It excludes track, group, and Master processing. Each stem keeps the clip gain. The source file stays unchanged.

A new group appears at the end of the track list with one audio track per stem. All four clips start at the original clip's position and have its duration. The original clip stays in place, disabled and shown with the same neutral header and muted foreground colors as disabled units. Right-click it and choose **Enable clip** to hear it again. Disable or mute the stems first if you do not want both versions to play.

You can separate clips before saving the project. Unsaved projects use temporary storage. The completed status dialog shows the stem folder. The first save copies the stems into `stems/<clip name>/` inside the project folder. Saved projects write new stems there directly. An existing folder is retained; a number is added to the new folder name. Long folder names are shortened to fit the filesystem limit. Save the project to retain the new arrangement. Save As copies generated stems to the new project folder.

Temporary stems remain available to the project and Undo/Redo until saved or discarded. Closing an unsaved project discards them. Temporary storage is not crash recovery; save the project to keep the stems.

Undo restores the arrangement. It retains generated files so Redo can restore the stems. After Save As copies the stems, Undo/Redo uses those copies, including when you save with the separation undone and then redo it. Failure or cancellation leaves the arrangement unchanged and removes unpublished stem files. Cancellation waits for the current model operation to return; it is not immediate during model loading or inference.

If you redo separation after saving it undone, use Save to copy any restored stems into the current project folder. Later saves and Undo/Redo use those copies. Edits made while Save runs remain marked as unsaved.

Separation can leave bleed or artifacts. It processes one clip at a time and needs memory for model processing, the arranged input, and prepared stem audio. Long clips can use substantial memory. When a validated CPU model is also selected, a CoreML error triggers a fresh CPU attempt and appears as a warning in the finished report. If CPU processing also fails, the report shows the error.
