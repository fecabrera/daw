//! Shared live spectrum geometry and paint; no audio or parameter ownership.
use crate::{layout, theme};
use daw_unit_core::{SpectrumCache, SpectrumMeter, SpectrumReading};
use egui::{Id, Rect, Stroke, Ui, Vec2};
pub const WIDTH: f32 = 3.0 * layout::UNIT_STRIP_WIDTH;
pub fn frequency_x(plot: Rect, hz: f32) -> f32 {
    plot.left() + plot.width() * (hz.clamp(20.0, 20000.0) / 20.0).ln() / 1000.0_f32.ln()
}
pub fn cached_reading(ui: &Ui, meter: &SpectrumMeter) -> Option<SpectrumReading> {
    let id = ui.id().with("spectrum-cache");
    let mut cache = ui
        .data_mut(|d| d.get_temp::<SpectrumCache>(id))
        .unwrap_or_default();
    let reading = cache.read(meter);
    ui.data_mut(|d| d.insert_temp(id, cache));
    reading
}
pub(crate) fn axes(ui: &Ui, chart: Rect, range: (f32, f32), ticks: &[f32]) -> Rect {
    let plot = chart.shrink2(Vec2::new(24.0, 17.0));
    let palette = theme::palette(ui.ctx());
    let p = ui.painter_at(chart);
    p.rect_filled(chart, 2.0, palette.background);
    let grid = Stroke::new(1.0, palette.secondary.gamma_multiply(0.12));
    let font = egui::FontId::proportional(9.0);
    for hz in [
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ] {
        p.vline(frequency_x(plot, hz), plot.y_range(), grid);
        if [20.0, 100.0, 1000.0, 10000.0, 20000.0].contains(&hz) {
            let label = if hz >= 1000.0 {
                format!("{:.0}k", hz / 1000.0)
            } else {
                format!("{hz:.0}")
            };
            p.text(
                egui::pos2(frequency_x(plot, hz), plot.bottom() + 4.0),
                egui::Align2::CENTER_TOP,
                label,
                font.clone(),
                palette.secondary,
            );
        }
    }
    for db in ticks {
        let y = plot.bottom() - plot.height() * (db - range.0) / (range.1 - range.0);
        p.hline(
            plot.x_range(),
            y,
            if *db == 0.0 {
                Stroke::new(1.0, palette.secondary.gamma_multiply(0.5))
            } else {
                grid
            },
        );
        p.text(
            egui::pos2(plot.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{db:+.0}"),
            font.clone(),
            palette.secondary,
        );
    }
    plot
}
pub(crate) fn paint(ui: &Ui, id: Id, plot: Rect, reading: Option<&SpectrumReading>, enabled: bool) {
    let palette = theme::palette(ui.ctx());
    let points = crate::eq_spectrum::points(ui, id, plot, reading, enabled);
    if points.is_empty() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let base = mesh.vertices.len() as u32;
        for pos in [
            egui::pos2(pair[0].x, plot.bottom()),
            pair[0],
            pair[1],
            egui::pos2(pair[1].x, plot.bottom()),
        ] {
            mesh.colored_vertex(pos, palette.secondary.gamma_multiply(0.12));
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    let p = ui.painter_at(plot);
    p.add(egui::Shape::mesh(mesh));
    p.add(egui::Shape::line(
        points,
        Stroke::new(1.0, palette.secondary.gamma_multiply(0.5)),
    ));
}

/// Shared EQ-style handle. The caller paints any label separately.
pub(crate) fn band_handle(
    ui: &Ui,
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    selected: bool,
    enabled: bool,
) -> egui::Color32 {
    let palette = theme::palette(ui.ctx());
    let color = if selected && enabled {
        theme::accent(ui.ctx())
    } else {
        palette.secondary
    };
    painter.circle_filled(center, radius, palette.panel);
    painter.circle_stroke(
        center,
        radius + if selected { 1.0 } else { 0.0 },
        egui::Stroke::new(1.25, color),
    );
    color
}
