use super::*;
use daw_core::{TrackKind, TrackSend};
#[test]
fn return_creation_pinning_selection_and_delete_undo_restore_routes() {
    for view in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, source) = fixture();
        let ctx = context();
        app.view_mode = view;
        frame(&mut app, &ctx, vec![]);
        app.perform_track_action(TrackAction::AddReturn, &ctx);
        let r = app.selected_track.unwrap();
        assert_eq!(
            app.session.project.tracks.last().unwrap().kind,
            TrackKind::Return
        );
        assert_eq!(
            app.effects_target(),
            Some(daw_core::EffectChainTarget::Track(r))
        );
        app.edit(Edit::SetTrackSends {
            track_id: source,
            sends: vec![TrackSend::new(r)],
        });
        frame(&mut app, &ctx, vec![]);
        if view == ViewMode::Timeline {
            assert!((app.lane_bounds[&r].bottom() - app.master_bounds.top()).abs() < 0.1);
            assert!(app.lane_bounds[&source].top() < app.lane_bounds[&r].top());
        } else {
            assert!(app.mixer_bounds[&r].right() <= app.master_bounds.left() + 0.1);
            assert!(app.mixer_bounds[&source].right() <= app.mixer_bounds[&r].left() + 0.1);
            let master = app.mixer_faders[&None];
            for id in [source, r] {
                let fader = app.mixer_faders[&Some(id)];
                assert!((fader.top() - master.top()).abs() < 0.1);
                assert!((fader.bottom() - master.bottom()).abs() < 0.1);
            }
        }
        let before = app.session.project.clone();
        app.select_track(r, false);
        app.perform_track_action(TrackAction::Delete, &ctx);
        assert!(
            app.session
                .project
                .tracks
                .iter()
                .all(|t| t.sends.is_empty())
        );
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
    }
}
#[test]
fn return_overflow_is_bounded_and_leaves_master_and_source_viewports() {
    for view in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, source) = fixture();
        let ctx = context();
        app.view_mode = view;
        for _ in 0..12 {
            app.session.project.add_return_track().unwrap();
        }
        for size in [Vec2::new(800.0, 600.0), Vec2::new(1280.0, 900.0)] {
            frame_sized(&mut app, &ctx, vec![], size);
            if view == ViewMode::Mixer {
                assert!(app.mixer_bounds[&source].left() < app.master_bounds.left() - 120.0);
                assert!(app.master_bounds.right() <= size.x);
            } else {
                assert!(app.timeline_bounds.height() > 50.0);
                assert!(app.master_bounds.bottom() <= size.y);
            }
        }
    }
}
#[test]
fn mixer_shows_sends_without_project_edits_and_menu_creation_is_atomic() {
    let (mut app, source) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    app.view_mode = ViewMode::Mixer;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    label(&shapes, "+ Send");
    assert!(
        !shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Sends"))
    );
    assert_eq!(app.session.project, before);
    let id = Id::new_v4();
    app.edit(Edit::Batch(vec![
        Edit::AddReturn {
            track_id: id,
            name: "Return 1".into(),
        },
        Edit::SetTrackSends {
            track_id: source,
            sends: vec![TrackSend::new(id)],
        },
    ]));
    frame(&mut app, &ctx, vec![]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, before);
}

fn label(shapes: &[egui::epaint::ClippedShape], name: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == name => {
                Some(Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {name}"))
}
#[test]
fn plus_send_menu_creates_empty_return_and_silent_send_as_one_undo_step() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let plus = label(&shapes, "+ Send");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(plus), button(plus, true)],
    );
    frame(&mut app, &ctx, vec![button(plus, false)]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let new = label(&shapes, "New Return");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(new), button(new, true)],
    );
    frame(&mut app, &ctx, vec![button(new, false)]);
    let source = app
        .session
        .project
        .tracks
        .iter()
        .find(|t| t.id == source)
        .unwrap();
    assert_eq!(source.sends.len(), 1);
    assert_eq!(source.sends[0].gain_db, daw_core::SILENT_GAIN_DB);
    assert!(
        app.session
            .project
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Return)
            .unwrap()
            .effects
            .is_empty()
    );
    frame(&mut app, &ctx, vec![]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, before);
}

