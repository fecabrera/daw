use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{AutoFilterSettings, FilterShape, LfoRateMode, LfoWave, NoteRate};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 4],
    pub lfo: Rect,
    pub sections: [Rect; 3],
}
fn float_choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    value: &mut T,
    choices: &[T],
    spec: KnobSpec,
) -> egui::Response {
    let mut position = choices.iter().position(|v| *v == *value).unwrap() as f32;
    let response = ui.add(Knob::new(&mut position, spec));
    if response.changed() {
        *value = choices[position.round().clamp(0.0, (choices.len() - 1) as f32) as usize];
    }
    response
}

fn filter_diagram(value: f32) -> Vec<Vec2> {
    let shape = FilterShape::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let band = (-((x - 0.5) * 5.0).powi(2)).exp();
            let level = match shape {
                FilterShape::LowPass => low,
                FilterShape::HighPass => 1.0 - low,
                FilterShape::BandPass => band,
                FilterShape::Notch => 1.0 - band,
            };
            Vec2::new(x - 0.5, 0.5 - level)
        })
        .collect()
}

fn lfo_diagram(value: f32) -> Vec<Vec2> {
    use daw_core::Waveform;
    let wave = LfoWave::ALL[value as usize];
    let waveform = match wave {
        LfoWave::Sine => Waveform::Sine,
        LfoWave::Triangle => Waveform::Triangle,
        LfoWave::SawUp | LfoWave::SawDown => Waveform::Saw,
        LfoWave::Square => Waveform::Square,
    };
    let mut points = knobs::waveform_path(waveform);
    if wave == LfoWave::SawDown {
        for point in &mut points {
            point.y = -point.y;
        }
    }
    points
}

// Saved note IDs stay unchanged; knob positions run from slowest to fastest.
const NOTE_RATES: [NoteRate; 27] = [
    NoteRate(1),
    NoteRate(0),
    NoteRate(4),
    NoteRate(2),
    NoteRate(3),
    NoteRate(7),
    NoteRate(5),
    NoteRate(6),
    NoteRate(10),
    NoteRate(8),
    NoteRate(9),
    NoteRate(13),
    NoteRate(11),
    NoteRate(12),
    NoteRate(16),
    NoteRate(14),
    NoteRate(15),
    NoteRate(19),
    NoteRate(17),
    NoteRate(18),
    NoteRate(22),
    NoteRate(20),
    NoteRate(21),
    NoteRate(25),
    NoteRate(23),
    NoteRate(24),
    NoteRate(26),
];
fn note_label(position: f32) -> String {
    NOTE_RATES[position.round().clamp(0.0, f32::from(NoteRate::COUNT - 1)) as usize].label()
}

