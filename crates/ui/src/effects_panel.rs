use super::*;

impl DawUi {
    pub(super) fn effects_view(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let border = Stroke::new(1.0, theme::palette(ui.ctx()).border);
        ui.painter()
            .hline(ui.max_rect().x_range(), ui.max_rect().top() + 0.5, border);
        ui.add_space(1.0);
        let header = toolbars::Toolbar::row(ui, |ui| {
            ui.label("Effects");
        });
        ui.painter().hline(
            header.response.rect.x_range(),
            header.response.rect.bottom() - 0.5,
            border,
        );
        let selected = self
            .session
            .project
            .tracks
            .iter()
            .any(|track| Some(track.id) == self.selected_track);
        ui.add_space(f32::from(theme::TOOLBAR_PADDING));
        if selected {
            ui.label("Track effects are not available yet.");
        } else {
            ui.label("Select a track to configure its effects.");
        }
    }
}
