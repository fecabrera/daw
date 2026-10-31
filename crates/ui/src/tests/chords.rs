use super::instrument_catalog::{click, key};
use super::*;

fn setup() -> (DawUi, egui::Context, Id, Id) {
    let mut app = DawUi::default();
    let midi = app.session.project.add_midi_track().unwrap();
    let audio = app.session.project.add_track().unwrap();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let toggle = app.chords.bounds.toggle.center();
    click(&mut app, &ctx, toggle, false);
    assert!(app.chords.visible);
    assert!(!app.timeline_catalog_visible && !app.clip_settings_visible);
    let folder = app.chords.bounds.folders[0].1.center();
    click(&mut app, &ctx, folder, false);
    assert_eq!(app.chords.bounds.entries.len(), 12);
    (app, ctx, midi, audio)
}
fn entry(app: &DawUi, index: usize) -> Pos2 {
    app.chords
        .bounds
        .entries
        .iter()
        .find(|(i, _)| *i == index)
        .unwrap()
        .1
        .center()
}
fn begin(app: &mut DawUi, ctx: &egui::Context, index: usize, point: Pos2) {
    let start = entry(app, index);
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(point)]);
}
fn release(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, false)],
    );
    wait_for_save(app);
}
#[test]
fn chord_browser_filters_notes_keeps_selection_and_browsing_out_of_project_history() {
    let (mut app, ctx, _, _) = setup();
    let before = serde_json::to_string(&app.session.project).unwrap();
    let point = entry(&app, 0);
    click(&mut app, &ctx, point, false);
    assert_eq!(app.chords.catalog.selected_types(), ["C major"]);
    let point = entry(&app, 1);
    click(&mut app, &ctx, point, true);
    assert_eq!(app.chords.catalog.selected_types(), ["C major", "C minor"]);
    let filter = app.chords.bounds.filter.center();
    click(&mut app, &ctx, filter, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280., 800.));
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if t.galley.text() == "D"
                    && t.pos.x >= app.chords.bounds.filter.left()
                    && s.clip_rect.contains(t.pos) =>
            {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    click(&mut app, &ctx, point, false);
    assert_eq!(app.chords.bounds.folders.len(), 1);
    assert_eq!(app.chords.bounds.folders[0].0, 2);
    assert!(
        app.chords
            .bounds
            .entries
            .iter()
            .all(|(i, _)| (24..36).contains(i))
    );
    assert!(app.chords.catalog.selected_types().is_empty());
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), before);
    assert!(!app.dirty);
}
#[test]
fn chord_drop_previews_notes_and_overwrites_only_on_release_with_undo_redo() {
    let (mut app, ctx, midi, _) = setup();
    app.session
        .project
        .add_midi_chords(
            Some(midi),
            0,
            &[daw_core::Chord {
                root: 9,
                kind: daw_core::ChordKind::Minor,
            }],
        )
        .unwrap();
    app.history = None;
    frame(&mut app, &ctx, vec![]);
    let before = serde_json::to_string(&app.session.project).unwrap();
    let point = app.lane_bounds[&midi].min + Vec2::new(app.zoom * 0.5, 30.);
    begin(&mut app, &ctx, 0, point);
    assert!(!app.chords.bounds.preview.is_empty());
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), before);
    assert!(!app.dirty);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280., 800.));
    assert!(shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="C major" && t.pos.x >= app.timeline_bounds.left())));
    release(&mut app, &ctx, point);
    assert!(app.chords.drag.is_none());
    assert_eq!(app.session.project.tracks[0].clips.len(), 2);
    let clip = app.session.project.tracks[0]
        .clips
        .iter()
        .find(|c| Some(c.id) == app.selected_clip)
        .unwrap();
    assert_eq!(clip.name, "C major");
    assert_eq!(clip.length_frames, app.session.project.midi_frame(3840));
    let after = serde_json::to_string(&app.session.project).unwrap();
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), before);
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), after);
}
#[test]
fn chord_groups_drop_into_empty_timeline_as_one_new_midi_track() {
    let (mut app, ctx, _, audio) = setup();
    let point = entry(&app, 0);
    click(&mut app, &ctx, point, false);
    let point = entry(&app, 1);
    click(&mut app, &ctx, point, true);
    let point = Pos2::new(
        app.timeline_bounds.left() + 100.,
        app.lane_bounds[&audio].bottom() + 25.,
    );
    begin(&mut app, &ctx, 0, point);
    assert_eq!(app.session.project.tracks.len(), 2);
    assert_eq!(app.chords.bounds.preview.len(), 2);
    release(&mut app, &ctx, point);
    assert_eq!(app.session.project.tracks.len(), 3);
    let track = &app.session.project.tracks[2];
    assert_eq!(track.kind, daw_core::TrackKind::Midi);
    assert_eq!(track.clips.len(), 2);
    assert_eq!(track.clips[0].name, "C major");
    assert_eq!(track.clips[1].name, "C minor");
    assert_eq!(track.clips[0].end(), track.clips[1].start_frame);
    assert_eq!(app.selected_clips.len(), 2);
}
#[test]
fn chord_drops_reject_audio_and_outside_and_cancel_on_escape_focus_or_dialog() {
    for reason in 0..5 {
        let (mut app, ctx, midi, audio) = setup();
        let before = serde_json::to_string(&app.session.project).unwrap();
        let point = if reason == 0 {
            app.lane_bounds[&audio].center()
        } else if reason == 1 {
            Pos2::new(1400., 400.)
        } else {
            app.lane_bounds[&midi].center()
        };
        begin(&mut app, &ctx, 0, point);
        if reason < 2 {
            assert!(app.chords.bounds.preview.is_empty());
        }
        if reason == 2 {
            key(&mut app, &ctx, egui::Key::Escape, false, false);
        }
        if reason == 3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.))),
                    focused: false,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        if reason == 4 {
            app.settings_open = true;
            frame(&mut app, &ctx, vec![]);
        }
        release(&mut app, &ctx, point);
        assert_eq!(
            serde_json::to_string(&app.session.project).unwrap(),
            before,
            "reason {reason}"
        );
        assert!(!app.dirty);
        assert!(app.chords.drag.is_none());
    }
}
#[test]
fn chord_enter_adds_selected_presets_and_catalog_tabs_remain_exclusive() {
    let (mut app, ctx, midi, _) = setup();
    app.select_track(midi, false);
    let point = entry(&app, 0);
    click(&mut app, &ctx, point, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    wait_for_save(&mut app);
    assert_eq!(app.session.project.tracks[0].clips.len(), 1);
    let toggle = app.effects_bounds.library_toggle.center();
    click(&mut app, &ctx, toggle, false);
    assert!(app.timeline_catalog_visible && !app.chords.visible);
    assert!(app.chords.catalog.selected_types().is_empty());
    let toggle = app.chords.bounds.toggle.center();
    click(&mut app, &ctx, toggle, false);
    let toggle = app.effects_bounds.clip_settings_toggle.center();
    click(&mut app, &ctx, toggle, false);
    assert!(app.clip_settings_visible && app.chords.visible);
}

#[test]
fn chord_keyboard_arrows_shift_selection_and_double_click_use_current_track_and_playhead() {
    let (mut app, ctx, midi, audio) = setup();
    app.select_track(midi, false);
    let point = entry(&app, 0);
    click(&mut app, &ctx, point, false);
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
    assert_eq!(app.chords.catalog.selected_types(), ["C minor"]);
    key(&mut app, &ctx, egui::Key::ArrowDown, true, false);
    assert_eq!(
        app.chords.catalog.selected_types(),
        ["C minor", "C diminished"]
    );
    key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(app.chords.catalog.selected_types().is_empty());
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.))),
            time: Some(ctx.input(|input| input.time) + 0.7),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    app.select_track(midi, false);
    app.session.project.transport.playhead_frame = 48_000;
    let point = entry(&app, 3);
    click(&mut app, &ctx, point, false);
    assert!(app.session.project.tracks[0].clips.is_empty());
    click(&mut app, &ctx, point, false);
    wait_for_save(&mut app);
    assert_eq!(app.session.project.tracks[0].clips.len(), 1);
    assert_eq!(app.session.project.tracks[0].clips[0].name, "C augmented");
    assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 48_000);
    app.select_track(audio, false);
    let before = serde_json::to_string(&app.session.project).unwrap();
    let point = entry(&app, 4);
    click(&mut app, &ctx, point, false);
    click(&mut app, &ctx, point, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), before);
}

