use super::*;

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

fn library_key(app: &mut DawUi, ctx: &egui::Context, key: egui::Key, shift: bool) {
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
            egui::Event::ModifiersChanged(Default::default()),
        ],
    );
}

#[test]
fn effects_library_keyboard_and_shift_click_select_without_project_edits() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let original = serde_json::to_string(&app.session.project).unwrap();
        let entries = effects_catalog::ENTRIES.map(|entry| entry.0);
        let point = app.effects_bounds.drive_entry.center();
        click(&mut app, &ctx, point);
        for (key, shift, expected) in [
            (egui::Key::ArrowDown, false, vec![entries[2]]),
            (egui::Key::ArrowDown, true, vec![entries[2], entries[3]]),
            (egui::Key::ArrowDown, true, vec![entries[2], entries[3]]),
            (egui::Key::ArrowUp, true, vec![entries[2]]),
            (egui::Key::ArrowUp, true, vec![entries[1], entries[2]]),
            (egui::Key::ArrowUp, false, vec![entries[0]]),
            (egui::Key::ArrowUp, false, vec![entries[0]]),
        ] {
            library_key(&mut app, &ctx, key, shift);
            assert_eq!(
                app.effects_catalog.selection,
                expected.into_iter().collect()
            );
        }
        let point = app.effects_bounds.compressor_entry.center();
        click_effect_with_shift(&mut app, &ctx, point, true);
        assert_eq!(
            app.effects_catalog.selection,
            HashSet::from([entries[0], entries[3]])
        );
        click_effect_with_shift(&mut app, &ctx, point, true);
        assert_eq!(app.effects_catalog.selection, HashSet::from([entries[0]]));
        library_key(&mut app, &ctx, egui::Key::Escape, false);
        assert!(app.effects_catalog.selection.is_empty());
        library_key(&mut app, &ctx, egui::Key::ArrowDown, false);
        library_key(&mut app, &ctx, egui::Key::Enter, false);
        assert!(app.effects_catalog.selection.is_empty());
        assert_eq!(
            serde_json::to_string(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn effects_library_empty_space_clears_selection_on_release_without_project_edits() {
    for (x, y) in [
        (0.01, 0.01),
        (0.99, 0.01),
        (0.5, 0.5),
        (0.01, 0.99),
        (0.99, 0.99),
    ] {
        let (mut app, _, ctx, ids) = chain_fixture(false, false);
        let point = app.effects_bounds.entry.center();
        click(&mut app, &ctx, point);
        library_key(&mut app, &ctx, egui::Key::ArrowDown, true);
        assert_eq!(app.effects_catalog.selection.len(), 2);
        app.selected_effects = HashSet::from([ids[0], ids[1]]);
        app.dirty = false;
        let original = serde_json::to_string(&app.session.project).unwrap();
        let blank = Rect::from_min_max(
            Pos2::new(
                app.effects_bounds.library.left(),
                app.effects_bounds.compressor_entry.bottom(),
            ),
            app.effects_bounds.library.max,
        );
        let point = Pos2::new(
            egui::lerp(blank.x_range(), x),
            egui::lerp(blank.y_range(), y),
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert_eq!(app.effects_catalog.selection.len(), 2);
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(
            app.effects_catalog.selection.is_empty(),
            "x={x} y={y} point={point:?} blank={blank:?}"
        );
        assert!(app.selected_effects.is_empty());
        assert_eq!(
            serde_json::to_string(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn effects_library_empty_click_respects_input_guards_and_other_catalog_controls() {
    for guard in ["header", "compact", "secondary", "modal", "focus", "drag"] {
        let (mut app, _, ctx, _) = chain_fixture(false, false);
        let entry = app.effects_bounds.entry.center();
        click(&mut app, &ctx, entry);
        let selected = app.effects_catalog.selection.clone();
        let blank = Pos2::new(
            app.effects_bounds.library.center().x,
            app.effects_bounds.library.bottom() - 8.0,
        );
        let point = match guard {
            "header" => app.effects_bounds.header.center(),
            "compact" => app.effects_bounds.compact.center(),
            _ => blank,
        };
        if guard == "modal" {
            app.unsaved_prompt = true;
        }
        if guard == "drag" {
            begin_effect_drag(&mut app, &ctx, entry, blank);
        }
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused: guard != "focus",
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: if guard == "secondary" {
                                egui::PointerButton::Secondary
                            } else {
                                egui::PointerButton::Primary
                            },
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        if matches!(guard, "header" | "modal" | "focus") {
            assert!(app.effects_catalog.selection.is_empty());
        } else {
            assert_eq!(app.effects_catalog.selection, selected, "guard={guard}");
        }
    }
}

#[test]
fn effects_catalog_selection_clears_on_focus_loss_but_not_pointer_hover() {
    for target in ["control", "surrender", "window", "view", "hover"] {
        let (mut app, _, ctx, _) = chain_fixture(false, false);
        let point = app.effects_bounds.entry.center();
        click(&mut app, &ctx, point);
        library_key(&mut app, &ctx, egui::Key::ArrowDown, true);
        assert_eq!(app.effects_catalog.selection.len(), 2);
        let original = serde_json::to_string(&app.session.project).unwrap();
        app.dirty = false;
        match target {
            "control" => {
                let point = app.effects_bounds.units[0].knobs[0][0].center();
                click(&mut app, &ctx, point);
            }
            "surrender" => {
                let id = ctx.memory(|memory| memory.focused()).unwrap();
                ctx.memory_mut(|memory| memory.surrender_focus(id));
                frame(&mut app, &ctx, vec![]);
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
            "view" => {
                app.set_view_mode(ViewMode::Timeline, &ctx);
                frame(&mut app, &ctx, vec![]);
            }
            _ => {
                let point = app.effects_bounds.strip.center();
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
            }
        }
        assert_eq!(
            app.effects_catalog.selection.len(),
            if target == "hover" { 2 } else { 0 },
            "target={target}"
        );
        assert_eq!(
            serde_json::to_string(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn effects_lane_selection_clears_on_focus_loss_and_keeps_controls_in_the_lane() {
    for target in [
        "catalog",
        "control",
        "outside",
        "surrender",
        "window",
        "view",
        "hover",
    ] {
        let (mut app, _, ctx, ids) = chain_fixture(false, false);
        let point = app.effects_bounds.units[0].header.center();
        click(&mut app, &ctx, point);
        let point = app.effects_bounds.drive_units[0].header.center();
        click_effect_with_shift(&mut app, &ctx, point, true);
        assert_eq!(app.selected_effects, HashSet::from([ids[0], ids[1]]));
        let original = serde_json::to_string(&app.session.project).unwrap();
        app.dirty = false;
        match target {
            "catalog" => {
                let point = app.effects_bounds.entry.center();
                click(&mut app, &ctx, point);
            }
            "control" => {
                let point = app.effects_bounds.units[0].knobs[0][0].center();
                click(&mut app, &ctx, point);
            }
            "outside" => {
                let point = Pos2::new(
                    app.effects_bounds.library.right() + 300.0,
                    app.effects_bounds.library.center().y,
                );
                click(&mut app, &ctx, point);
            }
            "surrender" => {
                let id = ctx.memory(|memory| memory.focused()).unwrap();
                ctx.memory_mut(|memory| memory.surrender_focus(id));
                frame(&mut app, &ctx, vec![]);
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
            "view" => {
                app.set_view_mode(ViewMode::Mixer, &ctx);
                frame(&mut app, &ctx, vec![]);
            }
            _ => {
                let point = app.effects_bounds.entry.center();
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
            }
        }
        let expected = match target {
            "hover" => HashSet::from([ids[0], ids[1]]),
            "control" => HashSet::from([ids[0]]),
            _ => HashSet::new(),
        };
        assert_eq!(app.selected_effects, expected, "target={target}");
        assert_eq!(
            serde_json::to_string(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn effects_library_enter_adds_all_selected_in_library_order_and_escape_keeps_units() {
    let (mut app, _, ctx, ids) = chain_fixture(false, false);
    app.selected_effects = HashSet::from([ids[0], ids[1]]);
    let point = app.effects_bounds.compressor_entry.center();
    click(&mut app, &ctx, point);
    let point = app.effects_bounds.entry.center();
    click_effect_with_shift(&mut app, &ctx, point, true);
    let selected = app.effects_catalog.selection.clone();
    library_key(&mut app, &ctx, egui::Key::Enter, false);
    let effects = &app.session.project.tracks[0].effects;
    assert_eq!(effects.len(), 5);
    assert!(matches!(effects[3].unit, daw_core::EffectSettings::Eq3(_)));
    assert!(matches!(
        effects[4].unit,
        daw_core::EffectSettings::Compressor(_)
    ));
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.id)
            .collect::<HashSet<_>>()
            .len(),
        5
    );
    assert_eq!(app.effects_catalog.selection, selected);
    assert!(app.selected_effects.is_empty());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(app.session.project.tracks[0].effects.len(), 7);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(app.session.project.tracks[0].effects.len(), 7);
    assert_eq!(app.effects_catalog.selection, selected);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    app.dirty = false;
    library_key(&mut app, &ctx, egui::Key::Escape, false);
    assert!(app.effects_catalog.selection.is_empty());
    assert!(app.selected_effects.is_empty());
    assert_eq!(app.session.project.tracks[0].effects.len(), 7);
    assert!(!app.dirty);
}

#[test]
fn effects_library_keys_respect_focus_track_and_modal_guards() {
    for guard in 0..4 {
        let (mut app, _, ctx, _) = chain_fixture(false, false);
        let point = app.effects_bounds.drive_entry.center();
        click(&mut app, &ctx, point);
        match guard {
            0 => app.selected_track = None,
            1 => app.unsaved_prompt = true,
            2 => {
                ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("other control")));
            }
            _ => {
                app.set_view_mode(ViewMode::Timeline, &ctx);
            }
        }
        app.dirty = false;
        for key in [egui::Key::ArrowDown, egui::Key::Enter, egui::Key::Escape] {
            library_key(&mut app, &ctx, key, false);
        }
        assert!(app.effects_catalog.selection.is_empty());
        assert_eq!(app.session.project.tracks[0].effects.len(), 3);
        assert!(!app.dirty);
    }
}

#[test]
fn effects_library_drag_uses_reorder_preview_and_inserts_group_on_release() {
    for compact in [false, true] {
        for index in [0, 1, 3] {
            let (mut app, _, ctx, ids) = chain_fixture(compact, false);
            let point = app.effects_bounds.compressor_entry.center();
            click(&mut app, &ctx, point);
            let start = app.effects_bounds.drive_entry.center();
            click_effect_with_shift(&mut app, &ctx, start, true);
            end_click_sequence(&mut app, &ctx);
            app.dirty = false;
            let selected = app.effects_catalog.selection.clone();
            app.selected_effects = HashSet::from([ids[0]]);
            let original = app.session.project.tracks[0].effects.clone();
            let viewport = app.effects_bounds.lane_viewport;
            let end = Pos2::new(
                if index < 3 {
                    app.effects_bounds.chain[index].1.left() + 5.0
                } else {
                    app.effects_bounds.chain[2].1.right() + 10.0
                },
                viewport.center().y,
            );
            begin_effect_drag(&mut app, &ctx, start, end);
            assert!(
                app.effects_drag
                    .as_ref()
                    .is_some_and(|drag| drag.active && drag.is_library())
            );
            assert_eq!(app.session.project.tracks[0].effects, original);
            assert!(!app.dirty);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let expected_x = if index < 3 {
                app.effects_bounds.chain[index].1.left()
            } else {
                app.effects_bounds.chain[2].1.right()
            }
            .clamp(viewport.left() + 1.0, viewport.right() - 1.0);
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, stroke } if points[0].x == expected_x && points[1].x == expected_x && *stroke == Stroke::new(2.0, theme::accent(&ctx)) && points[0].y == viewport.top() && points[1].y == viewport.bottom()
            )));
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.effects_drag.is_none());
            let effects = &app.session.project.tracks[0].effects;
            assert_eq!(effects.len(), 5);
            assert!(matches!(
                effects[index].unit,
                daw_core::EffectSettings::Drive(_)
            ));
            assert!(matches!(
                effects[index + 1].unit,
                daw_core::EffectSettings::Compressor(_)
            ));
            assert_eq!(
                effects
                    .iter()
                    .filter(|effect| ids.contains(&effect.id))
                    .cloned()
                    .collect::<Vec<_>>(),
                original
            );
            assert_eq!(
                effects
                    .iter()
                    .map(|effect| effect.id)
                    .collect::<HashSet<_>>()
                    .len(),
                5
            );
            assert_eq!(app.effects_catalog.selection, selected);
            assert!(app.selected_effects.is_empty());
            assert!(app.dirty);
        }
    }
}

#[test]
fn effects_library_drag_handles_empty_lanes_fast_release_and_cancellation() {
    for cancel in [
        "drop", "outside", "escape", "focus", "modal", "track", "view", "chain",
    ] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        let start = app.effects_bounds.drive_entry.center();
        let end = app.effects_bounds.lane_viewport.center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        assert!(app.effects_drag.is_some());
        if cancel == "drop" {
            // A fast drag can move and release in the same next frame.
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end), button(end, false)],
            );
            assert_eq!(app.session.project.tracks[0].effects.len(), 1);
            assert!(matches!(
                app.session.project.tracks[0].effects[0].unit,
                daw_core::EffectSettings::Drive(_)
            ));
            library_key(&mut app, &ctx, egui::Key::Enter, false);
            assert_eq!(app.session.project.tracks[0].effects.len(), 2);
        } else {
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            match cancel {
                "escape" => library_key(&mut app, &ctx, egui::Key::Escape, false),
                "focus" => {
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
                "modal" => app.unsaved_prompt = true,
                "track" => app.selected_track = Some(app.session.project.add_track().unwrap()),
                "view" => app.view_mode = ViewMode::Timeline,
                "chain" => app.session.project.tracks[0]
                    .effects
                    .push(daw_core::EffectInstance::equalizer()),
                _ => {}
            }
            let point = if cancel == "outside" { start } else { end };
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, false)],
            );
            assert_eq!(
                app.session.project.tracks[0].effects.len(),
                usize::from(cancel == "chain")
            );
        }
        assert!(app.effects_drag.is_none());
    }
}

#[test]
fn effects_library_drag_scrolls_to_offscreen_insertion_slots() {
    let (mut app, _, ctx, _) = chain_fixture(false, false);
    for _ in 0..8 {
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::equalizer());
    }
    frame_sized(&mut app, &ctx, vec![], Vec2::new(640.0, 800.0));
    let start = app.effects_bounds.drive_entry.center();
    let viewport = app.effects_bounds.lane_viewport;
    let end = Pos2::new(viewport.right() - 3.0, viewport.center().y);
    let original = app.session.project.tracks[0].effects.clone();
    frame_sized(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        Vec2::new(640.0, 800.0),
    );
    frame_sized(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        Vec2::new(640.0, 800.0),
    );
    for _ in 0..50 {
        frame_sized(&mut app, &ctx, vec![], Vec2::new(640.0, 800.0));
    }
    assert!(app.effects_bounds.lane_scroll > 0.0);
    assert_eq!(app.session.project.tracks[0].effects, original);
    let index = app
        .effects_bounds
        .chain
        .iter()
        .position(|(_, rect)| end.x < rect.center().x)
        .unwrap_or(original.len());
    frame_sized(
        &mut app,
        &ctx,
        vec![button(end, false)],
        Vec2::new(640.0, 800.0),
    );
    let effects = &app.session.project.tracks[0].effects;
    assert_eq!(effects.len(), original.len() + 1);
    assert!(matches!(
        effects[index].unit,
        daw_core::EffectSettings::Drive(_)
    ));
    assert!(app.effects_drag.is_none());
}

#[test]
fn effects_lane_spans_the_window_with_one_status_separator_and_an_aligned_overview() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for size in [
            Vec2::new(640.0, 260.0),
            Vec2::new(640.0, 480.0),
            Vec2::new(1280.0, 800.0),
        ] {
            let (mut app, track) = fixture();
            let clip = app.session.project.tracks[0].clips[0].id;
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_clip(clip, track, false);
            app.set_view_mode(ViewMode::Edit, &ctx);
            frame_sized(&mut app, &ctx, vec![], size);
            let editor_ruler = app.midi_editor.ruler_bounds;
            let editor_lane = app.lane_bounds[&track];
            let original = format!("{:?}", app.session.project);
            app.set_view_mode(ViewMode::Effects, &ctx);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let catalog = app.effects_bounds.library;
            let header = app.effects_bounds.header;
            let rack = app.effects_bounds.strip;
            let scrollbar = app.effects_bounds.scrollbar;
            let ruler = app.midi_editor.ruler_bounds;
            let lane = app.lane_bounds[&track];
            assert_eq!(catalog.width(), TRACK_WIDTH);
            assert!((catalog.top() - header.top()).abs() < 0.1);
            assert!((header.bottom() - ruler.bottom()).abs() < 0.1);
            assert!(
                (catalog.bottom() - rack.top()).abs() < 0.1,
                "size={size:?} catalog={catalog:?} rack={rack:?} scrollbar={scrollbar:?}"
            );
            assert_eq!(rack.left(), catalog.left());
            assert_eq!(rack.width(), size.x);
            assert_eq!(rack.right(), editor_lane.right());
            assert!(rack.top() >= lane.bottom());
            if size.y >= 480.0 {
                assert_eq!(rack.height(), panels::unit_height(false));
                assert!(rack.top() > lane.bottom() + 20.0);
            } else {
                assert!(rack.height() > 0.0 && rack.height() < panels::unit_height(false));
            }
            assert_eq!(ruler.height(), editor_ruler.height());
            assert_eq!(lane.height(), editor_lane.height());
            assert!((ruler.left() - editor_ruler.left() - TRACK_WIDTH).abs() < 0.1);
            assert_eq!(ruler.left(), lane.left());
            assert_eq!(ruler.bottom(), lane.top());
            assert_eq!(scrollbar.left(), ruler.left());
            assert_eq!(scrollbar.right(), ruler.right());
            assert_eq!(scrollbar.height(), SCROLLBAR_HEIGHT);
            assert_eq!(scrollbar.bottom(), rack.top());
            assert!(scrollbar.top() >= lane.bottom());
            if scrollbar.top() > lane.bottom() {
                for tick in app.musical_timeline().ticks(lane.width()) {
                    let x = lane.left() + tick.x;
                    if x < lane.left() || x > lane.right() {
                        continue;
                    }
                    let line = [Pos2::new(x, lane.bottom()), Pos2::new(x, scrollbar.top())];
                    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, .. } if *points == line)));
                }
            }
            let palette = theme::palette(&ctx);
            let line = [
                Pos2::new(rack.left(), rack.top() + 0.5),
                Pos2::new(rack.right(), rack.top() + 0.5),
            ];
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment {points, stroke} if *points == line && *stroke == Stroke::new(1.0, palette.border))));
            let bottom_lines = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if points[0].x == rack.left()
                            && points[1].x == rack.right()
                            && points[0].y == points[1].y
                            && (points[0].y - rack.bottom()).abs() <= 1.0 =>
                    {
                        Some((points, stroke))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                bottom_lines.len(),
                1,
                "one separator between the lane and status bar"
            );
            assert_eq!(*bottom_lines[0].1, Stroke::new(1.0, palette.border));
            assert_eq!(bottom_lines[0].0[0].y, rack.bottom() + 0.5);
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rack && r.fill == palette.background)));
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == catalog && r.fill == palette.panel)));
            let border = [
                Pos2::new(catalog.right() - 0.5, catalog.top()),
                Pos2::new(catalog.right() - 0.5, catalog.bottom()),
            ];
            assert_eq!(shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke } if *points == border && *stroke == Stroke::new(1.0, palette.border))).count(), 1);
            assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Track effects are not available yet." || t.galley.text() == "Oscillator" || t.galley.text() == "Envelope")));
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn effects_overview_follows_the_processing_track_and_clears_missing_selection() {
    let (mut app, audio) = fixture();
    let audio_clip = app.session.project.tracks[0].clips[0].id;
    let midi = app.session.project.add_midi_track().unwrap();
    let midi_clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
    let ctx = context();
    app.select_clip(audio_clip, audio, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    app.select_track(midi, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(audio_clip));
    assert_eq!(app.lane_bounds.len(), 1);
    assert!(app.lane_bounds.contains_key(&midi));
    assert!(app.midi_editor.clip_bounds.contains_key(&midi_clip));
    assert!(!app.midi_editor.clip_bounds.contains_key(&audio_clip));
    app.selected_track = None;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(app.lane_bounds.is_empty());
    assert_eq!(app.timeline_bounds, Rect::NOTHING);
    assert!(app.midi_editor.clip_bounds.is_empty());
    assert!(app.effects_bounds.strip.is_positive());
    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Select a track to configure its effects.")));
    assert!(!app.dirty);
}

#[test]
fn effects_ruler_seek_loop_and_zoom_use_the_overview_viewport() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.zoom = 240.0;
    app.midi_editor.scroll = 0.5;
    let timeline = (app.zoom, app.scroll);
    frame(&mut app, &ctx, vec![]);
    let ruler = app.midi_editor.ruler_bounds;
    click(
        &mut app,
        &ctx,
        Pos2::new(ruler.left() + 180.0, ruler.bottom() - 6.0),
    );
    assert_eq!(app.session.project.transport.playhead_frame, 60_000);
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
    assert_eq!((region.start_frame, region.end_frame), (72_000, 120_000));
    assert!(app.dirty);
    let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
    let point = app.effects_bounds.strip.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
    let point = app.lane_bounds[&track].left_center() + Vec2::new(120.0, 0.0);
    let anchor = app.view_scroll() + 120.0 / f64::from(app.view_zoom());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!(app.view_zoom(), 480.0);
    assert!((app.view_scroll() + 120.0 / f64::from(app.view_zoom()) - anchor).abs() < 0.001);
    let grid_point = Pos2::new(
        point.x,
        (app.lane_bounds[&track].bottom() + app.effects_bounds.scrollbar.top()) * 0.5,
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(grid_point),
            egui::Event::Zoom(1.25),
        ],
    );
    assert_eq!(app.view_zoom(), 600.0);
    app.set_view_scroll(0.0);
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let project = format!("{:?}", app.session.project);
    let start = app.effects_bounds.scrollbar.left_center() + Vec2::new(30.0, -1.0);
    let end = start + Vec2::new(80.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(app.view_scroll() > 0.0);
    assert_eq!(app.lane_bounds[&track], lane);
    assert_eq!(format!("{:?}", app.session.project), project);
    assert!(!app.dirty);
    assert_eq!((app.zoom, app.scroll), timeline);
}

