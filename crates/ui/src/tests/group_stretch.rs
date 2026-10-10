use super::*;

fn fixture_with_audio() -> (DawUi, [Id; 3], [Id; 3]) {
    let (mut app, tracks, ids) = multi_clip_fixture();
    app.session.project.assets[0].decoded_frame_count = frames(2.0);
    app.session.audio.insert(
        app.session.project.assets[0].id,
        daw_media::AudioData {
            samples: std::sync::Arc::new(vec![[0.2, -0.2]; frames(2.0) as usize]),
            peaks: std::sync::Arc::new(vec![[[-0.2, 0.2]; 2]; 375]),
            metadata: app.session.project.assets[0].source_metadata.clone(),
        },
    );
    (app, tracks, ids)
}

fn modifiers(shift: bool) -> egui::Modifiers {
    egui::Modifiers {
        alt: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    }
}

fn pointer_button(pos: Pos2, pressed: bool, shift: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: modifiers(shift),
    }
}

fn begin(
    app: &mut DawUi,
    ctx: &egui::Context,
    left: bool,
    header: bool,
    shift: bool,
    seconds_delta: f32,
) -> (Pos2, Vec<egui::epaint::ClippedShape>) {
    frame(app, ctx, vec![]);
    let entry = app.selected_clip_snapshots()[0].clone();
    let block = app.clip_block(
        app.lane_bounds[&entry.track],
        &entry.clip,
        entry.clip.start_frame,
    );
    let origin = Pos2::new(
        if left {
            block.left() + 1.0
        } else {
            block.right() - 1.0
        },
        block.top() + if header { 10.0 } else { 40.0 },
    );
    let end = origin + Vec2::new(seconds_delta * app.zoom, 0.0);
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers(shift)),
            egui::Event::PointerMoved(origin),
            pointer_button(origin, true, shift),
        ],
    );
    let shapes = frame_shapes(
        app,
        ctx,
        vec![egui::Event::PointerMoved(end)],
        Vec2::new(1280.0, 800.0),
    );
    (end, shapes)
}

#[test]
fn modifier_edges_stretch_all_selected_clips_after_release_and_processing() {
    for left in [false, true] {
        for header in [false, true] {
            for shift in [false, true] {
                let (mut app, _, ids) = fixture_with_audio();
                let ctx = context();
                select_group(&mut app, &ctx, ids);
                let before = app.selected_clip_snapshots();
                let project = format!("{:?}", app.session.project);
                let original_audio = app.session.audio[&before[0].clip.asset_id].samples.clone();
                let (end, shapes) = begin(
                    &mut app,
                    &ctx,
                    left,
                    header,
                    shift,
                    if left { -0.24 } else { 0.24 },
                );
                let drag = app.drag.as_ref().unwrap();
                assert_eq!(drag.clips.len(), 3);
                assert_eq!(
                    drag.mode,
                    if left {
                        ClipDragMode::StretchLeft
                    } else {
                        ClipDragMode::StretchRight
                    }
                );
                let previews = app.clip_drag_previews(drag, end, shift);
                assert_eq!(previews.len(), 3);
                let ratio = if shift { 1.48 } else { 1.5 };
                for (preview, original) in previews.iter().zip(&before) {
                    assert!(preview.valid);
                    assert_eq!(
                        preview.clip.length_frames,
                        (original.clip.length_frames as f64 * ratio).round() as u64
                    );
                    assert_eq!(
                        if left {
                            preview.clip.end()
                        } else {
                            preview.clip.start_frame
                        },
                        if left {
                            original.clip.end()
                        } else {
                            original.clip.start_frame
                        }
                    );
                    assert_eq!(preview.clip.asset_id, original.clip.asset_id);
                    assert_eq!(preview.clip.color, original.clip.color);
                    assert_eq!(
                        preview.clip.source_offset_frame,
                        (original.clip.source_offset_frame as f64 * ratio).round() as u64
                    );
                    if let Some(repeat) = original.clip.repeat {
                        let next = preview.clip.repeat.unwrap();
                        assert_eq!(
                            next.length_frames,
                            (repeat.length_frames as f64 * ratio).round() as u64
                        );
                        assert_eq!(
                            next.phase_frame,
                            (repeat.phase_frame as f64 * ratio).round() as u64
                        );
                    }
                    assert!(has_preview_outline(
                        &shapes,
                        app.clip_block(
                            app.lane_bounds[&preview.track],
                            &preview.clip,
                            preview.clip.start_frame
                        ),
                        theme::ACCENT
                    ));
                }
                assert_eq!(format!("{:?}", app.session.project), project);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![pointer_button(end, false, shift)]);
                assert!(app.job.is_some());
                assert_eq!(format!("{:?}", app.session.project), project);
                wait_for_save(&mut app);
                assert!(app.error.is_none(), "{:?}", app.error);
                assert!(app.dirty);
                for preview in previews {
                    let placed = app
                        .selected_clip_snapshots()
                        .into_iter()
                        .find(|entry| entry.clip.id == preview.clip.id)
                        .unwrap();
                    assert_eq!(format!("{:?}", placed.clip), format!("{:?}", preview.clip));
                    assert!(app.session.audio_for_clip(&placed.clip).is_some());
                }
                assert_eq!(app.selected_clips, ids.into_iter().collect());
                assert!(std::sync::Arc::ptr_eq(
                    &original_audio,
                    &app.session.audio[&before[0].clip.asset_id].samples
                ));
                app.session.project.validate().unwrap();
            }
        }
    }
}

