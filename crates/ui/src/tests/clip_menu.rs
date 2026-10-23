use super::*;

fn menu_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    target: Option<Id>,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            events,
            ..Default::default()
        },
        |ui| match target {
            Some(id) => app.clip_context_menu(ui, id),
            None => app.clip_menu(ui),
        },
    );
    output.textures_delta.clear();
    output.shapes
}

fn click(app: &mut DawUi, ctx: &egui::Context, target: Option<Id>, label: &str) {
    let shapes = menu_frame(app, ctx, target, vec![]);
    let point = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing clip action {label}"));
    menu_frame(
        app,
        ctx,
        target,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu_frame(app, ctx, target, vec![button(point, false)]);
}

#[test]
fn clip_menu_and_context_have_grouped_actions_and_edit_keeps_copy_paste() {
    let (mut app, track) = fixture();
    let ctx = context();
    let id = app.session.project.tracks[0].clips[0].id;
    app.select_clip(id, track, false);
    for target in [None, Some(id)] {
        menu_frame(&mut app, &ctx, target, vec![]);
        let shapes = menu_frame(&mut app, &ctx, target, vec![]);
        let labels: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if ClipMenuAction::ALL
                        .iter()
                        .any(|action| action.label() == text.galley.text()) =>
                {
                    Some(text.galley.text())
                }
                _ => None,
            })
            .collect();
        let expected: Vec<_> = ClipMenuAction::GROUPS[usize::from(target.is_some())..]
            .iter()
            .flat_map(|group| group.iter().map(|a| a.label()))
            .collect();
        assert_eq!(labels, expected);
        let separators = shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::LineSegment { points, .. } if points[0].y == points[1].y)).count();
        assert_eq!(separators, if target.is_some() { 1 } else { 2 });
    }
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.edit_menu(ui));
    output.textures_delta.clear();
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text()),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&"Copy"));
    assert!(labels.contains(&"Paste"));
    assert!(!labels.contains(&"Split at playhead"));
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.file_menu(ui));
    output.textures_delta.clear();
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == FileAction::Import.label())));
    assert!(!app.dirty);
}

#[test]
fn clip_actions_use_primary_or_clicked_target_and_delete_selected_group_atomically() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let (mut app, track) = fixture();
        let ctx = context();
        let first = app.session.project.tracks[0].clips[0].clone();
        let second = Clip {
            id: Id::new_v4(),
            start_frame: frames(12.0),
            ..first.clone()
        };
        app.session.project.tracks[0].clips.push(second.clone());
        app.select_clip(first.id, track, false);
        app.set_view_mode(view, &ctx);
        click(&mut app, &ctx, Some(second.id), "Disable clip");
        assert!(!app.session.project.tracks[0].clips[0].muted);
        assert!(app.session.project.tracks[0].clips[1].muted);
        assert_eq!(app.selected_clip, Some(first.id));
        click(&mut app, &ctx, None, "Disable clip");
        assert!(app.session.project.tracks[0].clips[0].muted);
        assert_eq!(
            app.clip_menu_action_label(ClipMenuAction::ToggleMuted),
            "Enable clip"
        );
        click(&mut app, &ctx, Some(second.id), "Edit clip");
        assert_eq!(app.view_mode, ViewMode::Edit);
        assert_eq!(app.selected_clip, Some(second.id));
        app.select_clip(first.id, track, false);
        click(&mut app, &ctx, Some(second.id), "Delete clip");
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
        assert_eq!(app.session.project.tracks[0].clips[0].id, first.id);
        app.session.project.tracks[0].clips.push(second.clone());
        app.select_clip(second.id, track, true);
        click(&mut app, &ctx, None, "Delete selected clips");
        assert!(app.session.project.tracks[0].clips.is_empty());
        assert!(app.selected_clip.is_none());
        assert!(app.selected_clips.is_empty());
        assert!(app.dirty);
        assert!(app.error.is_none());
    }
}

#[test]
fn clip_menu_creates_midi_on_selected_track_at_playhead_and_guards_actions() {
    let (mut app, audio) = fixture();
    let ctx = context();
    app.select_track(audio, false);
    menu_frame(&mut app, &ctx, None, vec![]);
    assert!(!app.clip_menu_action_enabled(ClipMenuAction::AddMidi, &ctx));
    let midi = app.session.project.add_midi_track().unwrap();
    app.select_track(midi, false);
    app.session.project.transport.playhead_frame = 96_000;
    assert!(app.clip_menu_action_enabled(ClipMenuAction::AddMidi, &ctx));
    click(&mut app, &ctx, None, "Add MIDI pattern");
    let clip = &app.session.project.tracks[1].clips[0];
    assert_eq!(clip.start_frame, 96_000);
    assert_eq!(app.selected_clip, Some(clip.id));
    assert!(!app.clip_menu_action_enabled(ClipMenuAction::Separate, &ctx));
    assert!(
        app.session
            .project
            .assets
            .iter()
            .any(|asset| asset.id == clip.asset_id && asset.midi_pattern.is_some())
    );
    let before = app.session.project.clone();
    for settings in [false, true] {
        if settings {
            app.settings_open = true;
        } else {
            app.error = Some("blocked".into());
        }
        for action in ClipMenuAction::ALL {
            assert!(!app.clip_menu_action_enabled(action, &ctx));
            app.perform_clip_menu_action(action, &ctx);
        }
        assert_eq!(app.session.project, before);
        app.settings_open = false;
        app.error = None;
    }
    app.set_view_mode(ViewMode::Mixer, &ctx);
    for action in ClipMenuAction::ALL
        .into_iter()
        .filter(|a| *a != ClipMenuAction::Import)
    {
        assert!(!app.clip_menu_action_enabled(action, &ctx));
    }
}

#[test]
fn clip_menu_separation_requires_a_selected_audio_clip_and_validated_model() {
    let (mut app, track) = fixture();
    let ctx = context();
    let clip = app.session.project.tracks[0].clips[0].id;
    app.select_clip(clip, track, false);
    menu_frame(&mut app, &ctx, None, vec![]);
    assert!(!app.clip_menu_action_enabled(ClipMenuAction::Separate, &ctx));
    for kind in daw_project::SeparationModelKind::ALL
        .into_iter()
        .filter(|k| k.available())
    {
        app.separation_models
            .validated
            .set(kind, Some("validated.onnx".into()));
        assert!(app.clip_menu_action_enabled(ClipMenuAction::Separate, &ctx));
        app.set_view_mode(ViewMode::Edit, &ctx);
        assert!(!app.clip_menu_action_enabled(ClipMenuAction::Separate, &ctx));
        app.set_view_mode(ViewMode::Timeline, &ctx);
        app.separation_models.validated.set(kind, None);
    }
}
