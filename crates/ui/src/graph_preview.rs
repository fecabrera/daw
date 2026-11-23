//! Transient graph plans reuse the coalesced worker, never project/history state.
use daw_core::{
    Edit, EffectChainTarget, EffectSettings, EnvelopeSettings, Id, ModulationSourceId,
    MultibandCompressorBand, MultibandEqBand, Project,
};
#[derive(Clone, PartialEq)]
enum Band {
    FixedEq(Box<daw_core::EqSettings>),
    Eq(MultibandEqBand),
    Compressor(MultibandCompressorBand),
    Curve(Box<daw_core::CompressorSettings>),
}
#[derive(Clone, PartialEq)]
pub(super) struct EffectPreview {
    project: Id,
    target: EffectChainTarget,
    effect: Id,
    before: Band,
    band: Band,
}
#[derive(Clone, PartialEq)]
pub(super) struct EnvelopePreview {
    pub project: Id,
    pub track: Id,
    pub source: ModulationSourceId,
    pub before: EnvelopeSettings,
    pub settings: EnvelopeSettings,
}
#[derive(Clone, PartialEq)]
pub(super) enum Preview {
    Effect(EffectPreview),
    Envelope(EnvelopePreview),
}
#[derive(Default)]
pub(super) struct State {
    offered: Option<(u64, Preview)>,
    published: Option<Preview>,
    output_present: bool,
}
impl State {
    pub fn offer(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: MultibandEqBand,
        band: MultibandEqBand,
    ) {
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::Eq(before),
                band: Band::Eq(band),
            }),
        ));
    }
    pub fn offer_equalizer(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: daw_core::EqSettings,
        band: daw_unit_core::MultibandEqBand,
    ) {
        let mut settings = before.clone();
        let index = (band.id.as_u128() - 1) as usize;
        settings.bands[index].gain_db = band.gain_db;
        if index != 1 {
            settings.bands[index].frequency_hz = band.frequency_hz;
            settings.bands[index].q = band.q;
        }
        if settings == before {
            return;
        }
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::FixedEq(Box::new(before)),
                band: Band::FixedEq(Box::new(settings)),
            }),
        ));
    }
    pub fn offer_compressor(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: MultibandCompressorBand,
        hz: f32,
    ) {
        let mut band = before.clone();
        band.upper_hz = hz;
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::Compressor(before),
                band: Band::Compressor(band),
            }),
        ));
    }
    pub fn offer_compressor_threshold(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: MultibandCompressorBand,
        db: f32,
    ) {
        let mut band = before.clone();
        band.threshold_db = db;
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::Compressor(before),
                band: Band::Compressor(band),
            }),
        ));
    }
    pub fn offer_compressor_curve(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: MultibandCompressorBand,
        parameters: [f32; 2],
    ) {
        let mut band = before.clone();
        band.threshold_db = parameters[0];
        band.ratio = parameters[1];
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::Compressor(before),
                band: Band::Compressor(band),
            }),
        ));
    }
    pub fn offer_curve(
        &mut self,
        frame: u64,
        project: Id,
        target: EffectChainTarget,
        effect: Id,
        before: daw_core::CompressorSettings,
        parameters: [f32; 2],
    ) {
        let mut settings = before.clone();
        settings.threshold_db = parameters[0];
        settings.ratio = parameters[1];
        self.offered = Some((
            frame,
            Preview::Effect(EffectPreview {
                project,
                target,
                effect,
                before: Band::Curve(Box::new(before)),
                band: Band::Curve(Box::new(settings)),
            }),
        ));
    }
    pub fn offer_envelope(&mut self, frame: u64, preview: EnvelopePreview) {
        if preview.before != preview.settings {
            self.offered = Some((frame, Preview::Envelope(preview)));
        }
    }
    pub fn desired(&self, project: &Project, frame: u64) -> Option<Preview> {
        let (seen, preview) = self.offered.as_ref()?;
        if *seen != frame {
            return None;
        }
        let p = match preview {
            Preview::Effect(p) if p.project == project.project_id => p,
            Preview::Envelope(p) if p.project == project.project_id => {
                let (_, envelope) = project
                    .instrument_settings(p.track)?
                    .envelope_destinations()
                    .find(|(source, envelope)| *source == p.source && envelope.present)?;
                return (envelope.settings == p.before).then(|| preview.clone());
            }
            _ => return None,
        };
        let (chain, _) = project.effect_chain(p.target)?;
        let unit = &chain.iter().find(|e| e.id == p.effect)?.unit;
        let current = match (unit, &p.before) {
            (EffectSettings::Eq3(s), Band::FixedEq(_)) => Band::FixedEq(Box::new(s.clone())),
            (EffectSettings::MultibandEq(s), Band::Eq(before)) => {
                Band::Eq(s.bands.iter().find(|b| b.id == before.id)?.clone())
            }
            (EffectSettings::MultibandCompressor(s), Band::Compressor(before)) => {
                Band::Compressor(s.bands.iter().find(|b| b.id == before.id)?.clone())
            }
            (EffectSettings::Compressor(s) | EffectSettings::Expander(s), Band::Curve(_)) => {
                Band::Curve(Box::new(s.clone()))
            }
            _ => return None,
        };
        (current == p.before).then(|| preview.clone())
    }
    pub fn needs_sync(&self, wanted: &Option<Preview>, output: bool) -> bool {
        self.published != *wanted || (wanted.is_some() && self.output_present != output)
    }
    pub fn synced(&mut self, wanted: Option<Preview>, output: bool) {
        self.published = wanted;
        self.output_present = output;
    }
    #[cfg(test)]
    pub fn published_plan(&self, session: &daw_project::Session) -> daw_engine::RenderPlan {
        Self::plan(session, self.published.as_ref()).unwrap()
    }
    pub fn plan(
        session: &daw_project::Session,
        preview: Option<&Preview>,
    ) -> Result<daw_engine::RenderPlan, String> {
        let mut plan = session.plan();
        if let Some(Preview::Effect(p)) = preview {
            let (chain, compact) = plan
                .project
                .effect_chain(p.target)
                .ok_or("Effect preview owner is unavailable")?;
            let mut chain = chain.to_vec();
            let unit = &mut chain
                .iter_mut()
                .find(|e| e.id == p.effect)
                .ok_or("Effect preview effect is unavailable")?
                .unit;
            match (unit, &p.band) {
                (EffectSettings::Eq3(s), Band::FixedEq(settings)) => *s = settings.as_ref().clone(),
                (EffectSettings::MultibandEq(s), Band::Eq(band)) => {
                    *s.bands
                        .iter_mut()
                        .find(|b| b.id == band.id)
                        .ok_or("Effect preview band is unavailable")? = band.clone()
                }
                (EffectSettings::MultibandCompressor(s), Band::Compressor(band)) => {
                    *s.bands
                        .iter_mut()
                        .find(|b| b.id == band.id)
                        .ok_or("Effect preview band is unavailable")? = band.clone()
                }
                (
                    EffectSettings::Compressor(s) | EffectSettings::Expander(s),
                    Band::Curve(settings),
                ) => *s = settings.as_ref().clone(),
                _ => return Err("Effect preview effect changed".into()),
            }
            plan.project
                .edit(p.target.edit(chain, compact))
                .map_err(|e| e.to_string())?;
        }
        if let Some(Preview::Envelope(p)) = preview {
            let mut instrument = plan
                .project
                .instrument_settings(p.track)
                .ok_or("Envelope preview instrument is unavailable")?
                .clone();
            let envelopes = if let Some(bus) = instrument.bus_mut(p.source.layer) {
                &mut bus.envelopes
            } else {
                &mut instrument
                    .layers
                    .iter_mut()
                    .find(|layer| layer.id == p.source.layer)
                    .ok_or("Envelope preview layer is unavailable")?
                    .envelopes
            };
            envelopes
                .iter_mut()
                .find(|envelope| envelope.id == p.source.unit && envelope.present)
                .ok_or("Envelope preview unit is unavailable")?
                .settings = p.settings;
            plan.project
                .edit(Edit::SetInstrument {
                    track_id: p.track,
                    instrument: Some(instrument),
                })
                .map_err(|error| error.to_string())?;
        }
        Ok(plan)
    }
}
