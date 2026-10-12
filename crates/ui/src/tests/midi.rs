use super::*;
use daw_core::{MidiNote, MidiPattern, TrackKind, Waveform};
fn draw_frame(app: &mut DawUi, ctx: &egui::Context, mut events: Vec<egui::Event>) {
    let modifiers = egui::Modifiers {
        command: true,
        mac_cmd: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    for event in &mut events {
        if let egui::Event::PointerButton {
            modifiers: value, ..
        } = event
        {
            *value = modifiers;
        }
    }
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    frame(app, ctx, events);
}
fn draw_click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    draw_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
    frame(app, ctx, vec![]);
}
fn fixture() -> (DawUi, Id, Id, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
    let note = Id::new_v4();
    app.session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![MidiNote {
                    id: note,
                    start_tick: 960,
                    length_ticks: 960,
                    pitch: 60,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    app.session.project.tracks[0].clips[0].length_frames = 96_000;
    app.select_clip(clip, track, false);
    (app, track, clip, note)
}
fn pattern(app: &DawUi) -> &MidiPattern {
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    app.session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap()
}
#[test]
fn midi_tracks_units_editor_and_clip_copy_use_shared_actions() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Units, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.edit(Edit::SetInstrument {
        track_id: track,
        instrument: Some(daw_core::OscillatorSettings {
            waveform: Waveform::Saw,
            ..Default::default()
        }),
    });
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .waveform,
        Waveform::Saw
    );
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.perform_track_action(TrackAction::AddMidi, &ctx);
    assert_eq!(app.session.project.tracks[1].kind, TrackKind::Midi);
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.view_mode, ViewMode::Edit);
    app.perform_clip_action(ClipAction::Copy, &ctx);
    assert!(!app.clip_action_enabled(ClipAction::Paste, &ctx));
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.session.project.transport.playhead_frame = 96_000;
    app.perform_clip_action(ClipAction::Paste, &ctx);
    assert_eq!(app.session.project.tracks[0].clips.len(), 2);
    assert_eq!(
        app.session.project.tracks[0].clips[0].asset_id,
        app.session.project.tracks[0].clips[1].asset_id
    );
    app.set_view_mode(ViewMode::Timeline, &ctx);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(
        !shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Missing source")
        )
    );
}
#[test]
fn toolbar_track_dropdown_switches_units_and_edit_without_changing_the_project() {
    let (mut app, first, first_clip, _) = fixture();
    let second = app.session.project.add_midi_track().unwrap();
    let second_clip = app
        .session
        .project
        .add_midi_pattern(second, 48_000)
        .unwrap();
    let empty = app.session.project.add_midi_track().unwrap();
    for (id, name) in [(first, "Lead"), (second, "Bass"), (empty, "Empty")] {
        app.session
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .name = name.into();
    }
    app.session
        .project
        .edit(Edit::SetInstrument {
            track_id: second,
            instrument: Some(daw_core::OscillatorSettings {
                waveform: Waveform::Square,
                ..Default::default()
            }),
        })
        .unwrap();
    let original = format!("{:?}", app.session.project);
    let dirty = app.dirty;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    for mode in [ViewMode::Units, ViewMode::Edit] {
        app.set_view_mode(mode, &ctx);
        // Edit still follows the active clip after selecting a track in Units.
        let current = "Lead";
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let point = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == current => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap();
        assert!(point.y < 130.0);
        click_note_or_lane(&mut app, &ctx, point);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let bass = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Bass" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        click_note_or_lane(&mut app, &ctx, bass);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.selected_track, Some(second));
        if mode == ViewMode::Units {
            assert_eq!(app.selected_clip, Some(first_clip));
        } else {
            assert_eq!(app.selected_clip, Some(second_clip));
            assert!(app.midi_editor.clip_bounds.contains_key(&second_clip));
            assert!(!app.midi_editor.clip_bounds.contains_key(&first_clip));
        }
    }
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let point = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Bass" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    click_note_or_lane(&mut app, &ctx, point);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let point = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Empty" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    click_note_or_lane(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_track, Some(empty));
    assert!(app.selected_clip.is_none() && app.selected_clips.is_empty());
    assert_eq!(format!("{:?}", app.session.project), original);
    assert_eq!(app.dirty, dirty);
}

#[test]
fn unit_header_text_and_toolbar_divider_follow_the_accent_in_both_themes() {
    let (mut app, _, _, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Units, &ctx);
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Purple,
            settings::AccentColor::Yellow,
        ] {
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    accent,
                    ..Default::default()
                },
            );
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let header = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.fill == accent.color() && r.rect.height() == 24.0 => {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .unwrap();
            let text = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Oscillator" => Some(t),
                    _ => None,
                })
                .unwrap();
            for section in &text.galley.job.sections {
                assert_eq!(section.format.font_id.size, 11.0);
                if accent == settings::AccentColor::Yellow {
                    assert!(section.format.color.r() < 60);
                } else {
                    assert!(
                        section.format.color.r() > 220,
                        "Vivid unit headers need bright text"
                    );
                }
            }
            let text_center = text.pos + text.galley.rect.center().to_vec2();
            assert!((text_center.x - header.center().x).abs() < 1.0);
            assert!((text_center.y - header.center().y).abs() < 1.0);
            let toolbar_bottom = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r)
                        if r.fill == theme::palette(&ctx).panel
                            && r.rect.width() > 1000.0
                            && r.rect.bottom() <= header.top() =>
                    {
                        Some(r.rect.bottom())
                    }
                    _ => None,
                })
                .max_by(f32::total_cmp)
                .unwrap();
            assert!(
                (header.top() - toolbar_bottom - 1.0).abs() < 0.1,
                "Only one point separates the toolbar and unit header: {}",
                header.top() - toolbar_bottom
            );
            let dividers = shapes.iter().filter(|s| matches!(&s.shape,
                egui::Shape::LineSegment { points, stroke, .. }
                    if points[0].y == points[1].y && (points[0].y - (header.top() - 0.5)).abs() < 0.1
                    && (points[1].x - points[0].x).abs() > 1000.0 && stroke.width == 1.0
            )).count();
            assert_eq!(dividers, 1);
        }
    }
}

