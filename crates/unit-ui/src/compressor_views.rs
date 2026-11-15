//! Transient view selection; band identities survive presentation reordering.
use crate::{
    compressor_curve,
    multiband_compressor::{Bounds, Controls},
    rows,
};
use daw_unit_core::{ModulationTargetId, MultibandCompressorSettings};
use egui::{Rect, Ui};

pub(crate) fn show(
    ui: &mut Ui,
    s: &mut MultibandCompressorSettings,
    dimensions: (bool, f32),
    c: &Controls<'_>,
    b: &mut Bounds,
) {
    let key = ui.id().with("compressor-band-view");
    let mut selected = ui
        .data(|d| d.get_temp::<ModulationTargetId>(key))
        .filter(|id| s.bands.iter().any(|band| band.id == *id));
    if !dimensions.0 {
        b.view_column = crate::layout::unit_strip_with_width(
            ui,
            dimensions.1,
            22.0 + 2.0 * f32::from(crate::layout::PANEL_MARGIN),
            egui::UiBuilder::new().id_salt("view-selector"),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("band-views")
                    .max_height(dimensions.1)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = crate::layout::UNIT_STRIP_GAP;
                        let all = rows::filled_square_toggle(ui, "A", selected.is_none())
                            .on_hover_text("All bands");
                        b.general_view = all.rect;
                        if all.clicked() {
                            selected = None;
                        }
                        let labels: Vec<_> = s
                            .bands
                            .iter()
                            .enumerate()
                            .map(|(i, band)| (egui::Id::new(band.id), (i + 1).to_string()))
                            .collect();
                        let group = rows::vertical_toggle_group(
                            ui,
                            &labels,
                            selected.and_then(|id| s.bands.iter().position(|band| band.id == id)),
                        );
                        for (index, response) in group.inner.into_iter().enumerate() {
                            let response =
                                response.on_hover_text(format!("Band {} curve", index + 1));
                            let id = s.bands[index].id;
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    response.enabled(),
                                    selected == Some(id),
                                    format!("Band {} curve", index + 1),
                                )
                            });
                            b.view_buttons.push(response.rect);
                            if response.clicked() {
                                selected = if selected == Some(id) { None } else { Some(id) };
                            }
                        }
                    });
            },
        )
        .response
        .rect;
    }
    b.selected_band = selected;
    if let Some(id) = selected {
        let index = s.bands.iter().position(|band| band.id == id).unwrap();
        let mut settings = s.bands[index].compressor_settings();
        settings.enabled &= s.enabled;
        let mut input = (c.curve_input)(id);
        input.live_edits = c.live_graph_edits;
        input.detector_level = crate::meters::unit_reading(settings.enabled, input.detector_level);
        let reduction = c
            .reductions
            .get(index)
            .copied()
            .unwrap_or(0.0.into())
            .for_unit(settings.enabled);
        let curve = ui
            .push_id(id, |ui| {
                compressor_curve::show(ui, &mut settings, dimensions, input, reduction)
            })
            .inner;
        s.bands[index].threshold_db = settings.threshold_db;
        s.bands[index].ratio = settings.ratio;
        b.graph = curve.section;
        b.plot = curve.plot;
        b.curve = Some((id, curve));
    } else {
        let graph =
            crate::compressor_graph::show(ui, s, dimensions, c.spectrum, c.live_graph_edits);
        b.graph = graph.section;
        b.plot = graph.plot;
        b.crossover_markers = graph.markers;
        b.crossover_preview = graph.preview;
        b.crossover_edits = graph.edits;
        b.threshold_markers = graph.thresholds;
        b.threshold_preview = graph.threshold_preview;
        b.threshold_edits = graph.threshold_edits;
    }
    ui.data_mut(|d| {
        if let Some(id) = selected {
            d.insert_temp(key, id);
        } else {
            d.remove::<ModulationTargetId>(key);
        }
    });
    // Do not leave stale hit targets on the hidden graph.
    if dimensions.0 {
        b.graph = Rect::NOTHING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, Vec2};
    fn pointer(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }
    }
    fn draw(
        ctx: &egui::Context,
        s: &mut MultibandCompressorSettings,
        events: Vec<Event>,
        compact: bool,
    ) -> Bounds {
        let mut b = Bounds::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 400.0))),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| {
                let mut solos = vec![false; s.bands.len()];
                b = crate::multiband_compressor::show(
                    ui,
                    s,
                    (compact, crate::layout::unit_control_height()),
                    Controls {
                        live_graph_edits: false,
                        spectrum: None,
                        curve_input: &|_| compressor_curve::Input {
                            live_edits: false,
                            detector_level: 0.25,
                            history: Some(Default::default()),
                        },
                        solos: &mut solos,
                        band_markers: &[],
                        output_markers: &[None, None],
                        level: 0.0.into(),
                        levels: &[],
                        reductions: &[],
                        add_enabled: true,
                    },
                );
            },
        );
        if !compact {
            for (label, rect) in std::iter::once(("A".to_owned(), b.general_view)).chain(
                b.view_buttons
                    .iter()
                    .enumerate()
                    .map(|(i, rect)| ((i + 1).to_string(), *rect)),
            ) {
                let center = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(t) if t.galley.text() == label => {
                            Some(t.pos + t.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    (center - rect.center()).length() <= 0.75,
                    "{label}: {center:?} vs {rect:?}"
                );
            }
        }
        output.textures_delta.clear();
        b
    }
    #[test]
    fn band_view_drags_commit_both_values_only_to_selected_identity_and_toggle_back() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let mut s = MultibandCompressorSettings::default();
        let before = s.clone();
        let b = draw(&ctx, &mut s, vec![], false);
        assert!(b.curve.is_none());
        let margin = f32::from(crate::layout::PANEL_MARGIN);
        assert_eq!(b.general_view.left() - b.view_column.left(), margin);
        assert_eq!(b.general_view.top() - b.view_column.top(), margin);
        assert_eq!(b.view_column.right() - b.general_view.right(), margin);
        assert_eq!(
            b.view_buttons[0].top() - b.general_view.bottom(),
            crate::layout::UNIT_STRIP_GAP
        );
        assert_eq!(b.view_buttons[1].top() - b.view_buttons[0].bottom(), 1.0);
        let button = b.view_buttons[1].center();
        assert!(button.x < b.graph.left());
        draw(
            &ctx,
            &mut s,
            vec![Event::PointerMoved(button), pointer(button, true)],
            false,
        );
        let b = draw(&ctx, &mut s, vec![pointer(button, false)], false);
        assert_eq!(s, before, "view selection is not a project edit");
        assert_eq!(b.selected_band, Some(before.bands[1].id));
        assert!(b.crossover_markers.is_empty());
        let start = b.curve.unwrap().1.handle.center();
        draw(
            &ctx,
            &mut s,
            vec![Event::PointerMoved(start), pointer(start, true)],
            false,
        );
        let end = start + Vec2::new(25.0, 18.0);
        let b = draw(&ctx, &mut s, vec![Event::PointerMoved(end)], false);
        assert!(b.curve.unwrap().1.preview.is_some());
        assert_eq!(s, before, "DAW drafts commit only on release");
        draw(&ctx, &mut s, vec![pointer(end, false)], false);
        assert_ne!(s.bands[1].threshold_db, before.bands[1].threshold_db);
        assert_ne!(s.bands[1].ratio, before.bands[1].ratio);
        assert_eq!(s.bands[0], before.bands[0]);
        assert_eq!(s.bands[2], before.bands[2]);
        let after = s.clone();
        let button = draw(&ctx, &mut s, vec![], false).general_view.center();
        draw(
            &ctx,
            &mut s,
            vec![Event::PointerMoved(button), pointer(button, true)],
            false,
        );
        let b = draw(&ctx, &mut s, vec![pointer(button, false)], false);
        assert!(b.curve.is_none());
        assert_eq!(b.crossover_markers.len(), 2);
        assert_eq!(s, after);
    }
    #[test]
    fn presentation_reorder_and_compact_mode_retain_selection_removal_returns_general_view() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let mut s = MultibandCompressorSettings::default();
        let b = draw(&ctx, &mut s, vec![], false);
        let button = b.view_buttons[1].center();
        draw(
            &ctx,
            &mut s,
            vec![Event::PointerMoved(button), pointer(button, true)],
            false,
        );
        let b = draw(&ctx, &mut s, vec![pointer(button, false)], false);
        let selected = b.selected_band.unwrap();
        s.presentation = s.bands.iter().rev().map(|b| b.id).collect();
        let b = draw(&ctx, &mut s, vec![], true);
        assert_eq!(b.selected_band, Some(selected));
        assert!(b.view_buttons.is_empty());
        assert_eq!(b.graph, Rect::NOTHING);
        let b = draw(&ctx, &mut s, vec![], false);
        assert_eq!(b.selected_band, Some(selected));
        s.remove_band(selected);
        let b = draw(&ctx, &mut s, vec![], false);
        assert!(b.selected_band.is_none());
        assert!(b.curve.is_none());
    }
}
