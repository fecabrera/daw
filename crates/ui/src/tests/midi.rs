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
    app.session.project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
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
#[test]
fn editor_and_timeline_share_clip_painting_and_group_selection() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for override_color in [
            None,
            Some(daw_core::RgbColor {
                r: 40,
                g: 180,
                b: 220,
            }),
        ] {
            let (mut app, track, clip_id, _) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.color = override_color;
            clip.repeat = Some(daw_core::ClipLoop {
                length_frames: 48_000,
                phase_frame: 0,
            });
            let clip = clip.clone();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            // Include selection membership without a primary clip, as with a group toggle.
            for selection in 0..3 {
                let mut appearances = Vec::new();
                for view in [ViewMode::Timeline, ViewMode::Edit] {
                    app.set_view_mode(view, &ctx);
                    app.selected_track = Some(track);
                    app.selected_clip = (selection == 1).then_some(clip_id);
                    app.selected_clips.clear();
                    if selection == 2 {
                        app.selected_clips.insert(clip_id);
                    }
                    app.zoom = 240.0;
                    app.midi_editor.zoom = 240.0;
                    frame(&mut app, &ctx, vec![]);
                    app.scroll = 0.25;
                    app.midi_editor.scroll = 0.25;
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    let lane = app.lane_bounds[&track];
                    let block = if view.has_track_overview() {
                        app.midi_editor.clip_bounds[&clip_id]
                    } else {
                        app.clip_block(lane, &clip, clip.start_frame)
                    };
                    assert!(block.left() < lane.left(), "exercise viewport clipping");
                    let colors =
                        theme::clip_colors(app.session.project.tracks[0].color, override_color);
                    let shell = shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.rect == block => Some(rect),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let body = shell.iter().find(|rect| rect.fill == colors.body).unwrap();
                    let outline = shell.iter().find(|rect| rect.stroke.width > 0.0).unwrap();
                    let expected_stroke = if selection == 0 {
                        Stroke::new(1.0, theme::palette(&ctx).divider)
                    } else {
                        theme::selection_outline(&ctx)
                    };
                    assert_eq!(outline.stroke, expected_stroke);
                    assert_eq!(outline.corner_radius, egui::CornerRadius::same(2));
                    let header = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.fill == colors.header && rect.rect.min == block.min =>
                            {
                                Some(rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(header.rect.height(), 22.0);
                    let title = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text)
                                if text.galley.text() == clip.name
                                    && text.pos.y >= block.top()
                                    && text.pos.y < header.rect.bottom() =>
                            {
                                Some(text)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(title.pos.x - block.left(), 7.0);
                    assert_eq!(title.fallback_color, colors.text);
                    let notes = shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.fill == colors.waveform
                                    && rect.rect.top() >= header.rect.bottom()
                                    && rect.rect.bottom() <= block.bottom() =>
                            {
                                Some((
                                    (rect.rect.left() - block.left()).round() as i32,
                                    rect.rect.width().round() as i32,
                                    ((block.bottom() - rect.rect.top()) / (block.height() - 22.0)
                                        * 1000.0)
                                        .round() as i32,
                                    rect.rect.height().round() as i32,
                                ))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(notes.len(), 2, "both repeated note previews remain visible");
                    let repeats = shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::LineSegment { points, stroke }
                                if stroke.color == colors.text.gamma_multiply(0.25)
                                    && points[0].y == block.top()
                                    && points[1].y == block.bottom() =>
                            {
                                Some((points[0].x - block.left()).round() as i32)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(repeats, vec![240]);
                    appearances.push((
                        (
                            header.rect.height(),
                            body.rect.height() - header.rect.height(),
                        ),
                        body.fill,
                        body.corner_radius,
                        header.fill,
                        header.corner_radius,
                        outline.stroke,
                        title.fallback_color,
                        title.galley.job.sections[0].format.font_id.clone(),
                        title.pos.y - block.top(),
                        notes,
                        repeats,
                    ));
                }
                assert!(appearances.windows(2).all(|pair| pair[0] == pair[1]));
                assert!(!app.dirty);
            }
        }
    }
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

pub(super) fn layer_header_fixture()
-> (DawUi, egui::Context, Vec<daw_core::InstrumentLayerSettings>) {
    let (mut app, _, _, _) = fixture();
    // Leave room for direct gestures on multiple rows above the pinned Main bus.
    app.timeline_effects_visible = false;
    let settings = app.session.project.tracks[0].instrument.as_mut().unwrap();
    settings.layers[0].name = "Lead".into();
    let mut second = daw_core::InstrumentLayerSettings {
        name: "Bass".into(),
        ..Default::default()
    };
    second.oscillators.push(daw_core::OscillatorSettings {
        transpose: -12,
        ..Default::default()
    });
    settings.layers.push(second);
    settings.layers.push(daw_core::InstrumentLayerSettings {
        name: "Layer 3".into(),
        ..Default::default()
    });
    let layers = settings.layers.clone();
    let ctx = context();
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, layers)
}
fn begin_layer_rename(app: &mut DawUi, ctx: &egui::Context, id: Id) {
    // Start a separate double-click gesture after the previous edit's clicks.
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|input| input.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    let point = app.midi_editor.layer_bounds[&id].name.center();
    for _ in 0..2 {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(app, ctx, vec![button(point, false)]);
    }
    frame(app, ctx, vec![]);
    assert!(
        app.instrument_ui.editing_name(),
        "name at {point:?}, model={}, focus={:?}",
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .name,
        ctx.memory(|m| m.focused())
    );
}
fn layer_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}
#[test]
fn layer_headers_share_track_geometry_and_leave_the_header_workspace_dark() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, mut layers) = layer_header_fixture();
        let envelope_only = daw_core::InstrumentLayerSettings {
            name: "Envelope only".into(),
            envelopes: vec![daw_core::EnvelopeInstance::default()],
            ..Default::default()
        };
        layers.push(envelope_only);
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers = layers.clone();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(640.0, 1600.0));
        let header = app.midi_editor.instrument_header_bounds;
        assert_eq!(header.width(), TRACK_WIDTH);
        let add = app.midi_editor.add_layer_bounds;
        assert!(header.contains_rect(add));
        assert!((header.right() - add.right() - f32::from(theme::TOOLBAR_PADDING)).abs() < 0.1);
        let blank = Pos2::new(header.right() + 40.0, header.center().y);
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect.contains(blank) && r.fill == theme::palette(&ctx).panel)));
        let below = Pos2::new(
            header.center().x,
            app.midi_editor.layer_bounds[&layers.last().unwrap().id]
                .row
                .bottom()
                + 10.0,
        );
        assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect.contains(below) && r.fill == theme::palette(&ctx).panel)));
        for layer in &layers {
            let bounds = &app.midi_editor.layer_bounds[&layer.id];
            let full_height = app.midi_editor.layer_bounds[&layers[0].id].row.height();
            assert_eq!(bounds.row.height(), full_height, "{}", layer.name);
            assert_eq!(bounds.label.height(), full_height, "{}", layer.name);
            // Missing units do not reserve a placeholder column before installed units.
            let first_unit = if bounds.oscillator_panel != Rect::NOTHING {
                bounds.oscillator_panel
            } else {
                bounds.envelope_panel
            };
            if first_unit != Rect::NOTHING {
                assert!(
                    (first_unit.left() - bounds.label.right() + 1.0).abs() < 0.01,
                    "{}: unit {} label {}",
                    layer.name,
                    first_unit.left(),
                    bounds.label.right()
                );
            }
            assert_eq!(bounds.label.width(), TRACK_WIDTH);
            assert_eq!(bounds.label.right(), header.right());
            assert_eq!(
                bounds.icon.size(),
                Vec2::new(icons::HEADER_ICON_WIDTH, toolbars::CONTROL_HEIGHT)
            );
            assert_eq!(bounds.icon.left(), bounds.label.left() + 7.0);
            assert!((bounds.name.left() - bounds.icon.right() - 6.0).abs() < 0.1);
            let text = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == layer.name => Some(t),
                    _ => None,
                })
                .unwrap();
            assert!(
                text.galley
                    .job
                    .sections
                    .iter()
                    .all(|s| s.format.font_id.size == rows::NAME_FONT_SIZE)
            );
        }
    }
}
pub(super) fn double_click_unit_header(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|input| input.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    for _ in 0..2 {
        click_note_or_lane(app, ctx, point);
    }
    frame(app, ctx, vec![]);
}

#[test]
fn unit_power_controls_toggle_on_release_and_remain_visible_when_collapsed() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Yellow,
        ] {
            for oscillator in [true, false] {
                let (mut app, ctx, layers) = layer_header_fixture();
                let id = layers[0].id;
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        accent,
                        ..Default::default()
                    },
                );
                frame(&mut app, &ctx, vec![]);
                for collapsed in [false, true] {
                    if collapsed {
                        let bounds = &app.midi_editor.layer_bounds[&id];
                        let header = if oscillator {
                            bounds.oscillator_header
                        } else {
                            bounds.envelope_header
                        };
                        double_click_unit_header(&mut app, &ctx, header.center());
                    }
                    app.dirty = false;
                    let before = app.session.project.tracks[0].instrument.clone().unwrap();
                    let bounds = &app.midi_editor.layer_bounds[&id];
                    let (header, power) = if oscillator {
                        (bounds.oscillator_header, bounds.oscillator_power)
                    } else {
                        (bounds.envelope_header, bounds.envelope_power)
                    };
                    assert!(header.contains_rect(power));
                    assert_eq!(
                        power.center().x,
                        header.left() + panels::HEADER_HEIGHT / 2.0
                    );
                    assert_eq!(power.center().y, header.top() + panels::HEADER_HEIGHT / 2.0);
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    let circle = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Circle(circle)
                                if circle.center == power.center()
                                    && (2.0 * circle.radius + circle.stroke.width
                                        - panels::UNIT_POWER_SIZE)
                                        .abs()
                                        < 0.01 =>
                            {
                                Some(circle)
                            }
                            _ => None,
                        })
                        .unwrap();
                    let on = if oscillator {
                        before.layers[0].oscillator_enabled
                    } else {
                        before.layers[0].envelopes[0].enabled
                    };
                    let fill = theme::unit_header_fill(&ctx, on);
                    if on {
                        assert_eq!(fill, ctx.global_style().visuals.selection.bg_fill);
                        assert_ne!(fill, theme::accent(&ctx));
                        assert_ne!(fill, theme::palette(&ctx).panel);
                    } else {
                        assert_eq!(
                            fill,
                            panels::PanelStyle::default()
                                .frame(&ctx, false)
                                .stroke
                                .color
                        );
                    }
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == Rect::from_min_size(header.min, Vec2::new(header.width(), panels::HEADER_HEIGHT)) && rect.fill == fill)));
                    let color = if on {
                        theme::accent(&ctx)
                    } else {
                        theme::palette(&ctx).secondary
                    };
                    if collapsed {
                        let panel = if oscillator {
                            app.midi_editor.layer_bounds[&id].oscillator_panel
                        } else {
                            app.midi_editor.layer_bounds[&id].envelope_panel
                        };
                        assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(rect) if rect.rect == panel && rect.fill == theme::palette(&ctx).panel)));
                    }
                    let name = if oscillator {
                        "Oscillator"
                    } else {
                        "Envelope 1"
                    };
                    let title = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text)
                                if text.galley.text() == name
                                    && header.contains(text.visual_bounding_rect().center()) =>
                            {
                                Some(text)
                            }
                            _ => None,
                        })
                        .unwrap();
                    let expected_text = if !on {
                        theme::palette(&ctx).secondary
                    } else if collapsed {
                        theme::palette(&ctx).text
                    } else {
                        theme::unit_header_text(fill)
                    };
                    assert_eq!(title.fallback_color, expected_text);
                    assert_eq!(circle.stroke.color, color);
                    assert_eq!(
                        circle.fill,
                        if on {
                            color
                        } else {
                            egui::Color32::TRANSPARENT
                        }
                    );
                    let point = power.center();
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(point), button(point, true)],
                    );
                    assert_eq!(
                        app.session.project.tracks[0].instrument.as_ref().unwrap(),
                        &before
                    );
                    assert!(!app.dirty);
                    frame(&mut app, &ctx, vec![button(point, false)]);
                    let mut expected = before.clone();
                    if oscillator {
                        expected.layers[0].oscillator_enabled = !on;
                    } else {
                        expected.layers[0].envelopes[0].enabled = !on;
                    }
                    assert_eq!(
                        app.session.project.tracks[0].instrument.as_ref().unwrap(),
                        &expected
                    );
                    assert!(app.dirty);
                    let after = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    if on {
                        let cap = Rect::from_min_size(
                            header.min,
                            Vec2::new(header.width(), panels::HEADER_HEIGHT),
                        );
                        assert!(after.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(rect) if rect.rect == cap && rect.fill == theme::palette(&ctx).border)));
                        let title = after
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Text(text)
                                    if text.galley.text() == name
                                        && header
                                            .contains(text.visual_bounding_rect().center()) =>
                                {
                                    Some(text)
                                }
                                _ => None,
                            })
                            .unwrap();
                        assert_eq!(title.fallback_color, theme::palette(&ctx).secondary);
                    }
                    let bounds = &app.midi_editor.layer_bounds[&id];
                    let actual_header = if oscillator {
                        bounds.oscillator_header
                    } else {
                        bounds.envelope_header
                    };
                    assert_eq!(
                        actual_header, header,
                        "Power clicks must not collapse or expand"
                    );
                    // A second quick click remains a toggle, even when interpreted as a double-click.
                    click_note_or_lane(&mut app, &ctx, point);
                    frame(&mut app, &ctx, vec![]);
                    assert_eq!(
                        app.session.project.tracks[0].instrument.as_ref().unwrap(),
                        &before
                    );
                    let bounds = &app.midi_editor.layer_bounds[&id];
                    assert_eq!(
                        if oscillator {
                            bounds.oscillator_header
                        } else {
                            bounds.envelope_header
                        },
                        header
                    );
                    // Restore off for the collapsed presentation's hollow-circle check.
                    click_note_or_lane(&mut app, &ctx, point);
                    frame(&mut app, &ctx, vec![]);
                    let before = app.session.project.tracks[0].instrument.clone().unwrap();
                    app.dirty = false;
                    for enabled in [false, true] {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                focused: !enabled,
                                screen_rect: Some(Rect::from_min_size(
                                    Pos2::ZERO,
                                    Vec2::new(1280.0, 800.0),
                                )),
                                events: vec![
                                    egui::Event::PointerMoved(point),
                                    button(point, true),
                                    button(point, false),
                                ],
                                ..Default::default()
                            },
                            |ui| {
                                ui.add_enabled_ui(enabled, |ui| app.show(ui));
                            },
                        );
                        output.textures_delta.clear();
                        assert_eq!(
                            app.session.project.tracks[0].instrument.as_ref().unwrap(),
                            &before
                        );
                        assert!(!app.dirty);
                    }
                    frame(&mut app, &ctx, vec![]);
                }
            }
        }
    }
}

