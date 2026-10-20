use super::*;
use daw_core::{EffectChainTarget, EffectInstance, TrackKind};
fn grouped_fixture() -> (DawUi, egui::Context, Id, [Id; 3]) {
    let (mut app, ids, _) = multi_clip_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.select_track(ids[0], false);
    app.select_track(ids[1], true);
    app.perform_track_action(TrackAction::Group, &ctx);
    let group = app.selected_track.unwrap();
    assert_eq!(app.session.project.tracks[0].kind, TrackKind::Group);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, group, ids)
}
#[test]
fn group_actions_reuse_selection_effect_targets_history_and_collapse_in_both_views() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        let (mut app, ctx, group, ids) = grouped_fixture();
        let grouped = app.session.project.clone();
        assert_eq!(app.effects_target(), Some(EffectChainTarget::Track(group)));
        app.view_mode = mode;
        app.edit(Edit::SetGroupCollapsed {
            track_id: group,
            collapsed: true,
        });
        frame(&mut app, &ctx, vec![]);
        if mode == ViewMode::Timeline {
            assert!(app.lane_bounds.contains_key(&group));
            assert!(!app.lane_bounds.contains_key(&ids[0]));
            let point = Pos2::new(
                app.timeline_bounds.left() - TRACK_WIDTH + 7.0 + toolbars::CONTROL_HEIGHT * 0.5,
                app.lane_bounds[&group].top() + 18.0,
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(&mut app, &ctx, vec![button(point, false)]);
            assert!(!app.session.project.tracks[0].group_collapsed);
            assert!(app.track_drag.is_none());
        }
        app.edit(Edit::SetGroupCollapsed {
            track_id: group,
            collapsed: false,
        });
        frame(&mut app, &ctx, vec![]);
        app.select_track(group, false);
        app.perform_track_action(TrackAction::Ungroup, &ctx);
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            ids
        );
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project, grouped);
    }
}
#[test]
fn group_drop_geometry_accepts_children_rejects_cycles_and_allows_root_exit() {
    let (app, _, group, ids) = grouped_fixture();
    let row = app.lane_bounds[&group];
    let inside = Pos2::new(row.left() - 100.0, row.center().y);
    let (parent, index, _) = app.track_drop_target(ids[2], inside).unwrap();
    assert_eq!(parent, Some(group));
    assert_eq!(index, 3);
    assert!(app.track_drop_target(group, inside).is_none());
    let after = Pos2::new(
        row.left() - TRACK_WIDTH + 10.0,
        app.lane_bounds[&ids[2]].bottom() + 1.0,
    );
    let (parent, index, _) = app.track_drop_target(ids[0], after).unwrap();
    assert_eq!(parent, None);
    assert_eq!(index, 3);
    assert!(
        app.file_target(inside, app.timeline_bounds, false)
            .is_none()
    );
}
#[test]
fn group_duplicate_and_delete_skip_selected_descendants() {
    let (mut app, ctx, group, ids) = grouped_fixture();
    app.select_track(ids[0], true);
    app.perform_track_action(TrackAction::Duplicate, &ctx);
    assert_eq!(app.session.project.tracks.len(), 7);
    assert_eq!(app.selected_tracks.len(), 1);
    let copy = app.selected_track.unwrap();
    assert_ne!(copy, group);
    app.select_track(group, false);
    app.select_track(ids[0], true);
    app.perform_track_action(TrackAction::Delete, &ctx);
    assert_eq!(app.session.project.tracks.len(), 4);
    assert!(app.session.project.tracks.iter().any(|t| t.id == copy));
    assert!(!app.session.project.tracks.iter().any(|t| t.id == ids[0]));
}
#[test]
fn group_effects_reuse_catalog_controls_and_atomic_transfers() {
    let (mut app, ctx, group, ids) = grouped_fixture();
    let source = EffectChainTarget::Track(ids[0]);
    let bus = EffectChainTarget::Track(group);
    let drive = EffectInstance::drive();
    let id = drive.id;
    app.edit(source.edit(vec![drive], false));
    let transfer = app
        .session
        .project
        .transfer_effects(source, bus, &[id], 0, false)
        .unwrap();
    app.edit(transfer.edit);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.chain[0].0, id);
    let transfer = app
        .session
        .project
        .transfer_effects(bus, source, &[id], 0, true)
        .unwrap();
    app.edit(transfer.edit);
    assert_ne!(
        app.session.project.effect_chain(source).unwrap().0[0].id,
        id
    );
    assert_eq!(app.session.project.effect_chain(bus).unwrap().0[0].id, id);
}

