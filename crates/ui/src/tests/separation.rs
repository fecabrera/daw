use super::*;

#[test]
fn both_separation_cancel_buttons_wait_for_acknowledgement_and_allow_close() {
    for status_bar in [false, true] {
        for acknowledgement in [false, true] {
            let (mut app, _) = fixture();
            let before = app.session.project.clone();
            let ctx = context();
            let progress = daw_project::SeparationProgress::default();
            let (sender, receiver) = mpsc::channel();
            app.job = Some(receiver);
            app.busy = "Separating stems…".into();
            app.settings_cancel = Some(progress.cancel.clone());
            app.separation_status = Some(crate::separation::Status {
                stems: None,
                progress: progress.clone(),
                state: job_report::State::Running,
                warnings: Vec::new(),
                sample_rate_hz: app.session.project.sample_rate_hz,
            });
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let mut buttons: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Cancel" => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                    }
                    _ => None,
                })
                .collect();
            buttons.sort_by(|a, b| a.y.total_cmp(&b.y));
            assert_eq!(buttons.len(), 2);
            let point = buttons[usize::from(status_bar)];
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(&mut app, &ctx, vec![button(point, false)]);
            assert!(progress.cancel.load(Ordering::Relaxed));
            assert!(app.job.is_some(), "Keep the acknowledgement receiver");
            assert!(!app.editing_enabled());
            assert!(!app.file_action_enabled(FileAction::Open));

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
            assert_eq!(
                app.separation_status.as_ref().unwrap().state,
                job_report::State::Running
            );
            if acknowledgement {
                assert!(sender.send(Err("Stem separation cancelled".into())).is_ok());
            }
            drop(sender);
            app.poll();
            assert!(app.job.is_none());
            assert!(app.settings_cancel.is_none());
            assert_eq!(
                app.separation_status.as_ref().unwrap().state,
                job_report::State::Cancelled
            );
            assert!(!app.file_action_enabled(FileAction::Open));
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let close = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Close" => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                    }
                    _ => None,
                })
                .unwrap();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(close), button(close, true)],
            );
            frame(&mut app, &ctx, vec![button(close, false)]);
            assert!(app.separation_status.is_none());
            assert!(app.editing_enabled());
            assert!(app.file_action_enabled(FileAction::Open));
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            assert!(app.error.is_none());
        }
    }
}

#[test]
fn model_validation_cancel_keeps_receiver_and_previous_model() {
    let (mut app, _) = fixture();
    let before = app.session.project.clone();
    let previous = std::path::PathBuf::from("previous.onnx");
    std::sync::Arc::make_mut(&mut app.settings.separation_models).cpu = Some(previous.clone());
    app.separation_models.validated.cpu = Some(previous.clone());
    let previous_info = daw_project::SeparationModelInfo {
        stems: daw_project::SeparationStemSet::Six,
        file_size_bytes: 258_000_000,
    };
    app.separation_models
        .info
        .insert(daw_project::SeparationModelKind::Cpu, previous_info);
    app.settings_open = true;
    let (release, wait) = mpsc::channel();
    app.start_model_validation(
        daw_project::SeparationModelKind::Cpu,
        "cancelled.onnx".into(),
        move |_, _, _| wait.recv().unwrap(),
    );
    let cancel = app.settings_cancel.clone().unwrap();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let point = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Cancel" => {
                Some(Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        })
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert!(cancel.load(Ordering::Relaxed));
    assert!(app.job.is_some());
    assert!(app.separation_models.running());
    assert!(!app.editing_enabled());
    release
        .send(Ok(daw_project::SeparationModelInfo {
            stems: daw_project::SeparationStemSet::Four,
            file_size_bytes: 1,
        }))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.job.is_some() {
        app.poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.separation_models.busy());
    assert!(app.settings_cancel.is_none());
    assert_eq!(app.settings.separation_models.cpu, Some(previous.clone()));
    assert_eq!(app.separation_models.validated.cpu, Some(previous));
    assert_eq!(
        app.separation_models.info[&daw_project::SeparationModelKind::Cpu],
        previous_info
    );
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    assert!(app.error.is_none());
}