#[test]
fn unit_headers_save_collapse_independently_without_changing_audio_settings_or_layer_height() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Yellow,
        ] {
            let (mut app, ctx, layers) = layer_header_fixture();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    accent,
                    ..Default::default()
                },
            );
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let before = serde_json::to_value(&app.session.project).unwrap();
            let first = layers[0].id;
            let second = layers[1].id;
            let initial = &app.midi_editor.layer_bounds[&first];
            let full_height = initial.row.height();
            let full_width = initial.oscillator_panel.width();
            let point = initial.oscillator_header.center();
            click_note_or_lane(&mut app, &ctx, point);
            frame(&mut app, &ctx, vec![]);
            assert_eq!(
                app.midi_editor.layer_bounds[&first]
                    .oscillator_panel
                    .width(),
                full_width,
                "A single click must leave the unit expanded"
            );
            double_click_unit_header(&mut app, &ctx, point);
            let first_bounds = &app.midi_editor.layer_bounds[&first];
            assert_eq!(
                first_bounds.oscillator_panel.width(),
                panels::HEADER_HEIGHT + 2.0
            );
            assert_eq!(first_bounds.oscillator_panel.height(), full_height);
            assert_eq!(first_bounds.row.height(), full_height);
            assert!(first_bounds.oscillators.is_empty());
            assert!(
                first_bounds
                    .envelope_stages
                    .iter()
                    .all(|rect| *rect != Rect::NOTHING)
            );
            assert!(
                app.midi_editor.layer_bounds[&second]
                    .oscillator_panel
                    .width()
                    > panels::HEADER_HEIGHT + 2.0
            );
            let point = first_bounds.envelope_header.center();
            double_click_unit_header(&mut app, &ctx, point);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let bounds = &app.midi_editor.layer_bounds[&first];
            assert!(
                bounds
                    .envelope_stages
                    .iter()
                    .all(|rect| *rect == Rect::NOTHING)
            );
            assert_eq!(bounds.envelope_panel.width(), panels::HEADER_HEIGHT + 2.0);
            assert_eq!(bounds.envelope_panel.height(), full_height);
            assert_eq!(
                bounds.oscillator_panel.right() - 1.0,
                bounds.envelope_panel.left()
            );
            for (name, panel, header) in [
                (
                    "Oscillator",
                    bounds.oscillator_panel,
                    bounds.oscillator_header,
                ),
                ("Envelope 1", bounds.envelope_panel, bounds.envelope_header),
            ] {
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.rect == panel && rect.fill == theme::palette(&ctx).panel)));
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if text.galley.text() == name
                                && text.angle != 0.0
                                && header.contains(text.visual_bounding_rect().center()) =>
                        {
                            Some(text)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(text.angle, -std::f32::consts::FRAC_PI_2);
                assert_eq!(text.fallback_color, theme::palette(&ctx).text);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.font_id.size == panels::HEADER_FONT_SIZE)
                );
                let body = Rect::from_min_max(
                    Pos2::new(header.left(), header.top() + panels::HEADER_HEIGHT),
                    header.max,
                );
                assert!(body.contains_rect(text.visual_bounding_rect()));
                assert!(
                    ((text.pos
                        + egui::emath::Rot2::from_angle(text.angle)
                            * text.galley.rect.center().to_vec2())
                        - body.center())
                    .length()
                        < 0.01
                );
                let cap = Rect::from_min_size(header.min, Vec2::splat(panels::HEADER_HEIGHT));
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.rect == cap && rect.fill == theme::unit_header_fill(&ctx, true))));
            }
            let mut saved = before.clone();
            saved["tracks"][0]["instrument"]["layers"][0]["oscillator_collapsed"] = true.into();
            saved["tracks"][0]["instrument"]["layers"][0]["envelopes"][0]["collapsed"] =
                true.into();
            assert_eq!(serde_json::to_value(&app.session.project).unwrap(), saved);
            assert!(app.dirty);
            app.dirty = false;
            let disabled_point = bounds.oscillator_header.center();
            for _ in 0..2 {
                for pressed in [true, false] {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(1280.0, 800.0),
                            )),
                            events: vec![
                                egui::Event::PointerMoved(disabled_point),
                                button(disabled_point, pressed),
                            ],
                            ..Default::default()
                        },
                        |ui| {
                            ui.disable();
                            app.show(ui);
                        },
                    );
                    output.textures_delta.clear();
                }
            }
            frame(&mut app, &ctx, vec![]);
            assert_eq!(
                app.midi_editor.layer_bounds[&first]
                    .oscillator_panel
                    .width(),
                panels::HEADER_HEIGHT + 2.0,
                "Disabled headers must not expand"
            );
            app.set_view_mode(ViewMode::Timeline, &ctx);
            frame(&mut app, &ctx, vec![]);
            app.set_view_mode(ViewMode::Instrument, &ctx);
            frame(&mut app, &ctx, vec![]);
            assert_eq!(
                app.midi_editor.layer_bounds[&first]
                    .oscillator_panel
                    .width(),
                panels::HEADER_HEIGHT + 2.0
            );
            // Saved presentation follows the layer when its row changes.
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers
                .swap(0, 1);
            frame(&mut app, &ctx, vec![]);
            assert_eq!(
                app.midi_editor.layer_bounds[&first]
                    .oscillator_panel
                    .width(),
                panels::HEADER_HEIGHT + 2.0
            );
            assert!(
                app.midi_editor.layer_bounds[&second]
                    .oscillator_panel
                    .width()
                    > panels::HEADER_HEIGHT + 2.0
            );
            let point = app.midi_editor.layer_bounds[&first]
                .oscillator_header
                .center();
            double_click_unit_header(&mut app, &ctx, point);
            assert_eq!(
                app.midi_editor.layer_bounds[&first]
                    .oscillator_panel
                    .width(),
                full_width
            );
            assert!(!app.midi_editor.layer_bounds[&first].oscillators.is_empty());
            assert_eq!(
                app.midi_editor.layer_bounds[&first].envelope_panel.width(),
                panels::HEADER_HEIGHT + 2.0
            );
            let point = app.midi_editor.layer_bounds[&first]
                .envelope_header
                .center();
            double_click_unit_header(&mut app, &ctx, point);
            assert!(
                app.midi_editor.layer_bounds[&first]
                    .envelope_stages
                    .iter()
                    .all(|rect| *rect != Rect::NOTHING)
            );
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers
                .swap(0, 1);
            assert_eq!(serde_json::to_value(&app.session.project).unwrap(), before);
            assert!(app.dirty);
        }
    }
}

#[test]
fn saved_unit_collapse_restores_in_a_fresh_ui_and_project_replacement_uses_its_own_state() {
    let (mut app, ctx, layers) = layer_header_fixture();
    frame(&mut app, &ctx, vec![]);
    let expanded = app.session.project.clone();
    let track = expanded.tracks[0].id;
    let first = layers[0].id;
    let second = layers[1].id;
    let full_width = app.midi_editor.layer_bounds[&first]
        .oscillator_panel
        .width();
    let header = app.midi_editor.layer_bounds[&first].oscillator_header;
    double_click_unit_header(&mut app, &ctx, header.center());
    let header = app.midi_editor.layer_bounds[&first].envelope_header;
    double_click_unit_header(&mut app, &ctx, header.center());
    assert!(app.dirty);
    let saved = serde_json::to_vec(&app.session.project).unwrap();
    let mut reopened = DawUi::default();
    reopened.session.project = serde_json::from_slice(&saved).unwrap();
    reopened.selected_track = Some(track);
    let fresh_ctx = context();
    reopened.set_view_mode(ViewMode::Instrument, &fresh_ctx);
    frame(&mut reopened, &fresh_ctx, vec![]);
    let collapsed_width = panels::HEADER_HEIGHT + 2.0;
    let bounds = &reopened.midi_editor.layer_bounds[&first];
    assert_eq!(bounds.oscillator_panel.width(), collapsed_width);
    assert_eq!(bounds.envelope_panel.width(), collapsed_width);
    assert!(bounds.oscillators.is_empty());
    assert!(
        bounds
            .envelope_stages
            .iter()
            .all(|rect| *rect == Rect::NOTHING)
    );
    assert!(
        reopened.midi_editor.layer_bounds[&second]
            .oscillator_panel
            .width()
            > collapsed_width
    );
    assert!(
        !reopened.dirty,
        "loading saved presentation must not dirty the project"
    );
    // The same IDs in the same UI context must still obey the replacement project.
    reopened.session.project = expanded;
    frame(&mut reopened, &fresh_ctx, vec![]);
    let bounds = &reopened.midi_editor.layer_bounds[&first];
    assert_eq!(bounds.oscillator_panel.width(), full_width);
    assert!(bounds.envelope_panel.width() > collapsed_width);
    assert!(!bounds.oscillators.is_empty());
    assert!(!reopened.dirty);
}

#[test]
fn layer_mix_controls_share_track_layout_and_edit_only_their_layer() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layers) = layer_header_fixture();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, vec![]);
        let first = layers[0].id;
        let original_peer = layers[1].clone();
        let original_track = (
            app.session.project.tracks[0].gain_db,
            app.session.project.tracks[0].pan,
            app.session.project.tracks[0].muted,
            app.session.project.tracks[0].soloed,
        );
        let bounds = &app.midi_editor.layer_bounds[&first];
        let [mute, solo, gain, gain_input, pan, pan_input] = bounds.mix_controls;
        assert_eq!(mute.size(), Vec2::splat(toolbars::CONTROL_HEIGHT));
        assert_eq!(solo.size(), mute.size());
        assert_eq!(bounds.label.right() - bounds.collapse.right(), 7.0);
        assert_eq!(bounds.collapse.left() - solo.right(), 6.0);
        assert_eq!(bounds.collapse.top() - bounds.label.top(), 7.0);
        assert_eq!(solo.left() - mute.right(), 6.0);
        for peer in &layers[1..] {
            let controls = app.midi_editor.layer_bounds[&peer.id].mix_controls;
            assert_eq!(controls[0].x_range(), mute.x_range());
            assert_eq!(controls[1].x_range(), solo.x_range());
        }
        assert_eq!(gain.size(), Vec2::splat(knobs::SIZE));
        assert_eq!(pan.size(), gain.size());
        assert_eq!(gain.left(), bounds.label.left() + 7.0);
        assert_eq!(
            gain.top(),
            bounds.label.top() + 7.0 + toolbars::CONTROL_HEIGHT + 4.0
        );
        assert_eq!(gain_input.width(), NUMERIC_WIDTH);
        assert_eq!(pan_input.width(), NUMERIC_WIDTH);
        assert_eq!(pan.left() - gain_input.right(), 4.0);
        let point = mute.center();
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .muted
        );
        assert!(!app.dirty);
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .muted
        );
        assert!(app.dirty);
        click_note_or_lane(&mut app, &ctx, solo.center());
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .soloed
        );
        click_note_or_lane(&mut app, &ctx, gain.center());
        frame(&mut app, &ctx, vec![layer_key(egui::Key::ArrowUp)]);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .gain_db
                > 0.0
        );
        click_note_or_lane(&mut app, &ctx, pan.center());
        frame(&mut app, &ctx, vec![layer_key(egui::Key::ArrowRight)]);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .pan
                > 0.0
        );
        for (input, text, key, expected) in [
            (gain_input, "-6.0", egui::Key::Enter, -6.0),
            (gain_input, "-3.0", egui::Key::Escape, -6.0),
            (
                gain_input,
                "-inf",
                egui::Key::Enter,
                daw_core::SILENT_GAIN_DB,
            ),
            (pan_input, "-0.75", egui::Key::Enter, -0.75),
            (pan_input, "0.9", egui::Key::Escape, -0.75),
        ] {
            click_note_or_lane(&mut app, &ctx, input.center());
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
            );
            frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
            app.dirty = false;
            frame(&mut app, &ctx, vec![layer_key(key)]);
            let layer = &app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0];
            assert_eq!(
                if input == gain_input {
                    layer.gain_db
                } else {
                    layer.pan
                },
                expected
            );
            assert_eq!(app.dirty, key == egui::Key::Enter);
        }
        let track = &app.session.project.tracks[0];
        assert_eq!(track.instrument.as_ref().unwrap().layers[1], original_peer);
        assert_eq!(
            (track.gain_db, track.pan, track.muted, track.soloed),
            original_track
        );
        assert!(app.error.is_none());
        click_note_or_lane(&mut app, &ctx, pan_input.center());
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        frame(&mut app, &ctx, vec![egui::Event::Text("1.01".into())]);
        app.dirty = false;
        frame(&mut app, &ctx, vec![layer_key(egui::Key::Enter)]);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .pan,
            -0.75
        );
        assert!(!app.dirty);
        assert!(app.error.is_some());
    }
}

