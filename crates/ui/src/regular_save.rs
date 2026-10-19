//! Ordinary Save writes a snapshot without disabling or replacing the live workspace.
use super::*;

pub(super) struct PendingSave {
    pub receiver: mpsc::Receiver<Result<(), String>>,
    pub project: Id,
    pub revision: u64,
    pub snapshot: Session,
    pub navigation_revision: u64,
}

impl DawUi {
    pub(super) fn mark_dirty(&mut self) {
        self.change_revision = self.change_revision.wrapping_add(1);
        self.dirty = true;
        self.record_history();
    }

    pub(super) fn save_snapshot(&mut self, folder: PathBuf) {
        let mut snapshot = self.session.clone();
        let (sender, receiver) = mpsc::channel();
        self.regular_save = Some(PendingSave {
            receiver,
            project: snapshot.project.project_id,
            revision: self.change_revision,
            snapshot: snapshot.clone(),
            navigation_revision: self.navigation_revision,
        });
        std::thread::spawn(move || {
            let result = snapshot.save(&folder).map_err(|e| e.to_string());
            let _ = sender.send(result);
        });
    }

    pub(super) fn poll_regular_save(&mut self) {
        let result = self
            .regular_save
            .as_ref()
            .and_then(|save| match save.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Project save stopped unexpectedly".into()))
                }
            });
        if let Some(result) = result {
            let saved = self.regular_save.take().unwrap();
            if result.is_ok() && saved.project == self.session.project.project_id {
                self.saved_navigation_revision = saved.navigation_revision;
                self.history_saved(&saved.snapshot);
                if let Some(history) = &self.history {
                    self.dirty = !history.is_saved()
                        || self.navigation_revision != self.saved_navigation_revision;
                }
            }
            // A regular save never publishes its old snapshot over current edits or transport.
            match result {
                Ok(())
                    if saved.project == self.session.project.project_id
                        && saved.revision == self.change_revision =>
                {
                    self.dirty = false
                }
                Ok(()) => {}
                Err(error) => self.fail(error),
            }
        }
    }
}