#[test]
fn piano_roll_add_move_resize_cancel_and_delete_preserve_source_until_release() {
    let (mut app, track, clip, note) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let source = app.session.project.tracks[0].clips[0].asset_id;
    let original = pattern(&app).clone();
    let start = app.midi_editor.note_bounds[&note].center();
    let end = start + Vec2::new(30.0, -18.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.midi_editor.dragging());
    assert_eq!(pattern(&app), &original);
    assert!(!app.file_action_enabled(FileAction::Save));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end + Vec2::new(30.0, 0.0))],
    );
    let release = end + Vec2::new(60.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(release), button(release, false)],
    );
    assert_eq!(pattern(&app).notes[0].start_tick, 1680);
    assert_eq!(pattern(&app).notes[0].pitch, 61);
    assert_ne!(app.session.project.tracks[0].clips[0].asset_id, source);
    let rect = app.midi_editor.note_bounds[&note];
    let edge = Pos2::new(rect.right() - 2.0, rect.center().y);
    let end = edge + Vec2::new(30.0, 0.0);
    let before = pattern(&app).clone();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(pattern(&app), &before);
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
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(pattern(&app), &before);
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.note_bounds[&note];
    let edge = Pos2::new(rect.right() - 2.0, rect.center().y);
    let end = edge + Vec2::new(30.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(pattern(&app), &before);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(pattern(&app).notes[0].length_ticks, 1200);
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.note_bounds[&note];
    let edge = Pos2::new(rect.left() + 2.0, rect.center().y);
    let end = edge + Vec2::new(30.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(pattern(&app).notes[0].start_tick, 1680);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(pattern(&app).notes[0].start_tick, 1920);
    assert_eq!(pattern(&app).notes[0].length_ticks, 960);
    frame(&mut app, &ctx, vec![]);
    let grid = app.midi_editor.grid_bounds;
    let point = Pos2::new(
        grid.left() + 15.0,
        grid.top() + f32::from(127 - 64u8) * 18.0 + 9.0,
    );
    draw_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert_eq!(pattern(&app).notes.len(), 2);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(pattern(&app).notes.len(), 1);
}

#[test]
fn new_destination_tracks_preserve_each_midi_instrument_and_track_gaps() {
    for duplicate in [false, true] {
        let (mut app, first, _, _) = fixture();
        let original = app.session.project.tracks[0].clips[0].clone();
        let second = app.session.project.add_midi_track().unwrap();
        let second_clip = Clip {
            id: Id::new_v4(),
            ..original.clone()
        };
        app.session
            .project
            .edit(Edit::OverwriteClips(vec![daw_core::ClipPlacement {
                track_id: second,
                clip: second_clip.clone(),
            }]))
            .unwrap();
        app.session
            .project
            .edit(Edit::SetInstrument {
                track_id: second,
                instrument: Some(daw_core::OscillatorSettings {
                    waveform: Waveform::Square,
                    ..Default::default()
                }),
            })
            .unwrap();
        let before = format!("{:?}", app.session.project);
        // Deliberately visit the lower destination first. The intervening gap stays empty.
        let previews = vec![
            ClipPreview {
                track: Id::nil(),
                track_index: 4,
                clip: second_clip.clone(),
                valid: true,
                removed: false,
            },
            ClipPreview {
                track: Id::nil(),
                track_index: 2,
                clip: original.clone(),
                valid: true,
                removed: false,
            },
        ];
        assert!(
            app.resolve_move_preview(previews.clone(), duplicate)
                .iter()
                .all(|p| p.valid)
        );
        assert_eq!(format!("{:?}", app.session.project), before);
        app.finish_clip_move(
            &Drag {
                clip: original,
                track: first,
                mode: ClipDragMode::Move,
                origin: Pos2::ZERO,
                duplicate,
                linked_boundary: false,
                clips: Vec::new(),
            },
            previews,
        );
        assert!(app.error.is_none(), "{:?}", app.error);
        let tracks = &app.session.project.tracks;
        assert_eq!(
            tracks[2].instrument.as_ref().unwrap().waveform,
            Waveform::Sine
        );
        assert_eq!(tracks[3].kind, TrackKind::Audio);
        assert!(tracks[3].clips.is_empty());
        assert_eq!(
            tracks[4].instrument.as_ref().unwrap().waveform,
            Waveform::Square
        );
        assert_eq!(tracks[0].clips.len(), usize::from(duplicate));
        assert_eq!(tracks[1].clips.len(), usize::from(duplicate));
        app.session.project.validate().unwrap();
    }
}

fn click_note_or_lane(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn empty_midi_lane_double_click_creates_one_beat_only_in_timeline() {
    let mut app = DawUi::default();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.perform_track_action(TrackAction::AddMidi, &ctx);
    let track = app.selected_track.unwrap();
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let point = Pos2::new(lane.left() + 70.0, lane.center().y);
    click_note_or_lane(&mut app, &ctx, point);
    assert!(app.session.project.tracks[0].clips.is_empty());
    click_note_or_lane(&mut app, &ctx, point);
    assert_eq!(app.view_mode, ViewMode::Timeline);
    let clip = &app.session.project.tracks[0].clips[0];
    assert_eq!(clip.start_frame, 48_000);
    assert_eq!(clip.length_frames, 24_000);
    assert_eq!(pattern(&app).length_ticks, 960);
    assert_eq!(app.selected_clip, Some(clip.id));
    for view in [ViewMode::Units, ViewMode::Edit, ViewMode::Mixer] {
        app.set_view_mode(view, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t)
            if ["Add MIDI track", "New MIDI pattern"].contains(&t.galley.text())))
        );
        let before = format!("{:?}", app.session.project);
        assert!(!app.track_action_enabled(TrackAction::AddMidi, &ctx));
        assert!(!app.track_action_enabled(TrackAction::Duplicate, &ctx));
        app.perform_track_action(TrackAction::AddMidi, &ctx);
        app.add_midi_pattern(track, 96_000, &ctx);
        assert_eq!(format!("{:?}", app.session.project), before);
    }
}

#[test]
fn double_click_deletes_only_the_edited_patterns_note() {
    let (mut app, track, clip, note) = fixture();
    let original = app.session.project.tracks[0].clips[0].clone();
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: Clip {
                id: Id::new_v4(),
                start_frame: original.end(),
                ..original.clone()
            },
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let point = app.midi_editor.note_bounds[&note].center();
    click_note_or_lane(&mut app, &ctx, point);
    assert_eq!(pattern(&app).notes.len(), 1);
    click_note_or_lane(&mut app, &ctx, point);
    assert!(pattern(&app).notes.is_empty());
    assert_eq!(app.session.project.tracks[0].clips.len(), 2);
    let untouched = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == original.asset_id)
        .unwrap();
    assert_eq!(untouched.midi_pattern.as_ref().unwrap().notes.len(), 1);
}

#[test]
fn edit_reuses_timeline_ruler_loop_seek_and_toolbar_zoom() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let timeline_zoom = app.midi_editor.zoom_bounds;
    let ruler_top = app.lane_bounds[&track].top() - RULER_HEIGHT;
    let timeline_transform = (app.zoom, app.scroll);
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.midi_editor.zoom_bounds, timeline_zoom);
    let ruler = app.midi_editor.ruler_bounds;
    assert!((ruler.top() - ruler_top).abs() < 0.1);
    assert!((ruler.height() - RULER_HEIGHT).abs() < 0.1);
    assert!((ruler.left() - app.midi_editor.grid_bounds.left()).abs() < 0.1);
    let seek = Pos2::new(ruler.left() + 180.0, ruler.bottom() - 6.0);
    click_note_or_lane(&mut app, &ctx, seek);
    assert_eq!(app.session.project.transport.playhead_frame, 36_000);
    let start = Pos2::new(ruler.left() + 240.0, ruler.top() + 5.0);
    let end = start + Vec2::new(240.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let region = &app.session.project.transport.r#loop;
    assert_eq!((region.start_frame, region.end_frame), (48_000, 96_000));
    assert!(app.loop_drag.is_none());
    assert!(app.dirty);
    assert_eq!((app.zoom, app.scroll), timeline_transform);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    assert_eq!(app.session.project.transport.playhead_frame, 36_000);
    assert_eq!(app.session.project.transport.r#loop.end_frame, 96_000);
}

#[test]
fn pattern_edge_resize_previews_commits_and_cancels_preserving_note_timing() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let original = format!("{:?}", app.session.project);
    let rect = app.midi_editor.pattern_bounds;
    let start = Pos2::new(rect.right() - 1.0, rect.center().y);
    let end = start + Vec2::new(60.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.midi_editor.dragging());
    assert_eq!(format!("{:?}", app.session.project), original);
    assert!(!app.view_switch_enabled());
    assert!(!app.file_action_enabled(FileAction::Save));
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(pattern(&app).length_ticks, 4320);
    assert_eq!(
        app.session.project.tracks[0].clips[0].length_frames,
        108_000
    );
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.pattern_bounds;
    let start = Pos2::new(rect.left() + 1.0, rect.center().y);
    let end = start + Vec2::new(120.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(pattern(&app).notes[0].start_tick, 960);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(pattern(&app).length_ticks, 3360);
    assert_eq!(pattern(&app).notes[0].start_tick, 0);
    assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 24_000);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let rect = app.midi_editor.pattern_bounds;
    let start = Pos2::new(rect.right() - 1.0, rect.center().y);
    let end = start + Vec2::new(60.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
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
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.midi_editor.dragging());
    assert_eq!(format!("{:?}", app.session.project), before);
}

