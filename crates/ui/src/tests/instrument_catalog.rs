use super::*;

pub(super) fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2, shift: bool) {
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        }],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::ModifiersChanged(Default::default())],
    );
}

pub(super) fn key(app: &mut DawUi, ctx: &egui::Context, key: egui::Key, shift: bool, repeat: bool) {
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
                repeat,
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

fn fixture(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id, [Id; 2]) {
    let mut app = DawUi::default();
    app.instrument_catalog.expand_folders();
    let track = app.session.project.add_midi_track().unwrap();
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].collapsed = true;
    instrument.layers.push(daw_core::InstrumentLayerSettings {
        collapsed: true,
        ..Default::default()
    });
    let layers = [instrument.layers[0].id, instrument.layers[1].id];
    app.select_track(track, false);
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, track, layers)
}

#[test]
fn both_catalogs_start_collapsed_and_retain_independent_expansion_choices() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for view in [ViewMode::Timeline, ViewMode::Instrument] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            app.select_track(track, false);
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            app.set_view_mode(view, &ctx);
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let original = serde_json::to_value(&app.session.project).unwrap();
            assert!(app.effects_bounds.folders.iter().all(Rect::is_positive));
            assert_eq!(
                [
                    app.effects_bounds.entry,
                    app.effects_bounds.auto_filter_entry,
                    app.effects_bounds.compressor_entry,
                    app.effects_bounds.ott_entry,
                    app.effects_bounds.limiter_entry,
                    app.effects_bounds.drive_entry,
                ],
                [Rect::NOTHING; 6]
            );
            let folder = app.effects_bounds.folders[0].center();
            click(&mut app, &ctx, folder, false);
            assert!(app.effects_bounds.entry.is_positive());
            assert!(app.effects_bounds.auto_filter_entry.is_positive());
            assert_eq!(app.effects_bounds.compressor_entry, Rect::NOTHING);
            assert_eq!(app.effects_bounds.drive_entry, Rect::NOTHING);
            for _ in 0..2 {
                let toggle = app.effects_bounds.library_toggle.center();
                click(&mut app, &ctx, toggle, false);
            }
            assert!(app.effects_bounds.entry.is_positive());
            app.set_view_mode(ViewMode::Instrument, &ctx);
            frame(&mut app, &ctx, vec![]);
            let toggle = app.instrument_catalog_bounds.toggle.center();
            click(&mut app, &ctx, toggle, false);
            assert!(
                app.instrument_catalog_bounds
                    .folders
                    .iter()
                    .all(Rect::is_positive)
            );
            assert_eq!(app.instrument_catalog_bounds.entries, [Rect::NOTHING; 7]);
            let sources = app.instrument_catalog_bounds.folders[0].center();
            click(&mut app, &ctx, sources, false);
            assert!(app.instrument_catalog_bounds.entries[0].is_positive());
            assert!(app.instrument_catalog_bounds.entries[5].is_positive());
            for index in [1, 2, 3, 4] {
                assert_eq!(app.instrument_catalog_bounds.entries[index], Rect::NOTHING);
            }
            let toggle = app.effects_bounds.library_toggle.center();
            click(&mut app, &ctx, toggle, false);
            assert!(app.effects_bounds.entry.is_positive());
            assert_eq!(app.effects_bounds.compressor_entry, Rect::NOTHING);
            assert_eq!(app.effects_bounds.drive_entry, Rect::NOTHING);
            let toggle = app.instrument_catalog_bounds.toggle.center();
            click(&mut app, &ctx, toggle, false);
            assert!(app.instrument_catalog_bounds.entries[0].is_positive());
            assert_eq!(app.instrument_catalog_bounds.entries[1], Rect::NOTHING);
            let sources = app.instrument_catalog_bounds.folders[0].center();
            click(&mut app, &ctx, sources, false);
            key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
            key(&mut app, &ctx, egui::Key::Enter, false, false);
            for index in [1, 2, 4] {
                assert!(app.instrument_catalog_bounds.entries[index].is_positive());
            }
            assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
            assert_eq!(app.instrument_catalog_bounds.entries[3], Rect::NOTHING);
            assert!(app.component_drag.is_none() && app.effects_drag.is_none());
            assert_eq!(
                serde_json::to_value(&app.session.project).unwrap(),
                original
            );
            assert!(!app.dirty);
        }
    }
}

