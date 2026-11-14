// Shared hover coverage for every effect editor, including product-specific wrappers.
#[test]
fn hovered_editor_controls_match_builtin_values_without_host_gestures() {
    fn frame(ctx: &egui::Context, point: egui::Pos2, mut draw: impl FnMut(&mut egui::Ui)) -> Vec<String> {
        let mut out = ctx.run_ui(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32))),
            time: Some(ctx.cumulative_frame_nr() as f64 * 0.1),
            focused: true,
            events: vec![Event::PointerMoved(point)],
            ..Default::default()
        }, |ui| draw(ui));
        let texts = out.shapes.iter().filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()), _ => None,
        }).collect();
        out.textures_delta.clear();
        texts
    }
    for appearance in 0..3 {
        for scale in [1.0, 1.5] {
            let state = PluginState::default();
            let probe = context(appearance, scale);
            let mut settings = state.to_settings();
            let mut controls = vec![];
            paint(&probe, |ui| controls = super::probe_controls(ui, &mut settings));
            let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32));
            let mut observed_hover = false;
            for (_, rect) in controls.into_iter().filter(|(_, r)| r.is_positive() && screen.contains_rect(*r)) {
                let model = Arc::new(Model::default());
                model.publish(state);
                model.appearance.store(appearance, Ordering::Relaxed);
                model.scale.store(scale.to_bits(), Ordering::Relaxed);
                let a = context(appearance, scale);
                let b = context(appearance, scale);
                for ctx in [&a, &b] {
                    ctx.all_styles_mut(|s| { s.interaction.tooltip_delay = 0.0; s.interaction.tooltip_grace_time = 0.0; });
                }
                let mut editor = Editor::new(model.clone(), || {});
                let mut reference = state.to_settings();
                let point = rect.center();
                let idle = frame(&a, egui::Pos2::new(-10.0, -10.0), |ui| editor.show(ui)).len();
                frame(&b, egui::Pos2::new(-10.0, -10.0), |ui| {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, daw_unit_ui::theme::unit_separator(ui.ctx()));
                    super::paint_reference(ui, &mut reference, &model);
                });
                for _ in 0..4 {
                    let actual = frame(&a, point, |ui| editor.show(ui));
                    let expected = frame(&b, point, |ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, daw_unit_ui::theme::unit_separator(ui.ctx()));
                        super::paint_reference(ui, &mut reference, &model);
                    });
                    assert_eq!(actual, expected, "hover parity: {} at {point:?}", daw_drive_plugin::IDENTITY.product);
                    observed_hover |= actual.len() > idle;
                }
                assert!(model.pop().is_none(), "hover must not enqueue host gestures");
                assert_eq!(PluginState::from_settings(&reference).unwrap(), PluginState::from_settings(&state.to_settings()).unwrap());
            }
            assert!(observed_hover, "hover coverage must paint at least one tooltip");
        }
    }
}
