use super::*;
use daw_core::{ArrangementMarker, TempoEvent};

fn expanded_fixture() -> (DawUi, Id) {
    let (mut app, track) = fixture();
    app.arrangement.collapsed = false;
    (app, track)
}

#[test]
fn track_toolbar_collapses_arrangement_without_leaving_a_row() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    assert_eq!(app.lane_bounds[&track].top(), ruler.bottom());
    let open_top = ruler.bottom() + 3.0 * beat_editor::SUBTRACK_LANE_HEIGHT;
    let project = app.session.project.clone();
    let toggle = Pos2::new(
        ruler.left() - f32::from(theme::TOOLBAR_PADDING) - 14.0,
        ruler.center().y,
    );
    for expected_top in [open_top, ruler.bottom()] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(toggle), button(toggle, true)],
        );
        frame(&mut app, &ctx, vec![button(toggle, false)]);
        assert_eq!(app.lane_bounds[&track].top(), expected_top);
        assert_eq!(app.arrangement.ruler_bounds, ruler);
        assert_eq!(app.session.project, project);
        assert!(!app.dirty);
    }
}

#[test]
fn lane_headers_use_semibold_labels_and_offer_only_visibility_actions() {
    let size = Vec2::new(1280.0, 800.0);
    let names = ["Markers", "Tempo", "Signature"];
    for lane in 0..names.len() {
        let (mut app, track) = expanded_fixture();
        let ctx = context();
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        for name in names {
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == name
                    && text.galley.job.sections[0].format.font_id == fonts::semibold(12.0))));
        }
        let ruler = app.arrangement.ruler_bounds;
        let point = Pos2::new(
            ruler.left() - TRACK_WIDTH + 25.0,
            ruler.bottom() + (lane as f32 + 0.5) * beat_editor::SUBTRACK_LANE_HEIGHT,
        );
        let track_top = app.lane_bounds[&track].top();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(!app.arrangement.collapsed);
        assert_eq!(app.lane_bounds[&track].top(), track_top);

        let secondary = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut app, &ctx, vec![secondary(true)]);
        frame(&mut app, &ctx, vec![secondary(false)]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        assert!(egui::Popup::is_any_open(&ctx));
        let labels: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect();
        for name in names {
            assert_eq!(labels.iter().filter(|&&label| label == name).count(), 2);
        }
        for removed in [
            "Collapse lanes",
            "Previous marker",
            "Next marker",
            "Add section at playhead",
            "Add tempo point at playhead",
            "Add time signature at playhead",
        ] {
            assert!(!labels.contains(&removed));
        }
        assert!(!app.dirty);
    }
}