#[test]
fn group_stretch_clamps_shared_ratio_at_each_members_limits() {
    let (mut app, _, ids) = fixture_with_audio();
    app.session.project.tracks[0].clips[1].stretch = daw_core::ClipStretch::new(1, 4);
    app.selected_clips = ids.into_iter().collect();
    app.selected_clip = Some(ids[0]);
    let entries = app.selected_clip_snapshots();
    let drag = Drag {
        clip: entries[0].clip.clone(),
        track: entries[0].track,
        mode: ClipDragMode::StretchRight,
        origin: Pos2::ZERO,
        duplicate: false,
        clips: entries.clone(),
    };
    let large = app.clip_drag_previews(&drag, Pos2::new(1e7, 0.0), true);
    assert!(large.iter().all(|preview| preview.valid));
    for (preview, original) in large.iter().zip(&entries) {
        assert_eq!(preview.clip.length_frames, original.clip.length_frames * 2);
    }
    assert_eq!(large[1].clip.stretch.unwrap().ratio(), 8.0);
    let small = app.clip_drag_previews(&drag, Pos2::new(-1e7, 0.0), true);
    assert!(small.iter().all(|preview| preview.valid));
    assert_eq!(
        small[0].clip.length_frames,
        entries[0].clip.length_frames / 8
    );
    let mut left = drag;
    left.mode = ClipDragMode::StretchLeft;
    // A reaches frame zero first and limits the shared expansion ratio to 5x.
    left.clips[1].clip.stretch = None;
    left.clips[1].clip.start_frame = frames(20.0);
    app.session.project.tracks[0].clips[1] = left.clips[1].clip.clone();
    let largest = app.clip_drag_previews(&left, Pos2::new(-1e7, 0.0), true);
    assert!(largest.iter().all(|preview| preview.valid));
    assert!(largest.iter().all(|preview| preview.clip.length_frames > 0));
    assert_eq!(largest[0].clip.start_frame, 0);
    assert_eq!(largest[0].clip.length_frames, frames(2.5));
    assert_eq!(largest[1].clip.length_frames, frames(3.75));
}

#[test]
fn group_stretch_overlap_trims_selected_and_unselected_neighbors_atomically() {
    for selected_neighbor in [false, true] {
        let (mut app, _, ids) = fixture_with_audio();
        // Expansion of A to one second would cover this touching neighbor.
        app.session.project.tracks[0].clips[1].start_frame = frames(2.75);
        let ctx = context();
        if selected_neighbor {
            select_group(&mut app, &ctx, ids);
        } else {
            select_group(&mut app, &ctx, [ids[0], ids[2]]);
        }
        let before = format!("{:?}", app.session.project);
        let (end, _) = begin(&mut app, &ctx, false, false, true, 0.5);
        let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
        assert!(previews.iter().all(|preview| preview.valid));
        let neighbor = previews
            .iter()
            .find(|preview| preview.clip.id == ids[1])
            .unwrap();
        assert!(!neighbor.removed);
        assert_eq!(neighbor.clip.start_frame, frames(3.0));
        assert_eq!(
            neighbor.clip.length_frames,
            frames(if selected_neighbor { 1.25 } else { 0.5 })
        );
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
        assert!(app.job.is_some());
        assert!(!app.dirty);
        assert_eq!(format!("{:?}", app.session.project), before);
        wait_for_save(&mut app);
        assert!(app.error.is_none());
        assert!(app.dirty);
        for preview in previews {
            let placed = app
                .session
                .project
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == preview.clip.id)
                .unwrap();
            assert_eq!(format!("{placed:?}"), format!("{:?}", preview.clip));
        }
        app.session.project.validate().unwrap();
    }
}