#[test]
fn pointer_track_drops_commit_on_release_and_escape_keeps_the_original_tree() {
    for cancel in [false, true] {
        let (mut app, ctx, group, ids) = grouped_fixture();
        let original = app.session.project.clone();
        let start = Pos2::new(
            app.timeline_bounds.left() - TRACK_WIDTH + 18.0,
            app.lane_bounds[&ids[2]].top() + 18.0,
        );
        let end = Pos2::new(start.x, app.lane_bounds[&group].center().y);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(0.0, -10.0))],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            app.session.project, original,
            "preview must not mutate the project"
        );
        assert!(app.track_drag.is_some());
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
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(app.track_drag.is_none());
        if cancel {
            assert_eq!(app.session.project, original);
        } else {
            assert_eq!(
                app.session.project.tracks.last().unwrap().group_id,
                Some(group)
            );
        }
    }
}

#[test]
fn group_and_master_titles_align_left_and_group_disclosures_have_no_frame() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            let (mut app, ctx, group, _) = grouped_fixture();
            app.view_mode = mode;
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            for collapsed in [false, true] {
                app.edit(Edit::SetGroupCollapsed {
                    track_id: group,
                    collapsed,
                });
                let original = app.session.project.clone();
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let (panel, inset) = if mode == ViewMode::Timeline {
                    let lane = app.lane_bounds[&group];
                    (
                        Rect::from_min_max(
                            Pos2::new(lane.left() - TRACK_WIDTH, lane.top()),
                            lane.left_bottom(),
                        ),
                        7.0,
                    )
                } else {
                    (app.mixer_bounds[&group], 4.0)
                };
                let text_bounds = |name: &str, top: f32, bottom: f32| {
                    shapes
                        .iter()
                        .find_map(|shape| {
                            if let egui::Shape::Text(text) = &shape.shape
                                && text.galley.text() == name
                                && text.pos.y >= top
                                && text.pos.y < bottom
                            {
                                Some(text.galley.rect.translate(text.pos.to_vec2()))
                            } else {
                                None
                            }
                        })
                        .unwrap()
                };
                let name = text_bounds("Group 1", panel.top(), panel.bottom());
                assert!(
                    (name.left() - (panel.left() + inset + 28.0 + 6.0)).abs() < 1.0,
                    "{mode:?}: {name:?} {panel:?}"
                );
                let master = text_bounds(
                    "Master",
                    app.master_bounds.top(),
                    app.master_bounds.bottom(),
                );
                assert!(
                    (master.left() - (app.master_bounds.left() + inset + 34.0)).abs() < 1.0,
                    "{mode:?}: {master:?}"
                );
                if mode == ViewMode::Timeline {
                    let gain_center = app.master_bounds.min + Vec2::new(21.0, 47.0);
                    assert!(
                        shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Circle(circle) if circle.center.distance(gain_center) < 1.0
                        )),
                        "Master gain must retain its row position"
                    );
                }
                let arrow = Pos2::new(panel.left() + inset + 14.0, name.center().y);
                assert!(
                    !shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.rect.contains(arrow)
                            && rect.rect.width() <= 30.0 && rect.rect.height() <= 26.0
                            && rect.stroke.width > 0.0
                    )),
                    "{mode:?}: disclosure must remain borderless"
                );
                assert_eq!(app.session.project, original);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(arrow), button(arrow, true)],
                );
                frame(&mut app, &ctx, vec![button(arrow, false)]);
                assert_eq!(app.session.project.tracks[0].group_collapsed, !collapsed);
                assert!(app.track_drag.is_none());
            }
        }
    }
}

