# Drive plugin Phase 4: shared editor

Date: 2026-10-26. Status: the shared editor is implemented in CLAP and VST3 development targets. Native macOS control/window checks pass. Phase 4 acceptance remains open for native screen-reader exposure, Windows/X11 checks, and the remaining display/host matrix. This is not a release claim.

Related: [accepted plugin plan](unit-plugin-builds.md), [Phase 3 processor](drive-plugin-phase3.md), [Drive](../effects/drive.md), and [user guide](../../docs/effects/drive-plugin.md).

## Shared presentation

`crates/unit-ui` owns the existing Knob/KnobSpec implementation, its interaction tests, portable parameter/amount definitions required by those tests, context-local theme primitives, font configuration/assets, compact row/dot helpers, and the Drive body. It depends only on egui and unit-core. The DAW modules reexport the shared controls. DAW project colors, preferences, shells, routing, selection and history remain in `daw-ui`.

Drive uses the original two 90-point strips, 10-point margins, 52-point knob rows, 8-point spacing, 18-point footer row, and center divider. Its fixed body is 180 × 218 logical points. The DAW passes existing modulation decorations and compact height; the plugin passes no modulation decorations and always shows the full body. There is no plugin power button, DAW header, routing strip, selection border, collapse or chain drag.

The shared base retains gain silence/log scales, choice diagrams, cyclic choice behavior, Shift sensitivity, keyboard steps, double-click reset, tooltips and egui widget metadata. Off disables Strength/Frequency input while retaining their values. Automation still updates these values. The existing DAW Drive appearance/input tests remain active after extraction.

`crates/drive-editor` binds the six controls to typed plugin parameters. It retains full-precision host values for parameters that the user did not edit; merely drawing an f32 control must not quantize a saved f64 value. It emits Begin/Value/End batches. Discrete, reset and keyboard edits use complete gestures; drags end on release, focus loss, or editor destruction.

The background menu selects Dark, Light, or Mist with the shared plugin default accent, Deep Orange (`#FF5722`). Preferences belong to an instance, survive editor reopen, and are separate from automatable parameters. They are not serialized yet. The background interaction is registered before the knobs so it cannot steal their clicks. No new inline text editor is added; existing host text conversion remains available in generic host controls.

## Native boundary and threading

`crates/plugin-window` is a thin egui-baseview 0.7.2/baseview 0.3.7 OpenGL child-window adapter. Native parents use raw-window-handle 0.6. Cocoa uses NSView logical points and native backing scale; Win32 and X11 use physical host dimensions. The editor compensates the backend zoom against native display scale and keeps its fixed body size. Content scale changes use format contracts rather than free resizing.

Window construction, destruction, host/controller callbacks and final GUI owners stay on the host UI thread. On X11 the backend's window thread signals CLAP `request_callback`; the host's main callback pumps backend work. VST3 uses the host frame's Linux `IRunLoop` timer to drain edits and pump main-thread work. A Linux host without that run loop is refused cleanly. Those paths need native platform validation.

GUI construction occurs only in CLAP set_parent or VST3 attached. Descriptor discovery, processor creation, VST3 createView/getSize, headless playback and state operations do not create graphics. Host parent pointers stay valid until child retirement. Window retirement synchronously removes the editor callback before its Rust owners are released. Rust UI objects remain inside their creating library; only SDK/native pointers cross the binary boundary.

The editor/control channel is a bounded queue protected by a mutex. Only UI/control threads access it. Each accepted batch is atomic; the editor reserves capacity for active gesture ends and disables input under backpressure. The audio callback never accesses this mutex or an egui/window object.

### CLAP

The main thread transfers editor messages to a preallocated 512-event SPSC queue (one producer and one consumer), requests params.flush, and publishes current audio/base state back to the editor. Published sequence acknowledgements prevent stale audio state from replacing pending UI values. The audio thread acknowledges events only after publishing their matching state snapshot. Other host parameters can continue updating while a UI value is pending. Process/flush drains the queue into CLAP live global parameter/gesture events at frame zero. Host output rejection retains the exact pending event and retries it before any later event. Bounded audio-thread notifications schedule control draining and output retries even after the editor has closed; gesture completion must not depend on an open window continuing to repaint. A value reaches DSP only after its event is accepted; no full-queue path can lose or reorder End. Existing host input events retain their original timing.

The GUI extension advertises only the current platform's embedded API. Floating windows and free resizing are unsupported. Create, parent, show/hide and destroy have independent lifecycles. Cocoa ignores explicit scaling because it uses logical points; other platforms accept bounded finite scaling.

### VST3

The controller retains the host component handler through COM ownership. Its view retains controller UI data independently of the controller object's lifetime; only one attached view is allowed per controller. Main-thread UI events call beginEdit, performEdit with the shared normalized mapping, and endEdit. The host remains responsible for sending processor parameter queues. Host setParamNormalized and state synchronization update the UI without echoing edits. Host callbacks use a cloned handler outside any RefCell borrow to permit synchronous reentry.

IPlugView owns attachment/removal, fixed geometry, frame references, and scale support. Unsupported platform types, null parents and duplicate attachments fail cleanly. A view can be queried without opening graphics. Removing/dropping it retires the native child and balances active gestures.

## Dependencies and notices

The production targets do not use nice-plug. The selected egui-baseview release requires its tracing feature to compile its error-reporting path. Baseview pins calloop exactly to 0.14.4, so the workspace's compatible 0.14 branch moves from 0.14.5 to 0.14.4. This is a required backend constraint, not an unrelated upgrade. Other existing package versions remain fixed.

Window/backend notices are in [plugin-window notices](../../crates/plugin-window/assets/licenses). Outfit and its notices moved to [unit-ui fonts](../../crates/unit-ui/assets/fonts). Additional egui fallback-font notices are in [unit-ui notices](../../crates/unit-ui/assets/licenses). Desktop packaging follows the new shared asset paths; development plugin bundles copy editor/font notices. A complete distribution dependency/notice audit remains a Phase 5 gate.

## Validation

The executable check record and remaining limits are in [validation history](../validation/README.md). `python3 tools/check_drive_editor.py` builds a native Cocoa SDK test host from the pinned Phase 0 interface sources, checks all six controls and balanced gestures, and repeats attachment/removal while audio runs. It writes a fresh incomplete report before work and marks it complete only after both formats pass. It requires a graphical login session and full Xcode. It never installs a plugin.

Synthetic egui tests check input, Off retention, focus loss, queue pressure and editor-close gestures. CLAP tests reject output events one at a time to check exact retry ordering. Existing exported processor parity/allocation/state/rate tests remain required. Synthetic widget metadata is not proof of native screen-reader support: egui-baseview currently exposes only the native window in the inspected accessibility tree. A native accessibility adapter or another backend solution is still required before advertising accessible plugin editors.