#[test]
fn piano_roll_note_size_and_snapping_follow_zoom_without_a_subdivision_selector() {
    for (zoom, expected_step) in [(30.0, 960), (480.0, 60)] {
        let mut app = DawUi::default();
        let track = app.session.project.add_midi_track().unwrap();
        let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        app.midi_editor.zoom = zoom;
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "1/16"))
        );
        let grid = app.midi_editor.grid_bounds;
        let point = Pos2::new(
            grid.left() + zoom * 0.5 * 0.35,
            grid.top() + f32::from(127 - 64u8) * 18.0 + 9.0,
        );
        draw_click(&mut app, &ctx, point);
        let note = &pattern(&app).notes[0];
        assert_eq!(note.length_ticks, expected_step);
        assert_eq!(note.start_tick % expected_step, 0);
    }
}

#[test]
fn shift_pattern_resize_is_free_and_focus_loss_cancels_the_pending_edit() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.pattern_bounds;
    let start = Pos2::new(rect.right() - 1.0, rect.center().y);
    let end = start + Vec2::new(1.0, 0.0);
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], true);
    shift_frame(&mut app, &ctx, vec![button(end, false)], true);
    assert_eq!(pattern(&app).length_ticks, 3848);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let rect = app.midi_editor.pattern_bounds;
    let start = Pos2::new(rect.right() - 1.0, rect.center().y);
    let end = start + Vec2::new(60.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.midi_editor.dragging());
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            focused: false,
            events: vec![egui::Event::WindowFocused(false)],
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.midi_editor.dragging());
    assert_eq!(format!("{:?}", app.session.project), before);
}

#[test]
fn drawing_notes_previews_same_pitch_trims_and_commits_only_on_release() {
    let (mut app, track, clip, note_id) = fixture();
    let mut content = pattern(&app).clone();
    let harmony = MidiNote {
        id: Id::new_v4(),
        pitch: 64,
        ..content.notes[0].clone()
    };
    content.notes.push(harmony.clone());
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content,
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let old_asset = app.session.project.tracks[0].clips[0].asset_id;
    let grid = app.midi_editor.grid_bounds;
    let y = grid.top() + f32::from(127 - 60u8) * 18.0 + 9.0;
    let start = Pos2::new(grid.left() + 60.0, y);
    let end = Pos2::new(grid.left() + 180.0, y);
    draw_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    assert!(app.midi_editor.dragging());
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(Default::default()),
            egui::Event::PointerMoved(end),
        ],
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.file_action_enabled(FileAction::Save));
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.midi_editor.dragging());
    let notes = &pattern(&app).notes;
    assert!(notes.contains(&harmony));
    let retained = notes.iter().find(|n| n.id == note_id).unwrap();
    assert_eq!((retained.start_tick, retained.length_ticks), (1440, 480));
    let added = notes
        .iter()
        .find(|n| n.id != note_id && n.pitch == 60)
        .unwrap();
    assert_eq!((added.start_tick, added.length_ticks), (480, 960));
    assert_ne!(app.session.project.tracks[0].clips[0].asset_id, old_asset);
    assert_eq!(
        app.session
            .project
            .assets
            .iter()
            .find(|a| a.id == old_asset)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap()
            .notes
            .len(),
        2
    );
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let grid = app.midi_editor.grid_bounds;
    let start = Pos2::new(
        grid.left() + 360.0,
        grid.top() + f32::from(127 - 65u8) * 18.0 + 9.0,
    );
    let end = start - Vec2::new(60.0, 0.0);
    draw_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
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
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(format!("{:?}", app.session.project), before);
}

#[test]
fn drawing_backwards_keeps_pitch_and_focus_loss_discards_the_preview() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let grid = app.midi_editor.grid_bounds;
    let y = grid.top() + f32::from(127 - 65u8) * 18.0 + 9.0;
    let start = Pos2::new(grid.left() + 360.0, y);
    let end = Pos2::new(grid.left() + 240.0, y - 36.0);
    draw_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let note = pattern(&app).notes.iter().find(|n| n.pitch == 65).unwrap();
    assert_eq!((note.start_tick, note.length_ticks), (1920, 960));
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let grid = app.midi_editor.grid_bounds;
    let start = Pos2::new(
        grid.left() + 60.0,
        grid.top() + f32::from(127 - 67u8) * 18.0 + 9.0,
    );
    let end = start + Vec2::new(60.0, 0.0);
    draw_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    let mut output = ctx.run_ui(
        egui::RawInput {
            focused: false,
            events: vec![egui::Event::WindowFocused(false)],
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.midi_editor.dragging());
}

#[test]
fn fast_note_clicks_and_draws_capture_the_press_even_when_release_is_in_the_same_frame() {
    for (delta, expected_length) in [(0.0, 120), (120.0, 960), (999.0, 3360)] {
        let (mut app, track, clip, _) = fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let grid = app.midi_editor.grid_bounds;
        let start = Pos2::new(
            grid.left() + 60.0,
            grid.top() + f32::from(127 - 65u8) * 18.0 + 9.0,
        );
        let end = start + Vec2::new(delta, 0.0);
        draw_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                button(start, true),
                egui::Event::PointerMoved(end),
                button(end, false),
            ],
        );
        let note = pattern(&app).notes.iter().find(|n| n.pitch == 65).unwrap();
        assert_eq!((note.start_tick, note.length_ticks), (480, expected_length));
        assert!(!app.midi_editor.dragging());
    }
}

#[test]
fn edit_shows_only_selected_tracks_patterns_with_shared_geometry_and_contiguous_borders() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track, first, note) = fixture();
        let source = app.session.project.tracks[0].clips[0].clone();
        let second = Clip {
            id: Id::new_v4(),
            start_frame: source.end(),
            ..source.clone()
        };
        app.session
            .project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: second.clone(),
            })
            .unwrap();
        let other_track = app.session.project.add_midi_track().unwrap();
        let other_clip = app
            .session
            .project
            .add_midi_pattern(other_track, 0)
            .unwrap();
        let other_note = Id::new_v4();
        app.session
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: other_clip,
                pattern: MidiPattern {
                    length_ticks: 960,
                    notes: vec![MidiNote {
                        id: other_note,
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 64,
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.open_clip_editor(first, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let lane = app.midi_editor.track_bounds;
        assert!((lane.top() - app.midi_editor.ruler_bounds.bottom()).abs() < 0.1);
        assert!((lane.left() - app.midi_editor.ruler_bounds.left()).abs() < 0.1);
        assert_eq!(app.midi_editor.clip_bounds.len(), 2);
        assert!(
            !app.midi_editor
                .track_note_bounds
                .contains_key(&(other_clip, other_note))
        );
        let a = app.midi_editor.clip_bounds[&first];
        let b = app.midi_editor.clip_bounds[&second.id];
        assert!((a.right() - b.left()).abs() < 0.1);
        let note_a = app.midi_editor.track_note_bounds[&(first, note)][0];
        let note_b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
        assert!(note_a.top() >= lane.bottom());
        assert!((note_a.left() - a.left() - 120.0).abs() < 0.1);
        assert!((note_b.left() - b.left() - 120.0).abs() < 0.1);
        for block in [a, b] {
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect.height() > 56.0 && (r.rect.left() - block.left()).abs() < 0.1
                    && (r.rect.right() - block.right()).abs() < 0.1
                    && r.stroke.width == 1.0 && r.stroke.color == theme::palette(&ctx).secondary))
            );
        }
        let transform = (app.midi_editor.zoom, app.midi_editor.scroll);
        click_note_or_lane(&mut app, &ctx, b.center());
        assert_eq!(app.selected_clip, Some(second.id));
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), transform);
        app.midi_editor.zoom = 120.0;
        app.midi_editor.scroll = 0.5;
        frame(&mut app, &ctx, vec![]);
        let b = app.midi_editor.clip_bounds[&second.id];
        let note_b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
        assert!((note_b.left() - b.left() - 60.0).abs() < 0.1);
        assert!((b.left() - lane.left() - 180.0).abs() < 0.1);
    }
}

