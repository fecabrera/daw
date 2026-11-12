use super::*;
use daw_core::BounceMode;
fn playable() -> (DawUi, Id, Id) {
    let (mut app, track) = fixture();
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.length_frames = 2400;
    let id = clip.id;
    let asset = clip.asset_id;
    app.session.project.assets[0].decoded_frame_count = 2400;
    app.session.audio.insert(
        asset,
        daw_media::AudioData {
            sample_rate_hz: 48000,
            samples: std::sync::Arc::new(vec![[0.1, -0.2]; 2400]),
            disk: None,
            metadata: app.session.project.assets[0].source_metadata.clone(),
            peaks: Default::default(),
        },
    );
    app.select_clip(id, track, false);
    (app, track, id)
}
fn wait(app: &mut DawUi) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        app.poll_bounce_publication();
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn key(modifiers: egui::Modifiers, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::B,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}
#[test]
fn success_auto_closes_after_complete_transaction_and_undo_is_one_step() {
    let (mut app, track, clip) = playable();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let original = app.session.project.clone();
    app.perform_clip_action(ClipAction::Bounce, &ctx);
    assert!(app.bounce_status.is_some());
    assert!(!app.file_action_enabled(FileAction::Save));
    assert!(!app.editing_enabled());
    wait(&mut app);
    assert!(app.bounce_status.is_none());
    assert!(app.error.is_none());
    assert_eq!(app.session.project.tracks.len(), 2);
    assert_eq!(app.selected_track, Some(track));
    assert_eq!(app.selected_clip, Some(clip));
    assert!(app.bounce_input_blocked);
    frame(&mut app, &ctx, vec![]);
    assert!(app.editing_enabled());
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, original);
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert_eq!(app.session.project.tracks.len(), 2);
}
#[test]
fn cancel_keeps_lock_until_worker_acknowledges_then_closes_without_history() {
    let (mut app, _, _) = playable();
    let before = app.session.project.clone();
    app.start_bounce(BounceMode::Clips, None, &context());
    app.bounce_status
        .as_ref()
        .unwrap()
        .progress
        .cancel
        .store(true, Ordering::Release);
    assert!(!app.editing_enabled());
    wait(&mut app);
    assert!(app.bounce_status.is_none());
    assert!(app.error.is_none());
    assert_eq!(app.session.project, before);
    assert!(!app.history.as_ref().unwrap().can_undo());
}
#[test]
fn errors_remain_in_one_dialog_and_do_not_publish_or_unlock() {
    let (mut app, _, _) = playable();
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    std::sync::Arc::make_mut(&mut app.session.audio.get_mut(&asset).unwrap().samples)[100][0] =
        f32::NAN;
    let before = app.session.project.clone();
    app.start_bounce(BounceMode::Tracks, None, &context());
    wait(&mut app);
    assert!(matches!(
        app.bounce_status.as_ref().unwrap().state,
        job_report::State::Failed(_)
    ));
    assert!(app.error.is_none());
    assert_eq!(app.session.project, before);
    assert!(!app.file_action_enabled(FileAction::Open));
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(
        shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Bounce failed"))
    );
}
#[test]
fn exact_shortcuts_reject_extras_repeat_and_shift_fallthrough() {
    for modifiers in [
        egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT | egui::Modifiers::ALT,
    ] {
        let (mut app, _, _) = playable();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![key(modifiers, false)]);
        assert!(app.bounce_status.is_none());
    }
    let (mut app, _, _) = playable();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    ctx.input_mut(|i| i.events = vec![key(egui::Modifiers::COMMAND, true)]);
    app.bounce_shortcuts(&ctx);
    assert!(app.bounce_status.is_none());
    app.selected_track = None;
    app.selected_tracks.clear();
    frame(
        &mut app,
        &ctx,
        vec![key(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            false,
        )],
    );
    assert!(app.bounce_status.is_none());
}
#[test]
fn completion_unlocks_while_held_keys_remain_suppressed() {
    let (mut app, _, _) = playable();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.start_bounce(BounceMode::Clips, None, &ctx);
    let result = take_worker_result(&mut app);
    let delete = |pressed, repeat| egui::Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed,
        repeat,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut app, &ctx, vec![delete(true, false)]); // Held while the progress guard owns editing.
    app.finish_bounce(result);
    let count = app
        .session
        .project
        .tracks
        .iter()
        .map(|t| t.clips.len())
        .sum::<usize>();
    frame(&mut app, &ctx, vec![delete(true, true)]);
    assert!(!app.bounce_input_blocked);
    assert!(app.editing_enabled());
    assert_eq!(
        app.session
            .project
            .tracks
            .iter()
            .map(|t| t.clips.len())
            .sum::<usize>(),
        count
    );
    frame(
        &mut app,
        &ctx,
        vec![delete(false, false), delete(true, false)],
    );
    assert!(
        app.session
            .project
            .tracks
            .iter()
            .map(|t| t.clips.len())
            .sum::<usize>()
            < count
    );
}
#[test]
fn bounce_shortcut_does_not_keep_the_workspace_disabled_until_key_release() {
    let (mut app, _, _) = playable();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![key(egui::Modifiers::COMMAND, false)]);
    wait(&mut app);
    assert!(app.bounce_status.is_none());
    frame(&mut app, &ctx, vec![key(egui::Modifiers::COMMAND, true)]);
    assert!(app.editing_enabled());
    assert!(app.bounce_status.is_none());
    assert_eq!(app.session.project.tracks.len(), 2);
}

