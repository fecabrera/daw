use super::*;

fn hold_save(app: &mut DawUi) -> mpsc::Sender<Result<(), String>> {
    let (sender, receiver) = mpsc::channel();
    app.regular_save = Some(regular_save::PendingSave {
        receiver,
        project: app.session.project.project_id,
        revision: app.change_revision,
        snapshot: app.session.clone(),
        navigation_revision: app.navigation_revision,
    });
    sender
}

#[test]
fn regular_save_keeps_workspace_paint_and_effect_selection_stable() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for view in [
            ViewMode::Timeline,
            ViewMode::Instrument,
            ViewMode::Edit,
            ViewMode::Mixer,
        ] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0]
                .oscillators
                .push(Default::default());
            app.session.project.tracks[0]
                .effects
                .push(daw_core::EffectInstance::equalizer());
            app.selected_clip = Some(clip);
            app.selected_clips.insert(clip);
            app.select_track(track, false);
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                ctx.style_mut_of(theme, |s| s.animation_time = 0.0);
            }
            app.set_view_mode(view, &ctx);
            app.timeline_catalog_visible = false;
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            if matches!(view, ViewMode::Timeline | ViewMode::Instrument) {
                let point = app.effects_bounds.units[0].header.center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                frame(&mut app, &ctx, vec![button(point, false)]);
                assert_eq!(app.selected_effects.len(), 1);
            }
            app.mark_dirty();
            let size = Vec2::new(1280.0, 800.0);
            frame_shapes(&mut app, &ctx, vec![], size);
            let workspace = |shapes: Vec<egui::epaint::ClippedShape>| -> Vec<_> {
                shapes
                    .into_iter()
                    .filter(|s| {
                        let rect = s.shape.visual_bounding_rect();
                        rect.is_positive() && rect.top() >= 110.0 && rect.bottom() <= 760.0
                    })
                    .collect()
            };
            let before = workspace(frame_shapes(&mut app, &ctx, vec![], size));
            let effects = app.selected_effects.clone();
            let clips = app.selected_clips.clone();
            let sender = hold_save(&mut app);
            let during = workspace(frame_shapes(&mut app, &ctx, vec![], size));
            assert_eq!(before.len(), during.len(), "{view:?} {theme:?}");
            assert!(
                before
                    .iter()
                    .zip(&during)
                    .all(|(a, b)| a.clip_rect == b.clip_rect && a.shape == b.shape),
                "workspace paint changed: {view:?} {theme:?}"
            );
            assert_eq!(app.selected_effects, effects);
            assert_eq!(app.selected_clips, clips);
            assert!(app.edit_environment_available());
            for action in FileAction::ALL {
                assert!(!app.file_action_enabled(action));
            }
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
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
            assert!(!app.allow_close && !app.unsaved_prompt && app.pending.is_none());
            sender.send(Ok(())).unwrap();
            app.poll();
            assert!(!app.dirty && app.regular_save.is_none());
        }
    }
}

#[test]
fn regular_save_writes_a_snapshot_and_keeps_later_edits_and_transport() {
    let location = SaveLocation::new();
    let mut app = DawUi::default();
    app.session.project.add_midi_track().unwrap();
    app.session.save(&location.0).unwrap();
    app.mark_dirty();
    app.save(false);
    assert!(app.job.is_none() && app.regular_save.is_some());
    app.session.project.tracks[0].name = "Later edit".into();
    app.changed();
    app.seek(1234);
    wait_for_save(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(app.dirty);
    assert_eq!(app.session.project.tracks[0].name, "Later edit");
    assert_eq!(app.session.project.transport.playhead_frame, 1234);
    let disk = Session::open(&location.0).unwrap();
    assert_ne!(disk.project.tracks[0].name, "Later edit");
    assert_ne!(disk.project.transport.playhead_frame, 1234);
    app.save(false);
    wait_for_save(&mut app);
    assert!(!app.dirty && app.error.is_none());
    let disk = Session::open(&location.0).unwrap();
    assert_eq!(disk.project.tracks[0].name, "Later edit");
    assert_eq!(disk.project.transport.playhead_frame, 1234);

    // Worker-prepared edits (for example a stretch) also invalidate an in-flight save.
    let sender = hold_save(&mut app);
    let mut prepared = app.session.clone();
    prepared.project.tracks[0].name = "Worker edit".into();
    let (tx, rx) = mpsc::channel();
    app.job = Some(rx);
    tx.send(Ok(Job::Loaded(prepared, true))).unwrap();
    app.poll();
    assert!(app.dirty);
    sender.send(Ok(())).unwrap();
    app.poll();
    assert!(app.dirty);
    assert_eq!(app.session.project.tracks[0].name, "Worker edit");
}

#[test]
fn regular_save_failure_disconnect_and_replaced_project_do_not_publish_old_state() {
    for outcome in ["failure", "disconnect", "replacement"] {
        let mut app = DawUi::default();
        app.mark_dirty();
        let sender = hold_save(&mut app);
        if outcome == "replacement" {
            app.session = Session::default();
        }
        let original = serde_json::to_value(&app.session.project).unwrap();
        match outcome {
            "failure" => sender.send(Err("test save failure".into())).unwrap(),
            "replacement" => sender.send(Ok(())).unwrap(),
            _ => {}
        }
        drop(sender);
        app.poll();
        assert!(app.regular_save.is_none());
        assert!(app.dirty);
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original
        );
        assert_eq!(app.error.is_some(), outcome != "replacement");
    }
}