#[test]
fn metadata_lanes_follow_the_ruler_and_marker_drags_commit_or_cancel() {
    let (mut app, track) = expanded_fixture();
    let marker = Id::new_v4();
    app.session.project.markers = vec![ArrangementMarker {
        id: marker,
        start_frame: frames(1.0),
        name: "Verse".into(),
        length_frames: Some(96_000),
    }];
    app.session.project.transport.playhead_frame = frames(2.0);
    let ctx = context();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let ruler = app.arrangement.ruler_bounds;
    assert_eq!(
        theme::arrangement_block_fill(&ctx),
        egui::Color32::from_gray(43)
    );
    assert_eq!(ruler.bottom(), app.effects_bounds.header.bottom());
    assert!(shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::LineSegment{points,stroke} if *stroke==Stroke::new(1.0,theme::palette(&ctx).border)&&points[0].x==ruler.left()-TRACK_WIDTH&&points[1].x==ruler.right()&&points[0].y==app.effects_bounds.header.bottom()-0.5&&points[1].y==points[0].y)));
    let playhead_x = ruler.left() + 2.0 * app.zoom;
    let marker_fill = shapes
        .iter()
        .position(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == theme::arrangement_block_fill(&ctx)
                    && rect.rect.x_range().contains(playhead_x)
                    && rect.rect.top() == ruler.bottom()
                    && rect.rect.bottom() == ruler.bottom() + beat_editor::SUBTRACK_LANE_HEIGHT - 1.0)
        })
        .expect("marker fill under the playhead");
    assert!(shapes[marker_fill + 1..].iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::LineSegment { points, stroke }
            if *stroke == Stroke::new(1.0, theme::accent(&ctx))
                && points[0] == Pos2::new(playhead_x, ruler.bottom())
                && points[1] == Pos2::new(playhead_x, app.timeline_bounds.bottom()))
    }));
    assert_eq!(
        app.lane_bounds[&track].top() - ruler.bottom(),
        3.0 * beat_editor::SUBTRACK_LANE_HEIGHT
    );
    let origin = Pos2::new(ruler.left() + app.zoom + 12.0, ruler.bottom() + 12.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    let end = origin + Vec2::new(app.zoom, 0.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.session.project.markers[0].start_frame, frames(1.0));
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project.markers[0].start_frame, frames(2.0));
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.markers[0].start_frame, frames(1.0));
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
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
    assert_eq!(app.session.project.markers[0].start_frame, frames(1.0));
}
#[test]
fn mapped_ruler_and_editor_grid_share_frame_geometry_and_active_transport_values() {
    let mut app = DawUi::default();
    let mut events = app.session.project.tempo_events();
    events.push(TempoEvent {
        id: Id::new_v4(),
        start_frame: 48_000,
        bpm: 60.0,
    });
    app.session
        .project
        .edit(Edit::SetTempoEvents { events })
        .unwrap();
    app.session.project.transport.playhead_frame = 96_000;
    let timeline = app.musical_timeline();
    assert!((timeline.pixel_at_frame(96_000) - 140.0).abs() < 0.001);
    assert_eq!(app.position_text(96_000), "0001.04");
    app.set_tempo(90.0);
    assert_eq!(app.session.project.tempo_bpm, 120.0);
    assert_eq!(app.session.project.timing.tempos[1].bpm, 90.0);
}

#[test]
fn new_sections_have_default_names_and_rename_inline_with_commit_and_cancel() {
    fn double_click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
        for _ in 0..2 {
            frame(
                app,
                ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(app, ctx, vec![button(point, false)]);
        }
        frame(app, ctx, vec![]);
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }
    let (mut app, _) = expanded_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    let point = Pos2::new(ruler.left() + 70.0, ruler.bottom() + 16.0);
    double_click(&mut app, &ctx, point);
    assert_eq!(app.session.project.markers.len(), 1);
    assert_eq!(app.session.project.markers[0].name, "New section");
    assert_eq!(app.session.project.markers[0].length_frames, Some(96_000));
    assert!(!ctx.text_edit_focused());
    let marker = app.session.project.markers[0].clone();
    let tracks = app.session.project.tracks.clone();
    for _ in 0..60 {
        frame(&mut app, &ctx, vec![]);
    }
    double_click(&mut app, &ctx, point + Vec2::new(10.0, 0.0));
    assert!(ctx.text_edit_focused());
    frame(&mut app, &ctx, vec![egui::Event::Text("Verse 🎹".into())]);
    assert_eq!(
        app.session.project.markers[0], marker,
        "Typing only changes the draft"
    );
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    assert_eq!(app.session.project.markers[0].id, marker.id);
    assert_eq!(
        app.session.project.markers[0].start_frame,
        marker.start_frame
    );
    assert_eq!(app.session.project.markers[0].name, "Verse 🎹");
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.markers[0], marker);
    for _ in 0..60 {
        frame(&mut app, &ctx, vec![]);
    }
    double_click(&mut app, &ctx, point + Vec2::new(10.0, 0.0));
    assert!(ctx.text_edit_focused());
    frame(&mut app, &ctx, vec![egui::Event::Text("Discard".into())]);
    frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
    frame(&mut app, &ctx, vec![]);
    assert!(!ctx.text_edit_focused());
    assert_eq!(app.session.project.markers[0], marker);
    assert_eq!(app.session.project.tracks, tracks);
}

