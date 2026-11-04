use super::*;
fn fixture() -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_track().unwrap();
    let effect = daw_core::EffectInstance::multiband_compressor();
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
fn band_controls_fit_shared_geometry_solo_is_not_an_edit_and_band_power_retains_audio_range() {
    for compact in [false, true] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut app, ctx, effect) = fixture();
            ctx.set_pixels_per_point(scale);
            app.session.project.tracks[0].effects_compact = compact;
            frame(&mut app, &ctx, vec![]);
            let b = &app.effects_bounds.multiband_compressor_units[0];
            assert!(b.header.is_positive());
            assert_eq!(b.body.bands.len(), 3);
            assert_eq!(b.body.crossovers.len(), if compact { 0 } else { 2 });
            for (i, band) in b.body.bands.iter().enumerate() {
                assert_eq!(band.width(), daw_unit_ui::layout::unit_columns_width(2));
                assert!(band.contains_rect(b.body.power[i]));
                assert!(band.contains_rect(b.body.solo[i]));
                assert!(
                    band.bottom() <= b.body.output.bottom() + 0.5,
                    "band: {band:?}, output: {:?}",
                    b.body.output
                );
                if !compact {
                    for knob in b.body.knobs[i] {
                        assert!(band.contains_rect(knob));
                    }
                    assert!(band.contains_rect(b.body.modes[i]));
                }
            }
            let solo = b.body.solo[1].center();
            let original = app.session.project.clone();
            app.dirty = false;
            click(&mut app, &ctx, solo);
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
            let band = app.session.project.tracks[0].effects[0]
                .unit
                .band_ids()
                .nth(1)
                .unwrap();
            assert!(app.exciter_audition.solos.contains(&(effect, band)));
            let power = app.effects_bounds.multiband_compressor_units[0].body.power[1].center();
            click(&mut app, &ctx, power);
            let daw_core::EffectSettings::MultibandCompressor(s) =
                &app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            assert!(!s.bands[1].enabled);
            assert!(app.exciter_audition.solos.contains(&(effect, band)));
            let unit = app.effects_bounds.multiband_compressor_units[0]
                .power
                .center();
            click(&mut app, &ctx, unit);
            assert!(app.exciter_audition.solos.is_empty());
        }
    }
}
#[test]
fn eq_solo_uses_the_same_button_and_never_changes_project_settings() {
    let (mut app, ctx, _) = fixture();
    let eq = daw_core::EffectInstance::multiband_equalizer();
    let effect = eq.id;
    let band = eq.unit.band_ids().next().unwrap();
    app.session.project.tracks[0].effects = vec![eq];
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let original = app.session.project.clone();
    let b = &app.effects_bounds.multiband_eq_units[0];
    assert!(b.bands[0].contains_rect(b.band_solo[0]));
    let point = b.band_solo[0].center();
    click(&mut app, &ctx, point);
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
    assert!(app.exciter_audition.solos.contains(&(effect, band)));
    let power = app.effects_bounds.multiband_eq_units[0].power.center();
    click(&mut app, &ctx, power);
    assert!(app.exciter_audition.solos.is_empty());
}

fn open_band_menu(
    app: &mut DawUi,
    ctx: &egui::Context,
    point: Pos2,
) -> Vec<egui::epaint::ClippedShape> {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, ctx, vec![])
}
fn menu_action(app: &mut DawUi, ctx: &egui::Context, point: Pos2, label: &str) {
    let shapes = open_band_menu(app, ctx, point);
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing band action: {label}"));
    click(app, ctx, point);
}
#[test]
fn band_context_menu_edits_power_and_removal_but_keeps_solo_temporary() {
    let (mut app, ctx, effect) = fixture();
    frame(&mut app, &ctx, vec![]);
    let band = app.session.project.tracks[0].effects[0]
        .unit
        .band_ids()
        .next()
        .unwrap();
    let point = app.effects_bounds.multiband_compressor_units[0].body.bands[0].left_top()
        + egui::vec2(3.0, 40.0);
    menu_action(&mut app, &ctx, point, "Disable band");
    let daw_core::EffectSettings::MultibandCompressor(s) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(!s.bands[0].enabled);
    assert!(app.dirty);
    let original = app.session.project.clone();
    app.dirty = false;
    let knob = app.effects_bounds.multiband_compressor_units[0].body.knobs[0][0].center();
    menu_action(&mut app, &ctx, knob, "Solo");
    assert!(app.exciter_audition.solos.contains(&(effect, band)));
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
    let solo = app.effects_bounds.multiband_compressor_units[0].body.solo[0].center();
    menu_action(&mut app, &ctx, solo, "Enable band");
    let daw_core::EffectSettings::MultibandCompressor(s) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(s.bands[0].enabled);
    menu_action(&mut app, &ctx, point, "Remove band");
    let daw_core::EffectSettings::MultibandCompressor(s) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert_eq!(s.bands.len(), 2);
    assert!(s.valid());
    assert!(!s.bands.iter().any(|b| b.id == band));
    assert!(!app.exciter_audition.solos.contains(&(effect, band)));
    menu_action(&mut app, &ctx, point, "Remove band");
    let original = app.session.project.clone();
    app.dirty = false;
    menu_action(&mut app, &ctx, point, "Remove band");
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
}

#[test]
fn eq_band_context_menu_keeps_solo_temporary_and_can_remove_the_last_band() {
    let (mut app, ctx, _) = fixture();
    let eq = daw_core::EffectInstance::multiband_equalizer();
    let effect = eq.id;
    let band = eq.unit.band_ids().next().unwrap();
    app.session.project.tracks[0].effects = vec![eq];
    frame(&mut app, &ctx, vec![]);
    let point =
        app.effects_bounds.multiband_eq_units[0].bands[0].left_top() + egui::vec2(3.0, 40.0);
    menu_action(&mut app, &ctx, point, "Disable band");
    let daw_core::EffectSettings::MultibandEq(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(!s.bands[0].enabled);
    let original = app.session.project.clone();
    app.dirty = false;
    let knob = app.effects_bounds.multiband_eq_units[0].knobs[0][0].center();
    menu_action(&mut app, &ctx, knob, "Solo");
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
    assert!(app.exciter_audition.solos.contains(&(effect, band)));
    let solo = app.effects_bounds.multiband_eq_units[0].band_solo[0].center();
    menu_action(&mut app, &ctx, solo, "Enable band");
    let daw_core::EffectSettings::MultibandEq(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(s.bands[0].enabled);
    menu_action(&mut app, &ctx, point, "Remove band");
    let daw_core::EffectSettings::MultibandEq(s) = &app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    assert!(s.bands.is_empty());
    assert!(s.valid());
    assert!(!app.exciter_audition.solos.contains(&(effect, band)));
    assert!(app.dirty);
}
