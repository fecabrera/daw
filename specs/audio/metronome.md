# Playback metronome

## Accepted behavior

Place a borderless Metronome toggle immediately after Tempo in the shared playback
transport toolbar in all four views. Use Lucide `Metronome`, the existing 24-point
transport button and 16-point artwork, seven-point spacing, shared hover overlay,
and Loop's gold selected fill with white active artwork in both themes. Retain
transport input/dialog/job guards, tooltip and accessibility name “Metronome”.

Default off. Keep the choice in app-session UI state, including across view and
project changes. Enabling it while stopped does not create an output device or
start playback. Existing Play eligibility and transport end behavior remain.
Toggling does not change the project, Undo/Redo, save state, tempo or playhead.
The first subsequent Play transfers the choice to a recreated output stream.

Click once per quarter-note beat, with a higher accent at the first beat of each
4/4 bar. Anchor beat zero to project frame zero. Derive each beat boundary from
absolute project frame and tempo, rounded independently to the nearest sample;
do not accumulate a rounded beat interval. A loop follows the project's musical
position, including when its start lies between beats. Pause, Stop, playback end
and disk buffering fade the click to silence. Seeks, loop wraps and active-renderer
tempo changes retain grid alignment with a 5 ms transition.

## Output ownership

`daw-output::metronome` owns a bounded procedural click and its power/transition
state. `AudioOutput::set_metronome` publishes one scalar through an atomic flag;
no renderer replacement, channel capacity or preparation job is required. Read it
at the device buffer boundary. State survives renderer replacement within the
callback and is recreated at project rate when device output is recreated.

Mix the centered click at project rate after the project Master processing/gain,
alongside audition, and before the existing device-rate converter. The output
meter includes the click; track meters do not. Project gain/effects do not process
the monitor click. Export and headless project rendering retain the project mix.

Use the renderer's actual audible frame after `next_sample`: its next playhead
minus one only while still playing and not buffering. This follows normalization
at loop entry and latency alignment without advancing an independent timer.
Use the active renderer's tempo, including during background publication.

Clicks use 20 ms decaying sine bursts, 5 ms attack, 2.4 kHz / 1.264911064 amplitude scale for
bar accents and 1.6 kHz / 0.885437745 for other beats. These scales include a
10 dB increase over the previous 0.40 / 0.28 levels. Power changes and discontinuities
use the existing 5 ms rate-based convention. No volume/count-in/subdivision controls
or project-format changes are introduced. Extremely high valid tempos are bounded
to at most one beat per sample. The callback adds no allocation, locking, file I/O,
logging, background work or final-owner retirement.

## Validation

Check button placement/style and state in both themes and all views, project/save
isolation, fractional-tempo drift at every project rate, bar accents, power/pause
fades, loop/seek synchronization, device-rate conversion and callback heap guards.
Verify that offline audio is unchanged. Native appearance and audible click level
remain separate from headless interaction and sample checks.
