use super::*;
use daw_core::{BeatEdit, BeatPattern, BeatSubtrack, InstrumentSettings, OscillatorSettings};
fn setup() -> (DawUi, egui::Context, Id, Id, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_beat_track().unwrap();
    let mut row = BeatSubtrack::new("Kick".into());
    row.instrument = InstrumentSettings::from(OscillatorSettings::default());
    let id = row.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    let clip = app.session.project.add_beat_pattern(track, 0).unwrap();
    app.select_clip(clip, track, false);
    app.view_mode = ViewMode::Edit;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    (app, ctx, track, id, clip)
}
fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
    frame(app, ctx, vec![]);
}

#[test]
fn velocity_popup_accepts_typed_values_and_preserves_other_clip_content() {
    let (mut app, ctx, parent, row, clip) = setup();
    let point = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, point);
    let mut other = app.session.project.tracks[0].clips[0].clone();
    other.id = Id::new_v4();
    other.start_frame += other.length_frames;
    app.session.project.tracks[0].clips.push(other.clone());
    app.session.project.validate().unwrap();
    let source = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == other.asset_id)
        .unwrap()
        .clone();
    let instrument = app.session.project.beat(parent).unwrap().clone();
    frame(&mut app, &ctx, vec![]);
    let point = app.beat_ui.steps[&(row, 0)].center();
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
    let mut current = 100;
    for (typed, expected) in [("63", 63), ("999", 127), ("0", 1)] {
        let label = current.to_string();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let input = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
            .expect("velocity value");
        click(&mut app, &ctx, input.center());
        assert!(egui::Popup::is_any_open(&ctx));
        assert!(ctx.text_edit_focused());
        frame(&mut app, &ctx, vec![egui::Event::Text(typed.into())]);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
        assert!(egui::Popup::is_any_open(&ctx));
        let active = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == clip)
            .unwrap();
        let pattern = app
            .session
            .project
            .assets
            .iter()
            .find(|a| a.id == active.asset_id)
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap();
        assert_eq!(pattern.pattern.steps.len(), 1);
        let step = &pattern.pattern.steps[0];
        assert_eq!(
            (step.subtrack_id, step.start_tick, step.velocity),
            (row, 0, expected)
        );
        assert_eq!(app.session.project.tracks[0].clips[1], other);
        assert_eq!(
            app.session
                .project
                .assets
                .iter()
                .find(|a| a.id == source.id)
                .unwrap(),
            &source
        );
        assert_eq!(app.session.project.beat(parent).unwrap(), &instrument);
        assert_eq!(app.selected_clip, Some(clip));
        current = expected;
    }
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!egui::Popup::is_any_open(&ctx));
}

#[test]
fn editor_subtrack_reorder_owns_its_release_with_tracks_open_or_closed() {
    for tracks_open in [false, true] {
        for cancel in ["none", "escape", "focus", "outside", "view"] {
            let (mut app, ctx, parent, first, _) = setup();
            let second = BeatSubtrack::new("Second".into());
            let second_id = second.id;
            app.session
                .project
                .edit(Edit::Beat {
                    track_id: parent,
                    edit: BeatEdit::Add {
                        subtrack: Box::new(second),
                        index: 1,
                    },
                })
                .unwrap();
            app.set_tracks_panel_visible(tracks_open);
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            let press = app.beat_ui.headers[&first].center();
            let mut release = Pos2::new(press.x, app.beat_ui.headers[&second_id].bottom() - 4.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(press), button(press, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(release)]);
            assert!(app.beat_dragging());
            assert_eq!(app.session.project, original, "preview must not commit");
            match cancel {
                "escape" => {
                    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false)
                }
                "focus" => {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(1280.0, 800.0),
                            )),
                            focused: false,
                            events: vec![egui::Event::WindowFocused(false)],
                            ..Default::default()
                        },
                        |ui| app.show(ui),
                    );
                    output.textures_delta.clear();
                }
                "outside" => release = Pos2::new(1270.0, 790.0),
                "view" => {
                    app.set_view_mode(ViewMode::Instrument, &ctx);
                    assert_eq!(
                        app.view_mode,
                        ViewMode::Edit,
                        "active drags guard view switching"
                    );
                    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
                    app.set_view_mode(ViewMode::Instrument, &ctx);
                    assert_eq!(app.view_mode, ViewMode::Instrument);
                    release = Pos2::new(1270.0, 790.0);
                }
                _ => {}
            }
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(release), button(release, false)],
            );
            frame(&mut app, &ctx, vec![]);
            assert!(!app.beat_dragging());
            if cancel == "none" {
                let actual: Vec<_> = app
                    .session
                    .project
                    .beat(parent)
                    .unwrap()
                    .subtracks
                    .iter()
                    .map(|s| s.id)
                    .collect();
                assert_eq!(actual, vec![second_id, first], "Tracks open={tracks_open}");
            } else {
                assert_eq!(app.session.project, original, "{tracks_open} {cancel}");
            }
        }
    }
}
#[test]
fn beat_grid_click_edits_only_active_source_and_survives_view_switch() {
    let (mut app, ctx, track, row, clip) = setup();
    let old = app.session.project.tracks[0].clips[0].asset_id;
    let point = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, point);
    assert_ne!(app.session.project.tracks[0].clips[0].asset_id, old);
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    assert_eq!(
        app.session
            .project
            .assets
            .iter()
            .find(|a| a.id == asset)
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap()
            .pattern
            .steps
            .len(),
        1
    );
    assert!(
        app.session
            .project
            .assets
            .iter()
            .find(|a| a.id == old)
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap()
            .pattern
            .steps
            .is_empty()
    );
    assert_eq!(app.selected_clip, Some(clip));
    app.view_mode = ViewMode::Instrument;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_track, Some(track));
    assert_eq!(
        app.selected_instrument_owner(),
        Some(app.session.project.beat(track).unwrap().output.id)
    );
    assert!(app.error.is_none(), "{:?}", app.error);
    app.view_mode = ViewMode::Edit;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.primary, Some(row));
}
#[test]
fn selected_subtrack_reuses_the_midi_layers_body_and_parent_has_only_final_output() {
    let (mut app, ctx, track, row, _) = setup();
    app.navigate_beat_owner(track, Some(row), false);
    app.view_mode = ViewMode::Instrument;
    let size = Vec2::new(1600.0, 1400.0);
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    let text: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains(&"Layers")
            && text.contains(&"Layer 1")
            && text.contains(&"LFO")
            && text.contains(&"Envelope")
    );
    assert_eq!(text.iter().filter(|text| **text == "Output").count(), 1);
    let settings = app
        .session
        .project
        .instrument_settings(row)
        .unwrap()
        .clone();
    let layer = settings.layers[0].id;
    let bounds = app.midi_editor.layer_bounds[&layer].row;
    let output_bounds = app.instrument_ui.main_bounds.row;
    let mut midi = DawUi::default();
    let midi_track = midi.session.project.add_midi_track().unwrap();
    midi.session.project.tracks[0].instrument = Some(settings);
    midi.select_track(midi_track, false);
    midi.view_mode = ViewMode::Instrument;
    frame_shapes(&mut midi, &ctx, vec![], size);
    assert_eq!(midi.midi_editor.layer_bounds[&layer].row, bounds);
    assert_eq!(midi.instrument_ui.main_bounds.row, output_bounds);
    app.navigate_beat_owner(track, None, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    assert!(app.midi_editor.layer_bounds.is_empty());
    assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if matches!(text.galley.text(), "Layers" | "Layer 1" | "LFO" | "Envelope" | "Subtracks"))));
    assert_eq!(
        app.selected_instrument_owner(),
        Some(app.session.project.beat(track).unwrap().output.id)
    );
}

#[test]
fn editor_without_active_clip_keeps_headers_but_shows_no_pattern_cells() {
    let (mut app, ctx, _, row, _) = setup();
    app.selected_clip = None;
    app.selected_clips.clear();
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.steps.is_empty());
    assert!(app.beat_ui.clip_steps.is_empty());
    assert!(app.beat_ui.disabled_steps.is_empty());
    assert!(app.beat_ui.headers.contains_key(&row));
    assert_eq!(app.session.project, before);
}

#[test]
fn beat_creation_uses_the_same_action_and_dynamic_pattern_label() {
    let mut app = DawUi::default();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.perform_track_action(TrackAction::AddBeat, &ctx);
    assert_eq!(
        app.session.project.tracks[0].kind,
        daw_core::TrackKind::Beat
    );
    assert_eq!(
        app.clip_menu_action_label(ClipMenuAction::AddMidi),
        "Add beat pattern"
    );
    assert!(app.track_action_enabled(TrackAction::AddBeat, &ctx));
    app.view_mode = ViewMode::Edit;
    frame(&mut app, &ctx, vec![]);
    let track = app.session.project.tracks[0].id;
    let point = app.lane_bounds[&track].center();
    click(&mut app, &ctx, point);
    click(&mut app, &ctx, point);
    assert_eq!(app.session.project.tracks[0].clips.len(), 1);
    let clip = &app.session.project.tracks[0].clips[0];
    assert!(
        app.session
            .project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .unwrap()
            .beat_pattern
            .is_some()
    );
    assert_eq!(app.selected_clip, Some(clip.id));
}
#[test]
fn hidden_hits_remain_in_source_after_grid_changes_from_shared_project_edits() {
    let (mut app, ctx, track, row, clip) = setup();
    let pattern = BeatPattern::default()
        .with_step(row, 240, Some(101))
        .unwrap()
        .with_grid(1, daw_core::BeatResolution::Quarter)
        .unwrap();
    app.edit(Edit::ReplaceBeatPattern {
        clip_id: clip,
        pattern,
    });
    frame(&mut app, &ctx, vec![]);
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    let pattern = &app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset)
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap()
        .pattern;
    assert_eq!(pattern.steps.len(), 1);
    assert_eq!(pattern.visible_steps().count(), 0);
    assert_eq!(app.selected_track, Some(track));
}

#[test]
fn shared_header_selection_delete_and_group_drag_work_in_both_views() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, first, clip) = setup();
        let mut ids = vec![first];
        for name in ["Snare", "Hat"] {
            let row = BeatSubtrack::new(name.into());
            ids.push(row.id);
            app.session
                .project
                .edit(Edit::Beat {
                    track_id: track,
                    edit: BeatEdit::Add {
                        subtrack: Box::new(row),
                        index: ids.len() - 1,
                    },
                })
                .unwrap();
        }
        app.view_mode = view;
        if view == ViewMode::Instrument {
            app.track_browser.visible = true;
        }
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let a = app.beat_ui.headers[&ids[0]].center();
        let b = app.beat_ui.headers[&ids[1]].center();
        super::instrument_catalog::click(&mut app, &ctx, a, false);
        super::instrument_catalog::click(&mut app, &ctx, b, true);
        assert!(!app.dirty, "selection must not edit the project");
        // Move the selected first pair after the unselected last row.
        let destination = Pos2::new(
            a.x,
            app.beat_ui.headers[&ids[2]].bottom()
                + if view == ViewMode::Instrument {
                    -1.0
                } else {
                    50.0
                },
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(a), button(a, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(destination)]);
        frame(&mut app, &ctx, vec![button(destination, false)]);
        frame(&mut app, &ctx, vec![]);
        let order: Vec<_> = app
            .session
            .project
            .beat(track)
            .unwrap()
            .subtracks
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(order, vec![ids[2], ids[0], ids[1]], "{view:?}");
        let point = app.beat_ui.headers[&first].center();
        super::instrument_catalog::click(&mut app, &ctx, point, false);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Delete, false, false);
        assert_eq!(app.session.project.beat(track).unwrap().subtracks.len(), 2);
        assert_eq!(
            app.session.project.tracks[0].clips[0].id, clip,
            "subtrack deletion must not delete a clip"
        );
        assert!(app.error.is_none(), "{:?}", app.error);
    }
}
#[test]
fn header_drag_escape_and_outside_release_preserve_identity_order_and_history() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, first, _) = setup();
        let row = BeatSubtrack::new("Snare".into());
        let second = row.id;
        app.session
            .project
            .edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: 1,
                },
            })
            .unwrap();
        app.view_mode = view;
        if view == ViewMode::Instrument {
            app.track_browser.visible = true;
        }
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let a = app.beat_ui.headers[&first].center();
        let b = app.beat_ui.headers[&second].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(a), button(a, true)],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(b + Vec2::new(0.0, 70.0))],
        );
        assert!(app.beat_subtrack_selected(first));
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        frame(&mut app, &ctx, vec![button(b, false)]);
        assert!(
            app.beat_subtrack_selected(first),
            "Escape cancels the drag before clearing selection"
        );
        assert_eq!(
            app.session.project.beat(track).unwrap().subtracks[0].id,
            first
        );
        assert!(!app.dirty);
    }
}

#[test]
fn step_and_unit_focus_do_not_delete_selected_subtracks() {
    let (mut app, ctx, track, row, clip) = setup();
    let name = app.beat_ui.headers[&row].center();
    click(&mut app, &ctx, name);
    let step = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, step);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Delete, false, false);
    assert_eq!(app.session.project.beat(track).unwrap().subtracks.len(), 1);
    assert_eq!(app.session.project.tracks[0].clips[0].id, clip);
    app.view_mode = ViewMode::Instrument;
    frame(&mut app, &ctx, vec![]);
    assert!(app.error.is_none());
}

