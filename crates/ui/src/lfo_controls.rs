//! Shared LFO parameter widgets for the component and Auto-filter.
use crate::knobs::{self, Knob, KnobSpec};
use daw_unit_core::{LfoRateMode, LfoWave, NoteRate};
use egui::{Ui, Vec2};
pub(super) fn choice<T: Copy + PartialEq>(
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

pub(super) fn diagram(value: f32) -> Vec<Vec2> {
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

pub(super) fn phase_spec(name: &'static str) -> KnobSpec {
    KnobSpec::new(name, -180.0..=180.0, 0.0, 1.0)
        .centered(0.0)
        .bipolar()
        .value_label(|v| format!("{v:.0}°"))
        .format(|v| format!("{v:.0}°"))
        .endpoint_labels("−180", "+180")
}

pub(super) fn wave(ui: &mut Ui, wave: &mut LfoWave) -> egui::Response {
    choice(
        ui,
        wave,
        LfoWave::ALL,
        KnobSpec::new("LFO wave", 0.0..=4.0, 0.0, 1.0)
            .stepped(1.0)
            .cyclic()
            .diagrams(diagram)
            .format(|v| LfoWave::ALL[v as usize].name().into()),
    )
}
pub(super) fn mode(ui: &mut Ui, mode: &mut LfoRateMode) -> egui::Response {
    choice(
        ui,
        mode,
        LfoRateMode::ALL,
        KnobSpec::new("LFO rate mode", 0.0..=1.0, 1.0, 1.0)
            .two_way_labels("Hz", "Note")
            .format(|v| LfoRateMode::ALL[v as usize].name().into()),
    )
}
pub(super) fn rate(
    ui: &mut Ui,
    mode: LfoRateMode,
    hz: &mut f32,
    note: &mut NoteRate,
) -> egui::Response {
    if mode == LfoRateMode::Hz {
        ui.add(Knob::new(
            hz,
            KnobSpec::new("LFO rate", 0.01..=20.0, 2.0, 0.01)
                .scale(
                    |v| (v / 0.01).ln() / 2000_f32.ln(),
                    |p| 0.01 * 2000_f32.powf(p),
                    |v, d| v * 2_f32.powf(d / 24.0),
                )
                .value_label(|v| format!("{v:.2} Hz"))
                .format(|v| format!("{v:.2} Hz"))
                .endpoint_labels("0.01", "20"),
        ))
    } else {
        let mut position = NOTE_RATES
            .iter()
            .position(|rate| *rate == *note)
            .unwrap_or(13) as f32;
        let response = ui.add(Knob::new(
            &mut position,
            KnobSpec::new("LFO note duration", 0.0..=f32::from(NoteRate::COUNT - 1), 13.0, 1.0)
                .stepped(1.0)
                .reference_values(&[1.0,4.0,7.0,10.0,13.0,16.0,19.0,22.0,25.0])
                .value_label(note_label)
                .format(note_label)
                .endpoint_labels("4b D", "1/64 T"),
        )).on_hover_text("D = dotted; T = triplet. Bounded from dotted 4 bars to triplet 1/64, slowest to fastest. Note rates follow project tempo and transport.");
        if response.changed() {
            *note =
                NOTE_RATES[position.round().clamp(0.0, f32::from(NoteRate::COUNT - 1)) as usize];
        }
        response
    }
}
