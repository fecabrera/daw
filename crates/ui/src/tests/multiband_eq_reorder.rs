use super::*;

fn fixture(compact: bool, master: bool) -> (DawUi, egui::Context) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.macro_module.add();
    let source = daw_core::ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.macro_module.controls[0].id,
    };
    let mut effect = daw_core::EffectInstance::multiband_equalizer();
    let daw_core::EffectSettings::MultibandEq(s) = &mut effect.unit else {
        unreachable!()
    };
    s.bands.extend([Default::default(), Default::default()]);
    for (i, band) in s.bands.iter_mut().enumerate() {
        band.frequency_hz = [150.0, 1000.0, 6000.0][i];
        band.gain_db = [6.0, -3.0, 2.0][i];
    }
    let mut route = daw_core::Modulation::new(daw_core::ModulationParameter::EqBandFrequency);
    route.target = Some(s.bands[0].id);
    route.source = (!master).then_some(source);
    route.amount = 1.0;
    s.modulations.push(route);
    if master {
        app.session.project.master.effects.push(effect);
        app.session.project.master.effects_compact = compact;
    } else {
        app.session.project.tracks[0].effects.push(effect);
        app.session.project.tracks[0].effects_compact = compact;
    }
    app.select_track(track, false);
    app.master_selected = master;
    app.set_effects_lane_visible(true);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    app.history = None;
    app.ensure_history();
    (app, ctx)
}
fn settings(app: &DawUi) -> &daw_core::MultibandEqSettings {
    let effects = if app.master_selected {
        &app.session.project.master.effects
    } else {
        &app.session.project.tracks[0].effects
    };
    let daw_core::EffectSettings::MultibandEq(s) = &effects[0].unit else {
        unreachable!()
    };
    s
}
fn button(p: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
fn points(app: &DawUi) -> (Pos2, Pos2) {
    let b = &app.effects_bounds.multiband_eq_units[0];
    // The label beside Power is noninteractive strip space, as on Oscillator.
    (
        Pos2::new(b.band_power[0].right() + 8.0, b.band_power[0].center().y),
        b.bands[2].right_center() - Vec2::new(2.0, 0.0),
    )
}
fn arm(app: &mut DawUi, ctx: &egui::Context, p: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(p), button(p, true)],
    );
    assert!(crate::strip_reorder::dragging(ctx));
}
#[test]
fn eq_band_reorder_reuses_strip_insertion_preserves_routes_and_commits_one_undo_step() {
    for compact in [false, true] {
        for master in [false, true] {
            let (mut app, ctx) = fixture(compact, master);
            let original = settings(&app).clone();
            let (start, end) = points(&app);
            arm(&mut app, &ctx, start);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert_eq!(settings(&app), &original);
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(!crate::strip_reorder::dragging(&ctx));
            let changed = settings(&app).clone();
            assert_eq!(
                changed.bands,
                [
                    original.bands[1].clone(),
                    original.bands[2].clone(),
                    original.bands[0].clone()
                ]
            );
            assert_eq!(changed.modulations, original.modulations);
            assert!(app.dirty);
            assert!(app.error.is_none(), "{:?}", app.error);
            app.session.project.validate().unwrap();
            let encoded = serde_json::to_string(&app.session.project).unwrap();
            assert_eq!(
                serde_json::from_str::<daw_core::Project>(&encoded).unwrap(),
                app.session.project
            );
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(settings(&app), &original);
            app.perform_history_action(HistoryAction::Redo, &ctx);
            assert_eq!(settings(&app), &changed);
        }
    }
}
#[test]
fn eq_band_reorder_cancels_escape_outside_focus_and_noop_without_project_edits() {
    for compact in [false, true] {
        for cancel in ["escape", "outside", "focus", "same"] {
            let (mut app, ctx) = fixture(compact, false);
            let original = app.session.project.clone();
            let (start, end) = points(&app);
            arm(&mut app, &ctx, start);
            let p = match cancel {
                "outside" => app.effects_bounds.multiband_eq_units[0].output.center(),
                "same" => start,
                _ => end,
            };
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p)]);
            let events = match cancel {
                "escape" => vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                "focus" => vec![egui::Event::WindowFocused(false)],
                _ => vec![button(p, false)],
            };
            if cancel == "focus" {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            } else {
                frame(&mut app, &ctx, events);
            }
            assert!(!crate::strip_reorder::dragging(&ctx), "{cancel}");
            assert_eq!(app.session.project, original, "{cancel}");
            assert!(!app.dirty, "{cancel}");
        }
    }
}
#[test]
fn eq_knobs_and_power_do_not_arm_band_reordering_and_fast_release_is_supported() {
    for compact in [false, true] {
        let (mut app, ctx) = fixture(compact, false);
        let b = &app.effects_bounds.multiband_eq_units[0];
        let power = b.band_power[0].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(power), button(power, true)],
        );
        assert!(!crate::strip_reorder::dragging(&ctx));
        frame(&mut app, &ctx, vec![button(power, false)]);
        assert!(!settings(&app).bands[0].enabled);
        if !compact {
            let knob = app.effects_bounds.multiband_eq_units[0].knobs[0][1].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(knob), button(knob, true)],
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(knob + Vec2::new(10.0, -10.0))],
            );
            assert!(!crate::strip_reorder::dragging(&ctx));
            frame(
                &mut app,
                &ctx,
                vec![button(knob + Vec2::new(10.0, -10.0), false)],
            );
        }
        let before = settings(&app).clone();
        let (start, end) = points(&app);
        arm(&mut app, &ctx, start);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end), button(end, false)],
        );
        assert_eq!(settings(&app).bands[2], before.bands[0]);
        assert_eq!(settings(&app).modulations, before.modulations);
    }
}
#[test]
fn eq_band_reorder_does_not_overwrite_an_edit_or_survive_a_host_change() {
    for host_change in [false, true] {
        let (mut app, ctx) = fixture(false, false);
        let (start, end) = points(&app);
        arm(&mut app, &ctx, start);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        if host_change {
            // Simulate a host replacement; ordinary view switching is guarded during a drag.
            app.view_mode = ViewMode::Mixer;
        } else {
            let daw_core::EffectSettings::MultibandEq(s) =
                &mut app.session.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            s.bands[1].gain_db = 4.0;
        }
        let before = app.session.project.clone();
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(app.session.project, before, "host_change={host_change}");
        assert!(!crate::strip_reorder::dragging(&ctx));
    }
}