fn open(app: &mut DawUi, ctx: &egui::Context) {
    let point = app.instrument_catalog_bounds.toggle.center();
    click(app, ctx, point, false);
    assert!(app.instrument_catalog_visible);
}

// Install through the catalog when a unit-control test needs an absent component.
pub(super) fn add_component(app: &mut DawUi, ctx: &egui::Context, layer: Id, index: usize) {
    if app.instrument_ui.selected_layer != Some(layer) {
        let point = if let Some(bounds) = app.instrument_ui.modulation_lane_bounds.get(&layer) {
            bounds.name.center()
        } else if app.session.project.tracks[0]
            .instrument
            .as_ref()
            .is_some_and(|i| i.main.id == layer)
        {
            app.instrument_ui.main_bounds.name.center()
        } else {
            app.midi_editor.layer_bounds[&layer].name.center()
        };
        click(app, ctx, point, false);
    }
    let was_open = app.instrument_catalog_visible;
    let effects_was_open = app.timeline_catalog_visible;
    if !was_open {
        open(app, ctx);
    }
    if !app.instrument_catalog_bounds.entries[index].is_positive() {
        app.instrument_catalog.expand_folders();
        frame(app, ctx, vec![]);
    }
    // Separate this click from earlier button and context-menu clicks.
    for _ in 0..40 {
        frame(app, ctx, vec![]);
    }
    let point = app.instrument_catalog_bounds.entries[index].center();
    click(app, ctx, point, false);
    key(app, ctx, egui::Key::Enter, false, false);
    if !was_open {
        let point = if effects_was_open {
            app.effects_bounds.library_toggle.center()
        } else {
            app.instrument_catalog_bounds.toggle.center()
        };
        click(app, ctx, point, false);
    }
    frame(app, ctx, vec![]);
}

