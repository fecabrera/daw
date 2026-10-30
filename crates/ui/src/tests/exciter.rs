use super::*;
fn fixture() -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_track().unwrap();
    let effect = daw_core::EffectInstance::exciter();
    let id = effect.id;
    app.session.project.tracks[0].effects.push(effect);
    app.select_track(track, false);
    app.set_effects_lane_visible(true);
    let ctx = egui::Context::default();
    theme::configure(&ctx);
    fonts::configure(&ctx);
    icons::configure(&ctx);
    (app, ctx, id)
}
fn frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    out.shapes
}
fn click(app: &mut DawUi, ctx: &egui::Context, p: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, ctx, vec![]);
}
#[test]
fn exciter_uses_dropdowns_knobs_shared_footers_and_output_meter_in_all_themes_and_scales() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            for compact in [false, true] {
                let (mut app, ctx, _) = fixture();
                ctx.set_pixels_per_point(scale);
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color,
                        ..Default::default()
                    },
                );
                app.session.project.tracks[0].effects_compact = compact;
                frame(&mut app, &ctx, vec![]);
                let b = &app.effects_bounds.exciter_units[0];
                assert_eq!(b.body.bands.len(), 4);
                assert_eq!(b.body.output.width(), panels::UNIT_STRIP_WIDTH);
                assert!(b.header.height() > 0.0);
                for i in 0..4 {
                    let band = b.body.bands[i];
                    assert_eq!(band.width(), panels::UNIT_STRIP_WIDTH);
                    assert!(band.contains_rect(b.body.power[i]));
                    assert!(band.contains_rect(b.body.solo[i]));
                    assert!(b.body.power[i].right() < b.body.solo[i].left());
                    if !compact {
                        assert!(band.contains_rect(b.body.modes[i]));
                        for knob in b.body.knobs[i] {
                            assert!(band.contains_rect(knob));
                            assert!((knob.width() - 42.0).abs() < 0.01);
                        }
                        assert!(b.body.modes[i].bottom() < b.body.knobs[i][0].top());
                        assert!(
                            b.body.knobs[i]
                                .windows(2)
                                .all(|p| p[0].bottom() < p[1].top())
                        );
                        assert!(b.body.knobs[i][2].bottom() < b.body.power[i].top());
                        assert!((band.bottom() - b.body.output.bottom()).abs() < 0.01);
                    }
                }
                if !compact {
                    assert_eq!(b.body.crossovers.len(), 3);
                    assert_eq!(
                        b.body.meter,
                        meters::unit_monitor_rail(
                            b.body.output,
                            panels::PanelStyle::default().margin
                        )
                    );
                }
                assert!(!app.dirty);
            }
        }
    }
}
#[test]
fn solo_is_ui_only_while_band_power_addition_and_removal_use_project_history() {
    let (mut app, ctx, effect) = fixture();
    frame(&mut app, &ctx, vec![]);
    let original = app.session.project.clone();
    let solo = app.effects_bounds.exciter_units[0].body.solo[1].center();
    click(&mut app, &ctx, solo);
    let daw_core::EffectSettings::Exciter(s) = &original.tracks[0].effects[0].unit else {
        unreachable!()
    };
    assert!(
        app.exciter_audition
            .solos
            .contains(&(effect, s.bands[1].id))
    );
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
    let power = app.effects_bounds.exciter_units[0].body.power[1].center();
    click(&mut app, &ctx, power);
    let daw_core::EffectSettings::Exciter(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(!s.bands[1].enabled);
    assert!(app.dirty);
    let add = app.effects_bounds.exciter_units[0].body.add.center();
    click(&mut app, &ctx, add);
    let daw_core::EffectSettings::Exciter(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert_eq!(s.bands.len(), 5);
    assert!(s.valid());
    // A different project cannot retain audition even when external IDs are reused.
    app.session.project.project_id = Id::new_v4();
    frame(&mut app, &ctx, vec![]);
    assert!(app.exciter_audition.solos.is_empty());
}
#[test]
fn mode_dropdown_and_context_removal_preserve_surviving_band_routes() {
    let (mut app, ctx, _) = fixture();
    let daw_core::EffectSettings::Exciter(s) = &mut app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    let removed = s.bands[0].id;
    let retained = s.bands[1].id;
    for id in [removed, retained] {
        let mut r = daw_core::Modulation::new(daw_core::ModulationParameter::SaturatorDrive);
        r.target = Some(id);
        s.modulations.push(r);
    }
    frame(&mut app, &ctx, vec![]);
    let mode = app.effects_bounds.exciter_units[0].body.modes[0].center();
    click(&mut app, &ctx, mode);
    let shapes = frame(&mut app, &ctx, vec![]);
    let choice = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Tube" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Mode must open a dropdown");
    click(&mut app, &ctx, choice);
    let daw_core::EffectSettings::Exciter(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert_eq!(s.bands[0].mode, daw_core::ExciterMode::Tube);
    let power = app.effects_bounds.exciter_units[0].body.power[0].center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(power),
                egui::Event::PointerButton {
                    pos: power,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    let shapes = frame(&mut app, &ctx, vec![]);
    let action = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Remove band" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Band controls must expose Remove band");
    click(&mut app, &ctx, action);
    let daw_core::EffectSettings::Exciter(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert_eq!(s.bands.len(), 3);
    assert!(s.valid());
    assert_eq!(s.modulations.len(), 1);
    assert_eq!(s.modulations[0].target, Some(retained));
    assert!(s.bands.iter().all(|b| b.id != removed));
}
