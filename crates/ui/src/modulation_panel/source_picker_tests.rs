use super::*;
use egui::{Event, PointerButton, Pos2, Shape};

fn source(layer: u128, unit: u128, layer_name: &str, name: &str, scope: ModulationScope) -> Source {
    Source {
        id: ModulationSourceId {
            layer: daw_core::Id::from_u128(layer),
            unit: daw_core::Id::from_u128(unit),
        },
        label: format!("{layer_name} · {name}"),
        layer_name: layer_name.into(),
        name: name.into(),
        scope,
        range: ModulationRange::Unipolar,
        behavior: daw_unit_core::ModulationBehavior::Contour,
        enabled: true,
    }
}
fn example() -> Vec<Source> {
    vec![
        source(1, 101, "Layer 1", "Envelope 1", ModulationScope::PerNote),
        source(2, 102, "Layer 2", "Envelope 1", ModulationScope::Shared),
        source(2, 103, "Layer 2", "LFO 1", ModulationScope::Shared),
        source(3, 104, "Output", "Macro 1", ModulationScope::Shared),
    ]
}
#[test]
fn groups_keep_layer_identity_order_names_and_scope_filters() {
    let mut sources = example();
    sources[2].enabled = false;
    let groups = source_groups(&sources, ModulationScope::PerNote);
    assert_eq!(
        groups.iter().map(|g| g.name).collect::<Vec<_>>(),
        ["Layer 1", "Layer 2", "Output"]
    );
    assert_eq!(
        groups[1]
            .sources
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Envelope 1", "LFO 1"]
    );
    let groups = source_groups(&sources, ModulationScope::Shared);
    assert_eq!(
        groups.iter().map(|g| g.name).collect::<Vec<_>>(),
        ["Layer 2", "Output"]
    );
    assert!(
        !groups[0].sources[1].enabled,
        "disabled providers remain selectable"
    );
    sources[1].layer_name = "Layer 1".into();
    sources[2].layer_name = "Layer 1".into();
    let groups = source_groups(&sources, ModulationScope::PerNote);
    assert_eq!(
        groups.len(),
        3,
        "equal names must not merge distinct layers"
    );
    assert_ne!(groups[0].id, groups[1].id);
}
struct Frame {
    button: Rect,
    text: Vec<(String, Rect)>,
    dividers: Vec<Rect>,
    wrapped: Vec<String>,
    borders: Vec<Rect>,
    fills: Vec<(Rect, egui::Color32, egui::CornerRadius)>,
}
fn frame(
    ctx: &egui::Context,
    selected: &mut Option<ModulationSourceId>,
    sources: &[Source],
    events: Vec<Event>,
) -> Frame {
    let mut button = Rect::NOTHING;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 500.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.set_width(70.0);
            button = source_picker(ui, selected, sources, ModulationScope::PerNote).rect;
        },
    );
    let mut text = Vec::new();
    let mut dividers = Vec::new();
    let mut wrapped = Vec::new();
    let mut borders = Vec::new();
    let mut fills = Vec::new();
    fn collect(
        shape: &Shape,
        text: &mut Vec<(String, Rect)>,
        dividers: &mut Vec<Rect>,
        wrapped: &mut Vec<String>,
        borders: &mut Vec<Rect>,
        fills: &mut Vec<(Rect, egui::Color32, egui::CornerRadius)>,
    ) {
        match shape {
            Shape::Text(t) => {
                if t.galley.rows.len() > 1 {
                    wrapped.push(t.galley.text().into());
                }
                text.push((
                    t.galley.text().into(),
                    Rect::from_min_size(t.pos, t.galley.size()),
                ));
            }
            Shape::LineSegment { points, .. } if (points[0].y - points[1].y).abs() < 0.1 => {
                dividers.push(Rect::from_two_pos(points[0], points[1]))
            }
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, text, dividers, wrapped, borders, fills)
                }
            }
            Shape::Rect(rect) => {
                if rect.stroke.width > 0.0 && rect.stroke.color.a() > 0 {
                    borders.push(rect.rect);
                }
                if rect.fill.a() > 0 {
                    fills.push((rect.rect, rect.fill, rect.corner_radius));
                }
            }
            _ => {}
        }
    }
    for shape in &output.shapes {
        collect(
            &shape.shape,
            &mut text,
            &mut dividers,
            &mut wrapped,
            &mut borders,
            &mut fills,
        );
    }
    output.textures_delta.clear();
    Frame {
        button,
        text,
        dividers,
        wrapped,
        borders,
        fills,
    }
}
fn click(
    ctx: &egui::Context,
    selected: &mut Option<ModulationSourceId>,
    sources: &[Source],
    pos: Pos2,
) -> Frame {
    frame(
        ctx,
        selected,
        sources,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        ctx,
        selected,
        sources,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    )
}
fn text_rect(frame: &Frame, name: &str) -> Rect {
    frame
        .text
        .iter()
        .rev()
        .find(|(text, _)| text == name)
        .unwrap_or_else(|| panic!("missing {name}: {:?}", frame.text))
        .1
}
#[test]
fn picker_has_a_divider_and_layer_submenus_that_assign_and_clear_duplicate_names() {
    let ctx = egui::Context::default();
    crate::fonts::configure(&ctx);
    crate::icons::configure(&ctx);
    theme::configure(&ctx);
    let sources = example();
    let mut selected = None;
    let initial = frame(&ctx, &mut selected, &sources, vec![]);
    click(&ctx, &mut selected, &sources, initial.button.center());
    let opened = frame(&ctx, &mut selected, &sources, vec![]);
    let layer1 = text_rect(&opened, "Layer 1");
    let layer2 = text_rect(&opened, "Layer 2");
    let output = text_rect(&opened, "Output");
    assert!(layer1.top() < layer2.top() && layer2.top() < output.top());
    assert!(
        !opened.text.iter().any(|(text, _)| text == "Envelope 1"),
        "sources belong inside submenus"
    );
    let none = opened
        .text
        .iter()
        .filter(|(text, _)| text == "None")
        .map(|(_, rect)| *rect)
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    assert!(
        opened
            .dividers
            .iter()
            .any(|line| line.top() > none.bottom() && line.bottom() < layer1.top()),
        "divider separates None from groups"
    );
    click(&ctx, &mut selected, &sources, layer2.center());
    assert_eq!(selected, None, "opening a layer must not assign a source");
    let submenu = frame(
        &ctx,
        &mut selected,
        &sources,
        vec![Event::PointerMoved(layer2.center())],
    );
    assert!(text_rect(&submenu, "LFO 1").is_positive());
    let envelope = text_rect(&submenu, "Envelope 1");
    click(&ctx, &mut selected, &sources, envelope.center());
    assert_eq!(selected, Some(sources[1].id));
    let closed = frame(&ctx, &mut selected, &sources, vec![]);
    assert!(!closed.text.iter().any(|(text, _)| text == "Layer 1"));
    click(&ctx, &mut selected, &sources, closed.button.center());
    let opened = frame(&ctx, &mut selected, &sources, vec![]);
    click(
        &ctx,
        &mut selected,
        &sources,
        text_rect(&opened, "None").center(),
    );
    assert_eq!(selected, None);
}

