use super::*;

fn menu_frame(app: &mut DawUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| app.view_menu(ui),
    );
    output.textures_delta.clear();
    output
}

fn label(output: &egui::FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}

#[test]
fn instrument_tabs_partition_units_and_keep_the_mixer_independent() {
    use crate::instrument_workspace::InstrumentSubview;
    use daw_core::InstrumentComponent;
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    instrument.layers[0].amplitude.present = true;
    instrument.layers[0].macro_module.add();
    instrument.main.macro_module.add();
    instrument.lfo_lane.lfos.push(Default::default());
    instrument.envelope_lane.envelopes.push(Default::default());
    let mut second = daw_core::InstrumentLayerSettings {
        name: "Second layer".into(),
        ..Default::default()
    };
    second.effects.push(daw_core::EffectInstance::filter());
    let [layer, output] = [instrument.layers[0].id, instrument.main.id];
    let [lfo, envelope] = [instrument.lfo_lane.id, instrument.envelope_lane.id];
    let other = second.id;
    instrument.layers.push(second);
    app.session.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    let ctx = context();
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("test-frame-size"), Vec2::new(1600.0, 1200.0))
    });
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_effects_lane_visible(false);
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    assert_eq!(app.instrument_subview, InstrumentSubview::Layer);
    let bounds = &app.midi_editor.layer_bounds[&layer];
    let effect = app.instrument_ui.effect_slots[0].2;
    assert!(bounds.oscillator_panel.bottom() < effect.top());
    assert!(effect.bottom() + 8.0 < bounds.label.top());
    assert!(
        bounds
            .mix_controls
            .iter()
            .all(|r| bounds.label.contains_rect(*r))
    );
    assert!(!app.midi_editor.layer_bounds.contains_key(&other));
    assert_eq!(
        app.effects_target(),
        Some(daw_core::EffectChainTarget::Track(track))
    );

    let click = |app: &mut DawUi, rect: Rect| {
        super::instrument_catalog::click(app, &ctx, rect.center(), false);
        frame(app, &ctx, vec![]);
    };
    let mixer_toggle = app.effects_bounds.track_mixer_toggle;
    click(&mut app, mixer_toggle);
    let panel = app.effects_bounds.track_mixer;
    assert_eq!(panel.width(), 2.0 * crate::mixer::STRIP_WIDTH);
    assert!(app.midi_editor.layer_bounds[&layer].toolbar.right() <= panel.left());

    let [lfos, envelopes, macros] = app.instrument_catalog_bounds.subviews;
    let [first, second] = [layer, other].map(|id| app.instrument_catalog_bounds.layer_tabs[&id]);
    assert_eq!(first.y_range(), second.y_range());
    assert_eq!(
        second.left() - first.right(),
        crate::rows::GROUP_DIVIDER_WIDTH
    );
    assert!(lfos.left() - second.right() > crate::rows::GROUP_DIVIDER_WIDTH);
    assert_eq!(
        envelopes.left() - lfos.right(),
        crate::rows::GROUP_DIVIDER_WIDTH
    );
    assert_eq!(
        macros.left() - envelopes.right(),
        crate::rows::GROUP_DIVIDER_WIDTH
    );
    click(&mut app, second);
    assert!(!app.midi_editor.layer_bounds.contains_key(&layer));
    assert!(app.midi_editor.layer_bounds.contains_key(&other));
    assert_eq!(app.instrument_ui.effect_slots[0].0, other);

    for (tab, subview, expected) in [
        (lfos, InstrumentSubview::Lfos, lfo),
        (envelopes, InstrumentSubview::Envelopes, envelope),
    ] {
        click(&mut app, tab);
        assert_eq!(app.instrument_subview, subview);
        assert_eq!(app.instrument_ui.modulation_lane_bounds.len(), 1);
        assert!(
            app.instrument_ui
                .modulation_lane_bounds
                .contains_key(&expected)
        );
        assert!(app.instrument_ui.effect_slots.is_empty());
        assert!(app.instrument_ui.source_drop_rows.is_empty());
    }
    click(&mut app, macros);
    assert_eq!(app.instrument_subview, InstrumentSubview::Macros);
    assert!(app.instrument_ui.modulation_lane_bounds.is_empty());
    assert_eq!(
        app.instrument_ui
            .component_slots
            .iter()
            .map(|(owner, component, _)| (*owner, *component))
            .collect::<Vec<_>>(),
        [
            (output, InstrumentComponent::Macros),
            (layer, InstrumentComponent::Macros)
        ]
    );
    click(&mut app, first);
    assert_eq!(app.instrument_subview, InstrumentSubview::Layer);
    assert_eq!(app.instrument_ui.effect_slots[0].0, layer);

    app.toggle_view_panel(ViewPanel::EffectsLane, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.strip.is_positive());
    assert_eq!(app.effects_bounds.chain.len(), 1);
    assert_eq!(app.effects_bounds.track_mixer_toggle, mixer_toggle);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert!(!app.view_panel_selected(ViewPanel::TrackMixer));
    app.set_view_mode(ViewMode::Instrument, &ctx);
    assert!(app.view_panel_selected(ViewPanel::TrackMixer));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn beat_parent_uses_the_audio_and_group_instrument_layout_in_every_tab() {
    use crate::instrument_workspace::InstrumentSubview;
    let mut app = DawUi::default();
    let audio = app.session.project.add_track().unwrap();
    let group = app.session.project.add_group_track().unwrap();
    let beat = app.session.project.add_beat_track().unwrap();
    for track in &mut app.session.project.tracks {
        track.effects.push(daw_core::EffectInstance::drive());
    }
    app.session.project.tracks[2]
        .beat
        .as_mut()
        .unwrap()
        .output
        .macro_module
        .add();
    let before = app.session.project.clone();
    let ctx = context();
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_effects_lane_visible(true);
    for subview in [
        InstrumentSubview::Sources,
        InstrumentSubview::Effects,
        InstrumentSubview::Modulation,
    ] {
        app.set_instrument_subview(subview, &ctx);
        let mut layout = None;
        for id in [audio, group, beat] {
            app.select_track(id, false);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 900.0));
            let text = |message| {
                shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == message => Some(text.pos),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing {message}"))
            };
            let empty = [text("Layers"), text("Select an instrument track")];
            assert_eq!(layout.get_or_insert(empty), &empty);
            assert!(app.midi_editor.layer_bounds.is_empty());
            assert!(app.instrument_ui.component_viewports.is_empty());
            assert!(app.instrument_catalog_target().is_none());
            assert_eq!(
                app.effects_target(),
                Some(daw_core::EffectChainTarget::Track(id))
            );
            assert_eq!(app.effects_bounds.chain.len(), 1);
        }
    }
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn panel_menu_availability_and_checks_follow_the_current_view() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    for (view, enabled) in [
        (
            ViewMode::Timeline,
            [false, false, true, true, true, true, true, true, true, true],
        ),
        (
            ViewMode::Instrument,
            [
                false, true, true, false, false, true, true, false, false, false,
            ],
        ),
        (
            ViewMode::Edit,
            [
                true, false, true, true, false, false, true, false, false, false,
            ],
        ),
        (
            ViewMode::Mixer,
            [
                false, false, true, false, false, false, true, false, true, true,
            ],
        ),
    ] {
        app.set_view_mode(view, &ctx);
        for (panel, available) in ViewPanel::ALL.into_iter().zip(enabled) {
            assert_eq!(
                app.view_panel_enabled(panel, &ctx),
                available,
                "{view:?}: {panel:?}"
            );
            if !available {
                assert!(!app.view_panel_selected(panel));
                app.toggle_view_panel(panel, &ctx);
                assert!(!app.view_panel_selected(panel));
            }
        }
    }
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.set_effects_catalog_visible(true);
    app.set_view_mode(ViewMode::Mixer, &ctx);
    app.set_effects_catalog_visible(false);
    assert!(!app.view_panel_selected(ViewPanel::EffectsCatalog));
    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    app.settings_open = true;
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    for panel in ViewPanel::ALL {
        assert!(!app.view_panel_enabled(panel, &ctx));
        let selected = app.view_panel_selected(panel);
        app.toggle_view_panel(panel, &ctx);
        assert_eq!(app.view_panel_selected(panel), selected);
    }
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn timeline_editor_shares_the_lower_lane_and_resizes_from_its_top_edge() {
    let (mut app, audio) = fixture();
    let ctx = context();
    let clip = app.session.project.tracks[0].clips[0].id;
    app.select_clip(clip, audio, false);
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let timeline_transform = (app.zoom, app.scroll);
    let editor_toggle = app.timeline_editor.toggle_bounds.center();
    let effects_toggle = app.effects_bounds.lane_toggle.center();
    let click = |app: &mut DawUi, pos| {
        super::instrument_catalog::click(app, &ctx, pos, false);
        frame(app, &ctx, vec![]);
    };
    let drag = |app: &mut DawUi, start, end| {
        frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(app, &ctx, vec![egui::Event::PointerMoved(end)]);
    };

    click(&mut app, editor_toggle);
    assert!(app.view_panel_selected(ViewPanel::EditorPane));
    assert!(!app.view_panel_selected(ViewPanel::EffectsLane));
    assert!(!app.effects_bounds.strip.is_positive());
    assert!(app.midi_editor.content.bounds.is_positive());
    assert_eq!(app.view_mode, ViewMode::Timeline);
    let divider = app.timeline_editor.divider_bounds;
    assert_eq!(app.midi_editor.panel_bounds.top(), divider.top());
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::LineSegment { points, stroke }
            if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                && points[0] == divider.left_top() + Vec2::new(0.0, 0.5)
                && points[1] == divider.right_top() + Vec2::new(0.0, 0.5)
    )));
    let start = app.timeline_editor.divider_bounds.center();
    assert!(app.timeline_bounds.bottom() <= app.timeline_editor.divider_bounds.top());
    let end = start - Vec2::new(0.0, 80.0);
    drag(&mut app, start, end);
    assert!(app.timeline_editor.resizing());
    assert!((app.timeline_editor.divider_bounds.center().y - end.y).abs() < 1.0);
    assert_eq!(
        app.current_project_display().timeline_editor_height_points,
        300.0
    );
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.timeline_editor.resizing());
    assert_eq!(
        app.current_project_display().timeline_editor_height_points,
        380.0
    );
    let resized = app.timeline_editor.divider_bounds;
    let end = resized.center() + Vec2::new(0.0, 40.0);
    drag(&mut app, resized.center(), end);
    assert!(app.timeline_editor.divider_bounds.top() > resized.top());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(app.timeline_editor.divider_bounds, resized);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.timeline_editor.resizing());

    click(&mut app, effects_toggle);
    assert!(app.view_panel_selected(ViewPanel::EffectsLane));
    assert!(!app.view_panel_selected(ViewPanel::EditorPane));
    assert!(app.effects_bounds.strip.is_positive());
    click(&mut app, editor_toggle);
    assert_eq!(app.timeline_editor.divider_bounds, resized);
    frame_sized(&mut app, &ctx, vec![], Vec2::new(1280.0, 400.0));
    assert!(app.midi_editor.panel_bounds.height() < 380.0);
    assert_eq!(
        app.current_project_display().timeline_editor_height_points,
        380.0
    );
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.timeline_editor.divider_bounds, resized);
    click(&mut app, editor_toggle);
    assert!(!app.view_panel_selected(ViewPanel::EffectsLane));
    assert!(!app.view_panel_selected(ViewPanel::EditorPane));
    assert_eq!(app.selected_clip, Some(clip));
    assert_eq!((app.zoom, app.scroll), timeline_transform);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn timeline_editor_follows_the_selected_track_and_edits_the_shared_beat_pattern() {
    let mut app = DawUi::default();
    let midi = app.session.project.add_midi_track().unwrap();
    let midi_clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
    let note = Id::new_v4();
    let pattern = app.session.project.assets[0].midi_pattern.as_mut().unwrap();
    pattern.notes.push(daw_core::MidiNote {
        id: note,
        start_tick: 0,
        length_ticks: daw_core::MIDI_TICKS_PER_BEAT / 4,
        pitch: 69,
        velocity: 100,
    });
    let beat = app.session.project.add_beat_track().unwrap();
    let row = daw_core::BeatSubtrack::new("Kick".into());
    let row_id = row.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: beat,
            edit: daw_core::BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    let beat_clip = app.session.project.add_beat_pattern(beat, 0).unwrap();
    let ctx = context();
    app.set_timeline_editor_visible(true);
    app.select_clip(midi_clip, midi, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.midi_editor.active_track, Some(midi));
    assert!(
        app.midi_editor
            .pattern_header_bounds
            .contains_key(&midi_clip)
    );
    assert!(
        app.midi_editor
            .content
            .bounds
            .intersects(app.midi_editor.note_bounds[&note])
    );
    app.set_view_mode(ViewMode::Edit, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.midi_editor
            .content
            .bounds
            .intersects(app.midi_editor.note_bounds[&note])
    );
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.select_track(beat, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.midi_editor.active_track, Some(beat));
    assert_eq!(app.selected_clip, Some(midi_clip));
    app.select_clip(beat_clip, beat, false);
    frame(&mut app, &ctx, vec![]);
    let step = app.beat_ui.steps[&(row_id, 0)].center();
    let before = app.session.project.clone();
    super::instrument_catalog::click(&mut app, &ctx, step, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.view_mode, ViewMode::Timeline);
    assert!(app.dirty);
    assert_ne!(app.session.project, before);
    assert_eq!(app.selected_clip, Some(beat_clip));
}

