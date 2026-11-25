# External VST3 and CLAP plugins

## Install and scan plugins

The desktop searches standard user and system plugin folders at startup. CLAP_PATH
adds CLAP search folders. Install plugins for the same processor architecture as
the DAW. See [Compatibility and limits](#compatibility-and-limits) for supported setups.

Open **App Settings > Plugins** to inspect default folders, add or remove extra
VST3/CLAP folders, scan, rescan, retry failures or cancel a scan. Folder choices
are saved on this computer and shared with CLI scans and rendering. Default
folders stay enabled. The status list shows modules and scan failures.

Scans run in disposable child processes. No separate scanner install is required.
A scan crash or timeout affects that plugin candidate. A failed module stays
unavailable until it changes or you retry it with the [plugin CLI](cli/plugins.md).

## Add a plugin

Select the destination effects lane or regular instrument layer. Expand **VST3**
or **CLAP** inside the **Effects** catalog or **Instrument components** catalog.
Effects lists effect plugins; Instrument components lists instrument plugins.
Drag an effect into a track, Master, or instrument layer effect lane. The
insertion line shows where it will go. The effect appears after it loads.
You can also double-click a plugin or select it and press Enter. An instrument
replaces the layer's source and receives polyphonic notes from MIDI, beat patterns and live input. Its internal voices
belong to the plugin. Built-in per-note amplitude and envelope settings remain
dormant while an external instrument owns the layer.

## Plugin controls and editor

### Search and adjust mapped controls

Mapped parameters and audio inputs use the standard unit control layout. Search
by parameter name or module, and use the page buttons to see more controls. Knobs
use the plugin's value range, steps and labels. Read-only controls display output
values. Hidden parameters stay out of the panel. Power fades the bypass transition.

### Open the native editor

Click the maximize icon labeled **Open plugin editor** below Power in the unit
header to open the plugin's own interface. Mapped and native edits use the same
project history.

### Reload or copy plugin state

Click the refresh icon below **Open plugin editor** to **Reload last captured
plugin state**. Reload starts the plugin again with its last captured settings.

Moving a plugin keeps its identity. A copy requires a current state capture and
gets a new identity. During state capture, an effect can briefly fade to its
unprocessed signal, and an instrument can briefly fade to silence. Save and export
wait for capture to finish. A plugin without state support opens at its defaults.
Its panel shows **State recall unavailable** and exact copying is unavailable.

## Modulation and audio sidechains

### Modulate a parameter

Open a parameter's modulation strip and choose a shared Macro, LFO, shared Envelope
or audio-derived source. Amounts add to the manual base with the common smoother.
Native editors display the plugin's own interpretation of modulation. Continuous
modulation does not become the saved manual value. Plugin-global parameters do not
accept per-note routes.

### Connect an audio sidechain

Audio sidechains are separate from modulation followers. In the plugin's audio
input strip, select an auxiliary input, source track, pre/post-fader tap, channel
mapping, gain and power. The host aligns the input with the plugin's main audio.
Disconnected inputs receive silence. Disabled routes still participate in cycle
checks. Invalid feedback routes are rejected as one edit.

## Plugin delay compensation

Plugin delay compensation (PDC) uses the latency each plugin reports to keep
tracks, groups, sends, and sidechains aligned during playback and export. PDC
works automatically. Bypassing a plugin keeps its delay in the signal path so
the timing stays aligned.

If a plugin reports a latency change during playback, the DAW suspends the
plugin and prepares it again. The new compensation takes effect when the
updated audio mix takes over. You can leave playback running. You may briefly
hear the signal without the effect, or the plugin instrument may go silent
while the update completes.

## Missing plugins and activation errors

Projects retain plugin identity, controls, connections and opaque state if a
plugin is unavailable. Desktop playback uses a delayed dry effect or a silent
instrument until the plugin becomes available. Installing the same plugin restores access. A changed bus
layout leaves incompatible audio inputs disconnected. A changed parameter layout
reports an error instead of guessing a mapping. Rendering fails for required
missing plugins unless the CLI's explicit degraded option is selected.

A native activation error identifies the module path and format. A VST3 controller
that does not implement component-state synchronization does not prevent processor
state recall. Actual processor-state restore errors still stop activation.

## Compatibility and limits

The host supports mono/stereo buses, one output bus, up to 32 input buses and one
note input. VST3 initialization requests mono/stereo bus arrangements before
validating actual channel counts. Plugins with surround defaults can load when
they accept mono/stereo. Layouts that remain unsupported report an error. Linux native editors require
X11 or XWayland support. AU hosting and cross-architecture bridging are outside
this feature.

Active plugins run inside the DAW process and can crash or hang the app.
Native device, listening and Windows/Linux editor acceptance are tracked in the
[validation record](../specs/validation/external-plugin-hosting-integration-2026-11-17.md).
