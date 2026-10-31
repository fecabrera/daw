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
    assert_eq!(app.selected_instrument_owner(), Some(row));
    assert!(app.error.is_none(), "{:?}", app.error);
    app.view_mode = ViewMode::Edit;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.beat_ui.primary, Some(row));
}
#[test]
fn instrument_groups_reuse_bodies_and_show_one_final_output() {
    let (mut app, ctx, track, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    let mut settings = app
        .session
        .project
        .instrument_settings(row)
        .unwrap()
        .clone();
    settings.main.collapsed = false;
    app.session
        .project
        .edit(Edit::SetInstrument {
            track_id: row,
            instrument: Some(settings),
        })
        .unwrap();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 1400.0));
    let names: Vec<_> = shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                Some(t.galley.text())
            } else {
                None
            }
        })
        .collect();
    assert!(names.contains(&"Kick"), "{names:?}");
    assert!(names.contains(&"Layer 1"), "{names:?}");
    assert!(names.contains(&"LFO"));
    assert!(names.contains(&"Envelope"));
    assert_eq!(names.iter().filter(|name| **name == "Output").count(), 2);
    assert_eq!(app.session.project.beat(track).unwrap().subtracks.len(), 1);
    assert!(app.error.is_none());
}
#[test]
fn editor_without_active_clip_keeps_subtracks_and_does_not_edit_another_pattern() {
    let (mut app, ctx, _, row, _) = setup();
    app.selected_clip = None;
    app.selected_clips.clear();
    let before = app.session.project.clone();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Create beat pattern")));
    assert!(!app.beat_ui.steps.is_empty());
    assert!(app.beat_ui.headers.contains_key(&row));
    let step = app
        .beat_ui
        .steps
        .values()
        .find(|rect| {
            rect.left() >= app.midi_editor.content.ruler.left()
                && rect.right() <= app.midi_editor.content.ruler.right()
        })
        .unwrap()
        .center();
    app.dirty = false;
    click(&mut app, &ctx, step);
    assert_eq!(app.selected_clip, None);
    assert!(!app.dirty);
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
            // Keep all three group headers visible during this ordering gesture.
            for id in &ids {
                app.toggle_beat_group(*id);
            }
        }
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let a = app.beat_ui.headers[&ids[0]].center();
        let b = app.beat_ui.headers[&ids[1]].center();
        super::instrument_catalog::click(&mut app, &ctx, a, false);
        super::instrument_catalog::click(&mut app, &ctx, b, true);
        assert!(!app.dirty, "selection must not edit the project");
        // Move the selected first pair after the unselected last row.
        let destination = Pos2::new(a.x, app.beat_ui.headers[&ids[2]].bottom() + 50.0);
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
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    frame(&mut app, &ctx, vec![button(b, false)]);
    assert_eq!(
        app.session.project.beat(track).unwrap().subtracks[0].id,
        first
    );
    assert!(!app.dirty);
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
fn component_catalog_targets_local_provider_lanes_and_fixed_output_across_frames() {
    let (mut app, ctx, track, row, _) = setup();
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
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let output = shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                (t.galley.text() == "Output").then(|| t.pos + t.galley.size() * 0.5)
            } else {
                None
            }
        })
        .unwrap();
    click(&mut app, &ctx, output);
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
fn empty_beat_page_reuses_midi_header_column_and_pinned_output_geometry() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let mut midi = DawUi::default();
        let midi_track = midi.session.project.add_midi_track().unwrap();
        midi.select_track(midi_track, false);
        midi.view_mode = ViewMode::Instrument;
        let ctx = context();
        midi.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        frame(&mut midi, &ctx, vec![]);
        let header = midi.midi_editor.instrument_header_bounds;
        let plus = midi.midi_editor.add_layer_bounds;
        let output = midi.instrument_ui.main_bounds.row;
        let mut beat = DawUi::default();
        let track = beat.session.project.add_beat_track().unwrap();
        beat.select_track(track, false);
        beat.view_mode = ViewMode::Instrument;
        let ctx = context();
        beat.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        let shapes = frame_shapes(&mut beat, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert_eq!(beat.midi_editor.instrument_header_bounds, header);
        assert_eq!(beat.midi_editor.add_layer_bounds, plus);
        assert_eq!(beat.beat_ui.final_output.unwrap(), output);
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Subtracks"))
        );
        assert!(!shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Add beat subtrack")
        ));
        click(&mut beat, &ctx, plus.center());
        assert_eq!(beat.session.project.beat(track).unwrap().subtracks.len(), 1);
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
        assert!((first.left() - ruler.left() - 1.0).abs() < 0.1);
        assert!((first.top() - ruler.bottom() - 3.0).abs() < 0.1);
        assert!(first.height() <= 26.0, "step lanes must remain thin");
        let clock = app.midi_editor.content.timeline(&app.session.project);
        let expected = clock.pixels_for_frames(i128::from(
            app.session
                .project
                .midi_frame(daw_core::MIDI_TICKS_PER_BEAT / 4),
        )) as f32;
        assert!((next.left() - first.left() - expected).abs() < 0.1);
    }
}

