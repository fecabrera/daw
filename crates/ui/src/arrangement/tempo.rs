//! Tempo point values and the shared transport-style vertical gesture.
use super::*;

// Logical points: exceed the point hit target before changing drag direction.
const AXIS_SWITCH_DISTANCE: f32 = 12.0;
#[derive(Clone, Copy)]
enum TempoAxis {
    Horizontal,
    Vertical,
}
pub(super) struct TempoDirection {
    axis: Option<TempoAxis>,
    anchor: Pos2,
    previous: Pos2,
    effective: Pos2,
    horizontal: bool,
}
impl TempoDirection {
    pub fn new(origin: Pos2, horizontal: bool) -> Self {
        Self {
            axis: None,
            anchor: origin,
            previous: origin,
            effective: origin,
            horizontal,
        }
    }
    pub fn update(&mut self, pointer: Pos2) -> Pos2 {
        let delta = pointer - self.previous;
        let Some(axis) = self.axis else {
            let axis = if self.horizontal && delta.x.abs() > delta.y.abs() {
                TempoAxis::Horizontal
            } else {
                TempoAxis::Vertical
            };
            self.axis = Some(axis);
            self.anchor = pointer;
            match axis {
                TempoAxis::Horizontal => self.effective.x += delta.x,
                TempoAxis::Vertical => self.effective.y += delta.y,
            }
            self.previous = pointer;
            return self.effective;
        };
        let cross = match axis {
            TempoAxis::Horizontal => pointer.y - self.anchor.y,
            TempoAxis::Vertical => pointer.x - self.anchor.x,
        };
        if self.horizontal && cross.abs() >= AXIS_SWITCH_DISTANCE {
            // Consume the tolerance before switching; never apply ignored motion
            // all at once to the other parameter.
            let excess = cross - cross.signum() * AXIS_SWITCH_DISTANCE;
            self.axis = Some(match axis {
                TempoAxis::Horizontal => {
                    self.effective.y += excess;
                    TempoAxis::Vertical
                }
                TempoAxis::Vertical => {
                    self.effective.x += excess;
                    TempoAxis::Horizontal
                }
            });
            self.anchor = pointer;
        } else {
            match axis {
                TempoAxis::Horizontal => self.effective.x += delta.x,
                TempoAxis::Vertical => self.effective.y += delta.y,
            }
        }
        self.previous = pointer;
        self.effective
    }
}

pub(super) struct TempoGesture {
    pub initial_bpm: f32,
    pub bpm: f32,
    pub previous_y: f32,
    pub range: (f32, f32),
    pub value: musical_time::TempoValueDrag,
    pub direction: TempoDirection,
}
pub(super) fn value_range(events: &[TempoEvent]) -> (f32, f32) {
    let low = events.iter().map(|e| e.bpm).fold(f32::MAX, f32::min);
    let high = events.iter().map(|e| e.bpm).fold(0.0, f32::max);
    let padding = ((high - low) * 0.1).max(10.0);
    ((low - padding).max(0.01), (high + padding).min(f32::MAX))
}
impl DawUi {
    pub(super) fn arrangement_tempos(&self) -> Vec<TempoEvent> {
        let mut events = self.session.project.tempo_events();
        if let Some(drag) = &self.arrangement.drag
            && let Some(tempo) = &drag.tempo
            && let Some(event) = events.iter_mut().find(|e| e.id == drag.item.id)
        {
            event.start_frame = drag.frame;
            event.bpm = tempo.bpm;
        }
        events.sort_by_key(|event| event.start_frame);
        events
    }
    pub(super) fn add_tempo_point(&mut self, item: Item) {
        let mut events = self.session.project.tempo_events();
        let event = if let Some(event) = events.iter().find(|e| e.start_frame == item.frame) {
            *event
        } else {
            let bpm = self
                .session
                .project
                .musical_clock()
                .map_or(self.session.project.tempo_bpm, |clock| {
                    clock.tempo_at(item.frame)
                });
            let event = TempoEvent {
                id: item.id,
                start_frame: item.frame,
                bpm,
            };
            events.push(event);
            self.edit(Edit::SetTempoEvents { events });
            event
        };
        unit_selection::select(
            &mut self.arrangement.selection,
            (Kind::Tempo, event.id),
            false,
        );
        self.arrangement.selected = Some(Item {
            id: event.id,
            frame: event.start_frame,
            label: format!("{} BPM", event.bpm),
            ..item
        });
    }
}

#[cfg(test)]
mod direction_tests {
    use super::*;
    #[test]
    fn initial_direction_ignores_jitter_and_switches_only_after_tolerance() {
        let mut drag = TempoDirection::new(Pos2::ZERO, true);
        assert_eq!(drag.update(Pos2::new(0.0, -10.0)), Pos2::new(0.0, -10.0));
        assert_eq!(drag.update(Pos2::new(10.0, -20.0)), Pos2::new(0.0, -20.0));
        assert_eq!(drag.update(Pos2::new(20.0, -21.0)), Pos2::new(8.0, -20.0));
        assert_eq!(drag.update(Pos2::new(25.0, -19.0)), Pos2::new(13.0, -20.0));
        assert_eq!(drag.update(Pos2::new(27.0, 0.0)), Pos2::new(13.0, -11.0));
        assert_eq!(drag.update(Pos2::new(28.0, 1.0)), Pos2::new(13.0, -10.0));
        let mut drag = TempoDirection::new(Pos2::ZERO, true);
        assert_eq!(drag.update(Pos2::new(10.0, 0.0)), Pos2::new(10.0, 0.0));
        assert_eq!(drag.update(Pos2::new(20.0, 11.0)), Pos2::new(20.0, 0.0));
        assert_eq!(drag.update(Pos2::new(21.0, 12.0)), Pos2::new(20.0, 0.0));
        assert_eq!(drag.update(Pos2::new(22.0, 14.0)), Pos2::new(20.0, 2.0));
    }
    #[test]
    fn first_large_diagonal_move_chooses_the_dominant_axis_and_stationary_frames_do_not_switch_it()
    {
        let mut horizontal = TempoDirection::new(Pos2::ZERO, true);
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
        let mut vertical = TempoDirection::new(Pos2::ZERO, true);
        assert_eq!(vertical.update(Pos2::new(20.0, 30.0)), Pos2::new(0.0, 30.0));
        assert_eq!(vertical.update(Pos2::new(20.0, 30.0)), Pos2::new(0.0, 30.0));
        assert_eq!(vertical.update(Pos2::new(25.0, 31.0)), Pos2::new(0.0, 31.0));
    }
    #[test]
    fn origin_point_remains_vertical_even_with_large_horizontal_movement() {
        let mut drag = TempoDirection::new(Pos2::new(2.0, 10.0), false);
        assert_eq!(drag.update(Pos2::new(40.0, 0.0)), Pos2::new(2.0, 0.0));
        assert_eq!(drag.update(Pos2::new(-40.0, 20.0)), Pos2::new(2.0, 20.0));
    }
}