#[test]
fn beat_editor_clip_header_restores_clip_delete_after_step_editing() {
    let (mut app, ctx, _, row, clip) = setup();
    let step = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, step);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Delete, false, false);
    assert_eq!(app.session.project.tracks[0].clips[0].id, clip);
    let block = app.track_overview_clip_rect(
        app.midi_editor.track_bounds,
        &app.session.project.tracks[0].clips[0],
    );
    click(
        &mut app,
        &ctx,
        Pos2::new(block.center().x, block.top() + 10.0),
    );
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Backspace, false, false);
    assert!(app.session.project.tracks[0].clips.is_empty());
}

#[test]
fn component_catalog_targets_local_provider_lanes_and_fixed_output_across_frames() {
    let (mut app, ctx, track, row, _) = setup();
    app.navigate_beat_owner(app.selected_track.unwrap(), app.beat_ui.primary, false);
    app.view_mode = ViewMode::Instrument;
    app.instrument_catalog_visible = true;
    app.instrument_catalog.expand_folders();
    let sibling = BeatSubtrack::new("Snare".into());
    let sibling_id = sibling.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(sibling),
                index: 1,
            },
        })
        .unwrap();
    let lfo = app
        .session
        .project
        .instrument_settings(row)
        .unwrap()
        .lfo_lane
        .id;
    frame(&mut app, &ctx, vec![]);
    app.instrument_ui.select_layer(lfo);
    app.instrument_ui.track_effects_target = false;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.selected_layer, Some(lfo));
    let entry = crate::instrument_catalog::ENTRIES
        .iter()
        .position(|e| e.type_id == "daw.lfo")
        .unwrap();
    assert!(app.instrument_catalog_bounds.entries[entry].is_positive());
    let point = app.instrument_catalog_bounds.entries[entry].center();
    super::instrument_catalog::click(&mut app, &ctx, point, false);
    assert!(
        app.instrument_catalog.selected_types().contains(&"daw.lfo"),
        "selected {:?}, error {:?}, owner {:?}",
        app.instrument_catalog.selected_types(),
        app.error,
        app.instrument_ui.selected_layer
    );
    assert_eq!(app.instrument_ui.selected_layer, Some(lfo));
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_eq!(
        app.session
            .project
            .instrument_settings(row)
            .unwrap()
            .lfo_lane
            .lfos
            .len(),
        1
    );
    assert!(
        app.session
            .project
            .instrument_settings(sibling_id)
            .unwrap()
            .lfo_lane
            .lfos
            .is_empty()
    );
    assert!(
        app.session.project.instrument_settings(row).unwrap().layers[0]
            .lfos
            .is_empty()
    );
    app.navigate_beat_owner(track, None, false);
    frame(&mut app, &ctx, vec![]);
    let root = app.session.project.beat(track).unwrap().output.id;
    assert_eq!(app.selected_instrument_owner(), Some(root));
    let entry = crate::instrument_catalog::ENTRIES
        .iter()
        .position(|e| e.type_id == "daw.macro")
        .unwrap();
    let point = app.instrument_catalog_bounds.entries[entry].center();
    super::instrument_catalog::click(&mut app, &ctx, point, false);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert_eq!(
        app.session
            .project
            .beat(track)
            .unwrap()
            .output
            .macro_module
            .controls
            .len(),
        1
    );
    assert_eq!(app.selected_track, Some(track));
    assert!(app.error.is_none(), "{:?}", app.error);
}

#[test]
fn catalog_drag_into_expanded_subtrack_reuses_release_only_component_placement() {
    let (mut app, ctx, _, row, _) = setup();
    let frame = |app: &mut DawUi, ctx: &egui::Context, events| {
        frame_shapes(app, ctx, events, Vec2::new(1600.0, 1600.0));
    };
    app.navigate_beat_owner(app.selected_track.unwrap(), app.beat_ui.primary, false);
    app.view_mode = ViewMode::Instrument;
    app.instrument_catalog_visible = true;
    app.instrument_catalog.expand_folders();
    let mut settings = app
        .session
        .project
        .instrument_settings(row)
        .unwrap()
        .clone();
    settings.main.collapsed = false;
    let layer = settings.layers[0].id;
    app.session
        .project
        .edit(Edit::SetInstrument {
            track_id: row,
            instrument: Some(settings),
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let entry = crate::instrument_catalog::ENTRIES
        .iter()
        .position(|e| e.type_id == "daw.macro")
        .unwrap();
    let origin = app.instrument_catalog_bounds.entries[entry].center();
    let viewport = app
        .instrument_ui
        .component_viewports
        .iter()
        .find(|(owner, _)| *owner == layer)
        .unwrap()
        .1;
    let destination = Pos2::new(viewport.right() - 8.0, viewport.center().y);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(destination)]);
    assert!(
        app.session.project.instrument_settings(row).unwrap().layers[0]
            .macro_module
            .controls
            .is_empty()
    );
    assert!(!app.dirty);
    frame(&mut app, &ctx, vec![button(destination, false)]);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.session.project.instrument_settings(row).unwrap().layers[0]
            .macro_module
            .controls
            .len(),
        1
    );
    assert!(app.error.is_none(), "{:?}", app.error);
}

#[test]
fn empty_beat_parent_has_a_fixed_output_without_source_or_group_rows() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let mut app = DawUi::default();
        let ctx = context();
        let track = app.session.project.add_beat_track().unwrap();
        app.select_track(track, false);
        app.view_mode = ViewMode::Instrument;
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let original = app.session.project.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert_eq!(
            app.selected_instrument_owner(),
            Some(app.session.project.beat(track).unwrap().output.id)
        );
        assert!(app.midi_editor.layer_bounds.is_empty());
        assert!(app.beat_ui.final_output.unwrap().is_positive());
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if matches!(text.galley.text(), "Layers" | "Subtracks" | "LFO" | "Envelope"))));
        assert_eq!(app.session.project, original);
        assert!(!app.dirty);
    }
}

#[test]
fn compact_beat_steps_share_the_lower_rulers_time_mapping() {
    let (mut app, ctx, _, row, _) = setup();
    for zoom in [1.0, 2.0] {
        if zoom > 1.0 {
            app.midi_editor.content.fitted = false;
            app.midi_editor.content.zoom *= zoom;
            frame(&mut app, &ctx, vec![]);
        }
        let ruler = app.midi_editor.content.ruler;
        let options = app.beat_ui.grid_options.unwrap();
        assert!(
            options.bottom() < ruler.top(),
            "pattern options belong in the topbar"
        );
        assert!(options.height() <= toolbars::CONTROL_HEIGHT);
        let first = app.beat_ui.steps[&(row, 0)];
        let next = app.beat_ui.steps[&(row, daw_core::MIDI_TICKS_PER_BEAT / 4)];
        assert!((first.left() - ruler.left() - 3.0).abs() < 0.1);
        assert!((first.top() - ruler.bottom() - 2.0).abs() < 0.1);
        assert!(first.height() <= 28.0, "step lanes must remain thin");
        let clock = app.midi_editor.content.timeline(&app.session.project);
        let expected = clock.pixels_for_frames(i128::from(
            app.session
                .project
                .midi_frame(daw_core::MIDI_TICKS_PER_BEAT / 4),
        )) as f32;
        assert!((next.left() - first.left() + 2.0 - expected).abs() < 0.1);
    }
}

#[test]
fn beat_source_grid_is_independent_of_clip_position_trim_stretch_and_repeat() {
    let (mut app, ctx, _, row, _) = setup();
    let original = app.beat_ui.steps.clone();
    let fit = app.midi_editor.content.fit_zoom;
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.start_frame = 240000;
    clip.source_offset_frame = 12000;
    clip.length_frames = 360000;
    clip.repeat = Some(daw_core::ClipLoop {
        length_frames: 96000,
        phase_frame: 12000,
    });
    clip.stretch = Some(daw_core::ClipStretch::new(1, 2).unwrap());
    app.session.project.validate().unwrap();
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.steps, original);
    assert_eq!(app.midi_editor.content.fit_zoom, fit);
    assert_eq!(app.midi_editor.content.start, 0.0);
    assert_eq!(
        app.beat_ui
            .steps
            .keys()
            .filter(|(id, _)| *id == row)
            .count(),
        16
    );
    assert!(app.beat_ui.repeated_steps.is_empty());
    assert_eq!(app.session.project, before);
}

#[test]
fn compact_beat_controls_follow_track_spacing_and_requested_order() {
    let (mut app, ctx, track, row, _) = setup();
    let [mute, solo, pan, gain] = app.beat_ui.mix_bounds[&row];
    let name = app.beat_ui.headers[&row];
    assert!(name.left() < mute.left() && mute.left() < solo.left());
    assert!(solo.right() < pan.left() && pan.left() < gain.left());
    assert!((solo.left() - mute.right() - 1.0).abs() < 0.1);
    assert!(mute.left() - name.right() >= mix_controls::GROUP_GAP - 0.1);
    let right = app.midi_editor.panel_bounds.right()
        - 7.0
        - (meters::STEREO_WIDTH - meters::SCALE_WIDTH)
        - mix_controls::GROUP_GAP;
    assert!(
        (gain.center().x - (right - knobs::SIZE * 0.5)).abs() < 0.1,
        "mix controls stay pinned immediately before the shared stereo meter"
    );
    for name in [
        "K",
        "A long subtrack name that must truncate before the controls",
    ] {
        app.session
            .project
            .edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Rename {
                    id: row,
                    name: name.into(),
                },
            })
            .unwrap();
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.beat_ui.mix_bounds[&row],
            [mute, solo, pan, gain],
            "name length must not move controls"
        );
    }
    assert!((gain.center().x - pan.center().x - knobs::SIZE - mix_controls::KNOB_GAP).abs() < 0.1);
    assert!((mute.width() - toolbars::CONTROL_HEIGHT).abs() < 0.1);
    assert!((pan.center().y - mute.center().y).abs() < 0.1);
    click(&mut app, &ctx, mute.center());
    assert!(app.session.project.beat(track).unwrap().subtracks[0].muted);
    click(&mut app, &ctx, solo.center());
    assert!(app.session.project.beat(track).unwrap().subtracks[0].soloed);
}

#[test]
fn subtrack_meters_share_track_rails_and_keep_editor_rows_and_instrument_controls_intact() {
    for color_theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            let (mut app, ctx, _, row, _) = setup();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            ctx.set_pixels_per_point(scale);
            let size = Vec2::new(1600.0, 1400.0);
            frame_shapes(&mut app, &ctx, vec![], size);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let name = app.beat_ui.headers[&row];
            let header = Rect::from_min_size(
                Pos2::new(app.midi_editor.panel_bounds.left(), name.center().y - 16.0),
                Vec2::new(TRACK_WIDTH, 32.0),
            );
            let mut rails: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.fill == theme::palette(&ctx).input
                            && rect.rect.width() == 8.0
                            && header.contains_rect(rect.rect) =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .collect();
            rails.sort_by(|a, b| a.left().total_cmp(&b.left()));
            assert_eq!(rails.len(), 2);
            for rail in &rails {
                assert!((rail.top() - header.top() - 2.0).abs() < 0.1);
                assert!((header.bottom() - rail.bottom() - 2.0).abs() < 0.1);
            }
            assert_eq!(header.right() - rails[1].right(), 7.0);
            assert_eq!(rails[1].left() - rails[0].right(), 4.0);
            let gain = app.beat_ui.mix_bounds[&row][3];
            assert!(rails[0].left() - gain.right() >= mix_controls::GROUP_GAP);
            assert!((gain.center().x - (header.right() - 47.0)).abs() < 0.75);
            let before = app.session.project.clone();
            app.dirty = false;
            click(&mut app, &ctx, rails[0].center());
            assert!(!app.beat_dragging());
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);

            app.navigate_beat_owner(app.selected_track.unwrap(), app.beat_ui.primary, false);
            app.view_mode = ViewMode::Instrument;
            frame_shapes(&mut app, &ctx, vec![], size);

            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            super::track_header::assert_header_meter_insets(
                &shapes,
                app.instrument_ui.main_bounds.label,
                &ctx,
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn subtrack_layer_view_keeps_the_real_parent_track_and_local_source_scope() {
    let (mut app, ctx, track, row, _) = setup();
    let other = BeatSubtrack::new("Other".into());
    let other_id = other.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(other),
                index: 1,
            },
        })
        .unwrap();
    let original = app.session.project.clone();
    app.view_mode = ViewMode::Instrument;
    for owner in [row, other_id, row] {
        app.navigate_beat_owner(track, Some(owner), false);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.selected_track, Some(track));
        assert_eq!(app.instrument_ui.track, Some(owner));
        let layer = app
            .session
            .project
            .instrument_settings(owner)
            .unwrap()
            .layers[0]
            .id;
        assert_eq!(app.midi_editor.layer_bounds.len(), 1);
        assert!(app.midi_editor.layer_bounds.contains_key(&layer));
        assert_eq!(app.session.project, original);
    }
}

#[test]
fn empty_beat_track_has_no_placeholder_pattern_and_keeps_mix_controls() {
    let (mut app, ctx, track, row, _) = setup();
    app.session.project.tracks[0].clips.clear();
    app.selected_clip = None;
    app.selected_clips.clear();
    frame(&mut app, &ctx, vec![]);
    assert!(app.beat_ui.steps.is_empty());
    assert!(app.beat_ui.disabled_steps.is_empty());
    let mute = app.beat_ui.mix_bounds[&row][0].center();
    click(&mut app, &ctx, mute);
    assert!(app.session.project.beat(track).unwrap().subtracks[0].muted);
}

