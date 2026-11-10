//! Passive output rails in the existing space after instrument audio units.
use crate::{DawUi, meters};
use daw_core::{Id, InstrumentComponent, InstrumentLayerSettings};

impl DawUi {
    pub(super) fn component_output_meter(
        &self,
        ui: &mut egui::Ui,
        track: Id,
        layer: &InstrumentLayerSettings,
        component: InstrumentComponent,
        rect: egui::Rect,
    ) {
        let target = match component {
            InstrumentComponent::Oscillators
            | InstrumentComponent::Sampler
            | InstrumentComponent::Noise => daw_engine::SourceOutputTarget::for_layer(track, layer),
            InstrumentComponent::Amplitude if layer.amplitude.present => Some((
                daw_engine::SourceOutputTarget::amplitude(track, layer.id),
                layer.amplitude.enabled,
            )),
            _ => None,
        };
        if let Some((target, enabled)) = target {
            meters::unit_gap_level(
                ui,
                rect,
                ui.id().with((target, "gap-output")),
                meters::StereoOutputLevel::new(
                    self.output
                        .as_ref()
                        .map_or([0.0; 2], |o| o.source_output_levels(target)),
                    self.output
                        .as_ref()
                        .and_then(|o| o.source_output_meter(target)),
                ),
                enabled,
            );
        }
    }
}
