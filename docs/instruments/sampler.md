# Sampler

Sampler plays a selected audio region for each MIDI note. It is a source component for regular MIDI instrument layers. MIDI notes play relative to the sample's root note, across the full piano-roll range.

## Load a sample

1. In Instrument, select an empty layer and add Sampler from Sources in the component catalog. A layer can contain one of Sampler, Oscillator, or Granulizer. Use Replace source in an existing source's header menu to switch types.
2. Drop one WAV, MP3, or AAC-LC file onto Sampler, or select **Choose sample…** in its header menu. Supported formats and limits match [clip import](../views/timeline.md#supported-imports).
3. To use a Timeline audio clip, drag it over the Instrument view button, then drop it onto a Sampler. The Timeline clip stays in place. Its visible trim, repeats, and stretch are retained; track gain, pan, and effects are not captured.
4. Play MIDI notes to audition the sample.

The sample name appears in the preview's clip header. Drag the selection-bar edges to resize the playback region, its body to move it, or outside it to select another region. Shift bypasses snapping. Release applies the selection; Escape or focus loss cancels. This does not edit the recording or project loop selection.

The ruler and waveform grid follow the region's effective playback duration and project tempo. A position marker shows playback; with overlapping notes it shows the furthest active position.

## Controls

| Control | Use |
| --- | --- |
| Gain | Silence through +12 dB, default 0 dB. Accepts Envelope, LFO, and Macro modulation. |
| Root note | Default C4. Drag to change notes or click to type a natural note name and octave. Use the adjacent sharp toggle where valid. Names match the piano roll, C−1 through G9. This is the MIDI note that plays the sample at its original pitch. |
| Duration mode | Off, Time (ms), or Bars; default Off. Choose it from the dropdown beside the duration value. |
| Duration | Drag or type the target duration. Off shows the original region duration as a disabled millisecond value. Time uses positive milliseconds; Bars uses whole four-beat bars with a minimum of 1. |
| Trigger | One-shot or Gate, default One-shot. |

Off preserves the selected region's duration. Time stretches to a fixed duration independent of tempo. Bars follows project tempo. Stretching preserves pitch and allows durations from one eighth to eight times the selected input. Invalid durations leave the previous sample and settings in place. Before loading, Time starts at 1000 ms and Bars at 1 bar.

One-shot plays to the region's end after note-off. Gate follows note-off and any Amplitude release shaping. Trigger changes apply to new notes. Neither mode repeats the region. Velocity and Amplitude can change the level in either mode.

Root note and sharp share one input group. Duration value and mode share another; Trigger sits beside them below the preview. Enter or focus loss accepts valid typed values; Escape cancels. Root-note drags apply on release and cancel with Escape or focus loss.

## Play different pitches

Set Root note to the note heard in the original sample. For example, with C4 as the root, C4 plays the original pitch, C5 plays one octave higher, and C3 plays one octave lower. Root note does not detect or correct the recording's tuning.

Every MIDI note from C−1 to G9 plays relative to the root. Pitch changes retain the selected duration in Off, Time, and Bars modes. One-shot and Gate keep their normal release behavior.

Sampler uses the Time stretch engine selected in Project Settings: Rubber Band or Integrated (experimental). Adding or changing MIDI pitches can require sample preparation before the edit applies. Root-note, region, duration, tempo, and processing-setting changes can also require preparation. Cancel or a preparation error keeps the previous settings and audio. Longer samples and more distinct pitches require more time and memory; shorten the selection if preparation reports a memory limit.

Very large shifts can reduce sound quality and require more preparation time. Notes can become inaudible if the recording shifts below the audible range or above half the project sample rate. Pitch changes do not preserve vocal formants. Strong upward shifts can reduce high-frequency content. The sample preview stays at the root pitch and shows the common duration.

## Gain modulation

Use the header's secondary-color dot to open connections. Gain accepts Per note Envelopes and shared Envelope, LFO, or Macro sources from the same instrument, including Output. Amount spans −24 to +24 dB per connection. The base Gain stays unchanged, and exact base silence stays silent. Pitch, duration, root note, region, and Trigger do not accept modulation.

## Files and limits

Existing projects use their saved root note for pitch playback. Non-root notes can therefore sound different from earlier versions.

Replacing audio selects the new full input while retaining an explicitly set duration target. Preparation failure or cancellation keeps the previous sample and playback. Missing recordings stay visible and play as silence; shortened recordings supply silence for unavailable parts. Saving and Save As retain external file references and do not bundle recordings.

Reverse playback, internal looping, slicing, automatic pitch/tempo detection, multisampling, and live pitch modulation are not available. See [time stretching](../audio/time-stretch.md) and [modulation](../modulation/README.md).

The source header menu offers **Choose sample…** and **Replace source with Granulizer**. Replacement retains the sample and selected region; see [Granulizer](granulizer.md) for grain controls.
