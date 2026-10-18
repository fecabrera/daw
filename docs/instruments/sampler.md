# Sampler

Sampler plays a selected audio region for each MIDI note. It is a source component for regular MIDI instrument layers. All notes currently play at the sample's original pitch; root note is descriptive only.

## Load a sample

1. In Instrument, select an empty layer and add Sampler from Sources in the component catalog. A layer can contain Sampler or Oscillator, not both. Use Replace source in an existing source's header menu to switch types.
2. Drop one WAV file onto Sampler. Mono/stereo PCM16, PCM24, and float32 WAV files are supported.
3. To use a Timeline audio clip, drag it over the Instrument view button, then drop it onto a Sampler. The Timeline clip stays in place. Its visible trim, repeats, and stretch are retained; track gain, pan, and effects are not captured.
4. Play MIDI notes to audition the sample.

The sample name appears in the preview's clip header. Drag the selection-bar edges to resize the playback region, its body to move it, or outside it to select another region. Shift bypasses snapping. Release applies the selection; Escape or focus loss cancels. This does not edit the recording or project loop selection.

The ruler and waveform grid follow the region's effective playback duration and project tempo. A position marker shows playback; with overlapping notes it shows the furthest active position.

## Controls

| Control | Use |
| --- | --- |
| Gain | Silence through +12 dB, default 0 dB. Accepts Envelope, LFO, and Macro modulation. |
| Root note | Default C4. Drag to change notes or click to type a natural note name and octave. Use the adjacent sharp toggle where valid. Names match the piano roll, C−1 through G9. This does not transpose playback yet. |
| Duration mode | Off, Time (ms), or Bars; default Off. Choose it from the dropdown beside the duration value. |
| Duration | Drag or type the target duration. Off shows the original region duration as a disabled millisecond value. Time uses positive milliseconds; Bars uses whole four-beat bars with a minimum of 1. |
| Trigger | One-shot or Gate, default One-shot. |

Off preserves the selected region's duration. Time stretches to a fixed duration independent of tempo. Bars follows project tempo. Stretching preserves pitch and allows durations from one eighth to eight times the selected input. Invalid durations leave the previous sample and settings in place. Before loading, Time starts at 1000 ms and Bars at 1 bar.

One-shot plays to the region's end after note-off. Gate follows note-off and any Amplitude release shaping. Trigger changes apply to new notes. Neither mode repeats the region. Velocity and Amplitude can change the level in either mode.

Root note and sharp share one input group. Duration value and mode share another; Trigger sits beside them below the preview. Enter or focus loss accepts valid typed values; Escape cancels. Root-note drags apply on release and cancel with Escape or focus loss.

## Gain modulation

Use the header's secondary-color dot to open connections. Gain accepts Per note Envelopes and shared Envelope, LFO, or Macro sources from the same instrument, including Main. Amount spans −24 to +24 dB per connection. The base Gain stays unchanged, and exact base silence stays silent. Pitch, duration, root note, region, and Trigger do not accept modulation.

## Files and limits

Replacing audio selects the new full input while retaining an explicitly set duration target. Preparation failure or cancellation keeps the previous sample and playback. Missing recordings stay visible and play as silence; shortened recordings supply silence for unavailable parts. Saving and Save As retain external file references and do not bundle recordings.

Reverse playback, internal looping, slicing, automatic pitch/tempo detection, multisampling, and pitch shifting are not available. See [time stretching](../audio/time-stretch.md) and [modulation](../modulation/README.md).
