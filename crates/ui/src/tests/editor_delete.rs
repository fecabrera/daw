use super::*;

fn delete(key: egui::Key, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: Default::default(),
    }
}
fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn editor_delete_removes_selected_clip_group_and_undo_restores_it() {
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        let (mut app, ctx, track, id) = super::audio_editor::audio_fixture();
        let mut second = app.session.project.tracks[0].clips[0].clone();
        second.id = Id::new_v4();
        second.start_frame = second.end();
        second.length_frames = 1000;
        second.source_offset_frame = 0;
        let mut third = second.clone();
        third.id = Id::new_v4();
        third.start_frame = second.end();
        app.session.project.tracks[0]
            .clips
            .extend([second.clone(), third.clone()]);
        app.select_clip(id, track, false);
        app.select_clip(second.id, track, true);
        app.history = None;
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        frame(&mut app, &ctx, vec![delete(key, false)]);
        assert_eq!(app.session.project.tracks[0].clips, vec![third.clone()]);
        assert!(app.selected_clip.is_none());
        assert!(app.selected_clips.is_empty());
        assert_eq!(app.session.project.assets, before.assets);
        assert!(app.session.audio.contains_key(&third.asset_id));
        assert!(app.dirty);
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn editor_delete_respects_text_modal_focus_and_repeat_guards() {
    for guard in 0..8 {
        let (mut app, ctx, track, _) = super::audio_editor::audio_fixture();
        let before = app.session.project.clone();
        let (_sender, receiver) = mpsc::channel();
        match guard {
            0 => app.settings_open = true,
            1 => app.error = Some("Pending error".into()),
            2 => {
                app.track_name_edit = Some(TrackNameEdit {
                    track,
                    text: "Audio 1".into(),
                    focus: true,
                });
                frame(&mut app, &ctx, vec![]);
            }
            5 => app.job = Some(receiver),
            _ => {}
        }
        if guard == 4 {
            ctx.input_mut(|input| {
                input.keys_down.insert(egui::Key::Delete);
            });
        }
        let mut events = vec![delete(egui::Key::Delete, guard == 4)];
        if guard == 7 {
            let point = app.midi_editor.content.bounds.center();
            events.insert(0, button(point, true));
            events.insert(0, egui::Event::PointerMoved(point));
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: guard != 3,
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(guard != 6, |ui| app.show(ui));
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.session.project, before, "guard {guard}");
        assert!(!app.dirty);
    }
}

#[test]
fn editor_delete_prefers_notes_and_header_click_returns_to_clip_deletion() {
    for header_first in [false, true] {
        let (mut app, track, clip, note) = super::midi::fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let point = app.midi_editor.note_bounds[&note].center();
        click(&mut app, &ctx, point);
        assert!(!app.midi_editor.selected_notes().is_empty());
        if !header_first {
            frame(&mut app, &ctx, vec![delete(egui::Key::Delete, false)]);
            assert_eq!(app.session.project.tracks[0].clips.len(), 1);
            assert!(app.midi_editor.selected_notes().is_empty());
            frame(&mut app, &ctx, vec![delete(egui::Key::Delete, true)]);
            assert_eq!(
                app.session.project.tracks[0].clips.len(),
                1,
                "note-delete repeats must not remove the clip"
            );
        }
        frame(&mut app, &ctx, vec![]);
        let header = app.midi_editor.pattern_header_bounds[&clip].center();
        click(&mut app, &ctx, header);
        assert!(app.midi_editor.selected_notes().is_empty());
        frame(&mut app, &ctx, vec![delete(egui::Key::Backspace, false)]);
        assert!(app.session.project.tracks[0].clips.is_empty());
    }
}

#[test]
fn editor_escape_deselects_clips_without_changing_track_project_or_viewport() {
    let (mut app, ctx, track, id) = super::audio_editor::audio_fixture();
    let mut second = app.session.project.tracks[0].clips[0].clone();
    second.id = Id::new_v4();
    second.start_frame = second.end();
    app.session.project.tracks[0].clips.push(second.clone());
    app.select_clip(id, track, false);
    app.select_clip(second.id, track, true);
    frame(&mut app, &ctx, vec![]);
    let project = app.session.project.clone();
    let dirty = app.dirty;
    let viewport = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
    frame(&mut app, &ctx, vec![delete(egui::Key::Escape, false)]);
    assert!(app.selected_clip.is_none());
    assert!(app.selected_clips.is_empty());
    assert_eq!(app.selected_track, Some(track));
    assert_eq!(app.session.project, project);
    assert_eq!(app.dirty, dirty);
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        viewport
    );
    frame(&mut app, &ctx, vec![delete(egui::Key::Escape, true)]);
    assert_eq!(app.selected_track, Some(track));
}