#[test]
fn track_and_master_icons_share_group_header_geometry_in_both_views() {
    for mode in [ViewMode::Timeline, ViewMode::Mixer] {
        for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            for scale in [1.0, 1.5] {
                let (mut app, ctx, group, ids) = grouped_fixture();
                let midi = app.session.project.add_midi_track().unwrap();
                app.edit(Edit::MoveTrack {
                    track_id: midi,
                    group_id: Some(group),
                    index: 3,
                });
                app.view_mode = mode;
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                ctx.set_pixels_per_point(scale);
                frame_sized(&mut app, &ctx, vec![], Vec2::new(1600.0, 1400.0));
                let original = app.session.project.clone();
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1600.0, 1400.0));
                for (id, icon) in [
                    (Some(group), icons::Lucide::ChevronDown),
                    (Some(ids[0]), icons::Lucide::AudioLines),
                    (Some(midi), icons::Lucide::KeyboardMusic),
                    (None, icons::Lucide::BoomBox),
                ] {
                    let name_text = id
                        .and_then(|id| {
                            app.session
                                .project
                                .tracks
                                .iter()
                                .find(|track| track.id == id)
                        })
                        .map_or("Master", |track| track.name.as_str());
                    let (left, top, bottom, inset, y) = match (mode, id) {
                        (ViewMode::Timeline, Some(id)) => {
                            let lane = app.lane_bounds[&id];
                            (
                                lane.left() - TRACK_WIDTH
                                    + crate::track_groups::panel_indent(
                                        app.session.project.track_depth(id),
                                    ),
                                lane.top(),
                                lane.bottom(),
                                7.0,
                                lane.top() + 18.0,
                            )
                        }
                        (ViewMode::Timeline, None) => {
                            let panel = app.master_bounds;
                            (
                                panel.left(),
                                panel.top(),
                                panel.bottom(),
                                7.0,
                                panel.top() + 18.0,
                            )
                        }
                        (_, id) => {
                            let strip = id.map_or(app.master_bounds, |id| app.mixer_bounds[&id]);
                            (
                                strip.left(),
                                strip.top(),
                                strip.bottom(),
                                4.0,
                                strip.top() + 12.0,
                            )
                        }
                    };
                    let name = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text)
                                if text.galley.text() == name_text
                                    && text.pos.y >= top
                                    && text.pos.y < bottom =>
                            {
                                Some(text.galley.rect.translate(text.pos.to_vec2()))
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| {
                            panic!("missing track name {name_text} in {mode:?} at {scale}×")
                        });
                    assert!(
                        (name.left() - left - inset - 34.0).abs() < 1.0,
                        "{mode:?} {name_text}: {name:?}"
                    );
                    let uri = icon.size(16.0).color(theme::palette(&ctx).secondary).uri();
                    let egui::load::TexturePoll::Ready { texture } = ctx
                        .try_load_texture(
                            &uri,
                            egui::TextureOptions::LINEAR,
                            egui::load::SizeHint::Size {
                                width: (16.0 * scale) as u32,
                                height: (16.0 * scale) as u32,
                                maintain_aspect_ratio: false,
                            },
                        )
                        .unwrap()
                    else {
                        panic!("track icon must be ready")
                    };
                    let expected_center = Pos2::new(left + inset + 14.0, y);
                    let icon_rect =
                        shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Rect(rect)
                                    if rect.brush.as_ref().is_some_and(|brush| {
                                        brush.fill_texture_id == texture.id
                                    }) && rect.rect.center().distance(expected_center) < 1.0 =>
                                {
                                    Some(rect.rect)
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "missing {} on {name_text} in {mode:?} at {scale}×",
                                    icon.name()
                                )
                            });
                    assert!((icon_rect.size() - Vec2::splat(16.0)).length() < 0.1);
                    assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.rect.contains(expected_center) && rect.rect.width() <= 30.0 && rect.rect.height() <= 26.0 && rect.stroke.width > 0.0
                    )));
                    if mode == ViewMode::Timeline {
                        let rails: Vec<_> = shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::Shape::Rect(rect)
                                    if rect.fill == theme::palette(&ctx).input
                                        && (rect.rect.width() - 8.0).abs() < 0.1
                                        && rect.rect.height() > 30.0
                                        && rect.rect.left() >= left
                                        && rect.rect.top() >= top
                                        && rect.rect.bottom() <= bottom =>
                                {
                                    Some(rect.rect)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(rails.len(), 2, "{name_text}");
                        assert!(
                            (rails[1].right() - (app.timeline_bounds.left() - 7.0)).abs() < 1.0,
                            "{name_text} meter must align with the control column's right inset: {:?}",
                            rails[1]
                        );
                    }
                }
                assert_eq!(app.session.project, original);
            }
        }
    }
}