#[test]
fn return_bank_drag_is_release_only_and_cancellation_keeps_routes() {
    for view in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, _) = fixture();
        let ctx = context();
        app.view_mode = view;
        app.return_bank_height = 260.0;
        let first = app.session.project.add_return_track().unwrap();
        let second = app.session.project.add_return_track().unwrap();
        frame(&mut app, &ctx, vec![]);
        app.select_track(first, false);
        let rect = if view == ViewMode::Mixer {
            app.mixer_bounds[&second]
        } else {
            app.lane_bounds[&second]
        };
        let at = if view == ViewMode::Mixer {
            Pos2::new(rect.right() - 8.0, rect.top() + 16.0)
        } else {
            Pos2::new(rect.left() - 60.0, rect.bottom() - 8.0)
        };
        let before = app.session.project.clone();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(at), button(at, true)],
        );
        app.select_track(first, false);
        app.track_drag = Some(first);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at)]);
        assert_eq!(app.session.project, before);
        frame(&mut app, &ctx, vec![button(at, false)]);
        let returns: Vec<_> = app
            .session
            .project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Return)
            .map(|t| t.id)
            .collect();
        assert_eq!(returns, [second, first]);
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, before);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(at), button(at, true)],
        );
        app.select_track(first, false);
        app.track_drag = Some(first);
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
        assert!(app.track_drag.is_none());
        assert_eq!(app.session.project, before);
        frame(&mut app, &ctx, vec![button(at, false)]);
    }
}

#[test]
fn send_context_menu_changes_the_tap_without_opening_the_track_menu() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    let r = app.session.project.add_return_track().unwrap();
    app.edit(Edit::SetTrackSends {
        track_id: source,
        sends: vec![TrackSend::new(r)],
    });
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(!shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if ["Pre","Post"].contains(&t.galley.text()))
    ));
    let source_rect = app.mixer_bounds[&source];
    let at = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text() == "Return 1" && source_rect.contains(text.pos) =>
            {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("send destination label");
    let secondary = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(at), secondary(true)],
    );
    frame(&mut app, &ctx, vec![secondary(false)]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let pre = label(&shapes, "Pre-fader");
    assert!(
        !shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Rename track"))
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pre), button(pre, true)],
    );
    frame(&mut app, &ctx, vec![button(pre, false)]);
    assert_eq!(
        app.session
            .project
            .tracks
            .iter()
            .find(|t| t.id == source)
            .unwrap()
            .sends[0]
            .tap,
        daw_core::SendTap::PreFader
    );
}

#[test]
fn send_stack_grows_with_its_rows_and_scrolls_with_the_complete_mixer() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    let returns: Vec<_> = (0..40)
        .map(|_| app.session.project.add_return_track().unwrap())
        .collect();
    app.edit(Edit::SetTrackSends {
        track_id: source,
        sends: returns.iter().copied().map(TrackSend::new).collect(),
    });
    let project = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    let source_before = app.mixer_bounds[&source];
    let master_before = app.master_bounds;
    assert!(
        source_before.height() > 800.0,
        "all send rows reserve their natural height"
    );
    let pointer = Pos2::new(master_before.center().x, 400.0);
    for _ in 0..3 {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -2000.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
        );
    }
    for _ in 0..10 {
        frame(&mut app, &ctx, vec![]);
    }
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let bounds = app.mixer_bounds[&source];
    assert!(bounds.top() < source_before.top());
    assert!(
        (app.master_bounds.top() - master_before.top() - (bounds.top() - source_before.top()))
            .abs()
            < 0.1
    );
    assert!(shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Return 40" && bounds.x_range().contains(t.pos.x))));
    assert_eq!(
        app.session.project, project,
        "scrolling does not edit routing"
    );
}

