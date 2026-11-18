//! Pointer gesture primitives shared by catalog and installed-unit drags.
use egui::{Context, Pos2, Response, Ui, Vec2};

pub fn cancelled(ui: &Ui) -> bool {
    !ui.is_enabled() || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
}

/// Arm on press, including a motion and release delivered together next frame.
pub fn press_origin(ui: &Ui, response: &Response) -> Option<Pos2> {
    if cancelled(ui)
        || !response.is_pointer_button_down_on()
        || !ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
    {
        return None;
    }
    ui.input(|input| input.pointer.press_origin())
        .filter(|origin| response.rect.contains(*origin))
}

pub fn crossed_threshold(ui: &Ui, origin: Pos2) -> bool {
    let threshold = ui
        .ctx()
        .options(|options| options.input_options.max_click_dist);
    ui.input(|input| input.pointer.latest_pos())
        .is_some_and(|pointer| pointer.distance(origin) > threshold)
}

pub fn restore_focus(ctx: &Context, focus: egui::Id) {
    ctx.memory_mut(|memory| memory.request_focus(focus));
}

#[derive(Clone, Copy)]
pub enum Axis {
    Horizontal,
    Vertical,
}

// Logical points: exceed the point hit target before changing drag direction.
const AXIS_SWITCH_DISTANCE: f32 = 12.0;
/// Locks a drag to its first dominant axis until the pointer crosses the other axis tolerance.
#[derive(Clone, Copy)]
pub struct AxisLock {
    axis: Option<Axis>,
    anchor: Pos2,
    previous: Pos2,
    effective: Pos2,
    horizontal: bool,
}
impl AxisLock {
    pub fn new(origin: Pos2, horizontal: bool) -> Self {
        Self {
            axis: None,
            anchor: origin,
            previous: origin,
            effective: origin,
            horizontal,
        }
    }

    pub(crate) fn delta(&mut self, pointer: Pos2) -> Vec2 {
        let before = self.effective;
        self.update(pointer) - before
    }

