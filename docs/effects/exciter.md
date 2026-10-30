# Multiband Exciter

Multiband Exciter adds harmonics to selected frequency bands. Add it from **Distortion → Multiband Exciter** to a track, group, Master, instrument layer, or Output.

## Frequency ranges

Four bands start at **below 150 Hz**, **150 Hz–2 kHz**, **2–6 kHz**, and **above 6 kHz**. The first band includes the lowest frequencies; the last extends to the project's Nyquist frequency, which is half its sample rate.

Use the knobs in **Ranges**, from top to bottom, to move the boundaries between adjacent bands. Boundaries stay ordered, between 20 Hz and 20 kHz, with at least 1 Hz between boundaries. A boundary cannot move past its neighbors. Hover a boundary to identify its neighboring bands. Scroll Ranges when more controls are present.

Use **Plus** to split the widest frequency range. The new band inherits that range's Mode, Drive, Mix, Level, and Power values. Existing modulation connections stay with the original band. Right-click a band's knob or Power button and choose **Remove band** to merge its range into the lower band, or into the next band when removing B1. At least one band remains. More bands use more CPU and memory. Removing a band also removes its modulation connections.

## Band controls

| Control | Range/options | Default | Use |
| --- | --- | --- | --- |
| Mode dropdown | Soft, Tube, Tape, Hard | Soft | Select the saturation character. |
| Drive knob | −36 to +36 dB | 0 dB | Increase or reduce distortion intensity. |
| Mix knob | 0–100% | 0% | Blend the original band with its excited signal. |
| Level knob | Silence to +12 dB | 0 dB | Adjust the whole band's level after its Mix control. |
| Power + label | On/Off | On | Bypass that band's excitation and Level while retaining its original audio. |
| S | Solo | Off | Hear this band while silencing the other bands in this effect. |

The footer identifies bands as **B1**, **B2**, and so on. Hover Power to see the full frequency range. Several bands can be soloed together. Solo works at 0% band Mix and 0% global Dry/Wet, so you can audition the original range too.

**Solo affects live project playback only.** It does not change project data, Undo/Redo, or export. Its state clears when you open another project or remove the band.

Soft gives gentle saturation. Tube adds asymmetric coloration, including even harmonics. Tape softens peaks and smooths the added upper harmonics. Hard gives sharper clipping. Drive increases distortion without adding the same input-gain boost to the output. Strong drive can reduce peaks; use Level to adjust the balance.

A band's range selects the audio feeding its saturation. New harmonics can extend above that range.

## Output and use

The **Out** strip has **Level** and **Dry/Wet** knobs, plus the [standard level meter and held-peak indicator](output-meter.md). Level adjusts the complete output, including at 0% Dry/Wet. Click the meter rail to reset its held peak.

1. Solo a band and adjust its range.
2. Choose Mode and raise Drive.
3. Raise Mix from 0% until the added harmonics suit the sound.
4. Release Solo, then adjust the band's Level and the global Dry/Wet.
5. Match the final level with Out Level.

The crossover filters retain a flat combined level response with neutral band controls, but change phase. At 0% global Dry/Wet, no excitation or band Level adjustment is applied; the crossover phase remains. Header Power bypasses the complete effect and retains its settings.

The effect uses 4× oversampling to reduce aliasing and keeps a fixed processing delay through power and mix changes. It does not limit final peaks.

## Modulation and editing

Each band's Drive, Mix and Level, plus Out Level and Dry/Wet, accept the shared modulation controls. Mode and crossover controls are not modulation destinations. Removing or copying bands preserves the other bands' connections. See [modulation](../modulation/README.md) and [unit editing](../editing/units.md) for routing, selection, copying, collapse and compact mode.

This release provides the built-in effect. Separate CLAP/VST3 products are not included.
