//! Native editor bindings for Compressor.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::CompressorSettings,
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
        daw_unit_ui::meters::unit_reading(settings.enabled, model.output_meter.bands.readings()[0]),
        model.output_meter.bands.history().and_then(|h| h.reading()),
    )
}
fn paint_extra(
    _: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    _: &daw_unit_core::CompressorSettings,
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
    settings: &mut daw_unit_core::CompressorSettings,
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
        daw_unit_ui::compressor::show(
            ui,
            settings,
            false,
            unit_control_height(),
            daw_unit_ui::compressor::Controls {
                markers: &std::array::from_fn::<_, 8, _>(|_| None),
                reduction,
                level,
                graph: daw_unit_ui::compressor_curve::Input {
                    live_edits: true,
                    detector_level: daw_unit_ui::meters::unit_reading(settings.enabled, levels[0]),
                    history: model.output_meter.bands.history().and_then(|h| h.reading()),
                },
            },
        )
    };
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::CompressorSettings,
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
    let builtin: daw_unit_core::CompressorSettings = Default::default();
    assert_eq!(native, builtin);
}

#[cfg(test)]
mod curve_gesture_tests {
    use super::*;
    use daw_drive_plugin::Parameter as P;
    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn draw(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>) -> Vec<egui::Pos2> {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, body::SIZE)),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        let color = daw_unit_ui::theme::palette(ctx).secondary;
        let markers = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c)
                    if c.radius == 5.5 && c.stroke.width == 1.25 && c.stroke.color == color =>
                {
                    Some(c.center)
                }
                _ => None,
            })
            .collect();
        let rail = out.shapes.iter().find_map(|p| match &p.shape {
            egui::Shape::Rect(r) if r.rect.width() == 8.0 && r.rect.height() > 100.0 => {
                Some(r.rect)
            }
            _ => None,
        });
        if let Some(rail) = rail {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("compressor-gr-test"), rail));
        }
        if let Some(point) = out.shapes.iter().find_map(|p| match &p.shape {
            egui::Shape::Text(t) if t.galley.text() == "RMS" => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        }) {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("compressor-rms-test"), point));
        }
        out.textures_delta.clear();
        markers
    }
    fn drain(model: &Model) -> Vec<Edit> {
        let mut edits = Vec::new();
        while let Some(e) = model.pop() {
            edits.push(e);
        }
        edits
    }
    #[test]
    fn curve_graph_uses_one_joint_gesture_and_balances_cancel_and_queue_retries() {
        for cancel in [false, true] {
            for blocked in [false, true] {
                if blocked && !cancel {
                    continue;
                }
                let ctx = egui::Context::default();
                daw_unit_ui::fonts::configure(&ctx);
                daw_unit_ui::theme::configure(&ctx);
                let model = std::sync::Arc::new(Model::default());
                let mut editor = Editor::new(model.clone(), || {});
                let markers = draw(&ctx, &mut editor, vec![]);
                assert_eq!(markers.len(), 1);
                assert!(drain(&model).is_empty());
                let start = markers[0];
                let end = start + egui::vec2(15.0, 20.0);
                draw(
                    &ctx,
                    &mut editor,
                    vec![egui::Event::PointerMoved(start), pointer(start, true)],
                );
                draw(&ctx, &mut editor, vec![egui::Event::PointerMoved(end)]);
                let mut edits = drain(&model);
                assert_eq!(edits.first(), Some(&Edit::Begin(P::Threshold)));
                assert!(
                    edits
                        .iter()
                        .any(|e| matches!(e,Edit::Value(P::Threshold,v) if *v > -18.0))
                );
                if blocked {
                    while model.push(&[Edit::Presentation(
                        daw_drive_plugin::PluginState::default().presentation(),
                    )]) {}
                }
                if cancel {
                    draw(
                        &ctx,
                        &mut editor,
                        vec![egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Default::default(),
                        }],
                    );
                    if blocked {
                        drain(&model);
                        draw(&ctx, &mut editor, vec![]);
                    }
                }
                draw(&ctx, &mut editor, vec![pointer(end, false)]);
                edits.extend(drain(&model));
                assert_eq!(
                    edits.iter().filter(|e| matches!(e, Edit::Begin(_))).count(),
                    2
                );
                assert_eq!(
                    edits.iter().filter(|e| matches!(e, Edit::End(_))).count(),
                    2
                );
                assert!(edits.iter().all(|e| matches!(
                    e,
                    Edit::Begin(P::Threshold)
                        | Edit::Value(P::Threshold, _)
                        | Edit::End(P::Threshold)
                        | Edit::Begin(P::Ratio)
                        | Edit::Value(P::Ratio, _)
                        | Edit::End(P::Ratio)
                )));
                if cancel {
                    assert!(edits.contains(&Edit::Value(P::Threshold, -18.0)));
                    assert!(edits.contains(&Edit::Value(P::Ratio, 4.0)));
                }
            }
        }
    }
    #[test]
    fn curve_gr_click_resets_only_reduction_without_parameter_events() {
        use daw_unit_core::MeterStorage;
        let ctx = egui::Context::default();
        daw_unit_ui::fonts::configure(&ctx);
        daw_unit_ui::theme::configure(&ctx);
        let model = std::sync::Arc::new(Model::default());
        model.output_meter.output.store([0.75, 0.0, 0.0]);
        model.output_meter.output.initialize([0.25, 0.0, 0.0]);
        model.output_meter.reduction.store(18.0);
        model.output_meter.reduction.initialize(12.0);
        let mut editor = Editor::new(model.clone(), || {});
        draw(&ctx, &mut editor, vec![]);
        let rect = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("compressor-gr-test")))
            .unwrap();
        let before = model.current();
        assert!(drain(&model).is_empty());
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut editor,
                vec![
                    egui::Event::PointerMoved(rect.center()),
                    pointer(rect.center(), pressed),
                ],
            );
        }
        assert_eq!(model.output_meter.reduction.peak(), 0.0);
        assert_eq!(model.output_meter.reduction.reading(), 12.0);
        assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.75);
        assert_eq!(
            model.meter_reset.load(std::sync::atomic::Ordering::Acquire),
            2
        );
        assert_eq!(model.current(), before);
        assert!(drain(&model).is_empty());
        model
            .meter_reset
            .store(0, std::sync::atomic::Ordering::Release);
        model.output_meter.reduction.store(18.0);
        model.output_meter.reduction.initialize(12.0);
        let rms = ctx
            .data(|d| d.get_temp::<egui::Pos2>(egui::Id::new("compressor-rms-test")))
            .unwrap();
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut editor,
                vec![egui::Event::PointerMoved(rms), pointer(rms, pressed)],
            );
        }
        assert_eq!(
            model.meter_reset.load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert_eq!(model.output_meter.reduction.peak(), 18.0);
        // Clicking a knob focuses it; keyboard input changes the bounded choice.
        draw(
            &ctx,
            &mut editor,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let edits = drain(&model);
        assert!(
            edits
                .iter()
                .any(|e| matches!(e, Edit::Value(P::Detection, 1.0)))
        );
        assert!(edits.iter().all(|e| matches!(
            e,
            Edit::Begin(P::Detection) | Edit::Value(P::Detection, _) | Edit::End(P::Detection)
        )));
    }
}