#[test]
fn panel_menu_clicks_share_catalog_exclusivity_and_toolbar_state() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    assert!(app.view_panel_selected(ViewPanel::Components));
    assert!(!app.view_panel_selected(ViewPanel::EffectsCatalog));
    let output = menu_frame(&mut app, &ctx, vec![]);
    let point = label(&output, "Effects Catalog");
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(app.view_panel_selected(ViewPanel::EffectsCatalog));
    assert!(!app.view_panel_selected(ViewPanel::Components));
    app.set_view_mode(ViewMode::Mixer, &ctx);
    let before = app.session.project.clone();
    let output = menu_frame(&mut app, &ctx, vec![]);
    let point = label(&output, "Returns");
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(!app.mixer_returns_visible);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn panel_items_have_outer_dividers_between_mixer_and_fullscreen() {
    let (mut app, _) = fixture();
    let ctx = context();
    menu_frame(&mut app, &ctx, vec![]);
    let output = menu_frame(&mut app, &ctx, vec![]);
    let mixer = label(&output, "Mixer");
    let first = label(&output, ViewPanel::Components.label());
    let last = label(&output, ViewPanel::ALL.last().unwrap().label());
    let fullscreen = label(&output, "Enter Full Screen");
    assert!(!output.shapes.iter().any(|item| {
        matches!(&item.shape, egui::Shape::Text(text) if text.galley.text() == "Tracks")
    }));
    let dividers: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, .. }
                if points[0].y == points[1].y && (points[1].x - points[0].x).abs() > 100.0 =>
            {
                Some(points[0].y)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dividers.len(), 2);
    assert!(mixer.y < dividers[0] && dividers[0] < first.y);
    assert!(last.y < dividers[1] && dividers[1] < fullscreen.y);
    let mut previous = mixer.y;
    for panel in ViewPanel::ALL
        .into_iter()
        .filter(|p| *p != ViewPanel::Tracks)
    {
        let point = label(&output, panel.label());
        assert!(point.y > previous);
        previous = point.y;
    }
    let point = fullscreen;
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    let output = menu_frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Fullscreen(true)))
    );
}