#[test]
fn track_type_icons_reuse_release_only_dragging_and_cancellation() {
    for midi in [false, true] {
        for cancel in [false, true] {
            let mut app = DawUi::default();
            let track = if midi {
                app.session.project.add_midi_track().unwrap()
            } else {
                app.session.project.add_track().unwrap()
            };
            let next = app.session.project.add_track().unwrap();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            let start = Pos2::new(
                app.timeline_bounds.left() - TRACK_WIDTH + 21.0,
                app.lane_bounds[&track].top() + 18.0,
            );
            let end = Pos2::new(start.x, app.lane_bounds[&next].bottom() - 2.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert_eq!(app.track_drag, Some(track));
            assert_eq!(app.session.project, original);
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
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.track_drag.is_none());
            assert!(app.track_name_edit.is_none());
            if cancel {
                assert_eq!(app.session.project, original);
            } else {
                assert_eq!(
                    app.session
                        .project
                        .tracks
                        .iter()
                        .map(|track| track.id)
                        .collect::<Vec<_>>(),
                    [next, track]
                );
            }
        }
    }
}

#[test]
fn group_name_and_blank_header_drags_move_the_complete_subtree() {
    for start_x in [45.0, 130.0] {
        let (mut app, ctx, group, ids) = grouped_fixture();
        let original = app.session.project.clone();
        let header_left = app.timeline_bounds.left() - TRACK_WIDTH;
        let start = Pos2::new(header_left + start_x, app.lane_bounds[&group].top() + 18.0);
        let end = Pos2::new(header_left + 18.0, app.lane_bounds[&ids[2]].bottom() - 2.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.track_drag, Some(group));
        assert_eq!(app.session.project, original);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(app.track_drag.is_none());
        assert!(app.track_name_edit.is_none());
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            [ids[2], group, ids[0], ids[1]]
        );
        assert_eq!(app.session.project.tracks[2].group_id, Some(group));
        assert_eq!(app.session.project.tracks[3].group_id, Some(group));
    }
}

#[test]
fn group_shortcuts_share_menu_commands_and_respect_text_focus_and_repeats() {
    for mac in [false, true] {
        let (mut app, ids, _) = multi_clip_fixture();
        let ctx = context();
        app.select_track(ids[0], false);
        app.select_track(ids[1], true);
        frame(&mut app, &ctx, vec![]);
        let event = |shift, repeat| egui::Event::Key {
            key: egui::Key::G,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers: egui::Modifiers {
                command: true,
                ctrl: !mac,
                mac_cmd: mac,
                shift,
                ..Default::default()
            },
        };
        frame(&mut app, &ctx, vec![event(false, false)]);
        assert_eq!(app.session.project.tracks.len(), 4);
        let group = app.selected_track.unwrap();
        frame(&mut app, &ctx, vec![event(false, true)]);
        assert_eq!(app.session.project.tracks.len(), 4);
        app.track_name_edit = Some(TrackNameEdit {
            track: group,
            text: "Bus".into(),
            focus: true,
        });
        frame(&mut app, &ctx, vec![]);
        let grouped = app.session.project.clone();
        frame(&mut app, &ctx, vec![event(true, false)]);
        assert_eq!(app.session.project, grouped);
        app.track_name_edit = None;
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::G,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        frame(&mut app, &ctx, vec![event(true, false)]);
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            ids
        );
    }
}

