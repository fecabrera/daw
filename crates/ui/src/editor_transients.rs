//! Optional Editor-only analysis. One cancellable worker, bounded disposable
//! cache, and weak source identities; no playback or project mutation.
use super::*;
use daw_media::AudioData;
use std::{
    collections::VecDeque,
    sync::{Arc, Weak, atomic::AtomicBool},
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    samples: usize,
    disk: usize,
    rate: u32,
    channels: u16,
}
impl Key {
    fn of(audio: &AudioData) -> Self {
        Self {
            samples: Arc::as_ptr(&audio.samples) as usize,
            disk: audio.disk.as_ref().map_or(0, |d| Arc::as_ptr(d) as usize),
            rate: audio.sample_rate_hz,
            channels: audio.metadata.channels,
        }
    }
}
struct Identity {
    key: Key,
    // Keep allocation identities from being reused without retaining source PCM.
    _samples: Weak<Vec<[f32; 2]>>,
    _disk: Option<Weak<daw_media::DiskAudio>>,
    _compound: Option<Weak<daw_core::CompiledAudioTrack>>,
}
impl Identity {
    fn compound(view: &Arc<daw_core::CompiledAudioTrack>, rate: u32) -> Self {
        Self {
            key: Key {
                samples: Arc::as_ptr(view) as usize,
                disk: usize::MAX,
                rate,
                channels: 2,
            },
            _samples: Weak::new(),
            _disk: None,
            _compound: Some(Arc::downgrade(view)),
        }
    }
    fn of(audio: &AudioData) -> Self {
        Self {
            key: Key::of(audio),
            _samples: Arc::downgrade(&audio.samples),
            _disk: audio.disk.as_ref().map(Arc::downgrade),
            _compound: None,
        }
    }
}
struct Entry {
    identity: Identity,
    result: Result<Arc<Vec<u64>>, String>,
}
struct Job {
    identity: Identity,
    generation: u64,
    cancelled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<Vec<u64>, String>>,
}
#[derive(Default)]
pub(super) struct State {
    pub enabled: bool,
    entries: VecDeque<Entry>,
    job: Option<Job>,
    project: Option<Id>,
    generation: u64,
    #[cfg(test)]
    pub toggle_bounds: Option<Rect>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl State {
    pub fn cancel(&mut self) {
        if let Some(job) = &self.job
            && !job.cancelled.swap(true, Ordering::Relaxed)
        {
            self.generation = self.generation.wrapping_add(1);
        }
    }
    pub fn poll(&mut self, project: Id, ctx: &egui::Context) {
        if self.project != Some(project) {
            self.cancel();
            self.entries.clear();
            self.project = Some(project);
        }
        if !self.enabled {
            self.cancel();
        }
        let result = self
            .job
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Transient analysis worker stopped".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            let job = self.job.take().unwrap();
            if job.generation == self.generation && !job.cancelled.load(Ordering::Relaxed) {
                self.insert(Entry {
                    identity: job.identity,
                    result: result.map(Arc::new),
                });
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    fn insert(&mut self, entry: Entry) {
        self.entries
            .retain(|old| old.identity.key != entry.identity.key);
        self.entries.push_back(entry);
        while self.entries.len() > 32 {
            self.entries.pop_front();
        }
    }
    fn cached_or_busy(&mut self, key: Key) -> bool {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.identity.key == key)
        {
            let entry = self.entries.remove(index).unwrap();
            self.entries.push_back(entry);
            return true;
        }
        self.job.is_some()
    }
    pub fn request(&mut self, audio: &AudioData, ctx: &egui::Context) {
        if self.cached_or_busy(Key::of(audio)) || audio.frame_count() == 0 {
            return;
        }
        let identity = Identity::of(audio);
        let audio = audio.clone();
        self.spawn(identity, ctx, move |cancel| {
            daw_media::audio_transients(&audio, &|| cancel.load(Ordering::Relaxed))
                .map_err(|e| e.to_string())
        });
    }
    pub fn request_compound(
        &mut self,
        session: &daw_project::Session,
        id: Id,
        ctx: &egui::Context,
    ) {
        let Some(view) = session.compound_views.get(&id) else {
            return;
        };
        let identity = Identity::compound(view, session.project.sample_rate_hz);
        if self.cached_or_busy(identity.key) {
            return;
        }
        let session = session.clone();
        self.spawn(identity, ctx, move |cancel| {
            let cancelled = || cancel.load(Ordering::Relaxed);
            let audio = session
                .render_compound_source(id, &cancelled)
                .map_err(|e| e.to_string())?;
            daw_media::audio_transients(&audio, &cancelled).map_err(|e| e.to_string())
        });
    }
    fn spawn(
        &mut self,
        identity: Identity,
        ctx: &egui::Context,
        action: impl FnOnce(&AtomicBool) -> Result<Vec<u64>, String> + Send + 'static,
    ) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancelled.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let repaint = ctx.clone();
        match std::thread::Builder::new()
            .name("editor-transients".into())
            .spawn(move || {
                let _ = sender.send(action(&worker_cancel));
                repaint.request_repaint();
            }) {
            Ok(_) => {
                self.job = Some(Job {
                    identity,
                    generation: self.generation,
                    cancelled,
                    receiver,
                })
            }
            Err(error) => self.insert(Entry {
                identity,
                result: Err(format!("Cannot start transient analysis: {error}")),
            }),
        }
        ctx.request_repaint();
    }
    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        let response = ui
            .checkbox(
                &mut self.enabled,
                egui::RichText::new("Show transients").small(),
            )
            .on_hover_text(
                "Show detected attacks in the audio waveform. Analysis runs in the background.",
            );
        #[cfg(test)]
        {
            self.toggle_bounds = Some(response.rect);
        }
        if response.changed() {
            if !self.enabled {
                self.cancel();
            } else {
                self.entries.retain(|entry| entry.result.is_ok());
            }
        }
        if self.enabled {
            if self.job.is_some() {
                ui.spinner().on_hover_text("Analyzing transients");
            }
            if let Some(error) = self
                .entries
                .iter()
                .find_map(|entry| entry.result.as_ref().err())
            {
                ui.small("Some sources unavailable").on_hover_text(error);
            }
        }
    }
    #[cfg(test)]
    pub(super) fn seed(&mut self, project: Id, audio: &AudioData, markers: Vec<u64>) {
        self.enabled = true;
        self.project = Some(project);
        self.insert(Entry {
            identity: Identity::of(audio),
            result: Ok(Arc::new(markers)),
        });
    }
    pub fn compound_markers(&self, session: &daw_project::Session, id: Id) -> Option<&[u64]> {
        self.markers_for_key(
            Identity::compound(
                session.compound_views.get(&id)?,
                session.project.sample_rate_hz,
            )
            .key,
        )
    }
    pub fn markers(&self, audio: &AudioData) -> Option<&[u64]> {
        self.markers_for_key(Key::of(audio))
    }
    fn markers_for_key(&self, key: Key) -> Option<&[u64]> {
        if !self.enabled {
            return None;
        }
        self.entries
            .iter()
            .find(|entry| entry.identity.key == key)?
            .result
            .as_ref()
            .ok()
            .map(|points| points.as_slice())
    }
}