#[test]
fn editing_an_unselected_pattern_keeps_the_viewport_and_other_source_users() {
    let (mut app, track, first, note) = fixture();
    let source = app.session.project.tracks[0].clips[0].clone();
    let second = Clip {
        id: Id::new_v4(),
        start_frame: source.end(),
        ..source.clone()
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(first, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.track_note_bounds[&(second.id, note)][0];
    let start = rect.center();
    let end = start + Vec2::new(30.0, -18.0);
    let original = format!("{:?}", app.session.project);
    let transform = (app.midi_editor.zoom, app.midi_editor.scroll);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.midi_editor.dragging());
    assert_eq!(app.selected_clip, Some(second.id));
    assert_eq!(format!("{:?}", app.session.project), original);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), transform);
    assert_eq!(pattern(&app).notes[0].pitch, 60);
    let edited = app.session.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == second.id)
        .unwrap();
    assert_ne!(edited.asset_id, source.asset_id);
    let edited_pattern = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == edited.asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(
        (
            edited_pattern.notes[0].start_tick,
            edited_pattern.notes[0].pitch
        ),
        (1200, 61)
    );
    frame(&mut app, &ctx, vec![]);
    let block = app.midi_editor.clip_bounds[&first];
    let grid = app.midi_editor.grid_bounds;
    let point = Pos2::new(
        block.left() + 30.0,
        grid.top() + f32::from(127 - 64u8) * 18.0 + 9.0,
    );
    draw_click(&mut app, &ctx, point);
    assert_eq!(app.selected_clip, Some(first));
    assert_eq!(pattern(&app).notes.len(), 2);
    assert_eq!(edited_pattern.notes.len(), 1);
}

#[test]
fn looped_trimmed_patterns_show_and_edit_each_visible_repetition() {
    let (mut app, track, clip, note) = fixture();
    app.session.project.tracks[0].clips[0].source_offset_frame = 24_000;
    app.session.project.tracks[0].clips[0].repeat = Some(daw_core::ClipLoop {
        length_frames: 48_000,
        phase_frame: 0,
    });
    app.session.project.tracks[0].clips[0].length_frames = 120_000;
    app.session.project.validate().unwrap();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let notes = &app.midi_editor.track_note_bounds[&(clip, note)];
    assert_eq!(notes.len(), 3);
    let lane = app.midi_editor.clip_bounds[&clip];
    for (index, rect) in notes.iter().enumerate() {
        assert!((rect.left() - lane.left() - index as f32 * 240.0).abs() < 0.1);
        assert!((rect.width() - 120.0).abs() < 0.1);
    }
    let point = notes[1].center() + Vec2::new(150.0, -18.0);
    // This empty row is in repetition 1; it maps back to source tick 2640.
    draw_click(&mut app, &ctx, point);
    let added = pattern(&app)
        .notes
        .iter()
        .find(|n| n.id != note)
        .unwrap()
        .clone();
    assert_eq!((added.start_tick, added.pitch), (2640, 61));
    frame(&mut app, &ctx, vec![]);
    assert!(app.midi_editor.track_note_bounds[&(clip, added.id)].len() >= 2);
}

#[test]
fn left_pattern_extension_moves_the_border_keeps_notes_fixed_and_clamps_at_project_start() {
    let (mut app, track, clip, note) = fixture();
    app.session.project.tracks[0].clips[0].start_frame = 48_000;
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.scroll = 0.0;
    frame(&mut app, &ctx, vec![]);
    let original = format!("{:?}", app.session.project);
    let note_x = app.midi_editor.track_note_bounds[&(clip, note)][0].left();
    let block = app.midi_editor.clip_bounds[&clip];
    let start = Pos2::new(block.left() + 1.0, block.top() + 10.0);
    let end = start - Vec2::new(120.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(format!("{:?}", app.session.project), original);
    assert!((app.midi_editor.clip_bounds[&clip].left() - block.left() + 120.0).abs() < 0.1);
    assert!((app.midi_editor.track_note_bounds[&(clip, note)][0].left() - note_x).abs() < 0.1);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let current = &app.session.project.tracks[0].clips[0];
    assert_eq!(
        (current.start_frame, current.length_frames),
        (24_000, 120_000)
    );
    assert_eq!(pattern(&app).notes[0].start_tick, 1920);
    frame(&mut app, &ctx, vec![]);
    let rect = app.midi_editor.clip_bounds[&clip];
    let start = Pos2::new(rect.left() + 1.0, rect.top() + 10.0);
    let end = start - Vec2::new(240.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
    assert_eq!(app.session.project.tracks[0].clips[0].end(), 144_000);
    assert!((app.midi_editor.track_note_bounds[&(clip, note)][0].left() - note_x).abs() < 0.1);
}

#[test]
fn dense_repeats_at_minimum_zoom_keep_paint_work_bounded() {
    let (mut app, track, clip, _) = fixture();
    let current = &mut app.session.project.tracks[0].clips[0];
    current.source_offset_frame = 24_000;
    current.repeat = Some(daw_core::ClipLoop {
        length_frames: 1,
        phase_frame: 0,
    });
    current.length_frames = 48_000 * 600;
    app.session.project.validate().unwrap();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    app.midi_editor.zoom = MIN_ZOOM;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(shapes.len() < 2_000);
    assert!(app.midi_editor.track_note_bounds.is_empty());
    assert!((app.midi_editor.clip_bounds[&clip].width() - 600.0).abs() < 0.1);
    app.midi_editor.zoom = MAX_ZOOM;
    frame(&mut app, &ctx, vec![]);
    assert!(app.midi_editor.clip_bounds.contains_key(&clip));
}

#[test]
fn plain_marquee_selects_pitches_without_editing_and_delete_removes_the_group() {
    let (mut app, track, clip, first) = fixture();
    let second = Id::new_v4();
    let mut content = pattern(&app).clone();
    content.notes.push(MidiNote {
        id: second,
        pitch: 64,
        ..content.notes[0].clone()
    });
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content,
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let dirty = app.dirty;
    let lower = app.midi_editor.note_bounds[&first];
    let upper = app.midi_editor.note_bounds[&second];
    let start = upper.left_top() - Vec2::splat(5.0);
    let end = lower.right_bottom() + Vec2::splat(5.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.midi_editor.selecting());
    assert_eq!(
        *app.midi_editor.selected_notes(),
        HashSet::from([(clip, first), (clip, second)])
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.midi_editor.dragging());
    assert_eq!(app.dirty, dirty);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(pattern(&app).notes.is_empty());
    assert!(app.midi_editor.selected_notes().is_empty());
}

#[test]
fn backwards_marquee_spans_pattern_gaps_and_distinguishes_shared_note_ids() {
    let (mut app, track, first, note) = fixture();
    let source = app.session.project.tracks[0].clips[0].clone();
    let second = Clip {
        id: Id::new_v4(),
        start_frame: 120_000,
        ..source.clone()
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
    let other = app.session.project.add_midi_track().unwrap();
    let untouched = Clip {
        id: Id::new_v4(),
        ..source
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: other,
            clip: untouched.clone(),
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(first, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let a = app.midi_editor.track_note_bounds[&(first, note)][0];
    let b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
    let start = b.right_bottom() + Vec2::splat(5.0);
    let end = a.left_top() - Vec2::splat(5.0);
    // A fast backwards gesture includes both copies but excludes the other track.
    let before = format!("{:?}", app.session.project);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            button(start, true),
            egui::Event::PointerMoved(end),
            button(end, false),
        ],
    );
    assert_eq!(
        *app.midi_editor.selected_notes(),
        HashSet::from([(first, note), (second.id, note)])
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    for clip in &app.session.project.tracks[0].clips {
        let asset = app
            .session
            .project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .unwrap();
        assert!(asset.midi_pattern.as_ref().unwrap().notes.is_empty());
    }
    assert_eq!(
        app.session.project.tracks[1].clips[0].asset_id,
        untouched.asset_id
    );
    let original = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == untouched.asset_id)
        .unwrap();
    assert_eq!(original.midi_pattern.as_ref().unwrap().notes.len(), 1);
}

#[test]
fn marquee_mode_is_latched_and_cancel_restores_the_previous_selection() {
    for lose_focus in [false, true] {
        let (mut app, track, clip, note) = fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let bounds = app.midi_editor.note_bounds[&note];
        click_note_or_lane(&mut app, &ctx, bounds.center());
        let before = format!("{:?}", app.session.project);
        let start = bounds.left_top() - Vec2::splat(10.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        // Pressing Command/Ctrl after the gesture starts must not turn it into drawing.
        draw_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start - Vec2::splat(5.0))],
        );
        assert!(app.midi_editor.selecting());
        assert!(app.midi_editor.selected_notes().is_empty());
        if lose_focus {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    events: vec![egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
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
        }
        frame(&mut app, &ctx, vec![button(start, false)]);
        assert!(!app.midi_editor.dragging());
        assert_eq!(
            *app.midi_editor.selected_notes(),
            HashSet::from([(clip, note)])
        );
        assert_eq!(format!("{:?}", app.session.project), before);
    }
}

#[test]
fn empty_click_clears_selection_without_drawing_and_loop_copies_select_once() {
    let (mut app, track, clip, note) = fixture();
    app.session.project.tracks[0].clips[0].repeat = Some(daw_core::ClipLoop {
        length_frames: 48_000,
        phase_frame: 0,
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let copies = app.midi_editor.track_note_bounds[&(clip, note)].clone();
    let start = copies[0].left_top() - Vec2::splat(5.0);
    let end = copies[1].right_bottom() + Vec2::splat(5.0);
    let before = format!("{:?}", app.session.project);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end), button(end, false)],
    );
    assert_eq!(
        *app.midi_editor.selected_notes(),
        HashSet::from([(clip, note)])
    );
    click_note_or_lane(&mut app, &ctx, start);
    assert!(app.midi_editor.selected_notes().is_empty());
    assert_eq!(format!("{:?}", app.session.project), before);
}

fn copy_note_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    mut events: Vec<egui::Event>,
    duplicate: bool,
    shift: bool,
) {
    let modifiers = egui::Modifiers {
        alt: duplicate && cfg!(target_os = "macos"),
        ctrl: duplicate && !cfg!(target_os = "macos"),
        command: duplicate && !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    };
    for event in &mut events {
        if let egui::Event::PointerButton {
            modifiers: value, ..
        } = event
        {
            *value = modifiers;
        }
    }
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    frame(app, ctx, events);
}
fn select_note_box(app: &mut DawUi, ctx: &egui::Context, bounds: Rect) {
    let start = bounds.left_top() - Vec2::splat(5.0);
    let end = bounds.right_bottom() + Vec2::splat(5.0);
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(end), button(end, false)],
    );
    frame(app, ctx, vec![]);
}
fn pattern_for_clip(app: &DawUi, clip: Id) -> &MidiPattern {
    let asset_id = app
        .session
        .project
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == clip)
        .unwrap()
        .asset_id;
    app.session
        .project
        .assets
        .iter()
        .find(|a| a.id == asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap()
}

