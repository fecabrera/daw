use super::*;

#[test]
fn layers_and_output_reuse_track_meter_geometry_in_every_presentation() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            for compact in [false, true] {
                let mut app = DawUi::default();
                let track = app.session.project.add_midi_track().unwrap();
                let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
                instrument.layers[0].collapsed = compact;
                instrument.main.collapsed = compact;
                let layer = instrument.layers[0].id;
                app.select_track(track, false);
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme,
                        ..Default::default()
                    },
                );
                ctx.set_pixels_per_point(scale);
                app.set_view_mode(ViewMode::Instrument, &ctx);
                let size = Vec2::new(1280.0, 1000.0);
                for _ in 0..2 {
                    frame_shapes(&mut app, &ctx, vec![], size);
                }
                let before = app.session.project.clone();
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let label = app.midi_editor.layer_bounds[&layer].label;
                let output = app.instrument_ui.main_bounds.label;
                for label in [label, output] {
                    let header = Rect::from_min_size(label.min, Vec2::new(TRACK_WIDTH, ROW_HEIGHT));
                    super::track_header::assert_header_meter_insets(&shapes, header, &ctx);
                    let point = Pos2::new(header.right() - 23.0, header.top() + 20.0);
                    frame_shapes(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(point), button(point, true)],
                        size,
                    );
                    frame_shapes(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(point - Vec2::new(0.0, 14.0))],
                        size,
                    );
                    assert!(
                        !app.instrument_ui.has_layer_gesture(),
                        "meters must not arm layer reordering"
                    );
                    frame_shapes(&mut app, &ctx, vec![button(point, false)], size);
                    assert_eq!(app.session.project, before);
                    assert!(!app.dirty);
                }
                let controls = app.midi_editor.layer_bounds[&layer].mix_controls;
                assert!(controls[3].right() < controls[2].left());
                assert!((controls[2].center().x - (label.right() - 47.0)).abs() < 0.75);
                assert!(
                    (app.instrument_ui.main_bounds.mix[0].center().x - (output.right() - 47.0))
                        .abs()
                        < 0.75
                );
            }
        }
    }
}