/// Test a visible frame interval using exactly the waveform's trim/loop mapping.
fn marker_in_window(
    markers: &[u64],
    clip: &Clip,
    local: u64,
    count: u64,
    stretch: Option<daw_core::ClipStretch>,
) -> bool {
    let mapped = |frame| stretch.map_or(frame, |ratio| ratio.scale(frame).unwrap_or(u64::MAX));
    waveforms::source_windows(clip, local, count)
        .into_iter()
        .any(|(start, count)| {
            if count == 0 {
                return false;
            }
            let index = markers.partition_point(|&frame| mapped(frame) < start);
            markers
                .get(index)
                .is_some_and(|&frame| mapped(frame) < start.saturating_add(count))
        })
}
impl DawUi {
    pub(super) fn paint_editor_transients(
        &self,
        painter: &egui::Painter,
        body: Rect,
        clip: &Clip,
        audio: &AudioData,
        stretch: Option<daw_core::ClipStretch>,
    ) {
        let markers = self.editor_transients.markers(audio);
        self.paint_transient_markers(painter, body, clip, markers, stretch);
    }
    pub(super) fn paint_compound_transients(
        &self,
        painter: &egui::Painter,
        body: Rect,
        clip: &Clip,
    ) {
        let markers = self
            .editor_transients
            .compound_markers(&self.session, clip.asset_id);
        self.paint_transient_markers(painter, body, clip, markers, None);
    }
    fn paint_transient_markers(
        &self,
        painter: &egui::Painter,
        body: Rect,
        clip: &Clip,
        markers: Option<&[u64]>,
        stretch: Option<daw_core::ClipStretch>,
    ) {
        if !self.midi_editor.content_active || self.view_mode != mixer::ViewMode::Edit {
            return;
        }
        let Some(markers) = markers else {
            return;
        };
        let visible = body.intersect(painter.clip_rect());
        if !visible.is_positive() {
            return;
        }
        let scale = f64::from(self.session.project.sample_rate_hz) / f64::from(self.view_zoom());
        let painter = painter.with_clip_rect(visible);
        let color = theme::DARK.grid.gamma_multiply(0.45);
        // At most one line per visible logical pixel, even for very short loops.
        for pixel in visible.left().floor() as i32..visible.right().ceil() as i32 {
            let start = ((f64::from(pixel) - f64::from(body.left())) * scale)
                .max(0.0)
                .ceil() as u64;
            let end = ((f64::from(pixel + 1) - f64::from(body.left())) * scale)
                .max(0.0)
                .ceil() as u64;
            if marker_in_window(markers, clip, start, end.saturating_sub(start), stretch) {
                painter.vline(
                    pixel as f32 + 0.5,
                    visible.y_range(),
                    Stroke::new(1.0, color),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_windows_follow_trims_loop_phase_and_preview_stretch() {
        let mut clip = Clip {
            id: Id::new_v4(),
            asset_id: Id::new_v4(),
            name: "Test".into(),
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 100,
            channels: Default::default(),
            fades: Default::default(),
            muted: false,
            color: None,
            repeat: None,
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
        };
        clip.source_offset_frame = 100;
        clip.length_frames = 50;
        assert!(marker_in_window(&[110], &clip, 10, 1, None));
        assert!(!marker_in_window(&[150], &clip, 49, 2, None));
        clip.repeat = Some(daw_core::ClipLoop {
            length_frames: 40,
            phase_frame: 30,
        });
        clip.length_frames = 105;
        for local in 0..110 {
            assert_eq!(
                marker_in_window(&[110], &clip, local, 1, None),
                [20, 60, 100].contains(&local)
            );
            assert_eq!(
                marker_in_window(&[55], &clip, local, 1, daw_core::ClipStretch::new(1, 2)),
                [20, 60, 100].contains(&local)
            );
        }
        assert!(marker_in_window(&[110], &clip, 0, 105, None));
        assert!(!marker_in_window(&[140], &clip, 0, 105, None));
    }
    fn audio() -> AudioData {
        AudioData {
            samples: Arc::new(vec![[0.0; 2]; 4800]),
            disk: None,
            peaks: Default::default(),
            sample_rate_hz: 48000,
            metadata: daw_core::SourceMetadata {
                sample_rate_hz: 48000,
                channels: 2,
                sample_format: "float".into(),
                bits_per_sample: 32,
            },
        }
    }
    #[test]
    fn optional_worker_publishes_once_and_new_audio_invalidates_cache() {
        let source = audio();
        let ctx = egui::Context::default();
        let project = Id::new_v4();
        let mut state = State::default();
        state.enabled = true;
        state.poll(project, &ctx);
        state.request(&source, &ctx);
        assert!(state.job.is_some());
        state.request(&source, &ctx);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while state.job.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
            state.poll(project, &ctx);
        }
        assert!(state.job.is_none());
        assert_eq!(state.markers(&source), Some([].as_slice()));
        state.request(&source, &ctx);
        assert!(
            state.job.is_none(),
            "cached result must not start another worker"
        );
        let mut replaced = source.clone();
        replaced.samples = Arc::new((*source.samples).clone());
        assert!(state.markers(&replaced).is_none());
    }
    #[test]
    fn cache_does_not_retain_pcm_and_stale_results_are_discarded() {
        let audio = audio();
        let project = Id::new_v4();
        let ctx = egui::Context::default();
        let mut state = State::default();
        state.enabled = true;
        state.project = Some(project);
        state.insert(Entry {
            identity: Identity::of(&audio),
            result: Ok(Arc::new(vec![100])),
        });
        assert_eq!(Arc::strong_count(&audio.samples), 1);
        assert_eq!(state.markers(&audio), Some([100].as_slice()));
        let (sender, receiver) = mpsc::channel();
        state.job = Some(Job {
            identity: Identity::of(&audio),
            generation: state.generation,
            cancelled: Arc::new(AtomicBool::new(false)),
            receiver,
        });
        state.cancel();
        sender.send(Ok(vec![200])).unwrap();
        state.poll(project, &ctx);
        assert_eq!(state.markers(&audio), Some([100].as_slice()));
        state.poll(Id::new_v4(), &ctx);
        assert!(state.markers(&audio).is_none());
        state.insert(Entry {
            identity: Identity::of(&audio),
            result: Ok(Arc::new(vec![300])),
        });
        state.enabled = false;
        assert!(state.markers(&audio).is_none());
    }
}