#[test]
fn layer_names_commit_on_enter_and_cancel_without_changing_instrument_settings() {
    let (mut app, ctx, layers) = layer_header_fixture();
    let id = layers[0].id;
    app.dirty = false;
    begin_layer_rename(&mut app, &ctx, id);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("  Lead 🎹  ".into())],
    );
    assert!(!app.dirty);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers,
        layers
    );
    frame(&mut app, &ctx, vec![layer_key(egui::Key::Enter)]);
    let mut expected = layers;
    expected[0].name = "Lead 🎹".into();
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers,
        expected
    );
    assert!(app.dirty);
    for cancel in [
        "escape",
        "blur",
        "view",
        "empty",
        "unchanged",
        "window",
        "track",
    ] {
        app.dirty = false;
        begin_layer_rename(&mut app, &ctx, id);
        if cancel != "unchanged" {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Text(
                    if cancel == "empty" { "  " } else { "Discard" }.into(),
                )],
            );
        }
        match cancel {
            "escape" => frame(&mut app, &ctx, vec![layer_key(egui::Key::Escape)]),
            "empty" | "unchanged" => frame(&mut app, &ctx, vec![layer_key(egui::Key::Enter)]),
            "view" => {
                app.set_view_mode(ViewMode::Timeline, &ctx);
                frame(&mut app, &ctx, vec![]);
                app.set_view_mode(ViewMode::Instrument, &ctx);
            }
            "track" => {
                let other = app.session.project.add_midi_track().unwrap();
                app.select_track(other, false);
                frame(&mut app, &ctx, vec![]);
                app.select_track(app.session.project.tracks[0].id, false);
            }
            "window" => {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
            _ => {
                let point = Pos2::new(1200.0, 750.0);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                frame(&mut app, &ctx, vec![button(point, false)]);
            }
        }
        assert!(!app.instrument_ui.editing_name(), "{cancel}");
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers,
            expected,
            "{cancel}"
        );
        assert!(!app.dirty, "{cancel}");
        frame(&mut app, &ctx, vec![]);
    }
}
#[test]
fn layer_headers_preview_and_commit_reorders_and_cancel_invalid_drops() {
    for area in ["icon", "name", "blank", "inset", "gutter", "body"] {
        for action in ["commit", "escape", "window", "outside", "same", "disabled"] {
            let (mut app, ctx, layers) = layer_header_fixture();
            app.timeline_effects_visible = false;
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let bounds = &app.midi_editor.layer_bounds[&layers[0].id];
            let start = match area {
                "icon" => bounds.icon.center(),
                "name" => bounds.name.center(),
                "blank" => Pos2::new(bounds.name.right() + 10.0, bounds.icon.center().y),
                "inset" => Pos2::new(bounds.label.left() + 2.0, bounds.icon.center().y),
                "gutter" => Pos2::new(bounds.label.right() - 2.0, bounds.label.bottom() - 10.0),
                _ => bounds.label.center(),
            };
            let end = if action == "same" {
                start + Vec2::new(8.0, 0.0)
            } else if action == "outside" {
                Pos2::new(start.x, 10.0)
            } else {
                Pos2::new(
                    start.x,
                    app.instrument_catalog_bounds.layer_viewport.bottom() - 2.0,
                )
            };
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            if action != "same" && action != "outside" {
                for _ in 0..45 {
                    frame(&mut app, &ctx, vec![]);
                }
            }
            assert!(app.instrument_ui.dragging());
            assert!(!app.file_action_enabled(FileAction::Import));
            assert!(!app.view_switch_enabled());
            assert_eq!(
                app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers,
                layers
            );
            assert!(!app.dirty);
            match action {
                "escape" => frame(&mut app, &ctx, vec![layer_key(egui::Key::Escape)]),
                "window" | "disabled" => {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            focused: action != "window",
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(1280.0, 800.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            ui.add_enabled_ui(action != "disabled", |ui| app.show(ui));
                        },
                    );
                    output.textures_delta.clear();
                }
                _ => (),
            }
            frame(&mut app, &ctx, vec![button(end, false)]);
            let expected = if action == "commit" {
                vec![layers[1].clone(), layers[2].clone(), layers[0].clone()]
            } else {
                layers
            };
            assert_eq!(
                app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers,
                expected,
                "{area}: {action}"
            );
            assert_eq!(app.dirty, action == "commit");
            assert!(!app.instrument_ui.dragging());
        }
    }
}

#[test]
fn layer_shift_selection_toggles_outlines_and_keeps_output_exclusive() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = layer_header_fixture();
        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
        for l in &mut i.layers {
            l.collapsed = true;
        }
        let ids: Vec<_> = i.layers.iter().map(|l| l.id).collect();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme,
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        for (index, shift) in [(2, false), (0, true)] {
            let p = app.midi_editor.layer_bounds[&ids[index]].icon.center();
            super::instrument_catalog::click(&mut app, &ctx, p, shift);
        }
        assert_eq!(
            app.instrument_ui.selected_layers,
            HashSet::from([ids[0], ids[2]])
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for id in [ids[0], ids[2]] {
            let label = app.midi_editor.layer_bounds[&id].label;
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == label && rect.stroke == theme::selection_outline(&ctx))));
        }
        // Repeated Shift-clicks on a name toggle membership without starting Rename.
        for count in [3, 2] {
            let p = app.midi_editor.layer_bounds[&ids[1]].name.center();
            super::instrument_catalog::click(&mut app, &ctx, p, true);
            assert_eq!(app.instrument_ui.selected_layers.len(), count);
            assert!(!app.instrument_ui.editing_name());
        }
        // Shift is also used by controls; it must not toggle layer selection there.
        let p = app.midi_editor.layer_bounds[&ids[0]].mix_controls[2].center();
        super::instrument_catalog::click(&mut app, &ctx, p, true);
        assert_eq!(app.instrument_ui.selected_layers.len(), 2);
        // Toggling the final member off remains empty on later frames.
        for index in [0, 2] {
            let p = app.midi_editor.layer_bounds[&ids[index]].icon.center();
            super::instrument_catalog::click(&mut app, &ctx, p, true);
        }
        assert!(app.instrument_ui.selected_layers.is_empty());
        assert_eq!(app.instrument_ui.selected_layer, None);
        let output = app.instrument_ui.main_bounds.icon.center();
        super::instrument_catalog::click(&mut app, &ctx, output, true);
        let bus = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .main
            .id;
        assert_eq!(app.instrument_ui.selected_layers, HashSet::from([bus]));
        let p = app.midi_editor.layer_bounds[&ids[1]].icon.center();
        super::instrument_catalog::click(&mut app, &ctx, p, true);
        assert_eq!(app.instrument_ui.selected_layers, HashSet::from([ids[1]]));
        assert!(!app.dirty);
        // A different track has an independent initial target.
        let other = app.session.project.add_midi_track().unwrap();
        app.select_track(other, false);
        frame(&mut app, &ctx, vec![]);
        assert!(
            ids.iter()
                .all(|id| !app.instrument_ui.selected_layers.contains(id))
        );
    }
}

#[test]
fn selected_layer_groups_move_copy_cancel_and_undo_in_saved_order() {
    for group in [[0, 1], [0, 2]] {
        for action in [
            "move",
            "copy",
            "same",
            "escape",
            "outside",
            "output",
            "unselected",
            "fast",
        ] {
            let (mut app, ctx, _) = layer_header_fixture();
            let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
            for l in &mut i.layers {
                l.collapsed = true;
                l.macro_module.add();
                l.effects.push(daw_core::EffectInstance::limiter());
            }
            let ids: Vec<_> = i.layers.iter().map(|l| l.id).collect();
            frame(&mut app, &ctx, vec![]);
            // Reverse click order must not determine processing or insertion order.
            for (index, shift) in [(group[1], false), (group[0], true)] {
                let p = app.midi_editor.layer_bounds[&ids[index]].icon.center();
                super::instrument_catalog::click(&mut app, &ctx, p, shift);
            }
            app.history = None;
            app.ensure_history();
            app.dirty = false;
            let original = app.session.project.clone();
            let anchor = if action == "unselected" {
                (0..3).find(|n| !group.contains(n)).unwrap()
            } else {
                group[1]
            };
            let start = app.midi_editor.layer_bounds[&ids[anchor]].icon.center();
            let end = match action {
                "same" => {
                    if group == [0, 1] {
                        start + Vec2::new(8.0, 0.0)
                    } else {
                        Pos2::new(
                            start.x,
                            app.midi_editor.layer_bounds[&ids[1]].row.bottom() - 8.0,
                        )
                    }
                }
                "outside" => Pos2::new(start.x, 5.0),
                "output" => app.instrument_ui.main_bounds.label.center(),
                "move" => Pos2::new(
                    start.x,
                    app.midi_editor.layer_bounds[&ids[2]].row.bottom() - 8.0,
                ),
                _ => Pos2::new(
                    start.x,
                    app.midi_editor.layer_bounds[&ids[0]].row.top() + 8.0,
                ),
            };
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            if action != "fast" {
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(app.session.project.tracks, original.tracks);
                assert!(!app.dirty);
                if action == "escape" {
                    frame(&mut app, &ctx, vec![layer_key(egui::Key::Escape)]);
                }
            }
            let copy = action == "copy" || action == "fast";
            let modifiers = if copy {
                layer_copy_modifiers()
            } else {
                Default::default()
            };
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(end),
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerButton {
                        pos: end,
                        pressed: false,
                        button: egui::PointerButton::Primary,
                        modifiers,
                    },
                ],
            );
            assert!(app.error.is_none(), "{group:?} {action}: {:?}", app.error);
            let current = app.session.project.tracks[0].instrument.as_ref().unwrap();
            let before = original.tracks[0].instrument.as_ref().unwrap();
            if copy {
                assert_eq!(current.layers.len(), 5);
                for (copy, index) in current.layers[..2].iter().zip(group) {
                    assert_ne!(copy.id, ids[index]);
                    assert_eq!(copy.name, before.layers[index].name);
                    assert_eq!(copy.effects[0].unit, before.layers[index].effects[0].unit);
                    assert_ne!(copy.effects[0].id, before.layers[index].effects[0].id);
                }
                assert_eq!(current.layers[2..], before.layers);
                assert_eq!(
                    app.instrument_ui.selected_layers,
                    current.layers[..2].iter().map(|l| l.id).collect()
                );
                assert_eq!(app.instrument_ui.selected_layer, Some(current.layers[1].id));
            } else if action == "move" || action == "unselected" {
                let moving = if action == "unselected" {
                    vec![anchor]
                } else {
                    group.to_vec()
                };
                let order: Vec<_> = if action == "move" {
                    (0..3)
                        .filter(|n| !moving.contains(n))
                        .chain(moving.iter().copied())
                        .collect()
                } else {
                    moving
                        .iter()
                        .copied()
                        .chain((0..3).filter(|n| !moving.contains(n)))
                        .collect()
                };
                let expected: Vec<_> = order
                    .into_iter()
                    .map(|n| before.layers[n].clone())
                    .collect();
                assert_eq!(current.layers, expected);
                assert_eq!(
                    app.instrument_ui.selected_layers,
                    moving.iter().map(|n| ids[*n]).collect()
                );
            } else if action == "same" && group == [0, 2] {
                // Closing a gap between selected members is a real group reorder.
                assert_eq!(
                    current.layers.iter().map(|l| l.id).collect::<Vec<_>>(),
                    vec![ids[1], ids[0], ids[2]]
                );
            } else {
                assert_eq!(current, before, "{group:?} {action}");
                assert!(!app.dirty, "{group:?} {action}");
            }
            app.session.project.validate().unwrap();
            assert!(!app.instrument_ui.dragging());
            if app.dirty {
                let changed = app.session.project.tracks.clone();
                app.perform_history_action(crate::HistoryAction::Undo, &ctx);
                assert_eq!(app.session.project.tracks, original.tracks);
                app.perform_history_action(crate::HistoryAction::Redo, &ctx);
                assert_eq!(app.session.project.tracks, changed);
            }
        }
    }
}

fn layer_copy_modifiers() -> egui::Modifiers {
    egui::Modifiers {
        alt: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        ..Default::default()
    }
}

#[test]
fn layer_drag_duplicates_on_release_and_preserves_the_complete_original() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for area in ["icon", "name", "blank"] {
                for fast in [false, true] {
                    let (mut app, ctx, _) = layer_header_fixture();
                    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                    for l in &mut i.layers {
                        l.collapsed = true;
                    }
                    i.layers[0].collapsed = compact;
                    i.layers[0].macro_module.add();
                    i.layers[0].effects = vec![daw_core::EffectInstance::limiter()];
                    app.set_settings(
                        &ctx,
                        settings::AppSettings {
                            theme,
                            ..Default::default()
                        },
                    );
                    frame(&mut app, &ctx, vec![]);
                    app.history = None;
                    app.ensure_history();
                    let original = app.session.project.clone();
                    let i = original.tracks[0].instrument.as_ref().unwrap();
                    let bounds = &app.midi_editor.layer_bounds[&i.layers[0].id];
                    let start = match area {
                        "icon" => bounds.icon.center(),
                        "name" => bounds.name.center(),
                        _ => Pos2::new(bounds.name.right() + 10.0, bounds.icon.center().y),
                    };
                    let last = app.midi_editor.layer_bounds[&i.layers[2].id].row;
                    let end = Pos2::new(start.x, last.top() + 8.0);
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(start), button(start, true)],
                    );
                    if !fast {
                        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                        assert!(app.instrument_ui.dragging());
                        assert_eq!(app.session.project.tracks, original.tracks);
                        assert!(!app.dirty);
                    }
                    let modifiers = layer_copy_modifiers();
                    frame(
                        &mut app,
                        &ctx,
                        vec![
                            egui::Event::PointerMoved(end),
                            egui::Event::ModifiersChanged(modifiers),
                            egui::Event::PointerButton {
                                pos: end,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                modifiers,
                            },
                        ],
                    );
                    assert!(app.error.is_none(), "{:?}", app.error);
                    assert!(!app.instrument_ui.dragging());
                    let current = app.session.project.tracks[0].instrument.as_ref().unwrap();
                    assert_eq!(current.layers.len(), 4, "{area} {compact} {fast}");
                    assert_eq!(current.layers[0], i.layers[0]);
                    assert_eq!(current.layers[1], i.layers[1]);
                    assert_eq!(current.layers[3], i.layers[2]);
                    assert_eq!(current.main, i.main);
                    let copy = &current.layers[2];
                    assert_ne!(copy.id, i.layers[0].id);
                    assert_eq!(copy.name, i.layers[0].name);
                    assert_eq!(copy.collapsed, compact);
                    assert_eq!(copy.effects[0].unit, i.layers[0].effects[0].unit);
                    assert_ne!(copy.effects[0].id, i.layers[0].effects[0].id);
                    assert_ne!(copy.oscillators[0].id, i.layers[0].oscillators[0].id);
                    assert_ne!(
                        copy.macro_module.controls[0].id,
                        i.layers[0].macro_module.controls[0].id
                    );
                    assert_eq!(app.instrument_ui.selected_layer, Some(copy.id));
                    assert_eq!(
                        app.session.project.tracks[0].clips,
                        original.tracks[0].clips
                    );
                    assert!(app.dirty);
                    app.session.project.validate().unwrap();
                    app.perform_history_action(crate::HistoryAction::Undo, &ctx);
                    assert_eq!(app.session.project.tracks, original.tracks);
                    app.perform_history_action(crate::HistoryAction::Redo, &ctx);
                    assert_eq!(
                        app.session.project.tracks[0]
                            .instrument
                            .as_ref()
                            .unwrap()
                            .layers
                            .len(),
                        4
                    );
                }
            }
        }
    }
}