#[test]
fn grouped_return_dropdown_retargets_the_send_and_undo_restores_its_settings() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    let first = app.session.project.add_return_track().unwrap();
    let second = app.session.project.add_return_track().unwrap();
    let mut send = TrackSend::new(first);
    send.enabled = false;
    send.gain_db = -6.0;
    send.tap = daw_core::SendTap::PreFader;
    app.edit(Edit::SetTrackSends {
        track_id: source,
        sends: vec![send.clone()],
    });
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let bounds = app.mixer_bounds[&source];
    let caption = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Return 1" && bounds.contains(t.pos) => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("source Return dropdown");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(caption), button(caption, true)],
    );
    frame(&mut app, &ctx, vec![button(caption, false)]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let option = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if t.galley.text() == "Return 2" && t.pos.x < bounds.right() + menus::MIN_WIDTH =>
            {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("available Return option");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(option), button(option, true)],
    );
    frame(&mut app, &ctx, vec![button(option, false)]);
    let actual = &app
        .session
        .project
        .tracks
        .iter()
        .find(|t| t.id == source)
        .unwrap()
        .sends[0];
    assert_eq!(
        actual,
        &TrackSend {
            return_id: second,
            ..send
        }
    );
    frame(&mut app, &ctx, vec![]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project, before);
}

#[test]
fn pinned_timeline_boundaries_have_one_full_width_divider() {
    for count in 0..=2 {
        let (mut app, _) = fixture();
        let ctx = context();
        app.view_mode = ViewMode::Timeline;
        frame(&mut app, &ctx, vec![]);
        for _ in 0..count {
            app.perform_track_action(TrackAction::AddReturn, &ctx);
        }
        for collapsed in [false, true] {
            app.track_rows_collapsed = collapsed;
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let master = app.master_bounds;
            let mut tops = vec![master.top()];
            if count > 0 {
                let top = app
                    .session
                    .project
                    .tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Return)
                    .map(|t| app.lane_bounds[&t.id].top())
                    .min_by(f32::total_cmp)
                    .unwrap();
                tops.push(top);
            }
            for top in tops {
                let lines: Vec<_> = shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::LineSegment { points, stroke }
                            if stroke.color == theme::palette(&ctx).border
                                && (points[0].x - master.left()).abs() < 0.1
                                && points[1].x > master.right()
                                && (points[0].y - points[1].y).abs() < 0.1
                                && (points[0].y - top).abs() <= 1.0 =>
                        {
                            Some((points[0].y, stroke.width))
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    lines.len(),
                    1,
                    "Returns={count}, collapsed={collapsed}, top={top}: {lines:?}"
                );
                assert!((lines[0].0 - (top - 0.5)).abs() < 0.1);
                assert_eq!(lines[0].1, 1.0);
            }
        }
    }
}

#[test]
fn mixer_returns_expand_to_show_every_strip_when_they_fit() {
    for count in [0, 1, 3, 5] {
        let (mut app, source) = fixture();
        let ctx = context();
        app.view_mode = ViewMode::Mixer;
        app.mixer_effects.catalog = false;
        let returns: Vec<_> = (0..count)
            .map(|_| app.session.project.add_return_track().unwrap())
            .collect();
        let before = app.session.project.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let master = app.master_bounds;
        let source = app.mixer_bounds[&source];
        for (index, id) in returns.iter().enumerate() {
            let rect = app.mixer_bounds[id];
            assert_eq!(rect.width(), crate::mixer::STRIP_WIDTH);
            assert!(rect.left() >= source.right());
            assert!(
                (rect.right()
                    - (master.left() - (count - index - 1) as f32 * crate::mixer::STRIP_WIDTH))
                    .abs()
                    < 0.1
            );
            let name = &app
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == *id)
                .unwrap()
                .name;
            let clip = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == *name && rect.contains(text.pos) =>
                    {
                        Some(shape.clip_rect)
                    }
                    _ => None,
                })
                .expect("Return header");
            assert!(
                clip.left() <= rect.left() + 0.1 && clip.right() >= rect.right() - 0.1,
                "Return {index} clipped: {rect:?} / {clip:?}"
            );
        }
        assert_eq!(app.session.project, before);
    }
}