#[test]
fn group_stretch_cancel_noop_and_processing_failure_keep_all_originals() {
    for action in ["escape", "focus", "noop", "failure"] {
        let (mut app, _, ids) = fixture_with_audio();
        if action == "failure" {
            app.session.project.tracks[0].clips[0].stretch = daw_core::ClipStretch::new(1, 2);
            app.session.prepare_stretches().unwrap();
            let mut asset = app.session.project.assets[0].clone();
            asset.id = Id::new_v4();
            let mut audio = app.session.audio[&app.session.project.assets[0].id].clone();
            audio.samples = std::sync::Arc::new(vec![[f32::NAN; 2]; frames(2.0) as usize]);
            app.session.project.tracks[1].clips[0].asset_id = asset.id;
            app.session.audio.insert(asset.id, audio);
            app.session.project.assets.push(asset);
        }
        let ctx = context();
        select_group(&mut app, &ctx, ids);
        let original = format!("{:?}", app.session.project);
        let original_cache = app.session.stretched_audio.clone();
        let (end, _) = begin(&mut app, &ctx, false, false, true, 0.25);
        let end = if action == "noop" {
            let origin = app.drag.as_ref().unwrap().origin;
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(origin)]);
            origin
        } else {
            end
        };
        if action == "escape" {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: modifiers(true),
                }],
            );
        } else if action == "focus" {
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
        frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
        if action == "failure" {
            assert!(app.job.is_some());
            wait_for_save(&mut app);
            assert!(app.error.as_ref().unwrap().contains("invalid samples"));
        } else {
            assert!(app.job.is_none());
        }
        assert_eq!(format!("{:?}", app.session.project), original);
        assert_eq!(app.session.stretched_audio.len(), original_cache.len());
        for (key, audio) in original_cache {
            assert!(std::sync::Arc::ptr_eq(
                &audio.samples,
                &app.session.stretched_audio[&key].samples
            ));
        }
        assert!(!app.dirty);
        assert_eq!(app.selected_clips, ids.into_iter().collect());
    }
}

#[test]
fn group_stretch_composes_existing_ratios_and_reuses_original_sources() {
    let (mut app, _, ids) = fixture_with_audio();
    app.session.project.tracks[0].clips[1].stretch = daw_core::ClipStretch::new(1, 4);
    app.session.project.tracks[1].clips[0].stretch = daw_core::ClipStretch::new(1, 2);
    app.session.prepare_stretches().unwrap();
    let ctx = context();
    select_group(&mut app, &ctx, ids);
    let original_audio = app.session.audio[&app.session.project.assets[0].id]
        .samples
        .clone();
    let original_cache = app.session.stretched_audio.clone();
    let (end, _) = begin(&mut app, &ctx, false, true, true, -0.25);
    let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
    assert!(previews.iter().all(|preview| preview.valid));
    assert_eq!(previews[0].clip.stretch.unwrap().ratio(), 0.5);
    assert_eq!(previews[1].clip.stretch.unwrap().ratio(), 2.0);
    assert_eq!(previews[2].clip.stretch, None);
    let old_two = original_cache[&(previews[1].clip.asset_id, previews[1].clip.stretch.unwrap())]
        .samples
        .clone();
    frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
    assert_eq!(app.session.stretched_audio.len(), original_cache.len());
    wait_for_save(&mut app);
    assert!(app.error.is_none());
    for preview in previews {
        let placed = app
            .selected_clip_snapshots()
            .into_iter()
            .find(|entry| entry.clip.id == preview.clip.id)
            .unwrap();
        assert_eq!(format!("{:?}", placed.clip), format!("{:?}", preview.clip));
        let resolved = app.session.audio_for_clip(&placed.clip).unwrap();
        if placed.clip.id == ids[1] {
            assert!(std::sync::Arc::ptr_eq(&old_two, &resolved.samples));
        } else if placed.clip.id == ids[2] {
            assert!(std::sync::Arc::ptr_eq(&original_audio, &resolved.samples));
        }
    }
    assert_eq!(app.session.stretched_audio.len(), 2);
    assert_eq!(app.selected_clips, ids.into_iter().collect());
}

