# Audio sidechain modulation

Use another track, group or Return to control a parameter. The connection measures
its audio and converts the level to a value from 0 to 1. It does not add the
source audio to the receiving track.

## Duck a bass from a kick

1. Open the bass unit's modulation controls and add a connection.
2. In Source, open Tracks and select the kick track.
3. Select a gain Destination and turn its Amount knob below zero.
4. Set Lower and Upper in Mapping to the useful level range of the kick.
5. Adjust Attack and Release in Response. Watch Input and Value during playback.

Lower maps to 0; Upper maps to 1. Levels outside that range stop at the nearest
endpoint. A negative gain Amount reduces the bass as the kick becomes louder.
Invert is optional; basic ducking does not need it. Amount keeps the destination's
usual units and range. Double-click it to reset to zero; hold Shift for fine control.

## Detector

Pre-fader measures the source after its effects, before its fader, pan, mute and
solo gate. Muting a kick can therefore leave its sidechain active. Post-fader
also follows that source's gain, pan, mute and solo gate. For groups and Returns,
the measured bus already contains the audible child or send mix.

Stereo links both channels. Left and Right use one channel. Mid measures the
common signal; Side measures the difference. Opposite-polarity channels can
cancel in Mid. Peak follows instantaneous peaks; RMS follows average energy.

Filter enables detector-only High-pass and Low-pass controls, from 20 Hz to
20 kHz. The filters do not change the audible source.

## Mapping and response

| Control | Default | Range or choices |
| --- | --- | --- |
| Tap | Pre-fader | Pre-fader, Post-fader |
| Channel | Stereo | Stereo, Left, Right, Mid, Side |
| Detector | Peak | Peak, RMS |
| Filter | Off | High-pass 30 Hz; Low-pass 20 kHz |
| Lower / Upper | −36 / −6 dBFS | −120 to +24 dBFS; Lower must be below Upper |
| Curve | dB | dB, Amplitude |
| Attack / Release | 5 / 120 ms | 0 to 10,000 ms |
| Invert | Off | Reverse the mapped value |

dB distributes the response across the level range in decibels. Amplitude uses
linear signal level. Attack sets how fast the detector rises; Release sets how
fast it falls. Input shows the detected level in dBFS. Value shows the mapped
value from 0 to 1. A dash means that a current reading is unavailable.

The extra controls fit in the layer lane beside Mod 1, with Detector, Mapping
and Response labels aligned along the bottom. They appear only for
audio sources. Envelope, LFO and Macro connections keep their existing controls.
The Source menu places None first, modulation sources next, and Tracks, Groups
and Returns last, with a divider between each section.

## Sources and transport

Feedback routes are unavailable, including paths through groups or Returns.
Disabling a connection or setting Amount to zero does not make such a path valid.
Self-taps are unavailable. Rename and reorder preserve assignments. Copies retain
the original source track; deleting that source leaves an unavailable assignment
that contributes no modulation, even with Invert enabled. Undo restores it.

Playback and export use the same detector and mapping. Source and destination
effect delays are aligned. Starting or seeking prepares detector history before playback resumes. Loop playback keeps the detector's response continuous
across the boundary. Audio sidechains can drive shared effects and compatible
per-note controls; they cannot keep a released note alive.

## Bounced tracks

[Bounce](../editing/bounce.md) keeps downstream routing live and retains captured
audio for existing sends and sidechains. Keep the original track when its identity
is still used as a sidechain source. Captured routing stages are snapshots.
