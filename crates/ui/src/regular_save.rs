//! Ordinary Save writes a snapshot without disabling or replacing the live workspace.
use super::*;

pub(super) struct PendingSave {
    pub receiver: mpsc::Receiver<Result<Session, String>>,
    pub project: Id,
    pub revision: u64,
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
            navigation_revision: self.navigation_revision,
        });
        std::thread::spawn(move || {
            let result = snapshot
                .save(&folder)
                .map(|()| snapshot)
                .map_err(|e| e.to_string());
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
            match result {
                Ok(written) if saved.project == self.session.project.project_id => {
                    // Merge only file locations. The written snapshot can be
                    // older than the current edits, history gesture or transport.
                    if let Err(error) = self.session.retarget_generated_sources(&written) {
                        self.fail(error);
                        return;
                    }
                    self.saved_navigation_revision = saved.navigation_revision;
                    self.history_saved(&written);
                    if let Some(history) = &self.history {
                        self.dirty = !history.is_saved()
                            || self.navigation_revision != self.saved_navigation_revision;
                    }
                    if saved.revision == self.change_revision {
                        self.dirty = false;
                    }
                }
                Ok(_) => {}
                Err(error) => self.fail(error),
            }
        }
    }
}