fn eq_settings(app: &DawUi) -> &daw_core::EqSettings {
    let daw_core::EffectSettings::Eq3(settings) = &app.session.project.tracks[0].effects[0].unit
    else {
        panic!("expected EQ")
    };
    settings
}

#[test]
fn effect_catalog_colors_follow_hover_and_selection_without_editing_the_project() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Yellow,
        ] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    accent,
                    ..Default::default()
                },
            );
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            let project = serde_json::to_string(&app.session.project).unwrap();
            app.dirty = false;
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerGone],
                Vec2::new(1280.0, 800.0),
            );
            let point = app.effects_bounds.entry.center();
            let palette = theme::palette(&ctx);
            let check = |shapes: &[egui::epaint::ClippedShape],
                         entry: Rect,
                         color: egui::Color32,
                         selected: bool| {
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == "3-band EQ" => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(text.override_text_color, Some(color));
                assert_eq!(
                    text.pos,
                    entry.min + Vec2::splat(f32::from(theme::TOOLBAR_PADDING))
                );
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.font_id.size == 11.0)
                );
                let fill = theme::toggle_colors(&ctx, true).0;
                assert_eq!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == entry && rect.fill == fill)), selected);
            };
            check(&shapes, app.effects_bounds.entry, palette.secondary, false);
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point)],
                Vec2::new(1280.0, 800.0),
            );
            check(&shapes, app.effects_bounds.entry, palette.text, false);
            click(&mut app, &ctx, point);
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerGone],
                Vec2::new(1280.0, 800.0),
            );
            check(&shapes, app.effects_bounds.entry, palette.text, true);
            assert_eq!(
                app.effects_catalog.selection,
                HashSet::from([daw_core::EQ_TYPE_ID])
            );
            assert!(app.session.project.tracks[0].effects.is_empty());
            assert_eq!(
                serde_json::to_string(&app.session.project).unwrap(),
                project
            );
            assert!(!app.dirty);
        }
    }
}
#[test]
fn eq_catalog_adds_on_double_click_and_controls_save_independent_states() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let entry = app.effects_bounds.entry.center();
    app.dirty = false;
    click(&mut app, &ctx, entry);
    assert!(app.session.project.tracks[0].effects.is_empty());
    assert!(!app.dirty);
    // Second click is the double-click release; only one instance is added.
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.tracks[0].effects.len(), 1);
    assert_eq!(eq_settings(&app).bands[0].frequency_hz, 250.0);
    assert_eq!(eq_settings(&app).bands[2].frequency_hz, 2500.0);
    assert!(app.dirty);
    let unit = &app.effects_bounds.units[0];
    assert_eq!(unit.knobs.len(), 3);
    for index in [0, 2] {
        assert!(unit.knobs[index].iter().all(Rect::is_positive));
    }
    assert!(unit.knobs[1][0].is_positive());
    assert_eq!(&unit.knobs[1][1..], &[Rect::NOTHING; 2]);
    assert_eq!(unit.bands[0].center().y, unit.bands[1].center().y);
    assert_eq!(unit.bands[1].center().y, unit.bands[2].center().y);
    let band = unit.bands[1].center();
    let original = eq_settings(&app).bands[1];
    click(&mut app, &ctx, band);
    let cut = eq_settings(&app).bands[1];
    assert!(!cut.enabled);
    assert_eq!(cut.gain_db, original.gain_db);
    assert_eq!(cut.frequency_hz, original.frequency_hz);
    assert_eq!(cut.q, original.q);
    let power = app.effects_bounds.units[0].power.center();
    click(&mut app, &ctx, power);
    assert!(!eq_settings(&app).enabled);
    assert!(!eq_settings(&app).bands[1].enabled);
    let compact = app.effects_bounds.compact.center();
    click(&mut app, &ctx, compact);
    frame(&mut app, &ctx, vec![]);
    assert!(app.session.project.tracks[0].effects_compact);
    assert_eq!(app.effects_bounds.strip.height(), ROW_HEIGHT);
    assert_eq!(app.effects_bounds.units[0].knobs, [[Rect::NOTHING; 3]; 3]);
    assert!(
        app.effects_bounds.units[0]
            .bands
            .iter()
            .all(Rect::is_positive)
    );
    let header = app.effects_bounds.units[0].header.center();
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, header);
    click(&mut app, &ctx, header);
    frame(&mut app, &ctx, vec![]);
    assert!(eq_settings(&app).collapsed);
    assert!(app.effects_bounds.units[0].power.is_positive());
    assert_eq!(app.effects_bounds.units[0].bands, [Rect::NOTHING; 3]);
    let saved = serde_json::to_string(&app.session.project).unwrap();
    let restored: daw_core::Project = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        restored.tracks[0].effects,
        app.session.project.tracks[0].effects
    );
    assert!(restored.tracks[0].effects_compact);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(eq_settings(&app).collapsed);
    assert!(!eq_settings(&app).enabled);
}
#[test]
fn eq_addition_is_guarded_without_a_track_during_disabled_editing_and_focus_loss() {
    for mode in 0..3 {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        if mode == 0 {
            app.selected_track = None;
        }
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.entry.center();
        if mode == 1 {
            app.unsaved_prompt = true;
        }
        app.dirty = false;
        if mode == 2 {
            for pressed in [true, false, true, false] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        events: vec![egui::Event::PointerMoved(entry), button(entry, pressed)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
        } else {
            click(&mut app, &ctx, entry);
            click(&mut app, &ctx, entry);
        }
        assert!(app.session.project.tracks[0].effects.is_empty());
        assert!(!app.dirty);
    }
}

fn end_click_sequence(app: &mut DawUi, ctx: &egui::Context) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|input| input.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
}
#[test]
fn eq_knobs_edit_saved_parameters_and_multiple_instances_have_distinct_identity() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    app.session.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::equalizer());
    frame(&mut app, &ctx, vec![]);
    for row in 0..3 {
        let point = app.effects_bounds.units[0].knobs[0][row].center();
        let before = eq_settings(&app).bands[0];
        let end = point - Vec2::new(0.0, 30.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let after = eq_settings(&app).bands[0];
        let values = |band: daw_core::EqBandSettings| [band.gain_db, band.frequency_hz, band.q];
        assert!(
            values(after)[row] > values(before)[row],
            "row={row} rect={:?} before={before:?} after={after:?}",
            app.effects_bounds.units[0].knobs[0][row]
        );
        assert!(after.valid());
    }
    let point = app.effects_bounds.units[0].knobs[1][0].center();
    let before = eq_settings(&app).bands[1];
    let end = point - Vec2::new(0.0, 30.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let after = eq_settings(&app).bands[1];
    assert!(after.gain_db > before.gain_db);
    assert_eq!(after.frequency_hz, before.frequency_hz);
    assert_eq!(after.q, before.q);
    end_click_sequence(&mut app, &ctx);
    let entry = app.effects_bounds.entry.center();
    click(&mut app, &ctx, entry);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    let effects = &app.session.project.tracks[0].effects;
    assert_eq!(effects.len(), 2);
    assert_ne!(effects[0].id, effects[1].id);
    assert_eq!(app.effects_bounds.units.len(), 2);
    let other = app.session.project.add_track().unwrap();
    app.select_track(other, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.units.is_empty());
    app.select_track(track, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.units.len(), 2);
}

#[test]
fn eq_footer_pairs_match_oscillator_spacing_in_both_modes_and_share_the_status_separator() {
    for compact in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::equalizer());
        app.session.project.tracks[0].effects_compact = compact;
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for (index, label) in ["Low", "Mid", "High"].into_iter().enumerate() {
            let button = app.effects_bounds.units[0].bands[index];
            let text = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                    _ => None,
                })
                .unwrap();
            let text_bounds = text.galley.rect.translate(text.pos.to_vec2());
            assert!((text_bounds.left() - button.right() - 8.0).abs() < 0.51);
            assert!((text_bounds.center().y - button.center().y).abs() < 0.51);
            assert_eq!(button.size(), Vec2::splat(icons::COMPACT_BUTTON_SIZE));
            assert_eq!(
                text.galley.job.sections[0].format.font_id.size,
                panels::HEADER_FONT_SIZE
            );
        }
        // The unit frame reserves its border geometry but omits the bottom stroke.
        let rack = app.effects_bounds.strip;
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.width > 0.0 && rect.stroke.color != egui::Color32::TRANSPARENT && (rect.rect.bottom() - rack.bottom()).abs() < 0.1 && rect.rect.width() > 200.0)));
    }
}

