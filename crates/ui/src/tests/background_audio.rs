use super::*;

fn pending_sampler() -> (DawUi, TestWav, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let layer = &mut app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.oscillators.clear();
    layer.sampler = Some(Default::default());
    let layer = layer.id;
    let file = TestWav::new();
    app.session = app
        .session
        .with_sampler_file(track, layer, &file.0)
        .unwrap();
    let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
    app.ensure_history();
    (app, file, clip)
}
fn pitch_edit(app: &mut DawUi, clip: Id, pitch: u8) {
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: daw_core::MidiPattern {
            length_ticks: 3840,
            notes: vec![daw_core::MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 960,
                pitch,
                velocity: 100,
            }],
        },
    });
}
fn wait_audio(app: &mut DawUi) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.background_media.running {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            app.background_media.error
        );
        app.poll();
        std::thread::yield_now();
    }
    assert!(app.media_ready(), "{:?}", app.background_media.error);
}
#[test]
fn rapid_midi_edits_publish_only_latest_audio_and_keep_history_and_selection() {
    let (mut app, _file, clip) = pending_sampler();
    pitch_edit(&mut app, clip, 64);
    pitch_edit(&mut app, clip, 67);
    assert!(app.job.is_none() && app.background_media.required && app.dirty);
    app.select_clip(clip, app.session.project.tracks[0].id, false);
    let before = app.session.project.clone();
    let revision = app.change_revision;
    wait_audio(&mut app);
    assert_eq!(app.session.project, before);
    assert_eq!(app.change_revision, revision);
    assert_eq!(app.selected_clip, Some(clip));
    let bank = app.session.sampler_audio.values().next().unwrap();
    assert_eq!(
        bank.variants().map(|(pitch, _)| pitch).collect::<Vec<_>>(),
        [0, 7]
    );
    let history = app.history.as_mut().unwrap();
    let previous = history.undo(&app.session).unwrap();
    assert_eq!(
        previous
            .project
            .assets
            .iter()
            .find_map(|a| a.midi_pattern.as_ref().filter(|p| !p.notes.is_empty()))
            .unwrap()
            .notes[0]
            .pitch,
        64
    );
}
#[test]
fn failed_audio_keeps_edits_dirty_and_undo_remains_available() {
    let (mut app, _file, clip) = pending_sampler();
    let before = app.session.project.clone();
    pitch_edit(&mut app, clip, 65);
    app.background_media.worker.as_ref().unwrap().cancel();
    let edited = app.session.project.clone();
    app.finish_background_media(Err(daw_project::Error("Injected failure".into())));
    assert_eq!(app.session.project, edited);
    assert!(app.dirty && app.background_media.required);
    assert!(app.error.is_none());
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    assert!(app.history_action_enabled(HistoryAction::Undo, &ctx));
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tracks, before.tracks);
    assert!(!app.reconcile_background_media());
    assert!(app.media_ready());
}
#[test]
fn save_records_pending_editor_revision_without_waiting_for_audio() {
    let (mut app, _file, clip) = pending_sampler();
    let folder = std::env::temp_dir().join(format!("daw-pending-save-{}", Id::new_v4()));
    app.session.save(&folder).unwrap();
    app.session.folder = Some(folder.clone());
    pitch_edit(&mut app, clip, 69);
    app.background_media.worker.as_ref().unwrap().cancel();
    app.background_media.running = false;
    app.save_snapshot(folder.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while app.regular_save.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_regular_save();
        std::thread::yield_now();
    }
    let saved = daw_project::inspect_project(&folder).unwrap();
    assert_eq!(saved.tracks, app.session.project.tracks);
    assert!(
        saved
            .assets
            .iter()
            .filter(|asset| asset.midi_pattern.is_some())
            .eq(app
                .session
                .project
                .assets
                .iter()
                .filter(|asset| asset.midi_pattern.is_some()))
    );
    assert!(app.background_media.required && !app.dirty);
    assert!(app.file_action_enabled(FileAction::Save));
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn loading_editor_is_interactive_but_play_and_source_operations_are_guarded() {
    let (mut app, _file, _) = pending_sampler();
    app.background_media.loading = true;
    app.background_media.required = true;
    assert!(app.editing_enabled());
    assert!(!app.media_ready());
    assert!(!app.file_action_enabled(FileAction::Import));
    assert!(!app.project_settings_enabled());
    app.play();
    assert!(app.output.is_none());
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    assert!(app.job.is_none());
    app.reset_background_media();
    assert!(app.media_ready());
}

#[test]
fn export_prepares_the_pending_snapshot_instead_of_rendering_the_previous_bank() {
    let (mut app, _file, clip) = pending_sampler();
    pitch_edit(&mut app, clip, 69);
    app.background_media.worker.as_ref().unwrap().cancel();
    app.background_media.running = false;
    let root = std::env::temp_dir().join(format!("daw-pending-export-{}", Id::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut reference = app.session.clone();
    reference
        .prepare_project_audio(&|| false, &|_, _| {})
        .unwrap();
    reference.export(&root.join("expected.wav"), false).unwrap();
    app.export(root.join("actual.wav"), false, ExportSettings::default());
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        std::thread::yield_now();
    }
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_eq!(
        std::fs::read(root.join("actual.wav")).unwrap(),
        std::fs::read(root.join("expected.wav")).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn undo_and_redo_after_loading_time_midi_edit_restore_audible_audio() {
    let (mut app, _file, clip) = pending_sampler();
    pitch_edit(&mut app, clip, 60);
    wait_audio(&mut app);
    let folder = std::env::temp_dir().join(format!("daw-loading-history-{}", Id::new_v4()));
    app.session.save(&folder).unwrap();
    app.reset_background_media();
    app.session = Session::open_document(&folder)
        .unwrap()
        .into_editor_session();
    app.history = None;
    app.ensure_history();
    app.background_media.loading = true;
    app.background_media.required = true;
    pitch_edit(&mut app, clip, 64);
    wait_audio(&mut app);

    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    for (action, expected_pitch) in [(HistoryAction::Undo, 60), (HistoryAction::Redo, 64)] {
        assert!(app.history_action_enabled(action, &ctx));
        app.perform_history_action(action, &ctx);
        assert!(!app.media_ready() || !app.session.audio.is_empty());
        frame(&mut app, &ctx, vec![]);
        assert!(!app.media_ready() || !app.session.audio.is_empty());
        let revision = app.change_revision;
        wait_audio(&mut app);
        assert_eq!(app.change_revision, revision);
        let asset_id = app.session.project.tracks[0].clips[0].asset_id;
        let pattern = app
            .session
            .project
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap();
        assert_eq!(pattern.notes[0].pitch, expected_pitch);
        let mut output = [[0.0; 2]; 4096];
        daw_engine::Renderer::new(app.session.plan()).render_offline(&mut output);
        assert!(output.iter().flatten().any(|sample| *sample != 0.0));
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn pending_midi_guards_audio_and_mixed_transpose_without_changing_history() {
    let (mut app, file, midi_clip) = pending_sampler();
    let audio_clip = app.session.import(&file.0, None, 0).unwrap();
    app.history = None;
    app.ensure_history();
    pitch_edit(&mut app, midi_clip, 65);

    // Running, paused and failed preparation all retain the same audio edit guard.
    for state in 0..3 {
        if state == 1 {
            app.background_media.worker.as_ref().unwrap().cancel();
            app.background_media.running = false;
        } else if state == 2 {
            app.finish_background_media(Err(daw_project::Error("Injected failure".into())));
        }
        for selection in [vec![audio_clip], vec![audio_clip, midi_clip]] {
            let before = app.session.project.clone();
            let revision = app.change_revision;
            app.edit(Edit::Batch(
                selection
                    .into_iter()
                    .map(|clip_id| Edit::SetClipTranspose {
                        clip_id,
                        transpose: 7,
                    })
                    .collect(),
            ));
            assert_eq!(app.session.project, before);
            assert_eq!(app.change_revision, revision);
            assert!(app.job.is_none());
            assert!(app.background_media.required);
        }
    }

    // Metadata edits stay available and retain the pending MIDI revision.
    let track = &app.session.project.tracks[0];
    let layer = track.instrument.as_ref().unwrap().layers[0].id;
    app.edit(Edit::SetLayerEffects {
        track_id: track.id,
        layer_id: layer,
        effects: vec![daw_core::EffectInstance::drive()],
        compact: false,
    });
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .effects
            .len(),
        1
    );
    app.edit(Edit::SetClipGain {
        clip_id: audio_clip,
        gain_db: -6.0,
    });
    app.edit(Edit::SetClipName {
        clip_id: audio_clip,
        name: "Renamed".into(),
    });
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_ne!(
        app.session.project.tracks.last().unwrap().clips[0].name,
        "Renamed"
    );
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert_eq!(
        app.session.project.tracks.last().unwrap().clips[0].name,
        "Renamed"
    );
    wait_audio(&mut app);

    // Once ready, the entire audio/MIDI selection follows the staged transaction.
    let before = app.session.project.clone();
    app.edit(Edit::Batch(
        [audio_clip, midi_clip]
            .into_iter()
            .map(|clip_id| Edit::SetClipTranspose {
                clip_id,
                transpose: 7,
            })
            .collect(),
    ));
    assert!(app.job.is_some());
    assert_eq!(app.session.project, before);
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        std::thread::yield_now();
    }
    assert!(app.error.is_none(), "{:?}", app.error);
    for clip_id in [audio_clip, midi_clip] {
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == clip_id)
                .unwrap()
                .transpose,
            7
        );
    }
}

#[test]
fn pending_audio_transpose_control_is_disabled_but_gain_remains_editable() {
    for editor in [false, true] {
        let (mut app, file, midi_clip) = pending_sampler();
        let audio_clip = app.session.import(&file.0, None, 0).unwrap();
        let track = app.session.project.tracks.last().unwrap().id;
        let ctx = context();
        app.select_clip(audio_clip, track, false);
        if editor {
            app.open_clip_editor(audio_clip, track, &ctx);
        } else {
            app.clip_settings_visible = true;
        }
        pitch_edit(&mut app, midi_clip, 65);
        app.background_media.worker.as_ref().unwrap().cancel();
        app.background_media.running = false;
        frame(&mut app, &ctx, vec![]);
        for (bounds, editable) in [
            (app.effects_bounds.clip_transpose, false),
            (app.effects_bounds.clip_gain, true),
        ] {
            assert!(bounds.is_positive());
            let point = bounds.center();
            for _ in 0..2 {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                frame(&mut app, &ctx, vec![button(point, false)]);
            }
            assert_eq!(app.clip_settings_draft.is_some(), editable);
        }
        assert!(app.job.is_none());
        assert!(app.background_media.required);
    }
}

#[test]
fn pending_modifier_stretches_preserve_project_history_and_selection_in_both_views() {
    for editor in [false, true] {
        for mixed in [false, true] {
            for state in 0..3 {
                let mut app = DawUi::default();
                let file = TestWav::new();
                let audio_clip = app.session.import(&file.0, None, 0).unwrap();
                let audio_track = app.session.project.tracks[0].id;
                let midi_track = app.session.project.add_midi_track().unwrap();
                let midi_clip = app.session.project.add_midi_pattern(midi_track, 0).unwrap();
                let folder =
                    std::env::temp_dir().join(format!("daw-pending-stretch-{}", Id::new_v4()));
                app.session.save(&folder).unwrap();
                std::fs::write(&file.0, b"invalid audio").unwrap();
                app.session = Session::open_document(&folder)
                    .unwrap()
                    .into_editor_session();
                app.ensure_history();
                let ctx = context();
                app.select_clip(audio_clip, audio_track, false);
                if editor {
                    app.open_clip_editor(audio_clip, audio_track, &ctx);
                    app.midi_editor.zoom = 120.0;
                    app.midi_editor.scroll = 0.0;
                } else {
                    app.zoom = 120.0;
                    app.scroll = 0.0;
                }
                if mixed {
                    app.select_clip(midi_clip, midi_track, false);
                    app.select_clip(audio_clip, audio_track, true);
                }
                // Hold each pending state without starting a real decoder. The
                // invalid source must never reach the ordinary stretch worker.
                app.background_media.required = true;
                app.background_media.loading = true;
                app.background_media.running = state == 0;
                app.background_media.error = (state == 2).then(|| "Injected failure".into());
                app.sync_needed = false;
                frame(&mut app, &ctx, vec![]);
                let before = app.session.project.clone();
                let revision = app.change_revision;
                let selection = app.selected_clips.clone();
                let clip = &app.session.project.tracks[0].clips[0];
                let block = if editor {
                    app.midi_editor.clip_bounds[&audio_clip]
                } else {
                    app.clip_block(app.lane_bounds[&audio_track], clip, clip.start_frame)
                };
                let origin = Pos2::new(block.right() - 1.0, block.top() + 10.0);
                let end = origin + Vec2::new(30.0, 0.0);
                let modifiers = egui::Modifiers {
                    alt: cfg!(target_os = "macos"),
                    ctrl: !cfg!(target_os = "macos"),
                    shift: true,
                    ..Default::default()
                };
                let press = |pos, pressed| egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers,
                };
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::PointerMoved(origin),
                        press(origin, true),
                    ],
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(
                    app.drag.as_ref().map(|drag| drag.mode),
                    Some(ClipDragMode::StretchRight)
                );
                frame(&mut app, &ctx, vec![press(end, false)]);
                assert!(app.job.is_none());
                assert_eq!(app.session.project, before);
                assert_eq!(app.change_revision, revision);
                assert_eq!(app.selected_clips, selection);
                assert!(!app.history.as_ref().unwrap().can_undo());
                assert!(!app.dirty);
                assert!(
                    app.notices
                        .iter()
                        .any(|notice| notice.contains("Wait for project audio"))
                );
                assert!(
                    app.session
                        .prepare_project_audio(&|| false, &|_, _| {})
                        .is_err()
                );
                std::fs::remove_dir_all(folder).unwrap();
            }
        }
    }
}

#[test]
fn midi_only_modifier_stretch_retains_its_pending_edit_and_prepares_normally() {
    let (mut app, _file, clip) = pending_sampler();
    pitch_edit(&mut app, clip, 65);
    app.background_media.worker.as_ref().unwrap().cancel();
    app.background_media.running = false;
    let track = app.session.project.tracks[0].id;
    app.select_clip(clip, track, false);
    let entry = app.selected_clip_snapshots()[0].clone();
    let original = entry.clip.length_frames;
    let drag = Drag {
        clip: entry.clip,
        track,
        mode: ClipDragMode::StretchRight,
        origin: Pos2::ZERO,
        duplicate: false,
        linked_boundary: false,
        clips: app.selected_clip_snapshots(),
    };
    app.finish_clip_stretch(&drag, Pos2::new(app.zoom * 0.25, 0.0), true);
    assert!(app.job.is_some());
    assert_eq!(
        app.session.project.tracks[0].clips[0].length_frames,
        original
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        std::thread::yield_now();
    }
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_ne!(
        app.session.project.tracks[0].clips[0].length_frames,
        original
    );
    assert!(!app.session.audio_preparation_needed(&app.session.project));
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    let pattern = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap();
    assert_eq!(pattern.notes[0].pitch, 65);
}