#[test]
fn group_stretch_rounds_unequal_lengths_without_timeline_overflow() {
    let (mut app, _, ids) = multi_clip_fixture();
    app.session.project.tracks[0].clips[0].start_frame = 100;
    app.session.project.tracks[0].clips[0].length_frames = 13;
    app.session.project.tracks[0].clips[1].start_frame = u64::MAX - 32;
    app.session.project.tracks[0].clips[1].length_frames = 29;
    app.session.project.tracks[1].clips[0].length_frames = 13;
    app.session.project.tracks[1].clips[0].repeat = None;
    app.selected_clips = ids.into_iter().collect();
    app.selected_clip = Some(ids[0]);
    let entries = app.selected_clip_snapshots();
    let drag = Drag {
        clip: entries[0].clip.clone(),
        track: entries[0].track,
        mode: ClipDragMode::StretchRight,
        origin: Pos2::ZERO,
        duplicate: false,
        clips: entries,
    };
    let previews = app.clip_drag_previews(&drag, Pos2::new(1e7, 0.0), true);
    assert!(previews.iter().all(|preview| preview.valid));
    assert_eq!(previews[0].clip.length_frames, 14);
    assert_eq!(previews[1].clip.length_frames, 31);
    assert_eq!(previews[1].clip.end(), u64::MAX - 1);
    assert_eq!(previews[2].clip.length_frames, 14);
    let before = format!("{:?}", app.session.project);
    assert!(!app.dirty);
    assert_eq!(app.session.project.tracks[0].clips[1].length_frames, 29);
    let stationary = app.clip_drag_previews(&drag, drag.origin, false);
    assert!(stationary.iter().all(|preview| preview.valid));
    assert_eq!(stationary[0].clip.length_frames, 13);
    assert_eq!(format!("{:?}", app.session.project), before);
}

fn neighbor_fixture(left: bool) -> (DawUi, Id, Clip, Clip, Clip) {
    let (mut app, tracks, _) = fixture_with_audio();
    let mut primary = app.session.project.tracks[0].clips[0].clone();
    primary.start_frame = frames(3.0);
    primary.length_frames = frames(1.0);
    primary.source_offset_frame = frames(0.125);
    let partial = Clip {
        id: Id::new_v4(),
        start_frame: frames(if left { 0.0 } else { 6.0 }),
        length_frames: frames(2.0),
        source_offset_frame: frames(0.25),
        repeat: Some(daw_core::ClipLoop {
            length_frames: frames(0.5),
            phase_frame: frames(0.125),
        }),
        stretch: daw_core::ClipStretch::new(1, 2),
        ..primary.clone()
    };
    let covered = Clip {
        id: Id::new_v4(),
        start_frame: frames(if left { 2.5 } else { 5.0 }),
        length_frames: frames(0.25),
        source_offset_frame: 0,
        ..primary.clone()
    };
    app.session.project.tracks[0].clips = vec![primary.clone(), partial.clone(), covered.clone()];
    app.session.project.validate().unwrap();
    app.session.prepare_stretches().unwrap();
    (app, tracks[0], primary, partial, covered)
}