#[test]
fn mixer_return_overflow_scrolls_without_moving_master_or_source() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    app.mixer_effects.catalog = false;
    let returns: Vec<_> = (0..12)
        .map(|_| app.session.project.add_return_track().unwrap())
        .collect();
    frame(&mut app, &ctx, vec![]);
    let master = app.master_bounds;
    let source_rect = app.mixer_bounds[&source];
    let first = app.mixer_bounds[&returns[0]];
    let before = app.session.project.clone();
    let point = Pos2::new(master.left() - 60.0, master.center().y);
    for _ in 0..3 {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-2000.0, 0.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
        );
    }
    for _ in 0..10 {
        frame(&mut app, &ctx, vec![]);
    }
    let last = app.mixer_bounds[returns.last().unwrap()];
    assert!(app.mixer_bounds[&returns[0]].left() < first.left());
    assert!(last.left() >= source_rect.right() && last.right() <= master.left() + 0.1);
    assert_eq!(app.master_bounds, master);
    assert_eq!(app.mixer_bounds[&source], source_rect);
    let point = last.center_top() + Vec2::new(0.0, 12.0);
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert_eq!(app.selected_track, returns.last().copied());
    assert_eq!(app.session.project, before);
}

#[test]
fn mixer_returns_toggle_is_right_aligned_and_only_changes_presentation() {
    let (mut app, source) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    let destination = app.session.project.add_return_track().unwrap();
    app.session
        .project
        .tracks
        .iter_mut()
        .find(|t| t.id == source)
        .unwrap()
        .sends = vec![TrackSend::new(destination)];
    app.select_track(destination, false);
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert_eq!(
        app.mixer_returns_bounds.size(),
        Vec2::splat(icons::TOOLBAR_BUTTON_SIZE)
    );
    assert_eq!(app.mixer_returns_bounds.size(), Vec2::splat(24.0));
    assert!(app.mixer_returns_visible && app.mixer_bounds.contains_key(&destination));
    let master = app.master_bounds;
    let point = app.mixer_returns_bounds.center();
    assert!(point.x > 1180.0 && point.x > app.tempo_bounds.right());
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, pressed)],
        );
    }
    assert!(!app.mixer_returns_visible && !app.mixer_bounds.contains_key(&destination));
    assert_eq!(app.master_bounds, master);
    assert_eq!(app.selected_track, Some(destination));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    app.view_mode = ViewMode::Timeline;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(app.lane_bounds.contains_key(&destination));
    assert!(app.mixer_returns_bounds.is_positive());
    assert!(
        !shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Returns"))
    );
    app.view_mode = ViewMode::Mixer;
    frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(!app.mixer_bounds.contains_key(&destination));
    let point = app.mixer_returns_bounds.center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, pressed)],
        );
    }
    assert!(app.mixer_bounds.contains_key(&destination));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn mixer_send_stacks_align_plus_at_the_bottom_for_different_send_counts() {
    let (mut app, first) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    app.mixer_effects.catalog = false;
    let second = app.session.project.add_track().unwrap();
    let third = app.session.project.add_track().unwrap();
    let returns: Vec<_> = (0..4)
        .map(|_| app.session.project.add_return_track().unwrap())
        .collect();
    for (id, count) in [(first, 4), (second, 1), (third, 0)] {
        app.session
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .sends = returns
            .iter()
            .take(count)
            .map(|id| TrackSend::new(*id))
            .collect();
    }
    let before = app.session.project.clone();
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let mut bottoms = vec![];
    for id in [first, second, third] {
        let strip = app.mixer_bounds[&id];
        let plus = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == "+ Send" && strip.contains(t.pos) => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                }
                _ => None,
            })
            .unwrap();
        let button = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(r)
                    if r.rect.contains_rect(plus)
                        && r.rect.height() < 30.0
                        && strip.contains_rect(r.rect) =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .unwrap();
        let divider = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, stroke }
                    if (points[0].x - strip.left()).abs() < 0.1
                        && (points[1].x - strip.right()).abs() < 0.1
                        && points[0].y > button.bottom()
                        && points[0].y - button.bottom() < 10.0 =>
                {
                    Some(points[0].y - stroke.width / 2.0)
                }
                _ => None,
            })
            .unwrap();
        assert!((divider - button.bottom() - 8.0).abs() < 0.1);
        bottoms.push(button.bottom());
    }
    assert!(
        bottoms
            .iter()
            .all(|bottom| (*bottom - bottoms[0]).abs() < 0.1)
    );
    assert_eq!(app.session.project, before);
}

