# Unit output meters

Every audio unit in the DAW has a vertical stereo meter in the gap after it,
including the last unit. It stays visible in compact and collapsed views. Left
and right channels have separate levels and held-peak markers. Click either rail
or the gap between them to reset both peaks. Envelope, LFO and Macro have no
audio meter.

DAW strips have no horizontal output-level rails. Per-band level meters,
gain-reduction meters and signed gain-change meters remain in their strips.
The Instrument Output layer keeps its stereo meter beside Gain.

Effects measure final stereo output after Level, Dry/Wet and bypass mixing.
Oscillator, Sampler, Granulizer and Noise measure their summed source output after
unit Level/Gain and note velocity, before Amplitude and layer processing.
Amplitude measures summed notes after its gain and envelopes, before layer
effects and mixing. Source meters can therefore show signal while downstream
controls silence playback. These meters measure sample peaks, not loudness or
True Peak.

The display spans −60 to 0 dBFS. A channel is green at or below 0 dBFS and red
above it. The level rises immediately and falls with a 300 ms release. A thin
grey marker holds the highest sample since reset. Hover to read current and held
levels, including values above the display limit. Reset changes monitoring only;
it does not change gain, automation, project data or Undo history. The next audio
sample can raise the peak again.

Disabling a unit empties its rails, clears held peaks and stops updates.
Re-enabling resumes monitoring. Stop and Pause hide current levels while
retaining held peaks. Track and Master meters keep measuring their own buses
during effect bypass.

## Out-strip coverage

| Location | Output meter |
| --- | --- |
| DAW audio units | Stereo rails in the gap after the unit. |
| Standalone Out/Output strips | Horizontal rail above the output controls. |
| Original Drive standalone editor | Horizontal rail above Volume in the Drive strip. |

## Standalone plugin editors

Standalone effect editors retain their horizontal output-level rails and held
peak indicators in the Out/Output strip. Original Drive places its rail above
Volume in the Drive strip. They use the same colors, scale, release and reset
behavior. Per-band and gain-reduction meters retain their existing positions.

These readings are not automatable parameters and are not saved in plugin state.
VST3 hosts must connect processor and controller through their standard SDK
interfaces for meter delivery; otherwise the editor remains empty. Native host
and platform verification limits are listed in the plugin guides.
