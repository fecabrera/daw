//! Shared level rails for effect Out strips and band monitors.
use crate::theme;
use egui::{Rect, Sense, Ui, Vec2};
pub const MIN_DBFS: f32 = -60.0;

/// Hide queued or stale readings as soon as the unit's saved power state is off.
pub fn unit_reading<T: Default>(enabled: bool, reading: T) -> T {
    if enabled { reading } else { T::default() }
}

/// Fit a four-point monitor above standard knob rows without changing unit height.
pub fn unit_monitor_rail(strip: Rect, margin: i8) -> Rect {
    Rect::from_min_size(
        strip.min + Vec2::new(f32::from(margin), 2.0),
        Vec2::new(strip.width() - 2.0 * f32::from(margin), 4.0),
    )
}

pub fn horizontal_rail(ui: &Ui, rail: Rect, progress: f32, color: egui::Color32) {
    ui.painter()
        .rect_filled(rail, 1.0, theme::palette(ui.ctx()).input);
    if progress > 0.0 {
        ui.painter().rect_filled(
            Rect::from_min_size(rail.min, Vec2::new(rail.width() * progress, rail.height())),
            1.0,
            color,
        );
    }
}

/// Disabled bands share the theme's secondary color for every monitor type.
pub fn band_color(ui: &Ui, enabled: bool, active: egui::Color32) -> egui::Color32 {
    if enabled {
        active
    } else {
        theme::palette(ui.ctx()).secondary
    }
}

/// Stereo peak level using the shared −60 to 0 dBFS scale; retain headroom in readings.
pub fn band_level_rail(ui: &mut Ui, rail: Rect, name: &str, peak: f32, enabled: bool) {
    let peak = if peak.is_finite() { peak.max(0.0) } else { 0.0 };
    let palette = theme::palette(ui.ctx());
    horizontal_rail(
        ui,
        rail,
        position_for_peak(peak),
        band_color(
            ui,
            enabled,
            if peak > 1.0 {
                palette.error
            } else {
                theme::accent(ui.ctx())
            },
        ),
    );
    let db = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        f32::NEG_INFINITY
    };
    let level = peak_text(peak);
    let response = ui.interact(rail, ui.id().with(("band-level", name)), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            ui.is_enabled(),
            format!("{name} output level: {level} dBFS"),
        );
        info.value = db.is_finite().then_some(f64::from(db));
        info
    });
    response.on_hover_text(format!(
        "{name} output level: {level} dBFS (display range −60 to 0 dBFS)"
    ));
}

pub fn position_for_dbfs(db: f32) -> f32 {
    ((db - MIN_DBFS) / -MIN_DBFS).clamp(0.0, 1.0)
}

pub fn position_for_peak(peak: f32) -> f32 {
    if peak > 0.0 {
        position_for_dbfs(20.0 * peak.log10())
    } else {
        0.0
    }
}

pub fn peak_text(peak: f32) -> String {
    if peak > 0.0 {
        let db = 20.0 * peak.log10();
        if db > 0.0 {
            format!("+{db:.1}")
        } else {
            format!("{db:.1}")
        }
    } else {
        "−∞".into()
    }
}

/// Current linked-stereo level and the shared click-resettable sample peak.
#[derive(Clone, Copy)]
pub struct OutputLevel<'a> {
    current: f32,
    peak: Option<&'a daw_unit_core::PeakHold>,
    reset_request: Option<&'a std::sync::atomic::AtomicBool>,
}
impl<'a> OutputLevel<'a> {
    pub fn with_reset_request(mut self, request: &'a std::sync::atomic::AtomicBool) -> Self {
        self.reset_request = Some(request);
        self
    }
    pub fn new(current: f32, peak: Option<&'a daw_unit_core::PeakHold>) -> Self {
        Self {
            current,
            peak,
            reset_request: None,
        }
    }
}
impl From<f32> for OutputLevel<'_> {
    fn from(current: f32) -> Self {
        Self::new(current, None)
    }
}

/// Shared one-point held-peak marker for output, gain-change, and reduction rails.
pub fn peak_marker(ui: &Ui, rail: Rect, x: f32) {
    ui.painter().vline(
        x.clamp(rail.left() + 0.5, rail.right() - 0.5),
        rail.y_range(),
        egui::Stroke::new(1.0, theme::palette(ui.ctx()).secondary),
    );
}