fn drive_settings(app: &DawUi) -> &daw_core::DriveSettings {
    let daw_core::EffectSettings::Drive(settings) = &app.session.project.tracks[0].effects[0].unit
    else {
        panic!("expected Drive")
    };
    settings
}
#[test]
fn drive_catalog_adds_independent_units_and_keeps_the_existing_eq() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let entry = app.effects_bounds.drive_entry.center();
    assert!(entry.y > app.effects_bounds.entry.bottom());
    app.dirty = false;
    click(&mut app, &ctx, entry);
    assert_eq!(
        app.effects_catalog.selection,
        HashSet::from([daw_core::DRIVE_TYPE_ID])
    );
    assert!(app.session.project.tracks[0].effects.is_empty());
    assert!(!app.dirty);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.tracks[0].effects.len(), 1);
    assert_eq!(drive_settings(&app), &daw_core::DriveSettings::default());
    assert!(app.dirty);
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, entry);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    let effects = &app.session.project.tracks[0].effects;
    assert_eq!(effects.len(), 2);
    assert_ne!(effects[0].id, effects[1].id);
    end_click_sequence(&mut app, &ctx);
    let eq_entry = app.effects_bounds.entry.center();
    click(&mut app, &ctx, eq_entry);
    click(&mut app, &ctx, eq_entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.drive_units.len(), 2);
    assert_eq!(app.effects_bounds.units.len(), 1);
    let other = app.session.project.add_track().unwrap();
    app.select_track(other, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.drive_units.is_empty());
    app.select_track(track, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.drive_units.len(), 2);
}
#[test]
fn drive_controls_edit_and_reset_saved_settings_and_off_color_retains_its_parameters() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    // Off prevents editing the two inactive color parameters.
    let before = drive_settings(&app).clone();
    for row in [1, 2] {
        let point = app.effects_bounds.drive_units[0].knobs[1][row].center();
        let end = point - Vec2::new(0.0, 30.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
    }
    assert_eq!(drive_settings(&app), &before);
    // The shape selector uses the shared cyclic knob and keyboard behavior.
    let point = app.effects_bounds.drive_units[0].knobs[1][0].center();
    click(&mut app, &ctx, point);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(
        drive_settings(&app).color_shape,
        daw_core::DriveColorShape::MidScoop
    );
    for (strip, row) in [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2)] {
        let before = drive_settings(&app).clone();
        let point = app.effects_bounds.drive_units[0].knobs[strip][row].center();
        let end = point - Vec2::new(0.0, 20.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let after = drive_settings(&app);
        let values = |settings: &daw_core::DriveSettings| {
            [
                settings.volume_db,
                settings.gain,
                settings.tone,
                settings.color_strength,
                settings.color_frequency_hz,
            ]
        };
        let index = if strip == 0 { row } else { row + 2 };
        assert!(
            values(after)[index] > values(&before)[index],
            "strip={strip} row={row}"
        );
        assert!(after.valid());
    }
    let saved = serde_json::to_string(&app.session.project).unwrap();
    let loaded: daw_core::Project = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        loaded.tracks[0].effects,
        app.session.project.tracks[0].effects
    );
    assert!(app.dirty);
    end_click_sequence(&mut app, &ctx);
    let point = app.effects_bounds.drive_units[0].knobs[0][1].center();
    click(&mut app, &ctx, point);
    click(&mut app, &ctx, point);
    assert_eq!(drive_settings(&app).gain, 0.2);
}
#[test]
fn drive_labels_and_knobs_align_with_eq_and_both_presentation_modes_persist() {
    for compact in [false, true] {
        for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.session.project.tracks[0].effects = vec![
                daw_core::EffectInstance::drive(),
                daw_core::EffectInstance::equalizer(),
            ];
            app.session.project.tracks[0].effects_compact = compact;
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let drive = &app.effects_bounds.drive_units[0];
            let eq = &app.effects_bounds.units[0];
            assert_eq!(drive.header.height(), eq.header.height());
            assert!((drive.labels[0].center().y - eq.bands[0].center().y).abs() < 0.51);
            assert!((drive.labels[1].center().y - drive.labels[0].center().y).abs() < 0.51);
            assert_eq!(
                (drive.labels[1].center().x - drive.labels[0].center().x).round(),
                crate::midi_editor::UNIT_STRIP_WIDTH
            );
            for label in ["Drive", "Color"] {
                assert!(
                    shapes.iter().any(
                        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label)
                    )
                );
            }
            for strip in 0..2 {
                for row in 0..3 {
                    if compact {
                        assert_eq!(drive.knobs[strip][row], Rect::NOTHING);
                    } else {
                        assert!(
                            (drive.knobs[strip][row].center().y - eq.knobs[0][row].center().y)
                                .abs()
                                < 0.1
                        );
                    }
                }
            }
            let power = drive.power.center();
            click(&mut app, &ctx, power);
            assert!(!drive_settings(&app).enabled);
            assert!(!drive_settings(&app).collapsed);
            end_click_sequence(&mut app, &ctx);
            let header = app.effects_bounds.drive_units[0].header.center();
            click(&mut app, &ctx, header);
            click(&mut app, &ctx, header);
            assert!(drive_settings(&app).collapsed);
            let saved = serde_json::to_string(&app.session.project).unwrap();
            let restored: daw_core::Project = serde_json::from_str(&saved).unwrap();
            assert_eq!(
                restored.tracks[0].effects,
                app.session.project.tracks[0].effects
            );
            assert_eq!(restored.tracks[0].effects_compact, compact);
            app.set_view_mode(ViewMode::Timeline, &ctx);
            app.set_view_mode(ViewMode::Effects, &ctx);
            frame(&mut app, &ctx, vec![]);
            assert!(drive_settings(&app).collapsed);
            assert!(!drive_settings(&app).enabled);
        }
    }
}
#[test]
fn drive_catalog_is_guarded_without_selection_and_during_modal_work() {
    for no_selection in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.drive_entry.center();
        if no_selection {
            app.selected_track = None;
        } else {
            app.unsaved_prompt = true;
        }
        app.dirty = false;
        click(&mut app, &ctx, entry);
        click(&mut app, &ctx, entry);
        assert!(app.session.project.tracks[0].effects.is_empty());
        assert!(!app.dirty);
    }
}

