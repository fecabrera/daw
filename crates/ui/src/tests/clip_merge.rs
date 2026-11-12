use super::*;

fn merge_fixture(beat: bool) -> (DawUi, egui::Context, Vec<Id>) {
    let mut app = DawUi::default();
    let ctx = context();
    let track = if beat {
        app.session.project.add_beat_track().unwrap()
    } else {
        app.session.project.add_midi_track().unwrap()
    };
    let length = app
        .session
        .project
        .midi_frame(if beat { 3840 } else { 960 });
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = if beat {
            app.session
                .project
                .add_beat_pattern(track, i * length)
                .unwrap()
        } else {
            app.session
                .project
                .add_midi_pattern(track, i * length)
                .unwrap()
        };
        app.session.project.tracks[0]
            .clips
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .length_frames = length;
        ids.push(id);
        app.select_clip(id, track, i != 0);
    }
    frame(&mut app, &ctx, vec![]);
    (app, ctx, ids)
}

#[test]
fn non_contiguous_merge_retains_gaps_and_one_undo_step_in_both_views() {
    for beat in [false, true] {
        for view in [ViewMode::Timeline, ViewMode::Edit] {
            let (mut app, ctx, ids) = merge_fixture(beat);
            let length = app.session.project.tracks[0].clips[0].length_frames;
            for (clip, multiple) in app.session.project.tracks[0]
                .clips
                .iter_mut()
                .zip([1, 3, 6])
            {
                clip.start_frame = multiple * length;
            }
            app.session.project.validate().unwrap();
            app.set_view_mode(view, &ctx);
            app.history = None;
            app.ensure_history();
            app.dirty = false;
            let before = app.session.project.clone();
            assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
            let shortcut = ClipAction::Merge.shortcut();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: shortcut.logical_key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: shortcut.modifiers,
                }],
            );
            assert!(app.error.is_none(), "{:?}", app.error);
            let merged = app.session.project.clone();
            assert_eq!(merged.tracks[0].clips.len(), 1);
            assert_eq!(merged.tracks[0].clips[0].start_frame, length);
            assert_eq!(merged.tracks[0].clips[0].end(), 7 * length);
            assert_eq!(app.selected_clips, HashSet::from([ids[0]]));
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, before);
            app.perform_history_action(HistoryAction::Redo, &ctx);
            assert_eq!(app.session.project, merged);
        }
    }
}

#[test]
fn merge_shortcut_selects_result_and_undo_restores_all_clips() {
    for beat in [false, true] {
        let (mut app, ctx, ids) = merge_fixture(beat);
        let before = app.session.project.clone();
        assert!(app.clip_action_enabled(ClipAction::Merge, &ctx));
        let shortcut = ClipAction::Merge.shortcut();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: shortcut.logical_key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: shortcut.modifiers,
            }],
        );
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
        assert_eq!(app.selected_clip, Some(ids[0]));
        assert_eq!(app.selected_clips.len(), 1);
        assert!(app.dirty);
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
        app.perform_history_action(HistoryAction::Redo, &ctx);
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
    }
}

#[test]
fn merge_menu_and_context_command_share_targets_and_guards() {
    for beat in [false, true] {
        for view in [ViewMode::Timeline, ViewMode::Edit] {
            for contextual in [false, true] {
                let (mut app, ctx, ids) = merge_fixture(beat);
                app.set_view_mode(view, &ctx);
                assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
                let menu = |app: &mut DawUi, events| {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(800.0, 600.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            if contextual {
                                app.clip_context_menu(ui, ids[1]);
                            } else {
                                app.clip_menu(ui);
                            }
                        },
                    );
                    output.textures_delta.clear();
                    output
                };
                let output = menu(&mut app, vec![]);
                let point = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == "Merge" => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .unwrap();
                menu(
                    &mut app,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                menu(&mut app, vec![button(point, false)]);
                assert_eq!(app.session.project.tracks[0].clips.len(), 1);
                assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
            }
        }
        let (mut app, ctx, _) = merge_fixture(beat);
        for view in [ViewMode::Mixer, ViewMode::Instrument] {
            app.set_view_mode(view, &ctx);
            assert!(!app.clip_action_enabled(ClipAction::Merge, &ctx));
        }
        app.set_view_mode(ViewMode::Timeline, &ctx);
        app.error = Some("Blocked".into());
        app.perform_clip_action(ClipAction::Merge, &ctx);
        assert_eq!(app.session.project.tracks[0].clips.len(), 3);
    }
}

