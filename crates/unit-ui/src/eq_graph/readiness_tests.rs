use super::*;
use daw_unit_core::{
    MeterStorage, SPECTRUM_BASS_READY, SPECTRUM_UPPER_READY, SpectrumCache, SpectrumMeter,
};
use std::time::Duration;

fn draw(
    ctx: &egui::Context,
    settings: &mut MultibandEqSettings,
    reading: Option<&SpectrumReading>,
    time: f64,
    visible: bool,
) -> (Bounds, egui::FullOutput) {
    let mut bounds = Bounds::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 320.0))),
            ..Default::default()
        },
        |ui| {
            bounds = show(
                ui,
                settings,
                layout::unit_control_height(),
                visible,
                Input {
                    live_edits: false,
                    spectrum: reading,
                    sample_rate_hz: 48000,
                    effective: &[],
                    output_gain_db: None,
                    response: |_, rate| FrequencyResponse::unity(rate),
                },
            );
        },
    );
    output.textures_delta.clear();
    (bounds, output)
}
fn footer(output: &egui::FullOutput) -> Option<(&str, Rect)> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text().starts_with("Response") => Some((
            text.galley.text(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        _ => None,
    })
}

#[test]
fn full_range_and_legacy_readiness_keep_the_response_footer_without_resizing_or_editing() {
    for (mode, palette) in [
        (egui::Theme::Dark, theme::LIGHT),
        (egui::Theme::Light, theme::LIGHT),
        (egui::Theme::Light, theme::MIST),
    ] {
        for scale in [1.0, 1.25, 2.0] {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            theme::apply(&ctx, mode, palette, theme::ACCENT);
            ctx.set_pixels_per_point(scale);
            let mut settings = MultibandEqSettings::default();
            let original = settings.clone();
            let meter = SpectrumMeter::default();
            let mut cache = SpectrumCache::default();
            let mut time = 0.0;
            let (baseline, out) = draw(&ctx, &mut settings, None, time, true);
            let (_, baseline_footer) = footer(&out).unwrap();
            for (ready, caption) in [
                (0, "Response"),
                (SPECTRUM_UPPER_READY, "Response"),
                (SPECTRUM_UPPER_READY | SPECTRUM_BASS_READY, "Response"),
                (SPECTRUM_UPPER_READY, "Response"),
            ] {
                meter.store(SpectrumReading {
                    sample_rate_hz: 48000,
                    valid_branches: ready,
                    upper_end_frame: if ready != 0 { 4096 } else { 0 },
                    bass_end_frame: if ready & SPECTRUM_BASS_READY != 0 {
                        32831
                    } else {
                        0
                    },
                    ..Default::default() // Valid silence must not show a collection status.
                });
                let frame = cache.read(&meter);
                time += 1.0 / 60.0;
                let (bounds, output) = draw(&ctx, &mut settings, frame.as_ref(), time, true);
                let (text, rect) = footer(&output).unwrap();
                assert_eq!(text, caption);
                assert_eq!(bounds.section, baseline.section);
                assert_eq!(bounds.plot, baseline.plot);
                assert_eq!(bounds.nodes, baseline.nodes);
                assert_eq!(rect.height(), baseline_footer.height());
                assert!((rect.center().y - baseline_footer.center().y).abs() < 0.01);
                assert!(bounds.section.contains_rect(rect));
            }
            meter.clear_at_rate(96000);
            let frame = cache.read(&meter);
            time += 1.0 / 120.0;
            let (_, output) = draw(&ctx, &mut settings, frame.as_ref(), time, true);
            assert_eq!(footer(&output).unwrap().0, "Response");
            let partial = SpectrumReading {
                valid_branches: SPECTRUM_UPPER_READY,
                ..Default::default()
            };
            settings.enabled = false;
            let (_, output) = draw(&ctx, &mut settings, Some(&partial), time + 0.019, true);
            assert_eq!(footer(&output).unwrap().0, "Response");
            settings.enabled = true;
            let (_, output) = draw(&ctx, &mut settings, Some(&partial), time + 0.041, false);
            assert!(footer(&output).is_none());
            let (_, output) = draw(&ctx, &mut settings, None, time + 0.05, true);
            assert_eq!(footer(&output).unwrap().0, "Response");
            assert_eq!(settings, original);
        }
    }
}

#[test]
fn silent_readings_do_not_request_collection_repaints() {
    for step in [1.0 / 60.0, 1.0 / 120.0, 0.023] {
        let ctx = egui::Context::default();
        let mut settings = MultibandEqSettings::default();
        let mut time = 0.0;
        let mut partial = SpectrumReading {
            sample_rate_hz: 48000,
            valid_branches: SPECTRUM_UPPER_READY,
            ..Default::default()
        };
        for reading in [
            None,
            Some(partial),
            {
                partial.valid_branches |= SPECTRUM_BASS_READY;
                Some(partial)
            },
            None,
        ] {
            let mut delay = Duration::ZERO;
            for _ in 0..8 {
                time += step;
                let (_, output) = draw(&ctx, &mut settings, reading.as_ref(), time, true);
                delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            }
            assert!(delay > Duration::from_millis(16), "idle repaint {delay:?}");
        }
    }
}
