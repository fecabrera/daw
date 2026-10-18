# Independent implementation record

Implementation record updated: 2026-10-18. Status: experimental.

The mandatory rule in `specs/audio/timestretch.md` applies: under no circumstances take code directly from Rubber Band. Copies, ports, translations, adaptations, mechanical rewrites, and indirect reuse are prohibited. No Rubber Band DSP implementation source was consulted for this implementation. The new engine and tests were written in this task from the mathematical design below. This is a provenance record, not a certification of formal clean-room development or legal clearance.

## Research inputs

- Julius O. Smith, [Spectral Audio Signal Processing: Phase Vocoder](https://www.dsprelated.com/freebooks/sasp/Phase_Vocoder.html): STFT analysis, instantaneous frequency from phase derivatives, FFT resynthesis.
- Bernardini, De Götzen, and Arfib, [Traditional (?) implementations of a phase-vocoder: the tricks of the trade](https://www.dafx.de/paper-archive/2000/pdf/Bernardini.pdf), DAFx 2000: windowing, phase, synthesis, and phase locking. Used as research context; no accompanying code was imported.
- Rubber Band [public API documentation](https://breakfastquay.com/rubberband/code-doc/classRubberBand_1_1RubberBandStretcher.html): reference adapter only, including offline study/process/retrieve and reported engine version. No implementation source links were followed.

Prior exposure: the existing DAW Rust adapter was read. Earlier specification work inspected the Rust binding's public option mappings/API declarations. The conversation contains public Rubber Band behavior descriptions. These facts rule out claiming that all participants had no prior exposure. No Rubber Band implementation material was used to derive the new DSP.

## Independent design

`src/spectral.rs` implements centered Hann-window STFT and weighted overlap-add in f64. For each bin k, expected angular frequency is 2πk/W. Measured frequency is expected frequency plus the principal phase residual divided by the actual input-center advance. Accumulate the difference between output and input phase advances. Apply a unit complex rotation to the original channel spectra; reconstruct negative frequencies by conjugation.

Stereo analysis sums per-channel complex cross-products between consecutive spectra. It never sums L/R samples, so an anti-phase signal does not cancel. Both channels receive the same rotation. Local spectral maxima define nearest-peak regions for identity phase locking. Both analysis and synthesis hops are bounded by W/8. Window length scales with sample rate. Windows extend beyond output bounds and normalize by their squared overlap weights, stored in one periodic hop-sized table; source endpoints use zero extension. Exact constant signals use a direct DC path. There is no resampling fallback. Identity requests bypass DSP after validation and preserve bits. Tests independently generate sines and use interpolated crossings to estimate pitch.

The analysis pass selects a short (about 2–3 ms) window for broadband material, using energy-weighted geometric/arithmetic spectral flatness above 0.15. Tonal material uses a longer (about 40–46 ms) window. This independent design choice improves attack localization in noise-like material. Selection applies to the complete source: mixed tonal/percussive material still requires listening evaluation. Neither audio similarity nor quality parity with Rubber Band is claimed.

## Phase continuity and attack preservation

The engine carries each bin’s applied locked synthesis rotation into the next frame. This keeps phase continuity when spectral peaks change and avoids overlap cancellation without output normalization, gain matching, limiting, or clipping.

Research context: Axel Röbel, [A new approach to transient processing in the phase vocoder](https://www.dafx.de/paper-archive/2003/pdfs/dafx32.pdf), DAFx 2003. The paper discusses phase coherence at attacks and the problems with early phase resets. No source code or Rubber Band implementation material was consulted. The following energy detector and guarded time map were independently written here; they are not the paper's per-bin detector.

`src/transients.rs` measures linked channel power in approximately 1 ms blocks. An attack exceeds six times the preceding eight-block mean and twice the maximum of the preceding two blocks, with a 30 ms refractory period. These relative comparisons preserve amplitude scaling and polarity invariance. During expansion only, detected onset positions remain anchored to the requested global ratio. Small source regions around them run at unit speed with original spectral phases. Guards cover half an analysis window plus 2 ms before and up to 10 ms after the onset; the extra post-onset time is capped at one quarter window to retain broadband decay energy; neighboring guards are capped at one third of their source separation. The intervening segments absorb the extra duration. This is a monotonic piecewise-linear time map, with rounded source centers and exact requested output length. Repeated centers retain the previous frequency estimate. Compression uses a linear time map with the same phase-continuity rule.

The analysis also measures flatness within the band containing the middle 90% of spectral energy when it spans at least one eighth of the analyzed bins. This avoids classifying band-limited cymbal/noise material as tonal solely because its low or high frequency margins are empty. Narrow tonal peaks retain the long-window path.

This is transient protection, not whole-file loudness normalization. Short pulses can have lower whole-file RMS after expansion because their attacks retain their duration while the spacing increases. The detector can miss attacks masked by a louder sustained signal. Dense events shorten the guard regions. Source-wide window selection remains a limitation, and sustained cymbal tails still need listening assessment. No subjective equivalence with R3 is claimed.

## Offline pitch composition

Research input: Julius O. Smith, [Windowed Sinc Interpolation](https://www.dsprelated.com/freebooks/pasp/Windowed_Sinc_Interpolation.html), mathematical band-limited interpolation and the low-pass requirement when reducing sample count. This extension uses those sampling equations. No third-party implementation, code, coefficient table, or test fixture was copied. No Rubber Band implementation source was inspected. The existing independently written reference adapter was read and now supplies its requested factor through the same public constructor.

Independent design: convert semitones `a` to frequency factor `s = 2^(a/12)`. Keep the final rational frame count `M`, stretch to `K = max(1, round(M*s))` with the existing linked processor, then sample that intermediate at positions `j*K/M`. The effective pitch error from integer rounding decreases with result length; very short files cannot establish meaningful pitch. Borrow input when staging needs no duration change. Zero pitch delegates to the original implementation without interpolation.

The new `pitch.rs` independently implements a centered Blackman-windowed sinc, cutoff `0.94/max(1,K/M)`, radius `ceil(32/cutoff)`, 1,025 stored fractional phases with linear coefficient interpolation, and f64 accumulation. These numerical choices, table layout, progress stages, memory accounting, and tests were designed here. Normalize each kernel's DC sum, never the processed signal's loudness. Both channels share identical weights and source positions. Bounds use zero extension; no extra filter delay is returned. This finite filter can attenuate the upper transition band and ring at boundaries. Formants shift with the signal. Full listening/performance acceptance and future DAW consumer design remain open. The existing stretch revision remains 2; the added optional pitch path has revision 1.

## Dependencies and reuse

Only RustFFT 6.4.1 is a direct DSP dependency (MIT OR Apache-2.0). Its resolved dependencies are num-complex, num-integer, num-traits, primal-check, strength_reduce, transpose, and build dependency autocfg. These are permissively licensed. Default RustFFT CPU acceleration features are enabled. Dependency license texts are in `licenses/`; the lockfile records resolved versions. No third-party time-stretch implementation or test fixture was copied. Original engine/tests are BSD-2-Clause, with the local LICENSE.

Rubber Band is not a core dependency. The independently written CLI adapter calls separately licensed Rubber Band R2/R3 through rubberband 0.1.5 / rubberband-sys 0.1.4 / native 4.0.0. Those reference dependencies remain GPL-2.0-or-later and are optional tool dependencies; enabling them does not make that combined tool permissively licensed. Their notices ship in the tool's `licenses/` directory.

## Full-range pitch composition — 2026-10-18

The pitch input range extends to ±127 semitones. The independent composition splits a large offset into passes of at most 24 semitones, applies duration only in the first pass, and reuses the existing spectral processor and sinc filter. Raising first resamples to a shorter buffer, then restores target duration; lowering retains stretch then resample. Downward passes place the remainder first. These choices follow the multiplicative pitch-factor identity and tests of low MIDI fundamentals; no new external algorithm/source was consulted. Direct bounded-pass analysis of an 8.18 Hz input, or a final downward pass analyzing near DC, failed tone checks. The new ordering passes the endpoint fixtures without adding a separate oscillator or signal-specific correction. Working-memory estimation includes the retained previous-pass buffer. Pitch revision is 2; offsets within ±24 keep their previous numerical path.

The separately licensed native adapters use only the public Rubber Band API with the shared pass schedule. No Rubber Band implementation, tests, tables or pseudocode were inspected or reused for this extension. Extreme-range quality remains limited by spectral processing, filtering, source duration and the audible/output frequency band.