#[test]
fn eq_band_reorder_scrolls_the_lane_without_committing_its_preview() {
    let (mut app, ctx) = fixture(false, false);
    let daw_core::EffectSettings::MultibandEq(s) =
        &mut app.session.project.tracks[0].effects[0].unit
    else {
        unreachable!()
    };
    s.bands.extend((0..24).map(|_| Default::default()));
    app.history = None;
    frame(&mut app, &ctx, vec![]);
    let before = settings(&app).clone();
    let first = app.effects_bounds.multiband_eq_units[0].bands[0];
    let start = first.left_center() + Vec2::new(2.0, 0.0);
    let edge = app.effects_bounds.lane_viewport.right_center() - Vec2::new(2.0, 0.0);
    let offset = app.effects_bounds.lane_scroll;
    arm(&mut app, &ctx, start);
    for _ in 0..16 {
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge)]);
    }
    assert!(app.effects_bounds.lane_scroll > offset);
    assert_eq!(settings(&app), &before);
    assert!(!app.dirty);
    frame(&mut app, &ctx, vec![button(edge, false)]);
    assert_ne!(settings(&app).bands[0].id, before.bands[0].id);
    assert_eq!(settings(&app).modulations, before.modulations);
    assert!(app.dirty);
}

#[test]
fn eq_band_reorder_works_in_regular_layer_and_output_lanes() {
    for main in [false, true] {
        for compact in [false, true] {
            let (mut app, ctx) = fixture(compact, false);
            let effect = app.session.project.tracks[0].effects.pop().unwrap();
            let daw_core::EffectSettings::MultibandEq(original) = &effect.unit else {
                unreachable!()
            };
            let original = original.clone();
            let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
            instrument.layers[0].collapsed = compact;
            instrument.main.collapsed = compact;
            if main {
                instrument.main.effects.push(effect);
            } else {
                instrument.layers[0].effects.push(effect);
            }
            app.set_view_mode(ViewMode::Instrument, &ctx);
            app.history = None;
            frame_sized(&mut app, &ctx, vec![], Vec2::new(1600.0, 1200.0));
            let (start, end) = points(&app);
            frame_sized(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
                Vec2::new(1600.0, 1200.0),
            );
            assert!(
                crate::strip_reorder::dragging(&ctx),
                "main={main}, compact={compact}, start={start:?}"
            );
            frame_sized(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end)],
                Vec2::new(1600.0, 1200.0),
            );
            assert!(!app.dirty);
            frame_sized(
                &mut app,
                &ctx,
                vec![button(end, false)],
                Vec2::new(1600.0, 1200.0),
            );
            let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
            let effects = if main {
                &instrument.main.effects
            } else {
                &instrument.layers[0].effects
            };
            let daw_core::EffectSettings::MultibandEq(changed) = &effects[0].unit else {
                unreachable!()
            };
            assert_eq!(
                changed.bands,
                [
                    original.bands[1].clone(),
                    original.bands[2].clone(),
                    original.bands[0].clone()
                ]
            );
            assert_eq!(changed.modulations, original.modulations);
            assert!(app.dirty);
            assert!(app.component_drag.is_none());
            assert!(app.effects_drag.is_none());
            app.session.project.validate().unwrap();
        }
    }
}