#[test]
fn chord_fast_drag_release_uses_preview_position_and_shift_bypasses_snapping() {
    for shift in [false, true] {
        let (mut app, ctx, midi, _) = setup();
        let start = entry(&app, 0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let point = app.lane_bounds[&midi].min + Vec2::new(app.zoom * 0.02, 30.);
        let raw = app
            .session
            .project
            .frames(f64::from((point.x - app.timeline_bounds.left()) / app.zoom));
        let modifiers = egui::Modifiers {
            shift,
            ..Default::default()
        };
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                },
            ],
        );
        wait_for_save(&mut app);
        let clip = &app.session.project.tracks[0].clips[0];
        if shift {
            assert_eq!(clip.start_frame, raw);
        } else {
            assert_ne!(clip.start_frame, raw);
        }
        assert!(
            (app.chords.bounds.preview.last().unwrap().left()
                - app
                    .clip_block(app.lane_bounds[&midi], clip, clip.start_frame)
                    .left())
            .abs()
                < 0.01
        );
    }
}

#[test]
fn chord_header_drops_use_frame_zero_when_timeline_is_scrolled() {
    let (mut app, ctx, midi, _) = setup();
    app.scroll = 5.0;
    frame(&mut app, &ctx, vec![]);
    let point = Pos2::new(
        app.timeline_bounds.left() - 40.,
        app.lane_bounds[&midi].center().y,
    );
    begin(&mut app, &ctx, 0, point);
    release(&mut app, &ctx, point);
    assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
}