fn chain_fixture(compact: bool, collapsed: bool) -> (DawUi, Id, egui::Context, Vec<Id>) {
    let (mut app, track) = fixture();
    let ctx = context();
    let mut eq = daw_core::EffectInstance::equalizer();
    let daw_core::EffectSettings::Eq3(settings) = &mut eq.unit else {
        unreachable!()
    };
    settings.collapsed = collapsed;
    settings.enabled = false;
    settings.bands[0].gain_db = -9.0;
    let mut drive = daw_core::EffectInstance::drive();
    let daw_core::EffectSettings::Drive(settings) = &mut drive.unit else {
        unreachable!()
    };
    settings.collapsed = collapsed;
    settings.color_shape = daw_core::DriveColorShape::MidHump;
    settings.gain = 0.7;
    app.session.project.tracks[0].effects = vec![eq, drive, daw_core::EffectInstance::equalizer()];
    app.session.project.tracks[0].effects_compact = compact;
    let ids = app.session.project.tracks[0]
        .effects
        .iter()
        .map(|unit| unit.id)
        .collect();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, track, ctx, ids)
}
fn begin_effect_drag(app: &mut DawUi, ctx: &egui::Context, point: Pos2, end: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
}
fn chain_ids(app: &DawUi) -> Vec<Id> {
    app.session.project.tracks[0]
        .effects
        .iter()
        .map(|unit| unit.id)
        .collect()
}
#[test]
fn effect_headers_preview_and_commit_order_on_release_in_all_presentation_modes() {
    for compact in [false, true] {
        for collapsed in [false, true] {
            for source in [0, 2] {
                let (mut app, track, ctx, ids) = chain_fixture(compact, collapsed);
                let before = app.session.project.tracks[0].effects.clone();
                let selected = app.selected_clips.clone();
                let header = if source == 0 {
                    app.effects_bounds.units[0].header
                } else {
                    app.effects_bounds.units[1].header
                };
                // Use the title below the cap in a collapsed strip; exclude the power control.
                let start = header.center();
                let chain = &app.effects_bounds.chain;
                let end = Pos2::new(
                    if source == 0 {
                        chain.last().unwrap().1.right() + 10.0
                    } else {
                        chain[0].1.left() + 2.0
                    },
                    start.y,
                );
                begin_effect_drag(&mut app, &ctx, start, end);
                assert_eq!(
                    app.effects_drag
                        .as_ref()
                        .map(|drag| (drag.track, drag.chain_unit().unwrap())),
                    Some((track, ids[source]))
                );
                assert!(!app.file_action_enabled(FileAction::Import));
                assert!(!app.view_switch_enabled());
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if points[0].x == points[1].x && *stroke == Stroke::new(2.0, theme::accent(&ctx)) && points[0].y >= app.effects_bounds.strip.top())));
                assert_eq!(app.session.project.tracks[0].effects, before);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![button(end, false)]);
                assert!(app.effects_drag.is_none());
                let expected = if source == 0 {
                    vec![ids[1], ids[2], ids[0]]
                } else {
                    vec![ids[2], ids[0], ids[1]]
                };
                assert_eq!(chain_ids(&app), expected);
                assert!(app.dirty);
                assert_eq!(app.selected_track, Some(track));
                assert_eq!(app.selected_clips, selected);
                for unit in &app.session.project.tracks[0].effects {
                    assert!(
                        before.contains(unit),
                        "move must preserve all settings and IDs"
                    );
                }
                let restored: daw_core::Project =
                    serde_json::from_str(&serde_json::to_string(&app.session.project).unwrap())
                        .unwrap();
                assert_eq!(
                    restored.tracks[0].effects,
                    app.session.project.tracks[0].effects
                );
                assert_eq!(restored.tracks[0].effects_compact, compact);
            }
        }
    }
}
#[test]
fn effect_reorder_cancels_and_same_position_drops_leave_the_project_clean() {
    for mode in [
        "escape", "focus", "outside", "same", "modal", "track", "changed",
    ] {
        let (mut app, _, ctx, ids) = chain_fixture(false, false);
        let original = app.session.project.tracks[0].effects.clone();
        let start = app.effects_bounds.units[0].header.center();
        let end = if mode == "same" {
            start + Vec2::new(10.0, 0.0)
        } else if mode == "outside" {
            start - Vec2::new(0.0, 40.0)
        } else {
            Pos2::new(
                app.effects_bounds.chain.last().unwrap().1.right() + 10.0,
                start.y,
            )
        };
        begin_effect_drag(&mut app, &ctx, start, end);
        assert!(app.effects_drag.is_some());
        match mode {
            "escape" => {
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
            "focus" => {
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
            "modal" => {
                app.unsaved_prompt = true;
            }
            "track" => {
                let other = app.session.project.add_track().unwrap();
                app.select_track(other, false);
            }
            "changed" => {
                app.session.project.tracks[0].effects.swap(1, 2);
            }
            _ => {}
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(app.effects_drag.is_none(), "{mode}");
        if mode == "changed" {
            assert_eq!(chain_ids(&app), [ids[0], ids[2], ids[1]]);
        } else {
            assert_eq!(app.session.project.tracks[0].effects, original, "{mode}");
        }
        assert!(!app.dirty, "{mode}");
    }
}
#[test]
fn effect_power_and_knob_drags_do_not_start_reordering_and_secondary_drags_do_not_move_units() {
    let (mut app, _, ctx, ids) = chain_fixture(false, false);
    for start in [
        app.effects_bounds.units[0].power.center(),
        app.effects_bounds.units[0].knobs[0][0].center(),
    ] {
        let end = start + Vec2::new(200.0, 0.0);
        begin_effect_drag(&mut app, &ctx, start, end);
        assert!(app.effects_drag.is_none());
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(chain_ids(&app), ids);
    }
    let start = app.effects_bounds.units[0].header.center();
    let end = start + Vec2::new(400.0, 0.0);
    let secondary = |point, pressed| egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), secondary(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![secondary(end, false)]);
    assert!(app.effects_drag.is_none());
    assert_eq!(chain_ids(&app), ids);
}
#[test]
fn effect_header_drag_scrolls_at_lane_edges_and_reaches_offscreen_units() {
    let (mut app, _, ctx, _) = chain_fixture(false, false);
    for _ in 0..8 {
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::equalizer());
    }
    frame_sized(&mut app, &ctx, vec![], Vec2::new(640.0, 800.0));
    let start = app.effects_bounds.units[0].header.center();
    let viewport = app.effects_bounds.lane_viewport;
    let end = Pos2::new(viewport.right() - 3.0, start.y);
    frame_sized(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        Vec2::new(640.0, 800.0),
    );
    frame_sized(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        Vec2::new(640.0, 800.0),
    );
    assert!(app.effects_drag.is_some());
    for _ in 0..50 {
        frame_sized(&mut app, &ctx, vec![], Vec2::new(640.0, 800.0));
    }
    assert!(app.effects_bounds.lane_scroll > 0.0);
    assert!(app.effects_bounds.chain[0].1.right() < viewport.right());
    let moved = app.session.project.tracks[0].effects[0].id;
    frame_sized(
        &mut app,
        &ctx,
        vec![button(end, false)],
        Vec2::new(640.0, 800.0),
    );
    assert_ne!(chain_ids(&app)[0], moved);
    assert!(app.dirty);
}

fn click_effect_with_shift(app: &mut DawUi, ctx: &egui::Context, point: Pos2, shift: bool) {
    for pressed in [true, false] {
        let modifiers = egui::Modifiers {
            shift,
            ..Default::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
    }
    frame(
        app,
        ctx,
        vec![egui::Event::ModifiersChanged(Default::default())],
    );
}
#[test]
fn effect_unit_click_selection_and_shift_toggle_share_the_track_outline_without_project_edits() {
    for compact in [false, true] {
        for collapsed in [false, true] {
            for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
                let (mut app, _, ctx, ids) = chain_fixture(compact, collapsed);
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                frame(&mut app, &ctx, vec![]);
                let before = app.session.project.clone();
                let first = app.effects_bounds.units[0].header.center();
                let second = app.effects_bounds.drive_units[0].header.center();
                click_effect_with_shift(&mut app, &ctx, first, false);
                assert_eq!(app.selected_effects, HashSet::from([ids[0]]));
                click_effect_with_shift(&mut app, &ctx, second, true);
                assert_eq!(app.selected_effects, HashSet::from([ids[0], ids[1]]));
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                for (id, rect) in &app.effects_bounds.chain {
                    let outlined = shapes.iter().any(|s| {
                        matches!(&s.shape, egui::Shape::Rect(r)
                        if r.rect == *rect && r.stroke == theme::selection_outline(&ctx)
                            && r.corner_radius == egui::CornerRadius::same(2))
                    });
                    assert_eq!(outlined, app.selected_effects.contains(id));
                }
                // Break the click sequence so the next Shift click does not collapse a header.
                end_click_sequence(&mut app, &ctx);
                click_effect_with_shift(&mut app, &ctx, second, true);
                assert_eq!(app.selected_effects, HashSet::from([ids[0]]));
                end_click_sequence(&mut app, &ctx);
                click_effect_with_shift(&mut app, &ctx, second, true);
                click_effect_with_shift(&mut app, &ctx, first, false);
                assert_eq!(app.selected_effects, HashSet::from([ids[0]]));
                end_click_sequence(&mut app, &ctx);
                click_effect_with_shift(&mut app, &ctx, first, true);
                assert!(app.selected_effects.is_empty());
                assert_eq!(format!("{:?}", app.session.project), format!("{before:?}"));
                assert!(!app.dirty);
            }
        }
    }
}
#[test]
fn effect_body_selection_keeps_child_controls_and_reordering_and_clears_on_view_changes() {
    let (mut app, _, ctx, ids) = chain_fixture(false, false);
    let rect = app.effects_bounds.chain[0].1;
    let blank = Pos2::new(rect.left() + 6.0, rect.center().y);
    click(&mut app, &ctx, blank);
    assert_eq!(app.selected_effects, HashSet::from([ids[0]]));
    let second = app.effects_bounds.drive_units[0].header.center();
    click_effect_with_shift(&mut app, &ctx, second, true);
    let selected = app.selected_effects.clone();
    let start = app.effects_bounds.units[0].header.center();
    let end = Pos2::new(
        app.effects_bounds.chain.last().unwrap().1.right() + 10.0,
        start.y,
    );
    begin_effect_drag(&mut app, &ctx, start, end);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.selected_effects, selected);
    assert_eq!(chain_ids(&app), [ids[2], ids[0], ids[1]]);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.selected_effects.is_empty());
    // A power click still changes enable state and selects its owning unit.
    let power = app.effects_bounds.drive_units[0].power.center();
    click(&mut app, &ctx, power);
    let daw_core::EffectSettings::Drive(settings) = &app.session.project.tracks[0].effects[2].unit
    else {
        panic!("expected moved Drive");
    };
    assert!(!settings.enabled);
    assert_eq!(app.selected_effects, HashSet::from([ids[1]]));
    app.session.project.tracks[0]
        .effects
        .retain(|effect| effect.id != ids[1]);
    frame(&mut app, &ctx, vec![]);
    assert!(app.selected_effects.is_empty());
}
#[test]
fn effect_unit_selection_clears_when_disabled_or_unfocused_without_editing_units() {
    for unfocused in [false, true] {
        let (mut app, _, ctx, ids) = chain_fixture(false, false);
        app.selected_effects.insert(ids[0]);
        let point = app.effects_bounds.drive_units[0].header.center();
        if !unfocused {
            app.unsaved_prompt = true;
        }
        let original = app.session.project.clone();
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: !unfocused,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    events: vec![egui::Event::PointerMoved(point), button(point, pressed)],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        assert!(app.selected_effects.is_empty());
        assert_eq!(
            format!("{:?}", app.session.project),
            format!("{original:?}")
        );
        assert!(!app.dirty);
    }
}

