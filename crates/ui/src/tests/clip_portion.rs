use super::*;

fn modifiers_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::CursorIcon {
    let events = events
        .into_iter()
        .map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            },
            other => other,
        })
        .collect::<Vec<_>>();
    let mut events = events;
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

fn x_at(app: &DawUi, lane: Rect, frame: u64) -> f32 {
    lane.left()
        + ((app.session.project.seconds(frame) - app.view_scroll()) as f32) * app.view_zoom()
}

#[test]
fn command_drag_selects_clips_and_silence_for_copy_move_and_delete() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
    let first = app.session.project.tracks[0].clips[0].id;
    let template = app.session.project.tracks[0].clips[0].clone();
    let second = Id::new_v4();
    app.session.project.tracks[0].clips.push(Clip {
        id: second,
        name: "Later".into(),
        start_frame: frames(4.0),
        length_frames: frames(1.0),
        source_offset_frame: 0,
        ..template
    });
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let start = Pos2::new(x_at(&app, lane, frames(1.0)), lane.top() + 40.0);
    let end = Pos2::new(x_at(&app, lane, frames(4.5)), lane.top() + 40.0);
    let command = egui::Modifiers::COMMAND;
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    let cursor = modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    assert!(app.drag.is_none());
    assert!(app.portion_drag.is_some());
    assert_eq!(cursor, egui::CursorIcon::Crosshair);
    assert!(!app.dirty);
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    let portion = app.portion.clone().expect("time range");
    assert_eq!(portion.tracks, vec![track]);
    assert_eq!(portion.start_frame, frames(1.0));
    assert_eq!(portion.end_frame, frames(4.5));
    assert!(app.selected_clips.is_empty());
    assert!(!app.dirty);

    app.perform_clip_action(ClipAction::Copy, &ctx);
    let clipboard = app.clip_clipboard.as_ref().unwrap();
    assert_eq!(clipboard.anchor_frame, Some(frames(1.0)));
    assert_eq!(clipboard.clips.len(), 2);
    assert_eq!(clipboard.clips[0].clip.length_frames, frames(1.0));
    assert_eq!(clipboard.clips[0].clip.source_offset_frame, frames(1.0));
    assert_eq!(clipboard.clips[1].clip.start_frame, frames(4.0));
    assert_eq!(clipboard.clips[1].clip.length_frames, frames(0.5));
    assert!(
        app.session.project.tracks[0]
            .clips
            .iter()
            .any(|clip| clip.id == first)
    );

    let from = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
    let to = Pos2::new(x_at(&app, lane, frames(6.5)), lane.top() + 40.0);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            egui::Event::PointerMoved(from),
            button(from, true),
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            egui::Event::PointerMoved(to),
        ],
    );
    assert!(app.drag.is_none());
    assert!(app.portion_drag.is_some());
    assert_eq!(app.session.project.tracks[0].clips.len(), 2);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            button(to, false),
        ],
    );
    assert!(app.error.is_none(), "{:?}", app.error);
    let clips = &app.session.project.tracks[0].clips;
    assert!(clips.iter().any(|clip| {
        clip.id == first && clip.start_frame == 0 && clip.length_frames == frames(1.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id == second && clip.start_frame == frames(4.5) && clip.length_frames == frames(0.5)
    }));
    assert!(clips.iter().any(|clip| {
        clip.start_frame == frames(6.0)
            && clip.length_frames == frames(1.0)
            && clip.source_offset_frame == frames(1.0)
    }));
    assert_eq!(app.portion.as_ref().unwrap().start_frame, frames(6.0));

    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    let clips = &app.session.project.tracks[0].clips;
    assert_eq!(clips.len(), 2);
    assert!(clips.iter().any(|clip| {
        clip.id == first && clip.start_frame == 0 && clip.length_frames == frames(1.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id == second && clip.start_frame == frames(4.5) && clip.length_frames == frames(0.5)
    }));
    assert!(app.portion.is_none());
}

#[test]
fn option_drag_duplicates_the_range_and_editor_uses_the_same_gesture() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
    let original = app.session.project.tracks[0].clips[0].id;
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let command = egui::Modifiers::COMMAND;
    let start = Pos2::new(x_at(&app, lane, frames(0.5)), lane.top() + 40.0);
    let end = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    assert_eq!(
        app.portion
            .as_ref()
            .map(|portion| (portion.start_frame, portion.end_frame)),
        Some((frames(0.5), frames(1.5)))
    );

    let copy = egui::Modifiers {
        alt: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    let from = Pos2::new(x_at(&app, lane, frames(1.0)), lane.top() + 40.0);
    let to = Pos2::new(x_at(&app, lane, frames(6.0)), lane.top() + 40.0);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        copy,
    );
    modifiers_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], copy);
    modifiers_frame(&mut app, &ctx, vec![button(to, false)], copy);
    assert!(app.error.is_none(), "{:?}", app.error);
    let clips = &app.session.project.tracks[0].clips;
    assert!(clips.iter().any(|clip| {
        clip.id == original && clip.start_frame == 0 && clip.length_frames == frames(2.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id != original
            && clip.start_frame == frames(5.5)
            && clip.length_frames == frames(1.0)
            && clip.source_offset_frame == frames(0.5)
    }));

    app.selected_track = Some(track);
    app.set_view_mode(ViewMode::Edit, &ctx);
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let start = Pos2::new(x_at(&app, lane, frames(0.5)), lane.center().y);
    let end = Pos2::new(x_at(&app, lane, frames(1.0)), lane.center().y);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    assert_eq!(app.portion.as_ref().unwrap().tracks, vec![track]);
    assert_eq!(app.portion.as_ref().unwrap().start_frame, frames(0.5));
    assert_eq!(app.portion.as_ref().unwrap().end_frame, frames(1.0));
}

