# Audio recording

## Routing and transport

Audio and MIDI track headers and compatible rows share the R control. When
no Audio or MIDI track is explicitly armed, the selected Audio track is the
recording destination. Its R text uses the accent color. Explicitly armed
tracks use the active fill and retain their destinations across selection
changes. Store channel routing in desktop session state by
track ID and selected device. Interpret an unconfigured route as Mono 1. Support
up to 16 armed Audio tracks and 16 distinct input channels. A stereo route uses
two adjacent channels. Validate the routes against the opened device. One
physical input stream serves all routes. Identical routes share one source with
a distinct clip on each destination track.

The existing transport and MIDI service coordinate audio-only and mixed takes.
An audio or mixed take freezes its destinations before start. Preserve the
fallback cue and input peak on the recording destination while capture runs.
Preserve the MIDI-only selected-track fallback and live input. An unarmed selected MIDI track
can play live during an audio-only take. Follow the stopped count-in and running
transport rules of [MIDI recording](midi-recording.md). Pause and Stop seal the
take. Record seals it while playback continues. Focus loss releases live MIDI
without closing an audio take. Panic seals an active take. Loop capture is
disabled. Save requests during audio capture run after finalization.

## Capture and alignment

Open the selected CPAL input on a worker. Account for separately negotiated
device and project rates. Callbacks copy the selected channels into a bounded
sample ring and publish timing and peak data without allocation or file I/O.
The input signal has no path to the renderer. Software input monitoring is off.

Map ADC and DAC timestamps through process-monotonic observations. Establish
separate origins for stream timestamps. Use callback-authoritative Start and
exclusive End project ordinals, produced device samples, converter delay, and
observed output blocks. Preserve the normal converter tail up to the take end.
Interpolate audio on the project clock using a windowed sinc on the worker.
A clock reversal, missing samples, buffer pressure or timeout ends a take at
the valid prefix. Native callback timing and pair compatibility require
physical loopback qualification.

## Publication and storage

A versioned Audio or mixed take journal records a durable base, route IDs,
transport origin, MIDI events, and checksummed aligned PCM blocks. Sync PCM
before publishing its journal descriptor. Preserve the existing MIDI journal
reader. Cap the raw spool at 2 GiB or 30 minutes, whichever comes first. Retain the
checked prefix after interruption and place all valid destinations in one
overwrite transaction. Export prepared mono or stereo float32 WAV at project
rate. Save copies the recorded files into `assets/sources/`. Private source
owners keep their PCM available through first Save, Save As and history.
Preserve ordinary clip placement, existing project schema, and headless
playback and export. See [recovery](../projects/autosave-recovery.md).

## Acceptance

| Area | Acceptance check |
| --- | --- |
| Clock and conversion | Test every project rate with converted output and independent input phase and drift. Verify the exact clip length and pulse positions. |
| Shared transport | Check count-in, Start, Record again, Pause and Stop with audio and MIDI armed. Check that software input monitoring stays off. |
| Routing | Check mono, stereo, shared routes, missing devices and unavailable channels. |
| Storage | Verify interrupted PCM recovery, one Undo step, first Save, Save As, and headless rendering of the saved project. |
| Native alignment | Test CoreAudio, WASAPI and ALSA with a qualified wired or dedicated digital loopback. Target placement within 3 ms across ten starts and drift of at most 2 ms over 30 minutes. Record the device pair, driver, rates and buffers. |
| Native operation | Check permission grant, denial and retry, device loss, callback and disk pressure, and audible Start and Stop boundaries. |

Follow the [development loopback instructions](../../docs/development/audio-loopback.md)
for device selection and test commands.