#[test]
fn stretch_neighbor_previews_commit_trims_and_removals_after_release_and_processing() {
    for left in [false, true] {
        for header in [false, true] {
            let (mut app, track, primary, partial, covered) = neighbor_fixture(left);
            let ctx = context();
            select_group(&mut app, &ctx, [primary.id]);
            let original = format!("{:?}", app.session.project);
            let (end, shapes) = begin(
                &mut app,
                &ctx,
                left,
                header,
                true,
                if left { -1.5 } else { 3.0 },
            );
            let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
            assert_eq!(previews.len(), 3);
            assert!(previews.iter().all(|preview| preview.valid));
            let trimmed = previews
                .iter()
                .find(|preview| preview.clip.id == partial.id)
                .unwrap();
            assert!(!trimmed.removed);
            assert_eq!(trimmed.clip.stretch, partial.stretch);
            assert_eq!(
                trimmed.clip.length_frames,
                frames(if left { 1.5 } else { 1.0 })
            );
            let advance = trimmed.clip.start_frame - partial.start_frame;
            for local in 0..trimmed.clip.length_frames {
                assert_eq!(
                    trimmed.clip.source_frame(local),
                    partial.source_frame(local + advance)
                );
            }
            assert!(has_preview_outline(
                &shapes,
                app.clip_block(
                    app.lane_bounds[&track],
                    &trimmed.clip,
                    trimmed.clip.start_frame
                ),
                theme::ACCENT
            ));
            assert!(
                previews
                    .iter()
                    .find(|preview| preview.clip.id == covered.id)
                    .unwrap()
                    .removed
            );
            assert_eq!(format!("{:?}", app.session.project), original);
            let origin = app.drag.as_ref().unwrap().origin;
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(origin)]);
            assert!(
                app.clip_drag_previews(app.drag.as_ref().unwrap(), origin, true)
                    .iter()
                    .all(|preview| !preview.removed)
            );
            assert_eq!(format!("{:?}", app.session.project), original);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
            assert!(app.job.is_some());
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
            wait_for_save(&mut app);
            assert!(app.error.is_none());
            for preview in previews {
                let result = app.session.project.tracks[0]
                    .clips
                    .iter()
                    .find(|clip| clip.id == preview.clip.id);
                if preview.removed {
                    assert!(result.is_none());
                } else {
                    assert_eq!(
                        format!("{:?}", result.unwrap()),
                        format!("{:?}", preview.clip)
                    );
                }
            }
            assert_eq!(app.selected_clips, HashSet::from([primary.id]));
            assert!(app.dirty);
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn stretch_neighbor_cancellation_and_worker_failure_leave_the_whole_project_unchanged() {
    for left in [false, true] {
        for action in ["escape", "focus", "failure"] {
            let (mut app, _, primary, _, covered) = neighbor_fixture(left);
            if action == "failure" {
                let audio = app.session.audio.get_mut(&primary.asset_id).unwrap();
                audio.samples = std::sync::Arc::new(vec![[f32::NAN; 2]; frames(2.0) as usize]);
            }
            let ctx = context();
            select_group(&mut app, &ctx, [primary.id]);
            let before = format!("{:?}", app.session.project);
            let cache = app.session.stretched_audio.clone();
            let (end, _) = begin(
                &mut app,
                &ctx,
                left,
                false,
                true,
                if left { -1.5 } else { 3.0 },
            );
            assert!(
                app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true)
                    .iter()
                    .find(|preview| preview.clip.id == covered.id)
                    .unwrap()
                    .removed
            );
            if action == "escape" {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: modifiers(true),
                    }],
                );
            } else if action == "focus" {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        events: vec![egui::Event::WindowFocused(false)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
            frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
            if action == "failure" {
                assert!(app.job.is_some());
                wait_for_save(&mut app);
                assert!(app.error.as_ref().unwrap().contains("invalid samples"));
            } else {
                assert!(app.job.is_none());
            }
            assert_eq!(format!("{:?}", app.session.project), before);
            assert!(!app.dirty);
            assert_eq!(app.session.stretched_audio.len(), cache.len());
            for (key, audio) in cache {
                assert!(std::sync::Arc::ptr_eq(
                    &audio.samples,
                    &app.session.stretched_audio[&key].samples
                ));
            }
        }
    }
}

#[test]
fn group_stretch_removes_fully_covered_selected_clips_and_prunes_selection() {
    let (mut app, _, ids) = fixture_with_audio();
    app.session.project.tracks[0].clips[1].start_frame = frames(2.75);
    app.session.project.tracks[0].clips[1].length_frames = frames(0.125);
    let ctx = context();
    select_group(&mut app, &ctx, ids);
    let before = format!("{:?}", app.session.project);
    let (end, _) = begin(&mut app, &ctx, false, true, true, 1.0);
    let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, true);
    assert!(
        previews
            .iter()
            .find(|preview| preview.clip.id == ids[1])
            .unwrap()
            .removed
    );
    assert_eq!(app.selected_clips, HashSet::from(ids));
    assert_eq!(format!("{:?}", app.session.project), before);
    frame(&mut app, &ctx, vec![pointer_button(end, false, true)]);
    assert_eq!(app.selected_clips, HashSet::from(ids));
    assert_eq!(format!("{:?}", app.session.project), before);
    wait_for_save(&mut app);
    assert!(app.error.is_none());
    assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[2]]));
    assert!(
        !app.session.project.tracks[0]
            .clips
            .iter()
            .any(|clip| clip.id == ids[1])
    );
    app.session.project.validate().unwrap();
}
