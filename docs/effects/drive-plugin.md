# Drive plugin development builds

Drive has independent CLAP and VST3 development plugins. Both use the original built-in Drive sound and the same two-strip editor as built-in Drive. External DAW and Windows/Linux acceptance checks are not complete, and this is not a release package.

See [build and packaging instructions](../development/plugins.md) to create the development artifacts. Packaging does not install them into a plugin folder.

## Host compatibility and installation

Use the VST3 build in a host that supports VST3. Logic Pro requires Audio Units; it cannot load these VST3 or CLAP builds. An Audio Unit version is not implemented. The macOS build is Apple Silicon only: use a native Apple Silicon host, not an Intel/Rosetta VST3 host.

On macOS, a `.vst3` plugin is a bundle directory containing `Contents/Info.plist` and its executable. A folder appearance alone does not establish whether it can load. Copy the whole `Drive.vst3` bundle to `/Library/Audio/Plug-Ins/VST3` or your user's `~/Library/Audio/Plug-Ins/VST3`. Keep its contents intact. In Ableton Live, enable the VST3 system folders and rescan after replacing the plugin. Restart the host if the old module was loaded.

New macOS development packages are ad-hoc signed after their resources are assembled. Older archives had only the Rust dylib's linker signature, which is invalid as a complete bundle signature and can prevent host loading. Use a newly packaged build. These previews are not notarized; downloaded builds can still need approval through macOS Privacy & Security. The packager does not install plugins automatically.

## Parameters

The host exposes Volume, Gain, Tone, Color shape, Color strength, Color frequency, and Bypass. Their defaults and audible behavior match [built-in Drive](drive.md). Bypass defaults off. Both development processors accept stereo 32-bit audio at 44.1, 48, 88.2, 96, 176.4, or 192 kHz; it rejects other rates.

The host can automate all seven parameters. Color Off retains Strength and Frequency, including automated changes. The Drive editor dims Strength and Frequency while Color is Off. A generic host editor may keep those fields editable.

Host Bypass passes dry audio with the existing smooth transition. If the host continues calling Drive, its filter histories keep running. A host that suspends the plugin controls when processing stops.

Saved host state retains the seven base parameters. It does not include DAW Envelope/LFO/Macro connections or exact filter histories. The host manages automation and Undo. Plugin reset clears filter histories while keeping parameter values.

Drive adds no processing delay. It reports a one-second residual tail to the host. Strong high-frequency drive can alias, as in the built-in effect. Offline and realtime processing use the same sound.

## Editor

The Drive strip contains Volume, Gain, and Tone. The Color strip contains Shape, Strength, and Frequency. Drag a knob up or right to increase its value. Hold Shift for smaller changes. Use arrow keys after selecting a knob. Double-click to reset it. The host receives edit gestures for automation and Undo.

Plugin editors use Deep Orange (`#FF5722`) as their default accent. Right-click the editor background to select Dark, Light, or Mist. This choice stays with the plugin instance when you close and reopen the editor. It does not change the sound and is not saved with host project state yet. The host supplies the window frame and Bypass control.

The editor has a fixed size. Native screen-reader control exposure and Windows/Linux editor behavior still require work and validation. These development artifacts are not release packages.

The editor includes the [standard output rail and held-peak indicator](output-meter.md). Clicking the rail resets monitoring without a host automation gesture. VST3 meter delivery uses the host's processor/controller connection; CLAP reads the instance's prepared atomic meter.
