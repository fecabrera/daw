//! Transient EQ plans use the ordinary coalesced preparation worker, never project/history state.
use daw_core::{EffectChainTarget, EffectSettings, Id, MultibandEqBand, Project};
#[derive(Clone, PartialEq)]
pub(super) struct Preview {
    project: Id,
    target: EffectChainTarget,
    effect: Id,
    before: MultibandEqBand,
    band: MultibandEqBand,
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
            Preview {
                project,
                target,
                effect,
                before,
                band,
            },
        ));
    }
    pub fn desired(&self, project: &Project, frame: u64) -> Option<Preview> {
        let (seen, p) = self.offered.as_ref()?;
        if *seen != frame || p.project != project.project_id {
            return None;
        }
        let (chain, _) = project.effect_chain(p.target)?;
        let EffectSettings::MultibandEq(s) = &chain.iter().find(|e| e.id == p.effect)?.unit else {
            return None;
        };
        (s.bands.iter().find(|b| b.id == p.band.id) == Some(&p.before)).then(|| p.clone())
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
        if let Some(p) = preview {
            let (chain, compact) = plan
                .project
                .effect_chain(p.target)
                .ok_or("EQ preview owner is unavailable")?;
            let mut chain = chain.to_vec();
            let EffectSettings::MultibandEq(s) = &mut chain
                .iter_mut()
                .find(|e| e.id == p.effect)
                .ok_or("EQ preview effect is unavailable")?
                .unit
            else {
                return Err("EQ preview effect changed".into());
            };
            let band = s
                .bands
                .iter_mut()
                .find(|b| b.id == p.band.id)
                .ok_or("EQ preview band is unavailable")?;
            *band = p.band.clone();
            plan.project
                .edit(p.target.edit(chain, compact))
                .map_err(|e| e.to_string())?;
        }
        Ok(plan)
    }
}