#[test]
fn nested_track_panels_share_container_edges_and_keep_timeline_lanes_aligned() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for scale in [1.0, 1.25, 2.0] {
            let (mut app, ctx, inner, ids) = grouped_fixture();
            let outer = Id::new_v4();
            app.edit(Edit::GroupTracks {
                track_ids: vec![inner, ids[2]],
                group_id: outer,
                name: "Outer".into(),
            });
            app.select_track(outer, false);
            app.select_track(inner, true);
            app.select_track(ids[1], true);
            ctx.set_pixels_per_point(scale);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            for collapsed in [false, true] {
                app.session
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == inner)
                    .unwrap()
                    .group_collapsed = collapsed;
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let left = app.timeline_bounds.left() - TRACK_WIDTH;
                let right = app.timeline_bounds.left();
                assert!(
                    app.lane_bounds
                        .values()
                        .all(|lane| (lane.left() - right).abs() < 0.01),
                    "lane alignment at scale {scale}"
                );
                let outer_rect = Rect::from_min_max(
                    Pos2::new(left, app.lane_bounds[&outer].top()),
                    Pos2::new(right, app.lane_bounds[&ids[2]].bottom()),
                );
                let inner_bottom = if collapsed {
                    app.lane_bounds[&inner].bottom()
                } else {
                    app.lane_bounds[&ids[1]].bottom()
                };
                let inner_rect = Rect::from_min_max(
                    Pos2::new(left + 16.0, app.lane_bounds[&inner].top()),
                    Pos2::new(right, inner_bottom),
                );
                let mut expected = vec![outer_rect, inner_rect];
                if !collapsed {
                    expected.push(Rect::from_min_max(
                        Pos2::new(left + 32.0, app.lane_bounds[&ids[1]].top()),
                        inner_rect.max,
                    ));
                }
                for rect in expected {
                    assert!(shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx) && r.stroke_kind == egui::StrokeKind::Inside)),"missing group/child outline {rect:?}");
                }
                assert!(!shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::LineSegment {points,stroke} if *stroke == Stroke::new(1.0,theme::palette(&ctx).border)
                        && points[0].x == left+0.5 && points[1].x == left+0.5
                        && points[0].y == outer_rect.top() && points[1].y == outer_rect.bottom()
                )),"root group reuses the workspace left border");
                let y = outer_rect.bottom() - 0.5;
                let bottom_edges = shapes.iter().filter(|s| matches!(&s.shape,egui::Shape::LineSegment {points,stroke} if points[0].y == y && points[1].y == y && points[0].x == left && points[1].x >= right && *stroke == Stroke::new(1.0,theme::palette(&ctx).border))).count();
                assert_eq!(bottom_edges, 1, "one shared bottom edge");
                assert_eq!(app.lane_bounds.contains_key(&ids[0]), !collapsed);
            }
            app.session
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == inner)
                .unwrap()
                .group_collapsed = false;
            assert_eq!(
                app.session.project, original,
                "layout never edits track data"
            );
        }
    }
}

#[test]
fn inset_child_drops_keep_the_parent_and_group_append_preview_uses_the_container_bottom() {
    let (app, _, group, ids) = grouped_fixture();
    let child = app.lane_bounds[&ids[1]];
    let header_left = child.left() - TRACK_WIDTH;
    let header_x = header_left + 16.0 + 7.0 + toolbars::CONTROL_HEIGHT * 0.5;
    let (parent, index, y) = app
        .track_drop_target(ids[0], Pos2::new(header_x, child.bottom() - 2.0))
        .unwrap();
    assert_eq!((parent, index), (Some(group), 2));
    assert_eq!(y, child.bottom());
    let (parent, _, _) = app
        .track_drop_target(ids[0], Pos2::new(header_left + 10.0, child.bottom() - 2.0))
        .unwrap();
    assert_eq!(parent, None);
    let group_lane = app.lane_bounds[&group];
    let (_, _, y) = app
        .track_drop_target(ids[2], Pos2::new(header_x, group_lane.center().y))
        .unwrap();
    assert_eq!(y, child.bottom());
}

