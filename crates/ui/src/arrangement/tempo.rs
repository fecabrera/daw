//! Tempo point values and the shared transport-style vertical gesture.
use super::*;

pub(super) struct TempoGesture {
    pub initial_bpm: f32,
    pub bpm: f32,
    pub previous_y: f32,
    pub range: (f32, f32),
    pub value: musical_time::TempoValueDrag,
    pub direction: daw_unit_ui::drag::AxisLock,
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