#[test]
fn layer_copy_modifier_can_be_released_and_invalid_drops_do_not_copy() {
    for action in [
        "move", "escape", "outside", "output", "same", "changed", "disabled", "focus",
    ] {
        let (mut app, ctx, _) = layer_header_fixture();
        for l in &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers
        {
            l.collapsed = true;
        }
        frame(&mut app, &ctx, vec![]);
        let original = app.session.project.tracks[0].instrument.clone().unwrap();
        let bounds = &app.midi_editor.layer_bounds[&original.layers[0].id];
        let start = bounds.icon.center();
        let end = match action {
            "outside" => Pos2::new(start.x, 5.0),
            "output" => app.instrument_ui.main_bounds.label.center(),
            "same" => start + Vec2::new(8.0, 0.0),
            _ => Pos2::new(
                start.x,
                app.midi_editor.layer_bounds[&original.layers[2].id]
                    .row
                    .top()
                    + 8.0,
            ),
        };
        let modifiers = layer_copy_modifiers();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        if action == "changed" {
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers
                .swap(1, 2);
        }
        let mut events = vec![];
        if action == "escape" {
            events.push(layer_key(egui::Key::Escape));
        }
        let release_modifiers = if action == "move" {
            Default::default()
        } else {
            modifiers
        };
        events.push(egui::Event::ModifiersChanged(release_modifiers));
        events.push(egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: release_modifiers,
        });
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: action != "focus",
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(action != "disabled", |ui| app.show(ui));
            },
        );
        output.textures_delta.clear();
        assert!(app.error.is_none(), "{action}");
        let layers = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        let mut expected = original.layers;
        if action == "move" {
            expected.swap(0, 1);
        }
        if action == "changed" {
            expected.swap(1, 2);
        }
        if action == "same" {
            assert_eq!(layers.len(), 4);
            assert_ne!(layers[0].id, expected[0].id);
            assert_eq!(&layers[1..], &expected);
        } else {
            assert_eq!(layers, &expected, "{action}");
        }
        assert_eq!(app.dirty, matches!(action, "move" | "same"), "{action}");
        assert!(!app.instrument_ui.dragging());
    }
}

#[test]
fn layer_control_and_name_editor_drags_do_not_reorder_layers() {
    for control in 0..8 {
        let (mut app, ctx, layers) = layer_header_fixture();
        let id = layers[0].id;
        let start = if control == 6 {
            begin_layer_rename(&mut app, &ctx, id);
            app.midi_editor.layer_bounds[&id].name.center()
        } else if control == 7 {
            app.midi_editor.layer_bounds[&id].collapse.center()
        } else {
            app.midi_editor.layer_bounds[&id].mix_controls[control].center()
        };
        let end = start + Vec2::new(0.0, -20.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(!app.instrument_ui.dragging(), "control {control}");
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(!app.instrument_ui.dragging(), "control {control}");
        let current = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        assert_eq!(
            current.iter().map(|layer| layer.id).collect::<Vec<_>>(),
            layers.iter().map(|layer| layer.id).collect::<Vec<_>>()
        );
        assert_eq!(current[1..], layers[1..]);
        if control == 2 {
            assert!(current[0].gain_db > layers[0].gain_db);
        } else if control == 4 {
            assert!(current[0].pan > layers[0].pan);
        }
        if control == 6 {
            assert!(app.instrument_ui.editing_name());
            frame(&mut app, &ctx, vec![layer_key(egui::Key::Escape)]);
        }
    }
}

#[test]
fn dragging_layers_scrolls_the_rows_while_the_instrument_header_stays_fixed() {
    let (mut app, ctx, mut layers) = layer_header_fixture();
    for number in 4..=12 {
        let mut layer = daw_core::InstrumentLayerSettings {
            name: format!("Layer {number}"),
            ..Default::default()
        };
        layer.oscillators.push(Default::default());
        layers.push(layer);
    }
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers = layers.clone();
    app.dirty = false;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let viewport = shapes
        .iter()
        .find(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Lead"))
        .unwrap()
        .clip_rect;
    let header = app.midi_editor.instrument_header_bounds;
    let first_top = app.midi_editor.layer_bounds[&layers[0].id].row.top();
    let bounds = &app.midi_editor.layer_bounds[&layers[0].id];
    let start = Pos2::new(bounds.name.right() + 10.0, bounds.icon.center().y);
    let end = Pos2::new(start.x, viewport.bottom() - 2.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    for _ in 0..25 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(app.midi_editor.layer_bounds[&layers[0].id].row.top() < first_top - 10.0);
    assert_eq!(app.midi_editor.instrument_header_bounds, header);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers,
        layers
    );
    frame(&mut app, &ctx, vec![layer_key(egui::Key::Escape)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(!app.dirty);
    assert!(!app.instrument_ui.dragging());
}

#[test]
fn instrument_rows_stay_blank_until_components_are_added_from_the_catalog() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let mut app = DawUi {
            timeline_effects_visible: false,
            ..Default::default()
        };
        let track = app.session.project.add_midi_track().unwrap();
        app.selected_track = Some(track);
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Instrument, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for text in ["Layers", "Layer 1"] {
            assert!(shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Text(t) if t.galley.text() == text)));
        }
        for text in [
            "No source",
            "No envelope",
            "Add source",
            "Add envelope",
            "Generate sound with an oscillator.",
            "Shape this layer’s volume over time.",
        ] {
            assert!(!shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Text(t) if t.galley.text() == text)));
        }
        let first = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .id;
        let bounds = &app.midi_editor.layer_bounds[&first];
        assert_eq!(bounds.label.left(), bounds.row.left());
        assert_eq!(bounds.row.height(), panels::unit_height(false));
        assert_eq!(bounds.oscillator_panel, Rect::NOTHING);
        assert_eq!(bounds.envelope_panel, Rect::NOTHING);
        assert_eq!(app.midi_editor.add_oscillator_bounds, Rect::NOTHING);
        assert!(bounds.oscillators.is_empty());
        assert!(bounds.envelope_stages.iter().all(|r| *r == Rect::NOTHING));
        assert!(
            app.midi_editor
                .instrument_header_bounds
                .contains_rect(app.midi_editor.add_layer_bounds)
        );
        assert_eq!(
            app.midi_editor.add_layer_bounds.size(),
            Vec2::new(30.0, toolbars::CONTROL_HEIGHT)
        );
        let blank = Rect::from_min_max(
            Pos2::new(bounds.label.right(), bounds.row.top()),
            Pos2::new(
                app.instrument_catalog_bounds.layer_viewport.right(),
                bounds.row.bottom(),
            ),
        )
        .shrink(2.0);
        assert!(!shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Rect(r) if r.rect.intersects(blank) && r.fill == theme::palette(&ctx).panel)),
            "Empty layer bodies retain the workspace background");
        assert!(!shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::LineSegment { points, .. } if blank.contains(points[0]) && blank.contains(points[1]))),
            "Empty layer bodies have no card outlines");
        let before = app.session.project.tracks[0].instrument.clone();
        click_note_or_lane(&mut app, &ctx, blank.center());
        assert_eq!(app.session.project.tracks[0].instrument, before);
        assert!(!app.dirty);
        super::instrument_catalog::add_component(&mut app, &ctx, first, 0);
        super::instrument_catalog::add_component(&mut app, &ctx, first, 1);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .envelopes[0]
                .present
        );
        assert!(
            app.midi_editor.layer_bounds[&first]
                .envelope_stages
                .iter()
                .all(|r| r.is_positive())
        );
        assert_eq!(app.midi_editor.layer_bounds[&first].oscillators.len(), 1);
        let add = app.midi_editor.add_layer_bounds.center();
        click_note_or_lane(&mut app, &ctx, add);
        frame(&mut app, &ctx, vec![]);
        let layers = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].id, first);
        let second = layers[1].id;
        assert_ne!(first, second);
        assert!(!layers[1].envelopes[0].present);
        assert!(layers[1].oscillators.is_empty());
        assert!(
            (app.midi_editor.layer_bounds[&first].row.bottom()
                - app.midi_editor.layer_bounds[&second].row.top()
                - 1.0)
                .abs()
                < 0.01
        );
        super::instrument_catalog::add_component(&mut app, &ctx, second, 0);
        let layers = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        assert_eq!(layers[0].oscillators.len(), 1);
        assert_eq!(layers[1].oscillators.len(), 1);
        assert!(!layers[1].envelopes[0].present);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(!shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Text(t) if t.galley.text() == "Add source")));
        let oscillator = app.midi_editor.layer_bounds[&second].oscillators[0];
        let pitch = Pos2::new(
            oscillator.center().x,
            oscillator.top() + knobs::CHOICE_SIZE * 1.5 + theme::TOOLBAR_PADDING as f32,
        );
        click_note_or_lane(&mut app, &ctx, pitch);
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
        let layers = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        assert_eq!(layers[0].oscillators[0].transpose, 0);
        assert_eq!(layers[1].oscillators[0].transpose, 1);
    }
}

#[test]
fn layer_and_envelope_removal_keep_other_layers_and_guard_the_last_layer() {
    fn menu(app: &mut DawUi, ctx: &egui::Context, point: Pos2, text: &str) -> Pos2 {
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("context menu item")
    }
    for collapsed in [false, true] {
        let (mut app, _, _, _) = fixture();
        app.timeline_effects_visible = false;
        let settings = app.session.project.tracks[0].instrument.as_mut().unwrap();
        settings.layers[0].envelopes[0].settings.attack.time_ms = 75.0;
        settings
            .layers
            .push(daw_core::InstrumentLayerSettings::default());
        let first = settings.layers[0].id;
        let second = settings.layers[1].id;
        let ctx = context();
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        let envelope = app.midi_editor.layer_bounds[&first].envelope_stages[0];
        let header = Pos2::new(
            envelope.center().x,
            envelope.top() - panels::HEADER_HEIGHT * 0.5,
        );
        let header = if collapsed {
            double_click_unit_header(&mut app, &ctx, header);
            app.midi_editor.layer_bounds[&first]
                .envelope_header
                .center()
        } else {
            header
        };
        let remove = menu(&mut app, &ctx, header, "Remove envelope");
        let before = app.session.project.tracks[0].instrument.clone().unwrap();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(remove), button(remove, true)],
        );
        assert_eq!(
            app.session.project.tracks[0].instrument.as_ref(),
            Some(&before)
        );
        frame(&mut app, &ctx, vec![button(remove, false)]);
        frame(&mut app, &ctx, vec![]);
        let layers = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers;
        assert!(!layers[0].envelopes[0].present);
        assert_eq!(layers[0].envelopes[0].settings.attack.time_ms, 75.0);
        assert_eq!(layers[1], before.layers[1]);
        super::instrument_catalog::add_component(&mut app, &ctx, first, 1);
        assert_eq!(
            app.session.project.tracks[0].instrument.as_ref(),
            Some(&before)
        );
        let label = app.midi_editor.layer_bounds[&second].label.center();
        let remove = menu(&mut app, &ctx, label, "Remove layer");
        click_note_or_lane(&mut app, &ctx, remove);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers,
            vec![before.layers[0].clone()]
        );
        let label = app.midi_editor.layer_bounds[&first].label.center();
        let remove = menu(&mut app, &ctx, label, "Remove layer");
        click_note_or_lane(&mut app, &ctx, remove);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers
                .len(),
            1
        );
    }
}

