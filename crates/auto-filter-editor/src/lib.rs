//! Native editor bindings for Auto-filter.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::AutoFilterSettings,
    model: &Model,
) -> body::Bounds {
    body::render(
        ui,
        settings,
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
        daw_unit_ui::meters::GainReduction::new(
            model.output_meter.reduction.reading(),
            Some(&model.output_meter.reduction),
        )
        .for_unit(settings.enabled),
        daw_unit_ui::meters::BandGainChange::new(
            model.output_meter.bands.readings(),
            Some(&model.output_meter.bands),
        )
        .for_unit(settings.enabled),
    )
}
fn paint_extra(
    _: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    _: &daw_unit_core::AutoFilterSettings,
) {
    for (mask, response) in &bounds.readouts {
        if response.clicked() {
            model
                .meter_reset
                .fetch_or(*mask, std::sync::atomic::Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;

#[cfg(test)]
fn paint_reference(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::AutoFilterSettings,
    model: &Model,
) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let reduction = daw_unit_ui::meters::GainReduction::new(
        model.output_meter.reduction.reading(),
        Some(&model.output_meter.reduction),
    )
    .for_unit(settings.enabled);
    let gains = daw_unit_ui::meters::BandGainChange::new(
        model.output_meter.bands.readings(),
        Some(&model.output_meter.bands),
    )
    .for_unit(settings.enabled);
    let levels = model.output_meter.bands.readings();
    let _ = (reduction, gains, levels);
    use daw_unit_ui::layout::unit_control_height;
    let _ = {
        daw_unit_ui::auto_filter::show(
            ui,
            settings,
            (false, unit_control_height()),
            &std::array::from_fn(|_| None),
            true,
            level,
        )
    };
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::AutoFilterSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = body::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn(|_| None),
        0.0.into(),
    );
    b.controls
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .filter(|r| r.enabled())
                .map(|r| (daw_drive_plugin::Parameter::ALL[i + 1], r.rect))
        })
        .collect()
}

#[cfg(test)]
fn assert_builtin_defaults() {
    let native = daw_drive_plugin::PluginState::default().to_settings();
    let builtin: daw_unit_core::AutoFilterSettings = Default::default();
    assert_eq!(native, builtin);
}

#[cfg(test)]
mod hidden_gesture_tests {
    use super::*;
    use daw_drive_plugin::{Parameter as P, PluginState};
    use std::sync::Arc;
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>, focused: bool) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(WIDTH as f32, HEIGHT as f32),
                )),
                focused,
                events,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        out.textures_delta.clear();
    }
    #[test]
    fn host_mode_changes_end_hidden_gestures_on_release_focus_loss_and_full_queue() {
        for scenario in 0..3 {
            let ctx = egui::Context::default();
            daw_unit_ui::theme::configure(&ctx);
            daw_unit_ui::fonts::configure(&ctx);
            let model = Arc::new(Model::default());
            let mut state = PluginState::default();
            state.set(P::RateMode, 1.0).unwrap();
            model.publish(state);
            let mut editor = Editor::new(model.clone(), || {});
            frame(&ctx, &mut editor, vec![], true);
            // The reverse host transition must restore the Hz control too.
            state.set(P::RateMode, 0.0).unwrap();
            model.publish(state);
            frame(&ctx, &mut editor, vec![], true);
            let mut settings = state.to_settings();
            let mut rect = egui::Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(WIDTH as f32, HEIGHT as f32),
                    )),
                    ..Default::default()
                },
                |ui| {
                    rect = probe_controls(ui, &mut settings)
                        .into_iter()
                        .find(|(p, _)| *p == P::RateHz)
                        .unwrap()
                        .1
                },
            );
            out.textures_delta.clear();
            frame(&ctx, &mut editor, vec![], true);
            let start = rect.center();
            let end = start - egui::vec2(0.0, 25.0);
            let button = |pressed| egui::Event::PointerButton {
                pos: if pressed { start } else { end },
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                &ctx,
                &mut editor,
                vec![egui::Event::PointerMoved(start), button(true)],
                true,
            );
            frame(
                &ctx,
                &mut editor,
                vec![egui::Event::PointerMoved(end)],
                true,
            );
            let mut before = Vec::new();
            while let Some(edit) = model.pop() {
                before.push(edit);
            }
            assert!(before.contains(&Edit::Begin(P::RateHz)), "{before:?}");
            assert!(!before.contains(&Edit::End(P::RateHz)));
            if scenario == 2 {
                assert!(model.push(&vec![Edit::Value(P::RateHz, 0.5); model.remaining()]));
            }
            state.set(P::RateMode, 1.0).unwrap();
            model.publish(state);
            frame(
                &ctx,
                &mut editor,
                if scenario == 1 {
                    vec![]
                } else {
                    vec![button(false)]
                },
                scenario != 1,
            );
            let mut after = Vec::new();
            while let Some(edit) = model.pop() {
                after.push(edit);
            }
            if scenario == 2 {
                assert!(!after.contains(&Edit::End(P::RateHz)), "queue was full");
                after.clear();
            }
            frame(&ctx, &mut editor, vec![], true);
            frame(&ctx, &mut editor, vec![], true);
            while let Some(edit) = model.pop() {
                after.push(edit);
            }
            assert_eq!(
                after.iter().filter(|e| **e == Edit::End(P::RateHz)).count(),
                1,
                "scenario={scenario}: {after:?}"
            );
        }
    }
}
