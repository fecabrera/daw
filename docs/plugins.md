# External VST3 and CLAP plugins

The desktop searches standard user and system plugin folders at startup. CLAP_PATH
adds CLAP search folders. Scans run in disposable child processes. A failed module
stays unavailable until it changes or you retry it with the [plugin CLI](cli/plugins.md).
Install plugins for the same processor architecture as the DAW.

Select the destination effects lane or regular instrument layer. Expand **VST3**
or **CLAP** inside the **Effects** catalog or **Instrument components** catalog.
Effects lists effect plugins; Instrument components lists instrument plugins.
Drag an effect into a track, Master, or instrument layer effect lane. The
insertion line shows where it will go. The effect appears after it loads.
You can also double-click a plugin or select it and press Enter. An instrument
replaces the layer's source and receives polyphonic notes from MIDI, beat patterns and live input. Its internal voices
belong to the plugin. Built-in per-note amplitude and envelope settings remain
dormant while an external instrument owns the layer.

Open **App Settings > Plugins** to inspect default folders, add or remove extra
VST3/CLAP folders, scan, rescan, retry failures or cancel a scan. Folder choices
are saved on this computer and shared with CLI scans and rendering. Default
folders stay enabled. The status list shows modules and scan failures. The
catalog's **Settings** button opens this page directly.

Mapped parameters use the standard unit controls in 90-point strips, with three
named control rows and a semibold footer. Audio inputs use the same row layout.
Search finds parameters by name
or module; page buttons show further controls. Knobs use the plugin's value domain,
steps and native value text. Read-only controls display output values. Hidden
parameters are omitted. Power uses the shared bypass ramp. Moves keep identities;
copies require a current opaque state capture and receive new identities.

Use **Open plugin editor** below Power in the unit header to open the plugin's
own interface. Its small maximize icon uses the filled modulation-button style.
The refresh icon below it is **Reload last captured plugin state**. Mapped and
native edits use the same project history. Reload creates a fresh instance from the last captured
state. State capture can briefly ramp a unit to dry audio or silence. Save and
export wait for current captures. A plugin without state support opens at defaults;
its panel shows **State recall unavailable** and exact copying is unavailable.

Open a parameter's modulation strip and choose a shared Macro, LFO, shared Envelope
or audio-derived source. Amounts add to the manual base with the common smoother.
Native editors display the plugin's own interpretation of modulation. Continuous
modulation does not become the saved manual value. Plugin-global parameters do not
accept per-note routes.

Audio sidechains are separate from modulation followers. In the plugin's audio
input strip, select an auxiliary input, source track, pre/post-fader tap, channel
mapping, gain and power. The host aligns the input with the plugin's main audio.
Disconnected inputs receive silence. Disabled routes still participate in cycle
checks. Invalid feedback routes are rejected as one edit.

Projects retain plugin identity, controls, connections and opaque state if a
plugin is unavailable. Desktop playback uses a delayed dry effect or a silent
instrument until the plugin becomes available. Installing the same plugin restores access. A changed bus
layout leaves incompatible audio inputs disconnected. A changed parameter layout
reports an error instead of guessing a mapping. Rendering fails for required
missing plugins unless the CLI's explicit degraded option is selected.

The host supports mono/stereo buses, one output bus, up to 32 input buses and one
note input. VST3 initialization requests mono/stereo bus arrangements before
validating actual channel counts. Plugins with surround defaults can load when
they accept mono/stereo. Layouts that remain unsupported report an error. Linux native editors require
X11 or XWayland support. AU hosting and cross-architecture bridging are outside
this feature. Active plugins run inside the DAW process and can crash or hang it.
Native device, listening and Windows/Linux editor acceptance are tracked in the
[validation record](../specs/validation/external-plugin-hosting-integration-2026-11-17.md).

Scanning runs in a child process of the application. No separate scanner install
is required. A plugin crash or timeout during scanning affects that candidate;
loaded plugins still run inside the DAW during playback.

A native activation error identifies the module path and format. A VST3 controller
that does not implement component-state synchronization does not prevent processor
state recall. Actual processor-state restore errors still stop activation.
