//! Granular controls use the same sample-input shell, strips and modulation controls as Sampler.
use super::*;
use daw_core::{GrainWindow, ModulationParameter as P, SamplerSettings};

pub(super) fn controls(
    ui: &mut egui::Ui,
    settings: &mut SamplerSettings,
    sources: &[modulation_panel::Source],
    live: Option<&daw_engine::ModulationSnapshot>,
    compact: bool,
) -> [Rect; 9] {
    let mut bounds = [Rect::NOTHING; 9];
    let mut values = settings.granulizer.unwrap().values();
    let columns = [
        ("Grain", [Some(0), Some(1), None]),
        ("Scan", [Some(2), Some(3), Some(4)]),
        ("Random", [Some(5), Some(6), Some(7)]),
    ];
    let mut window = settings.granulizer.unwrap().window;
    for (label, rows) in columns {
        let strip = egui::Frame::new()
            .inner_margin(panels::PanelStyle::default().margin)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                    ui.set_width(
                        midi_editor::UNIT_STRIP_WIDTH
                            - 2.0 * f32::from(panels::PanelStyle::default().margin),
                    );
                    ui.set_min_height(envelope_panel::body_height(compact));
                    if !compact {
                        for row in rows {
                            if let Some(index) = row {
                                let parameter = P::GRANULIZER[index + 1];
                                let marker = modulation_panel::marker(
                                    &settings.modulations,
                                    sources,
                                    parameter,
                                    values[index],
                                    settings.enabled,
                                    None,
                                    [live.map(|v| v[index + 3]), None],
                                );
                                bounds[index] = knobs::row(
                                    ui,
                                    &mut values[index],
                                    knobs::CHOICE_SIZE,
                                    |ui, value| {
                                        ui.add(
                                            knobs::Knob::new(
                                                value,
                                                parameter_knobs::destination(parameter),
                                            )
                                            .modulated(
                                                marker.as_ref(),
                                                &mut settings.modulation_open,
                                            ),
                                        )
                                    },
                                )
                                .rect;
                            } else {
                                let mut scalar = GrainWindow::ALL
                                    .iter()
                                    .position(|w| *w == window)
                                    .unwrap_or(0)
                                    as f32;
                                let response =
                                    knobs::row(ui, &mut scalar, knobs::CHOICE_SIZE, |ui, value| {
                                        ui.add(knobs::Knob::new(
                                            value,
                                            knobs::KnobSpec::new(
                                                "Grain window",
                                                0.0..=2.0,
                                                0.0,
                                                1.0,
                                            )
                                            .stepped(1.0)
                                            .cyclic()
                                            .diagrams(window_diagram)
                                            .format(
                                                |v| GrainWindow::ALL[v as usize].label().into(),
                                            ),
                                        ))
                                    });
                                bounds[8] = response.rect;
                                if response.changed() {
                                    window = GrainWindow::ALL[scalar as usize];
                                }
                            }
                        }
                    }
                    rows::centered(
                        ui,
                        if compact {
                            envelope_panel::body_height(true)
                        } else {
                            icons::COMPACT_BUTTON_SIZE
                        },
                        |ui| rows::unit_label(ui, label),
                    );
                });
            });
        panels::unit_divider(
            ui,
            strip.response.rect.right(),
            strip.response.rect.y_range().into(),
        );
    }
    let g = settings.granulizer.as_mut().unwrap();
    [
        g.size_ms,
        g.overlap,
        g.position,
        g.speed,
        g.pitch,
        g.position_jitter,
        g.pitch_jitter,
        g.pan,
    ] = values;
    g.window = window;
    bounds
}
fn window_diagram(value: f32) -> Vec<Vec2> {
    let window = GrainWindow::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let phase = i as f32 / 24.0;
            egui::vec2(phase - 0.5, 0.5 - window.amplitude(phase))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_diagrams_fit_the_shared_choice_icon_bounds() {
        for (index, _) in GrainWindow::ALL.iter().enumerate() {
            let points = window_diagram(index as f32);
            assert!(points.iter().all(|p| p.x.abs() <= 0.5 && p.y.abs() <= 0.5));
            assert_eq!(points.first(), Some(&egui::vec2(-0.5, 0.5)));
            assert_eq!(points.last(), Some(&egui::vec2(0.5, 0.5)));
            assert_eq!(points[12], egui::vec2(0.0, -0.5));
            for other in 0..index {
                assert_ne!(points, window_diagram(other as f32));
            }
        }
    }
}