#[test]
fn envelope_panel_matches_oscillator_height_and_stage_geometry_in_both_themes() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track, _, _) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Instrument, &ctx);
        for count in [1, 3] {
            let mut settings = app.session.project.tracks[0].instrument.clone().unwrap();
            settings.layers[0]
                .oscillators
                .resize_with(count, daw_core::OscillatorSettings::default);
            (
                settings.layers[0].envelopes[0].settings.attack.time_ms,
                settings.layers[0].envelopes[0].settings.sustain,
            ) = match count {
                0 => (5.0, 0.8),
                1 => (1200.0, 0.2),
                _ => (10_000.0, 0.99),
            };
            let readouts = [
                knobs::format_time(settings.layers[0].envelopes[0].settings.attack.time_ms),
                knobs::format_time(settings.layers[0].envelopes[0].settings.decay.time_ms),
                format!(
                    "{:.0}%",
                    settings.layers[0].envelopes[0].settings.sustain * 100.0
                ),
                knobs::format_time(settings.layers[0].envelopes[0].settings.release.time_ms),
            ];
            app.edit(Edit::SetInstrument {
                track_id: track,
                instrument: Some(settings),
            });
            // Timing and Mode share one strip before ADSR. Keep the full unit visible.
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 800.0));
            let oscillator = app.midi_editor.oscillator_panel_bounds;
            let envelope = app.midi_editor.envelope_panel_bounds;
            let waveform_size = shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Path(path) if (path.stroke.width - 1.25).abs() < 0.01 => {
                    let bounds = Rect::from_points(&path.points);
                    oscillator.contains_rect(bounds).then_some(bounds.size())
                }
                _ => None,
            });
            assert_eq!(waveform_size.is_some(), count > 0);
            assert_eq!(oscillator.top(), envelope.top());
            assert_eq!(oscillator.bottom(), envelope.bottom());
            assert!((envelope.left() - oscillator.right() + 1.0).abs() < 0.01);
            let divider_x = oscillator.right() - 0.5;
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, stroke }
                    if points[0] == Pos2::new(divider_x, oscillator.top())
                        && points[1] == Pos2::new(divider_x, envelope.bottom())
                        && stroke.width == 1.0 && stroke.color == theme::palette(&ctx).border)));
            for panel in [oscillator, envelope] {
                let edge = panel.shrink(0.5);
                // The preceding column/unit owns the shared left edge.
                for expected in [
                    [edge.left_top(), edge.right_top()],
                    [edge.left_bottom(), edge.right_bottom()],
                    [edge.right_top(), edge.right_bottom()],
                ] {
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::LineSegment { points, stroke }
                            if *points == expected && *stroke == Stroke::new(1.0, theme::palette(&ctx).border))));
                }
            }
            for (index, label) in ["Attack", "Decay", "Sustain", "Release"]
                .into_iter()
                .enumerate()
            {
                let strip = app.midi_editor.envelope_stage_bounds[index];
                assert!((strip.width() - midi_editor::UNIT_STRIP_WIDTH).abs() < 0.01);
                assert!((strip.top() - envelope.top() - panels::HEADER_HEIGHT - 1.0).abs() < 0.01);
                assert!((envelope.bottom() - strip.bottom() - 1.0).abs() < 0.01);
                if index > 0 {
                    assert_eq!(
                        strip.left(),
                        app.midi_editor.envelope_stage_bounds[index - 1].right()
                    );
                    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::LineSegment { points, stroke }
                            if points[0] == strip.left_top() && points[1] == strip.left_bottom()
                                && stroke.width == 1.0 && stroke.color == theme::unit_separator(&ctx))));
                }
                let mut centers: Vec<_> = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Circle(circle)
                            if (circle.radius - 9.0).abs() < 0.01
                                && strip.contains(circle.center) =>
                        {
                            Some(circle.center)
                        }
                        _ => None,
                    })
                    .collect();
                centers.sort_by(|a, b| a.y.total_cmp(&b.y));
                assert_eq!(centers.len(), if index == 2 { 1 } else { 3 });
                for (row, center) in centers.iter().enumerate() {
                    assert!((center.x - strip.center().x).abs() < 0.01);
                    let expected_y = strip.top()
                        + 10.0
                        + knobs::CHOICE_SIZE * 0.5
                        + row as f32 * (knobs::CHOICE_SIZE + f32::from(theme::TOOLBAR_PADDING));
                    assert!((center.y - expected_y).abs() < 0.01);
                    if row == 1 {
                        let row_bounds =
                            Rect::from_center_size(*center, Vec2::splat(knobs::CHOICE_SIZE));
                        let diagrams: Vec<_> = shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::Shape::Path(path)
                                    if (path.stroke.width - 1.25).abs() < 0.01
                                        && row_bounds
                                            .contains_rect(Rect::from_points(&path.points)) =>
                                {
                                    Some(path)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(diagrams.len(), 4);
                        for (choice, diagram) in diagrams.iter().enumerate() {
                            let bounds = Rect::from_points(&diagram.points);
                            assert!((bounds.width() - 10.0).abs() < 0.01);
                            assert!((bounds.height() - 6.0).abs() < 0.01);
                            if let Some(size) = waveform_size {
                                assert!((bounds.size() - size).length() < 0.01);
                            }
                            let start = diagram.points[0];
                            let end = *diagram.points.last().unwrap();
                            let deviation = diagram
                                .points
                                .iter()
                                .map(|point| {
                                    let t = (point.x - start.x) / (end.x - start.x);
                                    (point.y - (start.y + t * (end.y - start.y))).abs()
                                })
                                .fold(0.0_f32, f32::max);
                            if choice == 0 {
                                assert!(deviation < 0.01, "Linear stays straight");
                            } else {
                                assert!(deviation > 0.5, "Curve symbol must have a visible bend");
                            }
                        }
                    }
                    if row != 1 {
                        let endpoints = if index == 2 || row == 2 {
                            ["0%", "100%"]
                        } else {
                            ["0 ms", "10 s"]
                        };
                        let mut endpoint_bounds = Vec::new();
                        for endpoint in endpoints {
                            let text = shapes
                                .iter()
                                .find_map(|shape| match &shape.shape {
                                    egui::Shape::Text(text)
                                        if text.galley.text() == endpoint
                                            && strip.contains(text.pos)
                                            && text.pos.y > center.y
                                            && text.pos.y < center.y + knobs::CHOICE_SIZE * 0.5 =>
                                    {
                                        Some(text)
                                    }
                                    _ => None,
                                })
                                .unwrap();
                            let bounds = text.galley.rect.translate(text.pos.to_vec2());
                            assert!(bounds.top() > center.y);
                            assert!(bounds.bottom() <= center.y + knobs::CHOICE_SIZE * 0.5);
                            assert!(strip.contains_rect(bounds));
                            endpoint_bounds.push(bounds);
                        }
                        assert!(endpoint_bounds[0].center().x < center.x);
                        assert!(endpoint_bounds[1].center().x > center.x);
                        assert!(endpoint_bounds[0].right() < endpoint_bounds[1].left());
                    }
                }
                let readout = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if text.galley.text() == readouts[index]
                                && strip.contains(text.pos) =>
                        {
                            Some(text)
                        }
                        _ => None,
                    })
                    .unwrap();
                let readout_bounds = readout.galley.rect.translate(readout.pos.to_vec2());
                assert!(
                    (readout_bounds.center().x - centers[0].x).abs() < 0.51,
                    "{label} value {readout_bounds:?}, knob {:?}",
                    centers[0]
                );
                assert!((readout_bounds.bottom() - (centers[0].y - 15.0)).abs() < 0.51);
                assert!(readout_bounds.top() >= centers[0].y - knobs::CHOICE_SIZE * 0.5);
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                        _ => None,
                    })
                    .unwrap();
                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                assert!(
                    (bounds.center().x - strip.center().x).abs() < 0.51,
                    "{label} label {bounds:?}, strip {strip:?}"
                );
                assert!(
                    (bounds.center().y
                        - (strip.bottom() - 10.0 - icons::COMPACT_BUTTON_SIZE * 0.5))
                        .abs()
                        < 0.51
                );
                if count > 0 {
                    assert!(
                        (bounds.center().y - app.midi_editor.oscillator_power_bounds[0].center().y)
                            .abs()
                            < 0.51
                    );
                }
            }
            let header = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Envelope 1" => Some(text),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                header.galley.job.sections[0].format.color,
                theme::unit_header_text(theme::unit_header_fill(&ctx, true))
            );
        }
    }
}

#[test]
fn envelope_controls_edit_only_the_instrument_and_linear_retains_disabled_amount() {
    use daw_core::CurveShape;
    let (mut app, _, _, _) = fixture();
    let ctx = context();
    let assets = serde_json::to_value(&app.session.project.assets).unwrap();
    let sources = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .oscillators
        .clone();
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let point = |app: &DawUi, stage: usize, row: usize| {
        let strip = app.midi_editor.envelope_stage_bounds[stage];
        Pos2::new(
            strip.center().x,
            strip.top()
                + 10.0
                + knobs::CHOICE_SIZE * 0.5
                + row as f32 * (knobs::CHOICE_SIZE + f32::from(theme::TOOLBAR_PADDING)),
        )
    };
    let arrow = |app: &mut DawUi, key| {
        for pressed in [true, false] {
            frame(
                app,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
    };
    let disabled = point(&app, 0, 2);
    click_note_or_lane(&mut app, &ctx, disabled);
    arrow(&mut app, egui::Key::ArrowUp);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .attack
            .amount,
        0.5
    );
    let shape = point(&app, 0, 1);
    click_note_or_lane(&mut app, &ctx, shape);
    arrow(&mut app, egui::Key::ArrowUp);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .attack
            .shape,
        CurveShape::Exponential
    );
    let amount = point(&app, 0, 2);
    click_note_or_lane(&mut app, &ctx, amount);
    arrow(&mut app, egui::Key::ArrowUp);
    assert!(
        (app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .attack
            .amount
            - 0.51)
            .abs()
            < 1e-6
    );
    let time = point(&app, 0, 0);
    click_note_or_lane(&mut app, &ctx, time);
    arrow(&mut app, egui::Key::ArrowUp);
    assert!(
        (app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .attack
            .time_ms
            - 6.0)
            .abs()
            < 1e-5
    );
    let sustain = point(&app, 2, 0);
    click_note_or_lane(&mut app, &ctx, sustain);
    arrow(&mut app, egui::Key::ArrowDown);
    assert!(
        (app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .sustain
            - 0.79)
            .abs()
            < 1e-6
    );
    click_note_or_lane(&mut app, &ctx, shape);
    arrow(&mut app, egui::Key::ArrowDown);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .envelopes[0]
            .settings
            .attack
            .shape,
        CurveShape::Linear
    );
    click_note_or_lane(&mut app, &ctx, disabled);
    arrow(&mut app, egui::Key::ArrowUp);
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!((instrument.layers[0].envelopes[0].settings.attack.amount - 0.51).abs() < 1e-6);
    assert_eq!(instrument.layers[0].oscillators, sources);
    assert_eq!(
        serde_json::to_value(&app.session.project.assets).unwrap(),
        assets
    );
}

#[test]
fn oscillator_module_adds_sources_on_the_right_and_removes_the_target_through_its_menu() {
    for dark in [true, false] {
        let (mut app, track, _, _) = fixture();
        let assets = serde_json::to_value(&app.session.project.assets).unwrap();
        let ctx = context();
        if dark {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        for count in 1..=2 {
            for strip in &app.midi_editor.oscillator_strip_bounds {
                assert!(
                    (strip.width() - midi_editor::OSCILLATOR_STRIP_WIDTH).abs() < 0.1,
                    "strip width must not change when the source count changes"
                );
            }
            let button = app.midi_editor.add_oscillator_bounds;
            assert!(button.left() > app.midi_editor.oscillator_bounds[count - 1].right());
            let before = app.session.project.tracks[0].instrument.clone();
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(button.left_top() + Vec2::splat(3.0)),
                    super::button(button.left_top() + Vec2::splat(3.0), true),
                ],
            );
            assert_eq!(
                app.session.project.tracks[0].instrument, before,
                "commit addition on release"
            );
            frame(
                &mut app,
                &ctx,
                vec![super::button(button.left_top() + Vec2::splat(3.0), false)],
            );
            frame(&mut app, &ctx, vec![]);
            assert_eq!(
                app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers[0]
                    .oscillators
                    .len(),
                count + 1
            );
        }
        let mut settings = app.session.project.tracks[0].instrument.clone().unwrap();
        settings.layers[0].oscillators[0].waveform = Waveform::Saw;
        settings.layers[0].oscillators[1].waveform = Waveform::Square;
        settings.layers[0].oscillators[2].waveform = Waveform::Triangle;
        app.edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(settings),
        });
        frame(&mut app, &ctx, vec![]);
        let second = app.midi_editor.oscillator_bounds[1];
        let transpose = Pos2::new(
            second.center().x,
            second.top() + knobs::CHOICE_SIZE * 1.5 + theme::TOOLBAR_PADDING as f32,
        );
        click_note_or_lane(&mut app, &ctx, transpose);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowUp,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
        let settings = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(
            settings.layers[0]
                .oscillators
                .iter()
                .map(|s| s.transpose)
                .collect::<Vec<_>>(),
            vec![0, 1, 0]
        );
        for index in [1, 0, 0] {
            for strip in &app.midi_editor.oscillator_strip_bounds {
                assert!((strip.width() - midi_editor::OSCILLATOR_STRIP_WIDTH).abs() < 0.1);
            }
            let bounds = app.midi_editor.oscillator_bounds[index];
            // Cover strip padding as well as interactive knob faces.
            let point = if index == 1 {
                let strip = app.midi_editor.oscillator_strip_bounds[index];
                Pos2::new(strip.left() + 3.0, strip.center().y)
            } else {
                Pos2::new(bounds.center().x, bounds.top() + knobs::CHOICE_SIZE * 0.5)
            };
            let before = app.session.project.tracks[0].instrument.clone().unwrap();
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Secondary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let menu = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Remove oscillator" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("oscillator context menu");
            assert_eq!(
                app.session.project.tracks[0].instrument.as_ref().unwrap(),
                &before
            );
            click_note_or_lane(&mut app, &ctx, menu);
            frame(&mut app, &ctx, vec![]);
            let mut expected = before;
            expected.layers[0].oscillators.remove(index);
            assert_eq!(
                app.session.project.tracks[0].instrument.as_ref().unwrap(),
                &expected
            );
        }
        assert!(app.midi_editor.oscillator_bounds.is_empty());
        assert_eq!(app.midi_editor.add_oscillator_bounds, Rect::NOTHING);
        let layer = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .id;
        super::instrument_catalog::add_component(&mut app, &ctx, layer, 0);
        let added = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .oscillators;
        assert_eq!(added.len(), 1);
        assert!(!added[0].id.is_nil());
        assert_eq!(
            added[0],
            daw_core::OscillatorSettings {
                id: added[0].id,
                ..Default::default()
            }
        );
        assert_eq!(
            serde_json::to_value(&app.session.project.assets).unwrap(),
            assets
        );
        assert_eq!(app.selected_track, Some(track));
        assert!(app.dirty);
        let mut settings = app.session.project.tracks[0].instrument.clone().unwrap();
        settings.layers[0].oscillators[0].transpose = -24;
        settings.layers[0].oscillators[0].gain_db = daw_core::SILENT_GAIN_DB;
        app.edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(settings),
        });
        let readouts = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for value in ["-24", "−∞ dB"] {
            assert!(
                readouts
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == value))
            );
        }
    }
}