#[test]
fn section_edges_resize_on_release_and_cancel_without_changing_tracks() {
    let (mut app, _) = expanded_fixture();
    let marker = ArrangementMarker {
        id: Id::new_v4(),
        start_frame: 48_000,
        name: "Verse".into(),
        length_frames: Some(96_000),
    };
    app.session.project.markers = vec![marker.clone()];
    let tracks = app.session.project.tracks.clone();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    let y = ruler.bottom() + 16.0;
    let right = Pos2::new(ruler.left() + 3.0 * app.zoom - 2.0, y);
    let end = right + Vec2::new(app.zoom, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(right), button(right, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.session.project.markers[0], marker);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.markers[0].start_frame,
        marker.start_frame
    );
    assert_eq!(app.session.project.markers[0].length_frames, Some(144_000));
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    let left = Pos2::new(ruler.left() + app.zoom + 2.0, y);
    let end = left + Vec2::new(app.zoom * 0.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(left), button(left, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.session.project.markers[0], marker);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project.markers[0].start_frame, 72_000);
    assert_eq!(app.session.project.markers[0].length_frames, Some(72_000));
    assert_eq!(app.session.project.markers[0].id, marker.id);
    assert_eq!(app.session.project.markers[0].name, marker.name);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(right), button(right, true)],
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
    assert_eq!(app.session.project.markers[0], marker);
    assert_eq!(app.session.project.tracks, tracks);
}