#[test]
fn instrument_catalog_is_exclusive_and_reuses_the_control_panel_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _, _) = fixture(theme);
        let original = serde_json::to_value(&app.session.project).unwrap();
        let effects = app.effects_bounds.library;
        let toggle = app.instrument_catalog_bounds.toggle;
        assert_eq!(toggle.size(), Vec2::splat(24.0));
        assert_eq!(toggle.left(), app.effects_bounds.library_toggle.left());
        assert_eq!(
            app.effects_bounds.library_toggle.top() - toggle.bottom(),
            7.0
        );
        open(&mut app, &ctx);
        assert!(!app.timeline_catalog_visible);
        assert_eq!(app.effects_bounds.library, Rect::NOTHING);
        assert_eq!(app.instrument_catalog_bounds.library, effects);
        assert_eq!(
            app.midi_editor.instrument_header_bounds.left(),
            effects.right()
        );
        let point = app.effects_bounds.library_toggle.center();
        click(&mut app, &ctx, point, false);
        assert!(app.timeline_catalog_visible);
        assert!(!app.instrument_catalog_visible);
        assert_eq!(app.instrument_catalog_bounds.library, Rect::NOTHING);
        open(&mut app, &ctx);
        let point = app.instrument_catalog_bounds.toggle.center();
        click(&mut app, &ctx, point, false);
        assert!(!app.instrument_catalog_visible && !app.timeline_catalog_visible);
        assert_eq!(
            app.midi_editor.instrument_header_bounds.left(),
            app.effects_bounds.toolbar.right()
        );
        app.set_view_mode(ViewMode::Timeline, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.instrument_catalog_bounds.toggle, Rect::NOTHING);
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn catalog_keyboard_adds_selected_components_only_to_the_selected_layer() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, track, layers) = fixture(theme);
        assert_eq!(app.instrument_ui.selected_layer, Some(layers[0]));
        let point = app.midi_editor.layer_bounds[&layers[1]].name.center();
        click(&mut app, &ctx, point, false);
        assert_eq!(app.instrument_ui.selected_layer, Some(layers[1]));
        assert!(!app.dirty);
        open(&mut app, &ctx);
        let point = app.instrument_catalog_bounds.entries[1].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::ArrowDown, true, false);
        assert_eq!(app.instrument_catalog.selection.len(), 2);
        assert!(!app.dirty);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
        assert!(!app.dirty, "regular layers reject provider additions");
        let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
        let envelope = instrument.envelope_lane.id;
        let lfo = instrument.lfo_lane.id;
        app.instrument_ui.select_layer(envelope);
        let point = app.instrument_catalog_bounds.entries[1].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
        app.instrument_ui.select_layer(lfo);
        let point = app.instrument_catalog_bounds.entries[2].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
        let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(instrument.lfo_lane.lfos.len(), 1);
        assert_eq!(instrument.envelope_lane.envelopes.len(), 1);
        assert!(
            instrument
                .layers
                .iter()
                .all(|l| l.lfos.is_empty() && l.envelopes.iter().all(|e| !e.present))
        );
        assert_eq!(app.selected_track, Some(track));
        assert!(app.dirty);
        let saved = serde_json::to_value(&app.session.project).unwrap();
        let loaded: daw_core::Project = serde_json::from_value(saved).unwrap();
        loaded.validate().unwrap();
        let expected = app.session.project.clone();
        assert_eq!(loaded, expected);
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
        let held = serde_json::to_value(&app.session.project).unwrap();
        app.dirty = false;
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
        assert_eq!(serde_json::to_value(&app.session.project).unwrap(), held);
        assert!(!app.dirty);
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
        // Amplitude is a single retained audio stage; another Envelope is a new modulator.
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[1]
            .amplitude
            .level_db = -6.0;
        add_component(&mut app, &ctx, layers[1], 3);
        app.dirty = false;
        let original = serde_json::to_value(&app.session.project).unwrap();
        add_component(&mut app, &ctx, layers[1], 3);
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn component_double_click_adds_once_and_catalog_focus_and_selection_follow_shared_rules() {
    let (mut app, ctx, _, layers) = fixture(settings::ColorTheme::Dark);
    open(&mut app, &ctx);
    let oscillator = app.instrument_catalog_bounds.entries[0].center();
    // Separate the double-click from the preceding toolbar click.
    for _ in 0..40 {
        frame(&mut app, &ctx, vec![]);
    }
    click(&mut app, &ctx, oscillator, false);
    assert!(!app.dirty);
    click(&mut app, &ctx, oscillator, false);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .oscillators
            .len(),
        1
    );
    app.dirty = false;
    let envelope = app.instrument_catalog_bounds.entries[1].center();
    click(&mut app, &ctx, envelope, true);
    assert_eq!(app.instrument_catalog.selection.len(), 2);
    key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
    assert!(app.instrument_catalog.selection.is_empty()); // Modulators folder.
    key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
    assert_eq!(
        app.instrument_catalog.selection,
        HashSet::from(["daw.granulizer"])
    );
    key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
    assert_eq!(
        app.instrument_catalog.selection,
        HashSet::from(["daw.sampler"])
    );
    key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
    assert_eq!(
        app.instrument_catalog.selection,
        HashSet::from(["daw.oscillator"])
    );
    key(&mut app, &ctx, egui::Key::Escape, false, false);
    assert!(app.instrument_catalog.selection.is_empty());
    click(&mut app, &ctx, envelope, false);
    let blank = app.instrument_catalog_bounds.header.center();
    click(&mut app, &ctx, blank, false);
    assert!(app.instrument_catalog.selection.is_empty());
    click(&mut app, &ctx, oscillator, false);
    let point = app.midi_editor.layer_bounds[&layers[1]].name.center();
    click(&mut app, &ctx, point, false);
    assert!(app.instrument_catalog.selection.is_empty());
    assert_eq!(app.instrument_ui.selected_layer, Some(layers[1]));
    assert!(!app.dirty);
}

#[test]
fn component_addition_is_blocked_without_a_midi_target_and_during_modal_work() {
    for guard in ["none", "audio", "no-instrument", "modal"] {
        let (mut app, ctx, _, _) = fixture(settings::ColorTheme::Dark);
        open(&mut app, &ctx);
        let point = app.instrument_catalog_bounds.entries[0].center();
        click(&mut app, &ctx, point, false);
        match guard {
            "none" => app.selected_track = None,
            "audio" => {
                let audio = app.session.project.add_track().unwrap();
                app.select_track(audio, false);
            }
            "no-instrument" => app.session.project.tracks[0].instrument = None,
            "modal" => app.settings_open = true,
            _ => unreachable!(),
        }
        let original = serde_json::to_value(&app.session.project).unwrap();
        frame(&mut app, &ctx, vec![]);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
        let point = app.instrument_catalog_bounds.entries[0].center();
        click(&mut app, &ctx, point, false);
        click(&mut app, &ctx, point, false);
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{guard}"
        );
        assert!(!app.dirty, "{guard}");
    }
}