#[test]
fn separation_report_shows_tracks_and_project_quality_instead_of_destination() {
    use daw_project::SeparationStemSet as Stems;
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for rate in [44_100, 48_000, 192_000] {
            for state in [
                job_report::State::Running,
                job_report::State::Complete,
                job_report::State::Failed("test failure".into()),
            ] {
                let ctx = context();
                crate::theme::apply(
                    &ctx,
                    settings::AppSettings {
                        theme,
                        ..Default::default()
                    },
                );
                let status = crate::separation::Status {
                    progress: Default::default(),
                    stems: Some(Stems::Six),
                    sample_rate_hz: rate,
                    state,
                    warnings: vec!["test warning".into()],
                };
                let render = || {
                    ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(900.0, 800.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            status.show(ui.ctx());
                        },
                    )
                };
                let mut output = render();
                output.textures_delta.clear();
                output = render();
                output.textures_delta.clear();
                let text = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t.galley.text()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let expected = format!(
                    "Charon · HTDemucs · 6 stems\nDrums, Bass, Other, Vocals, Guitar, Piano\nWAV · IEEE float 32-bit · Stereo · {} kHz",
                    rate as f32 / 1000.0
                );
                assert!(text.contains(&expected), "{text}");
                for omitted in [
                    "Destination",
                    "Tracks to create",
                    "Created tracks",
                    "Planned tracks",
                    "Audio quality",
                    "1 group",
                    "CPU fallback:",
                ] {
                    assert!(!text.contains(omitted), "{text}");
                }
                assert!(text.contains("test warning"));
                if matches!(status.state, job_report::State::Failed(_)) {
                    assert!(text.contains("test failure"));
                }
            }
        }
    }
}

#[test]
fn separation_report_snapshots_validated_model_details_before_running() {
    use daw_project::{
        SeparationModelInfo as Info, SeparationModelKind as Kind, SeparationStemSet as Stems,
    };
    let mut app = DawUi::default();
    app.separation_models.validated.cpu = Some("cpu.onnx".into());
    app.separation_models.info.insert(
        Kind::Cpu,
        Info {
            stems: Stems::Four,
            file_size_bytes: 10,
        },
    );
    let coreml = Kind::CoreMl.available();
    if coreml {
        app.separation_models.validated.coreml = Some("coreml.onnx".into());
        app.separation_models.info.insert(
            Kind::CoreMl,
            Info {
                stems: Stems::Six,
                file_size_bytes: 20,
            },
        );
    }
    app.separate_clip(Id::new_v4());
    let status = app.separation_status.as_ref().unwrap();
    assert_eq!(
        status.stems,
        Some(if coreml { Stems::Six } else { Stems::Four })
    );
    assert_eq!(status.sample_rate_hz, app.session.project.sample_rate_hz);
    assert!(!app.dirty);
}

#[test]
fn separation_status_bar_and_dialog_share_each_running_phase() {
    for (total, done, cancelled, expected) in [
        (0, 0, false, "Preparing audio and loading model…"),
        (10, 3, false, "Separating audio…"),
        (10, 10, false, "Preparing stem tracks…"),
        (10, 10, true, "Cancelling…"),
    ] {
        let (mut app, _) = fixture();
        let ctx = context();
        let (_sender, receiver) = mpsc::channel();
        app.job = Some(receiver);
        app.busy = "Separating stems…".into();
        let progress = daw_project::SeparationProgress::default();
        progress.total.store(total, Ordering::Relaxed);
        progress.done.store(done, Ordering::Relaxed);
        progress.cancel.store(cancelled, Ordering::Relaxed);
        app.separation_status = Some(crate::separation::Status {
            progress,
            stems: Some(daw_project::SeparationStemSet::Six),
            sample_rate_hz: 48_000,
            state: job_report::State::Running,
            warnings: Vec::new(),
        });
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let labels: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels.iter().filter(|label| **label == expected).count(),
            2,
            "{expected}: {labels:?}"
        );
        assert!(!labels.contains(&"Separating stems…"));
    }
}