#[test]
fn signature_edges_move_shared_boundaries_and_keep_origin_fixed() {
    use daw_core::{SignatureEvent, TimeSignature};
    let (mut app, _) = expanded_fixture();
    let mut events = app.session.project.signature_events();
    events.push(SignatureEvent {
        id: Id::new_v4(),
        start_frame: 96_000,
        signature: TimeSignature {
            numerator: 3,
            denominator: 4,
        },
    });
    events.push(SignatureEvent {
        id: Id::new_v4(),
        start_frame: 192_000,
        signature: TimeSignature::default(),
    });
    app.session
        .project
        .edit(Edit::SetSignatureEvents {
            events: events.clone(),
        })
        .unwrap();
    let tracks = app.session.project.tracks.clone();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    let y = ruler.bottom() + 2.0 * beat_editor::SUBTRACK_LANE_HEIGHT + 16.0;
    // The origin block's right edge moves the following change.
    let right = Pos2::new(ruler.left() + 2.0 * app.zoom - 2.0, y);
    let end = right + Vec2::new(app.zoom * 0.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(right), button(right, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.session.project.signature_events(), events);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.signature_events()[1].start_frame,
        120_000
    );
    assert_eq!(app.session.project.signature_events()[0].id, events[0].id);
    assert_eq!(app.session.project.signature_events()[0].start_frame, 0);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    let left = Pos2::new(ruler.left() + 2.0 * app.zoom + 2.0, y);
    let end = left - Vec2::new(app.zoom * 0.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(left), button(left, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.signature_events()[1].start_frame,
        72_000
    );
    assert_eq!(app.session.project.signature_events()[0].id, events[0].id);
    assert_eq!(app.session.project.signature_events()[0].start_frame, 0);
    assert_eq!(app.session.project.signature_events()[2].id, events[2].id);
    assert_eq!(
        app.session.project.signature_events()[2].start_frame,
        events[2].start_frame
    );
    assert_eq!(app.session.project.tracks, tracks);
}

#[test]
fn section_selection_reuses_unit_header_typography_shift_and_empty_space_rules() {
    let (mut app, _) = expanded_fixture();
    let originals: Vec<_> = [(48_000, "Alpha"), (144_000, "Beta")]
        .into_iter()
        .map(|(start_frame, name)| ArrangementMarker {
            id: Id::new_v4(),
            start_frame,
            length_frames: Some(48_000),
            name: name.into(),
        })
        .collect();
    app.session.project.markers = originals.clone();
    app.dirty = false;
    let ctx = context();
    let draw = |app: &mut DawUi| frame_shapes(app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let check = |shapes: &[egui::epaint::ClippedShape], name: &str, selected| {
        let text = shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == name).then_some(text)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            text.galley.job.sections[0].format.font_id,
            fonts::semibold(panels::HEADER_FONT_SIZE)
        );
        assert_eq!(
            text.galley.job.sections[0].format.color,
            panels::unit_header_text(&ctx, selected)
        );
    };
    let shapes = draw(&mut app);
    check(&shapes, "Alpha", false);
    check(&shapes, "Beta", false);
    let ruler = app.arrangement.ruler_bounds;
    let alpha = Pos2::new(ruler.left() + app.zoom * 1.5, ruler.bottom() + 16.0);
    let beta = Pos2::new(ruler.left() + app.zoom * 3.5, alpha.y);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(alpha), button(alpha, true)],
    );
    frame(&mut app, &ctx, vec![button(alpha, false)]);
    let shapes = draw(&mut app);
    check(&shapes, "Alpha", true);
    check(&shapes, "Beta", false);
    assert!(!app.dirty);
    assert_eq!(app.session.project.transport.playhead_frame, 0);
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(shift),
                egui::Event::PointerMoved(beta),
                egui::Event::PointerButton {
                    pos: beta,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: shift,
                },
            ],
        );
    }
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::ModifiersChanged(Default::default())],
    );
    let shapes = draw(&mut app);
    check(&shapes, "Alpha", true);
    check(&shapes, "Beta", true);
    assert!(!shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.stroke == theme::selection_outline(&ctx) && r.rect.top() >= ruler.bottom() && r.rect.bottom() <= ruler.bottom()+32.0)));
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
    assert!(app.session.project.markers.is_empty());
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.markers, originals);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(alpha), button(alpha, true)],
    );
    frame(&mut app, &ctx, vec![button(alpha, false)]);
    let empty = Pos2::new(ruler.left() + app.zoom * 6.0, alpha.y);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(empty), button(empty, true)],
    );
    frame(&mut app, &ctx, vec![button(empty, false)]);
    let shapes = draw(&mut app);
    check(&shapes, "Alpha", false);
    check(&shapes, "Beta", false);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(alpha), button(alpha, true)],
    );
    frame(&mut app, &ctx, vec![button(alpha, false)]);
    app.set_view_mode(ViewMode::Edit, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    let shapes = draw(&mut app);
    check(&shapes, "Alpha", false);
}

#[test]
fn section_shared_boundary_can_remove_its_owner_and_ordinary_edges_overwrite() {
    let (mut app, _) = expanded_fixture();
    let originals: Vec<_> = [48_000, 96_000, 144_000]
        .into_iter()
        .map(|start_frame| ArrangementMarker {
            id: Id::new_v4(),
            start_frame,
            length_frames: Some(48_000),
            name: format!("Section {start_frame}"),
        })
        .collect();
    app.session.project.markers = originals.clone();
    let tracks = app.session.project.tracks.clone();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    let edge = Pos2::new(ruler.left() + app.zoom * 2.0 - 1.0, ruler.bottom() + 16.0);
    let end = edge - Vec2::new(app.zoom * 1.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.markers, originals);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project.markers.len(), 2);
    assert_eq!(app.session.project.markers[0].id, originals[1].id);
    assert_eq!(app.session.project.markers[0].start_frame, 48_000);
    assert_eq!(app.session.project.markers[0].length_frames, Some(96_000));
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    let edge = Pos2::new(ruler.left() + app.zoom * 2.0 - 6.0, edge.y);
    let end = edge + Vec2::new(app.zoom * 2.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.markers, originals);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.session.project.markers.len(), 1);
    assert_eq!(app.session.project.markers[0].id, originals[0].id);
    assert_eq!(app.session.project.markers[0].length_frames, Some(144_000));
    assert_eq!(app.session.project.tracks, tracks);
}