#[test]
fn oscillator_strips_have_centered_power_label_pairs_and_a_clickable_add_strip() {
    for dark in [true, false] {
        let (mut app, track, _, _) = fixture();
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .oscillators
            .push(Default::default());
        let ctx = context();
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.tracks[0].instrument.clone().unwrap();
        let assets = serde_json::to_value(&app.session.project.assets).unwrap();
        for (column, button) in app
            .midi_editor
            .oscillator_bounds
            .iter()
            .zip(&app.midi_editor.oscillator_power_bounds)
        {
            assert_eq!(button.size(), Vec2::splat(icons::COMPACT_BUTTON_SIZE));
            assert!(button.center().x < column.center().x);
            assert!((column.bottom() - button.bottom()).abs() < 0.1);
            assert!(button.top() > column.top() + 3.0 * knobs::CHOICE_SIZE);
        }
        assert_eq!(app.midi_editor.oscillator_power_bounds.len(), 2);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for (index, (column, button)) in app
            .midi_editor
            .oscillator_bounds
            .iter()
            .zip(&app.midi_editor.oscillator_power_bounds)
            .enumerate()
        {
            let name = format!("Osc {}", index + 1);
            let label = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == name => Some(text),
                    _ => None,
                })
                .unwrap();
            let bounds = label.galley.rect.translate(label.pos.to_vec2());
            assert!((button.union(bounds).center().x - column.center().x).abs() < 0.51);
            assert!((bounds.center().y - button.center().y).abs() < 0.51);
            assert!((bounds.left() - button.right() - 8.0).abs() < 0.51);
            assert!(column.contains_rect(bounds));
            assert_eq!(
                label.galley.job.sections[0].format.font_id.size,
                panels::HEADER_FONT_SIZE
            );
            assert_eq!(
                label.galley.job.sections[0].format.color,
                theme::palette(&ctx).text
            );
        }
        let columns = &app.midi_editor.oscillator_strip_bounds;
        let margin = f32::from(panels::PanelStyle::default().margin);
        for (strip, controls) in columns.iter().zip(&app.midi_editor.oscillator_bounds) {
            for padding in [
                controls.left() - strip.left(),
                strip.right() - controls.right(),
                controls.top() - strip.top(),
                strip.bottom() - controls.bottom(),
            ] {
                assert!(
                    (padding - margin).abs() < 0.1,
                    "equal padding on every strip side: {padding}"
                );
            }
        }
        assert!(
            (columns[0].right() - columns[1].left()).abs() < 0.1,
            "strips share one boundary"
        );
        let add_strip = app.midi_editor.add_oscillator_strip_bounds;
        assert_eq!(
            app.midi_editor.add_oscillator_bounds, add_strip,
            "the whole strip is clickable"
        );
        let add_button = Rect::from_center_size(
            add_strip.center(),
            app.midi_editor.oscillator_power_bounds[0].size(),
        );
        assert!(add_strip.contains_rect(add_button) && add_strip.height() > add_button.height());
        let add_circle = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Circle(c) if c.center == add_button.center() => Some(c),
                _ => None,
            })
            .expect("circular Add control");
        assert_eq!(
            add_circle.radius * 2.0 + add_circle.stroke.width,
            icons::COMPACT_BUTTON_SIZE
        );
        assert_eq!(
            add_circle.stroke,
            egui::Stroke::new(1.0, theme::palette(&ctx).secondary)
        );
        for power in &app.midi_editor.oscillator_power_bounds {
            let power_circle = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Circle(c) if c.center == power.center() => Some(c),
                    _ => None,
                })
                .expect("circular Power control");
            assert_eq!(add_circle.radius, power_circle.radius);
            assert_eq!(add_circle.stroke.width, power_circle.stroke.width);
        }
        assert!((add_strip.width() - icons::COMPACT_BUTTON_SIZE - 2.0 * margin).abs() < 0.1);
        let add_icon = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(image)
                    if image.brush.is_some() && add_strip.contains(image.rect.center()) =>
                {
                    Some(image.rect)
                }
                _ => None,
            })
            .expect("painted Plus icon in the add button");
        assert!(
            (add_icon.center() - add_strip.center()).length() < 0.1,
            "the actual icon must be centered on both axes"
        );
        let power_icon = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(image)
                    if image.brush.is_some()
                        && app.midi_editor.oscillator_power_bounds[0]
                            .contains(image.rect.center()) =>
                {
                    Some(image.rect)
                }
                _ => None,
            })
            .expect("painted Power icon");
        assert_eq!(add_icon.size(), power_icon.size());
        assert!((columns[1].right() - add_strip.left()).abs() < 0.1);
        assert!((columns[1].top() - add_strip.top()).abs() < 0.1);
        assert!((columns[1].bottom() - add_strip.bottom()).abs() < 0.1);
        for next in [columns[1], add_strip] {
            let separators = shapes
                .iter()
                .filter(|s| {
                    matches!(&s.shape,
                        egui::Shape::LineSegment { points, stroke, .. }
                            if (points[0].x - next.left()).abs() < 0.1 && points[0].x == points[1].x
                            && (points[0].y - columns[0].top()).abs() < 0.1
                            && (points[1].y - next.bottom()).abs() < 0.1
                            && stroke.width == 1.0 && stroke.color == theme::unit_separator(&ctx)
                    )
                })
                .count();
            assert_eq!(
                separators, 1,
                "separator before every subsequent strip, including Add"
            );
        }
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with("Oscillator "))));
        for power in &app.midi_editor.oscillator_power_bounds {
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Circle(c)
                if c.center == power.center() && c.fill == theme::toggle_colors(&ctx, true).0
                && c.stroke == egui::Stroke::new(1.0, theme::accent(&ctx))))
            );
        }
        let hover_shapes = frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(
                add_strip.left_top() + Vec2::splat(3.0),
            )],
            Vec2::new(1280.0, 800.0),
        );
        assert!(
            hover_shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Circle(c)
            if c.center == add_button.center() && c.radius * 2.0 + c.stroke.width == icons::COMPACT_BUTTON_SIZE
            && c.fill == ctx.global_style().visuals.widgets.hovered.weak_bg_fill))
        );
        assert!(
            hover_shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.rect == add_strip && r.stroke == egui::Stroke::NONE
            && r.fill == ctx.global_style().visuals.widgets.hovered.weak_bg_fill))
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(1000.0, 700.0))],
        );
        let idle_shapes = frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(1000.0, 700.0))],
            Vec2::new(1280.0, 800.0),
        );
        assert!(!idle_shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == add_strip && r.fill != egui::Color32::TRANSPARENT)));
        let palette = theme::palette(&ctx);
        let separator = theme::unit_separator(&ctx);
        for channel in 0..3 {
            let contrast =
                separator.to_array()[channel].abs_diff(palette.panel.to_array()[channel]);
            let border_contrast =
                palette.border.to_array()[channel].abs_diff(palette.panel.to_array()[channel]);
            assert!(
                contrast >= 8 && contrast < border_contrast,
                "separator must remain visible below border contrast in both themes"
            );
        }
        let point = app.midi_editor.oscillator_power_bounds[1].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert_eq!(
            app.session.project.tracks[0].instrument.as_ref().unwrap(),
            &before
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        frame(&mut app, &ctx, vec![]);
        let mut expected = before.clone();
        expected.layers[0].oscillators[1].muted = true;
        assert_eq!(
            app.session.project.tracks[0].instrument.as_ref().unwrap(),
            &expected
        );
        assert!(!app.session.project.tracks[0].muted);
        let muted_shapes = frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(1000.0, 700.0))],
            Vec2::new(1280.0, 800.0),
        );
        assert!(
            muted_shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Circle(c)
            if c.center == point && c.fill == theme::toggle_colors(&ctx, false).0
            && c.stroke == egui::Stroke::new(1.0, theme::palette(&ctx).secondary)))
        );
        assert!(muted_shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Osc 2"
                && text.galley.job.sections[0].format.color == theme::palette(&ctx).secondary)));
        click_note_or_lane(&mut app, &ctx, point);
        assert_eq!(
            app.session.project.tracks[0].instrument.as_ref().unwrap(),
            &before
        );
        assert_eq!(
            serde_json::to_value(&app.session.project.assets).unwrap(),
            assets
        );
        assert_eq!(app.selected_track, Some(track));
        // Both toggles remain undoable, but their final state matches the baseline.
        assert!(!app.dirty);
        assert!(app.history_action_enabled(HistoryAction::Undo, &ctx));
    }
}
#[test]
fn midi_tracks_units_editor_and_clip_copy_use_shared_actions() {
    let (mut app, track, clip, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.edit(Edit::SetInstrument {
        track_id: track,
        instrument: Some(
            daw_core::OscillatorSettings {
                waveform: Waveform::Saw,
                ..Default::default()
            }
            .into(),
        ),
    });
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .oscillators[0]
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
    assert!(app.clip_action_enabled(ClipAction::Paste, &ctx));
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
            instrument: Some(
                daw_core::OscillatorSettings {
                    waveform: Waveform::Square,
                    ..Default::default()
                }
                .into(),
            ),
        })
        .unwrap();
    let original = format!("{:?}", app.session.project);
    let dirty = app.dirty;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    for mode in [ViewMode::Instrument, ViewMode::Edit] {
        app.set_view_mode(mode, &ctx);
        // Edit still follows the active clip after selecting a track in Instrument.
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
        if mode == ViewMode::Instrument {
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
    app.set_view_mode(ViewMode::Instrument, &ctx);
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
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r)
                        if r.fill == theme::unit_header_fill(&ctx, true)
                            && r.rect.height() == 24.0
                            && r.rect.width() > 24.0 =>
                    {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .min_by(|a, b| a.top().total_cmp(&b.top()))
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
                assert_eq!(
                    section.format.color,
                    theme::unit_header_text(theme::unit_header_fill(&ctx, true))
                );
            }
            let text_center = text.pos + text.galley.rect.center().to_vec2();
            assert!((text_center.x - header.center().x).abs() < 1.0);
            assert!((text_center.y - header.center().y).abs() < 1.0);
            let instrument = app.midi_editor.instrument_header_bounds;
            let heading = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Layers" => Some(t),
                    _ => None,
                })
                .unwrap();
            assert!(
                (heading.pos.x - instrument.left() - f32::from(theme::TOOLBAR_PADDING)).abs() < 0.1
            );
            assert!(
                instrument.contains_rect(Rect::from_min_size(heading.pos, heading.galley.size()))
            );
            for section in &heading.galley.job.sections {
                assert_eq!(
                    heading.override_text_color.unwrap_or_else(|| {
                        if section.format.color == egui::Color32::PLACEHOLDER {
                            heading.fallback_color
                        } else {
                            section.format.color
                        }
                    }),
                    theme::palette(&ctx).text
                );
            }
            assert!(
                !shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Rect(r) if r.rect.intersect(instrument).is_positive() && r.fill == accent.color()))
            );
            let toolbar_bottom = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r)
                        if r.fill == theme::palette(&ctx).panel
                            && r.rect.width() > 1000.0
                            && r.rect.bottom() <= instrument.top() =>
                    {
                        Some(r.rect.bottom())
                    }
                    _ => None,
                })
                .max_by(f32::total_cmp)
                .unwrap();
            assert!(
                (instrument.top() - toolbar_bottom - 1.0).abs() < 0.1,
                "Only one point separates the toolbar and unit header: {}",
                instrument.top() - toolbar_bottom
            );
            let dividers = shapes.iter().filter(|s| matches!(&s.shape,
                egui::Shape::LineSegment { points, stroke, .. }
                    if points[0].y == points[1].y && (points[0].y - (instrument.top() - 0.5)).abs() < 0.1
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
fn new_destination_tracks_use_fresh_instruments_for_copies_and_preserve_move_sounds() {
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
                instrument: Some(
                    daw_core::OscillatorSettings {
                        waveform: Waveform::Square,
                        ..Default::default()
                    }
                    .into(),
                ),
            })
            .unwrap();
        let before = format!("{:?}", app.session.project);
        // Deliberately visit the lower destination first. The intervening gap stays empty.
        let originals = [original.clone(), second_clip.clone()];
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
        for (index, source) in [(2, 0), (4, 1)] {
            assert_eq!(tracks[index].kind, TrackKind::Midi);
            let instrument = tracks[index].instrument.as_ref().unwrap();
            if duplicate {
                assert_eq!(instrument.layers.len(), 1);
                assert!(instrument.layers[0].oscillators.is_empty());
                assert!(instrument.layers[0].sampler.is_none());
            } else {
                assert_eq!(Some(instrument), tracks[source].instrument.as_ref());
            }
            let placed = &tracks[index].clips[0];
            let mut expected = originals[source].clone();
            assert_eq!(placed.id == expected.id, !duplicate);
            expected.id = placed.id;
            assert_eq!(format!("{placed:?}"), format!("{expected:?}"));
            if duplicate {
                assert_eq!(tracks[source].clips.len(), 1);
                assert_eq!(
                    format!("{:?}", tracks[source].clips[0]),
                    format!("{:?}", originals[source])
                );
            }
        }
        assert_eq!(tracks[3].kind, TrackKind::Audio);
        assert!(tracks[3].clips.is_empty());
        assert_eq!(tracks[0].clips.len(), usize::from(duplicate));
        assert_eq!(tracks[1].clips.len(), usize::from(duplicate));
        app.session.project.validate().unwrap();
    }
}