/// Standard effect Out-strip monitor. Hide stale values immediately on bypass.
/// The rail occupies the existing top margin, so controls and strip size do not move.
pub fn effect_output_level<'a>(
    ui: &mut Ui,
    strip: Rect,
    margin: i8,
    reading: impl Into<OutputLevel<'a>>,
    enabled: bool,
) -> Rect {
    let reading = reading.into();
    if !enabled && let Some(peak) = reading.peak {
        peak.reset_peak();
    }
    let current = unit_reading(enabled, reading.current);
    let current = if current.is_finite() {
        current.max(0.0)
    } else {
        0.0
    };
    let held = unit_reading(enabled, reading.peak.map_or(0.0, |p| p.peak()));
    let rail = unit_monitor_rail(strip, margin);
    let palette = theme::palette(ui.ctx());
    horizontal_rail(
        ui,
        rail,
        position_for_peak(current),
        if current > 1.0 {
            palette.error
        } else {
            theme::accent(ui.ctx())
        },
    );
    if held > 0.0 {
        peak_marker(
            ui,
            rail,
            rail.left() + rail.width() * position_for_peak(held),
        );
    }
    let response = ui.interact(rail, ui.id().with("effect-output-level"), Sense::click());
    if enabled
        && response.clicked_by(egui::PointerButton::Primary)
        && ui.is_enabled()
        && ui.input(|i| i.focused)
    {
        if let Some(peak) = reading.peak {
            peak.reset_peak();
        }
        if let Some(request) = reading.reset_request {
            request.store(true, std::sync::atomic::Ordering::Release);
        }
        ui.ctx().request_repaint();
    }
    let level = peak_text(current);
    let peak = peak_text(held);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Output level: {level} dBFS. Held peak: {peak} dBFS. Click to reset peak"),
        );
        if current > 0.0 {
            info.value = Some(f64::from(20.0 * current.log10()));
        }
        info
    });
    response.on_hover_text(format!("Output level: {level} dBFS. Held peak: {peak} dBFS. Click to reset peak. Display range −60 to 0 dBFS."));
    rail
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{MeterStorage, PeakMeter};
    fn frame(
        ctx: &egui::Context,
        meter: &PeakMeter,
        current: f32,
        enabled: bool,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(90.0, 218.0),
                )),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| {
                effect_output_level(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(90.0, 218.0)),
                    10,
                    OutputLevel::new(current, Some(meter.hold())),
                    enabled,
                );
            },
        );
        output.textures_delta.clear();
        std::mem::take(&mut output.shapes)
    }
    #[test]
    fn standard_output_rail_preserves_geometry_overload_and_bypass_in_all_themes() {
        for (base, palette) in [
            (egui::Theme::Dark, theme::DARK),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            theme::configure(&ctx);
            theme::apply(&ctx, base, palette, theme::ACCENT);
            let meter = PeakMeter::default();
            meter.store(2.0);
            let shapes = frame(&ctx, &meter, 2.0, true, vec![]);
            let rail = Rect::from_min_size(egui::pos2(10.0, 2.0), Vec2::new(70.0, 4.0));
            assert!(shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rail && r.fill==palette.error)
            ));
            assert!(shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke} if points[0].x==rail.right()-0.5 && stroke.color==palette.secondary)));
            let shapes = frame(&ctx, &meter, 2.0, false, vec![]);
            assert_eq!(meter.peak(), 0.0);
            assert!(
                !shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::LineSegment { .. }))
            );
            assert!(!shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.intersects(rail) && (r.fill==palette.error || r.fill==theme::ACCENT))));
        }
    }
    #[test]
    fn clicking_the_standard_rail_resets_only_the_shared_peak() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        theme::configure(&ctx);
        let meter = PeakMeter::default();
        meter.store(0.8);
        meter.initialize(0.2);
        frame(&ctx, &meter, 0.2, true, vec![]);
        let pos = egui::pos2(45.0, 4.0);
        for pressed in [true, false] {
            frame(
                &ctx,
                &meter,
                0.2,
                true,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(meter.peak(), 0.0);
        assert_eq!(meter.reading(), 0.2);
    }
}
