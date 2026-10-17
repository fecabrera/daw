//! Range-bar geometry shared by the project loop ruler and sample selection.
use crate::{LoopDragMode, theme};
use egui::{Pos2, Rect, Ui};

pub(super) struct Gesture {
    pub mode: LoopDragMode,
    pub origin_frame: u64,
    pub at_frame: u64,
    pub delta_frames: i128,
}

/// Compute the complete bounded preview for both project loops and sampler selections.
pub(super) fn preview(
    gesture: Gesture,
    original: [u64; 2],
    maximum: u64,
    bypass: bool,
    mut snap: impl FnMut(i128, std::ops::RangeInclusive<u64>, bool) -> u64,
) -> [u64; 2] {
    let [a, b] = original;
    let at = gesture.at_frame;
    match gesture.mode {
        LoopDragMode::Create => {
            let origin = snap(i128::from(gesture.origin_frame), 0..=maximum, bypass);
            let at = snap(i128::from(at), 0..=maximum, bypass);
            [origin.min(at), origin.max(at)]
        }
        LoopDragMode::Start => [snap(i128::from(at), 0..=b - 1, bypass || at == a), b],
        LoopDragMode::End => [a, snap(i128::from(at), a + 1..=maximum, bypass || at == b)],
        LoopDragMode::Move => {
            let length = b - a;
            let start = snap(
                i128::from(a) + gesture.delta_frames,
                0..=maximum - length,
                bypass || gesture.delta_frames == 0,
            );
            [start, start + length]
        }
    }
}

pub(super) fn mode(left: f32, right: f32, pointer: Pos2) -> LoopDragMode {
    let a = (pointer.x - left).abs();
    let b = (pointer.x - right).abs();
    if right <= left {
        LoopDragMode::Create
    } else if a.min(b) <= 5.0 {
        if a <= b {
            LoopDragMode::Start
        } else {
            LoopDragMode::End
        }
    } else if (left..right).contains(&pointer.x) {
        LoopDragMode::Move
    } else {
        LoopDragMode::Create
    }
}
pub(super) fn paint(ui: &Ui, rect: Rect, left: f32, right: f32, enabled: bool) {
    if right <= rect.left() || left >= rect.right() || right <= left {
        return;
    }
    let (fill, handle) = theme::ruler_loop_colors(ui.ctx(), enabled);
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(left.max(rect.left()), rect.top()),
            Pos2::new(right.min(rect.right()), rect.bottom()),
        ),
        0.0,
        fill,
    );
    for x in [left, right] {
        if (rect.left()..=rect.right()).contains(&x) {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - 1.5, rect.top() + 2.0),
                    Pos2::new(x + 1.5, rect.bottom() - 2.0),
                ),
                0.0,
                handle,
            );
        }
    }
}
