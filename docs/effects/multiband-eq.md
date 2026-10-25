# Multiband EQ

Add **Multiband EQ** from **Effects > EQ & Filters** to a track, instrument layer, Output, or Master. It starts with one neutral Bell band. The existing 3-band EQ remains available separately.

The band, **+**, and **Out** strips follow the Oscillator layout. Each band keeps its four smaller knobs in one column.

Click **+** to add a band. Right-click a band's knob or power button and choose **Remove band** to remove it. There is no fixed band-count limit; more bands use more processing time. You can remove every band and use only Output Gain.

Each band has four knobs:

| Control | Choices or range | Default |
| --- | --- | --- |
| Mode | Bell, Low shelf, High shelf, Low pass, High pass, Notch | Bell |
| Frequency | 20 Hz–20 kHz | 1 kHz |
| Gain | −24 to +24 dB | 0 dB |
| Q | 0.1–10 | 0.71 |

Bell boosts or cuts around Frequency. Shelves adjust the frequencies below or above Frequency. Low pass removes higher frequencies; High pass removes lower frequencies. Notch removes frequencies around Frequency. Higher Q makes the response narrower or more resonant. Gain is available for Bell and shelves; pass and notch modes retain the saved Gain but do not use it.

The band's power button bypasses that band and retains its settings. Header power bypasses the entire unit. The final **Out** strip adjusts the processed signal from silence to +12 dB, with a default of 0 dB.

Each expanded band has a meter above its knobs. It shows the louder channel at that band's output, before later bands and Out Gain, on the shared −60 to 0 dBFS scale. Levels above 0 dBFS use the overload color; disabled bands use grey. Header power clears the meters. Stopped or paused playback shows empty rails. Compact and collapsed units hide them. Monitoring does not change the sound.

Open the header's modulation dot to connect shared Envelope, LFO, or Macro sources from the same MIDI instrument. Select **Band N · Gain**, **Frequency**, **Q**, or **Output · Gain**. Gain amounts use dB, Frequency uses octaves, and Q uses Q units. Knob arcs and live indicators show modulation while the pointer retains the saved base value. Mode and power are discrete controls.

Removing a band removes its modulation connections. Other bands keep their connections when their displayed numbers change. Undo restores a removed band and its connections.
