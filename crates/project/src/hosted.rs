//! Revision-checked opaque state receipts do not create additional history steps.
use crate::{Result, Session, error};
use daw_core::{Edit, HostedSettings, HostedStateReference};
impl Session {
    pub fn apply_hosted_state(
        &mut self,
        before: &HostedSettings,
        reference: HostedStateReference,
        states: &daw_plugin_host::StateBank,
    ) -> Result<bool> {
        if self.project.hosted_unit(before.id) != Some(before) {
            return Ok(false);
        }
        if reference.revision != before.edit_revision {
            return Err(error("Plugin state receipt has a stale revision"));
        }
        states
            .restore(before.plugin.identity.format(), &reference)
            .map_err(error)?;
        let mut settings = before.clone();
        settings.state = Some(reference);
        self.project
            .edit(Edit::SetHosted {
                unit_id: before.id,
                settings: Box::new(settings),
            })
            .map_err(error)?;
        self.plugin_states.merge(states);
        // History snapshots own their own immutable banks. Keep only the owners
        // referenced by this snapshot; old states remain in their history entry.
        self.plugin_states.retain(
            self.project
                .hosted_units()
                .into_iter()
                .filter_map(|(_, s)| s.state.as_ref()),
        );
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionHistory;
    use daw_core::*;
    use daw_plugin_host::{NativeState, PluginFormat, StateBank};
    use daw_unit_core::HostedPluginDescriptor;
    #[test]
    fn state_receipts_preserve_exact_undo_redo_and_reject_stale_revisions() {
        let mut session = Session::default();
        let track = session.project.add_track().unwrap();
        let mut states = StateBank::default();
        let mut settings = HostedSettings::new(
            HostedPluginDescriptor {
                identity: PluginIdentity::Clap("org.daw.test".into()),
                name: "Owned".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: HostedRole::Effect,
            },
            vec![],
        );
        let old = states.capture(NativeState::Clap(vec![1, 2]), 0).unwrap();
        settings.state = Some(old.clone());
        session.plugin_states = states;
        session
            .project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![EffectInstance {
                    id: Id::new_v4(),
                    unit: EffectSettings::Hosted(settings.clone()),
                }],
                compact: false,
            })
            .unwrap();
        let mut history = SessionHistory::new(&session, true).unwrap();
        let mut changed = settings.clone();
        changed.edit_revision = 1;
        session
            .project
            .edit(Edit::SetHosted {
                unit_id: settings.id,
                settings: Box::new(changed.clone()),
            })
            .unwrap();
        history.record(&session).unwrap();
        let mut captured = StateBank::default();
        let new = captured
            .capture(NativeState::Clap(vec![3, 4, 5]), 1)
            .unwrap();
        assert!(
            !session
                .apply_hosted_state(&settings, old, &captured)
                .unwrap()
        );
        assert!(
            session
                .apply_hosted_state(&changed, new.clone(), &captured)
                .unwrap()
        );
        history
            .apply_hosted_state(&changed, new.clone(), &captured)
            .unwrap();
        let undone = history.undo(&session).unwrap();
        let reference = undone
            .project
            .hosted_unit(settings.id)
            .unwrap()
            .state
            .as_ref()
            .unwrap();
        assert_eq!(
            undone
                .plugin_states
                .restore(PluginFormat::Clap, reference)
                .unwrap(),
            NativeState::Clap(vec![1, 2])
        );
        let redone = history.redo(&undone).unwrap();
        assert_eq!(
            redone
                .project
                .hosted_unit(settings.id)
                .unwrap()
                .state
                .as_ref(),
            Some(&new)
        );
        assert_eq!(
            redone
                .plugin_states
                .restore(PluginFormat::Clap, &new)
                .unwrap(),
            NativeState::Clap(vec![3, 4, 5])
        );
    }
}