#[test]
fn chord_sampler_edits_are_visible_and_failure_keeps_pending_edits_and_previous_audio() {
    for fail in [false, true] {
        let (mut app, ctx, midi, _) = setup();
        let layer = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.sampler = Some(Default::default());
        let layer_id = layer.id;
        let file = TestWav::new();
        app.session = app
            .session
            .with_sampler_file(midi, layer_id, &file.0)
            .unwrap();
        app.history = None;
        let before = serde_json::to_string(&app.session.project).unwrap();
        frame(&mut app, &ctx, vec![]);
        let point = app.lane_bounds[&midi].center();
        begin(&mut app, &ctx, 0, point);
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(app.job.is_none() && app.background_media.required);
        assert_ne!(serde_json::to_string(&app.session.project).unwrap(), before);
        assert!(app.selected_clip.is_some() && app.dirty);
        if fail {
            app.background_media.worker.as_ref().unwrap().cancel();
            let pending = app.session.project.clone();
            app.finish_background_media(Err(daw_project::Error(
                "Injected chord preparation failure".into(),
            )));
            assert_eq!(app.session.project, pending);
            assert!(app.selected_clip.is_some() && app.dirty);
            assert!(app.background_media.error.is_some());
            assert!(!app.media_ready());
        } else {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while app.background_media.running {
                assert!(
                    std::time::Instant::now() < deadline,
                    "chord preparation timed out"
                );
                std::thread::yield_now();
                app.poll();
            }
            assert!(app.media_ready(), "{:?}", app.background_media.error);
            let sampler = app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .sampler
                .as_ref()
                .unwrap()
                .id;
            let offsets = app.session.sampler_audio[&sampler]
                .variants()
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            assert_eq!(offsets, [0, 4, 7]);
            assert_eq!(app.session.project.tracks[0].clips.len(), 1);
            assert!(app.selected_clip.is_some() && app.dirty);
        }
    }
}

#[test]
fn chord_clicks_and_keyboard_selection_audition_only_the_selected_midi_track() {
    let (mut app, ctx, midi, audio) = setup();
    let point = entry(&app, 0);
    click(&mut app, &ctx, point, false);
    assert!(app.chords.audition.requests.is_empty());
    app.selected_track = Some(audio);
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
    assert!(app.chords.audition.requests.is_empty());
    app.selected_track = Some(midi);
    let before = serde_json::to_string(&app.session.project).unwrap();
    let dirty = app.dirty;
    let point = entry(&app, 2);
    click(&mut app, &ctx, point, false);
    assert_eq!(
        app.chords.audition.requests.last(),
        Some(&(
            midi,
            daw_core::Chord {
                root: 0,
                kind: daw_core::ChordKind::Diminished
            }
        ))
    );
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
    assert_eq!(
        app.chords.audition.requests.last(),
        Some(&(
            midi,
            daw_core::Chord {
                root: 0,
                kind: daw_core::ChordKind::Augmented
            }
        ))
    );
    assert_eq!(app.chords.audition.requests.len(), 2);
    assert_eq!(serde_json::to_string(&app.session.project).unwrap(), before);
    assert_eq!(app.dirty, dirty);
    let count = app.chords.audition.requests.len();
    let point = egui::pos2(
        app.timeline_bounds.left() + 50.0,
        app.timeline_bounds.bottom() - 30.0,
    );
    begin(&mut app, &ctx, 3, point);
    release(&mut app, &ctx, point);
    assert_eq!(
        app.chords.audition.requests.len(),
        count,
        "dragging does not audition"
    );
}

