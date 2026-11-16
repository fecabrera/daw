use super::*;
use daw_core::{InstrumentLayerSettings, ModulationParameter as P};
use egui::{Event, PointerButton, Pos2, Rect, Shape};

fn oscillators(count: usize) -> InstrumentLayerSettings {
    InstrumentLayerSettings {
        oscillators: (0..count).map(|_| Default::default()).collect(),
        ..Default::default()
    }
}

#[test]
fn oscillator_hierarchy_keeps_shared_roots_and_original_connection_default() {
    for count in [0, 1, 2, 12] {
        let layer = oscillators(count);
        let targets = crate::instrument_panel::oscillator_targets(&layer);
        let roots: Vec<_> = targets
            .iter()
            .filter(|t| t.group.is_none())
            .map(|t| t.leaf.as_str())
            .collect();
        assert_eq!(roots, ["Level", "Pitch", "Cents"]);
        let grouped = groups(&targets);
        assert_eq!(grouped.len(), count);
        for (i, group) in grouped.iter().enumerate() {
            assert_eq!(group.metadata.id, layer.oscillators[i].id);
            assert_eq!(group.metadata.name, format!("Osc {}", i + 1));
            assert_eq!(
                group
                    .targets
                    .iter()
                    .map(|t| t.leaf.as_str())
                    .collect::<Vec<_>>(),
                ["Pitch", "Gain", "Cents", "Width"]
            );
        }
        if count > 0 {
            assert_eq!(
                (targets[0].parameter, targets[0].id),
                (P::Pitch, Some(layer.oscillators[0].id))
            );
        }
    }
}

#[test]
fn all_variable_adapters_keep_output_roots_and_visible_strip_order() {
    let eq = daw_core::MultibandEqSettings::default();
    let targets = crate::multiband_eq_panel::modulation_targets(&eq);
    assert_eq!(groups(&targets).len(), eq.bands.len());
    assert_eq!(
        targets
            .iter()
            .filter(|t| t.group.is_none())
            .map(|t| t.label.as_str())
            .collect::<Vec<_>>(),
        ["Output · Gain"]
    );
    for group in groups(&targets) {
        assert_eq!(
            group
                .targets
                .iter()
                .map(|t| t.leaf.as_str())
                .collect::<Vec<_>>(),
            ["Gain", "Frequency", "Q"]
        );
    }
    let exciter = daw_core::ExciterSettings::default();
    let targets = crate::exciter_panel::modulation_targets(&exciter);
    assert_eq!(groups(&targets).len(), exciter.bands.len());
    for group in groups(&targets) {
        assert_eq!(
            group
                .targets
                .iter()
                .map(|t| t.leaf.as_str())
                .collect::<Vec<_>>(),
            ["Drive", "Level", "Mix"]
        );
    }
    assert_eq!(targets.iter().filter(|t| t.group.is_none()).count(), 2);
    let mut compressor = daw_core::MultibandCompressorSettings::default();
    compressor.presentation = compressor.bands.iter().rev().map(|b| b.id).collect();
    let targets = crate::multiband_compressor_panel::modulation_targets(&compressor);
    assert_eq!(
        targets[0].id,
        Some(compressor.bands[0].id),
        "display order must not change the new-route default"
    );
    let grouped = groups(&targets);
    assert_eq!(
        grouped.iter().map(|g| g.metadata.id).collect::<Vec<_>>(),
        compressor.presentation
    );
    assert_eq!(
        grouped[0].metadata.name, "Band 3",
        "names retain signal-band numbering"
    );
    assert_eq!(
        grouped[0]
            .targets
            .iter()
            .map(|t| t.leaf.as_str())
            .collect::<Vec<_>>(),
        ["Threshold", "Ratio", "Attack", "Release", "Knee", "Level"]
    );
}

