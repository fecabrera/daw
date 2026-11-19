use super::instrument_catalog::{add_component, click, key};
use super::*;
use daw_core::{InstrumentComponent, ModulationSourceId};

fn setup(theme: settings::ColorTheme) -> (DawUi, egui::Context, [Id; 2]) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.session.project.tracks[0].collect_modulation_lanes();
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    let owners = [i.lfo_lane.id, i.envelope_lane.id];
    app.select_track(track, false);
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_instrument_subview(
        crate::instrument_workspace::InstrumentSubview::Modulation,
        &ctx,
    );
    app.effects_lane_open = false;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, owners)
}

#[test]
fn envelope_plot_previews_sound_and_knobs_then_commits_once_or_restores() {
    for cancel in [false, true] {
        let (mut app, ctx, [_, lane]) = setup(settings::ColorTheme::Dark);
        let track = app.session.project.tracks[0].id;
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let envelope = daw_core::EnvelopeInstance::default();
        let source = ModulationSourceId {
            layer: lane,
            unit: envelope.id,
        };
        instrument.envelope_lane.collapsed = false;
        instrument.envelope_lane.envelopes.push(envelope);
        let layer = &mut instrument.layers[0];
        layer.oscillators.push(Default::default());
        layer.amplitude.present = true;
        layer.amplitude.modulations.push(daw_core::Modulation {
            source: Some(source),
            amount: 1.0,
            ..daw_core::Modulation::new(daw_core::ModulationParameter::Amplitude)
        });
        let clip_id = app.session.project.add_midi_pattern(track, 0).unwrap();
        app.session
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id,
                pattern: daw_core::MidiPattern {
                    length_ticks: 960,
                    notes: vec![daw_core::MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 69,
                        velocity: 127,
                    }],
                },
            })
            .unwrap();
        app.history = None;
        let original = app.session.project.clone();
        let size = Vec2::new(1600.0, 1200.0);
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        let header = app.instrument_ui.modulation_lane_bounds[&lane].envelopes[0];
        let plot_strip = Rect::from_min_size(
            Pos2::new(header.right(), header.top()),
            Vec2::new(daw_unit_ui::spectrum_graph::WIDTH, header.height()),
        );
        let sustain = shapes
            .iter()
            .filter_map(|item| match &item.shape {
                egui::Shape::Circle(circle)
                    if circle.radius == 5.5
                        && circle.stroke == egui::Stroke::NONE
                        && plot_strip.contains(circle.center) =>
                {
                    Some(circle.center)
                }
                _ => None,
            })
            .nth(1)
            .unwrap();
        let end = sustain + Vec2::new(0.0, 30.0);
        frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(sustain), button(sustain, true)],
            size,
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], size);
        assert_eq!(app.session.project, original);
        assert!(!app.dirty);
        let plan = app.graph_preview.published_plan(&app.session);
        let preview = plan
            .project
            .instrument_settings(track)
            .unwrap()
            .envelope_destinations()
            .find(|(id, _)| *id == source)
            .unwrap()
            .1
            .settings;
        assert!(
            preview.sustain < 0.8,
            "Expected sustain preview after dragging from {sustain:?}. UI error: {:?}",
            app.error
        );
        let knob =
            super::envelope_units::label(&shapes, &format!("{:.0}%", preview.sustain * 100.0));
        assert!(
            app.instrument_ui.modulation_lane_bounds[&lane].envelope_stages[0][2].contains(knob)
        );

        let sustain_peak = |plan| {
            let mut renderer = daw_engine::Renderer::new(plan);
            let mut samples = vec![[0.0; 2]; 8000];
            renderer.render_offline(&mut samples);
            samples[6000..]
                .iter()
                .map(|s| s[0].abs())
                .fold(0.0_f32, f32::max)
        };
        let base_peak = sustain_peak(app.session.plan());
        assert!(base_peak > 0.0);
        assert!(sustain_peak(plan) < base_peak);

        if cancel {
            frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                size,
            );
        }
        frame_shapes(&mut app, &ctx, vec![button(end, false)], size);
        let plan = app.graph_preview.published_plan(&app.session);
        assert_eq!(plan.project, app.session.project);
        if cancel {
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
        } else {
            assert_ne!(app.session.project, original);
            assert!(app.dirty);
            app.perform_history_action(crate::history::HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, original);
        }
    }
}

