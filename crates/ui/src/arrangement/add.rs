//! Header additions share the exact stopped playhead position.
use super::*;
impl DawUi {
    pub(super) fn add_arrangement_at_playhead(&mut self, kind: Kind) {
        let frame = self.session.project.transport.playhead_frame;
        let mut item = Item {
            kind,
            id: Id::new_v4(),
            frame,
            label: "New section".into(),
            length_frames: None,
        };
        match kind {
            Kind::Signature => {
                let mut events = self.session.project.signature_events();
                let event = if let Some(event) = events.iter().find(|e| e.start_frame == frame) {
                    *event
                } else {
                    let signature = self
                        .session
                        .project
                        .musical_clock()
                        .map_or(self.session.project.time_signature, |clock| {
                            clock.signature_at(frame)
                        });
                    let event = SignatureEvent {
                        id: item.id,
                        start_frame: frame,
                        signature,
                    };
                    events.push(event);
                    self.edit(Edit::SetSignatureEvents { events });
                    event
                };
                if !self
                    .session
                    .project
                    .signature_events()
                    .iter()
                    .any(|e| e.id == event.id)
                {
                    return;
                }
                item.id = event.id;
                item.label = format!(
                    "{}/{}",
                    event.signature.numerator, event.signature.denominator
                );
            }
            Kind::Marker => {
                self.arrangement_draft(item.clone(), true);
                if !self.session.project.markers.iter().any(|m| m.id == item.id) {
                    return;
                }
            }
            Kind::Tempo => {
                self.arrangement_draft(item, true);
                return;
            }
        }
        unit_selection::select(&mut self.arrangement.selection, (kind, item.id), false);
        self.arrangement.selected = Some(item);
    }
}
