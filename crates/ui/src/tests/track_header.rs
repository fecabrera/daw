use super::*;
use crate::mixer::ViewMode;

fn double_click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    // Separate gestures so egui does not classify their clicks as a triple-click.
    let time = ctx.input(|input| input.time) + 1.0;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(time),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    for _ in 0..2 {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(app, ctx, vec![button(point, false)]);
    }
}

fn header_point(app: &DawUi, track: Id) -> Pos2 {
    if app.view_mode == ViewMode::Mixer {
        let strip = app.mixer_bounds[&track];
        Pos2::new(strip.left() + 10.0, strip.top() + 12.0)
    } else {
        Pos2::new(110.0, app.lane_bounds[&track].top() + 18.0)
    }
}

#[test]
fn blank_track_headers_select_only_their_clips_and_allow_copy() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, tracks, ids) = multi_clip_fixture();
        let ctx = context();
        app.view_mode = mode;
        app.select_clip(ids[2], tracks[1], false);
        let before = format!("{:?}", app.session.project);
        frame(&mut app, &ctx, vec![]);
        let point = header_point(&app, tracks[0]);
        // A single click selects the track without replacing the clip selection.
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.selected_track, Some(tracks[0]));
        assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
        double_click(&mut app, &ctx, point);
        assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[1]]));
        assert_eq!(app.selected_clip, Some(ids[0]));
        assert!(app.track_name_edit.is_none());
        app.perform_clip_action(ClipAction::Copy, &ctx);
        assert_eq!(app.clip_clipboard.as_ref().unwrap().clips.len(), 2);
        let point = header_point(&app, tracks[2]);
        double_click(&mut app, &ctx, point);
        assert_eq!(app.selected_track, Some(tracks[2]));
        assert!(app.selected_clips.is_empty());
        assert!(app.selected_clip.is_none());
        assert!(app.track_name_edit.is_none());
        assert!(!app.clip_action_enabled(ClipAction::Copy, &ctx));
        assert!(!app.dirty);
        assert_eq!(format!("{:?}", app.session.project), before);
    }
}

#[test]
fn header_spacing_selects_clips_but_names_and_controls_keep_their_actions() {
    let (mut app, tracks, ids) = multi_clip_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let y = app.lane_bounds[&tracks[0]].top() + 18.0;
    double_click(&mut app, &ctx, Pos2::new(32.0, y)); // Gap after the grip.
    assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[1]]));
    app.select_clip(ids[2], tracks[1], false);
    double_click(&mut app, &ctx, Pos2::new(178.0, y)); // Mute button.
    assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
    assert!(!app.session.project.tracks[0].muted);
    double_click(&mut app, &ctx, Pos2::new(50.0, y)); // Visible name text.
    assert_eq!(app.track_name_edit.as_ref().unwrap().track, tracks[0]);
    assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
}

#[test]
fn blank_header_selection_is_blocked_during_jobs_and_dialogs() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        for dialog in [false, true] {
            let (mut app, tracks, ids) = multi_clip_fixture();
            let ctx = context();
            app.view_mode = mode;
            app.select_clip(ids[2], tracks[1], false);
            frame(&mut app, &ctx, vec![]);
            let point = header_point(&app, tracks[0]);
            let (_sender, receiver) = mpsc::channel();
            if dialog {
                app.settings_open = true;
            } else {
                app.job = Some(receiver);
            }
            double_click(&mut app, &ctx, point);
            assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
            assert_eq!(app.selected_track, Some(tracks[1]));
            assert!(app.track_name_edit.is_none());
            assert!(!app.dirty);
        }
    }
}