#[test]
fn selected_layer_identity_survives_reordering_and_views_and_recovers_after_removal() {
    let (mut app, ctx, track, layers) = fixture(settings::ColorTheme::Dark);
    let point = app.midi_editor.layer_bounds[&layers[1]].name.center();
    click(&mut app, &ctx, point, false);
    let mut settings = app.session.project.tracks[0].instrument.clone().unwrap();
    settings.layers[1].name = "Target".into();
    settings.layers.reverse();
    app.session
        .project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(settings),
        })
        .unwrap();
    app.set_view_mode(ViewMode::Timeline, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.selected_layer, Some(layers[1]));
    open(&mut app, &ctx);
    let point = app.instrument_catalog_bounds.entries[0].center();
    click(&mut app, &ctx, point, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    let settings = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(settings.layers[0].id, layers[1]);
    assert_eq!(settings.layers[0].oscillators.len(), 1);
    assert!(settings.layers[1].oscillators.is_empty());
    let mut settings = settings.clone();
    settings.layers.remove(0);
    app.session
        .project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(settings),
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.selected_layer, Some(layers[0]));
    app.pending = Some(Action::CloseProject);
    app.dirty = false;
    app.perform_pending();
    assert_eq!(app.instrument_ui.selected_layer, None);
    assert!(app.instrument_catalog.selection.is_empty());
}

#[test]
fn layer_target_resets_when_tracks_change_and_remains_visible_when_catalog_has_focus() {
    let (mut app, ctx, track, layers) = fixture(settings::ColorTheme::Light);
    let point = app.midi_editor.layer_bounds[&layers[1]].name.center();
    click(&mut app, &ctx, point, false);
    open(&mut app, &ctx);
    let point = app.instrument_catalog_bounds.entries[0].center();
    click(&mut app, &ctx, point, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let label = app.midi_editor.layer_bounds[&layers[1]].label;
    assert!(shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if rect.rect == label && rect.stroke == theme::selection_outline(&ctx))));
    let other = app.session.project.add_midi_track().unwrap();
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.select_track(other, false);
    frame(&mut app, &ctx, vec![]);
    app.select_track(track, false);
    frame(&mut app, &ctx, vec![]);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.instrument_ui.selected_layer, Some(layers[0]));
    assert!(app.instrument_catalog.selection.is_empty());
}

fn start_component_drag(app: &mut DawUi, ctx: &egui::Context, index: usize) {
    let source = app.instrument_catalog_bounds.entries[index].center();
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(source), button(source, true)],
    );
    assert!(app.component_drag.is_some());
}