#[test]
fn return_selection_backgrounds_keep_shared_borders_and_pinned_dividers() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for count in [1, 2, 12] {
            for expanded in [false, true] {
                let (mut app, _) = fixture();
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                app.track_rows_collapsed = !expanded;
                let returns: Vec<_> = (0..count)
                    .map(|_| app.session.project.add_return_track().unwrap())
                    .collect();
                app.select_track(returns[0], false);
                app.selected_tracks.extend(returns.iter().copied());
                let size = Vec2::new(1000.0, 650.0);
                frame_shapes(&mut app, &ctx, vec![], size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let palette = theme::palette(&ctx);
                let mut visible = 0;
                for id in &returns {
                    let lane = app.lane_bounds[id];
                    let header = Rect::from_min_max(
                        Pos2::new(lane.left() - TRACK_WIDTH, lane.top()),
                        Pos2::new(lane.left(), lane.bottom()),
                    );
                    let Some(selected) = shapes.iter().enumerate().find(|(_, shape)| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.rect == header && rect.fill == theme::selected_track_header_background(&ctx)
                    )) else { continue; };
                    let clipped = header.intersect(selected.1.clip_rect);
                    if !clipped.is_positive() {
                        continue;
                    }
                    visible += 1;
                    assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == header && rect.stroke == theme::selection_outline(&ctx))));
                    // Selected fills retain the full row bounds; neutral dividers remain above them.
                    assert_eq!(
                        selected.1.clip_rect.right().min(header.right()),
                        header.right()
                    );
                    let column = shapes
                        .iter()
                        .enumerate()
                        .filter_map(|(index, shape)| match &shape.shape {
                            egui::Shape::LineSegment { points, stroke }
                                if stroke.color == palette.border
                                    && points[0].x == points[1].x
                                    && (points[0].x - header.right()).abs() <= 1.0
                                    && points[0].y.min(points[1].y) < clipped.bottom()
                                    && points[0].y.max(points[1].y) > clipped.top() =>
                            {
                                Some(index)
                            }
                            _ => None,
                        })
                        .max()
                        .expect("continuous neutral column border");
                    assert!(
                        column > selected.0,
                        "final column border remains above selected fill"
                    );
                    let horizontal = shapes
                        .iter()
                        .enumerate()
                        .filter_map(|(index, shape)| match &shape.shape {
                            egui::Shape::LineSegment { points, stroke }
                                if stroke.color == palette.border
                                    && points[0].y == points[1].y
                                    && points[0].y >= clipped.top() - 1.0
                                    && points[0].y <= clipped.bottom()
                                    && points[0].x.min(points[1].x) <= header.left()
                                    && points[0].x.max(points[1].x) >= header.right() =>
                            {
                                Some(index)
                            }
                            _ => None,
                        })
                        .max();
                    if let Some(index) = horizontal {
                        assert!(
                            index > selected.0,
                            "final horizontal divider remains above selected fill"
                        );
                    }
                }
                assert!(visible > 0);
            }
        }
    }
}

