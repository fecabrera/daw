# Loudness Meter

Add **Loudness Meter** from **Analysis** in the Effects catalog. Use it on a track,
group, instrument bus or Master to measure the audio at that position in the
chain. The meter passes audio through unchanged. It adds no audio delay or tail.
It is also available as a stereo CLAP or VST3 plugin.

| Strip | Readings or controls |
| --- | --- |
| Options | Target, TP quality and Clock |
| Loudness | Momentary, Short term and Integrated loudness in LUFS |
| Maxima | Highest momentary and short-term loudness, and highest true peak |
| Range | Loudness range (LRA), and its low and high bounds |
| Dynamics | Current true peak, peak-to-short-term ratio (PSR), and peak-to-loudness ratio (PLR) |
| History | Momentary or Short term graph for the latest three minutes |
| Out | Sample-peak rail, measured duration, Hold/Resume and Reset |

Momentary measures the latest 400 ms. Short term measures the latest three seconds.
Their windows start with silence until enough audio arrives. Integrated measures
average loudness after excluding very quiet audio. LRA measures variation between
the lower and upper parts of the short-term loudness distribution. It needs at
least three seconds of audio. Silence and readings without enough audio show −∞.

PSR compares the latest 100 ms true peak with short-term loudness. PLR compares
the highest true peak with integrated loudness. Both ratios have a minimum of zero.
True peak estimates peaks between samples; the Out rail measures sample peaks.

| Option | Choices | Default |
| --- | --- | --- |
| Target | Off, −14 LUFS, −23 LUFS | −14 LUFS |
| TP quality | ECO, HQ | HQ |
| Clock | Transport, Continuous | Transport |

Target draws a reference line in History. It does not change gain. HQ estimates
more positions between samples than ECO and uses more CPU. Changing TP quality or
Clock starts a new measurement.

With **Transport**, Pause retains the current readings. Stop retains the summary;
the next Play starts a new measurement. A seek or loop wrap also starts a new
measurement. **Continuous** retains the measurement across transport changes until
Reset. The DAW measures processed audio only; it does not add time while playback
is idle. In a plugin, Continuous also measures audio blocks while the host is
stopped. A host that supplies no transport information is measured continuously.

**Hold** freezes analysis. **Resume** continues the same measurement and excludes
the held interval. **Reset** clears readings and History without changing audio.
Changing the History metric, Hold and Reset do not change the project or create
parameter automation. Analysis continues when the meter panel is hidden.

Header power turns analysis off, clears readings and empties the rail. Turning it
on starts a fresh measurement. Projects and presets save options, not readings,
History or Hold. Click the Out rail to reset its held sample peak independently.

If the meter shows **Incomplete**, Reset and measure again. This can occur if
analysis cannot keep up, invalid input reaches the analyzer, or loudness exceeds
its supported measurement range. The audio path remains unchanged.

This is a stereo meter with estimated true peaks. It has not been certified for
broadcast delivery. Dialogue-only and multichannel loudness modes are unavailable.