#[test]
fn dragging_selected_effects_preserves_chain_order_for_contiguous_and_separated_groups() {
    for compact in [false, true] {
        for collapsed in [false, true] {
            for forward in [false, true] {
                for separated in [false, true] {
                    let (mut app, _, ctx, _) = chain_fixture(compact, collapsed);
                    app.session.project.tracks[0].effects.extend([
                        daw_core::EffectInstance::drive(),
                        daw_core::EffectInstance::equalizer(),
                    ]);
                    frame(&mut app, &ctx, vec![]);
                    let ids = chain_ids(&app);
                    let selected = if forward {
                        [ids[0], ids[if separated { 2 } else { 1 }]]
                    } else {
                        [ids[if separated { 2 } else { 3 }], ids[4]]
                    };
                    // Deliberately select in reverse order; chain order owns movement order.
                    app.select_effect(selected[1], false);
                    app.select_effect(selected[0], true);
                    let before = app.session.project.tracks[0].effects.clone();
                    let index = if forward { 0 } else { 4 };
                    let rect = app.effects_bounds.chain[index].1;
                    let start = if forward {
                        app.effects_bounds.units[0].header.center()
                    } else {
                        Pos2::new(rect.center().x, rect.top() + 12.0)
                    };
                    let end = Pos2::new(
                        if forward {
                            app.effects_bounds.chain.last().unwrap().1.right() + 10.0
                        } else {
                            app.effects_bounds.chain[0].1.left() + 2.0
                        },
                        start.y,
                    );
                    begin_effect_drag(&mut app, &ctx, start, end);
                    assert_eq!(app.session.project.tracks[0].effects, before);
                    assert!(!app.dirty);
                    frame(&mut app, &ctx, vec![button(end, false)]);
                    let moving: Vec<_> = ids
                        .iter()
                        .copied()
                        .filter(|id| selected.contains(id))
                        .collect();
                    let rest: Vec<_> = ids
                        .iter()
                        .copied()
                        .filter(|id| !selected.contains(id))
                        .collect();
                    let expected: Vec<_> = if forward {
                        rest.into_iter().chain(moving).collect()
                    } else {
                        moving.into_iter().chain(rest).collect()
                    };
                    assert_eq!(chain_ids(&app), expected);
                    assert_eq!(app.selected_effects, HashSet::from(selected));
                    for unit in &app.session.project.tracks[0].effects {
                        assert!(before.contains(unit));
                    }
                    assert!(app.dirty);
                }
            }
        }
    }
}
#[test]
fn selected_effect_group_same_position_and_cancelled_moves_leave_order_and_selection_intact() {
    for cancel in [false, true] {
        let (mut app, _, ctx, ids) = chain_fixture(false, false);
        app.selected_effects = HashSet::from([ids[0], ids[1]]);
        let start = app.effects_bounds.units[0].header.center();
        let end = if cancel {
            Pos2::new(
                app.effects_bounds.chain.last().unwrap().1.right() + 10.0,
                start.y,
            )
        } else {
            start + Vec2::new(10.0, 0.0)
        };
        begin_effect_drag(&mut app, &ctx, start, end);
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
        assert_eq!(chain_ids(&app), ids);
        assert_eq!(app.selected_effects, HashSet::from([ids[0], ids[1]]));
        assert!(!app.dirty);
    }
}

