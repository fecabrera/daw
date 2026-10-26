# Effect output meters

The standard Out-strip meter shows the effect's final stereo output after its level and Dry/Wet controls. It uses the larger absolute sample from the two channels. It measures sample peaks, not loudness or True Peak.

The thin horizontal rail sits above the output controls. Its display spans −60 to 0 dBFS. The fill uses the accent color and turns red above 0 dBFS. The level rises immediately and falls with a 300 ms release. Hover over it to read the current level and held peak, including values above the display limit.

A thin marker shows the highest output sample since the last reset. Click the rail to reset this held peak. This changes monitoring only; it does not change the level control, automation, project state, or Undo history. The audio thread captures peaks even when the panel is hidden. After a reset, the next actual sample can raise the marker again; the current meter's falling display does not restore an old peak.

Disabling the effect empties the rail, clears its held peak, and stops meter updates. Re-enabling resumes monitoring. Transport stop hides the current reading while retaining the held peak. Compact and collapsed units hide the rail without resetting the peak. Track and Master meters keep measuring their own buses during effect bypass.

This standard is used by Bitcrusher, Drive, Drive v2, Saturator, Chorus, Filter, Auto-filter, and Multiband EQ. Original Drive keeps its two-strip layout: its output meter sits above Volume in the Drive strip. Multiband EQ's Out meter measures the complete unit; its individual band meters remain separate. Gain-reduction meters measure a different quantity and keep their own scales.

Drive and Drive v2 CLAP/VST3 editors use the same rail and peak marker. These readings are not automatable parameters and are not saved in plugin state. VST3 hosts must connect the processor and controller through their standard SDK connection interfaces for meter delivery; otherwise the editor remains empty. Native host/platform verification limits are listed in the plugin guides.