#[test]
fn selected_subtrack_adds_layers_through_the_standard_layers_plus() {
    let (mut app, ctx, track, row, _) = setup();
    app.navigate_beat_owner(track, Some(row), false);
    app.view_mode = ViewMode::Instrument;
    frame(&mut app, &ctx, vec![]);
    let plus = app.midi_editor.add_layer_bounds;
    assert!(app.midi_editor.instrument_header_bounds.contains_rect(plus));
    click(&mut app, &ctx, plus.center());
    assert_eq!(
        app.session
            .project
            .instrument_settings(row)
            .unwrap()
            .layers
            .len(),
        2
    );
    assert_eq!(app.session.project.beat(track).unwrap().subtracks.len(), 1);
    assert_eq!(app.selected_track, Some(track));
    assert_eq!(app.selected_instrument_owner(), Some(row));
    assert!(app.error.is_none());
}

#[test]
fn escape_deselects_subtracks_in_both_views_without_editing_the_project() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, first, clip) = setup();
        let second = BeatSubtrack::new("Snare".into());
        let second_id = second.id;
        app.session
            .project
            .edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(second),
                    index: 1,
                },
            })
            .unwrap();
        app.view_mode = view;
        if view == ViewMode::Instrument {
            app.track_browser.visible = true;
        }
        frame(&mut app, &ctx, vec![]);
        let a = app.beat_ui.headers[&first].center();
        let b = app.beat_ui.headers[&second_id].center();
        super::instrument_catalog::click(&mut app, &ctx, a, false);
        super::instrument_catalog::click(&mut app, &ctx, b, true);
        assert!(app.beat_subtrack_selected(first) && app.beat_subtrack_selected(second_id));
        let before = app.session.project.clone();
        app.dirty = false;
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        for _ in 0..2 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(
            !app.beat_subtrack_selected(first) && !app.beat_subtrack_selected(second_id),
            "{view:?}"
        );
        assert_eq!(app.session.project, before);
        assert_eq!(app.selected_track, Some(track));
        assert_eq!(app.selected_clip, Some(clip));
        assert!(!app.dirty);
        if view == ViewMode::Edit {
            super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
            assert!(app.selected_clip.is_none());
            assert!(app.selected_clips.is_empty());
            assert_eq!(app.selected_track, Some(track));
            assert_eq!(app.session.project, before);
        }
    }
}

#[test]
fn escape_cancels_a_subtrack_rename_without_clearing_its_selection() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, _, row, _) = setup();
        app.view_mode = view;
        if view == ViewMode::Instrument {
            app.track_browser.visible = true;
        }
        frame(&mut app, &ctx, vec![]);
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        click(&mut app, &ctx, name);
        assert!(
            ctx.text_edit_focused(),
            "{view:?}: focus {:?}, rename {}, drag {:?}",
            ctx.memory(|m| m.focused()),
            app.beat_renaming(row),
            app.track_drag
        );
        let before = app.session.project.clone();
        frame(&mut app, &ctx, vec![egui::Event::Text("draft".into())]);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        assert!(app.beat_subtrack_selected(row));
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn editor_blank_subtrack_header_double_click_renames_without_changing_mix() {
    let (mut app, ctx, _, row, _) = setup();
    let before = app.session.project.clone();
    let name = app.beat_ui.headers[&row];
    let mute = app.beat_ui.mix_bounds[&row][0];
    let blank = egui::pos2((name.right() + mute.left()) / 2.0, name.center().y);
    assert!(blank.x > name.right() && blank.x < mute.left());
    click(&mut app, &ctx, blank);
    assert!(!app.beat_renaming(row));
    click(&mut app, &ctx, blank);
    assert!(ctx.text_edit_focused());
    assert!(app.beat_renaming(row));
    assert_eq!(app.session.project, before);
    frame(&mut app, &ctx, vec![egui::Event::Text("Snare".into())]);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
    let mut expected = before;
    expected
        .edit(Edit::Beat {
            track_id: app.selected_track.unwrap(),
            edit: BeatEdit::Rename {
                id: row,
                name: "Snare".into(),
            },
        })
        .unwrap();
    assert_eq!(app.session.project, expected);
    for control in app.beat_ui.mix_bounds[&row] {
        click(&mut app, &ctx, control.center());
        click(&mut app, &ctx, control.center());
        assert!(!app.beat_renaming(row));
    }
}

#[test]
fn editor_subtrack_selection_uses_background_without_outline() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _, row, _) = setup();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let step = app.beat_ui.steps[&(row, 0)];
        let rect = Rect::from_min_size(
            Pos2::new(app.midi_editor.panel_bounds.left(), step.top() - 2.0),
            Vec2::new(TRACK_WIDTH, 32.0),
        );
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx))));
        assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.fill == theme::selected_track_header_background(&ctx))));
    }
}

#[test]
fn escape_deselects_an_instrument_subtrack_in_the_tracks_tree() {
    let (mut app, ctx, _, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    app.track_browser.visible = true;
    frame(&mut app, &ctx, vec![]);
    let name = app.beat_ui.headers[&row].center();
    click(&mut app, &ctx, name);
    assert!(app.beat_subtrack_selected(row));
    let before = app.session.project.clone();
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!app.beat_subtrack_selected(row));
    assert_eq!(app.session.project, before);
}

#[test]
fn instrument_escape_deselects_from_child_controls_and_shares_catalog_handling() {
    let (mut app, ctx, _, row, _) = setup();
    app.navigate_beat_owner(app.selected_track.unwrap(), app.beat_ui.primary, false);
    app.view_mode = ViewMode::Instrument;
    app.instrument_catalog_visible = true;
    app.instrument_catalog.expand_folders();
    frame(&mut app, &ctx, vec![]);
    app.navigate_beat_owner(app.selected_track.unwrap(), Some(row), false);
    let layer = app.session.project.instrument_settings(row).unwrap().layers[0].id;
    let mute = app.midi_editor.layer_bounds[&layer].mix_controls[0].center();
    click(&mut app, &ctx, mute);
    assert!(app.beat_subtrack_selected(row));
    let before = app.session.project.clone();
    app.dirty = false;
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!app.beat_subtrack_selected(row));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);

    app.navigate_beat_owner(app.selected_track.unwrap(), Some(row), false);
    let entry = app.instrument_catalog_bounds.entries[0].center();
    click(&mut app, &ctx, entry);
    assert!(app.beat_subtrack_selected(row));
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!app.beat_subtrack_selected(row));
    assert!(
        app.instrument_catalog.selection.is_empty(),
        "the catalog still handles its own Escape"
    );
    assert_eq!(app.session.project, before);
}

#[test]
fn editor_header_background_selects_subtracks_once_and_keeps_steps_separate() {
    let (mut app, ctx, track, first, _) = setup();
    let second = BeatSubtrack::new("Snare".into());
    let second_id = second.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(second),
                index: 1,
            },
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    let blank = |app: &DawUi, row| {
        Pos2::new(
            app.midi_editor.content.ruler.left() - TRACK_WIDTH + 2.0,
            app.beat_ui.headers[&row].center().y - 14.0,
        )
    };
    let before = app.session.project.clone();
    app.dirty = false;
    let first_blank = blank(&app, first);
    click(&mut app, &ctx, first_blank);
    assert!(app.beat_subtrack_selected(first));
    assert!(!app.beat_subtrack_selected(second_id));
    let right_padding = Pos2::new(app.midi_editor.content.ruler.left() - 2.0, first_blank.y);
    click(&mut app, &ctx, right_padding);
    assert_eq!(
        app.session.project, before,
        "header padding must not activate a neighboring step"
    );
    let second_blank = blank(&app, second_id);
    super::instrument_catalog::click(&mut app, &ctx, second_blank, true);
    assert!(app.beat_subtrack_selected(first) && app.beat_subtrack_selected(second_id));
    super::instrument_catalog::click(&mut app, &ctx, second_blank, true);
    assert!(app.beat_subtrack_selected(first) && !app.beat_subtrack_selected(second_id));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!app.beat_subtrack_selected(first) && !app.beat_subtrack_selected(second_id));
    let step = app.beat_ui.steps[&(second_id, 0)].center();
    click(&mut app, &ctx, step);
    assert!(!app.beat_subtrack_selected(first) && !app.beat_subtrack_selected(second_id));
}

#[test]
fn editor_header_controls_select_the_subtrack_and_keep_their_actions() {
    let (mut app, ctx, track, first, clip) = setup();
    let second = BeatSubtrack::new("Snare".into());
    let second_id = second.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(second),
                index: 1,
            },
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    for control in 0..4 {
        let name = app.beat_ui.headers[&first].center();
        super::instrument_catalog::click(&mut app, &ctx, name, false);
        let point = app.beat_ui.mix_bounds[&second_id][control].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert!(
            app.beat_subtrack_selected(second_id),
            "control {control} selects on press"
        );
        assert!(!app.beat_subtrack_selected(first));
        let end = if control >= 2 {
            point - Vec2::new(0.0, 20.0)
        } else {
            point
        };
        if control >= 2 {
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        frame(&mut app, &ctx, vec![]);
        let row = &app.session.project.beat(track).unwrap().subtracks[1];
        match control {
            0 => assert!(row.muted),
            1 => assert!(row.soloed),
            2 => assert!(row.instrument.main.pan > 0.0),
            3 => assert!(row.instrument.main.gain_db > 0.0),
            _ => unreachable!(),
        }
        let before = app.session.project.clone();
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        assert!(
            !app.beat_subtrack_selected(second_id),
            "Escape after control {control}"
        );
        assert_eq!(app.session.project, before);
        assert_eq!(app.selected_clip, Some(clip));
    }
}

#[test]
fn changing_subtracks_replaces_hidden_layer_selection_without_editing_the_project() {
    let (mut app, ctx, track, first, _) = setup();
    let second = BeatSubtrack::new("Other".into());
    let second_id = second.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(second),
                index: 1,
            },
        })
        .unwrap();
    let original = app.session.project.clone();
    app.view_mode = ViewMode::Instrument;
    app.navigate_beat_owner(track, Some(first), false);
    frame(&mut app, &ctx, vec![]);
    let first_layer = app
        .session
        .project
        .instrument_settings(first)
        .unwrap()
        .layers[0]
        .id;
    let point = app.midi_editor.layer_bounds[&first_layer].name.center();
    click(&mut app, &ctx, point);
    app.navigate_beat_owner(track, Some(second_id), false);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.cached_beat_layer_selected(first, first_layer));
    assert!(!app.midi_editor.layer_bounds.contains_key(&first_layer));
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
}

#[test]
fn editor_subtrack_headers_live_in_the_left_panel_and_share_the_instrument_plus() {
    let (mut app, ctx, track, row, clip) = setup();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let panel = app.midi_editor.panel_bounds;
    let ruler = app.midi_editor.content.ruler;
    assert!(
        (ruler.left() - panel.right() - track_overview::LABEL_WIDTH).abs() < 0.1,
        "the step ruler retains the shared editor gutter"
    );
    assert!(panel.contains_rect(app.beat_ui.headers[&row]));
    assert!(
        app.beat_ui.mix_bounds[&row]
            .iter()
            .all(|r| panel.contains_rect(*r))
    );
    assert!((app.beat_ui.steps[&(row, 0)].left() - ruler.left() - 3.0).abs() < 0.1);
    assert!(!shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Add beat subtrack")
    ));
    let header = app.midi_editor.instrument_header_bounds;
    let plus = app.midi_editor.add_layer_bounds;
    assert!(header.contains_rect(plus));
    assert!(panel.contains_rect(plus));
    let inset = header.right() - plus.right();
    assert!(inset <= 8.0, "Plus is pinned at the header's right inset");
    click(&mut app, &ctx, plus.center());
    assert_eq!(app.session.project.beat(track).unwrap().subtracks.len(), 2);
    let added = app.session.project.beat(track).unwrap().subtracks[1].id;
    assert!(
        app.midi_editor
            .panel_bounds
            .contains_rect(app.beat_ui.headers[&added])
    );
    assert_eq!(app.selected_clip, Some(clip));
    app.navigate_beat_owner(track, Some(added), false);
    app.view_mode = ViewMode::Instrument;
    frame(&mut app, &ctx, vec![]);
    let instrument_plus = app.midi_editor.add_layer_bounds;
    assert_eq!(instrument_plus.size(), plus.size());
    assert!(
        (app.midi_editor.instrument_header_bounds.right() - instrument_plus.right() - inset).abs()
            < 0.1
    );
}

#[test]
fn editor_left_headers_and_steps_scroll_together_without_project_changes() {
    let (mut app, ctx, track, first, _) = setup();
    for index in 0..18 {
        app.session
            .project
            .edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(BeatSubtrack::new(format!("Row {index}"))),
                    index: index + 1,
                },
            })
            .unwrap();
    }
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    app.dirty = false;
    let top = app.beat_ui.headers[&first].top();
    let pointer = Pos2::new(
        app.midi_editor.panel_bounds.left() + 2.0,
        app.midi_editor.content.ruler.bottom() + 20.0,
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: Vec2::new(0.0, -150.0),
                modifiers: Default::default(),
            },
        ],
    );
    for _ in 0..6 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(
        app.beat_ui
            .headers
            .get(&first)
            .is_none_or(|header| header.top() < top),
        "scrolling over the left panel scrolls the shared rows"
    );
    let mut visible = 0;
    for (row, header) in &app.beat_ui.headers {
        if let Some(step) = app.beat_ui.steps.get(&(*row, 0))
            && step.bottom() > app.midi_editor.content.ruler.bottom()
            && step.top() < app.midi_editor.panel_bounds.bottom()
        {
            visible += 1;
            assert!((header.center().y - step.center().y).abs() < 0.1);
            assert!((header.center().y - app.beat_ui.audition_steps[row].center().y).abs() < 0.1);
            assert!(header.left() < app.midi_editor.panel_bounds.right());
            assert!(step.left() >= app.midi_editor.panel_bounds.right());
        }
    }
    assert!(visible > 0);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn selected_beat_pattern_has_no_cells_beyond_the_source_and_no_phantom_edits() {
    let (mut app, ctx, _, row, clip) = setup();
    assert!(app.beat_ui.disabled_steps.is_empty());
    assert_eq!(app.beat_ui.steps.len(), 16);
    let before = app.session.project.clone();
    let ruler = app.midi_editor.content.ruler;
    let point = Pos2::new(
        ruler.right() - 0.25,
        app.beat_ui.steps[&(row, 0)].center().y + 32.0,
    );
    click(&mut app, &ctx, point);
    assert_eq!(app.session.project, before);
    assert_eq!(app.selected_clip, Some(clip));
}