#[test]
fn layer_tabs_reorder_and_receive_effects_on_release() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].collapsed = true;
    let effect = daw_core::EffectInstance::drive();
    let effect_id = effect.id;
    instrument.layers[0].effects.push(effect);
    let second = daw_core::InstrumentLayerSettings {
        collapsed: true,
        name: "Layer 2".into(),
        ..Default::default()
    };
    let [first_id, second_id] = [instrument.layers[0].id, second.id];
    instrument.layers.push(second);
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let first = app.instrument_catalog_bounds.layer_tabs[&first_id].center();
    let second = app.instrument_catalog_bounds.layer_tabs[&second_id];
    let end = second.right_center() - Vec2::X * 2.0;
    let drag = |app: &mut DawUi, start, end| {
        let before = app.session.project.clone();
        frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.session.project, before);
        frame(app, &ctx, vec![button(end, false)]);
        frame(app, &ctx, vec![]);
    };
    drag(&mut app, first, end);
    let layers = &app
        .session
        .project
        .instrument_settings(track)
        .unwrap()
        .layers;
    assert_eq!([layers[0].id, layers[1].id], [second_id, first_id]);
    let start = unit_slot_point(app.instrument_ui.effect_slots[0].2);
    let tab = app.instrument_catalog_bounds.layer_tabs[&second_id].center();
    drag(&mut app, start, tab);
    let layers = &app
        .session
        .project
        .instrument_settings(track)
        .unwrap()
        .layers;
    assert_eq!(layers[0].effects[0].id, effect_id);
    assert!(layers[1].effects.is_empty());
    assert_eq!(app.instrument_ui.layer_page, Some(second_id));
    assert!(app.session.project.tracks[0].effects.is_empty());
}