#[test]
fn range_overwrite_previews_match_release_and_escape_preserves_project() {
    for duplicate in [false, true] {
        for (start, length, expected) in [(4.0, 4.0, 2), (5.5, 2.0, 1), (5.0, 1.0, 0)] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.session.project.tracks[0].clips[0].length_frames = frames(3.0);
            let mut neighbor = app.session.project.tracks[0].clips[0].clone();
            neighbor.id = Id::new_v4();
            neighbor.start_frame = frames(start);
            neighbor.length_frames = frames(length);
            let neighbor_id = neighbor.id;
            app.session.project.tracks[0].clips.push(neighbor);
            frame(&mut app, &ctx, vec![]);
            app.portion = Some(super::super::portion::ClipPortion {
                tracks: vec![track],
                start_frame: frames(1.0),
                end_frame: frames(2.0),
            });
            let lane = app.lane_bounds[&track];
            let from = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
            let copy = egui::Modifiers {
                alt: duplicate && cfg!(target_os = "macos"),
                ctrl: duplicate && !cfg!(target_os = "macos"),
                ..Default::default()
            };
            modifiers_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from), button(from, true)],
                copy,
            );
            let to = Pos2::new(x_at(&app, lane, frames(5.5)), from.y);
            modifiers_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], copy);
            let drag = app.portion_drag.as_ref().unwrap();
            let piece = app
                .session
                .project
                .clip_range_slices(&[track], frames(1.0), frames(2.0))
                .unwrap()
                .remove(0)
                .clip;
            let incoming = vec![ClipPreview {
                track,
                track_index: 0,
                clip: Clip {
                    start_frame: frames(5.0),
                    ..piece
                },
                valid: true,
                removed: false,
            }];
            let display = app.portion_previews(drag, &incoming, duplicate);
            assert!(display.iter().any(|p| p.clip.id == neighbor_id));
            let survivors: Vec<_> = display
                .iter()
                .skip(1)
                .filter(|p| !p.removed && p.clip.start_frame >= frames(4.0))
                .collect();
            assert_eq!(survivors.len(), expected);
            let original = app.session.project.clone();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
            modifiers_frame(&mut app, &ctx, vec![button(from, false)], copy);
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
            app.portion = Some(super::super::portion::ClipPortion {
                tracks: vec![track],
                start_frame: frames(1.0),
                end_frame: frames(2.0),
            });
            let to = Pos2::new(x_at(&app, lane, frames(5.5)), from.y);
            modifiers_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from), button(from, true)],
                copy,
            );
            modifiers_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], copy);
            modifiers_frame(&mut app, &ctx, vec![button(to, false)], copy);
            assert!(app.error.is_none(), "{:?}", app.error);
            let retained: Vec<_> = app.session.project.tracks[0]
                .clips
                .iter()
                .filter(|c| {
                    c.start_frame >= frames(4.0)
                        && !(c.start_frame == frames(5.0) && c.length_frames == frames(1.0))
                })
                .map(|c| (c.start_frame, c.length_frames, c.source_offset_frame))
                .collect();
            let painted: Vec<_> = survivors
                .iter()
                .map(|p| {
                    (
                        p.clip.start_frame,
                        p.clip.length_frames,
                        p.clip.source_offset_frame,
                    )
                })
                .collect();
            assert_eq!(
                retained,
                painted,
                "duplicate={duplicate} neighbor={start}..{}",
                start + length
            );
        }
    }
}

