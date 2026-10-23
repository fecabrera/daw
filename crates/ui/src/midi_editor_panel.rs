//! Fixed editor options and the selected track's shared mixing controls.
use super::*;
use daw_core::TrackKind;
use daw_core::{MidiOptions, MidiScale as Scale};

impl DawUi {
    pub(super) fn editor_midi_options(&self) -> MidiOptions {
        self.editor_track()
            .filter(|track| track.kind == TrackKind::Midi)
            .map_or(MidiOptions::default(), |track| track.midi_options)
    }

    pub(super) fn midi_editor_panel(&mut self, ui: &mut egui::Ui, bounds: Rect) {
        let panel = panels::ControlPanel::begin(ui, bounds);
        let rect = Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, bounds.height()));
        #[cfg(test)]
        {
            self.midi_editor.panel_bounds = rect;
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("editor-options")
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing = Vec2::ZERO;
        let title = panel.header(&mut child, "Editor", |_| {});
        panels::ControlPanel::row_dividers(&child, title.response.rect, false, true);
        let header = Rect::from_min_size(
            child.next_widget_position(),
            Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
        );
        self.editor_track_header(&mut child);
        panels::ControlPanel::row_dividers(&child, header, false, true);
        let track_id = self
            .editor_track()
            .filter(|track| track.kind == TrackKind::Midi)
            .map(|track| track.id);
        let midi = track_id.is_some();
        let original = self.editor_midi_options();
        let mut options = original;
        let audio = self
            .editor_track()
            .is_some_and(|t| t.kind == TrackKind::Audio);
        let title = panel.header(
            &mut child,
            if audio {
                "Clip options"
            } else {
                "MIDI options"
            },
            |_| {},
        );
        panels::ControlPanel::row_dividers(&child, title.response.rect, false, true);
        #[cfg(test)]
        {
            self.midi_editor.scale_root_bounds = Rect::NOTHING;
            self.midi_editor.scale_mode_bounds = Rect::NOTHING;
            self.midi_editor.scale_snap_bounds = Rect::NOTHING;
        }
        if audio {
            let allowed = ui.is_enabled() && self.editing_enabled();
            let options_bounds = Rect::from_min_max(title.response.rect.min, rect.max);
            egui::ScrollArea::vertical()
                .id_salt("editor-clip-options-scroll")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    self.clip_settings_body(ui, &panel, options_bounds, allowed)
                });
        } else {
            self.clear_clip_settings_edits();
            egui::ScrollArea::vertical()
            .id_salt("editor-options-scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                panel.padded_body(ui, |ui| {
                    ui.add_space(8.0);
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                    ui.add_enabled_ui(midi && !self.midi_editor.dragging(), |ui| {
                        panel.field_row(ui, 45.0, |ui| { ui.label("Root"); }, |ui| {
                            const ROOTS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                            let response = egui::ComboBox::from_id_salt("scale-root")
                                .selected_text(options.root.map_or("Off", |root| ROOTS[usize::from(root)]))
                                .width(ui.available_width()).show_ui(ui, |ui| {
                                    ui.selectable_value(&mut options.root, None, "Off");
                                    for (root, name) in ROOTS.iter().enumerate() {
                                        ui.selectable_value(&mut options.root, Some(root as u8), *name);
                                    }
                                }).response;
                            #[cfg(test)] { self.midi_editor.scale_root_bounds = response.rect; }
                            #[cfg(not(test))] let _ = response;
                        });
                        ui.add_enabled_ui(options.root.is_some(), |ui| {
                            panel.field_row(ui, 45.0, |ui| { ui.label("Scale"); }, |ui| {
                                let response = egui::ComboBox::from_id_salt("scale-mode")
                                    .selected_text(options.scale.name()).width(ui.available_width())
                                    .show_ui(ui, |ui| {
                                        for scale in Scale::ALL {
                                            ui.selectable_value(&mut options.scale, scale, scale.name());
                                        }
                                    }).response;
                                #[cfg(test)] { self.midi_editor.scale_mode_bounds = response.rect; }
                                #[cfg(not(test))] let _ = response;
                            });
                            rows::left_aligned(ui, toolbars::CONTROL_HEIGHT, |ui| {
                                let response = ui.checkbox(&mut options.snap, "Snap to scale")
                                    .on_hover_text("Snap drawn notes and pitch moves. Group moves keep their intervals. Hold Shift to bypass.");
                                #[cfg(test)] { self.midi_editor.scale_snap_bounds = response.rect; }
                                #[cfg(not(test))] let _ = response;
                            });
                        });
                    });
                });
            });
        }
        if options != original
            && let Some(track_id) = track_id
        {
            self.edit(Edit::SetMidiOptions { track_id, options });
        }
        panel.finish(ui);
    }

    fn editor_track_header(&mut self, ui: &mut egui::Ui) {
        // Track selection can change inside this row. Resolve it again before mixing.
        panels::PanelStyle {
            margin: 6,
            ..Default::default()
        }
        .track_frame(ui.ctx())
        .show(ui, |ui| {
            ui.set_min_width(TRACK_WIDTH - 14.0);
            ui.set_min_height(ROW_HEIGHT - 14.0);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            let track = self.editor_track().cloned();
            let levels = track
                .as_ref()
                .and_then(|t| self.output.as_ref()?.track_meter(t.id))
                .map_or([(0.0, false); 2], |meter| meter.levels());
            let cleared = meters::stereo_panel(ui, ROW_HEIGHT - 14.0, levels, |ui| {
                let mut track = self.editor_track().cloned();
                rows::centered(ui, toolbars::CONTROL_HEIGHT, |ui| {
                    let name_width =
                        (ui.available_width() - 2.0 * toolbars::CONTROL_HEIGHT - 12.0).max(1.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(name_width, toolbars::CONTROL_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let icon = if track.as_ref().is_none_or(|t| t.kind == TrackKind::Midi) {
                                icons::Lucide::KeyboardMusic
                            } else {
                                icons::Lucide::AudioLines
                            };
                            icons::header_icon(ui, icon, Sense::hover());
                            let response = self.track_dropdown(ui, Some(ui.available_width()));
                            #[cfg(test)]
                            {
                                self.midi_editor.selector_bounds = response.rect;
                            }
                            #[cfg(not(test))]
                            let _ = response;
                        },
                    );
                    track = self.editor_track().cloned();
                    ui.add_enabled_ui(track.is_some() && !self.midi_editor.dragging(), |ui| {
                        let mut muted = track.as_ref().is_some_and(|t| t.muted);
                        let mut soloed = track.as_ref().is_some_and(|t| t.soloed);
                        let buttons = mix_controls::mute_solo(
                            ui,
                            &mut muted,
                            &mut soloed,
                            ["Mute track", "Solo track"],
                        );
                        #[cfg(test)]
                        {
                            self.midi_editor.mix_bounds = buttons.map(|r| r.rect);
                        }
                        #[cfg(not(test))]
                        let _ = buttons;
                        if let Some(t) = &mut track {
                            t.muted = muted;
                            t.soloed = soloed;
                        }
                    });
                });
                if let Some(mut track) = track {
                    let inputs = self.inputs.entry(track.id).or_insert_with(|| Inputs {
                        gain: faders::format_gain(track.gain_db),
                        pan: format!("{:.2}", track.pan),
                    });
                    let controls = ui
                        .add_enabled_ui(!self.midi_editor.dragging(), |ui| {
                            mix_controls::gain_pan(ui, &mut track.gain_db, &mut track.pan, inputs)
                        })
                        .inner;
                    if let Some(error) = controls.error {
                        self.fail(error);
                    }
                    if let Some(current) = self
                        .session
                        .project
                        .tracks
                        .iter_mut()
                        .find(|t| t.id == track.id)
                        && (current.gain_db != track.gain_db
                            || current.pan != track.pan
                            || current.muted != track.muted
                            || current.soloed != track.soloed)
                    {
                        current.gain_db = track.gain_db;
                        current.pan = track.pan;
                        current.muted = track.muted;
                        current.soloed = track.soloed;
                        self.changed();
                    }
                }
            });
            meters::reset_on_click(
                ui,
                track
                    .as_ref()
                    .and_then(|t| self.output.as_ref()?.track_meter(t.id)),
                cleared,
            );
        });
    }
}
