# Limiter

Limiter controls peaks toward a chosen output Ceiling. Add it to an audio/MIDI track or Master from Dynamics in the Effects catalog.

## Controls

| Setting | Default | Accepted range/options |
| --- | --- | --- |
| Input | 0 dB | −24 to +24 dB |
| Release | 100 ms | 1–2000 ms |
| Lookahead | 1 ms | 0–10 ms |
| Ceiling | −1 dB | −24 to 0 dB |
| Mode | Peak | Peak / True Peak (compact label: True) |
| Routing | L/R | L/R / M/S |

Input raises or lowers the signal before limiting. Release sets recovery time. Lookahead allows the limiter to respond before a peak reaches its output. Ceiling sets the output target.

Peak works on sample peaks. True Peak also estimates peaks between samples. L/R uses linked stereo reduction. M/S treats the center and side content separately, then applies final stereo peak protection.

Header power bypasses limiting while retaining the unit's delay and settings.

## Gain-reduction meter

The rail above the left **Limit** strip shows current gain reduction from 0–24 dB. A thin line in the secondary text color marks the highest reduction since the peak was reset. Click the rail to reset this line. The current reading and limiting settings stay the same. If limiting continues, new readings can immediately set another peak.

Hover for current and peak readings, including reductions above 24 dB. The peak line stops at the end of the rail. Compact and collapsed units hide the meter, but peak capture continues. The held peak remains during pause, stop, and seeking until you reset it or turn the unit off.

Turning the unit off clears the fill and peak line and stops meter updates. Turning it on resumes monitoring. The current reading shows zero when playback is stopped or paused, or no output device is available. Peak readings are temporary monitor state and are not saved with the project.

## Modulation

Input, Release, Lookahead, and Ceiling accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Open the secondary-color header dot for [connections](../modulation/README.md). Mode and Routing remain manual choices. Release Amount uses octaves of time; +1 doubles the duration and −1 halves it.

## Ceiling and timing limits

The ceiling applies at this unit's output after controls settle. Later effects, track gain/pan, or Master gain can raise the level again. Leave the control transition time to settle when judging a changed ceiling, including modulation.

True Peak is an estimate, not a guaranteed bound for every reconstructed waveform or a certification claim. A demanding near-Nyquist stereo burst exceeded the selected ceiling by about 0.21 dB in an independent reconstruction check. Leave additional headroom where the delivery target requires it.

Each instance adds a fixed delay of approximately 10 ms plus 32 samples, including Peak mode and bypass. Playback and export compensate for effect alignment. Lookahead changes do not move clips on the timeline.

## Output level meter

The **Out** strip has the [shared output level meter](output-meter.md), with a current level rail and click-resettable held sample peak. It measures the final stereo output. Its peak reset is independent of the gain-reduction meter.

This effect also has a [standalone CLAP/VST3 plugin](plugins.md).
