//! Send controls and independently pinned Return banks.
use super::*;
use daw_core::{SendTap, TrackKind, TrackSend};

impl DawUi {
    fn return_name(&self) -> String {
        let mut n = 1;
        loop {
            let name = format!("Return {n}");
            if !self.session.project.tracks.iter().any(|t| t.name == name) {
                return name;
            }
            n += 1;
        }
    }
    pub(super) fn send_rack(&mut self, ui: &mut egui::Ui, source: &daw_core::Track) {
        let returns: Vec<_> = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Return)
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let returns: Vec<_> = returns
            .iter()
            .enumerate()
            .map(|(i, (id, name))| {
                let label = if returns.iter().filter(|(_, other)| other == name).count() > 1 {
                    format!("{} · {name}", i + 1)
                } else {
                    name.clone()
                };
                (*id, label)
            })
            .collect();
        let mut sends = source.sends.clone();
        let mut remove = None;
        for send in &mut sends {
            ui.push_id(send.id, |ui| {
                if send_row(ui, send, &returns, &source.sends) {
                    remove = Some(send.id);
                }
            });
        }
        if let Some(id) = remove {
            sends.retain(|s| s.id != id);
        }
        if !source.sends.is_empty() {
            ui.add_space((mixer::OUT_CONTROL_SPACING - ui.spacing().item_spacing.y).max(0.0));
        }
        let button = ui.button("+ Send");
        let mut new_return = false;
        menus::Dropdown::show_from_button(&button, |ui| {
            menus::small_popup_style(ui.style_mut());
            for (id, name) in &returns {
                if ui
                    .add_enabled(
                        !sends.iter().any(|s| s.return_id == *id),
                        egui::Button::new(name),
                    )
                    .clicked()
                {
                    sends.push(TrackSend::new(*id));
                    ui.close();
                }
            }
            menus::separator(ui);
            if ui.button("New Return").clicked() {
                new_return = true;
                ui.close();
            }
        });
        if new_return {
            let id = Id::new_v4();
            sends.push(TrackSend::new(id));
            self.edit(Edit::Batch(vec![
                Edit::AddReturn {
                    track_id: id,
                    name: self.return_name(),
                },
                Edit::SetTrackSends {
                    track_id: source.id,
                    sends,
                },
            ]));
        } else if sends != source.sends {
            self.edit(Edit::SetTrackSends {
                track_id: source.id,
                sends,
            });
        }
    }

    pub(super) fn return_reorder(
        &mut self,
        ui: &egui::Ui,
        rows: &[(Id, Rect)],
        viewport: Rect,
        horizontal: bool,
    ) {
        let Some(id) = self.track_drag else { return };
        if !self
            .session
            .project
            .tracks
            .iter()
            .any(|t| t.id == id && t.kind == TrackKind::Return)
        {
            return;
        }
        if !ui.is_enabled()
            || !self.editing_enabled()
            || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape))
        {
            self.track_drag = None;
            return;
        }
        let all: Vec<_> = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Return)
            .map(|t| t.id)
            .collect();
        let moving: Vec<_> = all
            .iter()
            .copied()
            .filter(|&t| self.track_selected(t))
            .collect();
        let mut next: Vec<_> = all
            .iter()
            .copied()
            .filter(|t| !moving.contains(t))
            .collect();
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| viewport.contains(*p));
        if let Some(pointer) = pointer {
            let before = rows
                .iter()
                .find(|(_, r)| {
                    if horizontal {
                        pointer.x < r.center().x
                    } else {
                        pointer.y < r.center().y
                    }
                })
                .map(|(id, _)| *id);
            let at = before
                .and_then(|id| next.iter().position(|t| *t == id))
                .unwrap_or_else(|| {
                    rows.iter()
                        .filter(|(id, r)| {
                            !moving.contains(id)
                                && if horizontal {
                                    r.center().x < pointer.x
                                } else {
                                    r.center().y < pointer.y
                                }
                        })
                        .count()
                        .min(next.len())
                });
            next.splice(at..at, moving);
            if next != all {
                let line = before
                    .and_then(|id| rows.iter().find(|(i, _)| *i == id))
                    .map(|(_, r)| if horizontal { r.left() } else { r.top() })
                    .unwrap_or_else(|| {
                        rows.last().map_or(
                            if horizontal {
                                viewport.left()
                            } else {
                                viewport.top()
                            },
                            |(_, r)| if horizontal { r.right() } else { r.bottom() },
                        )
                    });
                let stroke = Stroke::new(2.0, theme::accent(ui.ctx()));
                if horizontal {
                    ui.painter_at(viewport)
                        .vline(line, viewport.y_range(), stroke);
                } else {
                    ui.painter_at(viewport)
                        .hline(viewport.x_range(), line, stroke);
                }
            }
            drag::scroll_drag(
                ui,
                if horizontal {
                    drag::Axis::Horizontal
                } else {
                    drag::Axis::Vertical
                },
                360.0,
                true,
                viewport,
            );
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            self.track_drag = None;
            if pointer.is_some() && next != all {
                self.edit(Edit::ReorderReturns { return_ids: next });
            }
        } else if !ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
            self.track_drag = None;
        }
    }

    pub(super) fn return_timeline_bank(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
    ) -> Vec<(Id, Rect)> {
        if !rect.is_positive() {
            return vec![];
        }
        let mut bank = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("return_timeline_bank")
                .max_rect(rect),
        );
        bank.set_clip_rect(rect.intersect(ui.clip_rect()));
        let rows = egui::ScrollArea::vertical()
            .id_salt("return_timeline_scroll")
            .auto_shrink([false, false])
            .show(&mut bank, |ui| {
                let rows = self.tracks_kind(ui, true);
                self.return_reorder(ui, &rows, rect, false);
                rows
            })
            .inner;
        let handle = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 4.0));
        let response = ui.interact(handle, ui.id().with("return_height"), Sense::drag());
        if response.dragged() {
            self.return_bank_height = (self.return_bank_height - response.drag_delta().y).max(48.0);
        }
        response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        rows
    }

    pub(super) fn return_mixer_bank(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        let tracks: Vec<_> = self
            .session
            .project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Return)
            .cloned()
            .collect();
        let mut bank = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("return_mixer_bank")
                .max_rect(rect),
        );
        bank.set_clip_rect(rect.intersect(ui.clip_rect()));
        egui::ScrollArea::horizontal()
            .id_salt("return_mixer_scroll")
            .auto_shrink([false, false])
            .show(&mut bank, |ui| {
                let mut rows = vec![];
                ui.horizontal_top(|ui| {
                    let height = ui.available_height();
                    for track in &tracks {
                        ui.push_id(track.id, |ui| {
                            let (r, response) = ui.allocate_exact_size(
                                Vec2::new(mixer::STRIP_WIDTH, height),
                                Sense::click(),
                            );
                            let mut strip =
                                ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("strip"));
                            let _ = self.mixer_strip(&mut strip, r, Some(track), true);
                            #[cfg(test)]
                            self.mixer_bounds.insert(track.id, r);
                            if self.editing_enabled()
                                && ui.rect_contains_pointer(r)
                                && ui.input(|i| i.focused && i.pointer.any_pressed())
                                && (!ui.input(|i| i.pointer.secondary_pressed())
                                    || !self.track_selected(track.id))
                            {
                                self.select_track(
                                    track.id,
                                    ui.input(|i| i.modifiers.shift || i.modifiers.command),
                                );
                            }
                            if let Some(action) = menus::ContextMenu::new(&response)
                                .show(|ui| self.track_context_menu(ui, Some(track.id)))
                                .and_then(|r| r.inner)
                            {
                                self.perform_track_action(action, ui.ctx());
                            }
                            if self.track_selected(track.id) {
                                panels::selection_border(ui, r);
                            }
                            rows.push((track.id, r));
                        });
                    }
                });
                self.return_reorder(ui, &rows, rect.intersect(ui.clip_rect()), true);
            });
    }
}