#[test]
fn modifier_note_drag_previews_stable_copies_and_retains_the_original_source() {
    let (mut app, track, clip, note) = fixture();
    let original = pattern(&app).clone();
    let source = app.session.project.tracks[0].clips[0].clone();
    let second = Clip {
        id: Id::new_v4(),
        start_frame: source.end(),
        ..source
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let start = app.midi_editor.note_bounds[&note].center();
    let end = start + Vec2::new(60.0, -18.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    let selected = app.midi_editor.selected_notes().clone();
    assert_eq!(selected.len(), 1);
    let copy = selected.iter().next().unwrap().1;
    assert_ne!(copy, note);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.file_action_enabled(FileAction::Save));
    copy_note_frame(&mut app, &ctx, vec![], true, false);
    assert_eq!(app.midi_editor.selected_notes(), &selected);
    assert!(
        app.midi_editor
            .track_note_bounds
            .contains_key(&(clip, note))
    );
    assert!(
        app.midi_editor
            .track_note_bounds
            .contains_key(&(clip, copy))
    );
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    assert!(!app.midi_editor.dragging());
    let notes = &pattern(&app).notes;
    assert_eq!(notes.len(), 2);
    assert!(notes.contains(&original.notes[0]));
    let copied = notes.iter().find(|n| n.id == copy).unwrap();
    assert_eq!(
        (
            copied.start_tick,
            copied.pitch,
            copied.length_ticks,
            copied.velocity
        ),
        (1440, 61, 960, 100)
    );
    assert_eq!(pattern_for_clip(&app, second.id), &original);
}

#[test]
fn selected_note_groups_duplicate_and_move_with_shared_time_and_pitch_offsets() {
    let (mut app, track, clip, note) = fixture();
    let mut content = pattern(&app).clone();
    let second = MidiNote {
        id: Id::new_v4(),
        start_tick: 2160,
        length_ticks: 480,
        pitch: 64,
        velocity: 37,
    };
    content.notes.push(second.clone());
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content.clone(),
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let bounds = app.midi_editor.note_bounds[&note].union(app.midi_editor.note_bounds[&second.id]);
    select_note_box(&mut app, &ctx, bounds);
    assert_eq!(app.midi_editor.selected_notes().len(), 2);
    let start = app.midi_editor.note_bounds[&second.id].center();
    let end = start + Vec2::new(60.0, -18.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    assert_eq!(pattern(&app), &content);
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    let copies = app.midi_editor.selected_notes().clone();
    assert_eq!(copies.len(), 2);
    assert_eq!(pattern(&app).notes.len(), 4);
    for original in &content.notes {
        assert!(pattern(&app).notes.contains(original));
        assert!(
            pattern(&app)
                .notes
                .iter()
                .any(|n| copies.contains(&(clip, n.id))
                    && n.start_tick == original.start_tick + 480
                    && n.pitch == original.pitch + 1
                    && n.length_ticks == original.length_ticks
                    && n.velocity == original.velocity)
        );
    }
    frame(&mut app, &ctx, vec![]);
    let copy_id = pattern(&app)
        .notes
        .iter()
        .find(|n| copies.contains(&(clip, n.id)) && n.velocity == 37)
        .unwrap()
        .id;
    let start = app.midi_editor.note_bounds[&copy_id].center();
    let end = start + Vec2::new(-30.0, 18.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        false,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        false,
        false,
    );
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], false, false);
    assert_eq!(app.midi_editor.selected_notes(), &copies);
    assert_eq!(pattern(&app).notes.len(), 4);
    for original in &content.notes {
        assert!(pattern(&app).notes.contains(original));
        assert!(
            pattern(&app)
                .notes
                .iter()
                .any(|n| copies.contains(&(clip, n.id))
                    && n.start_tick == original.start_tick + 240
                    && n.pitch == original.pitch
                    && n.length_ticks == original.length_ticks
                    && n.velocity == original.velocity)
        );
    }
}