#[test]
fn beat_ruler_and_steps_retain_trimmed_and_stretched_clip_mapping() {
    let (mut app, ctx, _, row, _) = setup();
    let step_ticks = daw_core::MIDI_TICKS_PER_BEAT / 4;
    let offset = app.session.project.midi_frame(step_ticks);
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.source_offset_frame = offset;
    clip.length_frames -= offset;
    frame(&mut app, &ctx, vec![]);
    assert!(!app.beat_ui.steps.contains_key(&(row, 0)));
    let first = app.beat_ui.steps[&(row, step_ticks)];
    assert!((first.left() - app.midi_editor.content.ruler.left() - 1.0).abs() < 0.1);
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.source_offset_frame *= 2;
    clip.length_frames *= 2;
    clip.stretch = Some(daw_core::ClipStretch {
        source_frames: 1,
        output_frames: 2,
    });
    frame(&mut app, &ctx, vec![]);
    let first = app.beat_ui.steps[&(row, step_ticks)];
    let next = app.beat_ui.steps[&(row, step_ticks * 2)];
    assert!((first.left() - app.midi_editor.content.ruler.left() - 1.0).abs() < 0.1);
    let expected = app
        .midi_editor
        .content
        .timeline(&app.session.project)
        .pixels_for_frames(i128::from(offset * 2)) as f32;
    assert!((next.left() - first.left() - expected).abs() < 0.1);
}

#[test]
fn compact_beat_controls_follow_track_spacing_and_requested_order() {
    let (mut app, ctx, track, row, _) = setup();
    let [mute, solo, pan, gain] = app.beat_ui.mix_bounds[&row];
    let name = app.beat_ui.headers[&row];
    assert!(mute.left() < solo.left() && solo.left() < name.left());
    assert!(name.right() < pan.left() && pan.left() < gain.left());
    assert!((solo.left() - mute.right() - mix_controls::GROUP_GAP).abs() < 0.1);
    assert!((name.left() - solo.right() - mix_controls::GROUP_GAP).abs() < 0.1);
    assert!((gain.center().x - pan.center().x - knobs::SIZE - mix_controls::KNOB_GAP).abs() < 0.1);
    assert!((mute.width() - toolbars::CONTROL_HEIGHT).abs() < 0.1);
    assert!((pan.center().y - mute.center().y).abs() < 0.1);
    click(&mut app, &ctx, mute.center());
    assert!(app.session.project.beat(track).unwrap().subtracks[0].muted);
    click(&mut app, &ctx, solo.center());
    assert!(app.session.project.beat(track).unwrap().subtracks[0].soloed);
}