#[test]
fn layer_tab_context_menu_edits_the_clicked_layer_and_duplicate_shortcut_selects_its_copy() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    let first = instrument.layers[0].id;
    let layer = daw_core::InstrumentLayerSettings {
        name: "Layer 2".into(),
        ..Default::default()
    };
    let second = layer.id;
    instrument.layers.push(layer);
    let ctx = context();
    ctx.set_os(egui::os::OperatingSystem::Mac);
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let size = Vec2::new(1280.0, 800.0);
    for action in ["Mute", "Solo", "Duplicate", "Delete"] {
        let point = app.instrument_catalog_bounds.layer_tabs[&second].center();
        let before = app.session.project.clone();
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
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
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        let label = |name| {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == name => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing {name}"))
        };
        assert_eq!(app.session.project, before);
        let positions = ["Duplicate", "Mute", "Solo", "Delete"].map(label);
        assert!(positions.windows(2).all(|pair| pair[0].y < pair[1].y));
        assert!(label("⌘D").x > positions[0].x);
        super::instrument_catalog::click(&mut app, &ctx, label(action), false);
        let layers = &app
            .session
            .project
            .instrument_settings(track)
            .unwrap()
            .layers;
        assert_eq!(
            layers[0],
            before.instrument_settings(track).unwrap().layers[0]
        );
        match action {
            "Mute" => assert!(layers[1].muted),
            "Solo" => assert!(layers[1].soloed),
            "Duplicate" => {
                assert_eq!(layers.len(), 3);
                assert_ne!(layers[2].id, second);
                assert!(layers[2].muted && layers[2].soloed);
                assert_eq!(app.instrument_ui.layer_page, Some(layers[2].id));
            }
            "Delete" => assert!(layers.iter().all(|layer| layer.id != second)),
            _ => unreachable!(),
        }
    }
    let point = app.instrument_catalog_bounds.layer_tabs[&first].center();
    super::instrument_catalog::click(&mut app, &ctx, point, false);
    let modifiers = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    for repeat in [false, true] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key: egui::Key::D,
                    physical_key: None,
                    pressed: true,
                    repeat,
                    modifiers,
                },
            ],
        );
        assert_eq!(app.session.project.tracks.len(), 1);
        let layers = &app
            .session
            .project
            .instrument_settings(track)
            .unwrap()
            .layers;
        assert_eq!(layers.len(), 3);
        assert_eq!(app.instrument_ui.layer_page, Some(layers[1].id));
    }
}

