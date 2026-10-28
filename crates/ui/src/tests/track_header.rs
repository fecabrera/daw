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
        Pos2::new(strip.right() - 10.0, strip.top() + 12.0)
    } else {
        Pos2::new(
            app.timeline_bounds.left() - TRACK_WIDTH + 110.0,
            app.lane_bounds[&track].top() + 18.0,
        )
    }
}

#[test]
fn track_headers_keep_compact_knobs_and_height_without_numeric_fields() {
    for color_theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            let (mut app, audio) = fixture();
            let midi = app.session.project.add_midi_track().unwrap();
            let before = app.session.project.clone();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            ctx.set_pixels_per_point(scale);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let headers = [
                (
                    Rect::from_min_size(
                        Pos2::new(
                            app.lane_bounds[&audio].left() - TRACK_WIDTH,
                            app.lane_bounds[&audio].top(),
                        ),
                        Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
                    ),
                    true,
                ),
                (
                    Rect::from_min_size(
                        Pos2::new(
                            app.lane_bounds[&midi].left() - TRACK_WIDTH,
                            app.lane_bounds[&midi].top(),
                        ),
                        Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
                    ),
                    true,
                ),
                (app.master_bounds, false),
            ];
            assert_eq!(app.lane_bounds[&audio].height(), 68.0);
            assert_eq!(app.lane_bounds[&midi].height(), 68.0);
            for (header, has_pan) in headers {
                assert_eq!(header.height(), 68.0);
                for x in if has_pan {
                    vec![21.0, 53.0]
                } else {
                    vec![21.0]
                } {
                    let center = header.min + Vec2::new(x, 47.0);
                    let radius = (knobs::SIZE / 2.0 - 5.0) * knobs::HEADER_SIZE / knobs::SIZE;
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Circle(c) if c.center.distance(center) < 1.0 && (c.radius - radius).abs() < 0.01)),
                        "small knob at {center:?}: {:?}", shapes.iter().filter_map(|s| match &s.shape {
                            egui::Shape::Circle(c) if header.contains(c.center) => Some((c.center, c.radius)), _ => None
                        }).collect::<Vec<_>>());
                    assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Circle(c) if c.center.distance(center) < 1.0 && (c.radius - 9.0).abs() < 0.01)));
                }
                assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(r) if r.fill == theme::palette(&ctx).input
                        && (r.rect.width() - NUMERIC_WIDTH).abs() < 1.0 && header.contains_rect(r.rect))),
                    "header numeric fields must be absent");
            }
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn editor_track_headers_have_only_compact_knobs_and_keep_their_height() {
    for color_theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            for midi in [false, true] {
                let (mut app, audio) = fixture();
                let midi_track = app.session.project.add_midi_track().unwrap();
                let track = if midi { midi_track } else { audio };
                app.selected_clip = None;
                app.selected_clips.clear();
                app.select_track(track, false);
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                ctx.set_pixels_per_point(scale);
                app.set_view_mode(ViewMode::Edit, &ctx);
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let header = app.midi_editor.track_header_bounds;
                let [gain, pan] = app.midi_editor.track_mix_bounds;
                assert_eq!(header.height(), 68.0);
                let selector = app.midi_editor.selector_bounds;
                let meter_left =
                    header.right() - 7.0 - (meters::STEREO_WIDTH - meters::SCALE_WIDTH);
                assert!((meter_left - selector.right() - 6.0).abs() < 1.0);
                assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(r) if r.fill == theme::palette(&ctx).input
                        && (r.rect.width() - NUMERIC_WIDTH).abs() < 1.0 && header.contains_rect(r.rect))));
                assert_eq!(gain.size(), Vec2::splat(knobs::HEADER_SIZE));
                assert_eq!(pan.size(), gain.size());
                assert!(gain.center().distance(header.min + Vec2::new(21.0, 47.0)) < 1.0);
                assert!(pan.center().distance(header.min + Vec2::new(53.0, 47.0)) < 1.0);
                assert!((pan.left() - gain.right() - 10.0).abs() < 1.0);
                assert!(!app.dirty);
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(gain.center()),
                        button(gain.center(), true),
                    ],
                );
                frame(&mut app, &ctx, vec![button(gain.center(), false)]);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::ArrowUp,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                );
                assert!(
                    app.session
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.id == track)
                        .unwrap()
                        .gain_db
                        > 0.0
                );
                assert!(app.dirty);
                assert_eq!(app.midi_editor.track_header_bounds, header);
            }
        }
    }
}

