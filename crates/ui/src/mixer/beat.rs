//! Beat lanes use the track-group hierarchy and the shared Mixer strip controls.
use super::*;

impl DawUi {
    pub(super) fn mixer_beat_children(
        &mut self,
        ui: &mut egui::Ui,
        parent: &daw_core::Track,
        viewport: Rect,
        outlines: &mut Vec<(Id, Rect)>,
    ) {
        if parent.group_collapsed {
            return;
        }
        let Some(beat) = &parent.beat else {
            return;
        };
        for row in &beat.subtracks {
            ui.push_id(row.id, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(STRIP_WIDTH, ui.available_height()),
                    Sense::click(),
                );
                let mut strip =
                    ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt("strip"));
                self.beat_mixer_strip(&mut strip, rect, parent, row, rect.left() > viewport.left());
                #[cfg(test)]
                self.mixer_bounds.insert(row.id, rect);
                if ui.is_enabled()
                    && self.editing_enabled()
                    && ui.rect_contains_pointer(rect)
                    && ui.input(|i| i.focused && i.pointer.any_pressed())
                {
                    let selected = self
                        .selected_mixer_beat_row()
                        .is_some_and(|(id, _)| id == parent.id)
                        && self.beat_subtrack_selected(row.id);
                    if ui.input(|i| i.pointer.primary_pressed()) || !selected {
                        self.select_track(parent.id, false);
                        self.navigate_beat_owner(
                            parent.id,
                            Some(row.id),
                            ui.input(|i| {
                                i.pointer.primary_pressed()
                                    && (i.modifiers.shift || i.modifiers.command)
                            }),
                        );
                    }
                }
                self.beat_header_menu(
                    ui,
                    parent.id,
                    &crate::beat_editor::BeatRow::from(row),
                    &response,
                    rect,
                );
                outlines.push((row.id, rect));
            });
        }
    }

    pub(super) fn beat_mixer_name(
        &mut self,
        ui: &mut egui::Ui,
        parent: &daw_core::Track,
        row: &daw_core::BeatSubtrack,
    ) {
        rows::left_aligned(ui, toolbars::CONTROL_HEIGHT, |ui| {
            self.center_mixer_header(ui, &row.name);
            let color = header_text(ui.ctx());
            icons::header_icon_tinted(
                ui,
                icons::Lucide::AudioLines,
                Sense::hover(),
                Some(color),
                icons::TOOLBAR_ICON_SIZE,
            )
            .on_hover_text("Beat subtrack");
            let snapshot = crate::beat_editor::BeatRow::from(row);
            if self
                .beat_rename_input(ui, parent.id, &snapshot, panels::HEADER_FONT_SIZE)
                .is_some()
            {
                return;
            }
            let (response, text) = rows::name_button(
                ui,
                ("beat-mixer-name", row.id),
                &row.name,
                panels::HEADER_FONT_SIZE,
                self.beat_subtrack_selected(row.id),
                Sense::click(),
                Some(theme::ClipColors {
                    text: color,
                    ..theme::clip_colors(parent.color, None)
                }),
            );
            let response = response.on_hover_text(&row.name);
            if self.editing_enabled()
                && response.double_clicked()
                && response
                    .interact_pointer_pos()
                    .is_some_and(|p| text.is_some_and(|r| r.contains(p)))
            {
                self.begin_beat_rename(row.id, row.name.clone());
            }
        });
    }

    /// Track-menu operations follow the selected lane, as its strip controls do.
    pub(crate) fn mixer_beat_action(&mut self, action: TrackAction) -> bool {
        let Some((parent, row)) = self.selected_mixer_beat_row() else {
            return false;
        };
        match action {
            TrackAction::Mute | TrackAction::Solo => self.timeline_beat_mix_action(action),
            TrackAction::Rename => {
                self.begin_beat_rename(row.id, row.name.clone());
                true
            }
            TrackAction::Delete => {
                let ids = self
                    .session
                    .project
                    .beat(parent)
                    .unwrap()
                    .subtracks
                    .iter()
                    .filter(|row| self.beat_subtrack_selected(row.id))
                    .map(|row| row.id)
                    .collect();
                self.edit_beat(parent, daw_core::BeatEdit::Remove { ids });
                true
            }
            _ => false,
        }
    }
}