#[test]
fn signature_spans_remain_continuous_through_resize_and_the_final_value_fills_the_lane() {
    use daw_core::{SignatureEvent, TimeSignature};
    let (mut app, _) = expanded_fixture();
    let mut events = app.session.project.signature_events();
    for (start_frame, numerator) in [(96_000, 3), (192_000, 5)] {
        events.push(SignatureEvent {
            id: Id::new_v4(),
            start_frame,
            signature: TimeSignature {
                numerator,
                denominator: 4,
            },
        });
    }
    app.session
        .project
        .edit(Edit::SetSignatureEvents {
            events: events.clone(),
        })
        .unwrap();
    let ctx = context();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let ruler = app.arrangement.ruler_bounds;
    let top = ruler.bottom() + 2.0 * beat_editor::SUBTRACK_LANE_HEIGHT;
    let fills_end = |shapes: &[egui::epaint::ClippedShape]| {
        shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(r) if r.rect.top()==top && r.rect.right()==ruler.right()-1.0 && r.fill==theme::arrangement_block_fill(&ctx)))
    };
    assert!(fills_end(&shapes));
    let bpm = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "120 BPM" => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        bpm.galley.job.sections[0].format.font_id,
        egui::TextStyle::Small.resolve(&ctx.global_style())
    );
    let edge = Pos2::new(ruler.left() + 2.0 * app.zoom - 6.0, top + 15.0);
    let end = edge + Vec2::new(app.zoom * 2.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let result = app.session.project.signature_events();
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].start_frame, 192_000);
    assert_eq!(result[1].id, events[2].id);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(fills_end(&shapes));
    // Ordinary shrinking pulls the following change back instead of leaving a gap.
    let edge = Pos2::new(ruler.left() + 4.0 * app.zoom - 6.0, edge.y);
    let end = edge - Vec2::new(app.zoom, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.signature_events()[1].start_frame,
        144_000
    );
    assert_eq!(
        app.session
            .project
            .musical_clock()
            .unwrap()
            .signature_at(400_000)
            .numerator,
        5
    );
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(fills_end(&shapes));
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.signature_events(), result);
}