#[test]
fn component_catalog_drags_preview_and_commit_selected_types_to_the_hovered_layer() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for expanded in [false, true] {
            for fast in [false, true] {
                for label in [false, true] {
                    let (mut app, ctx, _, layers) = fixture(theme);
                    app.timeline_effects_visible = false;
                    for layer in &mut app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .layers
                    {
                        layer.collapsed = !expanded;
                    }
                    frame(&mut app, &ctx, vec![]);
                    open(&mut app, &ctx);
                    let source = app.instrument_catalog_bounds.entries[0].center();
                    click(&mut app, &ctx, source, false);
                    let envelope = app.instrument_catalog_bounds.entries[1].center();
                    click(&mut app, &ctx, envelope, true);
                    let filter = app.instrument_catalog_bounds.entries[3].center();
                    click(&mut app, &ctx, filter, true);
                    let original = serde_json::to_value(&app.session.project).unwrap();
                    let target = &app.midi_editor.layer_bounds[&layers[1]];
                    let end = if label {
                        target.label.center()
                    } else {
                        Pos2::new(target.toolbar.left() - 20.0, target.row.center().y)
                    };
                    assert!(app.instrument_catalog_bounds.layer_viewport.contains(end));
                    start_component_drag(&mut app, &ctx, 0);
                    assert!(!app.file_action_enabled(FileAction::Save));
                    assert!(!app.view_switch_enabled());
                    assert_eq!(
                        serde_json::to_value(&app.session.project).unwrap(),
                        original
                    );
                    if !fast {
                        let shapes = frame_shapes(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end)],
                            Vec2::new(1280.0, 800.0),
                        );
                        assert!(app.component_drag.as_ref().unwrap().active);
                        assert_eq!(app.instrument_catalog_bounds.drop_target, Some(layers[1]));
                        let preview = app.instrument_catalog_bounds.drop_bounds;
                        assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(rect) if rect.rect == preview && rect.stroke == theme::selection_outline(&ctx))));
                        assert_eq!(app.instrument_catalog.selection.len(), 3);
                        assert_eq!(app.instrument_ui.selected_layer, Some(layers[0]));
                        assert_eq!(
                            serde_json::to_value(&app.session.project).unwrap(),
                            original
                        );
                        assert!(!app.dirty);
                    }
                    // A move and release together must retain the armed source and group.
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(end), button(end, false)],
                    );
                    assert!(app.component_drag.is_none());
                    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
                    assert!(
                        instrument.layers[0].oscillators.is_empty()
                            && !instrument.layers[0].envelopes[0].present
                            && !instrument.layers[0].envelopes[1].present
                    );
                    assert_eq!(instrument.layers[1].oscillators.len(), 1);
                    assert!(
                        !instrument.layers[1].envelopes[0].present,
                        "provider entries require their dedicated lanes"
                    );
                    assert!(instrument.layers[1].amplitude.present);
                    assert_eq!(app.instrument_ui.selected_layer, Some(layers[1]));
                    assert_eq!(app.instrument_catalog.selection.len(), 3);
                    assert!(app.session.project.tracks[0].effects.is_empty());
                    assert!(app.dirty);
                }
            }
        }
    }
}

#[test]
fn dragging_an_unselected_component_replaces_library_selection_and_existing_envelope_drop_stays_clean()
 {
    let (mut app, ctx, _, layers) = fixture(settings::ColorTheme::Dark);
    open(&mut app, &ctx);
    let envelope = app.instrument_catalog_bounds.entries[1].center();
    click(&mut app, &ctx, envelope, false);
    start_component_drag(&mut app, &ctx, 0);
    let end = app.midi_editor.layer_bounds[&layers[1]].label.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(
        app.instrument_catalog.selection,
        HashSet::from([crate::instrument_catalog::ENTRIES[0].type_id])
    );
    frame(&mut app, &ctx, vec![button(end, false)]);
    let target = &app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[1];
    assert_eq!(target.oscillators.len(), 1);
    assert!(!target.envelopes[0].present);
    // A drop onto an installed, disabled/collapsed envelope must retain all settings.
    let target = &mut app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[1];
    target.amplitude.present = true;
    target.amplitude.enabled = false;
    target.amplitude.collapsed = true;
    target.amplitude.level_db = -7.0;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let original = serde_json::to_value(&app.session.project).unwrap();
    start_component_drag(&mut app, &ctx, 3);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        serde_json::to_value(&app.session.project).unwrap(),
        original
    );
    assert!(!app.dirty);
    assert!(app.component_drag.is_none());
}

