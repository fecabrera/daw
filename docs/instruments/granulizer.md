# Granulizer

Granulizer turns an audio region into overlapping short pieces called grains. MIDI notes control grain pitch. Scan speed controls movement through the recording independently of pitch.

## Load a sample

1. Add **Granulizer** from **Instrument → Sources** to an empty layer.
2. Drop one WAV, MP3, or AAC-LC file onto the unit, or open its header menu and select **Choose sample…**. You can also drop an audio clip from the Timeline to retain its visible trim, repeats, and stretch.
3. Select the input region using the bar above the waveform. Set the root note to the MIDI note that should play at the original pitch; the default is C4.
4. Add notes to a MIDI pattern on the track and start playback.

Each layer accepts one of Oscillator, Sampler, or Granulizer. The source header menu can replace it. Switching between Sampler and Granulizer retains the sample and selected region. Other layer components remain in place.

Loading and region preparation run in the background. Failed or cancelled work retains the previous sample and settings. Missing recordings retain their saved reference and play silence. Saving retains external recording references; it does not bundle the files or change their contents. Projects containing Granulizer require a build that supports this component.

## Controls

| Strip | Control | Range and behavior | Default |
| --- | --- | --- | --- |
| Grain | Size | 10–500 ms per grain | 80 ms |
| Grain | Overlap | 0.25–8×. At 4×, a new grain starts every quarter of a grain's duration. Values below 1 leave gaps. | 4× |
| Grain | Window | Hann, Triangle, or Trapezoid fades. Trapezoid uses a quarter of the grain for each fade. | Hann |
| Scan | Position | 0–100% of the selected region. Changing it moves the scan position by the corresponding fraction. | 0% |
| Scan | Speed | −4 to +4×. Positive scans forward; negative scans backward; zero freezes the scan while grains continue. | 1× |
| Scan | Pitch | −48 to +48 semitones, added to the MIDI note's distance from the root note | 0 |
| Random | Position jitter | Random grain-start offset up to ±100% of the selected region | 0% |
| Random | Pitch jitter | Random pitch offset up to ±48 semitones per grain | 0 |
| Random | Pan randomness | Independent random stereo balance for each grain | 0% |
| Out | Gain | Silence to +12 dB | 0 dB |

Out includes the [standard unit level meter and held-peak indicator](../effects/output-meter.md). It measures the stereo source sum across notes after Gain and note velocity, before Amplitude and layer processing. Click the rail to reset only the held peak. Compact/collapsed views hide it; unit power-off clears it.

Negative Speed changes the order of grain positions. Audio inside each grain still plays forward. Window, grain size, pitch, and random pan changes apply when new grains start. Existing grains finish with their original controls. Overlap compensation limits increases in level when more grains overlap; it does not act as a limiter.

Below the waveform:

- **Root note** sets the original MIDI pitch across the full C−1–G9 range.
- **Gate** follows the MIDI note and the layer's Amplitude release. **One-shot** emits grains for the selected region's original duration, even after note-off, then lets those grains finish. Speed and MIDI pitch do not change that emission duration.
- **Loop** wraps the scan and grain reads within the selected region. When disabled, the scan stops at an edge and grains fade out. Freeze continues at its position. Gate is the default trigger, and Loop starts enabled.

Use the existing **Amplitude** component for the note envelope and layer effects for further processing. The unit has no internal LFO. Connect Envelope, LFO, or Macro to Gain or any continuous granular control through the standard modulation connections. Window, Loop, Trigger, and root note remain discrete controls.

## Waveform and practical limits

The accent line shows an active voice's scan position. Thin grey lines show up to sixteen active grain positions from that voice. Shading shows the position-jitter interval, clipped at the region edges; with Loop enabled, starts outside an edge wrap around. These readings appear during playback.

Random sequences repeat for the same sample, settings, and MIDI notes, including after save/reopen and during export. A fixed grain limit protects playback during rapid Size/Overlap changes. If the limit is reached, new grains are skipped until space is available; active grains finish normally.

Granulizer colors the source and can soften transients. Large upward pitch shifts can alias because grain playback uses linear interpolation. This version has no tempo-synced scan, transient detection, slices, live capture, or reverse playback inside individual grains.