#[test]
fn beat_editor_shows_only_the_selected_source_and_preserves_other_clips_when_editing() {
    let (mut app, ctx, track, row, clip) = setup();
    let other = app.session.project.add_beat_pattern(track, 192000).unwrap();
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: other,
            pattern: BeatPattern::default()
                .with_grid(1, daw_core::BeatResolution::Eighth)
                .unwrap(),
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.clip_steps.len(), 16);
    assert!(app.beat_ui.clip_steps.keys().all(|(id, _, _)| *id == clip));
    let builds = app.beat_ui.step_index.builds;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.step_index.builds, builds);
    app.select_clip(other, track, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.clip_steps.len(), 8);
    assert!(app.beat_ui.clip_steps.keys().all(|(id, _, _)| *id == other));
    let before = app.session.project.clone();
    let point = app.beat_ui.clip_steps[&(other, row, 0)].center();
    click(&mut app, &ctx, point);
    assert_eq!(
        app.session.project.tracks[0].clips[0],
        before.tracks[0].clips[0]
    );
    assert!(app.dirty);
}

#[test]
fn beat_editor_shared_audition_button_previews_only_enabled_steps() {
    let (mut app, ctx, track, row, _) = setup();
    let before = app.session.project.clone();
    let audition = app.midi_editor.audition_bounds;
    let panel = app.midi_editor.panel_bounds;
    let ruler = app.midi_editor.content.ruler;
    assert!(audition.left() >= panel.right());
    assert!(audition.right() <= ruler.left());
    assert_eq!(audition.size(), Vec2::splat(16.0));
    click(&mut app, &ctx, audition.center());
    assert!(app.midi_editor.audition_enabled);
    assert_eq!(app.session.project, before);
    let step = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, step);
    assert_eq!(app.chords.audition.beat_requests, vec![(track, row)]);
    click(&mut app, &ctx, step);
    assert_eq!(
        app.chords.audition.beat_requests.len(),
        1,
        "removing a step is silent"
    );
    click(&mut app, &ctx, audition.center());
    assert!(!app.midi_editor.audition_enabled);
    assert!(!app.chords.audition.scoped());
    click(&mut app, &ctx, step);
    assert_eq!(app.chords.audition.beat_requests.len(), 1);
    app.selected_clip = None;
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, audition.center());
    assert!(app.beat_ui.steps.is_empty());
    assert_eq!(app.chords.audition.beat_requests.len(), 1);
}

#[test]
fn beat_editor_scrollbar_pans_only_the_step_viewport_without_edits() {
    let (mut app, ctx, track, row, clip) = setup();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom *= 4.0;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let before = app.session.project.clone();
    let overview = (app.zoom, app.scroll);
    let header = app.beat_ui.headers[&row];
    let bounds = app.midi_editor.content.scrollbar_bounds;
    assert_eq!(bounds.height(), SCROLLBAR_HEIGHT);
    assert!((bounds.left() - app.midi_editor.content.ruler.left()).abs() < 0.1);
    let start = Pos2::new(bounds.left() + 8.0, bounds.bottom() - 4.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
    frame(&mut app, &ctx, vec![button(start, true)]);
    let end = start + Vec2::new(80.0, 0.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.midi_editor.content.scroll > 0.0,
        "dragging the shared thumb pans beat time"
    );
    assert_eq!((app.zoom, app.scroll), overview);
    assert_eq!(app.beat_ui.headers[&row], header);
    assert!((app.midi_editor.content.left - app.midi_editor.content.ruler.left()).abs() < 0.1);
    assert_eq!(app.selected_track, Some(track));
    assert_eq!(app.selected_clip, Some(clip));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    app.selected_clip = None;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.midi_editor.content.scrollbar_bounds, bounds);
    assert!(app.beat_ui.steps.is_empty());
    assert!(app.beat_ui.disabled_steps.is_empty());
}

#[test]
fn subtrack_name_input_selects_the_whole_name_and_matches_track_commit_and_cancel() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, row, _) = setup();
        app.view_mode = view;
        if view == ViewMode::Instrument {
            app.track_browser.visible = true;
        }
        frame(&mut app, &ctx, vec![]);
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        click(&mut app, &ctx, name);
        assert!(
            ctx.text_edit_focused(),
            "{view:?}: focus {:?}, rename {}, drag {:?}",
            ctx.memory(|m| m.focused()),
            app.beat_renaming(row),
            app.track_drag
        );
        frame(&mut app, &ctx, vec![egui::Event::Text("Bombo 🎵".into())]);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
        assert_eq!(
            app.session.project.beat(track).unwrap().subtracks[0].name,
            "Bombo 🎵"
        );
        app.dirty = false;
        frame(&mut app, &ctx, vec![]);
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        click(&mut app, &ctx, name);
        frame(&mut app, &ctx, vec![egui::Event::Text("Discard".into())]);
        let before = app.session.project.clone();
        ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
        frame(&mut app, &ctx, vec![]);
        assert!(!ctx.text_edit_focused());
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
}

#[test]
fn beat_body_edges_extend_in_timeline_and_editor_with_release_only_commit_and_escape() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        for edge in [daw_core::ClipEdge::Left, daw_core::ClipEdge::Right] {
            for cancel in [false, true] {
                let (mut app, ctx, track, row, clip) = setup();
                app.session.project.tracks[0].clips[0].start_frame = 96_000;
                app.view_mode = view;
                app.zoom = 120.0;
                app.scroll = 0.0;
                app.midi_editor.zoom = 120.0;
                app.midi_editor.scroll = 0.0;
                app.history = None;
                app.ensure_history();
                app.dirty = false;
                frame(&mut app, &ctx, vec![]);
                let original = app.session.project.clone();
                let current = &original.tracks[0].clips[0];
                let block = if view == ViewMode::Edit {
                    app.midi_editor.clip_bounds[&clip]
                } else {
                    app.clip_block(app.lane_bounds[&track], current, current.start_frame)
                };
                let origin = Pos2::new(
                    if edge == daw_core::ClipEdge::Left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    block.top() + 30.0,
                );
                let end = origin
                    + Vec2::new(
                        if edge == daw_core::ClipEdge::Left {
                            -60.0
                        } else {
                            60.0
                        },
                        0.0,
                    );
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(origin), button(origin, true)],
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(
                    app.session.project, original,
                    "drag preview cannot edit the project"
                );
                assert!(app.drag.is_some(), "body edge starts a resize in {view:?}");
                if cancel {
                    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
                }
                frame(&mut app, &ctx, vec![button(end, false)]);
                frame(&mut app, &ctx, vec![]);
                assert!(app.error.is_none(), "{:?}", app.error);
                if cancel {
                    assert_eq!(app.session.project, original);
                    assert!(!app.dirty);
                } else {
                    let resized = &app.session.project.tracks[0].clips[0];
                    assert_eq!(resized.length_frames, current.length_frames + 24_000);
                    assert_ne!(resized.asset_id, current.asset_id);
                    match edge {
                        daw_core::ClipEdge::Left => assert_eq!(resized.end(), current.end()),
                        daw_core::ClipEdge::Right => {
                            assert_eq!(resized.start_frame, current.start_frame)
                        }
                    }
                    assert!(app.dirty);
                    let extended = app.session.project.clone();
                    app.perform_history_action(crate::history::HistoryAction::Undo, &ctx);
                    assert_eq!(app.session.project, original);
                    app.perform_history_action(crate::history::HistoryAction::Redo, &ctx);
                    assert_eq!(app.session.project, extended);
                    if view == ViewMode::Edit && edge == daw_core::ClipEdge::Right {
                        frame(&mut app, &ctx, vec![]);
                        let point =
                            app.beat_ui.steps[&(row, 4 * daw_core::MIDI_TICKS_PER_BEAT)].center();
                        click(&mut app, &ctx, point);
                        let asset = app.session.project.tracks[0].clips[0].asset_id;
                        let pattern = &app
                            .session
                            .project
                            .assets
                            .iter()
                            .find(|a| a.id == asset)
                            .unwrap()
                            .beat_pattern
                            .as_ref()
                            .unwrap()
                            .pattern;
                        assert!(
                            pattern
                                .steps
                                .iter()
                                .any(|step| step.start_tick == 4 * daw_core::MIDI_TICKS_PER_BEAT)
                        );
                    }
                }
                assert_eq!(app.selected_clip, Some(clip));
                assert_eq!(app.selected_track, Some(track));
            }
        }
    }
}

#[test]
fn beat_length_monitor_follows_trim_stretch_extension_and_loop_without_editing_the_source() {
    fn shown(app: &mut DawUi, ctx: &egui::Context) -> String {
        let before = app.session.project.clone();
        let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
        let bounds = app.beat_ui.grid_options.unwrap();
        let value = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if bounds.contains(t.pos) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(
            app.session.project, before,
            "drawing derived values cannot edit the project"
        );
        value
    }
    let (mut app, ctx, _, _, _) = setup();
    assert_eq!(shown(&mut app, &ctx), "0001.00");
    let mut clip = app.session.project.tracks[0].clips[0].clone();
    clip.length_frames /= 2;
    app.session.project = app
        .session
        .project
        .trimmed_clips(&[clip], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "0000.02");
    let clip = app.session.project.tracks[0].clips[0].clone();
    let stretched = clip
        .stretched_to(clip.start_frame, clip.length_frames * 2)
        .unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[stretched], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "0001.00");
    let mut clip = app.session.project.tracks[0].clips[0].clone();
    clip.length_frames += 24_000;
    app.session.project = app
        .session
        .project
        .trimmed_clips(&[clip], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "0001.01");
    let clip = app.session.project.tracks[0].clips[0].clone();
    let looped = clip.looped_by(daw_core::ClipEdge::Right, 24_000);
    app.session.project = app
        .session
        .project
        .resized_clips(&[looped], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "0001.02");
}

#[test]
fn beat_length_monitor_reuses_transport_drag_sections_and_commits_on_release() {
    for cancellation in ["none", "escape", "focus"] {
        let (mut app, ctx, _, _, _) = setup();
        app.dirty = false;
        let before = app.session.project.clone();
        let bounds = app.beat_ui.grid_options.unwrap();
        let origin = Pos2::new(bounds.right() - 8.0, bounds.center().y);
        let end = origin - Vec2::new(0.0, 2.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(origin), button(origin, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.session.project, before, "{cancellation}");
        if cancellation == "escape" {
            super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        } else if cancellation == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused: false,
                    events: vec![egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        if cancellation != "none" {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end - Vec2::new(0.0, 3.0))],
            );
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        frame(&mut app, &ctx, vec![]);
        assert!(app.error.is_none(), "{:?}", app.error);
        if cancellation != "none" {
            assert_eq!(app.session.project, before, "{cancellation}");
            assert!(!app.dirty);
        } else {
            assert_eq!(
                app.session.project.tracks[0].clips[0].length_frames,
                144_000
            );
            assert!(app.dirty);
        }
    }
}

#[test]
fn subdivision_measures_source_step_duration_and_ignores_clip_mapping() {
    fn shown(app: &mut DawUi, ctx: &egui::Context) -> String {
        let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
        let bounds = app.beat_ui.subdivision_bounds.unwrap();
        shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if bounds.contains(t.pos) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }
    let (mut app, ctx, _, row, id) = setup();
    assert_eq!(shown(&mut app, &ctx), "1/16");
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: id,
            pattern: BeatPattern::default()
                .with_grid(2, daw_core::BeatResolution::Eighth)
                .unwrap(),
        })
        .unwrap();
    let clip = app.session.project.tracks[0].clips[0].clone();
    let compressed = clip.stretched_to(clip.start_frame, 96_000).unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[compressed], daw_core::ClipEdge::Right)
        .unwrap();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom = 480.0;
    assert_eq!(shown(&mut app, &ctx), "1/8");
    let measured_beats = |app: &DawUi| {
        let a = app.beat_ui.steps[&(row, 0)];
        let b = app.beat_ui.steps[&(row, 960)];
        f64::from(b.left() - a.left()) / f64::from(app.midi_editor.content.zoom) * 2.0
    };
    assert!((measured_beats(&app) - 1.0).abs() < 1e-6);
    let clip = app.session.project.tracks[0].clips[0].clone();
    let stretched = clip.stretched_to(clip.start_frame, 192_000).unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[stretched], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "1/8");
    assert!((measured_beats(&app) - 1.0).abs() < 1e-6);
    let mut clip = app.session.project.tracks[0].clips[0].clone();
    clip.length_frames = 96_000;
    app.session.project = app
        .session
        .project
        .trimmed_clips(&[clip], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "1/8");
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    let pattern = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset)
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap()
        .pattern
        .clone();
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: id,
            pattern: pattern.with_grid(3, pattern.resolution).unwrap(),
        })
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "1/8");
    let clip = app.session.project.tracks[0].clips[0].clone();
    let stretched = clip.stretched_to(clip.start_frame, 144_000).unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[stretched], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "1/8");
    assert!((measured_beats(&app) - 1.0).abs() < 1e-6);
    assert!(app.beat_ui.disabled_steps.is_empty());
}