#[test]
fn note_group_copy_clamps_as_a_group_and_zero_displacement_is_a_no_op() {
    let (mut app, track, clip, note) = fixture();
    let mut content = pattern(&app).clone();
    let second = MidiNote {
        id: Id::new_v4(),
        start_tick: 2880,
        pitch: 64,
        ..content.notes[0].clone()
    };
    content.notes.push(second.clone());
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content.clone(),
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let bounds = app.midi_editor.note_bounds[&note].union(app.midi_editor.note_bounds[&second.id]);
    select_note_box(&mut app, &ctx, bounds);
    let start = app.midi_editor.note_bounds[&note].center();
    let end = start + Vec2::new(60.0, 0.0);
    let before = format!("{:?}", app.session.project);
    let dirty = app.dirty;
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert_eq!(app.dirty, dirty);
    frame(&mut app, &ctx, vec![]);
    let start = app.midi_editor.note_bounds[&note].center();
    let end = start + Vec2::new(-500.0, -2000.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    let copies: Vec<_> = pattern(&app)
        .notes
        .iter()
        .filter(|n| app.midi_editor.selected_notes().contains(&(clip, n.id)))
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().any(|n| n.start_tick == 0 && n.pitch == 123));
    assert!(
        copies
            .iter()
            .any(|n| n.start_tick == 1920 && n.pitch == 127)
    );
    assert!(
        content
            .notes
            .iter()
            .all(|n| pattern(&app).notes.contains(n))
    );
}

#[test]
fn note_copy_modifier_can_change_during_drag_and_shift_bypasses_snapping() {
    for duplicate_on_release in [false, true] {
        let (mut app, track, clip, note) = fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let start = app.midi_editor.note_bounds[&note].center();
        let end = start + Vec2::new(13.0, 0.0);
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            false,
            true,
        );
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end)],
            true,
            true,
        );
        let copy = app.midi_editor.selected_notes().iter().next().unwrap().1;
        assert_ne!(copy, note);
        copy_note_frame(&mut app, &ctx, vec![], false, true);
        assert_eq!(
            app.midi_editor.selected_notes(),
            &HashSet::from([(clip, note)])
        );
        copy_note_frame(
            &mut app,
            &ctx,
            vec![button(end, false)],
            duplicate_on_release,
            true,
        );
        let notes = &pattern(&app).notes;
        assert_eq!(notes.len(), if duplicate_on_release { 2 } else { 1 });
        let changed = notes.iter().find(|n| n.start_tick == 1064).unwrap();
        assert_eq!(changed.id, if duplicate_on_release { copy } else { note });
        assert_eq!(
            (changed.pitch, changed.length_ticks, changed.velocity),
            (60, 960, 100)
        );
        if duplicate_on_release {
            assert!(notes.iter().any(|n| n.id == note && n.start_tick == 960));
        }
    }
}

#[test]
fn cancelling_a_note_copy_restores_sources_and_selection() {
    for lose_focus in [false, true] {
        let (mut app, track, clip, note) = fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let start = app.midi_editor.note_bounds[&note].center();
        click_note_or_lane(&mut app, &ctx, start);
        let before = format!("{:?}", app.session.project);
        let selected = app.midi_editor.selected_notes().clone();
        let end = start + Vec2::new(60.0, -18.0);
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            true,
            false,
        );
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end)],
            true,
            false,
        );
        assert_ne!(app.midi_editor.selected_notes(), &selected);
        if lose_focus {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    events: vec![egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
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
        }
        copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
        assert!(!app.midi_editor.dragging());
        assert_eq!(format!("{:?}", app.session.project), before);
        assert_eq!(app.midi_editor.selected_notes(), &selected);
    }
}

#[test]
fn group_note_copy_updates_patterns_once_per_clip_and_preserves_stretched_spacing() {
    let (mut app, track, first, note) = fixture();
    let source = app.session.project.tracks[0].clips[0].clone();
    app.session.project.tracks[0].clips[0].repeat = Some(daw_core::ClipLoop {
        length_frames: 48_000,
        phase_frame: 0,
    });
    let second = Clip {
        id: Id::new_v4(),
        start_frame: 96_000,
        length_frames: 192_000,
        stretch: daw_core::ClipStretch::new(1, 2),
        ..source.clone()
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(first, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.midi_editor.track_note_bounds[&(first, note)].len(), 2);
    let a = app.midi_editor.track_note_bounds[&(first, note)][0];
    let b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
    select_note_box(&mut app, &ctx, a.union(b));
    assert_eq!(app.midi_editor.selected_notes().len(), 2);
    let start = b.center();
    let end = start + Vec2::new(60.0, -18.0);
    let before = format!("{:?}", app.session.project);
    let assets = app.session.project.assets.len();
    let viewport = (app.midi_editor.zoom, app.midi_editor.scroll);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    assert_eq!(app.selected_clip, Some(second.id));
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), viewport);
    assert_eq!(app.session.project.assets.len(), assets + 2);
    let selected = app.midi_editor.selected_notes();
    let mut ids = HashSet::new();
    for (clip, expected_start) in [(first, 1440), (second.id, 1200)] {
        let pattern = pattern_for_clip(&app, clip);
        assert_eq!(pattern.notes.len(), 2);
        assert!(
            pattern
                .notes
                .iter()
                .any(|n| n.id == note && n.start_tick == 960 && n.pitch == 60)
        );
        let copy = pattern
            .notes
            .iter()
            .find(|n| selected.contains(&(clip, n.id)))
            .unwrap();
        assert_eq!(
            (copy.start_tick, copy.pitch, copy.length_ticks),
            (expected_start, 61, 960)
        );
        assert!(ids.insert(copy.id));
    }
    let original = app
        .session
        .project
        .assets
        .iter()
        .find(|a| a.id == source.asset_id)
        .unwrap();
    assert_eq!(original.midi_pattern.as_ref().unwrap().notes.len(), 1);
    app.session.project.validate().unwrap();
}

#[test]
fn copy_modifier_on_a_note_edge_resizes_without_adding_a_copy() {
    let (mut app, track, clip, note) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let bounds = app.midi_editor.note_bounds[&note];
    let start = Pos2::new(bounds.right() - 2.0, bounds.center().y);
    let end = start + Vec2::new(30.0, 0.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
        false,
    );
    copy_note_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        true,
        false,
    );
    copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, false);
    assert_eq!(pattern(&app).notes.len(), 1);
    assert_eq!(
        (
            pattern(&app).notes[0].id,
            pattern(&app).notes[0].length_ticks
        ),
        (note, 1200)
    );
}

#[test]
fn fast_note_drag_copy_captures_the_press_and_commits_on_release() {
    let (mut app, track, clip, note) = fixture();
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let start = app.midi_editor.note_bounds[&note].center();
    let end = start + Vec2::new(60.0, -18.0);
    copy_note_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            button(start, true),
            egui::Event::PointerMoved(end),
            button(end, false),
        ],
        true,
        false,
    );
    assert_eq!(pattern(&app).notes.len(), 2);
    assert!(!app.midi_editor.dragging());
    assert!(
        pattern(&app)
            .notes
            .iter()
            .any(|n| n.id != note && n.start_tick == 1440 && n.pitch == 61)
    );
}

fn shift_note_click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    shift_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        true,
    );
    shift_frame(app, ctx, vec![button(point, false)], true);
    frame(app, ctx, vec![]);
}

#[test]
fn shift_click_toggles_notes_across_patterns_without_deleting_on_a_double_click() {
    let (mut app, track, first, note) = fixture();
    let source = app.session.project.tracks[0].clips[0].clone();
    let second = Clip {
        id: Id::new_v4(),
        start_frame: source.end(),
        ..source
    };
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
    let ctx = context();
    app.open_clip_editor(first, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let before = format!("{:?}", app.session.project);
    let dirty = app.dirty;
    let viewport = (app.midi_editor.zoom, app.midi_editor.scroll);
    let a = app.midi_editor.track_note_bounds[&(first, note)][0].center();
    let b = app.midi_editor.track_note_bounds[&(second.id, note)][0].center();
    shift_note_click(&mut app, &ctx, a);
    assert_eq!(
        app.midi_editor.selected_notes(),
        &HashSet::from([(first, note)])
    );
    shift_note_click(&mut app, &ctx, b);
    assert_eq!(
        app.midi_editor.selected_notes(),
        &HashSet::from([(first, note), (second.id, note)])
    );
    assert_eq!(app.selected_clip, Some(second.id));
    // The same position clicked twice quickly reports a double click; Shift still only toggles.
    shift_note_click(&mut app, &ctx, b);
    assert_eq!(
        app.midi_editor.selected_notes(),
        &HashSet::from([(first, note)])
    );
    shift_note_click(&mut app, &ctx, a);
    assert!(app.midi_editor.selected_notes().is_empty());
    assert_eq!(format!("{:?}", app.session.project), before);
    assert_eq!(app.dirty, dirty);
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), viewport);
}