#[test]
fn instrument_tab_groups_match_toolbar_spacing_and_round_outer_corners() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    let size = Vec2::new(1280.0, 800.0);
    let layer = app
        .session
        .project
        .instrument_settings(track)
        .unwrap()
        .layers[0]
        .id;
    for count in [1, 2] {
        if count == 2 {
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers
                .push(daw_core::InstrumentLayerSettings::default());
        }
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        let plus = app.instrument_catalog_bounds.layer_add_tab;
        let last = app
            .session
            .project
            .instrument_settings(track)
            .unwrap()
            .layers
            .last()
            .unwrap()
            .id;
        let last = app.instrument_catalog_bounds.layer_tabs[&last];
        assert_eq!(plus.left() - last.right(), crate::rows::GROUP_DIVIDER_WIDTH);
        let lfos = app.instrument_catalog_bounds.subviews[0];
        let column = app.instrument_catalog_bounds.track_column;
        let tab = app.instrument_catalog_bounds.layer_tabs[&layer];
        let gap = lfos.left() - plus.right();
        assert_eq!(gap, f32::from(theme::TOOLBAR_PADDING));
        assert_eq!(gap, tab.left() - column.right());
        assert_eq!(gap, tab.top() - column.top());
        assert!(
            shapes
                .iter()
                .any(|item| matches!(&item.shape, egui::Shape::Rect(shape)
            if shape.rect == plus && shape.fill != egui::Color32::TRANSPARENT
                && [shape.corner_radius.nw, shape.corner_radius.ne,
                    shape.corner_radius.sw, shape.corner_radius.se] == [0, 5, 0, 5]))
        );
        assert!(
            shapes.iter().any(|item| {
                matches!(&item.shape, egui::Shape::Rect(shape)
                if shape.rect == tab
                    && shape.fill == ctx.global_style().visuals.selection.bg_fill
                    && [shape.corner_radius.nw, shape.corner_radius.ne,
                        shape.corner_radius.sw, shape.corner_radius.se] == [5, 0, 5, 0])
            }),
            "{count} layer tabs"
        );
    }
}