#[test]
fn instrument_subtrack_disclosure_groups_indented_children_without_editing_audio() {
    let (mut app, ctx, _, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    let click_group = |app: &mut DawUi, point: Pos2| {
        for events in [
            vec![egui::Event::PointerMoved(point), button(point, true)],
            vec![button(point, false)],
            vec![],
        ] {
            frame_shapes(app, &ctx, events, Vec2::new(1600.0, 1400.0));
        }
    };
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 1400.0));
    let text_left = |label: &str| {
        shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos.x),
                _ => None,
            })
            .unwrap()
    };
    let parent = text_left("Kick");
    for child in ["Layer 1", "LFO", "Envelope"] {
        assert!(
            text_left(child) >= parent + crate::panels::GroupPanel::indent(1) - 1.0,
            "{child} must be nested under its subtrack"
        );
    }
    let main = &app.instrument_ui.main_bounds;
    let (group, disclosure_bounds) = app.beat_ui.groups[&row];
    assert_eq!(
        group.height(),
        MASTER_HEIGHT,
        "subtrack headers use one compact track row"
    );
    let settings = app.session.project.instrument_settings(row).unwrap();
    let layer = &app.midi_editor.layer_bounds[&settings.layers[0].id];
    let lfo = &app.instrument_ui.modulation_lane_bounds[&settings.lfo_lane.id];
    let envelope = &app.instrument_ui.modulation_lane_bounds[&settings.envelope_lane.id];
    for child in [layer.label, lfo.label, envelope.label, main.label] {
        assert!((child.left() - group.left() - crate::panels::GroupPanel::indent(1)).abs() < 0.1);
        assert!(
            (child.right() - group.right()).abs() < 0.1,
            "child dividers must align with the subtrack"
        );
    }
    assert!(
        main.row.top() >= envelope.row.bottom() - 1.0,
        "subtrack Output follows its provider lanes"
    );
    assert!(
        (main.row.top() - envelope.row.bottom()).abs() < 0.1,
        "Envelope and Output must share a single separator: envelope={:?}, output={:?}",
        envelope.row,
        main.row
    );
    let boundary = envelope.row.bottom() - 0.5;
    let boundary_lines: Vec<_> = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::LineSegment { points, .. }
                if (points[0].y - points[1].y).abs() < 0.1
                    && (points[0].y - boundary).abs() < 2.0
                    && points[0].x.min(points[1].x) <= main.label.left()
                    && points[0].x.max(points[1].x) >= main.label.right() =>
            {
                Some(points[0].y)
            }
            _ => None,
        })
        .collect();
    assert!(!boundary_lines.is_empty());
    assert!(
        boundary_lines.iter().all(|y| (*y - boundary).abs() < 0.1),
        "no adjacent separator strokes: {boundary_lines:?}"
    );
    let group_bottom = main.row.bottom() - 0.5;
    assert!(
        shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::LineSegment { points, .. } if (points[0].y - group_bottom).abs() < 0.1
                && (points[1].y - group_bottom).abs() < 0.1
                && points[0].x.min(points[1].x) <= group.left()
                && points[0].x.max(points[1].x) >= group.right()
        )),
        "the whole group must have a bottom border"
    );
    let gutter_x = group.left() + crate::panels::GroupPanel::indent(1) * 0.5;
    let parent_bottom = group.bottom() - 0.5;
    assert!(
        !shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
            if (points[0].y - parent_bottom).abs() < 0.1
                && (points[1].y - parent_bottom).abs() < 0.1
                && points[0].x.min(points[1].x) < gutter_x
                && points[0].x.max(points[1].x) > gutter_x
        )),
        "an expanded subtrack must not close its child gutter with a bottom divider"
    );
    assert!(
        shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
            if (points[0].y - parent_bottom).abs() < 0.1
                && (points[1].y - parent_bottom).abs() < 0.1
                && (points[0].x.min(points[1].x) - layer.label.left()).abs() < 0.1
        )),
        "retain the separator from the child panel edge onward"
    );
    assert!(
        !shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
            if (points[0].y - points[1].y).abs() < 0.1
                && points[0].y > layer.label.top() + 0.1
                && points[0].y < envelope.label.bottom() - 0.1
                && points[0].x.min(points[1].x) < gutter_x
                && points[0].x.max(points[1].x) > gutter_x
        )),
        "child separators must not cross the group gutter"
    );
    let output = app.beat_ui.final_output;
    let before = app.session.project.clone();
    app.dirty = false;
    let disclosure = disclosure_bounds.center();
    click_group(&mut app, disclosure);
    assert!(app.beat_group_collapsed(row));
    assert!(app.instrument_ui.component_viewports.is_empty());
    assert!(app.instrument_ui.samplers.is_empty());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    assert_eq!(app.beat_ui.final_output, output);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 1400.0));
    assert!(!shapes.iter().any(
        |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Layer 1")
    ));
    let collapsed_bottom = app.beat_ui.groups[&row].0.bottom() - 0.5;
    assert!(
        shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::LineSegment { points, .. } if (points[0].y - collapsed_bottom).abs() < 0.1
                && (points[1].y - collapsed_bottom).abs() < 0.1
                && points[0].x.min(points[1].x) <= group.left()
                && points[0].x.max(points[1].x) >= group.right()
        )),
        "collapsed groups also retain a full bottom border"
    );
    assert!(
        shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
            if (points[0].y - parent_bottom).abs() < 0.1
                && (points[1].y - parent_bottom).abs() < 0.1
                && points[0].x.min(points[1].x) < gutter_x
                && points[0].x.max(points[1].x) > gutter_x
        )),
        "collapsed headers have a complete bottom border"
    );
    click_group(&mut app, disclosure);
    assert!(!app.beat_group_collapsed(row));
    assert_eq!(app.session.project, before);
    let strip_collapse = app.instrument_ui.main_bounds.collapse.center();
    click_group(&mut app, strip_collapse);
    assert!(!app.beat_group_collapsed(row));
    assert_eq!(app.beat_ui.groups[&row].0.height(), MASTER_HEIGHT);
    assert_ne!(
        app.session
            .project
            .instrument_settings(row)
            .unwrap()
            .main
            .collapsed,
        before.instrument_settings(row).unwrap().main.collapsed
    );
}