#[test]
fn instrument_wide_lanes_accept_only_their_registered_provider_type_from_the_catalog() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for lane in 0..2 {
            for entry in [1, 2] {
                let (mut app, ctx, owners) = setup(theme);
                let before = app.session.project.clone();
                let mut mixed = before.tracks[0].instrument.clone().unwrap();
                crate::instrument_catalog::add_components(
                    &mut mixed,
                    owners[lane],
                    &["daw.lfo", "daw.envelope"],
                );
                assert_eq!(Some(&mixed), before.tracks[0].instrument.as_ref());
                crate::instrument_catalog::add_components(
                    &mut mixed,
                    owners[lane],
                    &[
                        if lane == 0 { "daw.lfo" } else { "daw.envelope" },
                        "unregistered",
                    ],
                );
                assert_eq!(Some(&mixed), before.tracks[0].instrument.as_ref());
                add_component(&mut app, &ctx, owners[lane], entry);
                let accepted = entry != 4;
                assert_eq!(app.dirty, accepted, "lane {lane}, entry {entry}");
                let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                assert_eq!(
                    i.lfo_lane.lfos.iter().filter(|l| l.present).count(),
                    usize::from(entry == 2)
                );
                assert_eq!(
                    i.envelope_lane
                        .envelopes
                        .iter()
                        .filter(|e| e.present)
                        .count(),
                    usize::from(entry == 1)
                );
                assert!(
                    i.layers
                        .iter()
                        .all(|l| l.lfos.is_empty() && l.envelopes.is_empty())
                );
                if !accepted {
                    assert_eq!(app.session.project, before);
                }
                app.session.project.validate().unwrap();
            }
        }
    }
}
#[test]
fn provider_lanes_share_unit_bodies_toolbar_power_and_saved_presentation() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, owners) = setup(theme);
        add_component(&mut app, &ctx, owners[0], 2);
        add_component(&mut app, &ctx, owners[1], 1);
        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
        i.layers[0].collapsed = true;
        i.lfo_lane.collapsed = false;
        i.envelope_lane.collapsed = false;
        let before = app.session.project.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 1200.0));
        assert_eq!(app.session.project, before);
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Per note"))
        );
        for owner in owners {
            let b = &app.instrument_ui.modulation_lane_bounds[&owner];
            assert_eq!(b.row.height(), panels::unit_height(false));
            assert_eq!(b.toolbar.width(), lane_toolbar::WIDTH);
            assert_eq!(b.collapse.size(), Vec2::splat(icons::TOOLBAR_BUTTON_SIZE));
            let top = b.row.top() + 0.5;
            let stroke = Stroke::new(1.0, theme::palette(&ctx).border);
            let separators: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke: s }
                        if *s == stroke
                            && points[0].y == points[1].y
                            && (points[0].x - points[1].x).abs() > TRACK_WIDTH
                            && (points[0].y - top).abs() < 2.0 =>
                    {
                        Some(points[0].y)
                    }
                    _ => None,
                })
                .collect();
            assert!(!separators.is_empty());
            assert!(
                separators
                    .iter()
                    .all(|y| (*y - top).abs() < 0.1 || *y < b.row.top()),
                "neighboring lanes must share one separator pixel: {separators:?}"
            );
            assert_eq!(b.mix, [Rect::NOTHING]);
            assert!(b.powers[0].is_positive());
        }
        let point = app.instrument_ui.modulation_lane_bounds[&owners[1]].powers[0].center();
        frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            Vec2::new(1600.0, 1200.0),
        );
        frame_shapes(
            &mut app,
            &ctx,
            vec![button(point, false)],
            Vec2::new(1600.0, 1200.0),
        );
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .envelope_lane
                .envelopes[0]
                .enabled
        );
        let saved = serde_json::to_value(&app.session.project).unwrap();
        let loaded: daw_core::Project = serde_json::from_value(saved).unwrap();
        assert_eq!(loaded, app.session.project);
    }
}
#[test]
fn lane_unit_selection_delete_undo_and_invalid_cross_lane_drop_reuse_shared_gestures() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark);
    add_component(&mut app, &ctx, owners[0], 2);
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    let id = i.lfo_lane.lfos[0].id;
    let source = ModulationSourceId {
        layer: owners[0],
        unit: id,
    };
    assert_eq!(crate::modulation_panel::sources(Some(i))[0].id, source);
    let before = app.session.project.clone();
    let header = app.instrument_ui.modulation_lane_bounds[&owners[0]].lfos[0];
    let start = unit_title_point(header);
    let end = app.instrument_ui.modulation_lane_bounds[&owners[1]]
        .label
        .center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.component_drag.as_ref().is_some_and(|d| d.active));
    assert_eq!(app.instrument_catalog_bounds.drop_target, None);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project, before);
    click(&mut app, &ctx, start, false);
    assert!(
        app.instrument_ui
            .selected_components
            .contains(&(owners[0], InstrumentComponent::Lfo(id)))
    );
    key(&mut app, &ctx, egui::Key::Delete, false, false);
    assert!(
        !app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .lfo_lane
            .lfos[0]
            .present
    );
    app.perform_history_action(crate::history::HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, before);
    app.instrument_ui.select_layer(owners[0]);
    app.set_instrument_subview(
        crate::instrument_workspace::InstrumentSubview::Sources,
        &ctx,
    );
    frame(&mut app, &ctx, vec![]);
    let regular = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    let name = app.midi_editor.layer_bounds[&regular].name.center();
    click(&mut app, &ctx, name, true);
    assert_eq!(
        app.instrument_ui.selected_layers,
        HashSet::from([regular]),
        "fixed lanes stay outside regular layer groups"
    );
    assert_eq!(app.session.project, before);
}

