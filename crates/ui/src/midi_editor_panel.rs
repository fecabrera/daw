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
        let header = rows::fixed(&mut child, self.track_row_height(), |ui| {
            self.editor_track_header(ui)
        });
        #[cfg(test)]
        {
            self.midi_editor.track_header_bounds = header.response.rect;
        }
        panels::ControlPanel::row_dividers(&child, header.response.rect, false, true);
        let track_id = self
            .editor_track()
            .filter(|track| track.kind == TrackKind::Midi)
            .map(|track| track.id);
        let midi = track_id.is_some();
        let original = self.editor_midi_options();
        let mut options = original;
        let beat_track = self
            .editor_track()
            .filter(|t| t.kind == TrackKind::Beat)
            .map(|t| t.id);
        let beat = beat_track.is_some();
        let audio = self
            .editor_track()
            .is_some_and(|t| t.kind == TrackKind::Audio);
        let title = rows::fixed(&mut child, toolbars::HEIGHT, |ui| {
            if let Some(track) = beat_track {
                let header = self.instrument_lanes_header(
                    ui,
                    &panel,
                    "Beat subtracks",
                    "Add beat subtrack",
                    self.editing_enabled(),
                );
                if header.inner.clicked() {
                    self.add_beat_subtrack(track);
                }
                header.response
            } else {
                panel
                    .header(
                        ui,
                        if audio {
                            "Clip options"
                        } else {
                            "MIDI options"
                        },
                        |_| {},
                    )
                    .response
            }
        });
        panels::ControlPanel::row_dividers(&child, title.response.rect, false, true);
        #[cfg(test)]
        {
            self.midi_editor.scale_root_bounds = Rect::NOTHING;
            self.midi_editor.scale_mode_bounds = Rect::NOTHING;
            self.midi_editor.scale_snap_bounds = Rect::NOTHING;
            self.midi_editor.velocity_bounds = Rect::NOTHING;
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
        } else if !beat {
            self.clear_clip_settings_edits();
            egui::ScrollArea::vertical()
                .id_salt("editor-options-scroll")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    panel.padded_body(ui, |ui| {
                        ui.add_space(8.0);
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                        ui.add_enabled_ui(midi && !self.midi_editor.editing_note(), |ui| {
                            panel.field_row(
                                ui,
                                45.0,
                                |ui| {
                                    ui.label("Root");
                                },
                                |ui| {
                                    const ROOTS: [&str; 12] = [
                                        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#",
                                        "B",
                                    ];
                                    let response =
                                        crate::menus::Dropdown::from_id_salt("scale-root")
                                            .selected_text(
                                                options
                                                    .root
                                                    .map_or("Off", |root| ROOTS[usize::from(root)]),
                                            )
                                            .width(ui.available_width())
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut options.root, None, "Off");
                                                for (root, name) in ROOTS.iter().enumerate() {
                                                    ui.selectable_value(
                                                        &mut options.root,
                                                        Some(root as u8),
                                                        *name,
                                                    );
                                                }
                                            })
                                            .response;
                                    #[cfg(test)]
                                    {
                                        self.midi_editor.scale_root_bounds = response.rect;
                                    }
                                    #[cfg(not(test))]
                                    let _ = response;
                                },
                            );
                            ui.add_enabled_ui(options.root.is_some(), |ui| {
                                panel.field_row(
                                    ui,
                                    45.0,
                                    |ui| {
                                        ui.label("Scale");
                                    },
                                    |ui| {
                                        let response =
                                            crate::menus::Dropdown::from_id_salt("scale-mode")
                                                .selected_text(options.scale.name())
                                                .width(ui.available_width())
                                                .show_ui(ui, |ui| {
                                                    for scale in Scale::ALL {
                                                        ui.selectable_value(
                                                            &mut options.scale,
                                                            scale,
                                                            scale.name(),
                                                        );
                                                    }
                                                })
                                                .response;
                                        #[cfg(test)]
                                        {
                                            self.midi_editor.scale_mode_bounds = response.rect;
                                        }
                                        #[cfg(not(test))]
                                        let _ = response;
                                    },
                                );
                                rows::left_aligned(ui, toolbars::CONTROL_HEIGHT, |ui| {
                                    let response = ui
                                        .checkbox(&mut options.snap, "Snap to scale")
                                        .on_hover_text(format!(
                                            "Snap to scale: {}",
                                            if options.snap { "On" } else { "Off" }
                                        ));
                                    #[cfg(test)]
                                    {
                                        self.midi_editor.scale_snap_bounds = response.rect;
                                    }
                                    #[cfg(not(test))]
                                    let _ = response;
                                });
                            });
                        });
                        if let Some(track_id) = track_id {
                            self.midi_velocity_row(ui, &panel, track_id);
                        }
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

    fn midi_velocity_row(&mut self, ui: &mut egui::Ui, panel: &panels::ControlPanel, track_id: Id) {
        let selected = self.midi_editor.selected_notes().clone();
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.id == track_id)
        else {
            return;
        };
        let patterns: Vec<_> = track
            .clips
            .iter()
            .filter_map(|clip| {
                let pattern = self
                    .session
                    .project
                    .assets
                    .iter()
                    .find(|asset| asset.id == clip.asset_id)?
                    .midi_pattern
                    .as_ref()?;
                pattern
                    .notes
                    .iter()
                    .any(|note| selected.contains(&(clip.id, note.id)))
                    .then_some((clip.id, pattern))
            })
            .collect();
        let note = patterns.iter().find_map(|(clip, pattern)| {
            pattern
                .notes
                .iter()
                .find(|note| selected.contains(&(*clip, note.id)))
        });
        let mut velocity = note.map_or(100, |note| note.velocity);
        let original_velocity = i16::from(velocity);
        let (minimum, maximum) = patterns
            .iter()
            .flat_map(|(clip, pattern)| {
                pattern
                    .notes
                    .iter()
                    .filter(|note| selected.contains(&(*clip, note.id)))
            })
            .fold((127_i16, 1_i16), |(minimum, maximum), note| {
                let velocity = i16::from(note.velocity);
                (minimum.min(velocity), maximum.max(velocity))
            });
        let response = ui
            .add_enabled_ui(
                note.is_some() && self.editing_enabled_with_midi_selection(true),
                |ui| {
                    panel
                        .field_row(
                            ui,
                            45.0,
                            |ui| {
                                ui.label("Velocity");
                            },
                            |ui| {
                                ui.scope(|ui| {
                                    let font = egui::TextStyle::Small.resolve(ui.style());
                                    ui.style_mut()
                                        .text_styles
                                        .insert(egui::TextStyle::Body, font);
                                    ui.style_mut().drag_value_text_style = egui::TextStyle::Small;
                                    ui.spacing_mut().interact_size = Vec2::new(32.0, 14.0);
                                    ui.spacing_mut().slider_rail_height =
                                        toolbars::SLIDER_RAIL_HEIGHT;
                                    // Reserve the value input and its standard gap so the
                                    // complete control stays inside the panel's right inset.
                                    ui.spacing_mut().slider_width = (ui.available_width()
                                        - ui.spacing().interact_size.x
                                        - ui.spacing().item_spacing.x)
                                        .max(1.0);
                                    ui.add(
                                        egui::Slider::new(&mut velocity, 1..=127)
                                            .trailing_fill(false)
                                            .handle_shape(egui::style::HandleShape::Circle),
                                    )
                                    .on_hover_text(format!("Velocity: {velocity}"))
                                })
                                .inner
                            },
                        )
                        .inner
                },
            )
            .inner;
        #[cfg(test)]
        {
            self.midi_editor.velocity_bounds = response.rect;
        }
        if response.changed() {
            // Clamp the shared offset, rather than individual notes, so no
            // velocity differences are lost at either MIDI boundary.
            let delta = (i16::from(velocity) - original_velocity).clamp(1 - minimum, 127 - maximum);
            if delta == 0 {
                return;
            }
            let commands: Vec<_> = patterns
                .into_iter()
                .filter_map(|(clip_id, pattern)| {
                    let mut pattern = pattern.clone();
                    let mut changed = false;
                    for note in &mut pattern.notes {
                        if selected.contains(&(clip_id, note.id)) {
                            note.velocity = (i16::from(note.velocity) + delta) as u8;
                            changed = true;
                        }
                    }
                    changed.then_some(Edit::ReplaceMidiPattern { clip_id, pattern })
                })
                .collect();
            if !commands.is_empty() {
                self.edit(Edit::Batch(commands));
            }
        }
    }

    fn editor_track_header(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        {
            self.midi_editor.track_mix_bounds = [Rect::NOTHING; 2];
        }
        panels::PanelStyle {
            margin: 6,
            ..Default::default()
        }
        .track_frame(
            ui.ctx(),
            self.editor_track()
                .is_some_and(|track| self.track_selected(track.id)),
        )
        .show(ui, |ui| {
            ui.set_min_width(TRACK_WIDTH - 14.0);
            ui.set_min_height(self.track_row_height() - 14.0);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            let track = self.editor_track().map(track_rows::header_snapshot);
            let (levels, held) = track
                .as_ref()
                .and_then(|t| self.output.as_ref()?.track_meter(t.id))
                .map_or(([(0.0, false); 2], [0.0; 2]), |meter| {
                    (meter.levels(), meter.held_peaks())
                });
            let height = self.track_row_height() - 14.0;
            let cleared = meters::stereo_panel_held(ui, height, levels, held, |ui| {
                let track = self.editor_track().map(track_rows::header_snapshot);
                let mut header_rows = track_rows::HeaderRows::track(ui, false);
                rows::centered(&mut header_rows.name, toolbars::CONTROL_HEIGHT, |ui| {
                    let response = rows::left_aligned(ui, toolbars::CONTROL_HEIGHT, |ui| {
                        if let Some(track) = track.as_ref() {
                            self.track_name(ui, track);
                        } else {
                            ui.label("Select track");
                        }
                    });
                    #[cfg(test)]
                    {
                        self.midi_editor.name_bounds = response.response.rect;
                    }
                    #[cfg(not(test))]
                    let _ = response;
                });
                if let Some(mut track) = track {
                    let (controls, buttons) = header_rows
                        .mix
                        .add_enabled_ui(!self.midi_editor.editing_note(), |ui| {
                            mix_controls::track_header_row(
                                ui,
                                (&mut track.gain_db, &mut track.pan),
                                &mut track.muted,
                                &mut track.soloed,
                                ["Mute track", "Solo track"],
                                false,
                            )
                        })
                        .inner;
                    #[cfg(test)]
                    {
                        self.midi_editor.track_mix_bounds = controls.bounds;
                        self.midi_editor.mix_bounds = buttons.map(|r| r.rect);
                    }
                    #[cfg(not(test))]
                    let _ = (controls, buttons);
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
                } else {
                    header_rows.mix.add_enabled_ui(false, |ui| {
                        rows::centered(ui, knobs::SIZE, |ui| {
                            mix_controls::mute_solo(
                                ui,
                                &mut false,
                                &mut false,
                                ["Mute track", "Solo track"],
                            );
                        });
                    });
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