#[test]
fn mixer_master_toggle_frees_space_and_preserves_selection_and_project() {
    let (mut app, _) = fixture();
    let ctx = context();
    app.view_mode = ViewMode::Mixer;
    let destination = app.session.project.add_return_track().unwrap();
    app.select_master();
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let master = app.master_bounds;
    let return_strip = app.mixer_bounds[&destination];
    assert!(app.mixer_master_visible);
    assert!(app.mixer_returns_bounds.bottom() <= app.mixer_master_bounds.top());
    assert_eq!(app.mixer_master_bounds.size(), Vec2::splat(24.0));
    let point = app.mixer_master_bounds.center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, pressed)],
        );
    }
    assert!(!app.mixer_master_visible);
    assert_eq!(app.master_bounds, Rect::NOTHING);
    assert!(!app.mixer_faders.contains_key(&None));
    assert!(
        (app.mixer_bounds[&destination].right() - return_strip.right() - master.width()).abs()
            < 0.1
    );
    assert!(app.master_selected);
    app.toggle_view_panel(ViewPanel::Returns, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.mixer_bounds.contains_key(&destination));
    assert_eq!(app.master_bounds, Rect::NOTHING);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.master_bounds.is_positive());
    assert!(app.mixer_master_bounds.is_positive());
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.mixer_master_visible);
    assert_eq!(app.master_bounds, Rect::NOTHING);
    let point = app.mixer_master_bounds.center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, pressed)],
        );
    }
    assert!(app.mixer_master_visible);
    assert_eq!(app.master_bounds, master);
    assert!(!app.mixer_returns_visible);
    assert!(app.master_selected);
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn timeline_return_master_toggles_hide_rows_without_changing_mixer_or_project() {
    let (mut app, _) = fixture();
    let ctx = context();
    let destination = app.session.project.add_return_track().unwrap();
    app.select_master();
    let before = app.session.project.clone();
    frame(&mut app, &ctx, vec![]);
    let _shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(app.master_bounds.is_positive());
    assert!(app.lane_bounds.contains_key(&destination));
    assert!(app.mixer_returns_bounds.bottom() <= app.mixer_master_bounds.top());
    let rail = app.effects_bounds.clip_settings_toolbar;
    assert!(app.mixer_returns_bounds.left() >= rail.left());
    assert!(app.mixer_master_bounds.right() <= rail.right());
    assert!(app.mixer_master_bounds.bottom() > rail.bottom() - timeline_toolbar::WIDTH);
    for panel in [ViewPanel::Returns, ViewPanel::Master] {
        let point = match panel {
            ViewPanel::Returns => app.mixer_returns_bounds.center(),
            _ => app.mixer_master_bounds.center(),
        };
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, pressed)],
            );
        }
        frame(&mut app, &ctx, vec![]);
        assert!(!app.view_panel_selected(panel));
    }
    assert_eq!(app.master_bounds, Rect::NOTHING);
    assert!(!app.lane_bounds.contains_key(&destination));
    assert!(app.master_selected);
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.view_panel_selected(ViewPanel::Returns));
    assert!(app.view_panel_selected(ViewPanel::Master));
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.view_panel_selected(ViewPanel::Returns));
    assert!(!app.view_panel_selected(ViewPanel::Master));
    app.settings_open = true;
    app.toggle_view_panel(ViewPanel::Returns, &ctx);
    app.toggle_view_panel(ViewPanel::Master, &ctx);
    assert!(!app.timeline_returns_visible);
    assert!(!app.timeline_master_visible);
    app.settings_open = false;
    app.toggle_view_panel(ViewPanel::Returns, &ctx);
    app.toggle_view_panel(ViewPanel::Master, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.master_bounds.is_positive());
    assert!(app.lane_bounds.contains_key(&destination));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn visibility_toggles_stay_at_upper_toolbar_bottom_in_both_views() {
    for view in [ViewMode::Timeline, ViewMode::Mixer] {
        for lane in [false, true] {
            let (mut app, _) = fixture();
            let ctx = context();
            app.view_mode = view;
            app.set_effects_lane_visible(lane);
            frame(&mut app, &ctx, vec![]);
            let returns = app.mixer_returns_bounds;
            let master = app.mixer_master_bounds;
            assert_eq!(returns.size(), Vec2::splat(icons::TOOLBAR_BUTTON_SIZE));
            assert_eq!(master.size(), returns.size());
            assert_eq!(returns.left(), master.left());
            assert!(returns.bottom() < master.top());
            assert!(returns.top() > app.transport_bounds[0].bottom() + 100.0);
            let bottom = if lane {
                app.effects_bounds.lane_toolbar.top()
            } else {
                773.0
            };
            let inset = (timeline_toolbar::WIDTH - icons::TOOLBAR_BUTTON_SIZE) * 0.5;
            assert!(
                (master.bottom() - (bottom - inset)).abs() < 1.0,
                "{view:?}, effects={lane}: {master:?}"
            );
            if lane {
                assert!(master.bottom() < app.effects_bounds.lane_compact.top());
            }
            if view == ViewMode::Mixer {
                assert!(app.master_bounds.right() <= master.left());
            } else {
                assert!(
                    app.effects_bounds
                        .clip_settings_toolbar
                        .contains_rect(master)
                );
            }
        }
    }
}