fn tempo_point(app: &mut DawUi, ctx: &egui::Context, start_frame: u64) -> Pos2 {
    let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
    let ruler = app.arrangement.ruler_bounds;
    let x = ruler.left() + app.musical_timeline().pixel_at_frame(start_frame) as f32;
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle)
                if circle.radius == 3.0
                    && (circle.center.x - x).abs() < 0.1
                    && circle.center.y > ruler.bottom() + 32.0
                    && circle.center.y < ruler.bottom() + 64.0 =>
            {
                Some(circle.center)
            }
            _ => None,
        })
        .unwrap()
}
fn tempo_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}
#[test]
fn tempo_double_click_creates_points_without_dialog_and_delete_targets_the_clicked_point() {
    let (mut app, _) = expanded_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let tracks = app.session.project.tracks.clone();
    let ruler = app.arrangement.ruler_bounds;
    for seconds in [2.0, 3.0] {
        // Separate independent double-click sequences from egui's triple-click window.
        for _ in 0..60 {
            frame(&mut app, &ctx, vec![]);
        }
        let point = Pos2::new(ruler.left() + app.zoom * seconds, ruler.bottom() + 48.0);
        for _ in 0..2 {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(&mut app, &ctx, vec![button(point, false)]);
        }
        frame(&mut app, &ctx, vec![]);
        assert!(!ctx.text_edit_focused());
        assert!(app.error.is_none());
    }
    let events = app.session.project.tempo_events();
    assert_eq!(events.len(), 3);
    assert_eq!(events[1].start_frame, 96_000);
    assert_eq!(events[2].start_frame, 144_000);
    assert_eq!(events[1].bpm, 120.0);
    let point = tempo_point(&mut app, &ctx, 96_000);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    frame(&mut app, &ctx, vec![tempo_key(egui::Key::Delete)]);
    assert_eq!(
        app.session.project.tempo_events(),
        vec![events[0], events[2]]
    );
    assert_eq!(app.session.project.tracks, tracks);
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    frame(&mut app, &ctx, vec![tempo_key(egui::Key::Delete)]);
    assert_eq!(
        app.session.project.tracks, tracks,
        "Deleting an empty tempo selection must not delete clips"
    );
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tempo_events(), events);
    let origin = tempo_point(&mut app, &ctx, 0) + Vec2::new(2.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    frame(&mut app, &ctx, vec![button(origin, false)]);
    frame(&mut app, &ctx, vec![tempo_key(egui::Key::Delete)]);
    assert_eq!(app.session.project.tempo_events(), events);
    assert!(app.error.is_none());
}
#[test]
fn tempo_vertical_drag_previews_then_commits_and_allows_vertical_release_outside_the_row() {
    let (mut app, _) = expanded_fixture();
    let mut events = app.session.project.tempo_events();
    let id = Id::new_v4();
    events.push(TempoEvent {
        id,
        start_frame: 96_000,
        bpm: 120.0,
    });
    app.session
        .project
        .edit(Edit::SetTempoEvents {
            events: events.clone(),
        })
        .unwrap();
    let ctx = context();
    let point = tempo_point(&mut app, &ctx, 96_000);
    let up = point - Vec2::new(0.0, 10.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
    let preview = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(preview.iter().any(
        |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()=="130 BPM")
    ));
    assert_eq!(app.session.project.tempo_events(), events);
    let fine = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let fine_up = up - Vec2::new(0.0, 10.0);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(fine),
            egui::Event::PointerMoved(fine_up),
        ],
    );
    frame(&mut app, &ctx, vec![button(fine_up, false)]);
    let changed = app.session.project.tempo_events();
    assert_eq!(changed[1].id, id);
    assert_eq!(changed[1].start_frame, 96_000);
    assert_eq!(changed[1].bpm, 131.0);
    assert!(app.error.is_none());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::ModifiersChanged(Default::default())],
    );
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tempo_events(), events);
    let point = tempo_point(&mut app, &ctx, 96_000);
    let right = point + Vec2::new(app.zoom * 0.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
    frame(&mut app, &ctx, vec![button(right, false)]);
    assert_eq!(app.session.project.tempo_events()[1].start_frame, 120_000);
    assert_eq!(app.session.project.tempo_events()[1].bpm, 120.0);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    let point = tempo_point(&mut app, &ctx, 96_000);
    let down = point + Vec2::new(0.0, 40.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(down)]);
    assert_eq!(app.session.project.tempo_events(), events);
    frame(&mut app, &ctx, vec![button(down, false)]);
    assert_eq!(app.session.project.tempo_events()[1].bpm, 80.0);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    let point = tempo_point(&mut app, &ctx, 96_000);
    let up = point - Vec2::new(0.0, 8.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
    frame(&mut app, &ctx, vec![tempo_key(egui::Key::Escape)]);
    frame(&mut app, &ctx, vec![button(up, false)]);
    assert_eq!(app.session.project.tempo_events(), events);
    assert!(app.error.is_none());
}
#[test]
fn origin_tempo_point_changes_bpm_without_moving_and_stays_positive() {
    let mut app = DawUi::default();
    app.arrangement.collapsed = false;
    let ctx = context();
    let point = tempo_point(&mut app, &ctx, 0) + Vec2::new(2.0, 0.0);
    let up = point + Vec2::new(35.0, -10.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
    frame(&mut app, &ctx, vec![button(up, false)]);
    assert_eq!(app.session.project.tempo_events()[0].start_frame, 0);
    assert_eq!(app.session.project.tempo_bpm, 130.0);
    let point = tempo_point(&mut app, &ctx, 0) + Vec2::new(2.0, 0.0);
    let down = point + Vec2::new(0.0, 200.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(down)]);
    frame(&mut app, &ctx, vec![button(down, false)]);
    assert_eq!(app.session.project.tempo_bpm, 1.0);
    app.session.project.validate().unwrap();
    assert!(app.error.is_none());
}