#[test]
fn moving_an_assigned_source_updates_its_submenu_and_selected_owner() {
    use daw_core::{EnvelopeMode, InstrumentComponent, InstrumentLayerSettings, Project};
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    let instrument = project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers.push(InstrumentLayerSettings {
        name: "Layer 2".into(),
        ..Default::default()
    });
    instrument.layers[0].envelopes[0].present = true;
    instrument.layers[0].envelopes[0].mode = EnvelopeMode::Legato;
    let first = instrument.layers[0].id;
    let second = instrument.layers[1].id;
    let output = instrument.main.id;
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(ModulationSourceId {
        layer: first,
        unit: instrument.layers[0].envelopes[0].id,
    });
    route.amount = 0.75;
    instrument.layers[0].amplitude.present = true;
    instrument.layers[0].amplitude.modulations = vec![route];
    project.validate().unwrap();
    let ctx = egui::Context::default();
    crate::fonts::configure(&ctx);
    crate::icons::configure(&ctx);
    theme::configure(&ctx);
    for (destination, owner_name) in [(second, "Layer 2"), (output, "Output"), (first, "Layer 1")] {
        let previous = project.tracks[0].instrument.as_ref().unwrap().layers[0]
            .amplitude
            .modulations[0]
            .source
            .unwrap();
        let current = project.tracks[0].instrument.as_ref().unwrap();
        let insertion = if destination == current.main.id {
            current.main.visible_components().len()
        } else {
            current
                .layers
                .iter()
                .find(|layer| layer.id == destination)
                .unwrap()
                .visible_components()
                .len()
        };
        let transfer = project
            .component_group_transfer(
                track,
                previous.layer,
                destination,
                &[InstrumentComponent::Envelope(previous.unit)],
                insertion,
                false,
            )
            .unwrap();
        project.edit(transfer.edit).unwrap();
        project.validate().unwrap();
        let instrument = project.tracks[0].instrument.as_ref().unwrap();
        let assigned = instrument.layers[0].amplitude.modulations[0];
        assert_eq!(assigned.id, route.id);
        assert_eq!(assigned.amount, route.amount);
        assert_eq!(assigned.parameter, route.parameter);
        assert_eq!(assigned.enabled, route.enabled);
        let id = assigned.source.unwrap();
        assert_eq!(id.layer, destination);
        let available = super::sources(Some(instrument));
        assert!(
            !available.iter().any(|s| s.id == previous),
            "the former owner must no longer list the moved source"
        );
        let groups = source_groups(&available, ModulationScope::PerNote);
        let owners: Vec<_> = groups
            .iter()
            .filter(|g| g.sources.iter().any(|s| s.id == id))
            .collect();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].id, destination);
        assert_eq!(owners[0].name, owner_name);
        let moved = owners[0].sources.iter().find(|s| s.id == id).unwrap();
        assert_eq!(moved.label, format!("{owner_name} · {}", moved.name));
        let moved_name = moved.name.clone();
        // Render the already-assigned connection, open its new submenu and reselect
        // its leaf. This must preserve the transaction's current source identity.
        let mut selected = assigned.source;
        let initial = frame(&ctx, &mut selected, &available, vec![]);
        click(&ctx, &mut selected, &available, initial.button.center());
        let opened = frame(&ctx, &mut selected, &available, vec![]);
        let owner = text_rect(&opened, owner_name);
        click(&ctx, &mut selected, &available, owner.center());
        let submenu = frame(
            &ctx,
            &mut selected,
            &available,
            vec![Event::PointerMoved(owner.center())],
        );
        click(
            &ctx,
            &mut selected,
            &available,
            text_rect(&submenu, &moved_name).center(),
        );
        assert_eq!(selected, assigned.source);
    }
    // None is still available for a connection whose source has been moved.
    let instrument = project.tracks[0].instrument.as_ref().unwrap();
    let available = super::sources(Some(instrument));
    let mut assigned = instrument.layers[0].amplitude.modulations[0];
    let initial = frame(&ctx, &mut assigned.source, &available, vec![]);
    click(
        &ctx,
        &mut assigned.source,
        &available,
        initial.button.center(),
    );
    let opened = frame(&ctx, &mut assigned.source, &available, vec![]);
    click(
        &ctx,
        &mut assigned.source,
        &available,
        text_rect(&opened, "None").center(),
    );
    assert_eq!(assigned.source, None);
    assert_eq!(assigned.amount, route.amount);
    assert_eq!(assigned.id, route.id);
}