/// One send's controls and contextual actions; the caller commits the complete collection.
fn send_row(
    ui: &mut egui::Ui,
    send: &mut TrackSend,
    returns: &[(Id, String)],
    existing: &[TrackSend],
) -> bool {
    let name = returns
        .iter()
        .find(|(id, _)| *id == send.return_id)
        .map_or("Missing Return", |(_, name)| name.as_str());
    let response = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(menus::ITEM_PADDING, 0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let knob_size = knobs::HEADER_SIZE;
                let gap = f32::from(theme::TOOLBAR_PADDING);
                let left_width = (ui.available_width() - knob_size - gap).max(0.0);
                ui.spacing_mut().item_spacing.x = gap;
                ui.allocate_ui_with_layout(
                    Vec2::new(left_width, knob_size),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_width(left_width);
                        let border = ui.visuals().widgets.inactive.bg_stroke.width;
                        let group = rows::input_group(ui, |ui| {
                            let height = ui.spacing().interact_size.y;
                            ui.spacing_mut().button_padding.x = 0.0;
                            let color = if send.enabled {
                                theme::accent(ui.ctx())
                            } else {
                                theme::palette(ui.ctx()).secondary
                            };
                            let power = ui
                                .add_sized(
                                    Vec2::splat(height),
                                    egui::Button::image(
                                        icons::Lucide::Power.size(12.0).color(color).image(),
                                    )
                                    .selected(send.enabled),
                                )
                                .on_hover_text(format!("Send to {name}"));
                            power.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    send.enabled,
                                    format!("Send to {name}"),
                                )
                            });
                            if power.clicked() {
                                send.enabled = !send.enabled;
                            }
                            ui.spacing_mut().button_padding.x = 2.0;
                            ui.spacing_mut().icon_width = 10.0;
                            ui.spacing_mut().icon_spacing = 2.0;
                            let width = (left_width - 2.0 * border - height).max(0.0);
                            let destination = menus::Dropdown::from_id_salt("send_return")
                                .small_items()
                                .selected_text(egui::RichText::new(name).small())
                                .width(width)
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for (id, label) in returns {
                                        let available = *id == send.return_id
                                            || !existing.iter().any(|s| s.return_id == *id);
                                        if ui
                                            .add_enabled_ui(available, |ui| {
                                                ui.selectable_value(&mut send.return_id, *id, label)
                                            })
                                            .inner
                                            .clicked()
                                        {
                                            ui.close();
                                        }
                                    }
                                })
                                .response
                                .on_hover_text(name);
                            (power, destination)
                        });
                        let (power, destination) = group.inner;
                        rows::input_group_outline(
                            ui,
                            group.response.rect,
                            destination.rect.left(),
                            &[&power, &destination],
                        );
                    },
                );
                ui.add_enabled_ui(send.enabled, |ui| {
                    daw_unit_ui::knobs::send_gain_scaled(
                        ui,
                        &mut send.gain_db,
                        knob_size / knobs::SIZE,
                    );
                });
            });
        })
        .response;
    let context_click = ui.is_enabled()
        && ui.rect_contains_pointer(response.rect)
        && ui.input(|i| i.focused && i.pointer.secondary_clicked());
    let mut remove = false;
    menus::ContextMenu::new(&response)
        .open_memory(context_click.then_some(egui::SetOpenCommand::Bool(true)))
        .show(|ui| {
            if ui
                .button(if send.enabled {
                    "Disable send"
                } else {
                    "Enable send"
                })
                .clicked()
            {
                send.enabled = !send.enabled;
                ui.close();
            }
            for (tap, label) in [
                (SendTap::PreFader, "Pre-fader"),
                (SendTap::PostFader, "Post-fader"),
            ] {
                if ui.selectable_label(send.tap == tap, label).clicked() {
                    send.tap = tap;
                    ui.close();
                }
            }
            egui::containers::menu::SubMenuButton::from_button(
                egui::Button::new("Change Return")
                    .right_text(
                        icons::Lucide::ChevronRight
                            .size(icons::TOOLBAR_ICON_SIZE)
                            .image(),
                    )
                    .image_tint_follows_text_color(true),
            )
            .config(egui::containers::menu::MenuConfig::new().style(menus::small_popup_style))
            .ui(ui, |ui| {
                menus::layout(ui, menus::MIN_WIDTH, |ui| {
                    for (id, name) in returns {
                        let available =
                            *id == send.return_id || !existing.iter().any(|s| s.return_id == *id);
                        if ui.add_enabled(available, egui::Button::new(name)).clicked() {
                            send.return_id = *id;
                            ui.close();
                        }
                    }
                })
            });
            menus::separator(ui);
            if ui.button("Remove send").clicked() {
                remove = true;
                ui.close();
            }
        });
    remove
}
