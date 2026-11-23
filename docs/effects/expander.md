# Expander

Use Expander to turn down quiet material between wanted sounds. Add it from
Dynamics in the Effects catalog. The Curve graph shows how input level maps to
output level. The handle sits halfway along the knee. Drag it horizontally
to set Threshold and vertically to set Ratio.

- **Threshold** sets where expansion begins. **Ratio** controls how much quieter
  the signal gets below the threshold. At 4:1, a drop of 6 dB below a hard knee
  becomes a drop of 24 dB at the output.
- **Curve** shapes the response below the knee. Positive values increase
  attenuation. Negative values soften it.
- **Attack** sets how quickly the Expander opens as the signal gets louder.
  **Release** sets how quickly it closes as the signal gets quieter.
  **Knee** sets the width of the transition around Threshold.
- **Out** contains output gain, Dry/Wet, and Peak or RMS detection. The Curve
  graph shows input history and gain reduction. The Out strip shows output level.

The built-in unit supports modulation. Standalone builds are available in
CLAP, VST3, and macOS AUv2 formats.