#[test]
fn group_paste_creates_fresh_midi_tracks_and_retains_existing_destination_instruments() {
    let (mut app, first, first_clip, _) = fixture();
    let second = app.session.project.add_midi_track().unwrap();
    let second_clip = app.session.project.add_midi_pattern(second, 0).unwrap();
    app.session.project.tracks[1].instrument = Some(
        daw_core::OscillatorSettings {
            waveform: Waveform::Square,
            ..Default::default()
        }
        .into(),
    );
    app.selected_clip = Some(first_clip);
    app.selected_clips = HashSet::from([first_clip, second_clip]);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.perform_clip_action(ClipAction::Copy, &ctx);
    app.select_track(second, false);
    app.session.project.transport.playhead_frame = frames(10.0);
    let before = app.session.project.tracks.clone();
    app.perform_clip_action(ClipAction::Paste, &ctx);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_eq!(app.session.project.tracks.len(), 3);
    for (track, original) in app.session.project.tracks.iter().zip(&before) {
        assert_eq!(track.instrument, original.instrument);
        assert_eq!(
            format!("{:?}", track.clips[0]),
            format!("{:?}", original.clips[0])
        );
    }
    let fresh = &app.session.project.tracks[2];
    assert_eq!(fresh.kind, TrackKind::Midi);
    let instrument = fresh.instrument.as_ref().unwrap();
    assert_eq!(instrument.layers.len(), 1);
    assert!(instrument.layers[0].oscillators.is_empty());
    assert_eq!(fresh.clips[0].asset_id, before[1].clips[0].asset_id);
    assert_ne!(fresh.clips[0].id, second_clip);
    assert_eq!(fresh.clips[0].start_frame, frames(10.0));
    assert_eq!(app.selected_track, Some(second));
    assert_eq!(app.session.project.tracks[0].id, first);
    app.session.project.validate().unwrap();
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
fn empty_midi_lane_double_click_creates_one_beat_without_switching_views() {
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
    for view in [ViewMode::Instrument, ViewMode::Mixer] {
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
    assert!(app.drag.is_some());
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
    assert!(app.drag.is_none());
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
    assert!(app.drag.is_some());
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
    assert!(app.drag.is_none());
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
    let start = Pos2::new(block.left() + 1.0, block.top() + 30.0);
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
    let start = Pos2::new(rect.left() + 1.0, rect.top() + 30.0);
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

#[test]
fn off_grid_pattern_edges_snap_to_project_grid_after_scroll_and_stretch() {
    for zoom in [120.0, 240.0] {
        for stretched in [false, true] {
            for (edge, expand) in [
                (daw_core::ClipEdge::Left, false),
                (daw_core::ClipEdge::Left, true),
                (daw_core::ClipEdge::Right, false),
                (daw_core::ClipEdge::Right, true),
            ] {
                let (mut app, track, clip, _) = fixture();
                let current = &mut app.session.project.tracks[0].clips[0];
                current.start_frame = 10_200;
                if stretched {
                    current.stretch = daw_core::ClipStretch::new(2, 3);
                    current.length_frames = 144_000;
                }
                app.session.project.validate().unwrap();
                let original_clip = app.session.project.tracks[0].clips[0].clone();
                let ctx = context();
                app.open_clip_editor(clip, track, &ctx);
                app.midi_editor.zoom = zoom;
                app.midi_editor.scroll = 0.025;
                frame(&mut app, &ctx, vec![]);
                let original = format!("{:?}", app.session.project);
                let block = app.midi_editor.clip_bounds[&clip];
                let edge_frame = if edge == daw_core::ClipEdge::Left {
                    original_clip.start_frame
                } else {
                    original_clip.end()
                };
                let spacing = if zoom == 120.0 { 6000 } else { 3000 };
                let increasing = (edge == daw_core::ClipEdge::Right) == expand;
                let target = (edge_frame / spacing + if increasing { 2 } else { 0 }) * spacing
                    - if increasing { 0 } else { spacing };
                let origin = Pos2::new(
                    if edge == daw_core::ClipEdge::Left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    block.top() + 30.0,
                );
                // Request a position two pixels past the line, inside the ruler's snap radius.
                let end = origin
                    + Vec2::new(
                        (target as f64 - edge_frame as f64) as f32 / 48_000.0 * zoom + 2.0,
                        0.0,
                    );
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(origin), button(origin, true)],
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(format!("{:?}", app.session.project), original);
                let preview = app.midi_editor.clip_bounds[&clip];
                let preview_edge_x = if edge == daw_core::ClipEdge::Left {
                    preview.left()
                } else {
                    preview.right()
                };
                let target_x = app.midi_editor.track_bounds.left()
                    + (target as f64 / 48_000.0 - app.midi_editor.scroll) as f32 * zoom;
                assert!(
                    (preview_edge_x - target_x).abs() < 0.2,
                    "{preview_edge_x} != {target_x}"
                );
                frame(&mut app, &ctx, vec![button(end, false)]);
                assert!(app.error.is_none(), "{:?}", app.error);
                let result = &app.session.project.tracks[0].clips[0];
                let result_edge = if edge == daw_core::ClipEdge::Left {
                    result.start_frame
                } else {
                    result.end()
                };
                assert!(
                    result_edge.abs_diff(target) <= 19,
                    "{result_edge} != {target}"
                );
                assert_eq!(result.stretch, original_clip.stretch);
                if edge == daw_core::ClipEdge::Left {
                    assert!(result.end().abs_diff(original_clip.end()) <= 19);
                } else {
                    assert_eq!(result.start_frame, original_clip.start_frame);
                }
            }
        }
    }
}

#[test]
fn off_grid_notes_draw_move_and_resize_using_the_project_ruler_grid() {
    for stretched in [false, true] {
        for gesture in ["draw", "move", "start", "end"] {
            let (mut app, track, clip, note_id) = fixture();
            let source = app.session.project.tracks[0].clips[0].asset_id;
            let content = app
                .session
                .project
                .assets
                .iter_mut()
                .find(|a| a.id == source)
                .unwrap()
                .midi_pattern
                .as_mut()
                .unwrap();
            content.notes[0].start_tick = 1000;
            content.notes[0].length_ticks = 900;
            let current = &mut app.session.project.tracks[0].clips[0];
            current.start_frame = 10_200;
            if stretched {
                current.stretch = daw_core::ClipStretch::new(2, 3);
                current.length_frames = 144_000;
            }
            let original_clip = current.clone();
            let original_note = pattern(&app).notes[0].clone();
            let ctx = context();
            app.open_clip_editor(clip, track, &ctx);
            app.midi_editor.scroll = 0.025;
            frame(&mut app, &ctx, vec![]);
            let before = format!("{:?}", app.session.project);
            let note = app.midi_editor.note_bounds[&note_id];
            let target = match gesture {
                "end" => 90_000,
                "start" => 30_000,
                "move" => 60_000,
                _ => 48_000,
            };
            let target_x = app.midi_editor.track_bounds.left()
                + (target as f64 / 48_000.0 - app.midi_editor.scroll) as f32 * app.midi_editor.zoom;
            if gesture == "draw" {
                let point = Pos2::new(target_x + 2.0, note.center().y - 18.0);
                draw_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                assert_eq!(format!("{:?}", app.session.project), before);
                draw_frame(&mut app, &ctx, vec![button(point, false)]);
            } else {
                let origin = match gesture {
                    "start" => Pos2::new(note.left() + 1.0, note.center().y),
                    "end" => Pos2::new(note.right() - 1.0, note.center().y),
                    _ => note.center(),
                };
                let edge_x = if gesture == "end" {
                    note.right()
                } else {
                    note.left()
                };
                let end = origin + Vec2::new(target_x + 2.0 - edge_x, 0.0);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(origin), button(origin, true)],
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(format!("{:?}", app.session.project), before);
                frame(&mut app, &ctx, vec![button(end, false)]);
            }
            let result = pattern(&app)
                .notes
                .iter()
                .find(|n| {
                    if gesture == "draw" {
                        n.pitch == original_note.pitch + 1
                    } else {
                        n.id == note_id
                    }
                })
                .unwrap();
            let ticks = if gesture == "end" {
                result.start_tick + result.length_ticks
            } else {
                result.start_tick
            };
            let source_frame = app.session.project.midi_frame(ticks);
            let edge = original_clip.start_frame
                + original_clip
                    .stretch
                    .map_or(source_frame, |s| s.scale(source_frame).unwrap());
            assert!(
                edge.abs_diff(target) <= 19,
                "{gesture} / {stretched}: {edge} != {target}"
            );
            if gesture == "draw" {
                let length = app.session.project.midi_frame(result.length_ticks);
                assert_eq!(
                    original_clip
                        .stretch
                        .map_or(length, |s| s.scale(length).unwrap()),
                    3000
                );
            }
        }
    }
}

#[test]
fn oscillator_rows_share_knob_layout_and_edit_the_selected_instrument() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track, _, _) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.set_view_mode(ViewMode::Instrument, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let shapes: Vec<_> = shapes
            .into_iter()
            .filter(|shape| {
                shape
                    .shape
                    .visual_bounding_rect()
                    .intersects(app.midi_editor.oscillator_panel_bounds)
            })
            .collect();
        let mut centers: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle)
                    if (circle.radius - (knobs::SIZE / 2.0 - 5.0)).abs() < 0.01
                        && app
                            .midi_editor
                            .oscillator_panel_bounds
                            .contains(circle.center)
                        && !app
                            .midi_editor
                            .oscillator_power_bounds
                            .iter()
                            .any(|rect| rect.contains(circle.center))
                        && !app
                            .midi_editor
                            .add_oscillator_bounds
                            .contains(circle.center) =>
                {
                    Some(circle.center)
                }
                _ => None,
            })
            .collect();
        centers.sort_by(|a, b| a.y.total_cmp(&b.y));
        assert_eq!(centers.len(), 3);
        for (index, label) in [(1, "0"), (2, "-12.0 dB")] {
            let reference = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        ((bounds.center().x - centers[index].x).abs() < 0.1)
                            .then_some((bounds, text))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing reference label: {label}"));
            assert!(reference.0.bottom() <= centers[index].y - 15.0 + 0.01);
            assert!(
                Rect::from_center_size(centers[index], Vec2::splat(knobs::CHOICE_SIZE))
                    .contains_rect(reference.0),
                "Label: {:?}, center: {:?}",
                reference.0,
                centers[index]
            );
            assert_eq!(reference.1.fallback_color, theme::palette(&ctx).text);
        }
        let endpoints: Vec<_> = ["−48", "+48"]
            .into_iter()
            .map(|label| {
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing transpose endpoint label: {label}"));
                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                assert!(
                    Rect::from_center_size(centers[1], Vec2::splat(knobs::CHOICE_SIZE))
                        .contains_rect(bounds)
                );
                assert!(bounds.top() > centers[1].y + 10.0);
                assert_eq!(text.fallback_color, theme::palette(&ctx).text);
                bounds
            })
            .collect();
        assert!(endpoints[0].right() < centers[1].x);
        assert!(endpoints[1].left() > centers[1].x);
        assert!((endpoints[0].top() - endpoints[1].top()).abs() < 0.1);
        // Octave marks follow the transpose scale, independently of its one-semitone steps.
        let octave_marks: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, stroke }
                    if ((points[0] - centers[1]).length() >= 10.9
                        && ((points[1] - centers[1]).length() - 14.0).abs() < 0.01) =>
                {
                    Some((points, stroke))
                }
                _ => None,
            })
            .collect();
        assert_eq!(octave_marks.len(), 9);
        let gain_marks: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, stroke }
                    if ((points[0] - centers[2]).length() >= 10.9
                        && ((points[1] - centers[2]).length() - 14.0).abs() < 0.01) =>
                {
                    Some((points, stroke))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            gain_marks.len(),
            13,
            "enable the optional gain scale in Oscillator"
        );
        let unity: Vec<_> = gain_marks
            .iter()
            .filter(|(points, _)| (points[1] - (centers[2] - Vec2::new(0.0, 14.0))).length() < 0.01)
            .collect();
        assert_eq!(unity.len(), 1);
        assert_eq!(unity[0].1.width, 1.5);
        assert_eq!(unity[0].1.color, theme::palette(&ctx).text);
        assert_eq!(
            gain_marks
                .iter()
                .filter(|(_, stroke)| stroke.width == 1.0
                    && stroke.color == theme::palette(&ctx).secondary)
                .count(),
            12
        );
        for (index, (points, stroke)) in octave_marks.iter().enumerate() {
            let angle = std::f32::consts::PI * (0.75 + index as f32 / 8.0 * 1.5);
            let expected = centers[1] + Vec2::angled(angle) * 14.0;
            assert!((points[1] - expected).length() < 0.01);
            let zero = index == 4;
            assert_eq!(stroke.width, if zero { 1.5 } else { 1.0 });
            assert_eq!(
                stroke.color,
                if zero {
                    theme::palette(&ctx).text
                } else {
                    theme::palette(&ctx).secondary
                }
            );
        }
        let diagrams: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Path(path) if (path.stroke.width - 1.25).abs() < 0.01 => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(diagrams.len(), 4, "show one diagram for each waveform");
        for (index, diagram) in diagrams.iter().enumerate() {
            let bounds = Rect::from_points(&diagram.points);
            assert!(
                Rect::from_center_size(centers[0], Vec2::splat(knobs::CHOICE_SIZE))
                    .contains_rect(bounds)
            );
            let color = if index == 0 {
                theme::accent(&ctx)
            } else {
                theme::palette(&ctx).secondary
            };
            assert_eq!(diagram.stroke.color, egui::epaint::ColorMode::Solid(color));
        }
        assert_ne!(diagrams[1].points, diagrams[3].points);
        for center in &centers {
            assert!((center.x - centers[0].x).abs() < 0.1);
        }
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if matches!(text.galley.text(), "Waveform" | "Pitch" | "Gain" | "Sine" | " semitones" | "-12.0" | " dB")
        )), "Oscillator has no parameter-name labels or separate value column");
        for label in ["−∞", "+12"] {
            let bounds = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.galley.rect.translate(text.pos.to_vec2()))
                    }
                    _ => None,
                })
                .unwrap();
            assert!(
                Rect::from_center_size(centers[2], Vec2::splat(knobs::CHOICE_SIZE))
                    .contains_rect(bounds)
            );
            assert!(bounds.top() > centers[2].y + 10.0);
        }
        assert!(
            (centers[1].y - centers[0].y - knobs::CHOICE_SIZE - theme::TOOLBAR_PADDING as f32)
                .abs()
                < 0.1
        );
        assert!(
            (centers[2].y - centers[1].y - knobs::CHOICE_SIZE - theme::TOOLBAR_PADDING as f32)
                .abs()
                < 0.1
        );
        let assets = format!("{:?}", app.session.project.assets);
        for point in centers {
            click_note_or_lane(&mut app, &ctx, point);
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
        }
        let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(instrument.layers[0].oscillators[0].waveform, Waveform::Saw);
        assert_eq!(instrument.layers[0].oscillators[0].transpose, 1);
        assert!((instrument.layers[0].oscillators[0].gain_db + 11.9).abs() < 0.001);
        let updated = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for label in ["+1", "-11.9 dB"] {
            assert!(
                updated
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label))
            );
        }
        assert_eq!(format!("{:?}", app.session.project.assets), assets);
        assert_eq!(app.selected_track, Some(track));
        assert!(app.dirty);
    }
}

// Run identical arrangement actions through both view adapters. Keep these tests
// when changing clip_lane, snapping, resize, placement, or menu dispatch.
fn arrangement_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    mut events: Vec<egui::Event>,
    copy: bool,
) {
    let modifiers = egui::Modifiers {
        shift: true,
        alt: copy && cfg!(target_os = "macos"),
        ctrl: copy && !cfg!(target_os = "macos"),
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

#[test]
fn midi_clip_gestures_keep_timeline_editor_parity_and_cancel_without_changes() {
    for mode in [
        ClipDragMode::Move,
        ClipDragMode::TrimLeft,
        ClipDragMode::TrimRight,
        ClipDragMode::LoopLeft,
        ClipDragMode::LoopRight,
        ClipDragMode::StretchRight,
    ] {
        for copy in [false, true] {
            if copy && mode != ClipDragMode::Move {
                continue;
            }
            let mut outcomes = Vec::new();
            for view in [ViewMode::Timeline, ViewMode::Edit] {
                for cancel in [false, true] {
                    let (mut app, track, clip, _) = fixture();
                    app.session.project.tracks[0].clips[0].start_frame = 48_000;
                    let original = app.session.project.tracks[0].clips[0].clone();
                    let ctx = context();
                    if view.has_track_overview() {
                        app.set_view_mode(view, &ctx);
                    }
                    frame(&mut app, &ctx, vec![]);
                    if view.has_track_overview() {
                        app.midi_editor.zoom = 120.0;
                        app.midi_editor.scroll = 0.0;
                    } else {
                        app.zoom = 120.0;
                        app.scroll = 0.0;
                    }
                    frame(&mut app, &ctx, vec![]);
                    let lane = app.lane_bounds[&track];
                    let block = if view.has_track_overview() {
                        app.midi_editor.clip_bounds[&clip]
                    } else {
                        app.clip_block(lane, &original, original.start_frame)
                    };
                    let origin = match mode {
                        ClipDragMode::Move => block.min + Vec2::new(20.0, 10.0),
                        ClipDragMode::TrimLeft => block.min + Vec2::new(1.0, 30.0),
                        ClipDragMode::LoopLeft => block.min + Vec2::new(1.0, 10.0),
                        ClipDragMode::TrimRight => {
                            Pos2::new(block.right() - 1.0, block.top() + 30.0)
                        }
                        _ => Pos2::new(block.right() - 1.0, block.top() + 10.0),
                    };
                    let delta = if mode == ClipDragMode::Move {
                        300.0
                    } else if matches!(mode, ClipDragMode::TrimLeft | ClipDragMode::LoopLeft) {
                        -60.0
                    } else {
                        60.0
                    };
                    let end = origin + Vec2::new(delta, 0.0);
                    let before = format!("{:?}", app.session.project);
                    let modifier = copy || mode == ClipDragMode::StretchRight;
                    arrangement_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(origin), button(origin, true)],
                        modifier,
                    );
                    arrangement_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(end)],
                        modifier,
                    );
                    assert_eq!(app.drag.as_ref().map(|d| d.mode), Some(mode), "{view:?}");
                    assert_eq!(format!("{:?}", app.session.project), before);
                    assert!(!app.file_action_enabled(FileAction::Save));
                    if cancel {
                        arrangement_frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::Key {
                                key: egui::Key::Escape,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: Default::default(),
                            }],
                            modifier,
                        );
                    }
                    arrangement_frame(&mut app, &ctx, vec![button(end, false)], modifier);
                    finish_import(&mut app, &ctx);
                    assert!(app.drag.is_none());
                    assert!(app.error.is_none(), "{view:?}/{mode:?}: {:?}", app.error);
                    if cancel {
                        assert_eq!(format!("{:?}", app.session.project), before);
                        continue;
                    }
                    assert!(app.dirty, "{view:?}/{mode:?}/copy={copy}");
                    app.session.project.validate().unwrap();
                    let clips = &app.session.project.tracks[0].clips;
                    assert_eq!(clips.len(), if copy { 2 } else { 1 });
                    if copy {
                        assert_eq!(format!("{:?}", clips[0]), format!("{original:?}"));
                        assert_ne!(clips[1].id, original.id);
                    }
                    let result = clips.iter().max_by_key(|c| c.start_frame).unwrap();
                    let source = app
                        .session
                        .project
                        .assets
                        .iter()
                        .find(|a| a.id == result.asset_id)
                        .unwrap()
                        .midi_pattern
                        .as_ref()
                        .unwrap();
                    outcomes.push((
                        result.start_frame,
                        result.length_frames,
                        result.source_offset_frame,
                        result.repeat,
                        result.stretch,
                        source.length_ticks,
                        source.notes[0].start_tick,
                    ));
                    if mode == ClipDragMode::TrimLeft {
                        assert_eq!(result.start_frame, 24_000);
                        assert_eq!(result.end(), original.end());
                        assert_eq!(
                            result.start_frame
                                + app.session.project.midi_frame(source.notes[0].start_tick),
                            72_000
                        );
                    }
                    if mode == ClipDragMode::TrimRight {
                        assert_eq!(source.length_ticks, 4800);
                    }
                    if mode == ClipDragMode::Move {
                        assert_eq!(result.start_frame, 168_000);
                        assert_eq!(source.length_ticks, 3840);
                    }
                    let clip_count = clips.len();
                    let other = if view == ViewMode::Timeline {
                        ViewMode::Edit
                    } else {
                        ViewMode::Timeline
                    };
                    app.set_view_mode(other, &ctx);
                    frame(&mut app, &ctx, vec![]);
                    assert_eq!(app.session.project.tracks[0].clips.len(), clip_count);
                }
            }
            assert!(
                outcomes.windows(2).all(|pair| pair[0] == pair[1]),
                "{mode:?}/copy={copy}"
            );
        }
    }
}

