# Multiband EQ

Add **Multiband EQ** from **Effects > EQ & Filters** to a track, instrument layer, Output, or Master. It starts with one neutral Bell band. The existing 3-band EQ remains available separately.

Use **Multiband EQ** for targeted tonal corrections, resonant cuts and boosts, and flexible frequency shaping.

The band, **+**, and **Out** strips follow the Oscillator layout. Each band keeps its four smaller knobs in one column.

The **Response** graph sits before the band strips and spans the three standard control rows. It shows each band's response, the combined EQ curve, and a live output spectrum. The spectrum uses both stereo channels and clears when the unit is off. Compact and collapsed units hide the graph.

Drag a numbered band point left or right to change Frequency. For Bell and shelf bands, drag up or down to change Gain. Hold Command on macOS or Ctrl on Windows/Linux when starting the drag to adjust only Q: up or right increases Q; down or left decreases it. Hold Shift for fine adjustment. The selected point shows Frequency, Gain where available, and Q.

Dragging previews the curve and sound. Release to apply the values; Escape or loss of focus cancels and restores the original values. In the DAW, release creates one Undo step. Audible previews use the audio preparation worker and can follow the pointer with a delay in large projects. Standalone plugins send normal host parameter gestures during the preview. Modulation indicators retain the saved base values while the curves follow effective values.

Click **+** to add a band. Right-click a band or its controls for **Enable band**/**Disable band**, checked **Solo**, and **Remove band**. There is no fixed band-count limit; more bands use more processing time. You can remove every band and use only Output Gain.

Each band has four knobs:

| Control | Choices or range | Default |
| --- | --- | --- |
| Mode | Bell, Low shelf, High shelf, Low pass, High pass, Notch | Bell |
| Frequency | 20 Hz–20 kHz | 1 kHz |
| Gain | −24 to +24 dB | 0 dB |
| Q | 0.1–10 | 0.71 |

Bell boosts or cuts around Frequency. Shelves adjust the frequencies below or above Frequency. Low pass removes higher frequencies; High pass removes lower frequencies. Notch removes frequencies around Frequency. Higher Q makes the response narrower or more resonant. Gain is available for Bell and shelves; pass and notch modes retain the saved Gain but do not use it.

The band's power button bypasses that band and retains its settings. Header power bypasses the entire unit. The final **Out** strip adjusts the processed signal from silence to +12 dB, with a default of 0 dB.

Each expanded band has a meter above its knobs. It shows the louder channel at that band's output, before later bands and Out Gain, on the shared −60 to 0 dBFS scale. Enabled bands use the track meter's green at or below 0 dBFS and red above 0 dBFS, independently of the accent color; disabled bands use grey. Header power clears the meters. Stopped or paused playback shows empty current-level fills while held peaks remain. Compact and collapsed units hide them. Monitoring does not change the sound.

Each band level rail includes the shared held sample-peak marker. Click the rail
to reset only that band's level hold. Holds retain peaks between UI updates and
through pause, stop and band reordering. Turning the unit off clears them. Meter
resets do not change settings, Solo, undo history or the Out peak. Disabled band
rails retain the shared secondary-grey color.


Open the header's modulation dot to connect shared Envelope, LFO, or Macro sources from the same MIDI instrument. Select **Band N · Gain**, **Frequency**, **Q**, or **Output · Gain**. Gain amounts use dB, Frequency uses octaves, and Q uses Q units. Knob arcs and live indicators show modulation while the pointer retains the saved base value. Mode and power are discrete controls.

Removing a band removes its modulation connections. Other bands keep their connections when their displayed numbers change. Undo restores a removed band and its connections.

In the built-in effect, drag a band's label or unused strip space to reorder bands, as with oscillator strips. The insertion line shows the new position. Release within the band bank to apply the change. Escape, loss of focus, or release outside the bank cancels it. Drag near a lane edge to scroll. Knobs and Power keep their existing controls. Reordering retains each band's settings and modulation connections, changes the serial processing order, and supports Undo/Redo. In the standalone CLAP/VST3 plugins, use the same gesture to change visual strip order. Release within the bank to save it; Escape, focus loss or an outside drop cancels. The saved presentation order changes which strip appears first. Automation IDs, slot bindings and DSP processing order stay fixed.

The output strip uses the [standard effect output meter and held-peak indicator](output-meter.md). Click its rail to reset only the held peak.

This effect also has a [standalone CLAP/VST3 plugin](plugins.md).

## Solo

Click **S** beside a band's footer to isolate its frequency region. Bell and Notch
use Frequency and Q to define the audition region. Low shelf/Low pass audition
below Frequency; High shelf/High pass audition above it. Selected filters keep
their EQ processing. With band power off, the isolated region remains audible
without that filter's EQ processing. Out gain remains active.

Multiple S buttons combine their regions through complementary rejection filters;
overlapping regions are not added as duplicate audio. The transition is smoothed.
Normal EQ histories keep running on a separate path. Solo does not change saved
settings, modulation routes or undo history. Unit power off clears it. DAW export
ignores Solo; standalone offline behavior follows the [plugin guide](plugins.md).