#[test]
fn source_selector_fills_inner_strip_width_without_adding_button_padding() {
    for (width, padding) in [(54.0, 2.0), (70.0, 8.0), (110.0, 4.0)] {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::icons::configure(&ctx);
        theme::configure(&ctx);
        let sources = example();
        let mut selected = Some(sources[1].id);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 300.0))),
                ..Default::default()
            },
            |ui| {
                egui::Frame::new()
                    .inner_margin(10)
                    .outer_margin(4)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().button_padding.x = padding;
                        let inner = ui.available_rect_before_wrap();
                        let response = selector_row(ui, "Source", |ui| {
                            source_picker(ui, &mut selected, &sources, ModulationScope::PerNote)
                        });
                        assert!(
                            (response.rect.width() - inner.width()).abs() < 0.1,
                            "{:?} vs {:?}",
                            response.rect,
                            inner
                        );
                        assert!((response.rect.left() - inner.left()).abs() < 0.1);
                        assert!((response.rect.right() - inner.right()).abs() < 0.1);
                    });
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn destination_selector_fills_inner_strip_width_and_preserves_assigned_routes() {
    for width in [54.0, 70.0, 110.0] {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::icons::configure(&ctx);
        theme::configure(&ctx);
        let sources = example();
        let mut route = Modulation::new(ModulationParameter::Amplitude);
        route.source = Some(sources[1].id);
        let before = route;
        let targets = [Target {
            parameter: route.parameter,
            id: None,
            label: "Amplitude gain".into(),
        }];
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 300.0))),
                ..Default::default()
            },
            |ui| {
                egui::Frame::new()
                    .inner_margin(10)
                    .outer_margin(4)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        let inner = ui.available_rect_before_wrap();
                        let response = selector_row(ui, "Destination", |ui| {
                            destination_picker(
                                ui,
                                &mut route,
                                &targets,
                                ModulationScope::PerNote,
                                |_| &sources,
                            )
                        });
                        assert!((response.rect.width() - inner.width()).abs() < 0.1);
                        assert!((response.rect.left() - inner.left()).abs() < 0.1);
                        assert!((response.rect.right() - inner.right()).abs() < 0.1);
                    });
            },
        );
        output.textures_delta.clear();
        assert_eq!(route, before);
    }
}