#[test]
fn overflowing_layers_scroll_without_moving_add_or_modulation_tabs() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let layers = &mut app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers;
    let first = layers[0].id;
    layers.extend((2..=24).map(|index| daw_core::InstrumentLayerSettings {
        name: format!("Instrument layer {index}"),
        ..Default::default()
    }));
    let last = layers.last().unwrap().id;
    let ctx = context();
    let size = Vec2::new(900.0, 600.0);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("test-frame-size"), size));
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);

    let before = app.instrument_catalog_bounds.layer_tabs[&first];
    let providers = app.instrument_catalog_bounds.subviews;
    let plus = app.instrument_catalog_bounds.layer_add_tab;
    assert!(app.instrument_catalog_bounds.layer_tabs[&last].left() > plus.left());
    assert_eq!(
        providers[0].left() - plus.right(),
        f32::from(theme::TOOLBAR_PADDING)
    );
    assert!(providers[2].right() < size.x - timeline_toolbar::WIDTH);
    for tab in [plus, providers[1]] {
        frame(
            &mut app,
            &ctx,
            trackpad_scroll(tab.center(), Vec2::new(-150.0, 0.0)),
        );
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.instrument_catalog_bounds.layer_tabs[&first], before);
    }

    frame(
        &mut app,
        &ctx,
        trackpad_scroll(before.center(), Vec2::new(-4000.0, 0.0)),
    );
    frame(&mut app, &ctx, vec![]);
    assert!(app.instrument_catalog_bounds.layer_tabs[&first].right() < before.left());
    assert_eq!(app.instrument_catalog_bounds.subviews, providers);
    assert_eq!(app.instrument_catalog_bounds.layer_add_tab, plus);
    let last = app.instrument_catalog_bounds.layer_tabs[&last];
    assert!((plus.left() - last.right() - rows::GROUP_DIVIDER_WIDTH).abs() < 1.0);

    app.toggle_view_panel(ViewPanel::TrackMixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.instrument_track_mixer_visible);
    assert_eq!(app.instrument_catalog_bounds.subviews, providers);
    assert_eq!(app.instrument_catalog_bounds.layer_add_tab, plus);

    for (tab, subview) in providers.into_iter().zip([
        crate::instrument_workspace::InstrumentSubview::Lfos,
        crate::instrument_workspace::InstrumentSubview::Envelopes,
        crate::instrument_workspace::InstrumentSubview::Macros,
    ]) {
        super::instrument_catalog::click(&mut app, &ctx, tab.center(), false);
        assert_eq!(app.instrument_subview, subview);
    }
    assert!(!app.dirty);
    super::instrument_catalog::click(&mut app, &ctx, plus.center(), false);
    let layers = &app
        .session
        .project
        .instrument_settings(track)
        .unwrap()
        .layers;
    assert_eq!(layers.len(), 25);
    assert_eq!(
        app.instrument_ui.layer_page,
        layers.last().map(|layer| layer.id)
    );
}

