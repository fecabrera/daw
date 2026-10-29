//! Context-menu power actions share one target state and commit through their owning lane.
use daw_core::{
    EffectSettings, InstrumentBusSettings, InstrumentComponent, InstrumentLayerSettings,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const TAG: &str = "unit-power-menu";

/// Transient scope data lets nested component panels share the lane's group action.
#[derive(Clone)]
pub(super) struct PowerMenu {
    enable: bool,
    requested: Arc<AtomicBool>,
}
impl PowerMenu {
    pub fn new(states: impl IntoIterator<Item = bool>) -> Self {
        Self {
            enable: !states.into_iter().all(|enabled| enabled),
            requested: Default::default(),
        }
    }
    pub fn scope(&self, id: impl egui::AsIdSalt) -> egui::UiBuilder {
        egui::UiBuilder::new()
            .id_salt(id)
            .ui_stack_info(egui::UiStackInfo::default().with_tag_value(TAG, self.clone()))
    }
    pub fn action(&self) -> Option<bool> {
        self.requested
            .load(Ordering::Relaxed)
            .then_some(self.enable)
    }
    pub fn show(&self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                ui.input(|input| input.focused),
                egui::Button::new(if self.enable { "Enable" } else { "Disable" }),
            )
            .clicked()
        {
            self.requested.store(true, Ordering::Relaxed);
            ui.close();
        }
    }
    pub fn from_ui(ui: &egui::Ui) -> Option<Self> {
        ui.stack()
            .iter()
            .find_map(|s| s.tags().get_downcast::<Self>(TAG).cloned())
    }
}

pub(super) fn effect_enabled(settings: &mut EffectSettings) -> Option<&mut bool> {
    match settings {
        EffectSettings::Eq3(s) => Some(&mut s.enabled),
        EffectSettings::MultibandEq(s) => Some(&mut s.enabled),
        EffectSettings::Drive(s) => Some(&mut s.enabled),
        EffectSettings::DriveV2(s) => Some(&mut s.enabled),
        EffectSettings::AutoFilter(s) => Some(&mut s.enabled),
        EffectSettings::Filter(s) => Some(&mut s.enabled),
        EffectSettings::Compressor(s) => Some(&mut s.enabled),
        EffectSettings::Ott(s) => Some(&mut s.enabled),
        EffectSettings::Limiter(s) => Some(&mut s.enabled),
        EffectSettings::DirectionMixer(s) => Some(&mut s.enabled),
        EffectSettings::StereoSpread(s) => Some(&mut s.enabled),
        EffectSettings::Saturator(s) => Some(&mut s.enabled),
        EffectSettings::Bitcrusher(s) => Some(&mut s.enabled),
        EffectSettings::Chorus(s) => Some(&mut s.enabled),
        EffectSettings::Missing { .. } => None,
    }
}

pub(super) fn layer_enabled(
    owner: &mut InstrumentLayerSettings,
    unit: InstrumentComponent,
) -> Option<&mut bool> {
    match unit {
        InstrumentComponent::Oscillators => Some(&mut owner.oscillator_enabled),
        InstrumentComponent::Sampler => owner.sampler.as_mut().map(|s| &mut s.enabled),
        InstrumentComponent::Amplitude => Some(&mut owner.amplitude.enabled),
        InstrumentComponent::Envelope(id) => owner
            .envelopes
            .iter_mut()
            .find(|u| u.id == id)
            .map(|u| &mut u.enabled),
        InstrumentComponent::Lfo(id) => owner
            .lfos
            .iter_mut()
            .find(|u| u.id == id)
            .map(|u| &mut u.enabled),
        InstrumentComponent::Macros => Some(&mut owner.macro_module.enabled),
        _ => None,
    }
}

pub(super) fn bus_enabled(
    owner: &mut InstrumentBusSettings,
    unit: InstrumentComponent,
) -> Option<&mut bool> {
    match unit {
        InstrumentComponent::Envelope(id) => owner
            .envelopes
            .iter_mut()
            .find(|u| u.id == id)
            .map(|u| &mut u.enabled),
        InstrumentComponent::Lfo(id) => owner
            .lfos
            .iter_mut()
            .find(|u| u.id == id)
            .map(|u| &mut u.enabled),
        InstrumentComponent::Macros => Some(&mut owner.macro_module.enabled),
        _ => None,
    }
}

/// Popup closure clears focus at the end of its frame. Restore the owning header
/// on the next frame so the lane keeps its selection and keyboard actions.
pub(super) fn schedule_focus(ui: &egui::Ui, header: &egui::Response) {
    schedule_focus_id(ui.ctx(), header.id);
}
pub(super) fn schedule_focus_id(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("unit-menu-refocus"), Some(id)));
    ctx.request_repaint();
}
pub(super) fn restore_focus(ctx: &egui::Context) {
    if let Some(id) = ctx.data_mut(|data| {
        data.remove_temp::<Option<egui::Id>>(egui::Id::new("unit-menu-refocus"))
            .flatten()
    }) {
        ctx.memory_mut(|memory| memory.request_focus(id));
    }
}

fn focus_pending(ctx: &egui::Context) -> bool {
    ctx.data(|data| {
        data.get_temp::<Option<egui::Id>>(egui::Id::new("unit-menu-refocus"))
            .flatten()
            .is_some()
    })
}

pub(super) fn keeps_selection(ui: &egui::Ui) -> bool {
    ui.is_enabled()
        && ui.input(|input| input.focused)
        && (egui::Popup::is_any_open(ui.ctx()) || focus_pending(ui.ctx()))
}
