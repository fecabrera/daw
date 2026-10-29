use super::*;
use std::{
    fs,
    time::{Duration, Instant},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("daw-ui-collect-{}", Id::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn app(&self) -> DawUi {
        let path = self.0.join("source.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..12000 {
            writer.write_sample(0.2f32).unwrap();
            writer.write_sample(-0.2f32).unwrap();
        }
        writer.finalize().unwrap();
        let mut app = DawUi::default();
        let clip = app.session.import(&path, None, 0).unwrap();
        app.selected_clip = Some(clip);
        app.selected_clips.insert(clip);
        app.selected_track = Some(app.session.project.tracks[0].id);
        app.session.save(&self.0.join("project")).unwrap();
        app.ensure_history();
        app.sync_needed = false;
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn wait(app: &mut DawUi) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        app.poll();
        assert!(Instant::now() < deadline, "collection worker timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    theme::configure(&ctx);
    fonts::configure(&ctx);
    icons::configure(&ctx);
    ctx
}
fn frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    out.shapes
}
fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn text_point(shapes: &[egui::epaint::ClippedShape], label: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing text: {label}"))
}
// Hold discovery at the UI boundary to inject cancellation/lifecycle changes
// deterministically before automatic execution starts.
fn preflight(app: &mut DawUi) -> (Id, CollectionPreflight) {
    app.perform_file_action(FileAction::CollectSources);
    let receiver = app.job.take().unwrap();
    let message = receiver
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    app.busy.clear();
    app.settings_cancel = None;
    match message {
        Job::CollectionPreflight(ticket, result) => (ticket, result.unwrap()),
        _ => panic!("expected collection preflight"),
    }
}
fn has_text(shapes: &[egui::epaint::ClippedShape], text: &str) -> bool {
    shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains(text)))
}
#[test]
fn action_requires_saved_idle_focused_and_settled_state() {
    let mut unsaved = DawUi::default();
    assert!(!unsaved.file_action_enabled(FileAction::CollectSources));
    unsaved.perform_file_action(FileAction::CollectSources);
    assert!(unsaved.job.is_none());
    let f = Fixture::new();
    let mut app = f.app();
    assert!(app.file_action_enabled(FileAction::CollectSources));
    assert!(FileAction::CollectSources.shortcut().is_none());
    for guard in 0..4 {
        app.collection_input_ready = guard != 0;
        app.settings_open = guard == 1;
        app.strip_dragging = guard == 2;
        app.unsaved_prompt = guard == 3;
        assert!(!app.file_action_enabled(FileAction::CollectSources));
    }
    app.unsaved_prompt = false;
    app.perform_file_action(FileAction::CollectSources);
    assert!(app.job.is_some());
    assert!(
        FileAction::ALL
            .into_iter()
            .all(|a| !app.file_action_enabled(a))
    );
    assert!(!app.edit_environment_available());
    wait(&mut app);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    assert!(app.collection_status.is_none());
    assert!(app.edit_environment_available());
}
#[test]
fn dirty_project_collects_immediately_without_prompts_and_preserves_selection_audio_and_transport()
{
    let f = Fixture::new();
    let mut app = f.app();
    app.session.project.name = "Unsaved edit".into();
    app.mark_dirty();
    let clip = app.selected_clip;
    let selected = app.selected_clips.clone();
    let track = app.selected_track;
    let id = app.session.project.assets[0].id;
    let pcm = app.session.audio[&id].samples.clone();
    app.session.project.transport.playhead_frame = 123;
    app.perform_file_action(FileAction::CollectSources);
    assert!(!app.unsaved_prompt);
    assert!(app.error.is_none());
    assert!(app.collection_status_text().unwrap().contains("Checking"));
    wait(&mut app);
    let status = app.collection_status.as_ref().unwrap();
    assert_eq!(status.report, job_report::State::Complete);
    assert_eq!(status.progress.unwrap().files_copied, 1);
    assert!(!app.dirty);
    assert!(!app.sync_needed);
    assert_eq!(app.selected_clip, clip);
    assert_eq!(app.selected_clips, selected);
    assert_eq!(app.selected_track, track);
    assert!(Arc::ptr_eq(&pcm, &app.session.audio[&id].samples));
    assert_eq!(app.session.project.transport.playhead_frame, 123);
    assert!(app.session.project.assets[0].source.is_included_recording());
    fs::remove_file(f.0.join("source.wav")).unwrap();
    let reopened = Session::open(app.session.folder.as_ref().unwrap()).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.project.name, "Unsaved edit");
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(has_text(&shapes, "Project sources collected and saved"));
    for label in [
        "Collect and save",
        "Source-collection status",
        "Close",
        "Save changes",
    ] {
        assert!(!has_text(&shapes, label), "unexpected dialog: {label}");
    }
    assert!(app.collection_status.is_none());
    assert!(app.edit_environment_available());
}
#[test]
fn missing_sources_open_only_an_error_dialog_with_affected_assets() {
    let f = Fixture::new();
    let mut app = f.app();
    let before = fs::read(f.0.join("project/project.json")).unwrap();
    fs::remove_file(f.0.join("source.wav")).unwrap();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    assert!(matches!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Failed(_)
    ));
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(has_text(&shapes, "Assets: source"));
    assert!(has_text(&shapes, "Source collection failed"));
    assert!(has_text(&shapes, "Close"));
    assert!(!has_text(&shapes, "Collect and save"));
    assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
    click(&mut app, &ctx, text_point(&shapes, "Close"));
    assert!(app.collection_status.is_none());
    assert!(app.edit_environment_available());
}
#[test]
fn cancellation_during_discovery_retains_manifest_and_dirty_state_without_dialog() {
    let f = Fixture::new();
    let mut app = f.app();
    app.session.project.name = "Dirty".into();
    app.mark_dirty();
    let model = app.session.project.clone();
    let before = fs::read(f.0.join("project/project.json")).unwrap();
    app.perform_file_action(FileAction::CollectSources);
    app.collection_status
        .as_ref()
        .unwrap()
        .lease
        .cancel
        .store(true, Ordering::Relaxed);
    wait(&mut app);
    assert_eq!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Cancelled
    );
    let ctx = context();
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(has_text(&shapes, "Source collection cancelled"));
    assert!(!has_text(&shapes, "Close"));
    assert!(app.collection_status.is_none());
    assert_eq!(app.session.project, model);
    assert!(app.dirty);
    assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
}
#[test]
fn stale_project_view_or_navigation_blocks_automatic_execution() {
    for change in [0, 1, 2] {
        let f = Fixture::new();
        let mut app = f.app();
        let (ticket, report) = preflight(&mut app);
        match change {
            0 => app.session = Session::default(),
            1 => app.view_mode = ViewMode::Mixer,
            _ => app.navigation_revision += 1,
        }
        let model = app.session.project.clone();
        app.refresh_collection();
        app.finish_collection_preflight(ticket, Ok(report));
        assert!(app.job.is_none());
        assert_eq!(app.session.project, model);
        assert!(matches!(
            app.collection_status.as_ref().unwrap().report,
            job_report::State::Failed(_)
        ));
    }
}
#[test]
fn repeated_collection_copies_nothing_and_does_not_add_history_edits() {
    let f = Fixture::new();
    let mut app = f.app();
    let ctx = context();
    for expected in [1, 0] {
        app.perform_file_action(FileAction::CollectSources);
        wait(&mut app);
        let status = app.collection_status.as_ref().unwrap();
        assert_eq!(status.report, job_report::State::Complete);
        assert_eq!(status.progress.unwrap().files_copied, expected);
        frame(&mut app, &ctx, vec![]);
        assert!(app.collection_status.is_none());
        assert!(!app.history.as_ref().unwrap().can_undo());
    }
    app.mark_dirty();
    assert!(app.collection_status_text().is_none());
}
#[test]
fn copy_failure_and_cancelled_worker_keep_original_manifest_and_live_buffers() {
    for cancelled in [true, false] {
        let f = Fixture::new();
        let mut app = f.app();
        let (ticket, report) = preflight(&mut app);
        let before = fs::read(f.0.join("project/project.json")).unwrap();
        let model = app.session.project.clone();
        // Start the sealed plan after discovery; inject the failure before polling.
        if !cancelled {
            fs::remove_file(f.0.join("source.wav")).unwrap();
        }
        if cancelled {
            app.collection_status.as_mut().unwrap().stage = Stage::Copying;
            app.collection_status
                .as_ref()
                .unwrap()
                .lease
                .cancel
                .store(true, Ordering::Relaxed);
            let result = report
                .into_plan()
                .unwrap()
                .execute(&|_| true, &|| true, &mut |_| Ok(()))
                .map_err(|e| e.to_string());
            app.finish_collection(ticket, result);
        } else {
            app.finish_collection_preflight(ticket, Ok(report));
        }
        wait(&mut app);
        let report = &app.collection_status.as_ref().unwrap().report;
        if cancelled {
            assert_eq!(*report, job_report::State::Cancelled);
        } else {
            assert!(matches!(report, job_report::State::Failed(_)));
        }
        assert_eq!(app.session.project, model);
        assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
    }
}
#[test]
fn late_cancel_and_notification_error_publish_commit_with_status_bar_warnings() {
    let f = Fixture::new();
    let mut app = f.app();
    let (ticket, report) = preflight(&mut app);
    app.collection_status.as_mut().unwrap().stage = Stage::Copying;
    let outcome = report
        .into_plan()
        .unwrap()
        .execute(&|_| true, &|| false, &mut |p| {
            if p.phase == CollectionPhase::Committed {
                Err(daw_project::Error("notification failed".into()))
            } else {
                Ok(())
            }
        })
        .unwrap();
    app.collection_status
        .as_ref()
        .unwrap()
        .lease
        .cancel
        .store(true, Ordering::Relaxed);
    app.session.project.transport.playhead_frame = 777;
    app.finish_collection(ticket, Ok(outcome));
    assert_eq!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Complete
    );
    assert!(!app.notices.is_empty());
    assert!(app.session.project.assets[0].source.is_included_recording());
    assert_eq!(app.session.project.transport.playhead_frame, 777);
    let ctx = context();
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(!has_text(&shapes, "Close"));
    assert!(app.collection_status.is_none());
}
#[test]
fn committed_disk_after_lifecycle_change_keeps_active_state_and_reports_recovery() {
    let f = Fixture::new();
    let mut app = f.app();
    let (ticket, report) = preflight(&mut app);
    app.collection_status.as_mut().unwrap().stage = Stage::Copying;
    let outcome = report
        .into_plan()
        .unwrap()
        .execute(&|_| true, &|| false, &mut |_| Ok(()))
        .unwrap();
    app.session = Session::default();
    let before = app.session.project.clone();
    app.refresh_collection();
    app.finish_collection(ticket, Ok(outcome));
    assert_eq!(app.session.project, before);
    let status = app.collection_status.as_ref().unwrap();
    assert!(status.recovery);
    assert!(matches!(&status.report,job_report::State::Failed(e) if e.contains("saved on disk")));
    assert!(
        Session::open(&f.0.join("project"))
            .unwrap()
            .warnings
            .is_empty()
    );
}
#[test]
fn application_menu_groups_collection_with_project_settings_and_dispatches_immediately() {
    let f = Fixture::new();
    let mut app = f.app();
    let ctx = context();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.file_menu(ui));
    out.textures_delta.clear();
    assert_eq!(
        FileAction::CollectSources.label(),
        "Collect project sources"
    );
    let settings = text_point(&out.shapes, "Project Settings…");
    let collect = text_point(&out.shapes, FileAction::CollectSources.label());
    let export = text_point(&out.shapes, FileAction::Export.label());
    assert!(export.y < settings.y && settings.y < collect.y);
    for pressed in [true, false] {
        let mut out = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(collect),
                    egui::Event::PointerButton {
                        pos: collect,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ui| app.file_menu(ui),
        );
        out.textures_delta.clear();
    }
    assert!(app.job.is_some());
    wait(&mut app);
    assert!(app.session.project.assets[0].source.is_included_recording());
}
#[test]
fn running_collection_blocks_edits_and_close_without_showing_a_dialog() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    let (_sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    let ctx = context();
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(has_text(&shapes, "Checking project sources…"));
    assert!(!has_text(&shapes, "Collect and save"));
    assert!(!has_text(&shapes, "Source-collection status"));
    assert!(!app.file_action_enabled(FileAction::Save));
    assert!(!app.edit_environment_available());
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut out = ctx.run_ui(input, |ui| app.show(ui));
    out.textures_delta.clear();
    assert!(!app.allow_close);
    assert!(
        out.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
}
#[test]
fn completion_keeps_installed_effect_selection_and_focus_without_dismissal() {
    let f = Fixture::new();
    let mut app = f.app();
    let effect = daw_core::EffectInstance::equalizer();
    let id = effect.id;
    app.session.project.tracks[0].effects.push(effect);
    app.set_effects_lane_visible(true);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let point = app.effects_bounds.units[0].header.center();
    click(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    assert!(app.selected_effects.contains(&id));
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(app.collection_status.is_none());
    assert!(app.selected_effects.contains(&id));
}
#[test]
fn completion_keeps_instrument_component_selection_without_dismissal() {
    let f = Fixture::new();
    let mut app = f.app();
    let track = app.session.project.add_midi_track().unwrap();
    app.session
        .project
        .tracks
        .last_mut()
        .unwrap()
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .lfos
        .push(Default::default());
    app.selected_track = Some(track);
    app.view_mode = ViewMode::Instrument;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let (owner, unit, _) = app
        .instrument_ui
        .component_bounds
        .iter()
        .copied()
        .find(|(_, c, _)| matches!(c, daw_core::InstrumentComponent::Lfo(_)))
        .unwrap();
    app.instrument_ui.selected_components.insert((owner, unit));
    app.instrument_ui.component_refocus = true;
    frame(&mut app, &ctx, vec![]);
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(app.collection_status.is_none());
    assert!(
        app.instrument_ui
            .selected_components
            .contains(&(owner, unit))
    );
}
#[test]
fn focus_loss_and_text_editing_disable_collection_without_filesystem_checks() {
    let f = Fixture::new();
    let mut app = f.app();
    let ctx = context();
    let mut out = ctx.run_ui(
        egui::RawInput {
            focused: false,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    let mut value = String::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.text_edit_singleline(&mut value).request_focus();
            app.show(ui);
        });
        out.textures_delta.clear();
    }
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    assert!(app.job.is_none());
}
#[test]
fn status_bar_cancel_keeps_receiver_until_worker_acknowledges() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, text_point(&shapes, "Cancel"));
    assert!(app.job.is_some());
    assert!(
        app.collection_status
            .as_ref()
            .unwrap()
            .lease
            .cancel
            .load(Ordering::Relaxed)
    );
    let ticket = app.collection_status.as_ref().unwrap().ticket;
    sender
        .send(Ok(Job::CollectionPreflight(
            ticket,
            Err("cancelled".into()),
        )))
        .unwrap();
    app.poll();
    assert!(app.job.is_none());
    assert_eq!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Cancelled
    );
}
#[test]
fn automatic_execution_does_not_replace_a_competing_job_or_bypass_an_error() {
    for error in [false, true] {
        let f = Fixture::new();
        let mut app = f.app();
        let (ticket, report) = preflight(&mut app);
        let (_sender, receiver) = mpsc::channel();
        if error {
            app.error = Some("Device error".into());
        } else {
            app.job = Some(receiver);
        }
        app.finish_collection_preflight(ticket, Ok(report));
        assert_eq!(app.job.is_some(), !error);
        assert!(matches!(
            app.collection_status.as_ref().unwrap().report,
            job_report::State::Failed(_)
        ));
        assert!(!app.session.project.assets[0].source.is_included_recording());
    }
}

