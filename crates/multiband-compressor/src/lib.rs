//! Adjacent, phase-matched LR4 bands with independent linked-stereo compression.
use daw_filter::crossover::LinkwitzRileySplit as Split;
use daw_unit_core::{
    EffectUnit, LevelMonitor, ModulationContext, ModulationDestination, ModulationInputs,
    ModulationParameter as P, ModulationRamp as Ramp, ModulationScope, ModulationSmoother,
    ModulationTargetId, ModulationValue, MultibandCompressorBand, MultibandCompressorSettings,
    PortKind, ProcessingContext, RoutedDestination, UnitDescriptor,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn gain(db: f32) -> f32 {
    if db <= daw_unit_core::SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(db / 20.0)
    }
}
fn db(v: f32) -> f32 {
    if v <= 0.0 {
        daw_unit_core::SILENT_GAIN_DB
    } else {
        20.0 * v.log10()
    }
}
fn clean(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}
fn finite(v: f64) -> f32 {
    if v.is_finite() {
        v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    } else {
        0.0
    }
}
struct Band {
    id: ModulationTargetId,
    settings: MultibandCompressorBand,
    compressor: daw_compressor::Compressor,
    monitor: daw_unit_core::UnitMeter<daw_unit_core::BandMeter<3>>,
    solo: Option<Arc<AtomicBool>>,
    audible: Ramp,
}
impl Clone for Band {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            settings: self.settings.clone(),
            compressor: self.compressor.clone(),
            monitor: self.monitor.clone(),
            solo: None,
            audible: Ramp::new(1.0),
        }
    }
}
impl Band {
    fn new(c: ProcessingContext, s: &MultibandCompressorBand, enabled: bool) -> Self {
        let mut compressor = daw_compressor::Compressor::new(c, &s.compressor_settings());
        compressor.set_monitoring_enabled(enabled && s.enabled);
        Self {
            id: s.id,
            settings: s.clone(),
            compressor,
            monitor: daw_unit_core::UnitMeter::new(enabled),
            solo: None,
            audible: Ramp::new(1.0),
        }
    }
}
impl ModulationDestination for Band {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[
        P::CompressorThreshold,
        P::CompressorRatio,
        P::CompressorAttack,
        P::CompressorRelease,
        P::CompressorKnee,
        P::Level,
    ];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        Some(self.id)
    }
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        c: ModulationContext,
    ) -> [f32; 2] {
        let out = self.compressor.process_with_modulation(input, m, c);
        self.monitor.publish([
            out[0].abs().max(out[1].abs()),
            self.compressor.gain_reduction_db(),
            0.0,
        ]);
        out
    }
}
#[derive(Clone)]
struct Output {
    base: [Ramp; 2],
    external: ModulationSmoother<2>,
    effective: [f32; 2],
}
impl ModulationDestination for Output {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[P::Level, P::DryWet];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets([P::Level, P::DryWet]))
            .map(|v| v as f32);
        self.effective = [
            if b[0] == 0.0 {
                daw_unit_core::SILENT_GAIN_DB
            } else {
                (db(b[0]) + o[0]).clamp(daw_unit_core::SILENT_GAIN_DB, 12.0)
            },
            (b[1] + o[1]).clamp(0.0, 1.0),
        ];
        input
    }
}
pub struct MultibandCompressor {
    audition_allowed: Option<Arc<AtomicBool>>,
    bands: Vec<RoutedDestination<Band>>,
    order: Vec<usize>,
    present: Vec<bool>,
    output: RoutedDestination<Output>,
    // [current log Hz, target log Hz] share one clock to preserve boundary order.
    frequencies: Vec<[f32; 2]>,
    crossover_left: u32,
    splits: Vec<Split>,
    compensation: Vec<Vec<Split>>,
    solo_mix: Ramp,
    bypass: Ramp,
    fade_frames: u32,
    monitor: LevelMonitor<3>,
}
impl Clone for MultibandCompressor {
    fn clone(&self) -> Self {
        Self {
            audition_allowed: None,
            bands: self.bands.clone(),
            order: self.order.clone(),
            present: self.present.clone(),
            output: self.output.clone(),
            frequencies: self.frequencies.clone(),
            crossover_left: self.crossover_left,
            splits: self.splits.clone(),
            compensation: self.compensation.clone(),
            solo_mix: Ramp::new(0.0),
            bypass: self.bypass,
            fade_frames: self.fade_frames,
            monitor: self.monitor.clone(),
        }
    }
}
impl MultibandCompressor {
    pub const LATENCY_FRAMES: u32 = 0;
    pub fn new(c: ProcessingContext, s: &MultibandCompressorSettings) -> Self {
        let frequencies: Vec<_> = s
            .bands
            .iter()
            .take(s.bands.len().saturating_sub(1))
            .map(|b| b.upper_hz)
            .collect();
        Self {
            order: (0..s.bands.len()).collect(),
            present: vec![true; s.bands.len()],
            audition_allowed: None,
            bands: s
                .bands
                .iter()
                .map(|b| {
                    RoutedDestination::new(
                        Band::new(c, b, s.enabled),
                        &s.modulations,
                        c.sample_rate_hz,
                    )
                })
                .collect(),
            output: RoutedDestination::new(
                Output {
                    base: [gain(s.level_db), s.dry_wet].map(Ramp::new),
                    external: ModulationSmoother::new(c),
                    effective: [s.level_db, s.dry_wet],
                },
                &s.modulations,
                c.sample_rate_hz,
            ),
            splits: frequencies
                .iter()
                .map(|f| Split::new(c.sample_rate_hz, *f))
                .collect(),
            compensation: (0..s.bands.len())
                .map(|i| {
                    frequencies
                        .iter()
                        .skip(i + 1)
                        .map(|f| Split::new(c.sample_rate_hz, *f))
                        .collect()
                })
                .collect(),
            frequencies: frequencies.iter().map(|f| [f.ln(); 2]).collect(),
            crossover_left: 0,
            solo_mix: Ramp::new(0.0),
            bypass: Ramp::new(f32::from(s.enabled)),
            fade_frames: (c.sample_rate_hz / 200).max(1),
            monitor: LevelMonitor::new(c.sample_rate_hz, s.enabled),
        }
    }
    /// Prepare every plugin slot before activation. Presence is separate from band power.
    pub fn prepare_slots(
        c: ProcessingContext,
        bands: &[MultibandCompressorBand],
        present: &[bool],
        enabled: bool,
        level: f32,
        mix: f32,
    ) -> Self {
        let settings = MultibandCompressorSettings {
            bands: bands.to_vec(),
            enabled,
            level_db: level,
            dry_wet: mix,
            ..Default::default()
        };
        let mut unit = Self::new(c, &settings);
        unit.present.copy_from_slice(present);
        unit.rebuild_order();
        unit.reset();
        unit
    }
    fn rebuild_order(&mut self) {
        self.order.clear();
        for (slot, present) in self.present.iter().copied().enumerate() {
            if present {
                self.order.push(slot);
            }
        }
        self.order.sort_unstable_by(|a, b| {
            self.bands[*a]
                .destination
                .settings
                .upper_hz
                .total_cmp(&self.bands[*b].destination.settings.upper_hz)
                .then(a.cmp(b))
        });
        let count = self.order.len();
        let mut lower = 19.0;
        for (i, slot) in self
            .order
            .iter()
            .copied()
            .take(count.saturating_sub(1))
            .enumerate()
        {
            let upper = 20_000.0 - (count - i - 1) as f32;
            let hz = self.bands[slot]
                .destination
                .settings
                .upper_hz
                .clamp(lower + 1.0, upper);
            self.frequencies[i][1] = hz.ln();
            lower = hz;
        }
        self.crossover_left = self.fade_frames;
    }
    /// Allocation-free fixed-slot update. The host supplies validated scalar fields.
    pub fn retarget_slot(
        &mut self,
        slot: usize,
        settings: &MultibandCompressorBand,
        present: bool,
    ) {
        let Some(b) = self.bands.get_mut(slot) else {
            return;
        };
        if b.destination.id != settings.id {
            return;
        }
        let changed =
            b.destination.settings.upper_hz != settings.upper_hz || self.present[slot] != present;
        b.destination.settings = settings.clone();
        b.destination
            .compressor
            .retarget(&settings.compressor_settings());
        b.destination
            .compressor
            .set_monitoring_enabled(self.monitor.is_enabled() && settings.enabled && present);
        b.destination
            .monitor
            .set_enabled(self.monitor.is_enabled() && present);
        self.present[slot] = present;
        if !present && let Some(flag) = &b.destination.solo {
            flag.store(false, Ordering::Relaxed);
        }
        if changed {
            self.rebuild_order();
        }
    }
    pub fn retarget_output(&mut self, enabled: bool, level: f32, mix: f32) {
        for (r, v) in self
            .output
            .destination
            .base
            .iter_mut()
            .zip([gain(level), mix])
        {
            r.set(v, self.fade_frames);
        }
        self.bypass.set(f32::from(enabled), self.fade_frames);
        self.monitor.set_enabled(enabled);
        for (b, present) in self.bands.iter_mut().zip(&self.present) {
            b.destination
                .compressor
                .set_monitoring_enabled(enabled && b.destination.settings.enabled && *present);
            b.destination.monitor.set_enabled(enabled && *present);
            if !enabled && let Some(flag) = &b.destination.solo {
                flag.store(false, Ordering::Relaxed);
            }
        }
    }
    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }
    pub fn set_audition_gate(&mut self, allowed: Arc<AtomicBool>) {
        self.audition_allowed = Some(allowed);
    }
    /// Live monitoring only. Offline processors never receive these handles.
    pub fn set_band_level_meter(
        &mut self,
        id: ModulationTargetId,
        meter: Arc<daw_unit_core::BandMeter<3>>,
    ) {
        if let Some(b) = self.bands.iter_mut().find(|b| b.destination.id == id) {
            b.destination.monitor.attach(meter, [0.0; 3]);
        }
    }
    pub fn set_band_reduction_meter(
        &mut self,
        id: ModulationTargetId,
        meter: Arc<daw_unit_core::PeakMeter>,
    ) {
        if let Some(b) = self.bands.iter_mut().find(|b| b.destination.id == id) {
            b.destination
                .compressor
                .set_gain_reduction_peak_meter(meter);
        }
    }
    pub fn set_band_solo(&mut self, id: ModulationTargetId, solo: Arc<AtomicBool>) {
        if let Some(b) = self.bands.iter_mut().find(|b| b.destination.id == id) {
            b.destination.solo = Some(solo);
        }
    }
    pub fn retarget(&mut self, s: &MultibandCompressorSettings) -> bool {
        if self.bands.len() != s.bands.len()
            || !self
                .bands
                .iter()
                .zip(&s.bands)
                .all(|(a, b)| a.destination.id == b.id)
        {
            return false;
        }
        self.monitor.set_enabled(s.enabled);
        for (b, s) in self.bands.iter_mut().zip(&s.bands) {
            b.destination.settings = s.clone();
            b.destination.compressor.retarget(&s.compressor_settings());
            b.destination
                .compressor
                .set_monitoring_enabled(self.monitor.is_enabled() && s.enabled);
        }
        if self
            .frequencies
            .iter()
            .zip(&s.bands)
            .any(|(f, b)| f[1] != b.upper_hz.ln())
        {
            for (f, b) in self.frequencies.iter_mut().zip(&s.bands) {
                f[1] = b.upper_hz.ln();
            }
            self.crossover_left = self.fade_frames;
        }
        for (r, v) in self
            .output
            .destination
            .base
            .iter_mut()
            .zip([gain(s.level_db), s.dry_wet])
        {
            r.set(v, self.fade_frames);
        }
        self.bypass.set(f32::from(s.enabled), self.fade_frames);
        self.monitor.set_enabled(s.enabled);
        for b in &mut self.bands {
            b.destination.monitor.set_enabled(s.enabled);
            if !s.enabled
                && let Some(flag) = &b.destination.solo
            {
                flag.store(false, Ordering::Relaxed);
            }
        }
        true
    }
    pub fn inherit(&mut self, previous: &mut Self) {
        for next in &mut self.bands {
            if let Some(old) = previous
                .bands
                .iter_mut()
                .find(|b| b.destination.id == next.destination.id)
            {
                next.inherit_routes(old);
                std::mem::swap(&mut next.destination.solo, &mut old.destination.solo);
                std::mem::swap(
                    &mut next.destination.compressor,
                    &mut old.destination.compressor,
                );
                next.destination
                    .compressor
                    .retarget(&next.destination.settings.compressor_settings());
                next.destination.compressor.set_monitoring_enabled(
                    self.monitor.is_enabled() && next.destination.settings.enabled,
                );
                next.destination.audible = old.destination.audible;
                std::mem::swap(&mut next.destination.monitor, &mut old.destination.monitor);
                next.destination
                    .monitor
                    .set_enabled(self.monitor.is_enabled());
            }
        }
        self.output.inherit_routes(&mut previous.output);
        let targets = self.output.destination.base.map(|r| r.target());
        std::mem::swap(
            &mut self.output.destination,
            &mut previous.output.destination,
        );
        for (r, v) in self.output.destination.base.iter_mut().zip(targets) {
            r.set(v, self.fade_frames);
        }
        self.solo_mix = previous.solo_mix;
        let target = self.bypass.target();
        self.bypass = previous.bypass;
        self.bypass.set(target, self.fade_frames);
        if self.splits.len() == previous.splits.len() {
            std::mem::swap(&mut self.splits, &mut previous.splits);
            std::mem::swap(&mut self.compensation, &mut previous.compensation);
            let unchanged = self
                .frequencies
                .iter()
                .zip(&previous.frequencies)
                .all(|(a, b)| a[1] == b[1]);
            for (next, old) in self.frequencies.iter_mut().zip(&previous.frequencies) {
                next[0] = old[0];
            }
            self.crossover_left = if unchanged {
                previous.crossover_left
            } else {
                self.fade_frames
            };
        }
        let enabled = self.monitor.is_enabled();
        std::mem::swap(&mut self.monitor, &mut previous.monitor);
        self.monitor.set_enabled(enabled);
        if !enabled {
            for b in &self.bands {
                if let Some(flag) = &b.destination.solo {
                    flag.store(false, Ordering::Relaxed);
                }
            }
        }
    }
    pub fn process_at(
        &mut self,
        input: [f32; 2],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> [f32; 2] {
        let input = input.map(clean);
        let dry = input;
        let allowed = self
            .audition_allowed
            .as_ref()
            .is_none_or(|g| g.load(Ordering::Relaxed));
        let any_solo = self.bypass.target() != 0.0
            && allowed
            && self.order.iter().any(|slot| {
                self.bands[*slot]
                    .destination
                    .solo
                    .as_ref()
                    .is_some_and(|s| s.load(Ordering::Relaxed))
            });
        for (i, f) in self
            .frequencies
            .iter_mut()
            .take(self.order.len().saturating_sub(1))
            .enumerate()
        {
            if self.crossover_left > 0 {
                f[0] += (f[1] - f[0]) / self.crossover_left as f32;
            }
            let frequency = f[0].exp();
            self.splits[i].configure(frequency);
            // A lower branch receives the same smoothed boundary as the later split.
            for (row_index, row) in self.compensation.iter_mut().enumerate().take(i) {
                row[i - row_index - 1].configure(frequency);
            }
        }
        self.crossover_left = self.crossover_left.saturating_sub(1);
        // Keep crossover state finite even for extreme finite host samples.
        let mut rest = input.map(|v| v.clamp(-1e12, 1e12));
        let mut wet_sum = [0.0_f64; 2];
        let mut solo_dry = [0.0_f64; 2];
        for (i, slot) in self.order.iter().copied().enumerate() {
            let band = &mut self.bands[slot];
            let mut sample = if i + 1 < self.order.len() {
                let split = &mut self.splits[i];
                let (low, high) = split.sample(rest);
                rest = high;
                low
            } else {
                rest
            };
            for split in self.compensation[i]
                .iter_mut()
                .take(self.order.len().saturating_sub(i + 2))
            {
                let (low, high) = split.sample(sample);
                sample = std::array::from_fn(|ch| low[ch] + high[ch]);
            }
            let original = sample.map(clean);
            let compressed = band.process(original, shared, context);
            let b = &mut band.destination;
            let solo = b.solo.as_ref().is_some_and(|s| s.load(Ordering::Relaxed));
            if !allowed {
                b.audible = Ramp::new(1.0);
            }
            b.audible
                .set(f32::from(!any_solo || solo), self.fade_frames);
            let mask = f64::from(b.audible.next_value());

            for ch in 0..2 {
                wet_sum[ch] += mask * f64::from(compressed[ch]);
                solo_dry[ch] += mask * f64::from(original[ch]);
            }
        }
        if self.order.is_empty() {
            solo_dry = input.map(f64::from);
            wet_sum = solo_dry;
        }
        self.output.process([0.0; 2], shared, context);
        let [level, mix] = self.output.destination.effective;
        if !allowed {
            self.solo_mix = Ramp::new(0.0);
        }
        self.solo_mix.set(f32::from(any_solo), self.fade_frames);
        let solo_mix = self.solo_mix.next_value();
        let mix = f64::from(mix + (1.0 - mix) * solo_mix);
        let enabled = f64::from(self.bypass.next_value());
        let out = std::array::from_fn(|ch| {
            let base = solo_dry[ch];
            let wet = (base * (1.0 - mix) + mix * wet_sum[ch]) * f64::from(gain(level));
            if enabled == 0.0 {
                dry[ch]
            } else if enabled == 1.0 {
                finite(wet)
            } else {
                finite(f64::from(dry[ch]) * (1.0 - enabled) + wet * enabled)
            }
        });
        self.monitor.sample_output(out);
        out
    }
    pub fn band_parameters(&self, id: ModulationTargetId) -> Option<[f32; 7]> {
        self.bands
            .iter()
            .find(|b| b.destination.id == id)
            .map(|b| b.destination.compressor.effective_parameters())
    }
    pub fn effective_parameters(&self) -> [f32; 2] {
        self.output.destination.effective
    }
}
impl EffectUnit for MultibandCompressor {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::MULTIBAND_COMPRESSOR_TYPE_ID,
            name: "Multiband Compressor",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_at(input, &[], Default::default())
    }
    fn latency_frames(&self) -> u32 {
        Self::LATENCY_FRAMES
    }
    fn reset(&mut self) {
        self.monitor.reset();
        self.solo_mix.reset();
        self.bypass.reset();
        for b in &mut self.bands {
            b.reset_routes();
            b.destination.compressor.reset();
            b.destination.monitor.publish([0.0; 3]);
            b.destination.audible.reset();
        }
        self.output.reset_routes();
        for r in &mut self.output.destination.base {
            r.reset();
        }
        self.output.destination.external.reset();
        for f in &mut self.frequencies {
            f[0] = f[1];
        }
        self.crossover_left = 0;
        for s in &mut self.splits {
            s.reset();
        }
        for s in self.compensation.iter_mut().flatten() {
            s.reset();
        }
    }
    fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.order)
            + daw_unit_core::memory::vec_bytes(&self.present)
            + daw_unit_core::memory::vec_bytes(&self.bands)
            + self
                .bands
                .iter()
                .map(RoutedDestination::heap_bytes)
                .sum::<u64>()
            + self.output.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.frequencies)
            + daw_unit_core::memory::vec_bytes(&self.splits)
            + daw_unit_core::memory::vec_bytes(&self.compensation)
            + self
                .compensation
                .iter()
                .map(daw_unit_core::memory::vec_bytes)
                .sum::<u64>()
    }
}