#[test]
fn scrolled_group_outlines_retain_offscreen_headers_and_virtual_child_bounds() {
    let mut app = DawUi::default();
    let ids: Vec<_> = (0..24)
        .map(|_| app.session.project.add_track().unwrap())
        .collect();
    let group = Id::new_v4();
    app.session
        .project
        .edit(Edit::GroupTracks {
            track_ids: ids.clone(),
            group_id: group,
            name: "Group".into(),
        })
        .unwrap();
    app.select_track(group, false);
    let ctx = context();
    app.timeline_effects_visible = false;
    frame(&mut app, &ctx, vec![]);
    let point = app.lane_bounds[&ids[0]].min + Vec2::splat(20.0);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -600.0),
                modifiers: Default::default(),
                phase: egui::TouchPhase::Move,
            },
        ],
    );
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let left = app.timeline_bounds.left() - TRACK_WIDTH;
    let rect = Rect::from_min_max(
        Pos2::new(left, app.lane_bounds[&group].top()),
        Pos2::new(
            app.timeline_bounds.left(),
            app.lane_bounds[ids.last().unwrap()].bottom(),
        ),
    );
    let outline = shapes.iter().find(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx))).unwrap();
    assert!(rect.top() < outline.clip_rect.top());
    assert!(rect.bottom() > outline.clip_rect.bottom());
    assert!(
        app.lane_bounds
            .values()
            .all(|lane| lane.left() == app.timeline_bounds.left())
    );
}