#[test]
fn chord_audition_prepares_sampler_pitches_without_publishing_them() {
    let (mut app, _, midi, _) = setup();
    let file = TestWav::new();
    let layer = &mut app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.sampler = Some(Default::default());
    let layer_id = layer.id;
    app.session = app
        .session
        .with_sampler_file(midi, layer_id, &file.0)
        .unwrap();
    let project = serde_json::to_string(&app.session.project).unwrap();
    let prepared = app.session.sampler_audio.clone();
    let samples = app
        .session
        .audition_chord(
            midi,
            daw_core::Chord {
                root: 0,
                kind: daw_core::ChordKind::Major,
            },
            &|| false,
        )
        .unwrap();
    assert!(samples.iter().flatten().any(|s| s.abs() > 0.001));
    let note = app.session.audition_note(midi, 67, &|| false).unwrap();
    assert!(note.iter().flatten().any(|s| s.abs() > 0.001));
    let mut held = app.session.held_note_audition(midi, 67, &|| false).unwrap();
    assert!(
        held.next_chunk(false)
            .unwrap()
            .iter()
            .flatten()
            .any(|s| s.abs() > 0.001)
    );
    assert_eq!(
        serde_json::to_string(&app.session.project).unwrap(),
        project
    );
    for (id, before) in &prepared {
        let after = &app.session.sampler_audio[id];
        assert_eq!(
            before
                .variants()
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>(),
            after
                .variants()
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn editor_chords_share_catalog_actions_target_and_overview_drag_clock() {
    let (mut app, ctx, midi, audio) = setup();
    let clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
    app.open_clip_editor(clip, midi, &ctx);
    // Editor follows the active clip even if another view changed track selection.
    app.selected_track = Some(audio);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.zoom = 80.0;
    app.midi_editor.scroll = 3.0;
    app.zoom = 240.0;
    app.scroll = 0.0;
    frame(&mut app, &ctx, vec![]);
    let point = app.chords.bounds.toggle.center();
    click(&mut app, &ctx, point, false);
    assert!(app.chords_visible());
    assert!(!app.effects_catalog_visible());
    assert!(!app.effects_bounds.clip_settings_toggle.is_positive());
    assert_eq!(
        app.chords.bounds.toggle.top() - app.effects_bounds.library_toggle.bottom(),
        7.0
    );
    let before = app.session.project.clone();
    let target = app.timeline_bounds.min + Vec2::new(80.0, 30.0);
    begin(&mut app, &ctx, 0, target);
    assert_eq!(app.session.project, before);
    assert!(!app.chords.bounds.preview.is_empty());
    release(&mut app, &ctx, target);
    assert_eq!(
        app.session
            .project
            .tracks
            .iter()
            .find(|t| t.id == midi)
            .unwrap()
            .clips
            .len(),
        2
    );
    let added = app
        .session
        .project
        .tracks
        .iter()
        .find(|t| t.id == midi)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.id != clip)
        .unwrap();
    assert_eq!(added.start_frame, 192_000); // 3 seconds scroll + 1 second at 80 px/s.
    assert!(
        app.session
            .project
            .tracks
            .iter()
            .find(|t| t.id == audio)
            .unwrap()
            .clips
            .is_empty()
    );
    frame(&mut app, &ctx, vec![]);
    let library = app.effects_bounds.library_toggle.center();
    click(&mut app, &ctx, library, false);
    assert!(!app.chords_visible() && app.effects_catalog_visible());
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.chords.bounds.toggle.is_positive());
}