#[test]
fn subdivision_choices_show_source_bar_fractions_and_edit_the_source_grid_once() {
    let (mut app, ctx, _, row, id) = setup();
    let pattern = BeatPattern::default()
        .with_grid(1, daw_core::BeatResolution::ThirtySecond)
        .unwrap()
        .with_step(row, 120, Some(100))
        .unwrap()
        .with_grid(1, daw_core::BeatResolution::Sixteenth)
        .unwrap();
    let steps = pattern.steps.clone();
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: id,
            pattern,
        })
        .unwrap();
    let clip = app.session.project.tracks[0].clips[0].clone();
    let stretched = clip
        .stretched_to(clip.start_frame, clip.length_frames * 2)
        .unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[stretched], daw_core::ClipEdge::Right)
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    let bounds = app.beat_ui.subdivision_bounds.unwrap();
    click(&mut app, &ctx, bounds.center());
    assert_eq!(app.beat_ui.subdivision_items.len(), 4);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for ((_, bounds), expected) in app
        .beat_ui
        .subdivision_items
        .iter()
        .zip(["1/4", "1/8", "1/16", "1/32"])
    {
        assert!(
            shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Text(t) if bounds.contains(t.pos) && t.galley.text() == expected)),
            "source popup choice must show {expected} of one bar"
        );
    }
    let choice = app
        .beat_ui
        .subdivision_items
        .iter()
        .find(|(r, _)| *r == daw_core::BeatResolution::ThirtySecond)
        .unwrap()
        .1;
    click(&mut app, &ctx, choice.center());
    let clip = &app.session.project.tracks[0].clips[0];
    let source = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == clip.asset_id)
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap();
    assert_eq!(
        source.pattern.resolution,
        daw_core::BeatResolution::ThirtySecond
    );
    assert_eq!(source.pattern.steps, steps);
    assert_eq!(clip.stretch, daw_core::ClipStretch::new(1, 2));
    assert!(app.beat_ui.steps.contains_key(&(row, 120)));
}

#[test]
fn subtrack_preview_step_plays_the_clicked_instrument_without_editing_or_selecting_it() {
    let (mut app, ctx, track, first, clip) = setup();
    let mut other = BeatSubtrack::new("Snare".into());
    other.instrument = OscillatorSettings::default().into();
    let second = other.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(other),
                index: 1,
            },
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    let name = app.beat_ui.headers[&first].center();
    click(&mut app, &ctx, name);
    app.dirty = false;
    let before = app.session.project.clone();
    let revision = app.change_revision;
    let primary = app.beat_ui.primary;
    let preview = app.beat_ui.audition_steps[&second];
    click(&mut app, &ctx, preview.center());
    assert!(!app.midi_editor.audition_enabled);
    assert_eq!(app.chords.audition.beat_requests, vec![(track, second)]);
    assert!(
        app.chords.audition.scoped(),
        "direct preview survives the disabled automatic toggle"
    );
    assert_eq!(app.session.project, before);
    assert_eq!(app.change_revision, revision);
    assert_eq!(app.beat_ui.primary, primary);
    assert!(app.beat_subtrack_selected(first));
    assert!(!app.beat_subtrack_selected(second));
    assert_eq!(app.selected_clip, Some(clip));
    assert_eq!(app.selected_track, Some(track));
    assert!(!app.dirty);
}

#[test]
fn subtrack_preview_step_works_without_selected_or_existing_patterns_and_respects_dialogs() {
    for keep_patterns in [false, true] {
        let (mut app, ctx, track, row, _) = setup();
        app.selected_clip = None;
        if !keep_patterns {
            app.session.project.tracks[0].clips.clear();
        }
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let before = app.session.project.clone();
        let preview = app.beat_ui.audition_steps[&row];
        click(&mut app, &ctx, preview.center());
        assert_eq!(app.chords.audition.beat_requests, vec![(track, row)]);
        assert!(app.chords.audition.scoped());
        assert_eq!(app.selected_clip, None);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.beat_ui.disabled_steps.is_empty());
        app.settings_open = true;
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, preview.center());
        assert_eq!(app.chords.audition.beat_requests.len(), 1);
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn preview_steps_share_compact_tile_geometry_with_a_fixed_grid_separator() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, parent, row, _) = setup();
        let mut rows = vec![row];
        for index in 1..3 {
            let child = BeatSubtrack::new(format!("Subtrack {index}"));
            rows.push(child.id);
            app.session
                .project
                .edit(Edit::Beat {
                    track_id: parent,
                    edit: BeatEdit::Add {
                        subtrack: Box::new(child),
                        index,
                    },
                })
                .unwrap();
        }
        app.navigate_beat_owner(parent, Some(rows[1]), false);
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let preview = app.beat_ui.audition_steps[&row];
        let ruler = app.midi_editor.content.ruler;
        let first = app.beat_ui.steps[&(row, 0)];
        let next = app.beat_ui.steps[&(row, 240)];
        assert!((preview.left() - app.midi_editor.panel_bounds.right() - 18.0).abs() < 0.1);
        assert!((ruler.left() - preview.right() - 18.0).abs() < 0.1);
        assert_eq!(preview.width(), 16.0);
        assert_eq!(preview.height(), first.height());
        assert!(
            !shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Text(t) if preview.contains(t.pos) && t.galley.text() == "·")),
            "audition button has no visible dot"
        );
        assert!((preview.center().y - app.beat_ui.headers[&row].center().y).abs() < 0.1);
        assert!((first.left() - ruler.left() - 3.0).abs() < 0.1);
        assert!((next.left() - first.right() - 2.0).abs() < 0.1);
        assert!(
            shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::LineSegment { points, stroke }
                if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                    && (points[0].x - ruler.left() - 0.5).abs() < 0.1
                    && (points[1].x - points[0].x).abs() < 0.1
                    && (points[0].y - ruler.bottom()).abs() < 0.1
                    && (points[1].y - app.midi_editor.content.scrollbar_bounds.top()).abs() < 0.1
            )),
            "one shared-style divider spans the grid viewport"
        );
        assert_eq!(
            shapes
                .iter()
                .filter(|shape| shape.clip_rect.contains(Pos2::new(
                    app.midi_editor.panel_bounds.center().x,
                    ruler.bottom() - 0.5
                )) && matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                            && (points[0].x - app.midi_editor.panel_bounds.left()).abs() < 0.1
                            && (points[1].x - app.midi_editor.panel_bounds.right()).abs() < 0.1
                            && (points[0].y - (ruler.bottom() - 0.5)).abs() < 0.1
                            && (points[1].y - points[0].y).abs() < 0.1
                ))
                .count(),
            1,
            "ruler separator extends across headers to the left toolbar"
        );
        let selected_header = app.beat_ui.header_panels[&rows[1]];
        let selected_fill = shapes.iter().position(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.rect == selected_header && rect.fill == theme::selected_track_header_background(&ctx)
        )).expect("selected subtrack header background");
        let right_probe = Pos2::new(selected_header.right() - 0.5, selected_header.center().y);
        let divider = shapes.iter().enumerate().rfind(|(_, shape)| shape.clip_rect.contains(right_probe) && matches!(&shape.shape,
            egui::Shape::LineSegment { points, stroke }
                if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                    && (points[0].x - right_probe.x).abs() < 0.1
                    && (points[1].x - right_probe.x).abs() < 0.1
                    && points[0].y <= selected_header.top() && points[1].y >= selected_header.bottom()
        )).expect("continuous visible header right divider").0;
        assert!(
            divider > selected_fill,
            "right divider must paint after selected backgrounds"
        );
        for (index, id) in rows.iter().enumerate() {
            let header = app.beat_ui.header_panels[id];
            let name = &app
                .session
                .project
                .beat(parent)
                .unwrap()
                .subtracks
                .iter()
                .find(|row| row.id == *id)
                .unwrap()
                .name;
            let label_left = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == name && header.contains(text.pos) =>
                    {
                        Some(text.pos.x + text.galley.rect.left())
                    }
                    _ => None,
                })
                .expect("subtrack header label");
            assert!(
                (label_left - header.left() - 7.0).abs() < 0.1,
                "subtrack labels share the standard left inset"
            );
            let y = if index + 1 == rows.len() {
                header.bottom() - 0.5
            } else {
                app.beat_ui.header_panels[&rows[index + 1]].top() + 0.5
            };
            let dividers = shapes
                .iter()
                .filter(|shape| {
                    matches!(&shape.shape,
                        egui::Shape::LineSegment { points, stroke }
                            if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                && (points[0].x - header.left()).abs() < 0.1
                                && points[1].x >= ruler.right()
                                && (points[0].y - y).abs() < 0.1
                                && (points[1].y - y).abs() < 0.1
                    )
                })
                .count();
            assert_eq!(dividers, 1, "one row divider must span the header and grid");
        }
        app.midi_editor.content.fitted = false;
        app.midi_editor.content.scroll = 0.5;
        app.midi_editor.content.zoom *= 2.0;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.beat_ui.audition_steps[&row], preview,
            "audition stays in the fixed gutter"
        );
    }
}

#[test]
fn editor_subtrack_selection_replaces_final_output_target() {
    for target in ["name", "control", "blank"] {
        let (mut app, ctx, track, row, _) = setup();
        app.view_mode = ViewMode::Instrument;
        frame(&mut app, &ctx, vec![]);
        let output = app.session.project.beat(track).unwrap().output.id;
        let point = app.beat_ui.final_output.unwrap().left_top() + egui::vec2(100.0, 25.0);
        click(&mut app, &ctx, point);
        assert_eq!(app.selected_instrument_owner(), Some(output));
        app.view_mode = ViewMode::Edit;
        frame(&mut app, &ctx, vec![]);
        let header = app.beat_ui.headers[&row];
        let point = match target {
            "control" => app.beat_ui.mix_bounds[&row][2].center(),
            "blank" => Pos2::new(app.midi_editor.panel_bounds.left() + 2.0, header.center().y),
            _ => header.center(),
        };
        let original = app.session.project.clone();
        app.dirty = false;
        click(&mut app, &ctx, point);
        assert_eq!(app.beat_ui.primary, Some(row), "{target}");
        app.view_mode = ViewMode::Instrument;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.selected_instrument_owner(), Some(row), "{target}");
        assert_eq!(app.session.project, original, "{target}");
        assert!(!app.dirty, "{target}");
    }
}

#[test]
fn nested_layer_group_drag_moves_copies_and_cancels_without_early_edits() {
    for action in ["move", "copy", "escape", "focus", "outside"] {
        let (mut app, ctx, _, row, _) = setup();
        let instrument = app.session.project.instrument_settings_mut(row).unwrap();
        instrument.layers.extend([
            daw_core::InstrumentLayerSettings::default(),
            daw_core::InstrumentLayerSettings::default(),
        ]);
        for layer in &mut instrument.layers {
            layer.collapsed = true;
        }
        instrument.lfo_lane.collapsed = true;
        instrument.envelope_lane.collapsed = true;
        instrument.main.collapsed = true;
        let ids: Vec<_> = instrument.layers.iter().map(|l| l.id).collect();
        app.navigate_beat_owner(app.selected_track.unwrap(), app.beat_ui.primary, false);
        app.view_mode = ViewMode::Instrument;
        frame(&mut app, &ctx, vec![]);
        for (index, shift) in [(1, false), (0, true)] {
            let point = app.midi_editor.layer_bounds[&ids[index]].icon.center();
            super::instrument_catalog::click(&mut app, &ctx, point, shift);
        }
        let before = app.session.project.clone();
        app.dirty = false;
        let start = app.midi_editor.layer_bounds[&ids[0]].name.center();
        let last = app.midi_editor.layer_bounds[&ids[2]].row;
        let end = Pos2::new(start.x, last.bottom() - 8.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            app.session.project, before,
            "{action}: preview changed the project"
        );
        assert!(!app.dirty);
        if action == "escape" {
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
        }
        if action == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused: false,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        let end = if action == "outside" {
            Pos2::new(5.0, 5.0)
        } else {
            end
        };
        let modifiers = egui::Modifiers {
            alt: action == "copy" && cfg!(target_os = "macos"),
            ctrl: action == "copy" && !cfg!(target_os = "macos"),
            ..Default::default()
        };
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(end),
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                },
            ],
        );
        let actual = &app.session.project.instrument_settings(row).unwrap().layers;
        let original = &before.instrument_settings(row).unwrap().layers;
        match action {
            "move" => assert_eq!(
                actual,
                &vec![
                    original[2].clone(),
                    original[0].clone(),
                    original[1].clone()
                ]
            ),
            "copy" => {
                assert_eq!(actual.len(), 5);
                assert_eq!(&actual[..3], original);
                for (copy, source) in actual[3..].iter().zip(&original[..2]) {
                    assert_ne!(copy.id, source.id);
                    assert_eq!(copy.name, source.name);
                }
            }
            _ => {
                assert_eq!(app.session.project, before, "{action}");
                assert!(!app.dirty, "{action}");
            }
        }
        assert!(app.error.is_none(), "{action}: {:?}", app.error);
    }
}