#[test]
fn shift_marquee_toggles_from_its_original_selection_and_retains_notes_outside_the_box() {
    let (mut app, track, clip, first) = fixture();
    let mut content = pattern(&app).clone();
    let second = MidiNote {
        id: Id::new_v4(),
        pitch: 64,
        ..content.notes[0].clone()
    };
    let outside = MidiNote {
        id: Id::new_v4(),
        pitch: 67,
        ..content.notes[0].clone()
    };
    content.notes.extend([second.clone(), outside.clone()]);
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content,
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let a = app.midi_editor.note_bounds[&first];
    let b = app.midi_editor.note_bounds[&second.id];
    let c = app.midi_editor.note_bounds[&outside.id];
    shift_note_click(&mut app, &ctx, a.center());
    shift_note_click(&mut app, &ctx, c.center());
    let before = format!("{:?}", app.session.project);
    let start = b.left_top() - Vec2::splat(5.0);
    let end = a.right_bottom() + Vec2::splat(5.0);
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], true);
    let expected = HashSet::from([(clip, second.id), (clip, outside.id)]);
    assert_eq!(app.midi_editor.selected_notes(), &expected);
    shift_frame(&mut app, &ctx, vec![], true);
    assert_eq!(app.midi_editor.selected_notes(), &expected);
    // Releasing Shift during a box gesture must not change its initial toggle mode.
    shift_frame(&mut app, &ctx, vec![button(end, false)], false);
    assert_eq!(app.midi_editor.selected_notes(), &expected);
    shift_note_click(&mut app, &ctx, start);
    assert_eq!(app.midi_editor.selected_notes(), &expected);
    assert_eq!(format!("{:?}", app.session.project), before);
    // A subsequent ordinary empty click still clears the entire note selection.
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        false,
    );
    shift_frame(&mut app, &ctx, vec![button(start, false)], false);
    assert!(app.midi_editor.selected_notes().is_empty());
}

#[test]
fn shift_marquee_deduplicates_repetitions_and_cancels_back_to_the_previous_selection() {
    for cancel in [false, true] {
        let (mut app, track, first, note) = fixture();
        let source = app.session.project.tracks[0].clips[0].clone();
        app.session.project.tracks[0].clips[0].repeat = Some(daw_core::ClipLoop {
            length_frames: 48_000,
            phase_frame: 0,
        });
        let second = Clip {
            id: Id::new_v4(),
            start_frame: source.end(),
            ..source
        };
        app.session
            .project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: second.clone(),
            })
            .unwrap();
        let ctx = context();
        app.open_clip_editor(first, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let a = app.midi_editor.track_note_bounds[&(first, note)][0];
        let b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
        assert_eq!(app.midi_editor.track_note_bounds[&(first, note)].len(), 2);
        shift_note_click(&mut app, &ctx, a.center());
        let before = format!("{:?}", app.session.project);
        let start = b.right_bottom() + Vec2::splat(5.0);
        let end = a.left_top() - Vec2::splat(5.0);
        shift_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            true,
        );
        shift_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], true);
        assert_eq!(
            app.midi_editor.selected_notes(),
            &HashSet::from([(second.id, note)])
        );
        if cancel {
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
        }
        shift_frame(&mut app, &ctx, vec![button(end, false)], true);
        assert!(!app.midi_editor.dragging());
        let expected = if cancel { first } else { second.id };
        assert_eq!(
            app.midi_editor.selected_notes(),
            &HashSet::from([(expected, note)])
        );
        assert_eq!(format!("{:?}", app.session.project), before);
    }
}

#[test]
fn shift_selected_notes_still_move_as_a_group_with_free_time_adjustment() {
    let (mut app, track, clip, first) = fixture();
    let mut content = pattern(&app).clone();
    let second = MidiNote {
        id: Id::new_v4(),
        pitch: 64,
        ..content.notes[0].clone()
    };
    content.notes.push(second.clone());
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content,
    });
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    let a = app.midi_editor.note_bounds[&first].center();
    let b = app.midi_editor.note_bounds[&second.id].center();
    shift_note_click(&mut app, &ctx, a);
    shift_note_click(&mut app, &ctx, b);
    let selected = app.midi_editor.selected_notes().clone();
    assert_eq!(selected.len(), 2);
    let end = b + Vec2::new(13.0, 0.0);
    shift_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(b), button(b, true)],
        true,
    );
    shift_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], true);
    assert!(pattern(&app).notes.iter().all(|n| n.start_tick == 960));
    shift_frame(&mut app, &ctx, vec![button(end, false)], true);
    assert_eq!(app.midi_editor.selected_notes(), &selected);
    assert!(pattern(&app).notes.iter().all(|n| n.start_tick == 1064));
    assert_eq!(pattern(&app).notes.len(), 2);
}

fn note_resize_fixture() -> (DawUi, Id, Id, Id, Id) {
    let (mut app, track, clip, first) = fixture();
    let mut content = pattern(&app).clone();
    let second = MidiNote {
        id: Id::new_v4(),
        start_tick: 2160,
        length_ticks: 480,
        pitch: 64,
        velocity: 37,
    };
    content.notes.push(second.clone());
    app.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: content,
    });
    (app, track, clip, first, second.id)
}
fn note_edge(bounds: Rect, left: bool) -> Pos2 {
    Pos2::new(
        if left {
            bounds.left() + 2.0
        } else {
            bounds.right() - 2.0
        },
        bounds.center().y,
    )
}

#[test]
fn resizing_selected_note_edges_previews_the_group_and_commits_once() {
    for left in [false, true] {
        let (mut app, track, clip, first, second) = note_resize_fixture();
        let mut content = pattern(&app).clone();
        let unselected = MidiNote {
            id: Id::new_v4(),
            pitch: 67,
            ..content.notes[0].clone()
        };
        content.notes.push(unselected.clone());
        app.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: content,
        });
        let source = app.session.project.tracks[0].clips[0].clone();
        let untouched = Clip {
            id: Id::new_v4(),
            start_frame: source.end(),
            ..source.clone()
        };
        app.session
            .project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: untouched.clone(),
            })
            .unwrap();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let bounds =
            app.midi_editor.note_bounds[&first].union(app.midi_editor.note_bounds[&second]);
        select_note_box(&mut app, &ctx, bounds);
        let selected = app.midi_editor.selected_notes().clone();
        let original = pattern(&app).clone();
        let before = format!("{:?}", app.session.project);
        let assets = app.session.project.assets.len();
        let start = note_edge(app.midi_editor.note_bounds[&first], left);
        let end = start + Vec2::new(if left { -30.0 } else { 30.0 }, -18.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(app.midi_editor.dragging());
        assert!(!app.file_action_enabled(FileAction::Save));
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![]);
        for id in [first, second] {
            let resized = app.midi_editor.note_bounds[&id];
            let n = original.notes.iter().find(|n| n.id == id).unwrap();
            assert!((resized.width() - (n.length_ticks as f32 / 8.0 + 30.0)).abs() < 0.1);
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(!app.midi_editor.dragging());
        assert_eq!(app.midi_editor.selected_notes(), &selected);
        assert_eq!(app.session.project.assets.len(), assets + 1);
        assert_eq!(pattern_for_clip(&app, untouched.id), &original);
        assert!(pattern(&app).notes.contains(&unselected));
        for old in original
            .notes
            .iter()
            .filter(|n| selected.contains(&(clip, n.id)))
        {
            let next = pattern(&app).notes.iter().find(|n| n.id == old.id).unwrap();
            assert_eq!(next.length_ticks, old.length_ticks + 240);
            assert_eq!(
                next.start_tick,
                if left {
                    old.start_tick - 240
                } else {
                    old.start_tick
                }
            );
            assert_eq!((next.pitch, next.velocity), (old.pitch, old.velocity));
            if left {
                assert_eq!(
                    next.start_tick + next.length_ticks,
                    old.start_tick + old.length_ticks
                );
            }
        }
        let current = &app.session.project.tracks[0].clips[0];
        assert_eq!(
            (current.start_frame, current.length_frames),
            (source.start_frame, source.length_frames)
        );
    }
}