fn take_worker_result(
    app: &mut DawUi,
) -> Result<(daw_project::PreparedBounce, daw_project::SessionHistory), daw_project::BounceError> {
    let job = app
        .job
        .take()
        .unwrap()
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let Job::Bounced(result) = job else {
        panic!("Expected bounce result")
    };
    *result
}
#[test]
fn late_cancel_waits_for_file_cleanup_and_stale_completion_cannot_publish() {
    for cancel in [true, false] {
        let (mut app, _, _) = playable();
        let ctx = context();
        app.start_bounce(BounceMode::Clips, None, &ctx);
        let result = take_worker_result(&mut app);
        let path = PathBuf::from(
            &result
                .as_ref()
                .unwrap()
                .0
                .session
                .project
                .assets
                .last()
                .unwrap()
                .source
                .path,
        );
        assert!(path.exists());
        assert!(!app.editing_enabled()); // Status, not the dropped job receiver, owns this guard.
        if cancel {
            app.bounce_status
                .as_ref()
                .unwrap()
                .progress
                .cancel
                .store(true, Ordering::Release);
        } else {
            app.session.project.name = "Changed generation".into();
        }
        let original = app.session.project.clone();
        app.finish_bounce(result);
        assert!(app.job.is_some());
        assert!(!app.editing_enabled());
        wait(&mut app);
        assert!(!path.exists());
        assert_eq!(app.session.project, original);
        if cancel {
            assert!(app.bounce_status.is_none());
        } else {
            assert!(matches!(
                app.bounce_status.as_ref().unwrap().state,
                job_report::State::Failed(_)
            ));
        }
    }
}
#[test]
fn a_late_cancel_token_does_not_disguise_a_real_worker_error() {
    let (mut app, _, _) = playable();
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    std::sync::Arc::make_mut(&mut app.session.audio.get_mut(&asset).unwrap().samples)[20][0] =
        f32::INFINITY;
    app.start_bounce(BounceMode::Clips, None, &context());
    let result = take_worker_result(&mut app);
    assert!(matches!(result, Err(daw_project::BounceError::Failed(_))));
    app.bounce_status
        .as_ref()
        .unwrap()
        .progress
        .cancel
        .store(true, Ordering::Release);
    app.finish_bounce(result);
    assert!(matches!(
        app.bounce_status.as_ref().unwrap().state,
        job_report::State::Failed(_)
    ));
    assert!(app.error.is_none());
}

#[test]
fn window_close_remains_blocked_after_the_worker_receiver_clears() {
    let (mut app, _, _) = playable();
    let ctx = context();
    app.start_bounce(BounceMode::Clips, None, &ctx);
    let result = take_worker_result(&mut app);
    assert!(app.job.is_none());
    assert!(app.bounce_status.is_some());
    app.dirty = true;
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(!app.allow_close);
    assert!(!app.unsaved_prompt);
    assert!(app.pending.is_none());
    app.finish_bounce(result);
}