#[test]
fn horizontal_tempo_point_drag_preserves_fractional_bpm_exactly() {
    let (mut app, _) = expanded_fixture();
    let mut events = app.session.project.tempo_events();
    events.push(TempoEvent {
        id: Id::new_v4(),
        start_frame: 96_000,
        bpm: 123.456,
    });
    app.session
        .project
        .edit(Edit::SetTempoEvents { events })
        .unwrap();
    let ctx = context();
    let point = tempo_point(&mut app, &ctx, 96_000);
    let right = point + Vec2::new(app.zoom * 0.5, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![button(right, false)]);
    // Musical snapping and map reprojection can round up by one frame.
    assert!(
        app.session.project.tempo_events()[1]
            .start_frame
            .abs_diff(120_000)
            <= 1
    );
    assert_eq!(app.session.project.tempo_events()[1].bpm, 123.456);
    assert!(app.error.is_none());
}

#[test]
fn tempo_direction_lock_ignores_perpendicular_jitter_on_both_axes() {
    for vertical in [true, false] {
        let (mut app, _) = expanded_fixture();
        let mut events = app.session.project.tempo_events();
        events.push(TempoEvent {
            id: Id::new_v4(),
            start_frame: 96_000,
            bpm: 120.0,
        });
        app.session
            .project
            .edit(Edit::SetTempoEvents {
                events: events.clone(),
            })
            .unwrap();
        let ctx = context();
        let point = tempo_point(&mut app, &ctx, 96_000);
        let end = point
            + if vertical {
                Vec2::new(10.0, -20.0)
            } else {
                Vec2::new(35.0, 10.0)
            };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.session.project.tempo_events(), events);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let changed = app.session.project.tempo_events()[1];
        assert_eq!(changed.start_frame, if vertical { 96_000 } else { 120_000 });
        assert_eq!(changed.bpm, if vertical { 140.0 } else { 120.0 });
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project.tempo_events(), events);
    }
}
#[test]
fn tempo_direction_lock_switches_after_deliberate_cross_axis_motion_without_replaying_jitter() {
    for vertical_first in [true, false] {
        let (mut app, _) = expanded_fixture();
        let mut events = app.session.project.tempo_events();
        events.push(TempoEvent {
            id: Id::new_v4(),
            start_frame: 96_000,
            bpm: 120.0,
        });
        app.session
            .project
            .edit(Edit::SetTempoEvents {
                events: events.clone(),
            })
            .unwrap();
        let ctx = context();
        let point = tempo_point(&mut app, &ctx, 96_000);
        let moves = if vertical_first {
            [
                Vec2::new(0.0, -10.0),
                Vec2::new(20.0, -10.0),
                Vec2::new(35.0, -9.0),
            ]
        } else {
            [
                Vec2::new(35.0, 0.0),
                Vec2::new(36.0, -20.0),
                Vec2::new(37.0, -25.0),
            ]
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        for delta in moves {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point + delta)],
            );
        }
        assert_eq!(app.session.project.tempo_events(), events);
        let end = point + moves[2];
        frame(&mut app, &ctx, vec![button(end, false)]);
        let changed = app.session.project.tempo_events()[1];
        if vertical_first {
            assert!(changed.start_frame > 96_000);
            assert_eq!(changed.bpm, 130.0);
        } else {
            assert_eq!(changed.start_frame, 120_000);
            assert_eq!(changed.bpm, 133.0);
        }
        assert!(app.error.is_none());
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project.tempo_events(), events);
    }
}