#[test]
fn beat_merge_menu_accepts_two_sample_rounding_gap() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let mut app = DawUi::default();
        let ctx = context();
        app.session.project.tempo_bpm = 130.0;
        let track = app.session.project.add_beat_track().unwrap();
        let first = app.session.project.add_beat_pattern(track, 0).unwrap();
        app.session
            .project
            .edit(Edit::ReplaceBeatPattern {
                clip_id: first,
                pattern: daw_core::BeatPattern {
                    bars: 2,
                    resolution: daw_core::BeatResolution::Eighth,
                    ..Default::default()
                },
            })
            .unwrap();
        app.session.project.tracks[0].clips[0].length_frames = 177_230;
        let mut next = app.session.project.tracks[0].clips[0].clone();
        next.id = Id::new_v4();
        next.start_frame = 177_232;
        let second = next.id;
        app.session.project.tracks[0].clips.push(next);
        app.session.project.validate().unwrap();
        app.select_clip(first, track, false);
        app.select_clip(second, track, true);
        app.set_view_mode(view, &ctx);
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
        assert!(app.clip_action_enabled(ClipAction::Merge, &ctx));
        app.perform_clip_menu_action(ClipMenuAction::Merge, &ctx);
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
        assert_eq!(
            app.session.project.tracks[0].clips[0].length_frames,
            354_462
        );
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn single_looped_or_trimmed_pattern_merges_from_shortcut_and_menus_with_undo() {
    for beat in [false, true] {
        for view in [ViewMode::Timeline, ViewMode::Edit] {
            for mode in 0..5 {
                for dispatch in 0..3 {
                    let (mut app, ctx, ids) = merge_fixture(beat);
                    let track = app.session.project.tracks[0].id;
                    app.session.project.tracks[0]
                        .clips
                        .retain(|c| c.id == ids[0]);
                    let clip = &mut app.session.project.tracks[0].clips[0];
                    let base = clip.length_frames;
                    match mode {
                        0 => {
                            clip.repeat = Some(daw_core::ClipLoop {
                                length_frames: base,
                                phase_frame: 0,
                            });
                            clip.length_frames = base * 3;
                        }
                        1 => clip.length_frames = base / 2,
                        2 => {
                            clip.source_offset_frame = base / 2;
                            clip.length_frames = base / 2;
                        }
                        3 => {
                            clip.source_offset_frame = base / 4;
                            clip.length_frames = base / 2;
                        }
                        _ => {}
                    }
                    if mode == 4 {
                        app.session.project = if beat {
                            let mut candidate = app.session.project.tracks[0].clips[0].clone();
                            candidate.length_frames = base / 2;
                            app.session
                                .project
                                .trimmed_clips(&[candidate], daw_core::ClipEdge::Right)
                                .unwrap()
                        } else {
                            app.session
                                .project
                                .resized_midi_pattern(ids[0], 0, 480)
                                .unwrap()
                        };
                        app.session.project = serde_json::from_str(
                            &serde_json::to_string(&app.session.project).unwrap(),
                        )
                        .unwrap();
                    }
                    app.select_clip(ids[0], track, false);
                    app.set_view_mode(view, &ctx);
                    app.history = None;
                    frame(&mut app, &ctx, vec![]);
                    let before = app.session.project.clone();
                    assert!(app.clip_action_enabled(ClipAction::Merge, &ctx));
                    assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
                    match dispatch {
                        0 => {
                            let shortcut = ClipAction::Merge.shortcut();
                            frame(
                                &mut app,
                                &ctx,
                                vec![egui::Event::Key {
                                    key: shortcut.logical_key,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: shortcut.modifiers,
                                }],
                            );
                        }
                        1 => app.perform_clip_menu_action(ClipMenuAction::Merge, &ctx),
                        _ => {
                            let menu = |app: &mut DawUi, events| {
                                let mut output = ctx.run_ui(
                                    egui::RawInput {
                                        screen_rect: Some(Rect::from_min_size(
                                            Pos2::ZERO,
                                            Vec2::new(800.0, 600.0),
                                        )),
                                        events,
                                        ..Default::default()
                                    },
                                    |ui| app.clip_context_menu(ui, ids[0]),
                                );
                                output.textures_delta.clear();
                                output
                            };
                            let output = menu(&mut app, vec![]);
                            let point = output
                                .shapes
                                .iter()
                                .find_map(|shape| match &shape.shape {
                                    egui::Shape::Text(text) if text.galley.text() == "Merge" => {
                                        Some(text.pos + text.galley.size() * 0.5)
                                    }
                                    _ => None,
                                })
                                .unwrap();
                            menu(
                                &mut app,
                                vec![egui::Event::PointerMoved(point), button(point, true)],
                            );
                            menu(&mut app, vec![button(point, false)]);
                        }
                    }
                    assert!(app.error.is_none(), "{:?}", app.error);
                    let after = app.session.project.clone();
                    let clip = &after.tracks[0].clips[0];
                    assert_eq!(clip.id, ids[0]);
                    assert_ne!(clip.asset_id, before.tracks[0].clips[0].asset_id);
                    assert_eq!(clip.length_frames, before.tracks[0].clips[0].length_frames);
                    assert!(clip.repeat.is_none());
                    assert_eq!(clip.source_offset_frame, 0);
                    assert_eq!(app.selected_clip, Some(ids[0]));
                    assert_eq!(app.selected_clips.len(), 1);
                    app.perform_history_action(HistoryAction::Undo, &ctx);
                    assert_eq!(app.session.project, before);
                    app.perform_history_action(HistoryAction::Redo, &ctx);
                    assert_eq!(app.session.project, after);
                }
            }
        }
    }
}

#[test]
fn single_full_unlooped_pattern_can_be_consolidated() {
    for beat in [false, true] {
        let (mut app, ctx, ids) = merge_fixture(beat);
        let track = app.session.project.tracks[0].id;
        app.select_clip(ids[0], track, false);
        assert!(app.clip_action_enabled(ClipAction::Merge, &ctx));
        assert!(app.clip_menu_action_enabled(ClipMenuAction::Merge, &ctx));
        let before = app.session.project.clone();
        app.perform_clip_action(ClipAction::Merge, &ctx);
        assert_eq!(
            app.session.project.tracks[0].clips.len(),
            before.tracks[0].clips.len()
        );
        assert_ne!(
            app.session.project.tracks[0].clips[0].asset_id,
            before.tracks[0].clips[0].asset_id
        );
        assert!(app.dirty);
    }
}
