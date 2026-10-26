# Drive plugin development builds

Drive has independent CLAP and VST3 development plugins. Both use the original built-in Drive sound and the same two-strip editor as built-in Drive. External DAW and Windows/Linux acceptance checks are not complete, and this is not a release package.

See [CLAP build and checks](../../README.md#drive-clap-development-processor) or [VST3 build and checks](../../README.md#drive-vst3-development-processor) to create the development artifact. The build does not install it into a plugin folder. Release installation instructions will accompany the completed plugins.

## Parameters

The host exposes Volume, Gain, Tone, Color shape, Color strength, Color frequency, and Bypass. Their defaults and audible behavior match [built-in Drive](drive.md). Bypass defaults off. Both development processors accept stereo 32-bit audio at 44.1, 48, 88.2, 96, 176.4, or 192 kHz; it rejects other rates.

The host can automate all seven parameters. Color Off retains Strength and Frequency, including automated changes. The Drive editor dims Strength and Frequency while Color is Off. A generic host editor may keep those fields editable.

Host Bypass passes dry audio with the existing smooth transition. If the host continues calling Drive, its filter histories keep running. A host that suspends the plugin controls when processing stops.

Saved host state retains the seven base parameters. It does not include DAW Envelope/LFO/Macro connections or exact filter histories. The host manages automation and Undo. Plugin reset clears filter histories while keeping parameter values.

Drive adds no processing delay. It reports a one-second residual tail to the host. Strong high-frequency drive can alias, as in the built-in effect. Offline and realtime processing use the same sound.

## Editor

The Drive strip contains Volume, Gain, and Tone. The Color strip contains Shape, Strength, and Frequency. Drag a knob up or right to increase its value. Hold Shift for smaller changes. Use arrow keys after selecting a knob. Double-click to reset it. The host receives edit gestures for automation and Undo.

Right-click the editor background to select Dark, Light, or Mist. This choice stays with the plugin instance when you close and reopen the editor. It does not change the sound and is not saved with host project state yet. The host supplies the window frame and Bypass control.

The editor has a fixed size. Native screen-reader control exposure and Windows/Linux editor behavior still require work and validation. These development artifacts are not release packages.