#[test]
fn incompatible_range_moves_and_copies_cancel_without_a_dialog_or_edit() {
    for source_midi in [false, true] {
        for group_collapsed in [None, Some(false), Some(true)] {
            for controls in [false, true] {
                for duplicate in [false, true] {
                    let (mut app, audio) = fixture();
                    let source = if source_midi {
                        let source = app.session.project.add_midi_track().unwrap();
                        app.session.project.add_midi_pattern(source, 0).unwrap();
                        source
                    } else {
                        audio
                    };
                    let target = if let Some(collapsed) = group_collapsed {
                        let child = app.session.project.add_track().unwrap();
                        let group = Id::new_v4();
                        app.session
                            .project
                            .edit(Edit::GroupTracks {
                                track_ids: vec![child],
                                group_id: group,
                                name: "Group".into(),
                            })
                            .unwrap();
                        app.session
                            .project
                            .edit(Edit::SetGroupCollapsed {
                                track_id: group,
                                collapsed,
                            })
                            .unwrap();
                        group
                    } else if source_midi {
                        audio
                    } else {
                        app.session.project.add_midi_track().unwrap()
                    };
                    let ctx = context();
                    app.selected_track = Some(source);
                    frame(&mut app, &ctx, vec![]);
                    app.portion = Some(super::super::portion::ClipPortion {
                        tracks: vec![source],
                        start_frame: frames(0.0),
                        end_frame: frames(0.5),
                    });
                    app.selected_clip = None;
                    app.selected_clips.clear();
                    let before = app.session.project.clone();
                    let source_lane = app.lane_bounds[&source];
                    let from = Pos2::new(
                        x_at(&app, source_lane, frames(0.25)),
                        source_lane.top() + 40.0,
                    );
                    let target_lane = app.lane_bounds[&target];
                    let to = Pos2::new(
                        if controls {
                            app.timeline_bounds.left() - 100.0
                        } else {
                            x_at(&app, target_lane, frames(4.5))
                        },
                        target_lane.center().y,
                    );
                    let modifiers = egui::Modifiers {
                        alt: duplicate && cfg!(target_os = "macos"),
                        ctrl: duplicate && !cfg!(target_os = "macos"),
                        ..Default::default()
                    };
                    modifiers_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(from), button(from, true)],
                        modifiers,
                    );
                    let cursor = modifiers_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(to)],
                        modifiers,
                    );
                    assert!(app.portion_drag.is_some());
                    assert_eq!(
                        cursor,
                        egui::CursorIcon::NotAllowed,
                        "midi={source_midi}, group={group_collapsed:?}, controls={controls}, copy={duplicate}"
                    );
                    assert_eq!(app.session.project, before);
                    modifiers_frame(&mut app, &ctx, vec![button(to, false)], modifiers);
                    assert!(app.error.is_none(), "{:?}", app.error);
                    assert_eq!(app.session.project, before);
                    assert_eq!(app.selected_track, Some(source));
                    assert!(app.selected_clip.is_none() && app.selected_clips.is_empty());
                    let portion = app.portion.as_ref().unwrap();
                    assert_eq!(portion.tracks, vec![source]);
                    assert_eq!(
                        (portion.start_frame, portion.end_frame),
                        (frames(0.0), frames(0.5))
                    );
                    assert!(app.portion_drag.is_none() && app.job.is_none());
                    assert!(!app.dirty);
                    assert!(!app.history.as_ref().unwrap().can_undo());
                }
            }
        }
    }
}

