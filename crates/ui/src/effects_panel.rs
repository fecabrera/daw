use super::*;

#[cfg(test)]
pub(crate) struct EffectsUiBounds {
    pub library: Rect,
    pub header: Rect,
    pub strip: Rect,
    pub scrollbar: Rect,
}

#[cfg(test)]
impl Default for EffectsUiBounds {
    fn default() -> Self {
        Self {
            library: Rect::NOTHING,
            header: Rect::NOTHING,
            strip: Rect::NOTHING,
            scrollbar: Rect::NOTHING,
        }
    }
}

impl DawUi {
    pub(super) fn effects_view(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let border = Stroke::new(1.0, theme::palette(ui.ctx()).border);
        ui.painter()
            .hline(ui.max_rect().x_range(), ui.max_rect().top() + 0.5, border);
        ui.add_space(1.0);
        let bounds = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
        let below_track = (bounds.height() - RULER_HEIGHT - track_overview::STRIP_HEIGHT).max(0.0);
        let strip_height =
            panels::unit_height(false).min((below_track - SCROLLBAR_HEIGHT).max(0.0));
        let strip = Rect::from_min_max(
            Pos2::new(bounds.left(), bounds.bottom() - strip_height),
            bounds.max,
        );
        let upper = Rect::from_min_max(bounds.min, Pos2::new(bounds.right(), strip.top()));
        let library = panels::ControlPanel::begin(ui, upper);
        let (track, assets) = self.track_overview_content(ui);
        self.prepare_track_overview(track.as_ref());
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(TRACK_WIDTH, upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(TRACK_WIDTH);
                    ui.set_min_height(upper.height());
                    let header = library.header(ui, "Effects", |_| {});
                    panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
                    #[cfg(test)]
                    {
                        self.effects_bounds.library =
                            Rect::from_min_size(bounds.min, Vec2::new(TRACK_WIDTH, upper.height()));
                        self.effects_bounds.header = header.response.rect;
                    }
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), upper.height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(upper.height());
                    let overview = Rect::from_min_size(
                        ui.max_rect().min + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
                        Vec2::new(
                            (ui.available_width() - track_overview::LABEL_WIDTH).max(0.0),
                            (upper.height() - SCROLLBAR_HEIGHT).max(0.0),
                        ),
                    );
                    self.timeline_pinch(ui, overview);
                    self.timeline_swipe(ui, overview);
                    self.track_overview_ruler(ui);
                    if let Some(track) = &track {
                        self.track_overview_strip(ui, track, &assets);
                    } else {
                        self.lane_bounds.clear();
                        self.timeline_bounds = Rect::NOTHING;
                        #[cfg(test)]
                        {
                            self.midi_editor.track_bounds = Rect::NOTHING;
                            self.midi_editor.clip_bounds.clear();
                        }
                        rows::centered(ui, track_overview::STRIP_HEIGHT, |ui| {
                            ui.add_space(track_overview::LABEL_WIDTH);
                            ui.label("Select a track to configure its effects.");
                        });
                    }
                    self.finish_clip_drag(ui);
                    let (remaining, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), ui.available_height().max(0.0)),
                        Sense::hover(),
                    );
                    let scrollbar = Rect::from_min_max(
                        Pos2::new(
                            remaining.left() + track_overview::LABEL_WIDTH,
                            strip.top() - SCROLLBAR_HEIGHT,
                        ),
                        Pos2::new(remaining.right(), strip.top()),
                    );
                    let grid = Rect::from_min_max(
                        Pos2::new(scrollbar.left(), remaining.top()),
                        scrollbar.right_top(),
                    );
                    if grid.is_positive() {
                        let painter = ui.painter_at(grid);
                        self.paint_grid(&painter, grid);
                        self.paint_track_overview_playhead(&painter, grid);
                    }
                    self.timeline_scrollbar_at(ui, scrollbar, "effects_timeline_scrollbar");
                    #[cfg(test)]
                    {
                        self.effects_bounds.scrollbar = scrollbar;
                    }
                },
            );
        });
        library.finish(ui);
        ui.painter()
            .rect_filled(strip, 0.0, theme::palette(ui.ctx()).background);
        // The status panel owns the single separator below the effects lane.
        panels::ControlPanel::row_dividers(ui, strip, true, false);
        #[cfg(test)]
        {
            self.effects_bounds.strip = strip;
        }
    }
}