#[test]
fn flat_submenus_keep_single_line_labels_and_stable_geometry_through_hover() {
    for color in [
        crate::settings::ColorTheme::Dark,
        crate::settings::ColorTheme::Light,
    ] {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::icons::configure(&ctx);
        theme::configure(&ctx);
        theme::apply(
            &ctx,
            crate::settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let sources = example();
        let mut selected = Some(sources[0].id);
        let initial = frame(&ctx, &mut selected, &sources, vec![]);
        click(&ctx, &mut selected, &sources, initial.button.center());
        let root = frame(&ctx, &mut selected, &sources, vec![]);
        fn flat(frame: &Frame, text: Rect) {
            assert!(
                !frame
                    .borders
                    .iter()
                    .any(|border| border.contains(text.center())
                        && border.height() <= text.height() + 8.0),
                "outlined menu row: {:?}",
                frame.borders
            );
            assert!(
                frame.wrapped.is_empty(),
                "wrapped menu labels: {:?}",
                frame.wrapped
            );
        }
        for group in ["Layer 1", "Layer 2", "Output"] {
            flat(&root, text_rect(&root, group));
        }
        let layer = text_rect(&root, "Layer 1").center();
        let mut opened = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(layer)],
        );
        for _ in 0..3 {
            opened = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(layer)],
            );
        }
        let envelope = text_rect(&opened, "Envelope 1");
        flat(&opened, envelope);
        for _ in 0..5 {
            let hovered = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(envelope.center())],
            );
            let current = text_rect(&hovered, "Envelope 1");
            flat(&hovered, current);
            assert_eq!(
                current, envelope,
                "hover must not resize or move a submenu label"
            );
        }
        let layer2 = text_rect(&root, "Layer 2").center();
        for _ in 0..20 {
            opened = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(layer2)],
            );
        }
        let lfo = text_rect(&opened, "LFO 1");
        flat(&opened, lfo);
        let hovered = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(lfo.center())],
        );
        flat(&hovered, text_rect(&hovered, "LFO 1"));
        assert_eq!(
            selected,
            Some(sources[0].id),
            "hovering must not change the assigned source"
        );
    }
}