#[test]
fn range_drop_rejects_all_members_when_only_the_secondary_target_is_incompatible() {
    for duplicate in [false, true] {
        let (mut app, source) = fixture();
        let second = app.session.project.add_track().unwrap();
        let mut clip = app.session.project.tracks[0].clips[0].clone();
        clip.id = Id::new_v4();
        app.session.project.tracks[1].clips.push(clip);
        app.session.project.add_midi_track().unwrap();
        let ctx = context();
        app.selected_track = Some(source);
        frame(&mut app, &ctx, vec![]);
        app.portion = Some(super::super::portion::ClipPortion {
            tracks: vec![source, second],
            start_frame: frames(1.0),
            end_frame: frames(2.0),
        });
        app.selected_clip = None;
        app.selected_clips.clear();
        let before = app.session.project.clone();
        let lane = app.lane_bounds[&source];
        let from = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
        let target = app.lane_bounds[&second];
        let to = Pos2::new(x_at(&app, target, frames(4.5)), target.center().y);
        let modifiers = egui::Modifiers {
            alt: duplicate && cfg!(target_os = "macos"),
            ctrl: duplicate && !cfg!(target_os = "macos"),
            ..Default::default()
        };
        modifiers_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(from), button(from, true)],
            modifiers,
        );
        let cursor = modifiers_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(to)],
            modifiers,
        );
        assert_eq!(cursor, egui::CursorIcon::NotAllowed);
        modifiers_frame(&mut app, &ctx, vec![button(to, false)], modifiers);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.session.project, before);
        assert_eq!(app.selected_track, Some(source));
        assert!(app.selected_clip.is_none() && app.selected_clips.is_empty());
        let portion = app.portion.as_ref().unwrap();
        assert_eq!(portion.tracks, vec![source, second]);
        assert_eq!(
            (portion.start_frame, portion.end_frame),
            (frames(1.0), frames(2.0))
        );
        assert!(!app.dirty && app.job.is_none());
        assert!(!app.history.as_ref().unwrap().can_undo());
    }
}

#[test]
fn range_drag_recovers_from_an_invalid_target_before_release() {
    for duplicate in [false, true] {
        let (mut app, source) = fixture();
        let invalid = app.session.project.add_midi_track().unwrap();
        let valid = app.session.project.add_track().unwrap();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        app.portion = Some(super::super::portion::ClipPortion {
            tracks: vec![source],
            start_frame: frames(1.0),
            end_frame: frames(2.0),
        });
        let lane = app.lane_bounds[&source];
        let from = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
        let to = |app: &DawUi, track| {
            let lane = app.lane_bounds[&track];
            Pos2::new(x_at(app, lane, frames(4.5)), lane.center().y)
        };
        let invalid_point = to(&app, invalid);
        let valid_point = to(&app, valid);
        let modifiers = egui::Modifiers {
            alt: duplicate && cfg!(target_os = "macos"),
            ctrl: duplicate && !cfg!(target_os = "macos"),
            ..Default::default()
        };
        modifiers_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(from), button(from, true)],
            modifiers,
        );
        assert_eq!(
            modifiers_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(invalid_point)],
                modifiers
            ),
            egui::CursorIcon::NotAllowed
        );
        let cursor = modifiers_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(valid_point)],
            modifiers,
        );
        assert_eq!(
            cursor,
            if duplicate {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            }
        );
        modifiers_frame(&mut app, &ctx, vec![button(valid_point, false)], modifiers);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .find(|track| track.id == valid)
                .unwrap()
                .clips
                .len(),
            1
        );
        assert_eq!(app.portion.as_ref().unwrap().tracks, vec![valid]);
        assert!(app.dirty && app.history.as_ref().unwrap().can_undo());
    }
}