#[test]
fn dense_beat_grid_paints_visible_content_and_reuses_its_source_index() {
    let (mut app, ctx, track, first, clip) = setup();
    let beat = app.session.project.tracks[0].beat.as_mut().unwrap();
    beat.subtracks
        .extend((1..64).map(|i| BeatSubtrack::new(format!("Row {i}"))));
    let mut pattern = BeatPattern::default()
        .with_grid(16, daw_core::BeatResolution::ThirtySecond)
        .unwrap();
    for row in &beat.subtracks {
        for start_tick in (0..pattern.length_ticks()).step_by(120) {
            pattern.steps.push(daw_core::BeatStep {
                id: Id::new_v4(),
                subtrack_id: row.id,
                start_tick,
                velocity: 100,
            });
        }
    }
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.content.zoom = 480.0;
    app.midi_editor.content.scroll = 2.0;
    app.midi_editor.content.fitted = false;
    let original = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    let builds = app.beat_ui.step_index.builds;
    assert!(
        app.beat_ui.headers.len() < 32,
        "offscreen rows must not construct widgets"
    );
    assert!(
        app.beat_ui.steps.len() < app.beat_ui.headers.len() * 64,
        "offscreen time cells must not construct widgets: {} cells, {} rows, zoom {}, scroll {}",
        app.beat_ui.steps.len(),
        app.beat_ui.headers.len(),
        app.midi_editor.content.zoom,
        app.midi_editor.content.scroll,
    );
    assert!(!app.beat_ui.steps.contains_key(&(first, 0)));
    let (&(_, tick), bounds) = app
        .beat_ui
        .steps
        .iter()
        .find(|((id, _), bounds)| {
            *id == first
                && bounds.center().x > app.midi_editor.content.left
                && bounds.center().x < app.midi_editor.content.ruler.right()
        })
        .unwrap();
    let point = bounds.center();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.beat_ui.step_index.builds, builds,
        "unchanged source must reuse its index"
    );
    assert_eq!(app.session.project, original);
    click(&mut app, &ctx, point);
    let source = app.session.project.beat(track).unwrap().id;
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    let pattern = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset)
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap();
    assert_eq!(pattern.instrument_id, source);
    assert!(
        !pattern
            .pattern
            .steps
            .iter()
            .any(|s| s.subtrack_id == first && s.start_tick == tick)
    );
    assert!(app.beat_ui.step_index.builds > builds);
}

#[test]
#[ignore = "manual optimized beat-grid scaling probe"]
fn beat_editor_dense_grid_scaling() {
    for rows in [1, 8, 32, 64] {
        let mut app = DawUi::default();
        let track = app.session.project.add_beat_track().unwrap();
        let beat = app.session.project.tracks[0].beat.as_mut().unwrap();
        beat.subtracks = (0..rows)
            .map(|i| BeatSubtrack::new(format!("Row {i}")))
            .collect();
        let mut pattern = BeatPattern::default()
            .with_grid(16, daw_core::BeatResolution::ThirtySecond)
            .unwrap();
        for row in &beat.subtracks {
            for start_tick in (0..pattern.length_ticks()).step_by(120) {
                pattern.steps.push(daw_core::BeatStep {
                    id: Id::new_v4(),
                    subtrack_id: row.id,
                    start_tick,
                    velocity: 100,
                });
            }
        }
        let clip = app.session.project.add_beat_pattern(track, 0).unwrap();
        app.session
            .project
            .edit(Edit::ReplaceBeatPattern {
                clip_id: clip,
                pattern,
            })
            .unwrap();
        app.select_clip(clip, track, false);
        app.view_mode = ViewMode::Edit;
        let ctx = context();
        for _ in 0..2 {
            frame(&mut app, &ctx, vec![]);
        }
        let started = std::time::Instant::now();
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        eprintln!(
            "rows={rows} average_frame_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0 / 3.0
        );
    }
}

#[test]
fn trigger_pitch_names_round_trip_all_midi_notes_and_reject_invalid_input() {
    use crate::beat_editor::{parse_trigger_note, trigger_note_name};
    for pitch in 0..=127 {
        assert_eq!(
            parse_trigger_note(&trigger_note_name(pitch)),
            Some(f64::from(pitch))
        );
        assert_eq!(
            parse_trigger_note(&daw_unit_core::note_name(pitch)),
            Some(f64::from(pitch))
        );
    }
    assert_eq!(trigger_note_name(60), "C4");
    assert_eq!(trigger_note_name(51), "D3#");
    for invalid in [
        "", "60", "C", "C-2", "G9#", "E4#", "B3#", "D3##", "D#3#", "🎵4",
    ] {
        assert_eq!(parse_trigger_note(invalid), None, "{invalid}");
    }
}

#[test]
fn trigger_pitch_menu_aligns_note_value_and_commits_only_valid_notes() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, row, _) = setup();
        app.view_mode = view;
        app.track_browser.visible = true;
        frame(&mut app, &ctx, vec![]);
        let header = app.beat_ui.header_panels[&row];
        let before = app.session.project.clone();
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(header.center()),
                    egui::Event::PointerButton {
                        pos: header.center(),
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let text = |label: &str| {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing menu text: {label}"))
        };
        let rename = text("Rename");
        let delete = text("Delete selected subtracks");
        let label = text("Trigger pitch");
        let note = text("C4");
        let stops = text("Stops");
        let [pitch_row, input] = app.beat_ui.trigger_pitch_bounds.unwrap();
        assert!((label.left() - rename.left()).abs() < 0.1);
        assert!((label.left() - delete.left()).abs() < 0.1);
        assert!((label.left() - stops.left()).abs() < 0.1);
        assert!((label.left() - pitch_row.left() - f32::from(menus::ITEM_PADDING)).abs() < 0.1);
        assert!(
            (pitch_row.height() - label.height() - 2.0 * f32::from(menus::ITEM_PADDING)).abs()
                < 0.1
        );
        assert!((input.right() - pitch_row.right() + f32::from(menus::ITEM_PADDING)).abs() < 1.1);
        assert!((note.right() - input.right()).abs() < 0.1);
        assert!(note.left() - label.right() >= menus::ITEM_SPACING);
        assert!(
            (stops.top()
                - label.bottom()
                - 2.0 * f32::from(menus::ITEM_PADDING)
                - menus::ITEM_SPACING)
                .abs()
                < 0.1
        );
        assert_eq!(app.session.project, before);
        click(&mut app, &ctx, input.center());
        assert!(ctx.text_edit_focused());
        frame(&mut app, &ctx, vec![egui::Event::Text("D3#".into())]);
        assert_eq!(app.session.project, before);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Enter, false, false);
        assert_eq!(
            app.session.project.beat(track).unwrap().subtracks[0].trigger_pitch,
            51
        );
        frame(&mut app, &ctx, vec![]);
        let input = app.beat_ui.trigger_pitch_bounds.unwrap()[1];
        for (draft, key) in [("invalid", egui::Key::Enter), ("G9", egui::Key::Escape)] {
            click(&mut app, &ctx, input.center());
            frame(&mut app, &ctx, vec![egui::Event::Text(draft.into())]);
            super::instrument_catalog::key(&mut app, &ctx, key, false, false);
            assert_eq!(
                app.session.project.beat(track).unwrap().subtracks[0].trigger_pitch,
                51
            );
        }
    }
}

#[test]
fn subtrack_context_menu_opens_across_headers_without_changing_controls() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        {
            let (mut app, ctx, _, row, _) = setup();
            app.view_mode = view;
            if view == ViewMode::Instrument {
                app.track_browser.visible = true;
            }
            frame(&mut app, &ctx, vec![]);
            let rect = app.beat_ui.header_panels[&row];
            let original = app.session.project.clone();
            app.dirty = false;
            for x in [
                1.0,
                rect.width() * 0.2,
                rect.width() * 0.4,
                rect.width() * 0.6,
                rect.width() * 0.8,
                rect.width() - 1.0,
            ] {
                for y in [1.0, rect.height() * 0.5, rect.height() - 1.0] {
                    let point = rect.min + Vec2::new(x, y);
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
                    assert!(egui::Popup::is_any_open(&ctx), "{view:?} {point:?}");
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    assert!(
                        shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Text(text) if text.galley.text() == "Trigger pitch"
                        )),
                        "the shared subtrack menu must own the popup: {view:?} {point:?} {:?}",
                        shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::Shape::Text(text) => Some(text.galley.text()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(app.session.project, original);
                    assert!(!app.dirty);
                    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
                    assert!(!egui::Popup::is_any_open(&ctx));
                }
            }
        }
    }
}

#[test]
fn output_and_subtrack_pages_keep_owner_local_layer_selection() {
    let (mut app, ctx, track, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    app.navigate_beat_owner(track, None, false);
    frame(&mut app, &ctx, vec![]);
    let output = app.session.project.beat(track).unwrap().output.id;
    app.instrument_ui.select_layer(output);
    app.navigate_beat_owner(track, Some(row), false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.track, Some(row));
    assert!(!app.instrument_ui.selected_layers.contains(&output));
    app.navigate_beat_owner(track, None, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.track, Some(output));
    assert!(app.midi_editor.layer_bounds.is_empty());
    assert!(!app.dirty);
}

#[test]
#[ignore = "manual selected-owner paint scaling probe"]
fn selected_beat_owner_paint_ignores_sibling_processing_size() {
    let (mut app, ctx, parent, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    app.navigate_beat_owner(parent, Some(row), false);
    app.set_tracks_panel_visible(false);
    let sibling = BeatSubtrack::new("Hidden".into());
    app.session
        .project
        .edit(Edit::Beat {
            track_id: parent,
            edit: BeatEdit::Add {
                subtrack: Box::new(sibling),
                index: 1,
            },
        })
        .unwrap();
    for heavy in [false, true] {
        if heavy {
            app.session.project.tracks[0]
                .beat
                .as_mut()
                .unwrap()
                .subtracks[1]
                .instrument
                .layers[0]
                .effects = (0..4096)
                .map(|_| daw_core::EffectInstance::drive())
                .collect();
        }
        for _ in 0..5 {
            frame(&mut app, &ctx, vec![]);
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            frame(&mut app, &ctx, vec![]);
        }
        eprintln!(
            "hidden sibling heavy={heavy}: {:?} per 100 paints",
            start.elapsed()
        );
        assert_eq!(app.instrument_ui.track, Some(row));
    }
}

#[test]
#[ignore = "manual selected-owner paint scaling probe"]
fn editor_paint_ignores_sibling_processing_size() {
    let (mut app, ctx, parent, row, _) = setup();
    app.view_mode = ViewMode::Edit;
    app.navigate_beat_owner(parent, Some(row), false);
    app.set_tracks_panel_visible(false);
    let sibling = BeatSubtrack::new("Hidden".into());
    app.session
        .project
        .edit(Edit::Beat {
            track_id: parent,
            edit: BeatEdit::Add {
                subtrack: Box::new(sibling),
                index: 1,
            },
        })
        .unwrap();
    for heavy in [false, true] {
        if heavy {
            app.session.project.tracks[0]
                .beat
                .as_mut()
                .unwrap()
                .subtracks[1]
                .instrument
                .layers[0]
                .effects = (0..4096)
                .map(|_| daw_core::EffectInstance::drive())
                .collect();
        }
        for _ in 0..5 {
            frame(&mut app, &ctx, vec![]);
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            frame(&mut app, &ctx, vec![]);
        }
        eprintln!(
            "hidden sibling heavy={heavy}: {:?} per 100 paints",
            start.elapsed()
        );
        assert_eq!(app.instrument_ui.track, Some(row));
    }
}

#[test]
fn dense_shared_beat_thumbnails_have_bounded_shapes_and_reuse_conversion() {
    let (mut app, ctx, parent, _, clip_id) = setup();
    let mut rows = Vec::new();
    for i in 0..64 {
        rows.push(BeatSubtrack::new(format!("Row {i}")));
    }
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    app.session.project.tracks[0]
        .beat
        .as_mut()
        .unwrap()
        .subtracks = rows;
    let asset_id = app.session.project.tracks[0].clips[0].asset_id;
    let source = app
        .session
        .project
        .assets
        .iter_mut()
        .find(|a| a.id == asset_id)
        .unwrap()
        .beat_pattern
        .as_mut()
        .unwrap();
    source.pattern.bars = 16;
    source.pattern.resolution = daw_core::BeatResolution::ThirtySecond;
    source.pattern.steps = ids
        .iter()
        .flat_map(|row| {
            (0..512).map(move |slot| daw_core::BeatStep {
                id: Id::new_v4(),
                subtrack_id: *row,
                start_tick: slot * 120,
                velocity: 100,
            })
        })
        .collect();
    source.pattern.validate().unwrap();
    let frames = app.session.project.midi_frame(16 * 4 * 960);
    app.session.project.tracks[0].clips[0].length_frames = frames;
    app.session
        .project
        .assets
        .iter_mut()
        .find(|a| a.id == asset_id)
        .unwrap()
        .decoded_frame_count = frames;
    let clip = app.session.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == clip_id)
        .unwrap()
        .clone();
    app.session.project.validate().unwrap();
    app.set_view_zoom(25.0 / app.session.project.seconds(frames) as f32);
    for paint in 0..2 {
        let start = std::time::Instant::now();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 800.0))),
                ..Default::default()
            },
            |ui| {
                for i in 0..20 {
                    let rect = Rect::from_min_size(
                        Pos2::new(30.0 + i as f32 * 30.0, 50.0),
                        Vec2::new(25.0, 64.0),
                    );
                    app.paint_clip(
                        ui.painter(),
                        daw_core::LEGACY_CLIP_COLOR,
                        rect,
                        &clip,
                        false.into(),
                        None,
                    );
                }
            },
        );
        out.textures_delta.clear();
        eprintln!(
            "thumbnail paint={paint}: {:?}; shapes={}",
            start.elapsed(),
            out.shapes.len()
        );
        assert!(
            out.shapes.len() < 40_000,
            "zoomed-out markers must scale with pixels"
        );
        assert_eq!(
            app.beat_ui.thumbnails.borrow().builds,
            1,
            "shared immutable source converts once"
        );
    }
    assert_eq!(app.session.project.tracks[0].id, parent);
}