#[test]
fn header_mute_solo_align_with_knobs_and_layers_align_to_the_right() {
    for scope in ["timeline", "layer", "editor"] {
        for scale in [1.0, 1.5] {
            let (mut app, audio) = fixture();
            let midi = app.session.project.add_midi_track().unwrap();
            let ctx = context();
            ctx.set_pixels_per_point(scale);
            let target = if scope == "layer" { midi } else { audio };
            app.selected_clip = None;
            app.selected_clips.clear();
            app.select_track(target, false);
            app.set_view_mode(
                match scope {
                    "layer" => ViewMode::Instrument,
                    "editor" => ViewMode::Edit,
                    _ => ViewMode::Timeline,
                },
                &ctx,
            );
            frame(&mut app, &ctx, vec![]);
            let layer = app.session.project.tracks[1]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .id;
            let panel = match scope {
                "layer" => app.midi_editor.layer_bounds[&layer].label,
                "editor" => app.midi_editor.track_header_bounds,
                _ => Rect::from_min_size(
                    Pos2::new(
                        app.lane_bounds[&target].left() - TRACK_WIDTH,
                        app.lane_bounds[&target].top(),
                    ),
                    Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
                ),
            };
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            for (index, label) in ["M", "S"].into_iter().enumerate() {
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(t)
                            if t.galley.text() == label
                                && panel
                                    .contains_rect(t.galley.rect.translate(t.pos.to_vec2())) =>
                        {
                            Some(t)
                        }
                        _ => None,
                    })
                    .expect("mute/solo label in header");
                let point = text.galley.rect.translate(text.pos.to_vec2()).center();
                let x = if scope == "layer" {
                    app.midi_editor.layer_bounds[&layer].collapse.right() - 39.0
                        + index as f32 * 28.0
                } else {
                    panel.left() + 178.0 + index as f32 * 28.0
                };
                assert!(
                    point.distance(Pos2::new(x, panel.top() + 47.0)) < 1.0,
                    "{scope} {label}: {point:?}, panel {panel:?}"
                );
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                frame(&mut app, &ctx, vec![button(point, false)]);
                let (muted, soloed) = if scope == "layer" {
                    let layer = &app.session.project.tracks[1]
                        .instrument
                        .as_ref()
                        .unwrap()
                        .layers[0];
                    (layer.muted, layer.soloed)
                } else {
                    let track = &app.session.project.tracks[0];
                    (track.muted, track.soloed)
                };
                assert!(if index == 0 { muted } else { soloed });
                assert!(!app.instrument_ui.dragging());
            }
            if scope == "layer" {
                let bounds = &app.midi_editor.layer_bounds[&layer];
                assert_eq!(
                    bounds.collapse.center().y,
                    bounds.mix_controls[0].center().y
                );
            }
            assert!(app.error.is_none());
        }
    }
}

#[test]
fn removed_header_fields_do_not_take_text_focus_and_knob_values_reach_the_mixer() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let origin = Pos2::new(
        app.lane_bounds[&track].left() - TRACK_WIDTH,
        app.lane_bounds[&track].top(),
    );
    let former_input = origin + Vec2::new(129.0, 47.0);
    let before = app.session.project.clone();
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(former_input),
            button(former_input, true),
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![
            button(former_input, false),
            egui::Event::Text("-6.0".into()),
        ],
    );
    assert!(!ctx.text_edit_focused());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    let gain = origin + Vec2::new(21.0, 47.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(gain), button(gain, true)],
    );
    frame(&mut app, &ctx, vec![button(gain, false)]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    let expected = app.session.project.tracks[0].gain_db;
    assert!(expected > 0.0);
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.inputs[&track].gain, faders::format_gain(expected));
    assert_eq!(app.session.project.tracks[0].gain_db, expected);
    assert!(app.error.is_none());
}

