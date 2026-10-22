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
                progress: progress.clone(),
                state: job_report::State::Running,
                warnings: Vec::new(),
                path: std::env::temp_dir(),
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