#[test]
fn instrument_track_headers_share_timeline_height_and_disable_horizontal_controls() {
    let mut app = DawUi::default();
    let audio = app.session.project.add_track().unwrap();
    let midi = app.session.project.add_midi_track().unwrap();
    let ctx = context();
    app.select_track(audio, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let click = |app: &mut DawUi, point| super::instrument_catalog::click(app, &ctx, point, false);
    let midi_header = app.lane_bounds[&midi];
    assert!(
        app.instrument_catalog_bounds
            .track_column
            .contains_rect(midi_header)
    );
    click(&mut app, midi_header.left_top() + Vec2::new(65.0, 19.0));
    assert_eq!(
        app.effects_target(),
        Some(daw_core::EffectChainTarget::Track(midi))
    );
    assert_eq!(app.instrument_ui.track, Some(midi));

    let zoom = app.zoom;
    let zoom_point = app.midi_editor.zoom_bounds.right_center() - Vec2::X * 2.0;
    click(&mut app, zoom_point);
    assert_eq!(app.zoom, zoom);
    let compact = app.track_rows_bounds.center();
    click(&mut app, compact);
    let compact_height = app.lane_bounds[&midi].height();
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.lane_bounds[&midi].height(), compact_height);
    let slider = app.track_size_bounds.right_center() - Vec2::X * 2.0;
    click(&mut app, slider);
    let expanded_height = app.lane_bounds[&midi].height();
    assert!(expanded_height > compact_height);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.lane_bounds[&midi].height(), expanded_height);
    assert!(!app.dirty);
}

#[test]
fn instrument_has_one_tracks_panel_with_a_divider_below_its_buttons() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert_eq!(app.track_browser.bounds.toggle, Rect::NOTHING);
    assert!(!app.view_panel_enabled(ViewPanel::Tracks, &ctx));
    assert!(!app.tracks_panel_visible());
    let column = app.instrument_catalog_bounds.track_column;
    let border = theme::palette(&ctx).border;
    let y = column.top() + toolbars::HEIGHT - 0.5;
    assert_eq!(
        shapes
            .iter()
            .filter(
                |item| matches!(&item.shape, egui::Shape::LineSegment { points, stroke }
            if stroke.color == border && points[0] == Pos2::new(column.left(), y)
                && points[1] == Pos2::new(column.right(), y))
            )
            .count(),
        1
    );
    assert!(column.contains_rect(app.lane_bounds[&track]));
    assert!(app.instrument_catalog_bounds.toggle.is_positive());
    assert!(app.effects_bounds.library_toggle.is_positive());
}

