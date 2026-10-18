# Stereo Spread

Implementation requirements and acceptance criteria. For operation, see the [user guide](../../docs/effects/stereo-spread.md).

## Ownership and integration

`crates/unit-core` owns `StereoSpreadSettings`, the version-1 `daw.stereo-spread` identity, and four typed modulation targets. `crates/stereo-spread` owns portable filter banks, band distribution, smoothing, and final bounds. FunDSP types stay private. The crate has no DAW, UI, media, or device dependency. The engine constructs it for serial track and Master chains; live playback, offline export, random-access helpers, and CLI rendering share that path.

Register Stereo Spread once in `effects_catalog.rs::ENTRIES` under Imaging after Direction Mixer. Append the entry without changing existing indices/type IDs. Catalog folders start collapsed through the shared default. Reuse selection, keyboard navigation, group addition/drop, reorder/copy/delete, stable instance IDs, and common edit transactions. No constructor runs during a preview. Main provides modulation sources rather than another effect installation target.

## Controls and saved state

| Field | Bounds | Default |
| --- | --- | --- |
| `lower_intensity`, `upper_intensity` | −1–1 finite, displayed as signed percentages | 0 |
| `lower_frequency_hz` | 20–20,000 Hz finite | 20 |
| `upper_frequency_hz` | 20–20,000 Hz finite, at least 1 Hz above Lower | 20,000 |
| `order` | Integer 2–12 | 8 |
| `enabled`, `collapsed` | Independent power/presentation | true, false |
| `modulation_open`, `modulations` | Optional serde defaults; explicit null is invalid | false, empty |

Use the shared `EffectSettings` envelope and fresh `EffectInstance` UUID. Validate fields and routes atomically through common model transactions. Unknown future versions retain their opaque payload as missing bypassed effects; malformed supported data fails. Existing projects without this unit remain unchanged. Save/open, Save As, track duplication, processing-rate changes, copies, and reorder retain settings. Shared copy/transfer code remaps provider and route identities as applicable.

Use three standard 90-point strips with centered Lower, Upper, and Spread footers. Lower/Upper have Intensity above Frequency; Spread has Order at the top. Reserve the remaining shared knob rows so footers align. Reuse `PanelStyle::unit_in_lane`, shared margins/full-height dividers, power/collapse, modulation dot/connections, and normal compact/expanded lane heights. No audio meter is added. Compact/collapsed units hide normal controls.

All knobs use `Knob`, `KnobSpec`, and `knobs::row`. Signed Intensity is centered at zero with ±100 endpoints, 1-percentage-point keyboard steps, and zero reset. Frequency reuses the common logarithmic 20 Hz–20 kHz definition; defaults are 20 Hz and 20 kHz. Limit only the edited base frequency at its neighbor's 1 Hz boundary. Order uses a bounded whole-step knob, ticks, 2/12 endpoints, and reset 8. Parameter definitions and modulation amounts live in `parameter_knobs.rs`; retain Shift sensitivity, keyboard/focus guards, tooltips, reset, and accessibility in shared widgets.

## DSP

Derive Mid `M = (L + R) / 2` and retain the original input channels. Add a newly generated Side contribution `W`:

```text
left  = L + wet * W
right = R - wet * W
```

This preserves the original mono sum to floating-point precision at ordinary levels and retains the input's existing stereo difference. Signed Intensity reverses only the generated contribution. Both-zero intensity and settled bypass return finite dry samples exactly, including subnormals and headroom values. Sanitize nonfinite inputs to zero. Limit the Mid sent into private filters to ±1e12 to protect extreme finite intermediates; retain dry headroom and clamp final f64 sums to the f32 range. Silence after reset is exact. There is no alignment latency or automatic export extension for filter ringing.

Each bank applies two Butterworth SVF high-pass sections at Lower and two low-pass sections at Upper to Mid, giving fourth-order range rolloffs. Feed that result into `N = Order` parallel normalized band-pass filters. Space edges logarithmically between the constrained limits: `r = (Upper / Lower)^(1/N)`, center `i = Lower * r^(i + 0.5)`, and `Q = clamp(sqrt(r)/(r-1), 0.5, 32)`. Divide FunDSP's band-pass result by Q for unity peak gain. The Q limit bounds ringing for very narrow ranges; the range is a gradual filter region, not an exact spectral brick wall.

For each band, linearly interpolate Intensity by logarithmic band position: `amount_i = LowerIntensity + (UpperIntensity-LowerIntensity) * i/(N-1)`. Sum `W = Σ (-1)^i * amount_i * band_i`. The lowest band uses the positive coefficient. Filter phase contributes to the resulting frequency response. Upper/lower signs can differ, crossing neutral within the band set.