#[test]
fn lane_plus_buttons_add_at_the_playhead_without_collapsing_or_opening_dialogs() {
    for lane in 0..3 {
        let (mut app, track) = expanded_fixture();
        app.session.project.transport.playhead_frame = 48_000;
        if lane == 1 {
            let mut events = app.session.project.tempo_events();
            events.push(TempoEvent {
                id: Id::new_v4(),
                start_frame: 24_000,
                bpm: 90.0,
            });
            app.session
                .project
                .edit(Edit::SetTempoEvents { events })
                .unwrap();
        } else if lane == 2 {
            let mut events = app.session.project.signature_events();
            events.push(daw_core::SignatureEvent {
                id: Id::new_v4(),
                start_frame: 24_000,
                signature: daw_core::TimeSignature {
                    numerator: 3,
                    denominator: 4,
                },
            });
            app.session
                .project
                .edit(Edit::SetSignatureEvents { events })
                .unwrap();
        }
        let before = app.session.project.clone();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let ruler = app.arrangement.ruler_bounds;
        let point = Pos2::new(
            ruler.left() - 8.0 - icons::COMPACT_BUTTON_SIZE * 0.5,
            ruler.bottom() + beat_editor::SUBTRACK_LANE_HEIGHT * (lane as f32 + 0.5),
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(rect) if rect.rect.center()==point && rect.rect.size()==Vec2::splat(icons::COMPACT_BUTTON_SIZE))));
        let click = |app: &mut DawUi| {
            frame(
                app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(app, &ctx, vec![button(point, false)]);
            frame(app, &ctx, vec![]);
        };
        click(&mut app);
        assert!(!ctx.text_edit_focused());
        assert!(app.error.is_none());
        assert_eq!(
            app.lane_bounds[&track].top() - ruler.bottom(),
            3.0 * beat_editor::SUBTRACK_LANE_HEIGHT
        );
        match lane {
            0 => {
                assert_eq!(app.session.project.markers.len(), 1);
                assert_eq!(app.session.project.markers[0].start_frame, 48_000);
                assert_eq!(app.session.project.markers[0].name, "New section");
                assert_eq!(app.session.project.markers[0].length_frames, Some(96_000));
            }
            1 => {
                let events = app.session.project.tempo_events();
                assert_eq!(events.len(), 3);
                assert_eq!(events[2].start_frame, 48_000);
                assert_eq!(events[2].bpm, 90.0);
                click(&mut app);
                assert_eq!(app.session.project.tempo_events(), events);
            }
            _ => {
                let events = app.session.project.signature_events();
                assert_eq!(events.len(), 3);
                assert_eq!(events[2].start_frame, 48_000);
                assert_eq!(
                    events[2].signature,
                    daw_core::TimeSignature {
                        numerator: 3,
                        denominator: 4
                    }
                );
                click(&mut app);
                assert_eq!(app.session.project.signature_events(), events);
            }
        }
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
    }
}
#[test]
fn lane_plus_buttons_are_disabled_during_conflicting_jobs() {
    let (mut app, _) = expanded_fixture();
    app.session.project.transport.playhead_frame = 48_000;
    let before = app.session.project.clone();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let ruler = app.arrangement.ruler_bounds;
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    for lane in 0..3 {
        let point = Pos2::new(
            ruler.left() - 8.0 - icons::COMPACT_BUTTON_SIZE * 0.5,
            ruler.bottom() + beat_editor::SUBTRACK_LANE_HEIGHT * (lane as f32 + 0.5),
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.session.project, before);
    }
    drop(sender);
}