#[test]
fn smaller_track_and_master_knobs_keep_keyboard_editing_and_double_click_reset() {
    for control in ["gain", "pan", "master"] {
        let (mut app, audio) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let origin = if control == "master" {
            app.master_bounds.min
        } else {
            Pos2::new(
                app.lane_bounds[&audio].left() - TRACK_WIDTH,
                app.lane_bounds[&audio].top(),
            )
        };
        let point = origin + Vec2::new(if control == "pan" { 53.0 } else { 21.0 }, 47.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let value = |app: &DawUi| match control {
            "gain" => app.session.project.tracks[0].gain_db,
            "pan" => app.session.project.tracks[0].pan,
            _ => app.session.project.master.gain_db,
        };
        assert!(value(&app) > 0.0, "{control} keyboard edit");
        assert!(app.dirty);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        double_click(&mut app, &ctx, point);
        assert_eq!(value(&app), 0.0);
        assert_eq!(app.lane_bounds[&audio].height(), 68.0);
        assert_eq!(app.master_bounds.height(), 68.0);
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
    let left = app.timeline_bounds.left() - TRACK_WIDTH;
    double_click(&mut app, &ctx, Pos2::new(left + 110.0, y)); // Blank name area.
    assert_eq!(app.selected_clips, HashSet::from([ids[0], ids[1]]));
    app.select_clip(ids[2], tracks[1], false);
    double_click(&mut app, &ctx, Pos2::new(left + 178.0, y + 29.0)); // Mute button.
    assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
    assert!(!app.session.project.tracks[0].muted);
    double_click(&mut app, &ctx, Pos2::new(left + 50.0, y)); // Visible name text.
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

#[test]
fn tracks_and_layers_share_the_control_panel_surface_header_and_final_border() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for height in [400.0, 1300.0] {
            let mut app = DawUi::default();
            let midi = app.session.project.add_midi_track().unwrap();
            let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
            for index in 2..=4 {
                instrument.layers.push(daw_core::InstrumentLayerSettings {
                    name: format!("Layer {index}"),
                    ..Default::default()
                });
            }
            for _ in 0..8 {
                app.session.project.add_track().unwrap();
            }
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_track(midi, false);
            app.selected_tracks.insert(app.session.project.tracks[1].id);
            for view in [ViewMode::Timeline, ViewMode::Instrument] {
                app.set_view_mode(view, &ctx);
                let size = Vec2::new(1280.0, height);
                frame_shapes(&mut app, &ctx, vec![], size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let palette = theme::palette(&ctx);
                let column = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.fill == palette.panel
                                && rect.rect.width() == TRACK_WIDTH
                                && shape.clip_rect.contains_rect(rect.rect) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .max_by(|a, b| a.height().total_cmp(&b.height()))
                    .unwrap();
                let x = column.right() - 0.5;
                let borders = shapes.iter().enumerate().filter(|(_, shape)| matches!(
                    &shape.shape, egui::Shape::LineSegment { points, stroke }
                        if *points == [Pos2::new(x, column.top()), Pos2::new(x, column.bottom())]
                        && *stroke == Stroke::new(1.0, palette.border)
                )).collect::<Vec<_>>();
                assert_eq!(borders.len(), 1, "one continuous outer border in {view:?}");
                let border_index = borders[0].0;
                assert!(
                    shapes
                        .iter()
                        .enumerate()
                        .filter(|(_, shape)| matches!(
                            &shape.shape, egui::Shape::Rect(rect)
                                if rect.rect.left() <= x && rect.rect.right() >= x
                                    && rect.rect.top() < column.bottom()
                                    && rect.rect.bottom() > column.top()
                        ))
                        .all(|(index, shape)| {
                            // Selection is flush and paints last; neutral rows keep
                            // the common column border above their fills.
                            let selection = matches!(&shape.shape,
                                egui::Shape::Rect(rect) if rect.fill == egui::Color32::TRANSPARENT && rect.stroke == theme::selection_outline(&ctx));
                            if selection { index > border_index } else { index < border_index }
                        }),
                    "only flush selection can paint above the shared column border"
                );
                let title = if view == ViewMode::Timeline {
                    "Tracks"
                } else {
                    "Layers"
                };
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == title
                        && (text.pos.x - column.left() - f32::from(theme::TOOLBAR_PADDING)).abs() < 0.1)));
                if view == ViewMode::Timeline {
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.stroke == theme::selection_outline(&ctx)
                            && rect.rect.right() == column.right())));
                } else {
                    assert_eq!(
                        app.midi_editor.instrument_header_bounds.width(),
                        column.width()
                    );
                    assert!(
                        app.midi_editor
                            .instrument_header_bounds
                            .contains_rect(app.midi_editor.add_layer_bounds)
                    );
                }
            }
            assert!(!app.dirty);
        }
    }
}