#[test]
fn empty_effects_lane_click_clears_unit_selection_without_editing_the_project() {
    for compact in [false, true] {
        let (mut app, _, ctx, ids) = chain_fixture(compact, false);
        app.selected_effects = HashSet::from([ids[0], ids[1]]);
        let before = format!("{:?}", app.session.project);
        let strip = app.effects_bounds.strip;
        let empty = Pos2::new(strip.right() - 20.0, strip.center().y);
        // Moving focus to the catalog clears installed-unit selection.
        let catalog = app.effects_bounds.entry.center();
        click(&mut app, &ctx, catalog);
        assert!(app.selected_effects.is_empty());
        click(&mut app, &ctx, empty);
        assert!(app.selected_effects.is_empty());
        assert_eq!(format!("{:?}", app.session.project), before);
        assert!(!app.dirty);
    }
}
#[test]
fn effect_header_fast_drag_can_commit_when_motion_and_release_arrive_in_one_frame() {
    let (mut app, _, ctx, ids) = chain_fixture(false, false);
    let start = app.effects_bounds.units[0].header.center();
    let end = Pos2::new(
        app.effects_bounds.chain.last().unwrap().1.right() + 10.0,
        start.y,
    );
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
    assert_eq!(chain_ids(&app), [ids[1], ids[2], ids[0]]);
    assert_eq!(app.selected_effects, HashSet::from([ids[0]]));
    assert!(app.dirty);
}