#[test]
fn empty_midi_editor_lane_creation_and_cross_view_clipboard_use_shared_actions() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let mut app = DawUi::default();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        app.perform_track_action(TrackAction::AddMidi, &ctx);
        let track = app.selected_track.unwrap();
        app.set_view_mode(view, &ctx);
        frame(&mut app, &ctx, vec![]);
        if view.has_track_overview() {
            app.midi_editor.zoom = 120.0;
            app.midi_editor.scroll = 0.5;
        } else {
            app.zoom = 120.0;
            app.scroll = 0.5;
        }
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let point = Pos2::new(lane.left() + 120.0, lane.center().y);
        click_note_or_lane(&mut app, &ctx, point);
        assert!(app.session.project.tracks[0].clips.is_empty());
        click_note_or_lane(&mut app, &ctx, point);
        assert_eq!(app.view_mode, view);
        let clip = app.session.project.tracks[0].clips[0].clone();
        assert_eq!((clip.start_frame, clip.length_frames), (72_000, 24_000));
        assert_eq!(app.selected_clip, Some(clip.id));
        assert!(app.clip_action_enabled(ClipAction::Copy, &ctx));
        app.perform_clip_action(ClipAction::Copy, &ctx);
        let other = if view == ViewMode::Timeline {
            ViewMode::Edit
        } else {
            ViewMode::Timeline
        };
        app.set_view_mode(other, &ctx);
        app.session.project.transport.playhead_frame = 144_000;
        frame(&mut app, &ctx, vec![]);
        assert!(app.clip_action_enabled(ClipAction::Paste, &ctx));
        app.perform_clip_action(ClipAction::Paste, &ctx);
        assert_eq!(app.session.project.tracks[0].clips.len(), 2);
        let pasted = &app.session.project.tracks[0].clips[1];
        assert_ne!(pasted.id, clip.id);
        assert_eq!(
            (pasted.start_frame, pasted.asset_id),
            (144_000, clip.asset_id)
        );
        app.perform_track_action(TrackAction::Duplicate, &ctx);
        assert_eq!(app.session.project.tracks.len(), 2);
        assert!(app.session.project.tracks[1].clips.is_empty());
        assert_eq!(
            app.session.project.tracks[0].instrument,
            app.session.project.tracks[1].instrument
        );
    }
}

#[test]
fn midi_clip_context_menus_split_and_delete_in_both_views() {
    for view in [ViewMode::Timeline, ViewMode::Edit] {
        let (mut app, track, clip, _) = fixture();
        let ctx = context();
        if view == ViewMode::Edit {
            app.open_clip_editor(clip, track, &ctx);
        }
        frame(&mut app, &ctx, vec![]);
        app.session.project.transport.playhead_frame = 48_000;
        let asset = app.session.project.tracks[0].clips[0].asset_id;
        let click_menu = |app: &mut DawUi, clip_id: Id, label: &str| {
            frame(app, &ctx, vec![]);
            let current = app
                .session
                .project
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == clip_id)
                .unwrap()
                .clone();
            let lane = app.lane_bounds[&track];
            let block = if view == ViewMode::Edit {
                app.midi_editor.clip_bounds[&current.id]
            } else {
                app.clip_block(lane, &current, current.start_frame)
            };
            let point = block.center();
            for pressed in [true, false] {
                frame(
                    app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Secondary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            let shapes = frame_shapes(app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let item = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("shared clip context action {label}"));
            click_note_or_lane(app, &ctx, item);
            assert!(app.error.is_none(), "{:?}", app.error);
        };
        click_menu(&mut app, clip, "Split at playhead");
        assert_eq!(app.session.project.tracks[0].clips.len(), 2);
        let right = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|candidate| candidate.id != clip)
            .unwrap()
            .id;
        app.select_clip(right, track, false);
        click_menu(&mut app, right, "Delete clip");
        let clips = &app.session.project.tracks[0].clips;
        assert_eq!(clips.len(), 1);
        assert_eq!(
            (clips[0].id, clips[0].length_frames, clips[0].asset_id),
            (clip, 48_000, asset)
        );
        assert_eq!(app.view_mode, view);
    }
}

#[test]
fn layer_collapse_keeps_two_rows_track_height_and_restores_saved_presentation() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, mut layers) = layer_header_fixture();
        layers[0].oscillators.resize_with(12, Default::default);
        layers.push(daw_core::InstrumentLayerSettings {
            name: "Envelope only".into(),
            envelopes: vec![daw_core::EnvelopeInstance::default()],
            ..Default::default()
        });
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers = layers.clone();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 1400.0));
        let expanded = app.session.project.clone();
        let widths: Vec<_> = layers
            .iter()
            .map(|layer| {
                let bounds = &app.midi_editor.layer_bounds[&layer.id];
                (
                    bounds.oscillator_panel.width(),
                    bounds.envelope_panel.width(),
                )
            })
            .collect();
        for layer in &layers {
            let bounds = &app.midi_editor.layer_bounds[&layer.id];
            assert_eq!(bounds.collapse.left() - bounds.mix_controls[1].right(), 6.0);
            assert_eq!(bounds.label.right() - bounds.collapse.right(), 7.0);
            assert_eq!(bounds.collapse.top() - bounds.label.top(), 7.0);
            assert!(bounds.label.contains_rect(bounds.collapse));
            assert_eq!(
                bounds.collapse.width(),
                app.midi_editor.add_layer_bounds.width()
            );
            let point = bounds.collapse.center();
            app.dirty = false;
            frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
                Vec2::new(1280.0, 1400.0),
            );
            assert!(
                !app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers
                    .iter()
                    .find(|l| l.id == layer.id)
                    .unwrap()
                    .collapsed
            );
            assert!(!app.dirty);
            frame_shapes(
                &mut app,
                &ctx,
                vec![button(point, false)],
                Vec2::new(1280.0, 1400.0),
            );
            assert!(app.dirty);
            frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 1400.0));
        }
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let mut expected = expanded.clone();
        for layer in &mut expected.tracks[0].instrument.as_mut().unwrap().layers {
            layer.collapsed = true;
        }
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        for (index, layer) in layers.iter().enumerate() {
            let bounds = &app.midi_editor.layer_bounds[&layer.id];
            assert_eq!(bounds.row.height(), ROW_HEIGHT);
            assert_eq!(bounds.label.height(), ROW_HEIGHT);
            assert_eq!(
                bounds.collapse.width(),
                app.midi_editor.add_layer_bounds.width()
            );
            assert_eq!(bounds.collapse.left() - bounds.mix_controls[1].right(), 6.0);
            assert_eq!(bounds.label.right() - bounds.collapse.right(), 7.0);
            assert_eq!(bounds.collapse.top() - bounds.label.top(), 7.0);
            assert!(bounds.label.contains_rect(bounds.collapse));
            assert_eq!(bounds.oscillator_panel.width(), widths[index].0);
            assert_eq!(bounds.envelope_panel.width(), widths[index].1);
            for panel in [bounds.oscillator_panel, bounds.envelope_panel] {
                if panel != Rect::NOTHING {
                    assert_eq!(panel.height(), ROW_HEIGHT);
                }
            }
            for strip in bounds
                .oscillators
                .iter()
                .chain(&bounds.envelope_stages)
                .filter(|r| **r != Rect::NOTHING)
            {
                assert!(strip.height() < knobs::CHOICE_SIZE);
                assert!(
                    bounds.row.y_range().contains(strip.top())
                        && bounds.row.y_range().contains(strip.bottom())
                );
            }
            if index > 0 {
                assert_eq!(
                    app.midi_editor.layer_bounds[&layers[index - 1].id]
                        .row
                        .bottom()
                        - 1.0,
                    bounds.row.top()
                );
            }
        }
        for label in ["Osc 1", "Attack", "Decay", "Sustain", "Release"] {
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label)),
                "{label}"
            );
        }
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "5.0 ms" || t.galley.text() == "No source")));
        for label in ["No source", "No envelope", "Add source", "Add envelope"] {
            assert!(!shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Text(t) if t.galley.text() == label)));
        }
        // Compact footers and catalog additions do not resize the layer.
        let source_power = app.midi_editor.layer_bounds[&layers[0].id].source_powers[0].center();
        click_note_or_lane(&mut app, &ctx, source_power);
        frame(&mut app, &ctx, vec![]);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0]
                .oscillators[0]
                .muted
        );
        let empty = layers[2].id;
        for component in [0, 1] {
            super::instrument_catalog::add_component(&mut app, &ctx, empty, component);
            assert_eq!(
                app.midi_editor.layer_bounds[&empty].row.height(),
                ROW_HEIGHT
            );
        }
        let added = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[2];
        assert_eq!(added.oscillators.len(), 1);
        assert!(added.envelopes[0].present);
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let mut reopened = DawUi::default();
        reopened.session.project = serde_json::from_slice(&saved).unwrap();
        reopened.selected_track = Some(expanded.tracks[0].id);
        let fresh = context();
        reopened.set_view_mode(ViewMode::Instrument, &fresh);
        frame(&mut reopened, &fresh, vec![]);
        assert!(!reopened.dirty);
        for layer in &layers {
            assert_eq!(
                reopened.midi_editor.layer_bounds[&layer.id].row.height(),
                ROW_HEIGHT
            );
        }
        let point = reopened.midi_editor.layer_bounds[&layers[0].id]
            .collapse
            .center();
        click_note_or_lane(&mut reopened, &fresh, point);
        frame(&mut reopened, &fresh, vec![]);
        assert!(
            reopened.midi_editor.layer_bounds[&layers[0].id]
                .row
                .height()
                > ROW_HEIGHT
        );
        assert_eq!(
            reopened.midi_editor.layer_bounds[&layers[1].id]
                .row
                .height(),
            ROW_HEIGHT
        );
    }
}

#[test]
fn compact_layer_collapse_is_guarded_and_preserves_independent_unit_states_and_order() {
    let (mut app, ctx, mut layers) = layer_header_fixture();
    layers[0].oscillator_collapsed = true;
    layers[0].envelopes[0].collapsed = true;
    layers[0].collapsed = true;
    layers[2].collapsed = true;
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers = layers.clone();
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let first = layers[0].id;
    let point = app.midi_editor.layer_bounds[&first].collapse.center();
    for disabled in [false, true] {
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: disabled,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    events: vec![egui::Event::PointerMoved(point), button(point, pressed)],
                    ..Default::default()
                },
                |ui| {
                    if disabled {
                        ui.disable();
                    }
                    app.show(ui);
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers,
            layers
        );
        assert!(!app.dirty);
    }
    frame(&mut app, &ctx, vec![]);
    let bounds = &app.midi_editor.layer_bounds[&first];
    let panels = [
        (bounds.oscillator_panel, "Osc..."),
        (bounds.envelope_panel, "Env..."),
    ];
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for (panel, name) in panels {
        assert_eq!(
            panel.size(),
            Vec2::new(panels::HEADER_HEIGHT + 2.0, ROW_HEIGHT)
        );
        let text = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == name => Some(text),
                _ => None,
            })
            .unwrap();
        assert_eq!(text.angle, -std::f32::consts::FRAC_PI_2);
        let body = Rect::from_min_max(
            panel.min + Vec2::new(1.0, panels::HEADER_HEIGHT + 1.0),
            panel.max - Vec2::splat(1.0),
        );
        assert!(body.contains_rect(text.visual_bounding_rect()));
    }
    let bounds = &app.midi_editor.layer_bounds[&first];
    // Reordering mixed heights uses actual row bounds and keeps saved states with IDs.
    let start = bounds.icon.center();
    let end = app.midi_editor.layer_bounds[&layers[2].id].row.center() + Vec2::new(0.0, 8.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.instrument_ui.dragging());
    frame(&mut app, &ctx, vec![button(end, false)]);
    frame(&mut app, &ctx, vec![]);
    let expected = vec![layers[1].clone(), layers[2].clone(), layers[0].clone()];
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers,
        expected
    );
    let point = app.midi_editor.layer_bounds[&first].collapse.center();
    click_note_or_lane(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    let bounds = &app.midi_editor.layer_bounds[&first];
    assert!(bounds.row.height() > ROW_HEIGHT);
    assert_eq!(bounds.oscillator_panel.width(), panels::HEADER_HEIGHT + 2.0);
    assert_eq!(bounds.envelope_panel.width(), panels::HEADER_HEIGHT + 2.0);
    let track = app.session.project.tracks[0].id;
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.midi_editor.layer_bounds[&layers[2].id].row.height(),
        ROW_HEIGHT
    );
    app.select_track(track, false);
    begin_layer_rename(&mut app, &ctx, first);
    app.dirty = false;
    let point = app.midi_editor.layer_bounds[&first].collapse.center();
    click_note_or_lane(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    assert!(
        !app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[2]
            .collapsed
    );
    assert!(!app.dirty);
}