#[test]
fn timeline_row_dividers_share_track_panel_bounds_at_different_scales_and_scroll_offsets() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for scale in [1.0, 1.25, 2.0] {
            let mut app = DawUi::default();
            let tracks = (0..12)
                .map(|_| app.session.project.add_track().unwrap())
                .collect::<Vec<_>>();
            app.selected_track = Some(tracks[0]);
            app.selected_tracks = tracks.iter().copied().collect();
            let ctx = context();
            ctx.set_pixels_per_point(scale);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            let original = format!("{:?}", app.session.project);
            let size = Vec2::new(900.0, 800.0);
            frame_shapes(&mut app, &ctx, vec![], size);
            let first = app.lane_bounds[&tracks[0]];
            for scroll in [false, true] {
                let events = if scroll {
                    vec![
                        egui::Event::PointerMoved(first.min + Vec2::splat(20.0)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -114.25),
                            modifiers: Default::default(),
                            phase: egui::TouchPhase::Move,
                        },
                    ]
                } else {
                    vec![]
                };
                frame_shapes(&mut app, &ctx, events, size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                if scroll {
                    assert!(app.lane_bounds[&tracks[0]].top() < first.top());
                }
                let mut visible_rows = 0;
                for (index, track) in tracks.iter().enumerate() {
                    let lane = app.lane_bounds[track];
                    if lane.top() < app.timeline_bounds.top()
                        || lane.bottom() > app.master_bounds.top()
                    {
                        continue;
                    }
                    visible_rows += 1;
                    let panel = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.stroke == theme::selection_outline(&ctx)
                                    && rect.rect.right() == lane.left()
                                    && rect.rect.center().y == lane.center().y =>
                            {
                                Some(rect.rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(panel.y_range(), lane.y_range());
                    if index > 0 {
                        let y = panel.top() + 0.5;
                        let lines = shapes.iter().filter(|shape| matches!(
                            &shape.shape, egui::Shape::LineSegment { points, stroke }
                                if *points == [Pos2::new(panel.left(), y), Pos2::new(lane.right(), y)]
                                    && *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                        )).count();
                        assert_eq!(lines, 1, "one aligned separator across panel and lane");
                    }
                    assert!(
                        !shapes.iter().any(|shape| matches!(
                            &shape.shape, egui::Shape::LineSegment { points, stroke }
                                if *points == [lane.left_bottom(), lane.right_bottom()]
                                    && *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                        )),
                        "no separately positioned lane border"
                    );
                }
                assert!(visible_rows > 1);
            }
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn shortened_clip_previews_restore_lane_dividers_at_the_shared_row_coordinates() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for scale in [1.0, 1.25, 2.0] {
            for midi in [false, true] {
                for (index, count) in [(0, 2), (1, 3), (1, 2)] {
                    for left in [false, true] {
                        let (mut app, track) = fixture();
                        let clip = &mut app.session.project.tracks[0].clips[0];
                        clip.start_frame = frames(2.0);
                        clip.source_offset_frame = frames(1.0);
                        clip.length_frames = frames(2.0);
                        if midi {
                            app.session.project.tracks[0].clips.clear();
                            app.session.project.tracks[0].kind = daw_core::TrackKind::Midi;
                            app.session.project.tracks[0].instrument = Some(Default::default());
                            let clip_id = app
                                .session
                                .project
                                .add_midi_pattern(track, frames(2.0))
                                .unwrap();
                            app.session
                                .project
                                .edit(Edit::ReplaceMidiPattern {
                                    clip_id,
                                    pattern: daw_core::MidiPattern {
                                        length_ticks: 3840,
                                        notes: vec![],
                                    },
                                })
                                .unwrap();
                            app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
                        }
                        for _ in 1..count {
                            app.session.project.add_track().unwrap();
                        }
                        app.session.project.tracks.swap(0, index);
                        app.session.project.validate().unwrap();
                        let clip = app.session.project.tracks[index].clips[0].clone();
                        let original = format!("{:?}", app.session.project);
                        let ctx = context();
                        ctx.set_pixels_per_point(scale);
                        app.set_settings(
                            &ctx,
                            settings::AppSettings {
                                theme: color_theme,
                                ..Default::default()
                            },
                        );
                        frame(&mut app, &ctx, vec![]);
                        let lane = app.lane_bounds[&track];
                        let block = app.clip_block(lane, &clip, clip.start_frame);
                        let start = Pos2::new(
                            if left {
                                block.left() + 0.2
                            } else {
                                block.right() - 0.2
                            },
                            block.top() + 40.0,
                        );
                        let end = start + Vec2::new(if left { 0.5 } else { -0.5 } * app.zoom, 0.0);
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(start), button(start, true)],
                        );
                        let shapes = frame_shapes(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end)],
                            Vec2::new(1280.0, 800.0),
                        );
                        let drag = app.drag.as_ref().unwrap();
                        assert_eq!(
                            drag.mode,
                            if left {
                                ClipDragMode::TrimLeft
                            } else {
                                ClipDragMode::TrimRight
                            }
                        );
                        let previews = app.clip_drag_previews(drag, end, false);
                        let preview = previews.iter().find(|p| p.clip.id == clip.id).unwrap();
                        assert!(preview.clip.length_frames < clip.length_frames);
                        let next = app.clip_block(lane, &preview.clip, preview.clip.start_frame);
                        let gap_x = if left {
                            (block.left() + next.left()) * 0.5
                        } else {
                            (block.right() + next.right()) * 0.5
                        };
                        let palette = theme::palette(&ctx);
                        let mask = shapes.iter().rposition(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect == block && r.fill == palette.background)).expect("original clip erased by preview mask");
                        for y in [
                            (index > 0).then_some(lane.top() + 0.5),
                            (index + 1 == count).then_some(lane.bottom() - 0.5),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            let point = Pos2::new(gap_x, y);
                            assert!(shapes[mask+1..].iter().any(|s| s.clip_rect.contains(point) && matches!(&s.shape,egui::Shape::LineSegment {points,stroke} if points[0].y==y && points[1].y==y && points[0].x<=gap_x && points[1].x>=gap_x && *stroke==Stroke::new(1.0,palette.border))), "separator erased during trim: midi={midi}, left={left}, row={index}/{count}, scale={scale}, y={y}");
                        }
                        assert_eq!(format!("{:?}", app.session.project), original);
                        assert!(!app.dirty);
                    }
                }
            }
        }
    }
}