#[test]
fn detailed_beat_thumbnails_share_trim_repeat_stretch_mapping_with_midi() {
    let (mut app, ctx, _, row, clip_id) = setup();
    let pattern = BeatPattern::default()
        .with_step(row, 0, Some(100))
        .unwrap()
        .with_step(row, 960, Some(100))
        .unwrap();
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id,
            pattern: pattern.clone(),
        })
        .unwrap();
    let original = app.session.project.tracks[0].clips[0].clone();
    let midi = daw_core::MidiPattern {
        length_ticks: pattern.length_ticks(),
        notes: pattern
            .visible_steps()
            .map(|step| daw_core::MidiNote {
                id: step.id,
                start_tick: step.start_tick,
                length_ticks: pattern.resolution.step_ticks(),
                pitch: 60,
                velocity: step.velocity,
            })
            .collect(),
    };
    app.set_view_zoom(400.0);
    let color = theme::clip_colors(app.session.project.tracks[0].color, None).waveform;
    for trimmed in [false, true] {
        for stretched in [false, true] {
            for repeated in [false, true] {
                let mut clip = original.clone();
                if stretched {
                    clip.stretch = daw_core::ClipStretch::new(1, 2);
                }
                if trimmed {
                    clip.source_offset_frame = 6000;
                    clip.length_frames /= 2;
                }
                if repeated {
                    clip.repeat = Some(daw_core::ClipLoop {
                        length_frames: 24000,
                        phase_frame: 3000,
                    });
                    clip.length_frames *= 3;
                }
                let block = Rect::from_min_size(
                    Pos2::new(-100.0, 40.0),
                    Vec2::new(
                        app.session.project.seconds(clip.length_frames) as f32 * app.view_zoom(),
                        64.0,
                    ),
                );
                let mut markers = Vec::new();
                for beat in [true, false] {
                    let mut out = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(300.0, 200.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            let painter = ui.painter().with_clip_rect(Rect::from_min_max(
                                Pos2::new(30.0, 0.0),
                                Pos2::new(280.0, 200.0),
                            ));
                            if beat {
                                app.paint_clip(
                                    &painter,
                                    app.session.project.tracks[0].color,
                                    block,
                                    &clip,
                                    false.into(),
                                    None,
                                );
                            } else {
                                app.paint_midi_notes(&painter, block, &clip, &midi, color);
                            }
                        },
                    );
                    let mut rects: Vec<_> = out
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
                            _ => None,
                        })
                        .collect();
                    rects.sort_by(|a, b| {
                        a.min
                            .x
                            .total_cmp(&b.min.x)
                            .then(a.min.y.total_cmp(&b.min.y))
                    });
                    markers.push(rects);
                    out.textures_delta.clear();
                }
                assert_eq!(
                    markers[0], markers[1],
                    "trim={trimmed} stretch={stretched} repeat={repeated}"
                );
            }
        }
    }
}

#[test]
fn beat_source_growth_previews_keep_hits_in_both_views_and_edges() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        for left in [false, true] {
            let (mut app, ctx, track, row, clip) = setup();
            let pattern = BeatPattern::default()
                .with_step(row, 960, Some(100))
                .unwrap();
            app.session
                .project
                .edit(Edit::ReplaceBeatPattern {
                    clip_id: clip,
                    pattern,
                })
                .unwrap();
            app.session.project.tracks[0].clips[0].start_frame = 96_000;
            app.view_mode = view;
            app.zoom = 120.0;
            app.scroll = 0.0;
            app.midi_editor.zoom = 120.0;
            app.midi_editor.scroll = 0.0;
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            let current = original.tracks[0].clips[0].clone();
            let block = if view == ViewMode::Edit {
                app.midi_editor.clip_bounds[&clip]
            } else {
                app.clip_block(app.lane_bounds[&track], &current, current.start_frame)
            };
            let hit_x = block.left() + 60.0;
            let origin = Pos2::new(
                if left {
                    block.left() + 1.0
                } else {
                    block.right() - 1.0
                },
                block.top() + 30.0,
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(origin), button(origin, true)],
            );
            let builds = app.beat_ui.thumbnails.borrow().builds;
            for distance in [60.0, 70.0, 80.0] {
                let end = origin + Vec2::new(if left { -distance } else { distance }, 0.0);
                let shapes = frame_shapes(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(end)],
                    Vec2::new(1280.0, 800.0),
                );
                assert!(app.drag.is_some());
                assert_eq!(app.session.project, original);
                assert!(!shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Missing source")
                ));
                assert!(shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Rect(r) if (r.rect.left()-hit_x).abs()<1.0 && (r.rect.height()-2.0).abs()<0.01 && r.rect.top()>block.top()+22.0 && r.rect.bottom()<=block.bottom()+0.01)),"retained hit geometry: {view:?}, left={left}");
                assert_eq!(
                    app.beat_ui.thumbnails.borrow().builds,
                    builds,
                    "transient growth sources must not fill the live cache"
                );
            }
            super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
            assert_eq!(app.session.project, original);
        }
    }
}

#[test]
fn beat_thumbnails_keep_first_and_sixty_first_rows_separate() {
    let (mut app, ctx, _, _, clip) = setup();
    let rows: Vec<_> = (0..61)
        .map(|i| BeatSubtrack::new(format!("Row {i}")))
        .collect();
    let pattern = BeatPattern::default()
        .with_step(rows[0].id, 0, Some(100))
        .unwrap()
        .with_step(rows[60].id, 0, Some(100))
        .unwrap();
    app.session.project.tracks[0]
        .beat
        .as_mut()
        .unwrap()
        .subtracks = rows;
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    app.session.project.validate().unwrap();
    let clip = app.session.project.tracks[0].clips[0].clone();
    app.set_view_zoom(120.0);
    let color = theme::clip_colors(app.session.project.tracks[0].color, None).waveform;
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 500.0))),
            ..Default::default()
        },
        |ui| {
            app.paint_clip(
                ui.painter(),
                app.session.project.tracks[0].color,
                Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(240.0, 320.0)),
                &clip,
                false.into(),
                None,
            );
        },
    );
    let ys: Vec<_> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) if r.fill == color => Some(r.rect.top()),
            _ => None,
        })
        .collect();
    assert_eq!(ys.len(), 2);
    assert!((ys[0] - ys[1]).abs() > 200.0);
    out.textures_delta.clear();
}

#[test]
fn timeline_beat_children_use_group_geometry_and_saved_disclosure() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for collapsed in [true, false] {
            let (mut app, ctx, parent, child, _) = setup();
            app.settings.theme = theme;
            theme::apply(&ctx, app.settings.clone());
            app.view_mode = ViewMode::Timeline;
            app.track_rows_collapsed = collapsed;
            let group = Id::new_v4();
            app.edit(Edit::GroupTracks {
                track_ids: vec![parent],
                group_id: group,
                name: "Beats".into(),
            });
            app.dirty = false;
            let before = app.session.project.clone();
            frame(&mut app, &ctx, vec![]);
            assert!(!app.timeline_beats.rows.contains_key(&child));
            let toggle = app.timeline_beats.disclosure[&parent].center();
            click(&mut app, &ctx, toggle);
            let (owner, panel, lane) = app.timeline_beats.rows[&child];
            assert_eq!(owner, parent);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            super::track_header::assert_header_meter_insets(&shapes, panel, &ctx);
            let color = theme::clip_colors(
                app.session
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == parent)
                    .unwrap()
                    .color,
                None,
            )
            .body;
            let stripe =
                Rect::from_min_max(panel.min, Pos2::new(panel.left() + 16., panel.bottom()));
            let gutter = Rect::from_min_max(
                Pos2::new(panel.left() - 8., panel.top()),
                Pos2::new(panel.left(), panel.bottom()),
            );
            for expected in [stripe, gutter] {
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == expected && r.fill == color)));
            }
            let [mute, solo, pan, gain] = app.beat_ui.mix_bounds[&child];
            assert!(mute.left() >= panel.left() + 23.0);
            assert!(mute.right() <= solo.left());
            if collapsed {
                assert_eq!([pan, gain], [Rect::NOTHING; 2]);
            } else {
                assert!((gain.center().x - (panel.right() - 47.0)).abs() < 0.1);
                assert!((pan.center().x - (panel.right() - 79.0)).abs() < 0.1);
            }
            assert_eq!(panel.top(), app.lane_bounds[&parent].bottom());
            assert_eq!(panel.height(), app.track_row_height());
            assert_eq!(lane.left(), app.lane_bounds[&parent].left());
            assert_eq!(
                panel.left(),
                lane.left() - TRACK_WIDTH + panels::GroupPanel::indent(2)
            );
            assert!(
                !app.lane_bounds.contains_key(&child),
                "child lanes cannot receive clips"
            );
            assert!(
                app.file_target(lane.center(), app.timeline_bounds, false)
                    .is_none()
            );
            let panels = app.track_panel_containers(&[
                (group, Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0))),
                (
                    parent,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(250.0, panel.top())),
                ),
                (child, panel),
            ]);
            assert_eq!(panels[0].1.bottom(), panel.bottom());
            assert_eq!(panels[1].1.bottom(), panel.bottom());
            let mut expanded = before.clone();
            expanded
                .tracks
                .iter_mut()
                .find(|track| track.id == parent)
                .unwrap()
                .group_collapsed = false;
            assert_eq!(app.session.project, expanded);
            assert!(app.dirty);
            app.perform_history_action(HistoryAction::Undo, &ctx);
            frame(&mut app, &ctx, vec![]);
            assert_eq!(app.session.project, before);
            assert!(!app.timeline_beats.rows.contains_key(&child));
            app.perform_history_action(HistoryAction::Redo, &ctx);
            frame(&mut app, &ctx, vec![]);
            assert_eq!(app.session.project, expanded);
            assert!(app.timeline_beats.rows.contains_key(&child));
            app.session
                .project
                .edit(Edit::SetGroupCollapsed {
                    track_id: group,
                    collapsed: true,
                })
                .unwrap();
            frame(&mut app, &ctx, vec![]);
            assert!(app.timeline_beats.rows.is_empty());
            app.session
                .project
                .edit(Edit::SetGroupCollapsed {
                    track_id: group,
                    collapsed: false,
                })
                .unwrap();
            frame(&mut app, &ctx, vec![]);
            assert!(
                app.timeline_beats.rows.contains_key(&child),
                "enclosing collapse retains beat disclosure"
            );
            click(&mut app, &ctx, toggle);
            assert!(!app.timeline_beats.rows.contains_key(&child));
            assert_eq!(app.session.project, before);
        }
    }
}

#[test]
fn timeline_beat_child_controls_edit_only_the_shared_subtrack_bus() {
    let (mut app, ctx, parent, child, _) = setup();
    let other = BeatSubtrack::new("Snare".into());
    let other_id = other.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: parent,
            edit: BeatEdit::Add {
                subtrack: Box::new(other),
                index: 1,
            },
        })
        .unwrap();
    app.view_mode = ViewMode::Timeline;
    frame(&mut app, &ctx, vec![]);
    let before = app
        .session
        .project
        .tracks
        .iter()
        .find(|track| track.id == parent)
        .unwrap()
        .clone();
    let toggle = app.timeline_beats.disclosure[&parent].center();
    click(&mut app, &ctx, toggle);
    let [mute, solo, _, _] = app.beat_ui.mix_bounds[&child];
    click(&mut app, &ctx, mute.center());
    click(&mut app, &ctx, solo.center());
    let beat = app.session.project.beat(parent).unwrap();
    let row = beat.subtracks.iter().find(|row| row.id == child).unwrap();
    assert!(row.muted && row.soloed);
    let other = beat
        .subtracks
        .iter()
        .find(|row| row.id == other_id)
        .unwrap();
    assert!(!other.muted && !other.soloed);
    assert!(!before.muted && !before.soloed);
    let after = app
        .session
        .project
        .tracks
        .iter()
        .find(|track| track.id == parent)
        .unwrap();
    assert_eq!(after.clips, before.clips);
    assert_eq!(
        (after.gain_db, after.pan, after.muted, after.soloed),
        (before.gain_db, before.pan, before.muted, before.soloed)
    );
    assert_eq!(app.selected_track, Some(parent));
    assert!(app.timeline_beat_child_selected(child));
    assert_eq!(app.instrument_ui.track, Some(child));
    assert!(
        matches!(app.timeline_beat_effect_target(child), daw_core::EffectChainTarget::Layer { track, .. } if track == child)
    );
    let other_header = app.timeline_beats.rows[&other_id].1;
    let point = other_header.min + Vec2::new(30.0, 12.0);
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![button(point, false)], false);
    assert!(app.timeline_beat_child_selected(child));
    assert!(app.timeline_beat_child_selected(other_id));
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(!app.has_selected_beat_subtracks());
    let unchanged = app.session.project.clone();
    let clip = &before.clips[0];
    let start = app
        .clip_block(app.lane_bounds[&parent], clip, clip.start_frame)
        .min
        + Vec2::new(20.0, 12.0);
    let end = app.timeline_beats.rows[&child].2.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.drag.is_some());
    assert!(
        app.move_group_preview(app.drag.as_ref().unwrap(), end, true)
            .is_none()
    );
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project, unchanged,
        "child lanes reject clip placement"
    );
}

