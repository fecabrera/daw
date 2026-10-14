# Limiter

Implemented on 2026-10-14. `daw-limiter` owns portable stereo DSP. Saved settings and the effect descriptor belong to `daw-unit-core`; track/Master edits use core validation. Desktop playback, direct rendering, and CLI export use the same engine instance.

## Controls

Two standard 90-point strips use three shared knob rows and the regular unit header/footer. Limit contains Input, Release, Lookahead. Out contains Ceiling, Mode, Routing. A thin read-only gain-reduction rail sits below the header without increasing the shared unit height. The rail shows 0–24 dB; its tooltip and accessibility value retain larger reductions. Compact mode hides knob rows and the meter. Horizontal collapse, enable state, instance IDs, catalog actions, selection, reordering, and deletion use the shared unit shell and effects lane.

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

The ceiling applies to the unit output when enabled and settled. During a ceiling or mode change, previously emitted audio cannot be revised; allow the control ramp and reconstruction history to settle. Off crossfades to unity-gain dry audio at the same delay and retains settings/history. Any subsequent effect, track gain, pan, or Master gain can change the level after this unit. Nonfinite input becomes silence; finite extreme values stay finite. Gain-reduction monitoring uses a prepared atomic handle, shared by stable instance ID across compatible edits.

## Latency and transport

Each instance reports a fixed `ceil(sample_rate_hz / 100) + 32` frames, including bypass and Peak mode. This is the maximum 10 ms lookahead buffer plus one reconstruction block. Fixed latency lets Lookahead, Mode, and enable changes retain alignment without reallocating buffers or moving the timeline.

The engine delays shorter track chains to the longest track chain, then accounts for serial Master latency. All alignment buffers are prepared on the control thread. The renderer primes the chains and reads source frames ahead, so output frame positions, project length, and export length stay unchanged. Offline rendering ignores loop selection. Live rendering wraps the future source position through the loop, ends MIDI notes at the source wrap, and keeps effects running. Effect transport positions account for upstream limiter delays.

Seek and Stop reset and prime processing; Pause freezes the compensated pipeline and fades its held output, retaining note/effect history for resume. Compatible replacements retain delays, DSP history, voices, meters, and ramps. A changed latency topology resets and primes the prepared path at the current playhead. Reset, priming, retargeting, looping, and sample processing allocate nothing and perform no I/O or locking in the callback. Native callback timing and device behavior still need platform verification.

This supersedes the earlier zero-latency-only scope for built-in effects. Arbitrary third-party/plugin latency is outside this implementation.