#[test]
fn group_note_shortening_clamps_at_the_shortest_note_for_both_edges() {
    for left in [false, true] {
        let (mut app, track, clip, first, second) = note_resize_fixture();
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let bounds =
            app.midi_editor.note_bounds[&first].union(app.midi_editor.note_bounds[&second]);
        select_note_box(&mut app, &ctx, bounds);
        let original = pattern(&app).clone();
        let start = note_edge(app.midi_editor.note_bounds[&first], left);
        let end = start + Vec2::new(if left { 500.0 } else { -500.0 }, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(pattern(&app).notes.len(), 2);
        for old in &original.notes {
            let next = pattern(&app).notes.iter().find(|n| n.id == old.id).unwrap();
            assert_eq!(next.length_ticks, old.length_ticks - 479);
            assert_eq!(
                next.start_tick,
                if left {
                    old.start_tick + 479
                } else {
                    old.start_tick
                }
            );
        }
        assert_eq!(
            pattern(&app)
                .notes
                .iter()
                .find(|n| n.id == second)
                .unwrap()
                .length_ticks,
            1
        );
        app.session.project.validate().unwrap();
    }
}

#[test]
fn group_note_expansion_clamps_at_each_pattern_boundary_and_preserves_no_op_sources() {
    for left in [false, true] {
        let (mut app, track, clip, first, second) = note_resize_fixture();
        let mut original = pattern(&app).clone();
        let n = original.notes.iter_mut().find(|n| n.id == second).unwrap();
        n.start_tick = if left { 0 } else { 3360 };
        app.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: original.clone(),
        });
        let ctx = context();
        app.open_clip_editor(clip, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        // Starting a box before tick zero is outside the viewport, so use Shift-click here.
        let a = app.midi_editor.note_bounds[&first].center();
        let b = app.midi_editor.note_bounds[&second].center();
        shift_note_click(&mut app, &ctx, a);
        shift_note_click(&mut app, &ctx, b);
        assert_eq!(app.midi_editor.selected_notes().len(), 2);
        let before = format!("{:?}", app.session.project);
        let dirty = app.dirty;
        let start = note_edge(app.midi_editor.note_bounds[&first], left);
        let end = start + Vec2::new(if left { -60.0 } else { 60.0 }, 0.0);
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            false,
            false,
        );
        copy_note_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end)],
            false,
            false,
        );
        copy_note_frame(&mut app, &ctx, vec![button(end, false)], false, false);
        assert_eq!(format!("{:?}", app.session.project), before);
        assert_eq!(app.dirty, dirty);
        assert_eq!(pattern(&app), &original);
        assert_eq!(app.midi_editor.selected_notes().len(), 2);
    }
}

#[test]
fn group_note_resize_handles_stretch_and_repeats_per_clip_atomically() {
    for left in [false, true] {
        let (mut app, track, first, note) = fixture();
        let source = app.session.project.tracks[0].clips[0].clone();
        app.session.project.tracks[0].clips[0].repeat = Some(daw_core::ClipLoop {
            length_frames: 48_000,
            phase_frame: 0,
        });
        let second = Clip {
            id: Id::new_v4(),
            start_frame: 96_000,
            length_frames: 192_000,
            stretch: daw_core::ClipStretch::new(1, 2),
            ..source
        };
        app.session
            .project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: second.clone(),
            })
            .unwrap();
        let ctx = context();
        app.open_clip_editor(first, track, &ctx);
        frame(&mut app, &ctx, vec![]);
        let a = app.midi_editor.track_note_bounds[&(first, note)][0];
        let b = app.midi_editor.track_note_bounds[&(second.id, note)][0];
        assert_eq!(app.midi_editor.track_note_bounds[&(first, note)].len(), 2);
        select_note_box(&mut app, &ctx, a.union(b));
        let selected = app.midi_editor.selected_notes().clone();
        let before = format!("{:?}", app.session.project);
        let assets = app.session.project.assets.len();
        let start = note_edge(b, left);
        let end = start + Vec2::new(30.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(app.session.project.assets.len(), assets + 2);
        assert_eq!(app.midi_editor.selected_notes(), &selected);
        for (clip, delta) in [(first, 240), (second.id, 120)] {
            let content = pattern_for_clip(&app, clip);
            assert_eq!(content.notes.len(), 1);
            let n = &content.notes[0];
            assert_eq!(n.id, note);
            assert_eq!(n.start_tick, if left { 960 + delta } else { 960 });
            assert_eq!(n.length_ticks, if left { 960 - delta } else { 960 + delta });
        }
        app.session.project.validate().unwrap();
    }
}

#[test]
fn group_note_resize_with_shift_and_copy_modifier_cancels_and_never_duplicates() {
    for left in [false, true] {
        for lose_focus in [false, true] {
            let (mut app, track, clip, first, second) = note_resize_fixture();
            let ctx = context();
            app.open_clip_editor(clip, track, &ctx);
            frame(&mut app, &ctx, vec![]);
            let bounds =
                app.midi_editor.note_bounds[&first].union(app.midi_editor.note_bounds[&second]);
            select_note_box(&mut app, &ctx, bounds);
            let selected = app.midi_editor.selected_notes().clone();
            let original = pattern(&app).clone();
            let before = format!("{:?}", app.session.project);
            let start = note_edge(app.midi_editor.note_bounds[&first], left);
            let end = start + Vec2::new(13.0, 0.0);
            copy_note_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
                true,
                true,
            );
            copy_note_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end)],
                true,
                true,
            );
            assert_eq!(format!("{:?}", app.session.project), before);
            if lose_focus {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        events: vec![egui::Event::WindowFocused(false)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            } else {
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
            }
            copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, true);
            assert_eq!(format!("{:?}", app.session.project), before);
            assert_eq!(app.midi_editor.selected_notes(), &selected);
            assert!(!app.midi_editor.dragging());
            frame(&mut app, &ctx, vec![]);
            let start = note_edge(app.midi_editor.note_bounds[&first], left);
            let end = start + Vec2::new(13.0, 0.0);
            copy_note_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
                true,
                true,
            );
            copy_note_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end)],
                true,
                true,
            );
            copy_note_frame(&mut app, &ctx, vec![button(end, false)], true, true);
            assert_eq!(pattern(&app).notes.len(), 2);
            assert_eq!(app.midi_editor.selected_notes(), &selected);
            for old in &original.notes {
                let next = pattern(&app).notes.iter().find(|n| n.id == old.id).unwrap();
                assert_eq!(
                    next.length_ticks,
                    if left {
                        old.length_ticks - 104
                    } else {
                        old.length_ticks + 104
                    }
                );
                assert_eq!(
                    next.start_tick,
                    if left {
                        old.start_tick + 104
                    } else {
                        old.start_tick
                    }
                );
                assert_eq!((next.pitch, next.velocity), (old.pitch, old.velocity));
            }
        }
    }
}