#[test]
fn subtrack_name_buttons_hover_and_select_once_in_timeline_and_editor() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        for color in [
            settings::ColorTheme::Dark,
            settings::ColorTheme::Light,
            settings::ColorTheme::Mist,
        ] {
            let (mut app, ctx, parent, child, _) = setup();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color,
                    ..Default::default()
                },
            );
            app.view_mode = view;
            if view == ViewMode::Timeline {
                frame(&mut app, &ctx, vec![]);
                let point = app.timeline_beats.disclosure[&parent].center();
                click(&mut app, &ctx, point);
            }
            app.deselect_beat_subtracks();
            frame(&mut app, &ctx, vec![]);
            let bounds = app.beat_ui.headers[&child];
            assert!(bounds.width() > 30.0, "the full name area is interactive");
            let point = Pos2::new(bounds.right() - 2.0, bounds.center().y);
            let before = app.session.project.clone();
            app.dirty = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    events: vec![egui::Event::PointerMoved(point)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            assert_eq!(
                output.platform_output.cursor_icon,
                if view == ViewMode::Timeline {
                    egui::CursorIcon::PointingHand
                } else {
                    egui::CursorIcon::Grab
                }
            );
            output.textures_delta.clear();
            click(&mut app, &ctx, point);
            assert!(app.beat_subtrack_selected(child));
            assert!(
                !app.beat_renaming(child),
                "single clicks select without Rename"
            );
            let toggle = Pos2::new(bounds.left() + 2.0, bounds.center().y);
            shift_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(toggle), button(toggle, true)],
                true,
            );
            shift_frame(&mut app, &ctx, vec![button(toggle, false)], true);
            shift_frame(&mut app, &ctx, vec![], false);
            assert!(
                !app.beat_subtrack_selected(child),
                "Shift-click toggles exactly once"
            );
            assert!(!app.beat_renaming(child));
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
}

fn timeline_subtrack_shortcut_fixture() -> (DawUi, egui::Context, Id, [Id; 3]) {
    let (mut app, ctx, parent, first, _) = setup();
    app.use_native_menu();
    let mut ids = vec![first];
    for index in 1..3 {
        let mut row = BeatSubtrack::new(format!("Row {index}"));
        row.muted = index == 1;
        row.soloed = index == 1;
        ids.push(row.id);
        app.edit(Edit::Beat {
            track_id: parent,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index,
            },
        });
    }
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    let arrow = app.timeline_beats.disclosure[&parent].center();
    click(&mut app, &ctx, arrow);
    let point = app.beat_ui.headers[&first].center();
    click(&mut app, &ctx, point);
    let point = app.beat_ui.headers[&ids[1]].center();
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![button(point, false)], true);
    shift_frame(&mut app, &ctx, vec![], false);
    assert!(app.beat_subtrack_selected(first) && app.beat_subtrack_selected(ids[1]));
    (app, ctx, parent, ids.try_into().unwrap())
}

#[test]
fn timeline_subtrack_mix_shortcuts_toggle_selected_buses_as_one_history_edit() {
    let (mut app, ctx, parent, ids) = timeline_subtrack_shortcut_fixture();
    let layer = app.session.project.beat(parent).unwrap().subtracks[1]
        .instrument
        .layers[0]
        .id;
    // Cached Instrument selection must not redirect Timeline actions to source layers.
    app.instrument_ui.track = Some(ids[1]);
    app.instrument_ui.selected_layer = Some(layer);
    app.instrument_ui.selected_layers.insert(layer);
    let original = app.session.project.clone();
    let mut expected = original.clone();
    for key in [egui::Key::M, egui::Key::S] {
        let before = expected.clone();
        for row in &mut expected.tracks[0].beat.as_mut().unwrap().subtracks[..2] {
            if key == egui::Key::M {
                row.muted = !row.muted;
            } else {
                row.soloed = !row.soloed;
            }
        }
        super::instrument_catalog::key(&mut app, &ctx, key, false, false);
        assert!(
            !app.session.project.tracks[0].muted && !app.session.project.tracks[0].soloed,
            "subtrack shortcuts must leave the beat parent unchanged"
        );
        assert_eq!(app.session.project, expected);
        app.perform_history_action(HistoryAction::Undo, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.session.project, before,
            "one Undo restores the whole selected group"
        );
        app.perform_history_action(HistoryAction::Redo, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.session.project, expected);
    }
    let parent_lane = app.lane_bounds[&parent];
    let point = Pos2::new(
        parent_lane.left() - TRACK_WIDTH + 60.0,
        parent_lane.top() + 18.0,
    );
    click(&mut app, &ctx, point);
    assert!(
        !app.has_selected_beat_subtracks(),
        "explicit parent selection must clear child routing"
    );
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::M, false, false);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::S, false, false);
    expected.tracks[0].muted = true;
    expected.tracks[0].soloed = true;
    assert_eq!(app.session.project, expected);
}

#[test]
fn timeline_subtrack_mix_shortcuts_retain_input_guards_and_mixer_parent_scope() {
    let (mut app, ctx, parent, ids) = timeline_subtrack_shortcut_fixture();
    let original = app.session.project.clone();
    for (modifiers, repeat) in [
        (egui::Modifiers::COMMAND, false),
        (egui::Modifiers::ALT, false),
        (egui::Modifiers::CTRL, false),
    ] {
        for key in [egui::Key::M, egui::Key::S] {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat,
                        modifiers,
                    },
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    },
                ],
            );
            assert_eq!(app.session.project, original);
        }
    }
    for key in [egui::Key::M, egui::Key::S] {
        let event = |repeat, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut app, &ctx, vec![event(false, true)]);
        let held = app.session.project.clone();
        frame(&mut app, &ctx, vec![event(true, true), event(false, false)]);
        assert_eq!(
            app.session.project, held,
            "held keys must not toggle repeatedly"
        );
        super::instrument_catalog::key(&mut app, &ctx, key, false, false);
    }
    assert_eq!(app.session.project, original);
    app.begin_beat_rename(ids[0], "Kick".into());
    frame(&mut app, &ctx, vec![]);
    assert!(ctx.text_edit_focused());
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::M, false, false);
    assert_eq!(app.session.project, original);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.perform_track_action(TrackAction::Mute, &ctx);
    assert!(app.session.project.tracks[0].muted);
    assert_eq!(
        app.session.project.beat(parent),
        original.beat(parent),
        "Mixer keeps whole-track scope even with cached child selection"
    );
}

#[test]
fn full_source_shows_trimmed_steps_once_and_edits_preserve_clip_mapping() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, track, row, clip_id) = setup();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        let pattern = BeatPattern::default()
            .with_step(row, 0, Some(71))
            .unwrap()
            .with_step(row, 3360, Some(91))
            .unwrap();
        app.session
            .project
            .edit(Edit::ReplaceBeatPattern { clip_id, pattern })
            .unwrap();
        let clip = &mut app.session.project.tracks[0].clips[0];
        clip.source_offset_frame = 24000;
        clip.length_frames = 48000;
        clip.start_frame = 192000;
        app.session.project.validate().unwrap();
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.beat_ui.clip_steps.len(), 16);
        assert!(app.beat_ui.clip_steps.contains_key(&(clip_id, row, 0)));
        assert!(app.beat_ui.clip_steps.contains_key(&(clip_id, row, 3360)));
        let before = app.session.project.tracks[0].clips[0].clone();
        let point = app.beat_ui.clip_steps[&(clip_id, row, 0)].center();
        click(&mut app, &ctx, point);
        let after = &app.session.project.tracks[0].clips[0];
        assert_eq!(
            (
                after.start_frame,
                after.source_offset_frame,
                after.length_frames
            ),
            (
                before.start_frame,
                before.source_offset_frame,
                before.length_frames
            )
        );
        assert_ne!(after.asset_id, before.asset_id);
        assert_eq!(app.selected_track, Some(track));
        assert!(app.beat_ui.repeated_steps.is_empty());
    }
}

#[test]
fn beat_trimming_hides_steps_and_expansion_restores_them_in_both_views() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        for left in [false, true] {
            let (mut app, ctx, track, row, clip_id) = setup();
            let pattern = BeatPattern::default()
                .with_step(row, 0, Some(71))
                .unwrap()
                .with_step(row, 960, Some(91))
                .unwrap()
                .with_step(row, 3360, Some(127))
                .unwrap();
            app.session
                .project
                .edit(Edit::ReplaceBeatPattern { clip_id, pattern })
                .unwrap();
            app.session.project.tracks[0].clips[0].start_frame = 48_000;
            app.set_view_mode(view, &ctx);
            app.history = None;
            frame(&mut app, &ctx, vec![]);
            let before = app.session.project.clone();
            let hidden_tick = if left { 0 } else { 3360 };
            let drag_edge = |app: &mut DawUi, delta: f32| {
                if view.has_track_overview() {
                    app.midi_editor.zoom = 120.0;
                    app.midi_editor.scroll = 0.0;
                    app.midi_editor.content.zoom = 120.0;
                    app.midi_editor.content.scroll = 0.0;
                    app.midi_editor.content.fitted = false;
                } else {
                    app.zoom = 120.0;
                    app.scroll = 0.0;
                }
                frame(app, &ctx, vec![]);
                let current = app.session.project.tracks[0].clips[0].clone();
                let lane = app.lane_bounds[&track];
                let block = if view.has_track_overview() {
                    app.midi_editor.clip_bounds[&clip_id]
                } else {
                    app.clip_block(lane, &current, current.start_frame)
                };
                let origin = Pos2::new(
                    if left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    block.top() + 30.0,
                );
                let end = origin + Vec2::new(delta, 0.0);
                let unchanged = app.session.project.clone();
                frame(
                    app,
                    &ctx,
                    vec![egui::Event::PointerMoved(origin), button(origin, true)],
                );
                frame(app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(app.session.project, unchanged);
                frame(app, &ctx, vec![button(end, false)]);
                finish_import(app, &ctx);
                assert!(app.error.is_none(), "{:?}", app.error);
            };
            drag_edge(&mut app, if left { 60.0 } else { -60.0 });
            let cropped = app.session.project.clone();
            assert_eq!(cropped.assets, before.assets);
            assert_eq!(cropped.beat(track), before.beat(track));
            assert_eq!(
                cropped.tracks[0].clips[0].asset_id,
                before.tracks[0].clips[0].asset_id
            );
            assert_eq!(cropped.tracks[0].clips[0].length_frames, 72_000);
            if view == ViewMode::Edit {
                assert!(
                    app.beat_ui
                        .clip_steps
                        .contains_key(&(clip_id, row, hidden_tick))
                );
                assert!(app.beat_ui.clip_steps.contains_key(&(clip_id, row, 960)));
            }
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, before);
            app.perform_history_action(HistoryAction::Redo, &ctx);
            assert_eq!(app.session.project, cropped);
            drag_edge(&mut app, if left { -60.0 } else { 60.0 });
            assert_eq!(app.session.project, before);
            if view == ViewMode::Edit {
                assert!(
                    app.beat_ui
                        .clip_steps
                        .contains_key(&(clip_id, row, hidden_tick))
                );
            }
        }
    }
}

#[test]
fn beat_groups_have_empty_gutters_no_dots_and_follow_the_time_signature() {
    for denominator in [4, 8] {
        let (mut app, ctx, _, row, _) = setup();
        app.session.project.time_signature = daw_core::TimeSignature {
            numerator: 4,
            denominator,
        };
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let beat_ticks = daw_core::MIDI_TICKS_PER_BEAT * 4 / u32::from(denominator);
        let previous = app.beat_ui.steps[&(row, beat_ticks - 240)];
        let next = app.beat_ui.steps[&(row, beat_ticks)];
        assert!((next.left() - previous.right() - 6.0).abs() < 0.1);
        let first = app.beat_ui.steps[&(row, 0)];
        let second = app.beat_ui.steps[&(row, 240)];
        assert!((second.left() - first.right() - 2.0).abs() < 0.1);
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t)
            if t.galley.text() == "·" && t.pos.x >= app.midi_editor.content.ruler.left()
                && t.pos.y > app.midi_editor.content.ruler.bottom()))
        );
        let before = app.session.project.clone();
        click(
            &mut app,
            &ctx,
            Pos2::new((previous.right() + next.left()) * 0.5, next.center().y),
        );
        assert_eq!(
            app.session.project, before,
            "beat gutter must not activate either tile"
        );
        let ruler = app.midi_editor.content.ruler;
        click(&mut app, &ctx, ruler.center());
        assert_eq!(
            app.session.project, before,
            "source ruler must not seek or change project loop"
        );
    }
}

#[test]
fn fitted_dense_source_tiles_stay_inside_their_own_time_cells() {
    let (mut app, ctx, _, row, clip_id) = setup();
    app.session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id,
            pattern: BeatPattern::default()
                .with_grid(16, daw_core::BeatResolution::ThirtySecond)
                .unwrap(),
        })
        .unwrap();
    app.midi_editor.content.fitted = true;
    frame(&mut app, &ctx, vec![]);
    let mut tiles: Vec<_> = app
        .beat_ui
        .steps
        .iter()
        .filter(|((id, _), _)| *id == row)
        .map(|((_, tick), rect)| (*tick, *rect))
        .collect();
    assert_eq!(tiles.len(), 512, "fit shows the complete dense source");
    tiles.sort_by_key(|(tick, _)| *tick);
    for pair in tiles.windows(2) {
        assert!(
            pair[0].1.right() <= pair[1].1.left(),
            "dense tiles overlap: {:?}",
            pair
        );
    }
}
