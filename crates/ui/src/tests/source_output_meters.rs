use super::*;

#[test]
fn sources_keep_gap_meters_without_horizontal_output_rails_or_meter_drags() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            for (compact, collapsed) in [(false, false), (true, false), (false, true)] {
                for enabled in [false, true] {
                    for kind in 0..3 {
                        let mut app = DawUi::default();
                        let track = app.session.project.add_midi_track().unwrap();
                        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
                        if kind == 0 {
                            *instrument = daw_core::OscillatorSettings::default().into();
                        }
                        let layer = &mut instrument.layers[0];
                        layer.collapsed = compact;
                        if kind == 0 {
                            layer.oscillator_enabled = enabled;
                            layer.oscillator_collapsed = collapsed;
                        } else {
                            layer.sampler = Some(if kind == 1 {
                                daw_core::SamplerSettings::granular()
                            } else {
                                Default::default()
                            });
                            let source = layer.sampler.as_mut().unwrap();
                            source.enabled = enabled;
                            source.collapsed = collapsed;
                        }
                        app.select_track(track, false);
                        let ctx = context();
                        ctx.set_pixels_per_point(scale);
                        app.set_settings(
                            &ctx,
                            settings::AppSettings {
                                theme: color,
                                ..Default::default()
                            },
                        );
                        app.set_view_mode(ViewMode::Instrument, &ctx);
                        app.set_effects_lane_visible(false);
                        let size = Vec2::new(1600.0, 1000.0);
                        for _ in 0..2 {
                            frame_shapes(&mut app, &ctx, vec![], size);
                        }
                        app.dirty = false;
                        let before = app.session.project.clone();
                        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                        assert_eq!(app.session.project, before);
                        let strip = if kind == 0 {
                            app.midi_editor.oscillator_output_bounds
                        } else {
                            app.instrument_ui.samplers[0].controls
                        };
                        if collapsed {
                            assert!(!strip.is_positive());
                            continue;
                        }
                        assert_eq!(strip.width(), panels::UNIT_STRIP_WIDTH);
                        let rail =
                            meters::unit_monitor_rail(strip, panels::PanelStyle::default().margin);
                        let present = shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rail && r.fill == theme::palette(&ctx).input));
                        assert!(
                            !present,
                            "kind={kind}, compact={compact}, enabled={enabled}"
                        );
                        if !compact {
                            let knob = if kind == 0 {
                                app.midi_editor.oscillator_level_bounds
                            } else {
                                app.instrument_ui.samplers[0].gain
                            };
                            assert!(rail.bottom() < knob.top());
                            let unit = app
                                .instrument_ui
                                .component_slots
                                .iter()
                                .find(|(_, component, _)| component.is_source())
                                .unwrap()
                                .2;
                            let point = daw_unit_ui::meters::unit_gap_rect(unit).center();
                            frame_shapes(
                                &mut app,
                                &ctx,
                                vec![egui::Event::PointerMoved(point), button(point, true)],
                                size,
                            );
                            frame_shapes(
                                &mut app,
                                &ctx,
                                vec![egui::Event::PointerMoved(point + Vec2::new(14.0, 0.0))],
                                size,
                            );
                            assert!(app.component_drag.is_none());
                            assert!(app.effects_drag.is_none());
                            assert!(!app.instrument_ui.has_layer_gesture());
                            frame_shapes(&mut app, &ctx, vec![button(point, false)], size);
                            assert_eq!(app.session.project, before);
                            assert!(!app.dirty);
                        }
                    }
                }
            }
        }
    }
}