// Exercise the actual drag/release path, including modifier precedence over Copy.
fn clip_drag_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    pointer: Pos2,
    pressed: Option<bool>,
    duplicate: bool,
) -> egui::CursorIcon {
    let modifiers = egui::Modifiers {
        alt: duplicate && cfg!(target_os = "macos"),
        ctrl: duplicate && !cfg!(target_os = "macos"),
        ..Default::default()
    };
    let mut events = vec![
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::PointerMoved(pointer),
    ];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

#[test]
fn clip_drops_on_group_buses_show_invalid_cursor_and_reject_without_an_edit() {
    for midi in [false, true] {
        for collapsed in [false, true] {
            for duplicate in [false, true] {
                let (mut app, _) = fixture();
                let child = if midi {
                    app.session.project.add_midi_track().unwrap()
                } else {
                    app.session.project.add_track().unwrap()
                };
                let source = if midi {
                    app.session.project.add_midi_track().unwrap()
                } else {
                    app.session.project.add_track().unwrap()
                };
                let clip = if midi {
                    let id = app.session.project.add_midi_pattern(source, 0).unwrap();
                    app.session.project.tracks[2]
                        .clips
                        .iter()
                        .find(|clip| clip.id == id)
                        .unwrap()
                        .clone()
                } else {
                    let clip = Clip {
                        id: Id::new_v4(),
                        ..app.session.project.tracks[0].clips[0].clone()
                    };
                    app.session.project.tracks[2].clips.push(clip.clone());
                    clip
                };
                let group = Id::new_v4();
                app.session
                    .project
                    .edit(Edit::GroupTracks {
                        track_ids: vec![child],
                        group_id: group,
                        name: "Group".into(),
                    })
                    .unwrap();
                app.session
                    .project
                    .edit(Edit::SetGroupCollapsed {
                        track_id: group,
                        collapsed,
                    })
                    .unwrap();
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let block = app.clip_block(app.lane_bounds[&source], &clip, clip.start_frame);
                let start = block.min + Vec2::new(20.0, 12.0);
                let end = Pos2::new(start.x + 40.0, app.lane_bounds[&group].top() + 12.0);
                let before = app.session.project.clone();
                clip_drag_frame(&mut app, &ctx, start, Some(true), duplicate);
                let cursor = clip_drag_frame(&mut app, &ctx, end, None, duplicate);
                assert_eq!(cursor, egui::CursorIcon::NotAllowed);
                let previews = app.clip_drag_previews(app.drag.as_ref().unwrap(), end, false);
                assert_eq!(previews.len(), 1);
                assert_eq!(previews[0].track, group);
                assert!(!previews[0].valid);
                assert_eq!(app.session.project, before);
                clip_drag_frame(&mut app, &ctx, end, Some(false), duplicate);
                assert_eq!(app.session.project, before);
                assert_eq!(app.selected_clips, HashSet::from([clip.id]));
                assert_eq!(app.selected_track, Some(source));
                assert!(app.drag.is_none());
                assert!(app.job.is_none());
                assert!(app.error.is_none());
                assert!(!app.dirty);
                assert!(!app.history.as_ref().unwrap().can_undo());
            }
        }
    }
}

#[test]
fn clip_group_drop_rejects_all_members_when_only_a_secondary_clip_targets_a_bus() {
    for duplicate in [false, true] {
        let (mut app, ctx, group, tracks) = grouped_fixture();
        let first = app.session.project.tracks[1].clips[0].clone();
        let primary = app.session.project.tracks[2].clips[0].clone();
        select_group(&mut app, &ctx, [first.id, primary.id]);
        let before = app.session.project.clone();
        app.history = Some(daw_project::SessionHistory::new(&app.session, true).unwrap());
        app.dirty = false;
        let block = app.clip_block(app.lane_bounds[&tracks[1]], &primary, primary.start_frame);
        let start = block.min + Vec2::new(20.0, 12.0);
        let end = Pos2::new(start.x + 40.0, app.lane_bounds[&group].top() + 12.0);
        clip_drag_frame(&mut app, &ctx, start, Some(true), duplicate);
        let cursor = clip_drag_frame(&mut app, &ctx, end, None, duplicate);
        let drag = app.drag.as_ref().unwrap();
        let raw = app.move_group_preview(drag, end, false).unwrap();
        assert!(
            raw.iter()
                .any(|p| p.clip.id == primary.id && p.track == tracks[0] && p.valid)
        );
        assert!(
            raw.iter()
                .any(|p| p.clip.id == first.id && p.track == group && !p.valid)
        );
        assert!(
            app.clip_drag_previews(drag, end, false)
                .iter()
                .all(|p| !p.valid)
        );
        assert_eq!(cursor, egui::CursorIcon::NotAllowed);
        clip_drag_frame(&mut app, &ctx, end, Some(false), duplicate);
        assert_eq!(app.session.project, before);
        assert_eq!(app.selected_clips, HashSet::from([first.id, primary.id]));
        assert!(app.error.is_none());
        assert!(app.job.is_none());
        assert!(!app.dirty);
        assert!(!app.history.as_ref().unwrap().can_undo());
    }
}

#[test]
fn clip_drag_can_return_from_an_invalid_bus_to_a_valid_child_track() {
    for duplicate in [false, true] {
        let (mut app, ctx, group, tracks) = grouped_fixture();
        let clip = app.session.project.tracks[1].clips[0].clone();
        let block = app.clip_block(app.lane_bounds[&tracks[0]], &clip, clip.start_frame);
        let start = block.min + Vec2::new(20.0, 12.0);
        let invalid = Pos2::new(start.x + 200.0, app.lane_bounds[&group].top() + 12.0);
        clip_drag_frame(&mut app, &ctx, start, Some(true), duplicate);
        let cursor = clip_drag_frame(&mut app, &ctx, invalid, None, duplicate);
        assert_eq!(cursor, egui::CursorIcon::NotAllowed);
        let valid = Pos2::new(invalid.x, app.lane_bounds[&tracks[1]].top() + 12.0);
        let cursor = clip_drag_frame(&mut app, &ctx, valid, None, duplicate);
        assert_eq!(
            cursor,
            if duplicate {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            }
        );
        let previews = app
            .move_group_preview(app.drag.as_ref().unwrap(), valid, false)
            .unwrap();
        let placed = previews[0].clip.clone();
        clip_drag_frame(&mut app, &ctx, valid, Some(false), duplicate);
        assert!(app.error.is_none());
        assert!(app.job.is_none());
        let target = &app.session.project.tracks[2];
        assert!(
            target
                .clips
                .iter()
                .any(|c| c.start_frame == placed.start_frame && c.name == clip.name)
        );
        assert_eq!(
            app.session.project.tracks[1]
                .clips
                .iter()
                .any(|c| c.id == clip.id),
            duplicate
        );
        app.session.project.validate().unwrap();
    }
}