fn highlighted_row(frame: &Frame, label: &str) -> (Rect, egui::Color32, egui::CornerRadius) {
    let text = text_rect(frame, label);
    *frame
        .fills
        .iter()
        .find(|(rect, _, _)| rect.contains(text.center()) && rect.height() <= 40.0)
        .unwrap_or_else(|| panic!("missing row highlight for {label}: {:?}", frame.fills))
}
#[test]
fn list_menu_rows_share_minimum_width_padding_rounding_and_full_row_hover_color() {
    for color in [
        crate::settings::ColorTheme::Dark,
        crate::settings::ColorTheme::Light,
    ] {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::icons::configure(&ctx);
        theme::configure(&ctx);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        theme::apply(
            &ctx,
            crate::settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        let sources = example();
        let mut selected = None;
        let initial = frame(&ctx, &mut selected, &sources, vec![]);
        click(&ctx, &mut selected, &sources, initial.button.center());
        let root = frame(&ctx, &mut selected, &sources, vec![]);
        let (root_row, fill, radius) = highlighted_row(&root, "None");
        assert!(root_row.width() >= SOURCE_MENU_MIN_WIDTH - 0.1);
        let popup = root
            .fills
            .iter()
            .filter(|(rect, _, _)| {
                rect.height() > root_row.height() && rect.contains_rect(root_row)
            })
            .min_by(|a, b| a.0.width().total_cmp(&b.0.width()))
            .expect("root popup background")
            .0;
        assert!(
            (root_row.left() - popup.left() - 4.0 - 1.0).abs() < 0.1,
            "shared outer margin around menu rows: row={root_row:?}, popup={popup:?}"
        );
        assert!(
            (popup.right() - root_row.right() - 4.0 - 1.0).abs() < 0.1,
            "shared outer margin around menu rows: row={root_row:?}, popup={popup:?}"
        );
        assert_eq!(fill, theme::toggle_colors(&ctx, true).0);
        assert!(radius.nw > 0);
        assert!((text_rect(&root, "None").left() - root_row.left() - 4.0).abs() < 0.1);
        let first_group = text_rect(&root, "Layer 1").center();
        let first_group_hovered = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(first_group)],
        );
        let first_group_row = highlighted_row(&first_group_hovered, "Layer 1").0;
        let divider = root
            .dividers
            .iter()
            .find(|rect| {
                rect.center().y > root_row.bottom() && rect.center().y < first_group_row.top()
            })
            .expect("divider between None and first group");
        // The 1-point divider has no inset padding, only the shared row gap.
        assert!(
            (divider.center().y - root_row.bottom() - 4.5).abs() < 0.1,
            "divider={divider:?}, row={root_row:?}, first={first_group_row:?}"
        );
        assert!((first_group_row.top() - divider.center().y - 4.5).abs() < 0.1);
        let group = text_rect(&root, "Layer 2").center();
        let mut opened = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(group)],
        );
        for _ in 0..3 {
            opened = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(group)],
            );
        }
        let parent = highlighted_row(&opened, "Layer 2");
        assert_eq!(parent.1, theme::TOOLBAR_HOVER_FILL);
        for point in [
            group,
            egui::pos2(parent.0.left() + 1.0, parent.0.center().y),
            egui::pos2(parent.0.right() - 1.0, parent.0.center().y),
        ] {
            opened = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(point)],
            );
            assert_eq!(highlighted_row(&opened, "Layer 2"), parent);
        }
        for (shown, label) in [(&root, "None"), (&root, "Layer 2"), (&opened, "LFO 1")] {
            let font = egui::TextStyle::Small.resolve(&ctx.global_style());
            let expected = ctx.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(label.into(), font, egui::Color32::WHITE)
                    .size()
            });
            assert_eq!(text_rect(shown, label).size(), expected);
        }
        let first_leaf = text_rect(&opened, "Envelope 1").center();
        let first_hovered = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(first_leaf)],
        );
        let first_row = highlighted_row(&first_hovered, "Envelope 1").0;
        let leaf = text_rect(&first_hovered, "LFO 1");
        let mut hovered = frame(
            &ctx,
            &mut selected,
            &sources,
            vec![Event::PointerMoved(leaf.center())],
        );
        for _ in 0..2 {
            hovered = frame(
                &ctx,
                &mut selected,
                &sources,
                vec![Event::PointerMoved(leaf.center())],
            );
        }
        let (leaf_row, leaf_fill, leaf_radius) = highlighted_row(&hovered, "LFO 1");
        assert_eq!(leaf_row.width(), root_row.width());
        assert!(
            (leaf_row.top() - first_row.bottom() - 4.0).abs() < 0.1,
            "menu rows must have a 4-point gap"
        );
        assert_eq!(leaf_fill, theme::TOOLBAR_HOVER_FILL);
        assert_eq!(leaf_radius, radius);
        assert!((leaf.left() - leaf_row.left() - 4.0).abs() < 0.1);
        for point in [
            leaf.center(),
            egui::pos2(leaf_row.left() + 1.0, leaf_row.center().y),
            egui::pos2(leaf_row.right() - 1.0, leaf_row.center().y),
        ] {
            for _ in 0..3 {
                hovered = frame(
                    &ctx,
                    &mut selected,
                    &sources,
                    vec![Event::PointerMoved(point)],
                );
            }
            let (rect, current_fill, current_radius) = highlighted_row(&hovered, "LFO 1");
            assert_eq!(
                (rect, current_fill, current_radius),
                (leaf_row, theme::TOOLBAR_HOVER_FILL, radius)
            );
        }
        assert_eq!(selected, None);
    }
}
