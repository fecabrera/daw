# Independent implementation record

Implementation record updated: 2026-10-21. Status: experimental.

The mandatory rule in `specs/audio/timestretch.md` applies: under no circumstances take code directly from Rubber Band. Copies, ports, translations, adaptations, mechanical rewrites, and indirect reuse are prohibited. No Rubber Band DSP implementation source was consulted for this implementation. The new engine and tests were written in this task from the mathematical design below. This is a provenance record, not a certification of formal clean-room development or legal clearance.

## Research inputs

- Julius O. Smith, [Spectral Audio Signal Processing: Phase Vocoder](https://www.dsprelated.com/freebooks/sasp/Phase_Vocoder.html): STFT analysis, instantaneous frequency from phase derivatives, FFT resynthesis.
- Bernardini, De Götzen, and Arfib, [Traditional (?) implementations of a phase-vocoder: the tricks of the trade](https://www.dafx.de/paper-archive/2000/pdf/Bernardini.pdf), DAFx 2000: windowing, phase, synthesis, and phase locking. Used as research context; no accompanying code was imported.
- Rubber Band [public API documentation](https://breakfastquay.com/rubberband/code-doc/classRubberBand_1_1RubberBandStretcher.html): reference adapter only, including offline study/process/retrieve and reported engine version. No implementation source links were followed.

Prior exposure: the existing DAW Rust adapter was read. Earlier specification work inspected the Rust binding's public option mappings/API declarations. The conversation contains public Rubber Band behavior descriptions. These facts rule out claiming that all participants had no prior exposure. No Rubber Band implementation material was used to derive the new DSP.

## Independent design

`src/spectral.rs` implements centered Hann-window STFT and weighted overlap-add in f64. For each bin k, expected angular frequency is 2πk/W. Measured frequency is expected frequency plus the principal phase residual divided by the actual input-center advance. Accumulate the difference between output and input phase advances. Apply a unit complex rotation to the original channel spectra; reconstruct negative frequencies by conjugation.

Stereo analysis sums per-channel complex cross-products between consecutive spectra. It never sums L/R samples, so an anti-phase signal does not cancel. Both channels receive the same rotation. Local spectral maxima define nearest-peak regions for identity phase locking. Both analysis and synthesis hops are bounded by W/8. Window length scales with sample rate. Windows extend beyond output bounds and normalize by their squared overlap weights, stored in one periodic hop-sized table; source endpoints use zero extension. Exact constant signals use a direct DC path. There is no resampling fallback. Identity requests bypass DSP after validation and preserve bits. Tests independently generate sines and use interpolated crossings to estimate pitch.

Revision 2 selects a short (about 2–3 ms) window for broadband material, using energy-weighted geometric/arithmetic spectral flatness above 0.15. Tonal material uses a longer (about 40–46 ms) window. This independent design choice improves attack localization in noise-like material. Selection applies to the complete source: mixed tonal/percussive material still requires listening evaluation. Neither audio similarity nor quality parity with Rubber Band is claimed.

## Phase continuity and attack preservation

The engine carries each bin’s applied locked synthesis rotation into the next frame. This keeps phase continuity when spectral peaks change and avoids overlap cancellation without output normalization, gain matching, limiting, or clipping.

Research context: Axel Röbel, [A new approach to transient processing in the phase vocoder](https://www.dafx.de/paper-archive/2003/pdfs/dafx32.pdf), DAFx 2003. The paper discusses phase coherence at attacks and the problems with early phase resets. No source code or Rubber Band implementation material was consulted. The following energy detector and guarded time map were independently written here; they are not the paper's per-bin detector.

The revision-2 detector in `src/transients.rs` measures linked channel power in approximately 1 ms blocks. An attack exceeds six times the preceding eight-block mean and twice the maximum of the preceding two blocks, with a 30 ms refractory period. These relative comparisons preserve amplitude scaling and polarity invariance. During expansion only, detected onset positions remain anchored to the requested global ratio. Small source regions around them run at unit speed with original spectral phases. Guards cover half an analysis window plus 2 ms before and up to 10 ms after the onset; the extra post-onset time is capped at one quarter window to retain broadband decay energy; neighboring guards are capped at one third of their source separation. The intervening segments absorb the extra duration. This is a monotonic piecewise-linear time map, with rounded source centers and exact requested output length. Repeated centers retain the previous frequency estimate. Compression uses a linear time map with the same phase-continuity rule.

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

## Bass analysis and attack detection — 2026-10-21

Research input: Julius O. Smith, [Resolving Sinusoids](https://www.dsprelated.com/freebooks/sasp/Resolving_Sinusoids.html), describes the frequency-resolution tradeoff and the Hann main-lobe width. Only the mathematical discussion was used. A DAFx 2007 Olivier Derrien abstract on multi-scale Gabor analysis was also read during investigation as context; no paper implementation or code was used. No Rubber Band implementation source, tests, tables, comments, or pseudocode were inspected or reused. Native reference comparisons used only the existing independent public-API adapter.

Independent generated bass dyads exposed two coupled failures: close low fundamentals shared one spectral peak region, and the eight-block energy history mistook stationary beats for new attacks. A global longer-window experiment and an attack-disabled experiment each failed alone; the combined diagnostic restored partials but was unsuitable for percussion. Revision 3 instead selects a nominal 320 ms power-of-two window only for tonal sources with at least 50% non-DC power at/below 250 Hz. Broadband selection keeps precedence. The existing overlap schedule bounds both advances by one eighth of the selected window. Regression tests retain attack localization without increasing frame density separately. The 80-observation background spans several ordinary bass cycles and close-tone beat periods; startup divides by the populated count. Recent-rise and refractory checks, linked channel powers, guarded expansion timing, phase locking, and the pitch filter are reused. Thresholds and generated fixtures were designed here, not copied from a reference engine.

The existing 8 MiB spectral workspace allowance covers the bounded 65,536-frame window (less than 7 MiB of simultaneous stereo arrays/scratch excluding planner internals); the larger overlap/intermediate audio remains separately preflighted. Stretch revision is 3; pitch composition revision remains 2. Source-wide window selection and masked/dense attacks remain limits. Automated bass/drum checks do not establish subjective parity with any reference or full listening acceptance.

## Drum-aware bass selection — 2026-10-21

A subsequent zero-pitch 96→100 BPM drum report exposed a gap in revision 3: kick-dominated percussion passed its bass-energy condition, and compression does not use the expansion attack guards. Earlier mixed-drum tests exercised expansion and upward-pitch staging; they did not establish zero-pitch compression quality. Independently generated chirped kicks with weak seeded-noise hats reproduce increased pre-echo with the long window.

Revision 4 reuses the existing spectral-flatness equation and channel-power analysis in 250–1,000 Hz, 1–4 kHz, and 4 kHz–Nyquist bands. At least 0.1% of total power and energy-weighted band flatness above 0.15 veto the long bass window, retaining normal tonal resolution for mixtures. Global broadband selection still takes precedence. These bands, thresholds, and fixtures are an independent conservative design from the observed tonal/percussive ambiguity, not a reference implementation. No new external source was consulted; no Rubber Band implementation material was inspected or reused. References remain public-API calls only. The linked phase vocoder, overlap schedule, 80-block energy history, pitch filter, and allocation/preflight/cancellation paths are unchanged. Stretch revision is 4; pitch composition revision remains 2.

This source-wide choice can trade low-note separation for attack localization in mixed material. It does not solve every masked/dense attack, independently preserve frequency bands, or establish reference-quality parity. Compression pre-echo remains a listening target beyond the calibrated regression case.

## Experimental combined transient detector — 2026-11-08

Research context read in this conversation: Böck and Widmer, [Maximum Filter
Vibrato Suppression for Onset Detection](https://www.dafx.de/paper-archive/2013/papers/09.dafx2013_submission_12.pdf), DAFx 2013 (search-result abstract describing spectral trajectories and maximum filtering), and the previously recorded Röbel DAFx 2003 paper abstract. No SuperFlux implementation source or reference-engine DSP was inspected. Prior exposure remains as stated above; this is not formal independent-authorship certification.

The new detector is independently designed here: linked channel power FFTs in
nominal 16 ms windows every four energy observations; power growth and positive
three-bin-maximum novelty above 700 Hz; difference-energy block localization;
separate refractory checks followed by earliest-event merging with the unchanged
energy detector. Window/band/threshold/refinement choices and synthetic fixtures
are original engineering choices, not a transcription of a paper algorithm.
RustFFT 6.4.1 is reused through its existing public API; no new dependency or
license is introduced. Combined is opt-in and has a separate detector identity;
default revision-4 stretch/pitch paths retain the energy detector.

The first benchmark exposed spurious end-of-file spectral novelty on sustained
signals. Requiring a complete, unpadded window removes that artificial boundary
from the novelty test. This deliberately limits masked-edge detection rather
than changing evaluation annotations or suppressing false positives in metrics.
Initial and corrected CSVs remain in the ignored benchmark directory. No Rubber
Band implementation code, tests, comments, tables or pseudocode were read or reused.

## Revision 5 default and private real-audio auditions — 2026-11-08

The user explicitly requested making Combined the default. Default Request and
PitchRequest spectral processing now use Combined; the unchanged Energy detector
remains available for direct comparison. Stretch revision 5 invalidates prepared
R4 data. Existing DAW Integrated revisions 2–4 normalize to 5 and rebuild through
the existing project-open workers with a warning. Rubber Band remains the app's
initial backend and no native reference implementation material was inspected.

Default-adoption regressions exposed two issues not established by the earlier
opt-in fixture panel: pitch memory admission omitted the extra event capacity,
and a spectral candidate at a kick cutoff shortened adjacent guards and increased
attack residual from the allowed <0.02 to about 0.0526. The estimate is now a shared
48 bytes per maximum event (conservative for either mode). Combined retains Energy
timestamps within 30 ms, and additional spectral candidates require sustained
first-difference power growth over the following 1–8 ms against the preceding
8 ms, excluding the first impulse block. The cutoff regression remains unchanged
and an explicit detection regression was added. These are independent fixes from
local test failures, not reference-engine code or threshold relaxation. The final
detector comparison identity is energy-spectral-v4; earlier local v1/v2/v3 artifacts
are historical and must not be confused with final R5 auditions.

The real-audio panel reads six recordings from the user's Splice library with
explicit read-only authorization. All derived files, source paths, hashes, label
mappings and blank ratings remain in ignored work storage. No private recording
is redistributed or added to the repository. Source contents are SHA-256 checked
before and after. The listening tool and its generators are independently written,
using existing Hound/Rubato/SHA-256 APIs; no new production dependency is added.
No human ratings have been supplied. Automated preparation does not certify
perceived quality, stable acceptance, or reference-engine parity.

### Harmonic-rise rejection — revision 6, 2026-11-08

A user-reported clean guitar regression exposed four supplemental spectral
candidates in a ten-second excerpt. Growth was concentrated in a few harmonic
bins. Each extra candidate causes a whole-spectrum phase reset and can disturb
sustained notes. Add an independently chosen novelty participation test:
`(sum positive novelty)^2 >= 16 * sum(positive novelty squared)`. This is the
effective count of equally powered bins, with no absolute gain threshold. Keep
the original energy scan and broadband masked-attack tests. This conservative
supplement can miss narrowband masked attacks; it does not introduce signal
separation or guarantee artifact-free polyphonic stretching. No reference-engine
implementation material was inspected or reused. Stretch revision 6 and detector
identity energy-spectral-v5 invalidate prior prepared audio.

## Reusable analysis extraction — 2026-11-08

The revision-6 energy/spectral detector and its original unit tests moved to
crates/audio-analysis without changing thresholds, merge order or revision
identities. This crate now owns only the detection adapter, attack regions and
stretch synthesis. See audio-analysis/PROVENANCE.md for the extraction boundary.