#[test]
fn status_bar_reports_copy_verification_and_publication_progress_and_worker_disconnect() {
    let f = Fixture::new();
    let mut app = f.app();
    let (_ticket, report) = preflight(&mut app);
    let status = app.collection_status.as_mut().unwrap();
    status.stage = Stage::Copying;
    let (sender, receiver) = mpsc::sync_channel(1);
    status.receiver = Some(receiver);
    let mut progress = report
        .into_plan()
        .unwrap()
        .execute(&|_| true, &|| false, &mut |_| Ok(()))
        .unwrap()
        .progress();
    progress.files_copied = 0;
    progress.copied_bytes = 10;
    for (phase, text) in [
        (CollectionPhase::Verifying, "Verifying sources…"),
        (CollectionPhase::Publishing, "Saving collected project…"),
    ] {
        progress.phase = phase;
        sender.send(progress).unwrap();
        app.refresh_collection();
        let text_status = app.collection_status_text().unwrap();
        assert!(text_status.contains(text));
        assert!(text_status.contains("0 of 1 files"));
        assert!(text_status.contains("10 of"));
    }
    let (worker, receiver) = mpsc::channel();
    app.job = Some(receiver);
    drop(worker);
    app.poll();
    assert!(matches!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Failed(_)
    ));
    assert_eq!(
        app.collection_status_text().as_deref(),
        Some("Source collection failed")
    );
}