#[test]
fn component_drags_cancel_without_project_edits_on_invalid_context_or_drop() {
    for guard in [
        "escape", "focus", "disabled", "modal", "outside", "header", "empty", "hidden", "view",
        "track", "project", "changed", "removed",
    ] {
        let (mut app, ctx, _, layers) = fixture(settings::ColorTheme::Dark);
        let other = app.session.project.add_midi_track().unwrap();
        open(&mut app, &ctx);
        start_component_drag(&mut app, &ctx, 0);
        let mut end = app.midi_editor.layer_bounds[&layers[1]].label.center();
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(app.component_drag.as_ref().unwrap().active);
        match guard {
            "modal" => app.unsaved_prompt = true,
            "outside" => end = app.effects_bounds.strip.center(),
            "header" => end = app.midi_editor.instrument_header_bounds.center(),
            "empty" => {
                end = app.instrument_catalog_bounds.layer_viewport.center_bottom()
                    - Vec2::new(0.0, 5.0)
            }
            "hidden" => app.instrument_catalog_visible = false,
            "view" => app.view_mode = ViewMode::Timeline,
            "track" => app.selected_track = Some(other),
            "project" => app.session = Session::default(),
            "changed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .envelopes[0]
                    .settings
                    .release
                    .time_ms = 731.0
            }
            "removed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers
                    .remove(1);
            }
            _ => {}
        }
        let original = serde_json::to_value(&app.session.project).unwrap();
        let mut events = vec![egui::Event::PointerMoved(end), button(end, false)];
        if guard == "escape" {
            events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: guard != "focus",
                events,
                ..Default::default()
            },
            |ui| {
                if guard == "disabled" {
                    ui.disable();
                }
                app.show(ui);
            },
        );
        output.textures_delta.clear();
        assert!(app.component_drag.is_none(), "{guard}");
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{guard}"
        );
        assert!(!app.dirty, "{guard}");
    }
}

#[test]
fn component_catalog_drag_scrolls_to_an_offscreen_layer_and_keeps_preview_clean() {
    let (mut app, ctx, _, _) = fixture(settings::ColorTheme::Light);
    let layers = &mut app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers;
    for _ in 0..10 {
        layers.push(daw_core::InstrumentLayerSettings {
            collapsed: true,
            ..Default::default()
        });
    }
    let last = layers.last().unwrap().id;
    frame(&mut app, &ctx, vec![]);
    open(&mut app, &ctx);
    let viewport = app.instrument_catalog_bounds.layer_viewport;
    assert!(app.midi_editor.layer_bounds[&last].row.top() >= viewport.bottom());
    start_component_drag(&mut app, &ctx, 0);
    let edge = Pos2::new(
        viewport.right() - lane_toolbar::WIDTH - 20.0,
        viewport.bottom() - 3.0,
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge)]);
    let original = serde_json::to_value(&app.session.project).unwrap();
    for _ in 0..180 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(app.instrument_catalog_bounds.layer_scroll > 0.0);
    let visible = app.midi_editor.layer_bounds[&last]
        .row
        .intersect(app.instrument_catalog_bounds.layer_viewport);
    assert!(
        visible.is_positive(),
        "last layer did not scroll into view: {visible:?}"
    );
    assert_eq!(
        serde_json::to_value(&app.session.project).unwrap(),
        original
    );
    assert!(!app.dirty);
    let end = visible.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.instrument_catalog_bounds.drop_target, Some(last));
    frame(&mut app, &ctx, vec![button(end, false)]);
    let layers = &app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers;
    assert!(
        layers
            .iter()
            .filter(|layer| layer.id != last)
            .all(|layer| layer.oscillators.is_empty())
    );
    assert_eq!(layers.last().unwrap().oscillators.len(), 1);
    assert_eq!(app.instrument_ui.selected_layer, Some(last));
    assert!(app.dirty);
}

#[test]
fn amplitude_double_click_adds_once_to_the_selected_layer() {
    let (mut app, ctx, _, layers) = fixture(settings::ColorTheme::Dark);
    open(&mut app, &ctx);
    for _ in 0..40 {
        frame(&mut app, &ctx, vec![]);
    }
    let point = app.instrument_catalog_bounds.entries[3].center();
    click(&mut app, &ctx, point, false);
    assert!(!app.dirty);
    click(&mut app, &ctx, point, false);
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!(instrument.layers[0].amplitude.present);
    assert!(
        !instrument.layers[0].envelopes[0].present && instrument.layers[0].oscillators.is_empty()
    );
    assert!(!instrument.layers[1].amplitude.present);
    assert_eq!(app.instrument_ui.selected_layer, Some(layers[0]));
    app.dirty = false;
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert!(!app.dirty);
}