fn frequency() -> KnobSpec {
    KnobSpec::new("Filter frequency", 20.0..=20_000.0, 1000.0, 1.0)
        .scale(
            |v| (v / 20.0).log10() / 3.0,
            |p| 20.0 * 1000_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels("20", "20k")
}
fn phase(name: &'static str) -> KnobSpec {
    KnobSpec::new(name, -180.0..=180.0, 0.0, 1.0)
        .centered(0.0)
        .bipolar()
        .value_label(|v| format!("{v:.0}°"))
        .format(|v| format!("{v:.0}°"))
        .endpoint_labels("−180", "+180")
}
fn row(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    strip: usize,
    index: usize,
    markers: &[Option<knobs::ModulationMarker>; 4],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        match (strip, index) {
            (0, 0) => float_choice(ui, &mut s.shape, FilterShape::ALL,
                KnobSpec::new("Filter shape", 0.0..=3.0, 0.0, 1.0)
                    .stepped(1.0).cyclic().diagrams(filter_diagram)
                    .format(|v| FilterShape::ALL[v as usize].name().into())),
            (0, 1) => ui.add(Knob::new(&mut s.frequency_hz, frequency()).modulated(markers[0].as_ref(), &mut s.modulation_open)),
            (0, 2) => ui.add(Knob::new(
                &mut s.q,
                KnobSpec::new(
                    "Resonance",
                    0.1..=10.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.01,
                )
                .scale(
                    |v| (v / 0.1).log10() / 2.0,
                    |p| 0.1 * 100_f32.powf(p),
                    |v, d| v * 2_f32.powf(d / 24.0),
                )
                .value_label(|v| format!("Q {v:.2}"))
                .format(|v| format!("Q {v:.2}"))
                .endpoint_labels("0.1", "10"),
            ).modulated(markers[1].as_ref(), &mut s.modulation_open)),
            (1, 0) => float_choice(ui, &mut s.wave, LfoWave::ALL,
                KnobSpec::new("LFO wave", 0.0..=4.0, 0.0, 1.0)
                    .stepped(1.0).cyclic().diagrams(lfo_diagram)
                    .format(|v| LfoWave::ALL[v as usize].name().into())),
            (1, 1) => float_choice(ui, &mut s.rate_mode, LfoRateMode::ALL,
                KnobSpec::new("LFO rate mode", 0.0..=1.0, 1.0, 1.0)
                    .two_way_labels("Hz", "Note")
                    .format(|v| LfoRateMode::ALL[v as usize].name().into())),
            (1, 2) if s.rate_mode == LfoRateMode::Hz => ui.add(Knob::new(
                &mut s.rate_hz,
                KnobSpec::new("LFO rate", 0.01..=20.0, 2.0, 0.01)
                    .scale(
                        |v| (v / 0.01).ln() / 2000_f32.ln(),
                        |p| 0.01 * 2000_f32.powf(p),
                        |v, d| v * 2_f32.powf(d / 24.0),
                    )
                    .value_label(|v| format!("{v:.2} Hz"))
                    .format(|v| format!("{v:.2} Hz"))
                    .endpoint_labels("0.01", "20"),
            )),
            (1, 2) => {
                let mut position = NOTE_RATES.iter().position(|rate| *rate == s.note_rate).unwrap() as f32;
                let response = ui.add(Knob::new(
                    &mut position,
                    KnobSpec::new("LFO note duration", 0.0..=f32::from(NoteRate::COUNT - 1), 13.0, 1.0)
                        .stepped(1.0)
                        // Straight durations in the slowest-to-fastest choice order.
                        .reference_values(&[1.0, 4.0, 7.0, 10.0, 13.0, 16.0, 19.0, 22.0, 25.0])
                        .value_label(note_label)
                        .format(note_label)
                        .endpoint_labels("4b D", "1/64 T"),
                ))
                .on_hover_text("D = dotted; T = triplet. Bounded from dotted 4 bars to triplet 1/64, slowest to fastest. Note rates follow project tempo and transport.");
                if response.changed() {
                    s.note_rate = NOTE_RATES[position.round().clamp(0.0, f32::from(NoteRate::COUNT - 1)) as usize];
                }
                response
            }
            (2, 0) => ui.add(Knob::new(
                &mut s.strength_octaves,
                KnobSpec::new("LFO strength", 0.0..=6.0, 1.0, 0.1)
                    .reference_step(1.0)
                    .value_label(|v| format!("{v:.1} oct"))
                    .format(|v| format!("{v:.1} oct"))
                    .endpoint_labels("0", "6"),
            )),
            (2, 1) => ui.add(Knob::new(
                &mut s.sync_phase_degrees,
                phase("LFO sync phase"),
            )),
            (2, 2) => ui.add(Knob::new(
                &mut s.stereo_phase_degrees,
                phase("LFO stereo phase"),
            )),
            (3, 0) => ui.add(Knob::new(&mut s.level_db, knobs::marked_gain_spec().named("Filter output level")).modulated(markers[2].as_ref(), &mut s.modulation_open)),
            (3, 1) => ui.add(Knob::new(
                &mut s.dry_wet,
                KnobSpec::new("Dry/wet", 0.0..=1.0, 1.0, 0.01)
                    .value_label(|v| format!("{:.0}%", v * 100.0))
                    .format(|v| format!("{:.0}%", v * 100.0))
                    .endpoint_labels("Dry", "Wet"),
            ).modulated(markers[3].as_ref(), &mut s.modulation_open)),
            _ => ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover()),
        }
    })
    .rect
}
fn controls(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    strip: usize,
    bounds: &mut Bounds,
    markers: &[Option<knobs::ModulationMarker>; 4],
) {
    for index in 0..3 {
        bounds.knobs[strip][index] = row(ui, settings, strip, index, markers);
    }
}
fn footer(ui: &mut Ui, name: &str, compact: bool) {
    rows::centered(
        ui,
        if compact {
            crate::envelope_panel::body_height(true)
        } else {
            icons::COMPACT_BUTTON_SIZE
        },
        |ui| {
            rows::unit_label(ui, name);
        },
    );
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut AutoFilterSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
    sample_rate_hz: u32,
) -> (Bounds, egui::Response) {
    ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),|ui| {
    let modulation_open=settings.modulation_open;
    let bases = [settings.frequency_hz, settings.q, settings.level_db, settings.dry_wet];
    let mut markers: [Option<knobs::ModulationMarker>; 4] = std::array::from_fn(|i| crate::modulation_panel::marker(
        &settings.modulations, sources, daw_core::ModulationParameter::FILTER[i], bases[i], settings.enabled,
        (i==0).then_some((settings.lfo_enabled, settings.strength_octaves)),
        [live.map(|v| v[if i==0 {0}else{i+1}]), if i==0 {live.map(|v|v[1])}else{None}],
    ));
    if let Some(marker) = &mut markers[0] { marker.range = marker.range.map(|v| v.min(sample_rate_hz as f32 * 0.45)); }
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 4],
        lfo: Rect::NOTHING,
        sections: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(ui, ("Auto-filter", compact), height, &mut enabled, &mut collapsed, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for section in 0..3 {
                let rect = ui.push_id(section, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        // One LFO strip owns two knob columns and three equal horizontal gaps.
                        // Match the control inset in the single-column Cutoff and Out strips.
                        let column_width = knobs::CHOICE_SIZE;
                        let column_padding = (UNIT_STRIP_WIDTH - column_width) * 0.5;
                        let section_width = if section == 1 {
                            2.0 * column_width + 3.0 * column_padding
                        } else {
                            UNIT_STRIP_WIDTH
                        };
                        let width = section_width - 2.0 * f32::from(panel.margin);
                        ui.set_width(width);
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        if !compact {
                            if section == 1 {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = column_padding;
                                    ui.add_space(column_padding - f32::from(panel.margin));
                                    for strip in 1..=2 { ui.push_id(strip, |ui| ui.allocate_ui_with_layout(Vec2::new(column_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                        ui.set_width(column_width);
                                        controls(ui, settings, strip, &mut bounds, &markers);
                                    })); }
                                });
                            } else { controls(ui, settings, if section == 0 { 0 } else { 3 }, &mut bounds, &markers); }
                        }
                        if section == 1 {
                            rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                                let response = rows::power_label(ui, "LFO", "Toggle filter modulation", settings.lfo_enabled).on_hover_text("Off keeps the static filter active at its Frequency and Resonance settings.");
                                bounds.lfo = response.rect;
                                if response.clicked() { settings.lfo_enabled = !settings.lfo_enabled; }
                            });
                        } else { footer(ui, if section == 0 { "Cutoff" } else { "Out" }, compact); }
                    });
                })).inner.response.rect;
                bounds.sections[section] = rect;
                if section > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
            }
            if modulation_open {
                let rect = bounds.sections[2];
                panels::unit_divider(ui, rect.right(), rect.y_range().into());
                crate::modulation_panel::body(ui, &mut settings.modulations, sources, &daw_core::ModulationParameter::FILTER, compact);
            }
        });
    });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    crate::modulation_panel::toggle(ui,&result.header,settings.collapsed,&mut settings.modulation_open);
    (bounds, result.header)
    }).inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagrams_show_the_filter_response_and_each_distinct_waveform() {
        let low = filter_diagram(0.0);
        let high = filter_diagram(1.0);
        let band = filter_diagram(2.0);
        let notch = filter_diagram(3.0);
        assert!(low[0].y < low[24].y);
        assert!(high[0].y > high[24].y);
        assert!(band[12].y < band[0].y && band[12].y < band[24].y);
        assert!(notch[12].y > notch[0].y && notch[12].y > notch[24].y);
        for (index, _) in LfoWave::ALL.iter().enumerate() {
            let wave = lfo_diagram(index as f32);
            assert!(
                wave.iter()
                    .all(|point| point.x.abs() <= 0.5 && point.y.abs() <= 0.5)
            );
            for other in 0..index {
                assert_ne!(wave, lfo_diagram(other as f32));
            }
        }
        let up = lfo_diagram(2.0);
        let down = lfo_diagram(3.0);
        assert!(up[0].y > up[1].y);
        assert!(down[0].y < down[1].y);
    }
}