Reuse FunDSP from the existing workspace dependency. Existing bundled FunDSP notices in the oscillator/equalizer license set cover that dependency in packaged applications; no dependency version upgrade or separate backend exposure is needed.

## Smoothing and runtime

Reuse `ModulationRamp` for independent 5 ms base intensities, log2 frequencies, and whole-unit wet bypass. Presentation-only edits do not restart ramps. Reuse `ModulationSmoother<4>` for the shared 1 ms additive offset smoother. Resolve effective Intensities in −1–1 and Frequencies in 20–20,000 Hz. `StereoSpreadSettings::constrained_frequencies` raises Upper to at least 21 Hz and limits Lower to Upper−1; Upper takes precedence when modulated boundaries meet. Publish these actual constrained values.

Prepare two fixed banks, each with twelve band-pass and four range-filter instances, before callback publication. Cache frequency configurations. Compatible retargeting keeps the active bank's history. Order changes reset/reconfigure the spare bank and crossfade its generated Side over 5 ms. If another Order arrives during that transition, retain only its latest target and begin the next transition after the current one finishes. Do not restart or interrupt an in-flight fade. Stop/seek reset both banks and smoothers to current targets; Pause retains state. Muted tracks, loop boundaries, and gaps follow existing effect-chain behavior. Filters and offsets continue advancing through bypass.

No processing, reset, compatible inheritance, or Order change allocates, locks, logs, performs file I/O, or retires buffer owners on the callback. Construction and route vectors stay in the host's existing preparation path.

## Shared modulation

`ModulationParameter::STEREO_SPREAD` contains Lower Intensity, Upper Intensity, Lower Frequency, and Upper Frequency in that order. Settings and DSP implement the common destination traits in Shared scope. Append typed offset slots 49–52 after the existing 49 slots, for a total of 53; preserve all prior identities and indices.

| Destination | Signed amount | Final conversion |
| --- | --- | --- |
| Lower/Upper Intensity | −1–1, displayed ±100 percentage points | Add to base, clamp to −1–1. |
| Lower/Upper Frequency | ±6 octaves | Add to log2 base, exponentiate, constrain range and ordering. |

Reuse source discovery/publication, same-instrument scope validation, cached provider fan-out, route resolution/power/amount ramps, signed cancellation, `process_modulation`, and identity-based route retargeting. Do not add unit-specific source lookup or routing. Shared Retrigger/Legato Envelope, LFO, and Macro sources include Main. Reject foreign and Per note references. Missing, unavailable, disabled, and unassigned sources contribute zero. Order/power remain discrete.

Reuse Source/Destination/Amount strips, connection controls, destination-derived signed amount knobs, correlated range arcs, and processed dots. Audio snapshots report the four effective values in `STEREO_SPREAD` order; unused slots stay unavailable. The fixed snapshot capacity remains 19. Joint frequency limits can constrain a dot more than the independent possible-range arc; explain the 1 Hz rule in the common tooltip. Binding, discovery, stable effect identity, and telemetry publication stay shared.

## Provenance and acceptance

The band-distribution concept and Order meaning follow Apple's public [Stereo Spread guide](https://support.apple.com/guide/logicpro/lgcef240df26/mac). Signed intensity range and final defaults follow the user's current requirements. The filter-bank construction, interpolation, mono-preserving matrix, and transitions above are independently defined. No exact match to Logic's unpublished filters, signed intensity law, or response is claimed without reference measurements. No private recording is a repository fixture.

Check all six processing rates: neutral/bypass exactness; mono widening and mono fold-down; alternating midrange channel emphasis; separate endpoint intensities; positive/negative and mixed signs; frequency selectivity outside both limits; 2/8/12 bands; finite extreme inputs, crossed modulation limits, rapid Order changes, reset, and base/modulated parity. Check shared/local source scope and all four typed destinations.

Verify track and Master installation, live/offline/block/random-access/loop parity, all three provider kinds, route cancellation/reorder, compatible history, and allocation-free sampling/replacement/transport. Check snapshots, atomic invalid-edit rollback, source moves/remapping, legacy optional modulation fields, future payloads, save/reopen/Save As/duplication/rate persistence. UI checks cover both themes, all view targets, Imaging catalog order and default collapse, five knobs/reset/defaults, fixed equal strips, power/compact/collapse, removal, shared amount scales, markers, and connection opening. Native listening/appearance/device timing and Windows/Linux require separate verification; record executed checks in [validation history](../validation/README.md).