    pub fn update(&mut self, pointer: Pos2) -> Pos2 {
        if pointer == self.previous {
            return self.effective;
        }
        let delta = pointer - self.previous;
        let Some(axis) = self.axis else {
            let axis = if self.horizontal && delta.x.abs() > delta.y.abs() {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
            self.axis = Some(axis);
            self.anchor = pointer;
            match axis {
                Axis::Horizontal => self.effective.x += delta.x,
                Axis::Vertical => self.effective.y += delta.y,
            }
            self.previous = pointer;
            return self.effective;
        };
        let cross = match axis {
            Axis::Horizontal => pointer.y - self.anchor.y,
            Axis::Vertical => pointer.x - self.anchor.x,
        };
        if self.horizontal && cross.abs() >= AXIS_SWITCH_DISTANCE {
            // Consume the tolerance before switching; never apply ignored motion
            // all at once to the other parameter.
            let excess = cross - cross.signum() * AXIS_SWITCH_DISTANCE;
            self.axis = Some(match axis {
                Axis::Horizontal => {
                    self.effective.y += excess;
                    Axis::Vertical
                }
                Axis::Vertical => {
                    self.effective.x += excess;
                    Axis::Horizontal
                }
            });
            self.anchor = pointer;
        } else {
            match axis {
                Axis::Horizontal => self.effective.x += delta.x,
                Axis::Vertical => self.effective.y += delta.y,
            }
        }
        self.previous = pointer;
        self.effective
    }
}

/// Scroll the containing viewport while the pointer is held near an edge.
pub fn scroll_near_edges(ui: &Ui, axis: Axis, speed: f32) {
    scroll_drag(ui, axis, speed, ui.is_enabled(), ui.clip_rect());
}

/// The host can keep a validated drag scrolling while its parameter controls are disabled.
pub fn scroll_drag(ui: &Ui, axis: Axis, speed: f32, allowed: bool, bounds: egui::Rect) {
    if !allowed
        || ui.input(|input| {
            !input.focused || input.key_pressed(egui::Key::Escape) || !input.pointer.primary_down()
        })
    {
        return;
    }
    let viewport = ui.clip_rect().intersect(bounds);
    let Some(pointer) = ui
        .input(|input| input.pointer.latest_pos())
        .filter(|p| viewport.contains(*p))
    else {
        return;
    };
    let (position, start, end) = match axis {
        Axis::Horizontal => (pointer.x, viewport.left(), viewport.right()),
        Axis::Vertical => (pointer.y, viewport.top(), viewport.bottom()),
    };
    let edge = 24.0_f32.min((end - start) * 0.5);
    let direction = if position < start + edge {
        1.0
    } else if position > end - edge {
        -1.0
    } else {
        return;
    };
    let delta = direction * speed * ui.input(|input| input.stable_dt.min(0.05));
    ui.scroll_with_delta_animation(
        match axis {
            Axis::Horizontal => Vec2::new(delta, 0.0),
            Axis::Vertical => Vec2::new(0.0, delta),
        },
        egui::style::ScrollAnimation::none(),
    );
    ui.ctx().request_repaint();
}

/// Shared horizontal insertion policy for effects, oscillator sources and routes.
pub fn insertion_slot(pointer: Pos2, slots: impl IntoIterator<Item = egui::Rect>) -> usize {
    slots
        .into_iter()
        .take_while(|r| pointer.x >= r.center().x)
        .count()
}
/// Shared group insertion and clean no-op calculation for installed unit lanes.
pub fn group_insertion<T: Copy + Eq>(order: &[T], moving: &[T], slot: usize) -> (usize, bool) {
    let index = order[..slot].iter().filter(|c| !moving.contains(c)).count();
    let mut next: Vec<_> = order
        .iter()
        .copied()
        .filter(|c| !moving.contains(c))
        .collect();
    next.splice(index..index, moving.iter().copied());
    (index, next != order)
}
pub fn insertion_line(ui: &Ui, slots: &[egui::Rect], slot: usize, viewport: egui::Rect) {
    if viewport.width() < 2.0 || viewport.height() <= 0.0 {
        return;
    }
    let x = slots
        .get(slot)
        .map_or_else(
            || slots.last().map_or(viewport.left(), |r| r.right()),
            |r| r.left(),
        )
        .clamp(viewport.left() + 1.0, viewport.right() - 1.0);
    ui.painter_at(viewport).vline(
        x,
        viewport.y_range(),
        egui::Stroke::new(2.0, crate::theme::accent(ui.ctx())),
    );
}

#[cfg(test)]
mod direction_tests {
    use super::*;
    #[test]
    fn initial_direction_ignores_jitter_and_switches_only_after_tolerance() {
        let mut drag = AxisLock::new(Pos2::ZERO, true);
        assert_eq!(drag.update(Pos2::new(0.0, -10.0)), Pos2::new(0.0, -10.0));
        assert_eq!(drag.update(Pos2::new(10.0, -20.0)), Pos2::new(0.0, -20.0));
        assert_eq!(drag.update(Pos2::new(20.0, -21.0)), Pos2::new(8.0, -20.0));
        assert_eq!(drag.update(Pos2::new(25.0, -19.0)), Pos2::new(13.0, -20.0));
        assert_eq!(drag.update(Pos2::new(27.0, 0.0)), Pos2::new(13.0, -11.0));
        assert_eq!(drag.update(Pos2::new(28.0, 1.0)), Pos2::new(13.0, -10.0));
        let mut drag = AxisLock::new(Pos2::ZERO, true);
        assert_eq!(drag.update(Pos2::new(10.0, 0.0)), Pos2::new(10.0, 0.0));
        assert_eq!(drag.update(Pos2::new(20.0, 11.0)), Pos2::new(20.0, 0.0));
        assert_eq!(drag.update(Pos2::new(21.0, 12.0)), Pos2::new(20.0, 0.0));
        assert_eq!(drag.update(Pos2::new(22.0, 14.0)), Pos2::new(20.0, 2.0));
    }
    #[test]
    fn first_large_diagonal_move_chooses_the_dominant_axis_and_stationary_frames_do_not_switch_it()
    {
        let mut horizontal = AxisLock::new(Pos2::ZERO, true);
        assert_eq!(horizontal.update(Pos2::ZERO), Pos2::ZERO);
        assert_eq!(
            horizontal.update(Pos2::new(30.0, 20.0)),
            Pos2::new(30.0, 0.0)
        );
        assert_eq!(
            horizontal.update(Pos2::new(30.0, 20.0)),
            Pos2::new(30.0, 0.0)
        );
        assert_eq!(
            horizontal.update(Pos2::new(31.0, 25.0)),
            Pos2::new(31.0, 0.0)
        );
        let mut vertical = AxisLock::new(Pos2::ZERO, true);
        assert_eq!(vertical.update(Pos2::new(20.0, 30.0)), Pos2::new(0.0, 30.0));
        assert_eq!(vertical.update(Pos2::new(20.0, 30.0)), Pos2::new(0.0, 30.0));
        assert_eq!(vertical.update(Pos2::new(25.0, 31.0)), Pos2::new(0.0, 31.0));
    }
    #[test]
    fn origin_point_remains_vertical_even_with_large_horizontal_movement() {
        let mut drag = AxisLock::new(Pos2::new(2.0, 10.0), false);
        assert_eq!(drag.update(Pos2::new(40.0, 0.0)), Pos2::new(2.0, 0.0));
        assert_eq!(drag.update(Pos2::new(-40.0, 20.0)), Pos2::new(2.0, 20.0));
    }
}
