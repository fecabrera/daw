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
            app.toggle_beat_group(first);
            app.toggle_beat_group(second);
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
        assert!((first.left() - ruler.left() - 4.0).abs() < 0.1);
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
    assert!((first.left() - app.midi_editor.content.ruler.left() - 4.0).abs() < 0.1);
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
    assert!((first.left() - app.midi_editor.content.ruler.left() - 4.0).abs() < 0.1);
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
    assert!(name.left() < mute.left() && mute.left() < solo.left());
    assert!(solo.right() < pan.left() && pan.left() < gain.left());
    assert!((solo.left() - mute.right() - mix_controls::GROUP_GAP).abs() < 0.1);
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

            app.view_mode = ViewMode::Instrument;
            frame_shapes(&mut app, &ctx, vec![], size);
            app.toggle_beat_group(row);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            super::track_header::assert_header_meter_insets(
                &shapes,
                app.beat_ui.groups[&row].0,
                &ctx,
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
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
            app.toggle_beat_group(first);
            app.toggle_beat_group(second_id);
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
    }
}

#[test]
fn escape_cancels_a_subtrack_rename_without_clearing_its_selection() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, _, row, _) = setup();
        app.view_mode = view;
        frame(&mut app, &ctx, vec![]);
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        click(&mut app, &ctx, name);
        assert!(ctx.text_edit_focused());
        let before = app.session.project.clone();
        frame(&mut app, &ctx, vec![egui::Event::Text("draft".into())]);
        super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
        assert!(app.beat_subtrack_selected(row));
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn subtrack_outlines_match_track_style_and_paint_above_column_dividers() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for view in [ViewMode::Edit, ViewMode::Instrument] {
            let (mut app, ctx, _, row, _) = setup();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color,
                    ..Default::default()
                },
            );
            app.view_mode = view;
            if view == ViewMode::Instrument {
                app.toggle_beat_group(row);
            }
            frame(&mut app, &ctx, vec![]);
            let name = app.beat_ui.headers[&row].center();
            click(&mut app, &ctx, name);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let rect = if view == ViewMode::Instrument {
                app.beat_ui.groups[&row].0
            } else {
                let step = app.beat_ui.steps[&(row, 0)];
                Rect::from_min_size(
                    Pos2::new(app.midi_editor.panel_bounds.left(), step.top() - 3.0),
                    Vec2::new(TRACK_WIDTH, 32.0),
                )
            };
            let index = shapes
                .iter()
                .position(|s| {
                    matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect == rect && r.stroke == theme::selection_outline(&ctx))
                })
                .expect("full subtrack selection outline");
            let egui::Shape::Rect(outline) = &shapes[index].shape else {
                unreachable!()
            };
            assert_eq!(outline.corner_radius, egui::CornerRadius::same(2));
            assert_eq!(outline.stroke_kind, egui::StrokeKind::Inside);
            assert!(shapes[index].clip_rect.contains_rect(rect));
            if view == ViewMode::Instrument {
                for (border_index, shape) in shapes.iter().enumerate() {
                    if matches!(&shape.shape, egui::Shape::LineSegment { points, stroke }
                        if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                        && (points[0].x - (rect.right() - 0.5)).abs() < 0.1
                        && (points[1].x - points[0].x).abs() < 0.1
                        && points[0].y.min(points[1].y) <= rect.center().y
                        && points[0].y.max(points[1].y) >= rect.center().y)
                    {
                        assert!(
                            border_index < index,
                            "track-style outline must paint after the neutral divider"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn escape_deselects_an_expanded_instrument_subtrack() {
    let (mut app, ctx, _, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    frame(&mut app, &ctx, vec![]);
    let name = app.beat_ui.headers[&row].center();
    click(&mut app, &ctx, name);
    assert!(app.beat_subtrack_selected(row));
    let disclosure = app.beat_ui.groups[&row].1.center();
    click(&mut app, &ctx, disclosure);
    click(&mut app, &ctx, disclosure);
    assert!(!app.beat_group_collapsed(row));
    let before = app.session.project.clone();
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(
        !app.beat_subtrack_selected(row),
        "expanded group selection must clear with Escape; focused={:?}, editing={}",
        ctx.memory(|m| m.focused()),
        app.editing_enabled()
    );
    assert_eq!(app.session.project, before);
}

#[test]
fn instrument_escape_deselects_from_child_controls_and_shares_catalog_handling() {
    let (mut app, ctx, _, row, _) = setup();
    app.view_mode = ViewMode::Instrument;
    app.instrument_catalog_visible = true;
    app.instrument_catalog.expand_folders();
    frame(&mut app, &ctx, vec![]);
    let name = app.beat_ui.headers[&row].center();
    click(&mut app, &ctx, name);
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

    let name = app.beat_ui.headers[&row].center();
    click(&mut app, &ctx, name);
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
fn plain_layer_selection_replaces_selection_in_other_subtracks() {
    let (mut app, ctx, track, first, _) = setup();
    let second = BeatSubtrack::new("Snare".into());
    let second_id = second.id;
    let second_layer = second.instrument.layers[0].id;
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
    app.view_mode = ViewMode::Instrument;
    let size = Vec2::new(1600.0, 2000.0);
    let click = |app: &mut DawUi, point: Pos2, shift| {
        let modifiers = egui::Modifiers {
            shift,
            ..Default::default()
        };
        frame_shapes(
            app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                },
            ],
            size,
        );
        frame_shapes(
            app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
            size,
        );
        frame_shapes(
            app,
            &ctx,
            vec![egui::Event::ModifiersChanged(Default::default())],
            size,
        )
    };
    let initial_shapes = frame_shapes(&mut app, &ctx, vec![], size);
    let initial_second = app.midi_editor.layer_bounds[&second_layer].label;
    let lfo_id = app
        .session
        .project
        .instrument_settings(first)
        .unwrap()
        .lfo_lane
        .id;
    let lfo = &app.instrument_ui.modulation_lane_bounds[&lfo_id];
    let lfo_rect = lfo.label;
    let lfo_name = lfo.name.center();
    let before = app.session.project.clone();
    app.dirty = false;
    let shapes = click(&mut app, lfo_name, false);
    let selected = |shapes: &[egui::epaint::ClippedShape], rect| {
        shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx)))
    };
    assert!(
        !selected(&initial_shapes, initial_second),
        "inactive groups must not create a default layer selection"
    );
    assert!(selected(&shapes, lfo_rect));
    let second_name = app.midi_editor.layer_bounds[&second_layer].name.center();
    let shapes = click(&mut app, second_name, false);
    assert_eq!(app.beat_ui.primary, Some(second_id));
    assert!(
        !selected(&shapes, lfo_rect),
        "plain layer click must clear the previous subtrack's provider selection"
    );
    let second_rect = app.midi_editor.layer_bounds[&second_layer].label;
    assert!(selected(&shapes, second_rect));
    let shapes = click(&mut app, lfo_name, true);
    assert!(
        selected(&shapes, lfo_rect) && selected(&shapes, second_rect),
        "Shift preserves the existing layer selection"
    );
    let shapes = click(&mut app, second_name, false);
    assert!(!selected(&shapes, lfo_rect) && selected(&shapes, second_rect));
    // Escape clears both the active state and the other subtrack's cached selection.
    click(&mut app, lfo_name, true);
    super::instrument_catalog::key(&mut app, &ctx, egui::Key::Escape, false, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
    assert!(!selected(&shapes, lfo_rect) && !selected(&shapes, second_rect));
    let shapes = click(&mut app, lfo_name, false);
    assert!(selected(&shapes, lfo_rect));
    let output = app.beat_ui.final_output.unwrap();
    let shapes = click(&mut app, output.min + Vec2::new(45.0, 15.0), false);
    assert!(
        !selected(&shapes, lfo_rect),
        "the overall Output also replaces old layer selections"
    );
    assert_eq!(app.session.project, before);
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
    assert!((app.beat_ui.steps[&(row, 0)].left() - ruler.left() - 4.0).abs() < 0.1);
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
fn selected_beat_patterns_keep_disabled_steps_beyond_their_end() {
    let (mut app, ctx, _, row, clip) = setup();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom *= 0.5;
    frame(&mut app, &ctx, vec![]);
    let ruler = app.midi_editor.content.ruler;
    let disabled = app
        .beat_ui
        .disabled_steps
        .iter()
        .find(|(id, rect)| {
            *id == row && rect.left() >= ruler.left() && rect.right() <= ruler.right()
        })
        .expect("disabled cells beyond the one-bar pattern")
        .1
        .center();
    let before = app.session.project.clone();
    app.dirty = false;
    click(&mut app, &ctx, disabled);
    assert_eq!(app.session.project, before);
    assert_eq!(app.selected_clip, Some(clip));
    assert!(!app.dirty);
    let end_frame = app.session.project.tracks[0].clips[0].end();
    let edge = app.midi_editor.content.ruler.left()
        + app
            .midi_editor
            .content
            .timeline(&app.session.project)
            .pixel_at_frame(end_frame) as f32;
    let boundary = Pos2::new(edge + 0.5, app.beat_ui.steps[&(row, 0)].center().y);
    click(&mut app, &ctx, boundary);
    assert_eq!(
        app.session.project, before,
        "disabled space must not activate the nearest active step through its hit radius"
    );
    let active = app.beat_ui.steps[&(row, 0)].center();
    click(&mut app, &ctx, active);
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
}

#[test]
fn disabled_beat_steps_fill_before_and_after_the_selected_clip_and_ignore_other_patterns() {
    let (mut app, ctx, track, row, clip) = setup();
    let duration = app.session.project.tracks[0].clips[0].length_frames;
    app.session.project.tracks[0].clips[0].start_frame = duration;
    let other = app
        .session
        .project
        .add_beat_pattern(track, duration * 3)
        .unwrap();
    app.session.project.validate().unwrap();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom *= 0.25;
    app.midi_editor.content.scroll = 0.0;
    frame(&mut app, &ctx, vec![]);
    let clock = app.midi_editor.content.timeline(&app.session.project);
    let ruler = app.midi_editor.content.ruler;
    let disabled: Vec<_> = app
        .beat_ui
        .disabled_steps
        .iter()
        .filter(|(id, rect)| {
            *id == row && rect.left() >= ruler.left() && rect.right() <= ruler.right()
        })
        .map(|(_, rect)| {
            (
                clock.frame_at_pixel(f64::from(rect.center().x - ruler.left())) as u64,
                rect.center(),
            )
        })
        .collect();
    assert!(disabled.iter().any(|(frame, _)| *frame < duration));
    assert!(disabled.iter().any(|(frame, _)| *frame >= duration * 2));
    assert!(
        disabled
            .iter()
            .all(|(frame, _)| *frame < duration || *frame >= duration * 2)
    );
    let over_other = disabled
        .iter()
        .find(|(frame, _)| *frame >= duration * 3 && *frame < duration * 4)
        .expect("unselected patterns remain disabled in the lower editor")
        .1;
    let before = app.session.project.clone();
    click(&mut app, &ctx, over_other);
    assert_eq!(app.selected_clip, Some(clip));
    assert_ne!(app.selected_clip, Some(other));
    assert_eq!(app.session.project, before);
    app.selected_clip = None;
    app.selected_clips.clear();
    frame(&mut app, &ctx, vec![]);
    assert!(!app.beat_ui.disabled_steps.is_empty());
    let point = app
        .beat_ui
        .disabled_steps
        .iter()
        .find(|(_, rect)| rect.left() >= ruler.left() && rect.right() <= ruler.right())
        .unwrap()
        .1
        .center();
    click(&mut app, &ctx, point);
    assert_eq!(app.selected_clip, None);
    assert_eq!(app.session.project, before);
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
    let disabled = app.beat_ui.disabled_steps[0].1.center();
    click(&mut app, &ctx, disabled);
    assert_eq!(app.chords.audition.beat_requests.len(), 1);
}

#[test]
fn beat_editor_scrollbar_pans_only_the_step_viewport_without_edits() {
    let (mut app, ctx, track, row, clip) = setup();
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
    assert!(app.midi_editor.content.scroll > 0.0);
    assert!(!app.beat_ui.disabled_steps.is_empty());
}

#[test]
fn subtrack_name_input_selects_the_whole_name_and_matches_track_commit_and_cancel() {
    for view in [ViewMode::Edit, ViewMode::Instrument] {
        let (mut app, ctx, track, row, _) = setup();
        app.view_mode = view;
        frame(&mut app, &ctx, vec![]);
        let name = app.beat_ui.headers[&row].center();
        click(&mut app, &ctx, name);
        click(&mut app, &ctx, name);
        assert!(ctx.text_edit_focused());
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
fn subdivision_measures_stretched_step_duration_against_one_bar_and_ignores_clip_length() {
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
    assert_eq!(shown(&mut app, &ctx), "1/16");
    let measured_beats = |app: &DawUi| {
        let a = app.beat_ui.steps[&(row, 0)];
        let b = app.beat_ui.steps[&(row, 480)];
        f64::from(b.left() - a.left()) / f64::from(app.midi_editor.content.zoom) * 2.0
    };
    assert!((measured_beats(&app) - 0.25).abs() < 1e-6);
    let clip = app.session.project.tracks[0].clips[0].clone();
    let stretched = clip.stretched_to(clip.start_frame, 192_000).unwrap();
    app.session.project = app
        .session
        .project
        .stretched_clips(&[stretched], daw_core::ClipEdge::Right)
        .unwrap();
    assert_eq!(shown(&mut app, &ctx), "1/8");
    assert!((measured_beats(&app) - 0.5).abs() < 1e-6);
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
    assert_eq!(shown(&mut app, &ctx), "3/16");
    assert!((measured_beats(&app) - 0.75).abs() < 1e-6);
    app.midi_editor.content.zoom = 120.0;
    shown(&mut app, &ctx);
    let active = app.beat_ui.steps[&(row, 0)];
    let disabled = app
        .beat_ui
        .disabled_steps
        .iter()
        .find(|(id, r)| {
            *id == row
                && r.left() > active.left() + 400.0
                && r.right() < app.midi_editor.content.ruler.right()
        })
        .unwrap()
        .1;
    assert!(
        (disabled.width() - active.width()).abs() < 0.1,
        "disabled grid uses the same stretched step length"
    );
}

#[test]
fn subdivision_choices_show_audible_bar_fractions_and_edit_the_source_grid_once() {
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
        .zip(["1/2", "1/4", "1/8", "1/16"])
    {
        assert!(
            shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Text(t) if bounds.contains(t.pos) && t.galley.text() == expected)),
            "stretched popup choice must show {expected} of one bar"
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
        assert!(!app.beat_ui.disabled_steps.is_empty());
        app.settings_open = true;
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, preview.center());
        assert_eq!(app.chords.audition.beat_requests.len(), 1);
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn preview_steps_share_row_geometry_and_four_point_insets_with_a_fixed_grid_separator() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, _, row, _) = setup();
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
        assert!((first.left() - ruler.left() - 4.0).abs() < 0.1);
        assert!((next.left() - first.right() - 8.0).abs() < 0.1);
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
