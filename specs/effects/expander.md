# Expander

Expander is a downward dynamics effect with type ID `daw.expander` and project
settings version 1. It uses the Compressor control ranges and defaults. Its
shared editor shows Curve, Expand, Response, and Out in the same geometry as
[Compressor](compressor.md), with the standard handles, meters, and history.

The graph handle sits halfway along the visible soft-knee curve, measured in
screen space. The output axis extends when needed to show the entire knee.
A hard knee places the handle at the threshold corner. Dragging still changes
Threshold horizontally and Ratio vertically.

The linked stereo Peak or RMS detector drives the same gain on both channels.
Peak mode follows the larger channel peak with a 30 ms decay half-life so
periodic signals retain their level through zero crossings.
At a hard knee with Curve at zero, output below threshold is
`threshold + ratio * (input - threshold)`. Above threshold, the transfer is
unity before output gain. Gain reduction is limited to 120 dB so the envelope
opens from a bounded level after silence. With a soft knee, the transition spans
half the knee width on either side of threshold. Ratio 1 is unity for every knee and
Curve value. Below the lower knee edge, positive Curve adds attenuation and
negative Curve reduces it. The gain function joins the soft knee continuously
with the same slope. Attack controls opening and Release controls closing.
Dry/Wet, output gain, smoothing, bypass, and the eight modulation destinations
use the Compressor processing path. Expander adds no latency or audible tail.

Project state uses an independent type ID. The CLAP, VST3 and macOS AUv2 products
share the typed model and editor code with Compressor and register independent
product identities. Plugin state starts at version 1 with the `DAWEXP01` magic.
