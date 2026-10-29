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
    app.collection_input_ready = false;
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    app.collection_input_ready = true;
    app.settings_open = true;
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    app.settings_open = false;
    app.strip_dragging = true;
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    app.strip_dragging = false;
    app.unsaved_prompt = true;
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    app.unsaved_prompt = false;
    let (_tx, rx) = mpsc::channel();
    app.regular_save = Some(regular_save::PendingSave {
        receiver: rx,
        project: app.session.project.project_id,
        revision: 0,
        navigation_revision: 0,
    });
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    app.regular_save = None;
    app.perform_file_action(FileAction::CollectSources);
    assert!(app.job.is_some());
    assert!(
        FileAction::ALL
            .into_iter()
            .all(|a| !app.file_action_enabled(a))
    );
    assert!(!app.edit_environment_available());
    wait(&mut app);
    assert!(app.collection_status.as_ref().unwrap().stage == Stage::Review);
    assert!(!app.editing_enabled());
}
#[test]
fn preflight_lists_counts_and_confirmation_commits_without_selection_audio_or_transport_changes() {
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
    wait(&mut app);
    let status = app.collection_status.as_ref().unwrap();
    assert!(status.detail.contains("Files to copy: 1"));
    assert!(status.detail.contains("also saves"));
    assert!(status.preflight.as_ref().unwrap().is_ready());
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    let point = text_point(&shapes, "Collect and save");
    click(&mut app, &ctx, point);
    wait(&mut app);
    app.session.project.transport.playhead_frame = 321;
    assert_eq!(
        app.collection_status.as_ref().unwrap().report,
        job_report::State::Complete
    );
    assert!(!app.dirty);
    assert!(!app.sync_needed);
    assert_eq!(app.selected_clip, clip);
    assert_eq!(app.selected_clips, selected);
    assert_eq!(app.selected_track, track);
    assert!(Arc::ptr_eq(&pcm, &app.session.audio[&id].samples));
    let status = app.collection_status.as_ref().unwrap();
    assert_eq!(status.progress.unwrap().files_copied, 1);
    assert!(app.session.project.assets[0].source.is_included_recording());
    fs::remove_file(f.0.join("source.wav")).unwrap();
    assert!(
        Session::open(app.session.folder.as_ref().unwrap())
            .unwrap()
            .warnings
            .is_empty()
    );
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    let point = text_point(&shapes, "Close");
    click(&mut app, &ctx, point);
    assert!(app.collection_status.is_none());
    assert!(app.edit_environment_available());
}
#[test]
fn missing_sources_block_confirmation_and_report_affected_assets() {
    let f = Fixture::new();
    let mut app = f.app();
    let before = fs::read(f.0.join("project/project.json")).unwrap();
    fs::remove_file(f.0.join("source.wav")).unwrap();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    let status = app.collection_status.as_ref().unwrap();
    assert!(status.stage == Stage::Review);
    assert!(!status.preflight.as_ref().unwrap().is_ready());
    app.confirm_collection();
    assert!(app.job.is_none());
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("Assets: source"))
    ));
    assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
}
#[test]
fn cancel_preflight_or_review_retains_manifest_live_project_and_dirty_state() {
    for checking in [true, false] {
        let f = Fixture::new();
        let mut app = f.app();
        app.session.project.name = "Dirty".into();
        app.mark_dirty();
        let model = app.session.project.clone();
        let before = fs::read(f.0.join("project/project.json")).unwrap();
        app.perform_file_action(FileAction::CollectSources);
        if checking {
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
        } else {
            wait(&mut app);
            let ctx = context();
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
            assert!(app.collection_status.is_none());
        }
        assert_eq!(app.session.project, model);
        assert!(app.dirty);
        assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
    }
}
#[test]
fn stale_project_view_or_navigation_prevents_confirmation_and_keeps_active_state() {
    for change in [0, 1, 2] {
        let f = Fixture::new();
        let mut app = f.app();
        app.perform_file_action(FileAction::CollectSources);
        wait(&mut app);
        match change {
            0 => app.session = Session::default(),
            1 => app.view_mode = ViewMode::Mixer,
            _ => app.navigation_revision += 1,
        }
        let model = app.session.project.clone();
        app.confirm_collection();
        assert!(app.job.is_none());
        assert_eq!(app.session.project, model);
        assert!(matches!(
            &app.collection_status.as_ref().unwrap().report,
            job_report::State::Failed(_)
        ));
    }
}
#[test]
fn repeated_collection_explains_no_copy_and_does_not_add_history_edits() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    app.confirm_collection();
    wait(&mut app);
    app.collection_status = None;
    assert!(!app.history.as_ref().unwrap().can_undo());
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    assert!(
        app.collection_status
            .as_ref()
            .unwrap()
            .detail
            .contains("No new copies")
    );
    app.confirm_collection();
    wait(&mut app);
    assert_eq!(
        app.collection_status
            .as_ref()
            .unwrap()
            .progress
            .unwrap()
            .files_copied,
        0
    );
    assert!(!app.history.as_ref().unwrap().can_undo());
}
#[test]
fn copy_failure_and_cancelled_worker_keep_original_manifest_and_live_buffers() {
    for cancelled in [true, false] {
        let f = Fixture::new();
        let mut app = f.app();
        app.perform_file_action(FileAction::CollectSources);
        wait(&mut app);
        let before = fs::read(f.0.join("project/project.json")).unwrap();
        let model = app.session.project.clone();
        if cancelled {
            app.collection_status
                .as_ref()
                .unwrap()
                .lease
                .cancel
                .store(true, Ordering::Relaxed);
        } else {
            fs::remove_file(f.0.join("source.wav")).unwrap();
        }
        app.confirm_collection();
        wait(&mut app);
        let report = &app.collection_status.as_ref().unwrap().report;
        if cancelled {
            assert_eq!(*report, job_report::State::Cancelled);
        } else {
            assert!(matches!(report, job_report::State::Failed(_)));
        }
        assert_eq!(app.session.project, model);
        assert_eq!(fs::read(f.0.join("project/project.json")).unwrap(), before);
        assert!(!app.session.project.assets[0].source.is_included_recording());
    }
}
#[test]
fn late_cancel_and_notification_error_still_publish_confirmed_commit_with_warnings() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    let status = app.collection_status.as_mut().unwrap();
    let ticket = status.ticket;
    let plan = status.preflight.take().unwrap().into_plan().unwrap();
    status.stage = Stage::Copying;
    let outcome = plan
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
    let status = app.collection_status.as_ref().unwrap();
    assert_eq!(status.report, job_report::State::Complete);
    assert!(!status.warnings.is_empty());
    assert!(app.session.project.assets[0].source.is_included_recording());
    assert_eq!(app.session.project.transport.playhead_frame, 777);
}
#[test]
fn confirmed_disk_commit_after_lifecycle_change_does_not_replace_the_active_project() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    let status = app.collection_status.as_mut().unwrap();
    let ticket = status.ticket;
    let plan = status.preflight.take().unwrap().into_plan().unwrap();
    status.stage = Stage::Copying;
    let outcome = plan.execute(&|_| true, &|| false, &mut |_| Ok(())).unwrap();
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
fn application_menu_dispatches_the_shared_action_and_modal_blocks_edits_and_window_close() {
    let f = Fixture::new();
    let mut app = f.app();
    let ctx = context();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
            ..Default::default()
        },
        |ui| app.file_menu(ui),
    );
    out.textures_delta.clear();
    let point = text_point(&out.shapes, FileAction::CollectSources.label());
    for pressed in [true, false] {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events: vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
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
    assert!(app.collection_status.is_some());
    wait(&mut app);
    let before = app.session.project.clone();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    assert!(!app.file_action_enabled(FileAction::Save));
    assert!(!app.edit_environment_available());
    assert_eq!(app.session.project, before);
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
fn collection_modal_and_dismissal_keep_installed_effect_selection_and_focus() {
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
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    assert!(app.selected_effects.contains(&id));
    app.confirm_collection();
    wait(&mut app);
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    let close = text_point(&shapes, "Close");
    click(&mut app, &ctx, close);
    frame(&mut app, &ctx, vec![]);
    assert!(app.selected_effects.contains(&id));
}

#[test]
fn modal_keeps_instrument_component_selection_after_cancel_and_refocus() {
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
    assert!(
        app.instrument_ui
            .selected_components
            .contains(&(owner, unit))
    );
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    assert!(
        app.instrument_ui
            .selected_components
            .contains(&(owner, unit))
    );
    let cancel = text_point(&shapes, "Cancel");
    click(&mut app, &ctx, cancel);
    frame(&mut app, &ctx, vec![]);
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
    // A stable text-edit response is read on the following show frame, as native
    // menu validation reads the latest application input state.
    let mut value = String::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let response = ui.text_edit_singleline(&mut value);
            response.request_focus();
            app.show(ui);
        });
        out.textures_delta.clear();
    }
    assert!(!app.file_action_enabled(FileAction::CollectSources));
    assert!(app.job.is_none());
}
#[test]
fn checking_report_cancel_button_keeps_receiver_until_worker_acknowledges() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    // Use a held receiver to verify that the generic status-bar Cancel path
    // cannot discard the collection completion/commit result.
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame(&mut app, &ctx, vec![]);
    let cancel = text_point(&shapes, "Cancel");
    click(&mut app, &ctx, cancel);
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
fn review_confirmation_does_not_replace_a_competing_job_or_bypass_an_error_dialog() {
    let f = Fixture::new();
    let mut app = f.app();
    app.perform_file_action(FileAction::CollectSources);
    wait(&mut app);
    let (_sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    app.confirm_collection();
    assert!(app.job.is_some());
    assert!(app.collection_status.as_ref().unwrap().stage == Stage::Review);
    app.job = None;
    app.error = Some("Device error".into());
    app.confirm_collection();
    assert!(app.job.is_none());
    assert!(app.collection_status.as_ref().unwrap().stage == Stage::Review);
    assert!(!app.session.project.assets[0].source.is_included_recording());
}
