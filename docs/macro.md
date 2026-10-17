# Macro component

Macro provides several independent shared controls in one Instrument component. Each strip has one Value knob, a circular Power button, and a name label that defaults to Macro N. Its output can control several parameters at the same time. The component also accepts modulation, so an LFO, a shared Envelope, or another Macro can change a strip's value.

## Controls and connections

Add Macro from the Instrument catalog on a regular layer or Main. The first addition creates one strip. Further catalog additions or the component's Plus strip add a macro. There is no fixed strip limit. Each strip has a stable UUID; its default number follows its saved list position. The Power/label pair, 90-point strip width, Plus button, separators, padding, panel height, header power/collapse, and compact layer presentation reuse the existing unit controls. With no visible strips, Plus fills one 90-point column so the header controls remain separate.

Double-click a strip's name label to rename it, including in compact layers and on Main. The editor selects the current name. Enter saves a trimmed, nonempty name; Escape, loss of focus, view/track/project changes, or unavailable controls cancel the draft. Control characters and blank names are rejected. An unchanged name does not mark the project modified. Long names are truncated inside the fixed-width strip, with the full name in its tooltip. The Power button keeps its separate toggle action. Names also appear in modulation source lists and the strip's Value destination label. Duplicate names are allowed; routing continues to use stable UUIDs.

Value spans 0–100%, defaults to 0%, and uses one-percentage-point keyboard steps. Shift, pointer adjustment, reset, and focus behavior come from the shared knob. The header's secondary-color dot opens the existing modulation strips to the right without hiding the value knobs. Assign Source, Destination, signed Amount, and Power there. Destinations are Macro N · Value, identified by UUID rather than strip position. Each Amount spans −100% to +100%; double-click resets it to zero. The same destination definition supplies both the Value knob and its amount controls. Assignment dots, possible-range arcs, click-to-open behavior, and live values use the shared indicators and callback telemetry.

Select a Macro as Source on Oscillator Pitch, Amplitude, Auto-filter, Drive, LFO, or another Macro. Each receiving connection keeps its own parameter, signed amount, and power. For example:

1. Add two Macro strips and set Macro 1 to 25%.
2. Connect Macro 1 to Auto-filter Frequency with Amount +2 octaves, and to Drive Gain with Amount +40%.
3. Open Macro's modulation controls, select an LFO, choose Macro 1 · Value, and set Amount +50%.
4. Use Macro 2 for a second independent group of parameters.

The knob/readout stays at the saved base value while the live dot shows the processed value. Incoming offsets sum with the base value and clamp to 0–100%. A unipolar source moves in one direction; a bipolar LFO moves above and below the base. Output is always unipolar 0–1. At Amplitude, full positive depth makes value zero silence and value one unity. Macro Value is a signal, not a contribution-depth control; power off supplies a neutral contribution through the shared enable weight.

## Shared processing and scope

The portable BSD-2-Clause `daw-macro` crate depends only on `daw-unit-core`. Each strip implements `ModulationSource` and `ModulationDestination`; `MacroInstance` supplies source metadata and `MacroModuleSettings` supplies targeted destination settings. Source behavior is `Continuous`, scope is `Shared`, and both modulation ports are declared. It has no audio input/output, DAW, UI, or device dependency.

The engine prepares strip instances and target-filtered routes outside the callback. LFO and Macro share the host's continuous-provider preparation, dependency ordering, sampling, publication, reset, telemetry, and compatible state-retention path. Every provider advances once per sample; consumers reuse its cached value through `process_modulation`. Macro uses the common 1 ms additive offset smoother and shared 5 ms ramps for manual changes, source power, and connection amount/power. The final value bounds belong to the portable Macro processor. No callback allocation, locks, logging, or I/O are added.

Sources stay inside the same MIDI instrument, including Main. Macro accepts shared Retrigger/Legato Envelopes, LFOs, and other macros. Per note Envelopes cannot feed it. The common graph code evaluates each target separately, orders providers before consumers, and rejects self-connections and feedback cycles. Retained, disabled, and zero-amount connections count toward cycle checks. The source picker excludes invalid choices for the selected target; changing a target clears an assignment that is invalid there. Independent strips may feed one another as long as the graph remains acyclic.

Macro runs independently of notes and cannot create or prolong a voice. Pause retains state. Stop/reset restores the base value and resets offset smoothing. A direct seek resets the offset smoother at the destination; it does not reconstruct earlier smoothing history. An upstream LFO retains its documented [Rate history limit](lfo.md#depth-and-rate-modulation). Export and random-range rendering process from frame zero; uninterrupted playback and export share processing and remain independent of block size. Clip trims, repeats, and loop jumps retain the existing host note and provider lifecycle.

## Editing and saved state

The module saves presence, header power/collapse, modulation-open state, strips, and targeted routes in `macro_module` on each layer and Main. Each strip saves UUID, optional name, presence, power, versioned Value settings, and its connections through the module's route list. Missing or null strip names retain the default Macro N label. Missing `macro_module` fields default to an empty, absent module, preserving older projects' audio. Explicit null modules or malformed state are rejected. Validate names, finite 0–1 values, supported versions, unique provider/route IDs, existing target IDs, source scope, and graph dependencies atomically.

Right-click a strip's empty space, Value knob, or Power button and select Remove macro. This retains its identity, value, power, and routes while making its output neutral. The menu also works in compact layers and on Main; opening or dismissing it does not change the project. Plus restores the first retained strip before adding another. Remove Macro component in the header menu retains the whole module; catalog addition restores it. Header power and strip power remain independent. Unconnected macros do not change audio.

Drag the installed component's header to move its strips between regular layers and Main. A populated destination merges them and keeps its own header settings. Moving updates all consumers, including other macros, LFOs, Oscillator, Amplitude, and track effects. Identity collisions receive fresh IDs and matching target/source updates. Modifier dragging copies the module with fresh strip/route IDs and matching new targets; copied incoming connections retain their original source assignments. Names survive moves, copies, removal, and restoration. Existing preview, cancellation, and release-only transactions are shared with other components. Individual strip drag transfers remain outside this change.

Save/reopen, Save As, track duplication, sample-rate changes, and layer reordering retain macro values, identities, routes, and presentation. Compatible live value/power/routing edits retain other providers, voices, and phases. Checks and native-platform limits are recorded in [validation](validation.md).