fn auto_filter_settings(app: &DawUi) -> &daw_core::AutoFilterSettings {
    let daw_core::EffectSettings::AutoFilter(settings) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        panic!("expected Auto-filter")
    };
    settings
}
#[test]
fn auto_filter_catalog_adds_independent_saved_units_and_respects_guards() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let entry = app.effects_bounds.auto_filter_entry.center();
    assert!(entry.y > app.effects_bounds.drive_entry.bottom());
    app.dirty = false;
    click(&mut app, &ctx, entry);
    assert_eq!(
        app.effects_catalog.selection,
        HashSet::from([daw_core::AUTO_FILTER_TYPE_ID])
    );
    assert!(!app.dirty);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        auto_filter_settings(&app),
        &daw_core::AutoFilterSettings::default()
    );
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, entry);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.auto_filter_units.len(), 2);
    assert_ne!(
        app.session.project.tracks[0].effects[0].id,
        app.session.project.tracks[0].effects[1].id
    );
    for blocked in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        if blocked {
            app.unsaved_prompt = true;
        } else {
            app.selected_track = None;
        }
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.auto_filter_entry.center();
        click(&mut app, &ctx, entry);
        click(&mut app, &ctx, entry);
        assert!(app.session.project.tracks[0].effects.is_empty());
    }
}
#[test]
fn auto_filter_has_three_strips_with_evenly_spaced_lfo_columns_and_a_centered_footer() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.session.project.tracks[0].effects = vec![
                daw_core::EffectInstance::auto_filter(),
                daw_core::EffectInstance::drive(),
            ];
            app.session.project.tracks[0].effects_compact = compact;
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let bounds = &app.effects_bounds.auto_filter_units[0];
            assert_eq!(
                bounds.header.height(),
                app.effects_bounds.drive_units[0].header.height()
            );
            assert!((bounds.sections[0].width() - midi_editor::UNIT_STRIP_WIDTH).abs() < 0.1);

            assert!((bounds.sections[2].width() - midi_editor::UNIT_STRIP_WIDTH).abs() < 0.1);
            let text = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "LFO" => Some(text),
                    _ => None,
                })
                .unwrap();
            let text_rect = text.galley.rect.translate(text.pos.to_vec2());
            assert!((text_rect.left() - bounds.lfo.right() - 8.0).abs() < 0.51);
            assert!(
                ((bounds.lfo.left() + text_rect.right()) * 0.5 - bounds.sections[1].center().x)
                    .abs()
                    < 0.51
            );
            assert_eq!(bounds.lfo.size(), Vec2::splat(icons::COMPACT_BUTTON_SIZE));
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { points, .. } if points[0].x == points[1].x && (points[0].x - bounds.sections[1].center().x).abs() < 1.0 && (points[1].y - points[0].y).abs() > 15.0)));
            if !compact {
                for row in 0..3 {
                    let left = bounds.knobs[1][row];
                    let right = bounds.knobs[2][row];
                    let outer_left = left.left() - bounds.sections[1].left();
                    let gap = right.left() - left.right();
                    let outer_right = bounds.sections[1].right() - right.right();
                    assert!((gap - outer_left).abs() < 0.1);
                    assert!((gap - outer_right).abs() < 0.1);
                    assert!(
                        (outer_left - (bounds.knobs[0][row].left() - bounds.sections[0].left()))
                            .abs()
                            < 0.1
                    );
                }
                for strip in 0..4 {
                    for row in 0..3 {
                        assert!(
                            (bounds.knobs[strip][row].top() - bounds.knobs[0][row].top()).abs()
                                < 0.1
                        );
                    }
                }
                assert!(
                    (bounds.lfo.center().y
                        - app.effects_bounds.drive_units[0].labels[0].center().y)
                        .abs()
                        < 0.51
                );
            }
            let lfo = bounds.lfo.center();
            click(&mut app, &ctx, lfo);
            assert!(!auto_filter_settings(&app).lfo_enabled);
            assert!(auto_filter_settings(&app).enabled);
            let power = app.effects_bounds.auto_filter_units[0].power.center();
            click(&mut app, &ctx, power);
            assert!(!auto_filter_settings(&app).enabled);
            end_click_sequence(&mut app, &ctx);
            let header = app.effects_bounds.auto_filter_units[0].header.center();
            click(&mut app, &ctx, header);
            click(&mut app, &ctx, header);
            assert!(auto_filter_settings(&app).collapsed);
            let saved = serde_json::to_string(&app.session.project).unwrap();
            let restored: daw_core::Project = serde_json::from_str(&saved).unwrap();
            assert_eq!(
                restored.tracks[0].effects,
                app.session.project.tracks[0].effects
            );
        }
    }
}
#[test]
fn auto_filter_controls_edit_phases_depth_and_both_retained_rate_modes() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::auto_filter());
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    for (strip, row) in [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 2),
        (2, 0),
        (2, 1),
        (2, 2),
        (3, 0),
        (3, 1),
    ] {
        end_click_sequence(&mut app, &ctx);
        let before = auto_filter_settings(&app).clone();
        let point = app.effects_bounds.auto_filter_units[0].knobs[strip][row].center();
        let end = point + Vec2::new(0.0, if (strip, row) == (3, 1) { 30.0 } else { -30.0 });
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_ne!(
            *auto_filter_settings(&app),
            before,
            "strip={strip} row={row}"
        );
        assert!(auto_filter_settings(&app).valid());
    }
    let saved_note = auto_filter_settings(&app).note_rate;
    end_click_sequence(&mut app, &ctx);
    let mode = app.effects_bounds.auto_filter_units[0].knobs[1][1].center();
    click(&mut app, &ctx, mode);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(
        auto_filter_settings(&app).rate_mode,
        daw_core::LfoRateMode::Hz
    );
    assert_eq!(auto_filter_settings(&app).note_rate, saved_note);
    let point = app.effects_bounds.auto_filter_units[0].knobs[1][2].center();
    let end = point - Vec2::new(0.0, 30.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(auto_filter_settings(&app).rate_hz > 2.0);
    assert_eq!(auto_filter_settings(&app).note_rate, saved_note);
}

#[test]
fn auto_filter_note_rate_is_bounded_ordered_and_labeled_and_resets_to_a_quarter_note() {
    let (mut app, track) = fixture();
    let ctx = context();
    let mut effect = daw_core::EffectInstance::auto_filter();
    let daw_core::EffectSettings::AutoFilter(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.note_rate = daw_core::NoteRate(1); // Slowest: dotted 4 bars.
    app.session.project.tracks[0].effects.push(effect);
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for label in ["4b D", "1/64 T"] {
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label))
        );
    }
    let check_marker = |app: &mut DawUi| {
        let shapes = frame_shapes(app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let rect = app.effects_bounds.auto_filter_units[0].knobs[1][2];
        let pointer = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::LineSegment { points, .. }
                    if points.iter().all(|point| rect.contains(*point))
                        && (points[0].distance(points[1]) - 5.0).abs() < 0.01 =>
                {
                    Some((points[1] - rect.center()).normalized())
                }
                _ => None,
            })
            .unwrap();
        let marked = shapes.iter().any(|s| match &s.shape {
            egui::Shape::LineSegment { points, .. }
                if points.iter().all(|point| rect.contains(*point))
                    && points[0].distance(points[1]) <= 3.1 =>
            {
                let direction = (points[1] - rect.center()).normalized();
                direction.dot(pointer) > 0.99999
            }
            _ => false,
        });
        let label = auto_filter_settings(app).note_rate.label();
        assert_eq!(
            marked,
            !label.ends_with('D') && !label.ends_with('T'),
            "{label}"
        );
    };
    check_marker(&mut app);
    let rate = app.effects_bounds.auto_filter_units[0].knobs[1][2].center();
    click(&mut app, &ctx, rate);
    let key = |app: &mut DawUi, key| {
        for pressed in [true, false] {
            frame(
                app,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    };
    app.dirty = false;
    key(&mut app, egui::Key::ArrowDown);
    assert_eq!(auto_filter_settings(&app).note_rate, daw_core::NoteRate(1));
    assert!(!app.dirty);
    let mut seen = std::collections::HashSet::new();
    seen.insert(auto_filter_settings(&app).note_rate.0);
    for _ in 0..26 {
        let previous = auto_filter_settings(&app).note_rate.beats();
        key(&mut app, egui::Key::ArrowUp);
        let next = auto_filter_settings(&app).note_rate;
        assert!(next.beats() < previous, "previous={previous} next={next:?}");
        seen.insert(next.0);
        check_marker(&mut app);
    }
    assert_eq!(seen.len(), 27);
    assert_eq!(auto_filter_settings(&app).note_rate, daw_core::NoteRate(26));
    app.dirty = false;
    key(&mut app, egui::Key::ArrowUp);
    assert_eq!(auto_filter_settings(&app).note_rate, daw_core::NoteRate(26));
    assert!(!app.dirty);
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, rate);
    click(&mut app, &ctx, rate);
    assert_eq!(
        auto_filter_settings(&app).note_rate,
        daw_core::NoteRate::default()
    );
    assert_eq!(auto_filter_settings(&app).rate_hz, 2.0);
}