#[test]
fn category_folders_group_entries_and_hide_selected_children_without_editing_the_project() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _, _) = fixture(theme);
        open(&mut app, &ctx);
        let original = serde_json::to_value(&app.session.project).unwrap();
        let entries = app.instrument_catalog_bounds.entries;
        let folders = app.instrument_catalog_bounds.folders;
        let rows = [
            folders[0], entries[0], entries[5], folders[1], entries[1], entries[2], entries[4],
            folders[2], entries[3],
        ];
        assert!(
            rows.windows(2)
                .all(|pair| pair[0].bottom() <= pair[1].top())
        );
        assert!(
            rows.iter()
                .all(|rect| app.instrument_catalog_bounds.library.contains_rect(*rect))
        );
        click(&mut app, &ctx, entries[0].center(), false);
        click(&mut app, &ctx, entries[1].center(), true);
        click(&mut app, &ctx, folders[0].center(), false);
        assert!(app.component_drag.is_none());
        assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
        assert_eq!(app.instrument_catalog_bounds.entries[5], Rect::NOTHING);
        assert_eq!(
            app.instrument_catalog.selection,
            HashSet::from(["daw.envelope"])
        );
        for repeat in [false, true] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat,
                    modifiers: Default::default(),
                }],
            );
            assert!(app.instrument_catalog_bounds.entries[0].is_positive());
        }
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
        assert_eq!(
            app.instrument_catalog.selection,
            HashSet::from(["daw.envelope"])
        );
        key(&mut app, &ctx, egui::Key::Space, false, false);
        assert!(app.instrument_catalog_bounds.entries[0].is_positive());
        key(&mut app, &ctx, egui::Key::Enter, false, false);
        assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
        let toggle = app.instrument_catalog_bounds.toggle.center();
        click(&mut app, &ctx, toggle, false);
        open(&mut app, &ctx);
        assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
        app.set_view_mode(ViewMode::Timeline, &ctx);
        frame(&mut app, &ctx, vec![]);
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}

#[test]
fn category_keyboard_navigation_uses_visible_rows_and_preserves_component_addition() {
    let (mut app, ctx, _, _) = fixture(settings::ColorTheme::Dark);
    open(&mut app, &ctx);
    let owner = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .envelope_lane
        .id;
    app.instrument_ui.select_layer(owner);
    let source = app.instrument_catalog_bounds.entries[0].center();
    click(&mut app, &ctx, source, false);
    key(&mut app, &ctx, egui::Key::ArrowLeft, false, false);
    assert_eq!(app.instrument_catalog_bounds.entries[0], Rect::NOTHING);
    assert!(app.instrument_catalog.selection.is_empty());
    key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    assert!(app.instrument_catalog_bounds.entries[0].is_positive());
    key(&mut app, &ctx, egui::Key::ArrowLeft, false, false);
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false); // Modulators.
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false); // Envelope.
    assert_eq!(
        app.instrument_catalog.selection,
        HashSet::from(["daw.envelope"])
    );
    key(&mut app, &ctx, egui::Key::ArrowDown, true, false);
    key(&mut app, &ctx, egui::Key::ArrowDown, true, false);
    assert_eq!(
        app.instrument_catalog.selected_types(),
        vec!["daw.envelope", "daw.lfo", "daw.macro"]
    );
    key(&mut app, &ctx, egui::Key::ArrowUp, true, false);
    assert_eq!(
        app.instrument_catalog.selected_types(),
        vec!["daw.envelope", "daw.lfo"]
    );
    key(&mut app, &ctx, egui::Key::ArrowLeft, false, false);
    assert_eq!(app.instrument_catalog_bounds.entries[1], Rect::NOTHING);
    assert!(app.instrument_catalog.selection.is_empty());
    key(&mut app, &ctx, egui::Key::ArrowRight, false, false);
    key(&mut app, &ctx, egui::Key::ArrowDown, false, false);
    key(&mut app, &ctx, egui::Key::Enter, false, false);
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!(instrument.envelope_lane.envelopes[0].present);
    assert!(instrument.layers[0].oscillators.is_empty() && instrument.layers[0].lfos.is_empty());
    assert!(app.dirty);
}

