# Shimmer Reverb

Shimmer Reverb adds a stereo reverb tail, two regenerating pitch voices and a
vowel color. Find it under **Reverb** in the Effects catalog. It is also available
as a separate [CLAP or VST3 plugin](plugins.md).

Use **Shimmer Reverb** for spacious tails with rising or falling harmonics and
vowel color.

1. Add the unit to a track, instrument layer, Output bus, group or Master.
2. Set Size and Pre-delay to shape the space.
3. Set a Shimmer voice's Pitch, then raise its Amount. Try +12 st for an octave
   above the input, or −12 st for an octave below it. A zero Pitch mutes that
   voice's regeneration, even when Amount is raised.
4. Use Vowel Position and Amount for a vocal color. Use Motion Rate and Depth
   for movement in the tail. Zero Depth stops delay motion.
5. Adjust Damping and Warm, then set Dry/Wet and Level. The curved mix can raise
   the combined level at intermediate settings. Watch the output meter.

| Strip | Controls and defaults |
| --- | --- |
| Reverb | Size 75%; Pre-delay 0 ms; Time mode |
| Vowel | Position 0%; Amount 0% |
| Shimmer | Two voices, each at 0 st and 0% Amount |
| Motion | Rate about 39%; Depth about 43% |
| Damping | Low Damp 0%; High Damp 0%; Warm Off |
| Out | Level 0 dB; Dry/Wet 50%; output level and held peak |

Size changes delay lengths and feedback together. It is not a decay-time
readout. Rate, Depth, Position, Amount, Size and both damping controls range from
0% to 100%. Low Damp reduces low-frequency energy inside the tail. High Damp
reduces high-frequency energy there. Warm softens the wet output independently.

In Time mode, Pre-delay ranges from 0 to 1000 ms. In Sync mode, select a musical
division from 1/64 to dotted 1/2; T means triplet and D means dotted. The retained
initial division is dotted 1/16. Sync follows project tempo, or host tempo in a
plugin, and limits the actual delay to eight seconds at slow tempos. Switching
modes retains both choices. Plugins use 120 BPM until a valid host tempo arrives,
then retain the last valid tempo when timing is absent.

Pitch spans −12 to +12 semitones. Choices near zero are −1, −0.75, −0.5, −0.25,
0, +0.25, +0.5, +0.75 and +1. Other choices use whole semitones. Each voice feeds
its shifted sound back into itself, so repeated intervals can build a cascade.
Equal pitches receive less combined injection to avoid doubling their level.

Level ranges from exact silence to +12 dB and applies after the mix. Header power
bypasses the complete unit. It clears the meters immediately while its histories
continue, so re-enabling can reveal a remaining tail. Click a held peak to reset
it. Parameter changes retain the tail. Stop and seek reconstruct processing
history; Pause preserves it and loop boundaries let it continue.

The completed wet signal has 15 dB of calibrated makeup gain before Dry/Wet.
This fixed gain leaves Size, damping and pitch regeneration free to change the
tail's energy; it does not raise a fading tail automatically. The regeneration
loops are unchanged. Existing projects use the louder wet output with their
saved settings. Reduce Dry/Wet or Level as needed and check the output meter;
makeup gain does not impose an output ceiling.

Continuous controls support the DAW's [shared modulation](../modulation/README.md).
Pre-delay modulation applies in Time mode. Pitch choices, Sync, note division and
Warm remain discrete. Standalone plugins expose host automation without DAW
modulation connections. Settings are saved with the project or plugin preset.

The unit has no dry-path alignment latency. DAW exports retain the existing
ten-second tail limit and warn when a longer tail budget is truncated. External
plugin host and listening acceptance remain open. This is an original effect;
it does not claim exact Solaris sound or preset compatibility.
