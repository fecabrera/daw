# Unit output meters

Every audio unit with an **Out** or **Output** strip uses the standard level meter and held-peak indicator. For effects, it measures final stereo output after level and Dry/Wet. For Oscillator and Granulizer, it measures the combined source output after the unit's Level/Gain and note velocity, before Amplitude and downstream processing. It uses the larger absolute sample from the two channels. It measures sample peaks, not loudness or True Peak.

The thin horizontal rail sits above the output controls. Its display spans −60 to 0 dBFS. The fill uses the same theme palette as track level meters: green at or below 0 dBFS and red above 0 dBFS. Changing the accent color does not change level meter colors. The level rises immediately and falls with a 300 ms release. Hover over it to read the current level and held peak, including values above the display limit.

A thin secondary-grey marker shows the highest output sample since the last reset. Click the rail to reset this held peak. This changes monitoring only; it does not change the level control, automation, project state, or Undo history. The audio thread captures peaks even when the panel is hidden. After a reset, the next actual sample can raise the marker again; the current meter's falling display does not restore an old peak.

Disabling the unit empties the rail, clears its held peak, and stops meter updates. Re-enabling resumes monitoring. Transport stop hides the current reading while retaining the held peak. Compact and collapsed units hide the rail without resetting the peak. Track and Master meters keep measuring their own buses during effect bypass.

This standard is used by Bitcrusher, Drive, Drive v2, Saturator, Chorus, Filter, Auto-filter, Multiband EQ, Multiband Exciter, Compressor, OTT, and Limiter. Original Drive keeps its two-strip layout: its output meter sits above Volume in the Drive strip. Multiband EQ's Out meter measures the complete unit; its individual band meters remain separate. Limiter keeps its reduction rail above Limit on the left and its level rail above Out on the right. Compressor and OTT add the level rail only above Out. Gain-reduction meters measure a different quantity and keep their own scales.

All standalone effect editors use the shared level-meter colors, rail and peak marker where those monitors are present. These readings are not automatable parameters and are not saved in plugin state. VST3 hosts must connect the processor and controller through their standard SDK connection interfaces for meter delivery; otherwise the editor remains empty. Native host/platform verification limits are listed in the plugin guides.

## Out-strip coverage

| Units | Meter placement |
| --- | --- |
| Bitcrusher, Drive v2, Saturator, Chorus, Filter, Auto-filter, Multiband EQ, Multiband Exciter, Compressor, OTT and Limiter | Existing Out/Output strip. |
| Original Drive | Existing Volume control in its Drive strip. |
| Oscillator | Out strip, after summing all oscillator sources and notes. |
| Granulizer | Out strip, after summing stereo grains across notes. |
| Instrument Output layer | Existing track-style stereo meter in its layer header. |

Source meters can show a signal while Amplitude, a layer fader, or a later bus silences playback. They measure the source at its own Out strip. Their current level hides when playback stops or pauses; the held peak remains until reset or unit power-off. Compact and collapsed units hide the rail while capture continues.

Regular Sampler uses a **Sample** strip rather than Out. The 3-band EQ, Direction Mixer, Stereo Spread, Amplitude, Envelope, LFO and Macro have no Out strip; this standard does not add strips to those units. Existing meters retain their roles.
