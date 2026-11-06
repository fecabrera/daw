use super::*;
use daw_project::{NormalizeMode, NormalizeOptions, NormalizeProgress};

fn playable() -> (DawUi, Id) {
    let (mut app, track) = fixture();
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.length_frames = 4800;
    let id = clip.id;
    let asset = clip.asset_id;
    app.session.audio.insert(
        asset,
        daw_media::AudioData {
            sample_rate_hz: 48000,
            samples: std::sync::Arc::new(vec![[0.25, -0.125]; 4800]),
            disk: None,
            metadata: app.session.project.assets[0].source_metadata.clone(),
            peaks: Default::default(),
        },
    );
    app.session.project.assets[0].decoded_frame_count = 4800;
    app.select_clip(id, track, false);
    (app, id)
}
fn wait(app: &mut DawUi) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn normalize_menu_dialog_runs_without_a_completion_modal_and_undo_restores_gain() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let (mut app, id) = playable();
        let ctx = context();
        app.set_view_mode(view, &ctx);
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        app.perform_clip_menu_action(ClipMenuAction::Normalize, &ctx);
        assert!(app.normalize_dialog.is_some());
        assert!(!app.file_action_enabled(FileAction::Save));
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let point = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Normalize" => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, pressed)],
            );
        }
        assert!(app.normalize_dialog.is_none());
        assert!(app.normalize_progress.is_some());
        assert!(
            app.normalization_status_text()
                .unwrap()
                .to_lowercase()
                .contains("normaliz")
        );
        assert!(!app.file_action_enabled(FileAction::Open));
        wait(&mut app);
        assert!(app.normalize_progress.is_none());
        assert!(
            app.error.is_none() && app.export_status.is_none() && app.separation_status.is_none()
        );
        assert!(app.session.project.tracks[0].clips[0].gain_db > 11.0);
        assert_eq!(app.selected_clip, Some(id));
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
    }
}
#[test]
fn normalize_failure_and_cancellation_leave_project_and_history_unchanged() {
    let (mut app, id) = playable();
    let before = app.session.project.clone();
    app.start_normalization(
        vec![id],
        NormalizeOptions {
            mode: NormalizeMode::Integrated,
            ..Default::default()
        },
    );
    wait(&mut app);
    assert!(app.error.as_ref().unwrap().contains("400 ms"));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    app.error = None;
    let progress = NormalizeProgress::default();
    progress.cancel.store(true, Ordering::Relaxed);
    app.normalize_progress = Some(progress);
    app.finish_normalization(Ok(vec![Edit::SetClipGain {
        clip_id: id,
        gain_db: 10.0,
    }]));
    assert_eq!(app.session.project, before);
    assert!(app.error.is_none());
    assert!(!app.dirty);
}
#[test]
fn normalize_menu_guards_mixed_midi_selection_and_modal_cancel() {
    let (mut app, id) = playable();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.perform_clip_menu_action(ClipMenuAction::Normalize, &ctx);
    assert!(app.normalize_dialog.is_some());
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
    assert!(app.normalize_dialog.is_none() && app.job.is_none());
    assert!(!app.dirty);
    app.session.project.assets[0].midi_pattern = Some(Default::default());
    assert!(!app.clip_menu_action_enabled(ClipMenuAction::Normalize, &ctx));
    assert!(!app.normalization_targets_are_audio(&[id, Id::new_v4()]));
}

#[test]
fn normalize_status_bar_cancel_retains_receiver_until_acknowledgement() {
    let (mut app, id) = playable();
    let ctx = context();
    let before = app.session.project.clone();
    let progress = NormalizeProgress::default();
    progress.total.store(100, Ordering::Relaxed);
    progress.done.store(50, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    app.job = Some(rx);
    app.settings_cancel = Some(progress.cancel.clone());
    app.normalize_progress = Some(progress.clone());
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Normalizing gain… 50%")
    ));
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Cancel" => {
                Some(t.pos + t.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, pressed)],
        );
    }
    assert!(progress.cancel.load(Ordering::Relaxed));
    assert!(app.job.is_some());
    assert!(!app.file_action_enabled(FileAction::Open));
    tx.send(Ok(Job::Normalized(vec![Edit::SetClipGain {
        clip_id: id,
        gain_db: 10.0,
    }])))
    .unwrap();
    app.poll();
    assert!(app.job.is_none() && app.normalize_progress.is_none());
    assert!(app.error.is_none());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}