#[test]
fn timeline_track_and_master_selection_strokes_remain_flush_above_column_dividers() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for master in [false, true] {
            let mut app = DawUi::default();
            let track = app.session.project.add_track().unwrap();
            app.session.project.add_track().unwrap();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_track(track, false);
            app.master_selected = master;
            app.timeline_effects_visible = false;
            app.set_view_mode(ViewMode::Timeline, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let rect = if master {
                app.master_bounds
            } else {
                let lane = app.lane_bounds[&track];
                Rect::from_min_max(
                    Pos2::new(lane.left() - TRACK_WIDTH, lane.top()),
                    Pos2::new(lane.left(), lane.bottom()),
                )
            };
            let index = shapes.iter().position(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rect && r.stroke==theme::selection_outline(&ctx))).expect("flush selection outline");
            let egui::Shape::Rect(outline) = &shapes[index].shape else {
                unreachable!()
            };
            assert_eq!(outline.stroke.width, 2.0);
            assert_eq!(outline.stroke_kind, egui::StrokeKind::Inside);
            assert!(shapes[index].clip_rect.contains_rect(rect));
            assert!(!shapes[index + 1..].iter().any(|s| {
                match &s.shape {
                    egui::Shape::LineSegment { points, stroke } => {
                        *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                            && points[0].x == rect.right() - 0.5
                            && points[1].x == rect.right() - 0.5
                            && points[0].y < rect.center().y
                            && points[1].y > rect.center().y
                    }
                    egui::Shape::Rect(r) => {
                        r.fill != egui::Color32::TRANSPARENT
                            && r.rect
                                .intersect(s.clip_rect)
                                .contains(rect.right_center() - Vec2::new(1.0, 0.0))
                    }
                    _ => false,
                }
            }));
            assert!(!app.dirty);
        }
    }
}