#[test]
fn auto_filter_diagrams_and_bounded_mode_selector_match_their_controls() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::auto_filter());
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let bounds = &app.effects_bounds.auto_filter_units[0];
        for (rect, count) in [(bounds.knobs[0][0], 4), (bounds.knobs[1][0], 5)] {
            let diagrams: Vec<_> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Path(path)
                        if path.stroke.width == 1.25
                            && path.points.iter().all(|point| rect.contains(*point)) =>
                    {
                        Some(path)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(diagrams.len(), count);
            assert_eq!(
                diagrams
                    .iter()
                    .filter(|path| path.stroke.color
                        == egui::epaint::ColorMode::Solid(theme::accent(&ctx)))
                    .count(),
                1
            );
        }
        for (rect, expected) in [(bounds.knobs[1][2], 9), (bounds.knobs[2][0], 7)] {
            let marks = shapes
                .iter()
                .filter(|s| {
                    matches!(&s.shape,
                        egui::Shape::LineSegment { points, .. }
                            if points.iter().all(|point| rect.contains(*point))
                            && points[0].distance(points[1]) <= 3.1
                    )
                })
                .count();
            assert_eq!(marks, expected);
        }
        for name in ["Low-pass", "Sine"] {
            assert!(
                !shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == name))
            );
        }
        let mode = bounds.knobs[1][1];
        for (name, left) in [("Hz", true), ("Note", false)] {
            let text = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == name => Some(t),
                    _ => None,
                })
                .unwrap();
            assert!((text.pos.y - mode.top()).abs() < 0.1);
            let text_rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            assert!(
                (if left {
                    text_rect.left() - mode.left()
                } else {
                    text_rect.right() - mode.right()
                })
                .abs()
                    < 0.1
            );
        }
        let note = auto_filter_settings(&app).note_rate;
        let hz = auto_filter_settings(&app).rate_hz;
        click(&mut app, &ctx, mode.center());
        let key = |app: &mut DawUi, key| {
            for pressed in [true, false] {
                frame(
                    app,
                    &ctx,
                    vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
        };
        for (key_code, expected) in [
            (egui::Key::ArrowUp, daw_core::LfoRateMode::Note),
            (egui::Key::ArrowDown, daw_core::LfoRateMode::Hz),
            (egui::Key::ArrowDown, daw_core::LfoRateMode::Hz),
            (egui::Key::ArrowUp, daw_core::LfoRateMode::Note),
        ] {
            key(&mut app, key_code);
            assert_eq!(auto_filter_settings(&app).rate_mode, expected);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let arcs: Vec<_> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Path(path)
                        if path.stroke.width == 2.0
                            && path.points.iter().all(|point| mode.contains(*point)) =>
                    {
                        Some(path)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(arcs.len(), 1);
            assert_eq!(
                arcs[0].stroke.color,
                egui::epaint::ColorMode::Solid(theme::palette(&ctx).border)
            );
            let center = mode.center();
            // The neutral arc spans only the two upper choices, rather than a full circle.
            assert!(arcs[0].points.iter().all(|point| point.y < center.y));
            assert!(arcs[0].points.first().unwrap().x < center.x);
            assert!(arcs[0].points.last().unwrap().x > center.x);
            for name in ["Hz", "Note"] {
                let color = if name == expected.name() {
                    theme::accent(&ctx)
                } else {
                    theme::palette(&ctx).secondary
                };
                let text = shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.text() == name => Some(t),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(text.galley.job.sections[0].format.color, color);
            }
            let marks: Vec<_> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if points.iter().all(|point| mode.contains(*point))
                            && points[0].distance(points[1]) <= 3.1 =>
                    {
                        Some((points, stroke))
                    }
                    _ => None,
                })
                .collect();
            assert!(marks.is_empty());
        }
        // Large drags clamp to either endpoint and never wrap.
        for (delta, expected) in [
            (300.0, daw_core::LfoRateMode::Hz),
            (-300.0, daw_core::LfoRateMode::Note),
        ] {
            end_click_sequence(&mut app, &ctx);
            let start = mode.center();
            let end = start + Vec2::new(0.0, delta);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert_eq!(auto_filter_settings(&app).rate_mode, expected);
        }
        assert_eq!(auto_filter_settings(&app).note_rate, note);
        assert_eq!(auto_filter_settings(&app).rate_hz, hz);
    }
}

fn compressor_settings(app: &DawUi) -> &daw_core::CompressorSettings {
    let daw_core::EffectSettings::Compressor(settings) =
        &app.session.project.tracks[0].effects[0].unit
    else {
        panic!("expected Compressor")
    };
    settings
}
#[test]
fn compressor_catalog_adds_independent_units_and_blocks_without_a_track() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let entry = app.effects_bounds.compressor_entry.center();
    assert!(entry.y > app.effects_bounds.auto_filter_entry.bottom());
    app.dirty = false;
    click(&mut app, &ctx, entry);
    assert_eq!(
        app.effects_catalog.selection,
        HashSet::from([daw_core::COMPRESSOR_TYPE_ID])
    );
    assert!(!app.dirty);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        compressor_settings(&app),
        &daw_core::CompressorSettings::default()
    );
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, entry);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.compressor_units.len(), 2);
    assert_ne!(
        app.session.project.tracks[0].effects[0].id,
        app.session.project.tracks[0].effects[1].id
    );
    for blocked in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        if blocked {
            app.unsaved_prompt = true;
        } else {
            app.selected_track = None;
        }
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.compressor_entry.center();
        click(&mut app, &ctx, entry);
        click(&mut app, &ctx, entry);
        assert!(app.session.project.tracks[0].effects.is_empty());
    }
}
#[test]
fn compressor_layout_compact_collapse_and_readonly_meter_follow_unit_conventions() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.session.project.tracks[0].effects = vec![daw_core::EffectInstance::compressor()];
            app.session.project.tracks[0].effects_compact = compact;
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let bounds = &app.effects_bounds.compressor_units[0];
            for (column, section) in bounds.sections.iter().enumerate() {
                assert!((section.width() - crate::midi_editor::UNIT_STRIP_WIDTH).abs() < 0.1);
                if column > 0 {
                    assert!((section.left() - bounds.sections[column - 1].right()).abs() < 0.1);
                }
            }
            for name in ["Comp", "Response", "Out"] {
                assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == name)));
            }
            if compact {
                assert!(
                    bounds
                        .controls
                        .iter()
                        .flatten()
                        .all(|rect| !rect.is_positive())
                );
            } else {
                for row in 0..3 {
                    for column in 0..3 {
                        assert!(
                            (bounds.controls[column][row].center().y
                                - bounds.controls[0][row].center().y)
                                .abs()
                                < 0.1
                        );
                    }
                }
                // The third Out row is a meter, not a control which edits saved state.
                let meter = bounds.controls[2][2].center();
                app.dirty = false;
                let before = compressor_settings(&app).clone();
                click(&mut app, &ctx, meter);
                assert_eq!(compressor_settings(&app), &before);
                assert!(!app.dirty);
            }
            let power = app.effects_bounds.compressor_units[0].power.center();
            end_click_sequence(&mut app, &ctx);
            click(&mut app, &ctx, power);
            assert!(!compressor_settings(&app).enabled);
            let header = app.effects_bounds.compressor_units[0].header.center();
            end_click_sequence(&mut app, &ctx);
            click(&mut app, &ctx, header);
            click(&mut app, &ctx, header);
            frame(&mut app, &ctx, vec![]);
            assert!(compressor_settings(&app).collapsed);
        }
    }
}
#[test]
fn compressor_controls_edit_in_the_agreed_order_and_peak_rms_is_bounded() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0].effects = vec![daw_core::EffectInstance::compressor()];
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    for (column, row, key) in [
        (0, 0, egui::Key::ArrowRight),
        (0, 1, egui::Key::ArrowRight),
        (1, 0, egui::Key::ArrowRight),
        (1, 1, egui::Key::ArrowRight),
        (1, 2, egui::Key::ArrowRight),
        (2, 0, egui::Key::ArrowRight),
        (2, 1, egui::Key::ArrowLeft),
    ] {
        let before = compressor_settings(&app).clone();
        let point = app.effects_bounds.compressor_units[0].controls[column][row].center();
        end_click_sequence(&mut app, &ctx);
        click(&mut app, &ctx, point);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let after = compressor_settings(&app);
        assert!(after.valid());
        let mut expected = before.clone();
        match (column, row) {
            (0, 0) => {
                assert!(after.threshold_db > before.threshold_db);
                expected.threshold_db = after.threshold_db;
            }
            (0, 1) => {
                assert!(after.ratio > before.ratio);
                expected.ratio = after.ratio;
            }
            (1, 0) => {
                assert!(after.attack_ms > before.attack_ms);
                expected.attack_ms = after.attack_ms;
            }
            (1, 1) => {
                assert!(after.release_ms > before.release_ms);
                expected.release_ms = after.release_ms;
            }
            (1, 2) => {
                assert!(after.knee_db > before.knee_db);
                expected.knee_db = after.knee_db;
            }
            (2, 0) => {
                assert!(after.level_db > before.level_db);
                expected.level_db = after.level_db;
            }
            (2, 1) => {
                assert!(after.dry_wet < before.dry_wet);
                expected.dry_wet = after.dry_wet;
            }
            _ => unreachable!(),
        }
        assert_eq!(after, &expected);
    }
    for (delta, expected) in [
        (300.0, daw_core::CompressorDetection::Peak),
        (-300.0, daw_core::CompressorDetection::Rms),
        (-300.0, daw_core::CompressorDetection::Rms),
    ] {
        let start = app.effects_bounds.compressor_units[0].controls[0][2].center();
        let end = start + Vec2::new(0.0, delta);
        end_click_sequence(&mut app, &ctx);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(compressor_settings(&app).detection, expected);
    }
}
#[test]
fn compressor_peak_rms_paint_uses_the_shared_neutral_arc_and_choice_labels() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for detection in [
            daw_core::CompressorDetection::Peak,
            daw_core::CompressorDetection::Rms,
        ] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            let mut effect = daw_core::EffectInstance::compressor();
            let daw_core::EffectSettings::Compressor(settings) = &mut effect.unit else {
                unreachable!()
            };
            settings.detection = detection;
            app.session.project.tracks[0].effects = vec![effect];
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let rect = app.effects_bounds.compressor_units[0].controls[0][2];
            for (name, selected, left) in [
                (
                    "Peak",
                    detection == daw_core::CompressorDetection::Peak,
                    true,
                ),
                (
                    "RMS",
                    detection == daw_core::CompressorDetection::Rms,
                    false,
                ),
            ] {
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == name => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert!(text.pos.y < rect.center().y);
                assert_eq!(text.pos.x < rect.center().x, left);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.color
                            == if selected {
                                theme::accent(&ctx)
                            } else {
                                theme::palette(&ctx).secondary
                            })
                );
            }
            let arcs: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Path(path)
                        if path.points.len() == 33
                            && path.points.iter().all(|point| rect.contains(*point)) =>
                    {
                        Some(path)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(arcs.len(), 1);
            assert_eq!(
                arcs[0].stroke.color,
                egui::epaint::ColorMode::Solid(theme::palette(&ctx).border)
            );
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment {points,..} if points.iter().all(|point| rect.contains(*point)) && points[0].distance(points[1]) <= 3.1)));
        }
    }
}
