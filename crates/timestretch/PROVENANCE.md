# Independent implementation record

Date: 2026-10-14. Processing revision: 1. Status: experimental.

The mandatory rule in `docs/timestretch.md` applies: under no circumstances take code directly from Rubber Band. Copies, ports, translations, adaptations, mechanical rewrites, and indirect reuse are prohibited. No Rubber Band DSP implementation source was consulted for this implementation. The new engine and tests were written in this task from the mathematical design below. This is a provenance record, not a certification of formal clean-room development or legal clearance.

## Research inputs

- Julius O. Smith, [Spectral Audio Signal Processing: Phase Vocoder](https://www.dsprelated.com/freebooks/sasp/Phase_Vocoder.html): STFT analysis, instantaneous frequency from phase derivatives, FFT resynthesis.
- Bernardini, De Götzen, and Arfib, [Traditional (?) implementations of a phase-vocoder: the tricks of the trade](https://www.dafx.de/paper-archive/2000/pdf/Bernardini.pdf), DAFx 2000: windowing, phase, synthesis, and phase locking. Used as research context; no accompanying code was imported.
- Rubber Band [public API documentation](https://breakfastquay.com/rubberband/code-doc/classRubberBand_1_1RubberBandStretcher.html): reference adapter only, including offline study/process/retrieve and reported engine version. No implementation source links were followed.

Prior exposure: the existing DAW Rust adapter was read. Earlier specification work inspected the Rust binding's public option mappings/API declarations. The conversation contains public Rubber Band behavior descriptions. These facts rule out claiming that all participants had no prior exposure. No Rubber Band implementation material was used to derive the new DSP.

## Independent design

`src/spectral.rs` implements centered Hann-window STFT and weighted overlap-add in f64. For each bin k, expected angular frequency is 2πk/W. Measured frequency is expected frequency plus the principal phase residual divided by the actual input-center advance. Accumulate the difference between output and input phase advances. Apply a unit complex rotation to the original channel spectra; reconstruct negative frequencies by conjugation.

Stereo analysis sums per-channel complex cross-products between consecutive spectra. It never sums L/R samples, so an anti-phase signal does not cancel. Both channels receive the same rotation. Local spectral maxima define nearest-peak regions for identity phase locking. Both analysis and synthesis hops are bounded by W/8. Window length scales with sample rate. Windows extend beyond output bounds and normalize by their squared overlap weights, stored in one periodic hop-sized table; source endpoints use zero extension. Exact constant signals use a direct DC path. There is no resampling fallback. Identity requests bypass DSP after validation and preserve bits. Tests independently generate sines and use interpolated crossings to estimate pitch.

The analysis pass selects a short (about 2–3 ms) window for broadband material, using energy-weighted geometric/arithmetic spectral flatness above 0.15. Tonal material uses a longer (about 40–46 ms) window. This independent design choice improves attack localization in noise-like material. Selection applies to the complete source: mixed tonal/percussive material still requires listening evaluation. Neither audio similarity nor quality parity with Rubber Band is claimed.

## Dependencies and reuse

Only RustFFT 6.4.1 is a direct DSP dependency (MIT OR Apache-2.0). Its resolved dependencies are num-complex, num-integer, num-traits, primal-check, strength_reduce, transpose, and build dependency autocfg. These are permissively licensed. Default RustFFT CPU acceleration features are enabled. Dependency license texts are in `licenses/`; the lockfile records resolved versions. No third-party time-stretch implementation or test fixture was copied. Original engine/tests are BSD-2-Clause, with the local LICENSE.

Rubber Band is not a core dependency. The independently written CLI adapter calls separately licensed Rubber Band R2/R3 through rubberband 0.1.5 / rubberband-sys 0.1.4 / native 4.0.0. Those reference dependencies remain GPL-2.0-or-later and are optional tool dependencies; enabling them does not make that combined tool permissively licensed. Their notices ship in the tool's `licenses/` directory.