#[test]
fn duplicate_macro_names_stay_separate_and_retained_controls_stay_hidden() {
    let mut module = daw_core::MacroModuleSettings {
        controls: (0..3)
            .map(|_| daw_core::MacroInstance {
                name: Some("Same · 名".into()),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    module.controls[1].present = false;
    let targets = crate::macro_panel::modulation_targets(&module);
    let grouped = groups(&targets);
    assert_eq!(grouped.len(), 2);
    assert_ne!(grouped[0].metadata.id, grouped[1].metadata.id);
    assert_eq!(targets[0].label, "Same · 名 · Value");
    assert_eq!(grouped[0].targets[0].leaf, "Value");
    assert!(targets.iter().all(|t| t.group.is_some()));
}

#[test]
fn reorder_add_remove_and_serialization_keep_the_assigned_target_identity() {
    let mut layer = oscillators(2);
    let id = layer.oscillators[1].id;
    let mut route = Modulation::new(P::Pitch);
    route.target = Some(id);
    layer.oscillator_modulations.push(route);
    layer.oscillators.swap(0, 1);
    layer.oscillators.push(Default::default());
    layer.oscillators.remove(1);
    let saved = serde_json::to_string(&layer).unwrap();
    let restored: InstrumentLayerSettings = serde_json::from_str(&saved).unwrap();
    let targets = crate::instrument_panel::oscillator_targets(&restored);
    assert_eq!(restored.oscillator_modulations[0], route);
    assert_eq!(
        targets
            .iter()
            .find(|t| t.id == route.target && t.parameter == route.parameter)
            .unwrap()
            .label,
        "Osc 1 · Pitch"
    );
}

#[test]
fn selecting_a_leaf_keeps_identity_clamps_amount_and_checks_target_sources() {
    let a = daw_core::Id::new_v4();
    let b = daw_core::Id::new_v4();
    let source = Source {
        audio: false,
        id: ModulationSourceId { layer: a, unit: a },
        label: "Source".into(),
        layer_name: "Layer".into(),
        name: "Source".into(),
        scope: ModulationScope::Shared,
        range: ModulationRange::Unipolar,
        behavior: daw_unit_core::ModulationBehavior::Continuous,
        enabled: true,
    };
    let sources = [source];
    let mut route = Modulation::new(P::Pitch);
    route.target = Some(a);
    route.source = Some(sources[0].id);
    route.enabled = false;
    let route_id = route.id;
    for amount in [-1000.0, 1000.0] {
        route.amount = amount;
        let target = Target::strip(P::OscillatorGain, a, 0, "A", "Gain");
        assign(&mut route, &target, ModulationScope::Shared, &|_| &sources);
        assert_eq!(route.amount, amount.signum() * P::OscillatorGain.limit());
        assert_eq!(route.source, Some(sources[0].id));
    }
    assign(
        &mut route,
        &Target::strip(P::MacroValue, b, 1, "B", "Value"),
        ModulationScope::Shared,
        &|_| &[],
    );
    assert_eq!(route.source, None);
    assert_eq!(route.id, route_id);
    assert!(!route.enabled);
}

struct Frame {
    button: Rect,
    text: Vec<(String, Rect)>,
    dividers: usize,
}
fn frame(
    ctx: &egui::Context,
    route: &mut Modulation,
    targets: &[Target],
    events: Vec<Event>,
) -> Frame {
    let mut button = Rect::NOTHING;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 650.0))),
            focused: true,
            events,
            ..Default::default()
        },
        |ui| {
            ui.set_width(100.0);
            button = destination_picker(ui, route, targets, ModulationScope::PerNote, |_| &[]).rect;
        },
    );
    let mut result = Frame {
        button,
        text: vec![],
        dividers: 0,
    };
    fn collect(shape: &Shape, result: &mut Frame) {
        match shape {
            Shape::Text(t) => result.text.push((
                t.galley.text().into(),
                Rect::from_min_size(t.pos, t.galley.size()),
            )),
            Shape::LineSegment { .. } => result.dividers += 1,
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, result);
                }
            }
            _ => {}
        }
    }
    for shape in &output.shapes {
        collect(&shape.shape, &mut result);
    }
    output.textures_delta.clear();
    result
}
fn click(ctx: &egui::Context, route: &mut Modulation, targets: &[Target], pos: Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            route,
            targets,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn rightmost(frame: &Frame, label: &str) -> Rect {
    frame
        .text
        .iter()
        .filter(|(text, _)| text == label)
        .map(|(_, rect)| *rect)
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap_or_else(|| panic!("missing {label}: {:?}", frame.text))
}
#[test]
fn actual_popup_browsing_is_read_only_and_submenu_leaf_assigns_then_closes() {
    for color in [
        crate::settings::ColorTheme::Dark,
        crate::settings::ColorTheme::Light,
        crate::settings::ColorTheme::Mist,
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
        let layer = oscillators(2);
        let targets = crate::instrument_panel::oscillator_targets(&layer);
        let mut route = Modulation::new(P::Pitch);
        route.target = targets[0].id;
        let before = route;
        let first = frame(&ctx, &mut route, &targets, vec![]);
        click(&ctx, &mut route, &targets, first.button.center());
        let opened = frame(&ctx, &mut route, &targets, vec![]);
        assert!(rightmost(&opened, "Level").top() < rightmost(&opened, "Osc 1").top());
        assert_eq!(opened.dividers, 1);
        assert!(!opened.text.iter().any(|(text, _)| text == "Gain"));
        let parent = rightmost(&opened, "Osc 2").center();
        click(&ctx, &mut route, &targets, parent);
        let child = frame(
            &ctx,
            &mut route,
            &targets,
            vec![Event::PointerMoved(parent)],
        );
        assert_eq!(route, before);
        let gain = rightmost(&child, "Gain");
        assert!(gain.left() > rightmost(&child, "Osc 2").left());
        click(&ctx, &mut route, &targets, gain.center());
        assert_eq!(
            (route.parameter, route.target),
            (P::OscillatorGain, Some(layer.oscillators[1].id))
        );
        let closed = frame(&ctx, &mut route, &targets, vec![]);
        assert!(!closed.text.iter().any(|(text, _)| text == "Osc 2"));
        assert!(closed.text.iter().any(|(text, _)| text == "Osc 2 · Gain"));
    }
}