#[test]
fn track_scroll_follows_both_views_and_is_saved_from_instrument() {
    let mut app = DawUi::default();
    let first = app.session.project.add_track().unwrap();
    for _ in 0..19 {
        app.session.project.add_track().unwrap();
    }
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let timeline_top = app.lane_bounds[&first].top();
    let timeline_pointer = app.lane_bounds[&first].left_center() - Vec2::new(100.0, 0.0);
    frame(
        &mut app,
        &ctx,
        trackpad_scroll(timeline_pointer, Vec2::new(0.0, -150.0)),
    );
    let offset = app.track_scroll_offset;
    assert!(offset > 0.0);

    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let column = app.instrument_catalog_bounds.track_column;
    let instrument_top = column.top() + toolbars::HEIGHT;
    assert_eq!(app.track_scroll_offset, offset);
    assert!((app.lane_bounds[&first].top() - (instrument_top - offset)).abs() < 1.0);

    frame(
        &mut app,
        &ctx,
        trackpad_scroll(column.center(), Vec2::new(0.0, -120.0)),
    );
    let updated = app.track_scroll_offset;
    assert!(updated > offset);
    assert_eq!(
        app.current_project_display().vertical_scroll_points,
        updated
    );

    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.track_scroll_offset, updated);
    assert!((app.lane_bounds[&first].top() - (timeline_top - updated)).abs() < 1.0);
    assert!(!app.dirty);
}

#[test]
fn instrument_paints_the_expanded_beat_gutter_and_empty_tab_divider() {
    let mut app = DawUi::default();
    let beat = app.session.project.add_beat_track().unwrap();
    let ctx = context();
    for (index, name) in ["Kick", "Snare"].into_iter().enumerate() {
        app.session
            .project
            .edit(Edit::Beat {
                track_id: beat,
                edit: daw_core::BeatEdit::Add {
                    subtrack: Box::new(daw_core::BeatSubtrack::new(name.into())),
                    index,
                },
            })
            .unwrap();
    }
    app.session
        .project
        .edit(Edit::SetGroupCollapsed {
            track_id: beat,
            collapsed: false,
        })
        .unwrap();
    app.select_track(beat, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let header = app.lane_bounds[&beat];
    let last = app
        .session
        .project
        .beat(beat)
        .unwrap()
        .subtracks
        .last()
        .unwrap();
    let child = app.timeline_beats.rows[&last.id].1;
    let gutter = Rect::from_min_max(
        Pos2::new(header.left(), header.bottom()),
        Pos2::new(child.left(), child.bottom()),
    );
    let color = theme::clip_colors(app.session.project.tracks[0].color, None).body;
    assert!(shapes.iter().any(|item| {
        matches!(&item.shape, egui::Shape::Rect(rect) if rect.rect == gutter && rect.fill == color)
    }));

    let column = app.instrument_catalog_bounds.track_column;
    let y = column.top() + toolbars::HEIGHT - 0.5;
    let border = theme::palette(&ctx).border;
    assert_eq!(
        shapes
            .iter()
            .filter(
                |item| matches!(&item.shape, egui::Shape::LineSegment { points, stroke }
                if stroke.color == border && points[0] == Pos2::new(column.right(), y)
                    && points[1].y == y && points[1].x > column.right() + 100.0)
            )
            .count(),
        1
    );
    let padding = f32::from(theme::TOOLBAR_PADDING);
    let expected = Pos2::new(column.right(), y + 0.5) + Vec2::splat(padding);
    assert!(shapes.iter().any(|item| {
        matches!(&item.shape, egui::Shape::Text(text)
            if text.galley.text() == "Select an instrument track"
                && text.pos.distance(expected) < 1.0)
    }));
}
