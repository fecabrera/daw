# Compressor

Compressor reduces loud passages according to Threshold and Ratio. Add it to an audio/MIDI track or Master from Dynamics in the Effects catalog.

## Controls

| Control | Range | Default |
| --- | --- | --- |
| Threshold | −60–0 dBFS | −18 dBFS |
| Ratio | 1:1–20:1 | 4:1 |
| Detection | Peak, RMS | Peak |
| Attack | 0.1–200 ms, logarithmic | 10 ms |
| Release | 10–2000 ms, logarithmic | 100 ms |
| Knee | 0–24 dB | 6 dB |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |

Threshold is the level where compression begins. Ratio sets how strongly levels above it are reduced. A ratio of 1:1 gives no compression after any previous reduction releases. Knee softens the transition around Threshold; zero gives a hard transition.

Peak responds to short peaks. RMS responds to average signal energy. The stereo channels receive the same reduction to retain their balance. Attack controls how quickly reduction increases; Release controls how quickly it recovers. These are response speeds, not fixed playback delays.

Dry/Wet blends original and compressed audio. Level adjusts the complete blend. Header power bypasses the effect and passes the original input without Level gain.

## Gain-reduction meter

The read-only meter shows reduction before Dry/Wet and Level. Its rail covers 0–24 dB; the numeric reading can report larger reductions. It reads zero when playback is stopped or paused, or no output device is available.

## Modulation and limits

Threshold, Ratio, Attack, Release, Knee, Level, and Dry/Wet accept shared Envelope, LFO, and Macro sources from the same MIDI instrument. Open the secondary-color header dot for [connections](../modulation/README.md). Peak/RMS remains a manual choice. Attack and Release amounts use octaves of time: +1 doubles the duration and −1 halves it.

Lookahead, automatic makeup gain, external sidechain routing, and detector high-pass controls are not available.
