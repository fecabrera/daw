# Limiter

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/effects/limiter.md).

Implemented on 2026-10-14. `daw-limiter` owns portable stereo DSP. Saved settings and the effect descriptor belong to `daw-unit-core`; track/Master edits use core validation. Desktop playback, direct rendering, and CLI export use the same engine instance.

## Controls

Two standard 90-point strips use three shared knob rows and the regular unit header/footer. Limit contains Input, Release, Lookahead. Out contains Ceiling, Mode, Routing. A thin read-only gain-reduction rail sits below the header without increasing the shared unit height. The rail shows 0–24 dB; its tooltip and accessibility value retain larger reductions. Whole-unit off clears the meter and suspends atomic publication through the [shared effect-meter lifecycle](../architecture/modular-audio-units.md#effect-meter-lifecycle). The panel masks queued readings through `meters::unit_reading`. Re-enabling resumes publication while preserving detector, delay, and bypass-ramp behavior. Compact mode hides knob rows and the meter. Horizontal collapse, enable state, instance IDs, catalog actions, selection, reordering, and deletion use the shared unit shell and effects lane.

| Setting | Default | Accepted range/options |
| --- | --- | --- |
| Input | 0 dB | −24 to +24 dB |
| Release | 100 ms | 1–2000 ms |
| Lookahead | 1 ms | 0–10 ms |
| Ceiling | −1 dB | −24 to 0 dB |
| Mode | Peak | Peak / True Peak (compact label: True) |
| Routing | L/R | L/R / M/S |

Mode and Routing use the shared bounded two-way knob with neutral arcs and accent-colored selected labels. Double-click restores each setting's default. Header power and collapse are saved separately. State version 1 uses type ID `daw.limiter`. Invalid/nonfinite parameters reject the complete track or Master edit. Future versions retain their opaque payload as missing effects and bypass processing.

## Processing

Input gain precedes limiting. L/R uses a linked peak detector and one gain for both channels. M/S uses `M=(L+R)/2` and `S=(L−R)/2`, independent component envelopes, and `L=M+S`, `R=M−S`. A final linked L/R guard enforces the sample ceiling after recombination. Routing changes blend the continuously running branches over 5 ms.

A preallocated maximum tree finds peaks between the delayed output sample and its Lookahead endpoint. The attack follows that peak with a time derived from Lookahead; zero lookahead uses instantaneous attack. Release restores gain exponentially. The final linked guard catches any residual sample overshoot. Input, release, lookahead, ceiling, routing, and enable targets ramp over 5 ms. Unrelated edits retain active ramps, delay contents, and detector history.

True Peak adds an eight-phase, 64-tap windowed-sinc reconstruction guard. It examines each pending 32-frame block and its complete FIR support against the committed output history, then applies one linked block gain. It reserves 0.175 dB of reconstruction headroom. This limits an oversampled estimate; it is not a claim of ITU certification or an exact bound for every possible analog reconstruction. Tests use an independent 16×/128-tap reconstruction, including near-Nyquist stereo bursts. The [ITU true-peak recommendations](https://www.itu.int/rec/R-REC-BS.1770/en) explain oversampling estimation and under-read error. The coefficients here are generated from the sinc/window equations; they are not copied from a third-party implementation.

The ceiling applies to the unit output when enabled and settled. During a ceiling or mode change, including Ceiling modulation, previously emitted audio cannot be revised; allow the control ramp and reconstruction history to settle. Off crossfades to unity-gain dry audio at the same delay and retains settings/history. Any subsequent effect, track gain, pan, or Master gain can change the level after this unit. Nonfinite input becomes silence; finite extreme values stay finite. Gain-reduction monitoring uses a prepared atomic handle, shared by stable instance ID across compatible edits.

## Latency and transport

Each instance reports a fixed `ceil(sample_rate_hz / 100) + 32` frames, including bypass and Peak mode. This is the maximum 10 ms lookahead buffer plus one reconstruction block. Fixed latency lets Lookahead, Mode, and enable changes retain alignment without reallocating buffers or moving the timeline.

The engine delays shorter track chains to the longest track chain, then accounts for serial Master latency. All alignment buffers are prepared on the control thread. The renderer primes the chains and reads source frames ahead, so output frame positions, project length, and export length stay unchanged. Offline rendering ignores loop selection. Live rendering wraps the future source position through the loop, ends MIDI notes at the source wrap, and keeps effects running. Effect transport positions account for upstream limiter delays.

Seek and Stop reset and prime processing; Pause freezes the compensated pipeline and fades its held output, retaining note/effect history for resume. Compatible replacements retain delays, DSP history, voices, meters, and ramps. A changed latency topology resets and primes the prepared path at the current playhead. Reset, priming, retargeting, looping, and sample processing allocate nothing and perform no I/O or locking in the callback. Native callback timing and device behavior still need platform verification.

This supersedes the earlier zero-latency-only scope for built-in effects. Arbitrary third-party/plugin latency is outside this implementation.


## Modulation — 2026-10-17

Input, Release, Lookahead, and Ceiling consume the [shared modulation contract](../modulation/contract.md). Connections accept shared Retrigger/Legato Envelope, LFO, and Macro sources from the same MIDI instrument, including Main. Per note sources cannot feed a track effect. Audio tracks and Master have no instrument providers; unassigned controls remain available. Mode and Routing stay discrete, and gain reduction stays read-only.

| Destination | Saved amount per connection | Conversion and final bounds |
| --- | --- | --- |
| Input | ±24 dB | Add the smoothed offset to the ramped input gain; clamp to −24…+24 dB. |
| Release | ±6 octaves | Multiply the ramped duration by `2^offset`; clamp to 1…2000 ms. |
| Lookahead | ±10 ms | Add the smoothed offset to the ramped duration; clamp to 0…10 ms. |
| Ceiling | ±24 dB | Add the smoothed offset to the ramped ceiling; clamp to −24…0 dB. |

Settings and DSP declare the same four typed parameters in `ModulationParameter::LIMITER` order and Shared scope. The host prepares routes and calls `process_modulation` with cached source values. The common resolver sums signed contributions before the shared 1 ms additive smoother. Source/route amount and power changes retain their common 5 ms ramps. Input/Ceiling gain offsets and Release octave offsets are limited to ±96 before exponential conversion; final parameter bounds still apply. Base values and their existing 5 ms ramps remain separate from modulation. Neutral modulation preserves the unmodulated arithmetic and audio exactly.

Lookahead modulation selects a range in the existing maximum-peak tree. It does not resize a buffer or change the reported latency, including in bypass. Compatible modulation edits retain delay contents, detector/guard histories, stereo routing, meter handles, and in-flight ramps. Reset clears modulation smoothing and takes current route targets. The ordinary `process_sample` API remains the neutral-offset adapter. Playback, export, and CLI rendering use the same portable typed destination path. Source publication, scope checks, route validation/resolution, smoothing, history retargeting, and callback storage remain shared code; Limiter owns final bounds and protection DSP.

The secondary-color header dot opens the standard connection strips on the right. Reuse their Source/Destination selectors, signed destination-matched amount knobs, Power/removal/Plus/reordering, and the shared assignment marker, tooltip, possible-range arc, and moving processed-value dot. Input/Ceiling amounts use their native linear dB scales and 0.1 dB steps; Input retains 6 dB reference marks. Release uses the logarithmic octave adapter and 1/24-octave keyboard steps. Lookahead uses its native linear scale with 0.1 ms steps. Zero is neutral and double-click resets the amount to zero. Base pointers/readouts, the two regular strips, footer alignment, and gain-reduction rail retain their layout. Compact/collapsed controls follow the common shell.

Prepared atomic snapshots publish actual Input dB, Release ms, Lookahead ms, and Ceiling dB in slots 0–3; unused slots are NaN. Monitoring never advances DSP or estimates a source clock. State version 1 adds optional empty `modulations` and false `modulation_open` defaults. Validate supported parameters, bounded finite amounts, ordinary untargeted routes, unique valid route IDs, and same-instrument shared providers. Invalid edits fail atomically. Save/reopen, Save As, rate changes, effect transfers/copies, track duplication, and provider moves retain the common identity and persistence rules. All route vectors and snapshot handles are prepared outside the callback; sampling, reset, priming, and compatible retargeting allocate nothing and perform no I/O or locking.

Ceiling modulation retains the existing settled-control and reconstruction limits. The guard uses pending blocks and already emitted history, so a rapidly changing ceiling is not an instantaneous bound on earlier output. An additional 48 kHz near-Nyquist M/S burst check measured about 0.21 dB above the configured ceiling under independent 16× reconstruction. Fixed settings produce the same output. This records an existing estimator limit; see the [validation record](../validation/README.md#limiter-modulation--2026-10-17). The True Peak guard remains an oversampled estimate, without certification or an exact analog bound.
