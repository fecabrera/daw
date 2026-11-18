//! Native services are provided by the desktop. UI stores portable descriptors
//! and main-thread receipts, never SDK interfaces or native window handles.
use crate::*;
use daw_core::{EffectChainTarget, HostedRole, HostedSettings, PluginIdentity};
use daw_plugin_host::{
    HostClient, InstalledPlugin, NativeMetadata, NativeNotification, NativeState, StateBank,
};
use std::sync::{Arc, mpsc};

#[derive(Clone)]
pub struct PluginSnapshot {
    pub settings: HostedSettings,
    pub states: StateBank,
}
#[derive(Clone)]
pub enum PluginRequest {
    Editor(Id),
    Reopen(Id),
    Capture(Box<PluginSnapshot>),
    Scan(daw_plugin_host::ScanMode),
    CancelScan,
}
enum Target {
    Effects(EffectChainTarget, Option<EffectInsertion>),
    Source { track: Id, layer: Id },
}
struct EffectInsertion {
    index: usize,
    order: Vec<Id>,
    inline_before: Option<daw_core::InstrumentComponent>,
}
struct Installation {
    project: Id,
    target: Target,
    receipt: mpsc::Receiver<daw_plugin_host::Result<InstalledPlugin>>,
}
#[derive(Default)]
pub(super) struct State {
    pub scan: crate::plugin_settings::ScanStatus,
    pub client: Option<Arc<HostClient>>,
    catalog: Vec<daw_unit_core::HostedPluginDescriptor>,
    installation: Option<Installation>,
    pub(super) requests: Vec<PluginRequest>,
    pub gestures: std::collections::HashSet<(Id, u32)>,
    barrier: Option<Barrier>,
    resuming: bool,
}
pub(super) enum Deferred {
    Save(bool),
    SaveNamed(String, Option<PathBuf>),
    Export(PathBuf, bool, ExportSettings),
}
struct Barrier {
    project: Id,
    remaining: std::collections::HashSet<Id>,
    action: Deferred,
}
pub(super) fn parameter_text(ctx: &egui::Context, unit: Id, parameter: u32, value: f64) -> String {
    ctx.data(|d| {
        d.get_temp::<(u64, String)>(egui::Id::new(("external-value-text", unit, parameter)))
    })
    .filter(|(bits, _)| *bits == value.to_bits())
    .map_or_else(|| format!("{value:.4}"), |(_, text)| text)
}
impl DawUi {
    pub fn shutdown_plugin_audio(&mut self) {
        self.output = None;
        self.session.plugin_host = None;
    }
    pub fn enable_plugin_hosting(&mut self, client: Arc<HostClient>) {
        self.session.plugin_host = Some(client.clone());
        self.hosting.client = Some(client);
        self.hosting.scan.available = true;
        self.sync_needed = true;
    }
    pub fn set_plugin_scan_status(
        &mut self,
        roots: Vec<daw_plugin_host::SearchRoot>,
        scanning: bool,
        error: Option<String>,
    ) {
        self.hosting.scan.roots = roots;
        self.hosting.scan.scanning = scanning;
        self.hosting.scan.error = error;
    }
    pub fn set_plugin_catalog(&mut self, registry: &daw_plugin_host::PluginRegistry) {
        self.sync_needed = true;
        self.hosting.catalog = registry
            .entries
            .iter()
            .flat_map(|e| &e.plugins)
            .filter(|p| registry.resolve(&p.identity).is_some())
            .fold(Vec::new(), |mut all, p| {
                if !all
                    .iter()
                    .any(|v: &daw_unit_core::HostedPluginDescriptor| v.identity == p.identity)
                {
                    all.push(p.clone());
                }
                all
            });
        self.hosting.scan.inventory = registry
            .entries
            .iter()
            .filter(|e| e.present)
            .flat_map(|e| {
                let state = match &e.state {
                    daw_plugin_host::ScanState::Ready => "Available".to_owned(),
                    daw_plugin_host::ScanState::Quarantined(reason) => format!("Failed: {reason}"),
                    _ => "Pending".to_owned(),
                };
                if e.plugins.is_empty() {
                    vec![(
                        e.candidate
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        state,
                        e.candidate.path.display().to_string(),
                    )]
                } else {
                    e.plugins
                        .iter()
                        .map(|p| {
                            (
                                format!(
                                    "{} · {}",
                                    p.identity.format().extension().to_ascii_uppercase(),
                                    p.name
                                ),
                                state.clone(),
                                e.candidate.path.display().to_string(),
                            )
                        })
                        .collect()
                }
            })
            .collect();
        self.effects_catalog.set_external(
            self.hosting
                .catalog
                .iter()
                .filter(|p| p.role == HostedRole::Effect)
                .cloned()
                .collect(),
        );
        self.instrument_catalog.set_external(
            self.hosting
                .catalog
                .iter()
                .filter(|p| p.role == HostedRole::Instrument)
                .cloned()
                .collect(),
        );
    }
    pub fn plugin_snapshots(&self) -> Vec<PluginSnapshot> {
        self.session
            .project
            .hosted_units()
            .into_iter()
            .map(|(_, s)| PluginSnapshot {
                settings: s.clone(),
                states: self.session.plugin_states.clone(),
            })
            .collect()
    }
    pub fn plugin_diagnostic(&mut self, message: impl ToString) {
        self.fail(message);
    }
    pub fn set_plugin_parameter_text(
        &self,
        ctx: &egui::Context,
        unit: Id,
        parameter: u32,
        value: f64,
        text: String,
    ) {
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new(("external-value-text", unit, parameter)),
                (value.to_bits(), text),
            )
        });
    }
    pub fn set_plugin_metadata(&self, ctx: &egui::Context, unit: Id, metadata: NativeMetadata) {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("external-ports", unit)), metadata));
    }
    pub fn take_plugin_requests(&mut self, ctx: &egui::Context) -> Vec<PluginRequest> {
        let editors = ctx
            .data_mut(|d| d.remove_temp::<Vec<Id>>(egui::Id::new("external-editor-requests")))
            .unwrap_or_default();
        self.hosting
            .requests
            .extend(editors.into_iter().map(PluginRequest::Editor));
        if let Some(units) =
            ctx.data_mut(|d| d.remove_temp::<Vec<Id>>(egui::Id::new("external-reopen-requests")))
        {
            self.hosting
                .requests
                .extend(units.into_iter().map(PluginRequest::Reopen));
        }
        std::mem::take(&mut self.hosting.requests)
    }
    pub fn plugin_notification(
        &mut self,
        unit: Id,
        event: NativeNotification,
        ctx: &egui::Context,
    ) {
        match event {
            NativeNotification::PresetBegin => {
                self.hosting.gestures.insert((unit, u32::MAX));
                self.ensure_history();
                if let Some(h) = &mut self.history {
                    h.begin_group();
                }
            }
            NativeNotification::PresetEnd => {
                self.hosting.gestures.remove(&(unit, u32::MAX));
                if self.hosting.gestures.is_empty()
                    && let Some(h) = &mut self.history
                {
                    h.finish_group();
                }
            }
            NativeNotification::Telemetry(parameter, value) => {
                if value.is_finite() {
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new(("external-telemetry", unit, parameter)),
                            value,
                        )
                    });
                }
            }
            NativeNotification::Begin(parameter) => {
                self.hosting.gestures.insert((unit, parameter));
                self.ensure_history();
                if let Some(h) = &mut self.history {
                    h.begin_group();
                }
            }
            NativeNotification::End(parameter) => {
                self.hosting.gestures.remove(&(unit, parameter));
                if self.hosting.gestures.is_empty()
                    && let Some(h) = &mut self.history
                {
                    h.finish_group();
                }
            }
            NativeNotification::Value(parameter, value) => {
                let Some(mut settings) = self.session.project.hosted_unit(unit).cloned() else {
                    return;
                };
                let Some(binding) = settings
                    .parameters
                    .iter_mut()
                    .find(|p| p.definition.id == parameter && !p.definition.read_only)
                else {
                    return;
                };
                let Some(value) = binding.definition.domain.constrain(value) else {
                    return;
                };
                if binding.base_value == value {
                    return;
                }
                settings.edit_revision = settings.edit_revision.saturating_add(1);
                binding.base_value = value;
                binding.edit_revision = settings.edit_revision;
                self.edit(Edit::SetHosted {
                    unit_id: unit,
                    settings: Box::new(settings),
                });
            }
            NativeNotification::StateDirty => {
                let Some(mut settings) = self.session.project.hosted_unit(unit).cloned() else {
                    return;
                };
                settings.edit_revision = settings.edit_revision.saturating_add(1);
                self.edit(Edit::SetHosted {
                    unit_id: unit,
                    settings: Box::new(settings),
                });
                self.request_plugin_capture(unit);
            }
            NativeNotification::Restart(_) => {
                if self
                    .session
                    .project
                    .hosted_unit(unit)
                    .is_some_and(|s| s.state.is_none())
                {
                    self.plugin_reopened(unit);
                    return;
                }
                // Capture the current opaque state at the suspended boundary
                // before preparing an independent processor with fresh metadata.
                self.request_plugin_capture(unit);
            }
            NativeNotification::Fault => {
                self.fail(format!("Plugin processing failed in unit {unit}"))
            }
            _ => {}
        }
    }
    pub fn plugin_reopened(&mut self, unit: Id) {
        if let Some(mut settings) = self.session.project.hosted_unit(unit).cloned() {
            settings.edit_revision = settings.edit_revision.saturating_add(1);
            if let Some(state) = &mut settings.state {
                state.revision = settings.edit_revision;
            }
            self.edit(Edit::SetHosted {
                unit_id: unit,
                settings: Box::new(settings),
            });
        }
    }
    fn request_plugin_capture(&mut self, unit: Id) {
        if let Some(settings) = self
            .session
            .project
            .hosted_unit(unit)
            .filter(|s| s.state.is_some())
            .cloned()
        {
            self.hosting
                .requests
                .push(PluginRequest::Capture(Box::new(PluginSnapshot {
                    settings,
                    states: self.session.plugin_states.clone(),
                })));
        }
    }
    pub(super) fn defer_hosted(&mut self, action: Deferred) -> bool {
        if self.hosting.resuming || self.hosting.client.is_none() {
            return false;
        }
        let units: Vec<_> = self
            .session
            .project
            .hosted_units()
            .into_iter()
            .filter_map(|(_, s)| s.state.is_some().then_some(s.id))
            .collect();
        if units.is_empty() {
            return false;
        }
        self.hosting.barrier = Some(Barrier {
            project: self.session.project.project_id,
            remaining: units.iter().copied().collect(),
            action,
        });
        for unit in units {
            self.request_plugin_capture(unit);
        }
        true
    }
    pub fn plugin_capture_unavailable(&mut self, unit: Id) {
        if let Some(barrier) = &mut self.hosting.barrier {
            barrier.remaining.remove(&unit);
        }
    }
    pub fn plugin_capture_failed(&mut self, unit: Id, message: impl ToString) {
        if self
            .hosting
            .barrier
            .as_ref()
            .is_some_and(|b| b.remaining.contains(&unit))
        {
            self.hosting.barrier = None;
        }
        self.fail(message);
    }
    pub fn plugin_state_receipt(
        &mut self,
        before: &HostedSettings,
        state: NativeState,
    ) -> daw_project::Result<bool> {
        let mut states = StateBank::default();
        let reference = states
            .capture(state, before.edit_revision)
            .map_err(|e| daw_project::Error(e.to_string()))?;
        if !self
            .session
            .apply_hosted_state(before, reference.clone(), &states)?
        {
            if self
                .hosting
                .barrier
                .as_ref()
                .is_some_and(|b| b.remaining.contains(&before.id))
            {
                self.request_plugin_capture(before.id);
            }
            return Ok(false);
        }
        self.sync_needed = true;
        if let Some(history) = &mut self.history {
            history.apply_hosted_state(before, reference, &states)?;
        }
        if let Some(barrier) = &mut self.hosting.barrier {
            barrier.remaining.remove(&before.id);
        }
        Ok(true)
    }
    pub(super) fn poll_hosting(&mut self, ctx: &egui::Context) {
        self.session.plugin_host = self.hosting.client.clone();
        self.session.plugin_states.retain(
            self.session
                .project
                .hosted_units()
                .into_iter()
                .filter_map(|(_, s)| s.state.as_ref()),
        );
        if self
            .hosting
            .barrier
            .as_ref()
            .is_some_and(|b| b.project != self.session.project.project_id)
        {
            self.hosting.barrier = None;
        }
        if self
            .hosting
            .barrier
            .as_ref()
            .is_some_and(|b| b.remaining.is_empty())
        {
            let action = self.hosting.barrier.take().unwrap().action;
            self.hosting.resuming = true;
            match action {
                Deferred::Save(save_as) => self.save(save_as),
                Deferred::SaveNamed(name, parent) => self.save_named(name, parent),
                Deferred::Export(path, overwrite, settings) => {
                    self.export(path, overwrite, settings)
                }
            }
            self.hosting.resuming = false;
        }
        let ready = self
            .hosting
            .installation
            .as_ref()
            .and_then(|i| match i.receipt.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    daw_plugin_host::Error::Invalid("Plugin installation stopped".into()),
                )),
                _ => None,
            });
        if let Some(result) = ready {
            let installation = self.hosting.installation.take().unwrap();
            if installation.project == self.session.project.project_id {
                match result {
                    Ok(installed) => {
                        let settings = installed.settings;
                        match installation.target {
                            Target::Effects(target, insertion) => {
                                let id = settings.id;
                                if let Some(edit) =
                                    self.installed_effect_edit(target, insertion, settings)
                                {
                                    self.set_plugin_metadata(ctx, id, installed.metadata);
                                    self.session.plugin_states.merge(&installed.states);
                                    self.edit(edit);
                                }
                            }
                            Target::Source { track, layer } => {
                                if let Some(mut instrument) =
                                    self.session.project.instrument_snapshot(track)
                                    && let Some(destination) =
                                        instrument.layers.iter_mut().find(|l| l.id == layer)
                                {
                                    self.set_plugin_metadata(ctx, settings.id, installed.metadata);
                                    self.session.plugin_states.merge(&installed.states);
                                    destination.clear_sources();
                                    destination.hosted =
                                        Some(daw_core::HostedSource::Known(settings));
                                    self.edit(Edit::SetInstrument {
                                        track_id: track,
                                        instrument: Some(instrument),
                                    });
                                }
                            }
                        }
                    }
                    Err(error) => self.fail(error),
                }
            }
        }
        if !ctx.input(|i| i.pointer.any_down()) && self.hosting.gestures.is_empty() {
            let dirty: Vec<_> = self
                .session
                .project
                .hosted_units()
                .into_iter()
                .filter_map(|(_, s)| (s.state.is_some() && !s.recallable()).then_some(s.id))
                .collect();
            for unit in dirty {
                self.request_plugin_capture(unit);
            }
        }
    }
    fn installed_effect_edit(
        &mut self,
        target: EffectChainTarget,
        insertion: Option<EffectInsertion>,
        settings: HostedSettings,
    ) -> Option<Edit> {
        let (effects, compact) = self.effect_chain(target)?;
        if insertion.as_ref().is_some_and(|slot| {
            slot.index > effects.len()
                || !effects
                    .iter()
                    .map(|effect| effect.id)
                    .eq(slot.order.iter().copied())
        }) {
            // Another edit changed the chain while the plugin was loading.
            return None;
        }
        let mut effects = effects.to_vec();
        let id = Id::new_v4();
        let index = insertion.as_ref().map_or(effects.len(), |slot| slot.index);
        effects.insert(
            index,
            daw_core::EffectInstance {
                id,
                unit: daw_core::EffectSettings::Hosted(settings),
            },
        );
        let edit = target.edit(effects, compact);
        if let Some(slot) = insertion
            && matches!(target, EffectChainTarget::Layer { .. })
        {
            match self.session.project.position_inline_effects(
                daw_core::EffectTransfer {
                    edit,
                    selected: vec![id],
                },
                target,
                slot.inline_before,
            ) {
                Ok(transfer) => Some(transfer.edit),
                Err(error) => {
                    self.fail(error);
                    None
                }
            }
        } else {
            Some(edit)
        }
    }

    pub(super) fn can_install_external_effect(&self, identity: &PluginIdentity) -> bool {
        self.hosting.client.is_some()
            && self.hosting.installation.is_none()
            && self
                .hosting
                .catalog
                .iter()
                .any(|plugin| &plugin.identity == identity && plugin.role == HostedRole::Effect)
    }

    pub(super) fn install_external_effect(
        &mut self,
        identity: PluginIdentity,
        target: EffectChainTarget,
        index: usize,
        order: Vec<Id>,
        inline_before: Option<daw_core::InstrumentComponent>,
    ) {
        if self.can_install_external_effect(&identity) {
            self.request_plugin_install(
                identity,
                Target::Effects(
                    target,
                    Some(EffectInsertion {
                        index,
                        order,
                        inline_before,
                    }),
                ),
            );
        }
    }

    pub(super) fn add_external_plugin(&mut self, identity: PluginIdentity) {
        if self.hosting.installation.is_some() {
            return;
        }
        let plugin = self
            .hosting
            .catalog
            .iter()
            .find(|p| p.identity == identity)
            .cloned();
        if let Some(plugin) = plugin {
            let target = match plugin.role {
                HostedRole::Effect => self
                    .effect_catalog_target()
                    .map(|target| Target::Effects(target, None)),
                HostedRole::Instrument => {
                    self.instrument_catalog_target().and_then(|(track, layer)| {
                        self.session
                            .project
                            .instrument_snapshot(track)
                            .filter(|i| i.layers.iter().any(|l| l.id == layer))
                            .map(|_| Target::Source { track, layer })
                    })
                }
                _ => None,
            };
            let Some(target) = target else {
                self.fail("Select an effect lane, or a regular MIDI/beat layer for an instrument");
                return;
            };
            self.request_plugin_install(plugin.identity, target);
        }
    }

    fn request_plugin_install(&mut self, identity: PluginIdentity, target: Target) {
        if let Some(client) = &self.hosting.client {
            match client.install(
                identity,
                self.session.project.sample_rate_hz,
                self.session.project.buffer_size_frames,
            ) {
                Ok(receipt) => {
                    self.hosting.installation = Some(Installation {
                        project: self.session.project.project_id,
                        target,
                        receipt,
                    });
                }
                Err(error) => self.fail(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        HostedParameter, HostedParameterBinding, HostedParameterDomain, HostedPluginDescriptor,
    };
    #[test]
    fn external_effect_drop_loads_only_on_release_and_inserts_at_the_preview() {
        use crate::tests::{button, context, frame};
        for (identity, format_index, inline) in [
            (PluginIdentity::Vst3([1; 16]), 0, false),
            (PluginIdentity::Clap("org.daw.drop-test".into()), 1, false),
            (
                PluginIdentity::Clap("org.daw.layer-drop-test".into()),
                1,
                true,
            ),
        ] {
            let ctx = context();
            let mut app = DawUi::default();
            let original = daw_core::EffectInstance::drive();
            let (track, target) = if inline {
                let track = app.session.project.add_midi_track().unwrap();
                let layer = &mut app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0];
                layer.effects.push(original.clone());
                (
                    track,
                    EffectChainTarget::Layer {
                        track,
                        layer: layer.id,
                    },
                )
            } else {
                let track = app.session.project.add_track().unwrap();
                app.session.project.tracks[0].effects.push(original.clone());
                (track, EffectChainTarget::Track(track))
            };
            let other = app.session.project.add_track().unwrap();
            app.select_track(track, false);
            if inline {
                app.set_view_mode(ViewMode::Instrument, &ctx);
            }
            app.set_effects_lane_visible(!inline);
            app.set_effects_catalog_visible(true);
            let (_host, client) = daw_plugin_host::HostService::new(Default::default());
            app.enable_plugin_hosting(client);
            let plugin = HostedPluginDescriptor {
                identity: identity.clone(),
                name: "Drop test".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: HostedRole::Effect,
            };
            app.hosting.catalog.push(plugin.clone());
            app.effects_catalog.set_external(vec![plugin.clone()]);
            app.effects_catalog
                .filter_folder(Some(crate::effects_catalog::CATEGORY_COUNT + format_index));
            frame(&mut app, &ctx, vec![]);
            assert!(
                app.effects_bounds.external_entry.is_positive(),
                "{identity:?}"
            );
            let start = app.effects_bounds.external_entry.center();
            let end = if inline {
                let (_, _, rect) = app
                    .instrument_ui
                    .effect_slots
                    .iter()
                    .find(|(_, id, _)| *id == original.id)
                    .unwrap();
                Pos2::new(rect.left() + 2.0, rect.center().y)
            } else {
                let lane = app.effects_bounds.lane_viewport;
                Pos2::new(
                    app.effects_bounds.chain[0].1.left().max(lane.left()) + 2.0,
                    lane.center().y,
                )
            };
            let before = app.session.project.clone();
            for release in [app.effects_bounds.library.center(), end] {
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
                assert!(
                    app.effects_editing_available(&ctx),
                    "effect editing blocked: {:?}",
                    app.error
                );
                assert!(app.can_install_external_effect(&identity));
                frame(&mut app, &ctx, vec![button(start, true)]);
                assert!(
                    app.effects_drag.is_some(),
                    "press failed for {identity:?} at {start:?}"
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(release)]);
                assert!(
                    app.effects_drag
                        .as_ref()
                        .is_some_and(|drag| drag.active && drag.is_library()),
                    "motion failed for {identity:?} from {start:?} to {release:?}"
                );
                assert!(app.hosting.installation.is_none());
                assert_eq!(app.session.project, before);
                frame(&mut app, &ctx, vec![button(release, false)]);
                assert!(app.effects_drag.is_none());
                assert_eq!(app.session.project, before);
            }
            let pending = app
                .hosting
                .installation
                .as_mut()
                .expect("valid drop requests a load");
            let (send, receive) = mpsc::channel();
            pending.receipt = receive;
            app.select_track(other, false);
            send.send(Ok(InstalledPlugin {
                settings: HostedSettings::new(plugin, Vec::new()),
                states: StateBank::default(),
                metadata: NativeMetadata {
                    sample_rate_hz: 48000,
                    parameters: Vec::new(),
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    latency_frames: 0,
                    tail_frames: 0,
                    state_supported: false,
                    editor_supported: false,
                },
            }))
            .unwrap();
            app.poll_hosting(&ctx);
            let (effects, _) = app.effect_chain(target).unwrap();
            assert_eq!(effects.len(), 2, "{:?}", app.error);
            assert!(
                matches!(&effects[0].unit, daw_core::EffectSettings::Hosted(s) if s.plugin.identity == identity)
            );
            assert_eq!(effects[1], original);
            assert!(app.session.project.tracks[1].effects.is_empty());
        }
    }

    #[test]
    fn native_preset_values_and_opaque_receipt_form_one_undo_step() {
        let mut app = DawUi {
            session: daw_project::Session::default(),
            ..Default::default()
        };
        let track = app.session.project.add_track().unwrap();
        let mut settings = HostedSettings::new(
            HostedPluginDescriptor {
                identity: PluginIdentity::Clap("org.daw.history-fixture".into()),
                name: "Fixture".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: HostedRole::Effect,
            },
            [10, 20]
                .map(|id| {
                    HostedParameterBinding::new(
                        HostedParameter {
                            id,
                            name: format!("Control {id}"),
                            module: String::new(),
                            domain: HostedParameterDomain::Normalized { step_count: 0 },
                            default_value: 0.5,
                            automatable: true,
                            modulatable: false,
                            read_only: false,
                            hidden: false,
                        },
                        0.5,
                    )
                })
                .into(),
        );
        settings.state = Some(
            app.session
                .plugin_states
                .capture(NativeState::Clap(vec![1]), 0)
                .unwrap(),
        );
        app.session
            .project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![daw_core::EffectInstance {
                    id: Id::new_v4(),
                    unit: daw_core::EffectSettings::Hosted(settings.clone()),
                }],
                compact: false,
            })
            .unwrap();
        let ctx = egui::Context::default();
        for event in [
            NativeNotification::PresetBegin,
            NativeNotification::Value(10, 0.25),
            NativeNotification::Value(20, 0.75),
            NativeNotification::StateDirty,
            NativeNotification::PresetEnd,
        ] {
            app.plugin_notification(settings.id, event, &ctx);
        }
        let before = app
            .session
            .project
            .hosted_unit(settings.id)
            .unwrap()
            .clone();
        assert!(
            app.plugin_state_receipt(&before, NativeState::Clap(vec![2, 3]))
                .unwrap()
        );
        let history = app.history.as_mut().unwrap();
        let old = history.undo(&app.session).unwrap();
        assert!(
            !history.can_undo(),
            "The complete preset must have one undo transaction"
        );
        assert_eq!(old.project.hosted_unit(settings.id).unwrap(), &settings);
        let new = history.redo(&old).unwrap();
        let unit = new.project.hosted_unit(settings.id).unwrap();
        assert_eq!(
            unit.parameters
                .iter()
                .map(|p| p.base_value)
                .collect::<Vec<_>>(),
            vec![0.25, 0.75]
        );
        assert_eq!(
            new.plugin_states
                .restore(unit.plugin.identity.format(), unit.state.as_ref().unwrap())
                .unwrap(),
            NativeState::Clap(vec![2, 3])
        );
    }
}