#[test]
fn lfo_lane_header_reordering_commits_on_release_and_undo_restores_order() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Light);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("test-frame-size"), Vec2::new(1600.0, 800.0)));
    add_component(&mut app, &ctx, owners[0], 2);
    add_component(&mut app, &ctx, owners[0], 2);
    let before = app.session.project.clone();
    let i = before.tracks[0].instrument.as_ref().unwrap();
    let (a, b) = (i.lfo_lane.lfos[0].id, i.lfo_lane.lfos[1].id);
    let bounds = &app.instrument_ui.modulation_lane_bounds[&owners[0]];
    let start = unit_title_point(bounds.lfos[0]);
    let end = Pos2::new(
        unit_panel(&ctx, bounds.lfos[1]).right() - 3.0,
        bounds.lfos[1].center().y,
    );
    assert!(
        app.instrument_ui
            .component_viewports
            .iter()
            .any(|(id, viewport)| *id == owners[0] && viewport.contains(end))
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.session.project, before);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(
        i.lfo_lane.visible_components(),
        [InstrumentComponent::Lfo(b), InstrumentComponent::Lfo(a)]
    );
    app.perform_history_action(crate::history::HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, before);
}

#[test]
fn global_envelope_mode_keeps_shared_consumer_guards_and_per_note_availability() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark);
    add_component(&mut app, &ctx, owners[1], 1);
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.envelope_lane.collapsed = false;
    i.envelope_lane.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
    let source = ModulationSourceId {
        layer: owners[1],
        unit: i.envelope_lane.envelopes[0].id,
    };
    let mut lfo = daw_core::LfoInstance::default();
    lfo.modulations.push(daw_core::Modulation {
        source: Some(source),
        amount: 1.0,
        ..daw_core::Modulation::new(daw_core::ModulationParameter::LfoDepth)
    });
    i.lfo_lane.lfos.push(lfo);
    app.session.project.validate().unwrap();
    app.dirty = false;
    let before = app.session.project.clone();
    let size = Vec2::new(1600.0, 1200.0);
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    let label = |shapes: &[egui::epaint::ClippedShape], text: &str| {
        shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {text}"))
    };
    let mode = label(&shapes, "Legato");
    frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(mode), button(mode, true)],
        size,
    );
    frame_shapes(&mut app, &ctx, vec![button(mode, false)], size);
    let popup = frame_shapes(&mut app, &ctx, vec![], size);
    let per_note = label(&popup, "Per note");
    frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(per_note), button(per_note, true)],
        size,
    );
    frame_shapes(&mut app, &ctx, vec![button(per_note, false)], size);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty && app.error.is_none());
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .lfo_lane
        .lfos[0]
        .modulations
        .clear();
    let mut popup = frame_shapes(&mut app, &ctx, vec![], size);
    if !popup
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Per note"))
    {
        frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(mode), button(mode, true)],
            size,
        );
        frame_shapes(&mut app, &ctx, vec![button(mode, false)], size);
        popup = frame_shapes(&mut app, &ctx, vec![], size);
    }
    let per_note = label(&popup, "Per note");
    frame_shapes(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(per_note), button(per_note, true)],
        size,
    );
    frame_shapes(&mut app, &ctx, vec![button(per_note, false)], size);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .envelope_lane
            .envelopes[0]
            .mode,
        daw_core::EnvelopeMode::PerNote
    );
    assert!(app.dirty && app.error.is_none());
}