#[test]
fn empty_beat_track_keeps_disabled_steps_visible_after_scrolling() {
    let (mut app, ctx, track, row, _) = setup();
    app.session.project.tracks[0].clips.clear();
    app.selected_clip = None;
    app.selected_clips.clear();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.scroll = 10.0;
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.midi_editor.content.ruler;
    let point = app
        .beat_ui
        .steps
        .values()
        .find(|rect| rect.left() >= ruler.left() && rect.right() <= ruler.right())
        .unwrap()
        .center();
    click(&mut app, &ctx, point);
    assert!(!app.beat_ui.steps.is_empty());
    assert_eq!(app.session.project, before);
    assert_eq!(app.selected_clip, None);
    let mute = app.beat_ui.mix_bounds[&row][0].center();
    click(&mut app, &ctx, mute);
    assert!(app.session.project.beat(track).unwrap().subtracks[0].muted);
}

#[test]
fn subtrack_header_adds_layers_and_moves_mix_controls_with_group_collapse() {
    let (mut app, ctx, track, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    let size = Vec2::new(1600.0, 1800.0);
    let click = |app: &mut DawUi, point: Pos2| {
        for events in [
            vec![egui::Event::PointerMoved(point), button(point, true)],
            vec![button(point, false)],
            vec![],
        ] {
            frame_shapes(app, &ctx, events, size);
        }
    };
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    let group = app.beat_ui.groups[&row].0;
    assert!(
        !shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Layers"))
    );
    assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "M" && group.contains(t.pos))));
    let output = app.instrument_ui.main_bounds.label;
    assert!(
        app.beat_ui.mix_bounds[&row]
            .iter()
            .all(|r| output.contains_rect(*r))
    );
    let first = app.session.project.instrument_settings(row).unwrap().layers[0].id;
    let add = app.beat_ui.add_layers[&row];
    assert!(group.contains_rect(add));
    assert!(
        (add.right() - (group.right() - 7.0)).abs() < 0.1,
        "Add layer must stay at the compact header's right inset"
    );
    click(&mut app, add.center());
    let settings = app.session.project.instrument_settings(row).unwrap();
    assert_eq!(settings.layers.len(), 2);
    assert_eq!(settings.layers[0].id, first);
    assert_eq!(settings.layers[1].name, "Layer 2");
    let disclosure = app.beat_ui.groups[&row].1.center();
    click(&mut app, disclosure);
    assert!(app.beat_group_collapsed(row));
    assert!(!app.beat_ui.add_layers.contains_key(&row));
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    let group = app.beat_ui.groups[&row].0;
    assert_eq!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Output"))
            .count(),
        1
    );
    assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if matches!(t.galley.text(), "Layer 1" | "Layer 2" | "LFO" | "Envelope"))));
    let mute = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "M" && group.contains(t.pos) => {
                Some(t.pos + Vec2::splat(3.0))
            }
            _ => None,
        })
        .expect("collapsed header retains its mix controls");
    click(&mut app, mute);
    assert!(app.session.project.beat(track).unwrap().subtracks[0].muted);
}