#[test]
fn category_group_drag_uses_display_order_and_excludes_closed_folders() {
    let (mut app, ctx, _, _) = fixture(settings::ColorTheme::Dark);
    open(&mut app, &ctx);
    for (index, shift) in [(3, false), (4, true), (1, true)] {
        let point = app.instrument_catalog_bounds.entries[index].center();
        click(&mut app, &ctx, point, shift);
    }
    start_component_drag(&mut app, &ctx, 1);
    let crate::instrument_component_drag::ComponentDragSource::Catalog { types, .. } =
        &app.component_drag.as_ref().unwrap().source
    else {
        panic!("expected catalog drag")
    };
    assert_eq!(types, &["daw.envelope", "daw.macro", "daw.amplitude"]);
    let outside = app.effects_bounds.strip.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(outside), button(outside, false)],
    );
    for (index, shift) in [(3, false), (4, true), (1, true)] {
        let point = app.instrument_catalog_bounds.entries[index].center();
        click(&mut app, &ctx, point, shift);
    }
    let folder = app.instrument_catalog_bounds.folders[1].center();
    click(&mut app, &ctx, folder, false);
    assert!(app.component_drag.is_none());
    assert_eq!(
        app.instrument_catalog.selected_types(),
        vec!["daw.amplitude"]
    );
    start_component_drag(&mut app, &ctx, 3);
    let crate::instrument_component_drag::ComponentDragSource::Catalog { types, .. } =
        &app.component_drag.as_ref().unwrap().source
    else {
        panic!("expected catalog drag")
    };
    assert_eq!(types, &["daw.amplitude"]);
    assert!(!app.dirty);
}

#[test]
fn category_catalog_scrolls_in_a_short_viewport_and_keeps_the_header_fixed() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _, _) = fixture(theme);
        open(&mut app, &ctx);
        let source = app.instrument_catalog_bounds.entries[0].center();
        click(&mut app, &ctx, source, false);
        let size = Vec2::new(1280.0, 480.0);
        frame_shapes(&mut app, &ctx, vec![], size);
        let header = app.instrument_catalog_bounds.header;
        let library = app.instrument_catalog_bounds.library;
        assert!(app.instrument_catalog_bounds.entries[3].bottom() > library.bottom());
        let pointer = Pos2::new(library.center().x, library.bottom() - 12.0);
        for _ in 0..10 {
            frame_shapes(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: Vec2::new(0.0, -100.0),
                        modifiers: Default::default(),
                    },
                ],
                size,
            );
        }
        let amplitude = app.instrument_catalog_bounds.entries[3];
        assert!(library.contains_rect(amplitude));
        assert_eq!(app.instrument_catalog_bounds.header, header);
        assert_eq!(
            app.instrument_catalog.selection,
            HashSet::from(["daw.oscillator"])
        );
        frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            size,
        );
        for _ in 0..30 {
            frame_shapes(&mut app, &ctx, vec![], size);
        }
        // Each catalog retains its own scroll position.
        for instrument in [false, true] {
            let toggle = if instrument {
                app.instrument_catalog_bounds.toggle.center()
            } else {
                app.effects_bounds.library_toggle.center()
            };
            for pressed in [true, false] {
                frame_shapes(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(toggle), button(toggle, pressed)],
                    size,
                );
            }
            frame_shapes(&mut app, &ctx, vec![], size);
            if instrument {
                assert!(library.contains_rect(app.instrument_catalog_bounds.entries[3]));
            } else {
                assert!(app.effects_bounds.entry.top() >= app.effects_bounds.header.bottom());
            }
        }
        assert!(app.instrument_catalog.selection.is_empty());
        assert!(app.component_drag.is_none());
        assert!(!app.dirty);
    }
}

#[test]
fn category_folders_cannot_toggle_during_disabled_or_modal_ui() {
    for modal in [false, true] {
        let (mut app, ctx, _, _) = fixture(settings::ColorTheme::Dark);
        open(&mut app, &ctx);
        let point = app.instrument_catalog_bounds.folders[0].center();
        let original = serde_json::to_value(&app.session.project).unwrap();
        app.settings_open = modal;
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    events: vec![egui::Event::PointerMoved(point), button(point, pressed)],
                    ..Default::default()
                },
                |ui| {
                    if !modal {
                        ui.disable();
                    }
                    app.show(ui);
                },
            );
            output.textures_delta.clear();
        }
        app.settings_open = false;
        frame(&mut app, &ctx, vec![]);
        assert!(app.instrument_catalog_bounds.entries[0].is_positive());
        assert!(app.component_drag.is_none());
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original
        );
        assert!(!app.dirty);
    }
}
