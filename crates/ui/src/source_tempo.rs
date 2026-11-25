//! Optional Clip Settings job. The worker owns source data only until completion.
use super::*;
use daw_media::{DiskAudio, Tempo};
use daw_project::{TempoCatalog, TempoSource};
use std::{
    collections::VecDeque,
    sync::{Arc, Weak, atomic::AtomicBool},
};

#[derive(Clone)]
enum InputIdentity {
    File {
        samples: Weak<Vec<[f32; 2]>>,
        disk: Option<Weak<DiskAudio>>,
        rate: u32,
        frames: u64,
        channels: u16,
    },
    Compound {
        catalog: Weak<TempoCatalog>,
        failure_revision: u64,
    },
}

impl PartialEq for InputIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::File {
                    samples,
                    disk,
                    rate,
                    frames,
                    channels,
                },
                Self::File {
                    samples: b,
                    disk: d,
                    rate: r,
                    frames: f,
                    channels: c,
                },
            ) => {
                samples.ptr_eq(b)
                    && *rate == *r
                    && *frames == *f
                    && *channels == *c
                    && match (disk, d) {
                        (Some(a), Some(b)) => a.ptr_eq(b),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Compound {
                    catalog,
                    failure_revision,
                },
                Self::Compound {
                    catalog: b,
                    failure_revision: r,
                },
            ) => catalog.ptr_eq(b) && failure_revision == r,
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Key {
    epoch: u64,
    asset: Id,
    source: InputIdentity,
}

struct Entry {
    key: Key,
    result: Result<Tempo, String>,
    retried: bool,
}

struct Job {
    key: Key,
    cancelled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<Tempo, String>>,
    retried: bool,
}

#[derive(Default)]
pub(super) struct State {
    epoch: u64,
    current: Option<Key>,
    visible: bool,
    job: Option<Job>,
    entries: VecDeque<Entry>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl State {
    pub(super) fn reset(&mut self) {
        self.cancel();
        self.epoch = self.epoch.wrapping_add(1);
        self.current = None;
        self.entries.clear();
    }

    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn begin_frame(&mut self) {
        self.visible = false;
    }

    pub(super) fn finish_frame(&mut self, ctx: &egui::Context) {
        if !self.visible {
            self.clear_demand(ctx);
        }
    }

    pub(super) fn clear_demand(&mut self, ctx: &egui::Context) {
        self.current = None;
        self.cancel();
        self.poll(ctx);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let result = self.job.as_ref().and_then(|job| {
            worker_result::poll(&job.receiver, || "Tempo analysis worker stopped".into())
        });
        if let Some(result) = result {
            let job = self.job.take().unwrap();
            if self.current.as_ref() == Some(&job.key) && !job.cancelled.load(Ordering::Relaxed) {
                self.insert(Entry {
                    key: job.key,
                    result,
                    retried: job.retried,
                });
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn insert(&mut self, entry: Entry) {
        self.entries.retain(|old| old.key != entry.key);
        if self.entries.len() == 32 {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub(super) fn readout(
        &mut self,
        asset: Id,
        input: TempoSource<'_>,
        ctx: &egui::Context,
    ) -> (String, String) {
        self.visible = true;
        let source = match input {
            TempoSource::File(audio) if audio.disk.as_ref().is_some_and(|disk| disk.failed()) => {
                self.clear_demand(ctx);
                return status("Analysis failed", "The prepared source could not be read.");
            }
            TempoSource::File(audio) => InputIdentity::File {
                samples: Arc::downgrade(&audio.samples),
                disk: audio.disk.as_ref().map(Arc::downgrade),
                rate: audio.sample_rate_hz,
                frames: audio.frame_count(),
                channels: audio.metadata.channels,
            },
            TempoSource::Compound(catalog) => InputIdentity::Compound {
                catalog: Arc::downgrade(catalog),
                failure_revision: daw_media::disk_failure_revision(),
            },
            TempoSource::Pending => {
                self.clear_demand(ctx);
                return status(
                    "Waiting for audio",
                    "Source analysis starts when project audio is ready.",
                );
            }
            TempoSource::Missing => {
                self.clear_demand(ctx);
                return status(
                    "Missing source",
                    "The original audio source is unavailable.",
                );
            }
            TempoSource::Unavailable => {
                self.clear_demand(ctx);
                return status(
                    "Tempo unavailable",
                    "This compound source is unavailable for bounded analysis.",
                );
            }
        };
        let key = Key {
            epoch: self.epoch,
            asset,
            source,
        };
        let revisited = self.current.as_ref() != Some(&key);
        self.current = Some(key.clone());
        if self.job.as_ref().is_some_and(|job| job.key != key) {
            self.cancel();
        }
        self.poll(ctx);

        let mut retried = false;
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            let entry = self.entries.remove(index).unwrap();
            if revisited && entry.result.is_err() && !entry.retried && self.job.is_none() {
                retried = true;
            } else {
                let text = result_text(&entry.result);
                self.entries.push_back(entry);
                return text;
            }
        }
        if self.job.is_none() {
            match input {
                TempoSource::File(audio) => {
                    let audio = audio.clone();
                    self.spawn(key, retried, ctx, move |cancel| {
                        daw_media::audio_tempo(&audio, &|| cancel.load(Ordering::Relaxed))
                            .map_err(|e| e.to_string())
                    });
                }
                TempoSource::Compound(catalog) => {
                    let catalog = catalog.clone();
                    self.spawn(key, retried, ctx, move |cancel| {
                        catalog.analyze(asset, cancel)
                    });
                }
                _ => unreachable!(),
            }
        }
        status(
            "Analyzing…",
            "Estimating the pulse of the source audio in the background.",
        )
    }

    fn spawn(
        &mut self,
        key: Key,
        retried: bool,
        ctx: &egui::Context,
        analyze: impl FnOnce(&Arc<AtomicBool>) -> Result<Tempo, String> + Send + 'static,
    ) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let repaint = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("source-tempo".into())
            .spawn(move || {
                let result = analyze(&worker_cancelled);
                let _ = sender.send(result);
                repaint.request_repaint();
            }) {
            Ok(_) => {
                self.job = Some(Job {
                    key,
                    cancelled,
                    receiver,
                    retried,
                })
            }
            Err(error) => self.insert(Entry {
                key,
                retried,
                result: Err(error.to_string()),
            }),
        }
        ctx.request_repaint();
    }
}

fn status(text: &str, details: &str) -> (String, String) {
    (text.into(), details.into())
}

fn result_text(result: &Result<Tempo, String>) -> (String, String) {
    match result {
        Ok(Tempo::Estimate { bpm, sampled }) => (
            format!("≈ {bpm:.0} BPM"),
            if *sampled {
                "Estimated from the start, middle and end of the source. Unsampled tempo changes may be missed."
            } else {
                "Estimated from the source audio before clip and track processing."
            }.into()
        ),
        Ok(Tempo::OctaveAmbiguous { bpm, sampled }) => {
            let audio = if *sampled { "sampled audio" } else { "source audio" };
            let mut details = format!(
                "Half and double rates fit the {audio}. The supported rate closest to {} BPM is shown.",
                Tempo::PREFERRED_OCTAVE_BPM
            );
            if *sampled {
                details.push_str(" Unsampled tempo changes may be missed.");
            }
            (format!("≈ {bpm:.0} BPM"), details)
        }
        Ok(Tempo::Weak) => status("Tempo unavailable", "The source does not have enough consistent rhythmic evidence."),
        Ok(Tempo::Ambiguous) => status("Ambiguous tempo", "The source supports competing pulse rates or the analyzed sections disagree."),
        Err(error) if error.starts_with("Missing") => status("Missing source", error),
        Err(error) if error.starts_with("Incomplete") || error.contains("limit exceeded") || error.contains("admission exceeded") => {
            status("Tempo unavailable", error)
        }
        Err(error) => status("Analysis failed", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octave_ambiguity_shows_the_preferred_bpm_and_explains_the_choice() {
        for sampled in [false, true] {
            let (label, tooltip) = result_text(&Ok(Tempo::OctaveAmbiguous { bpm: 70.0, sampled }));
            assert_eq!(label, "≈ 70 BPM");
            assert!(tooltip.contains("closest to 100 BPM"));
            assert_eq!(tooltip.contains("Unsampled tempo changes"), sampled);
        }
        assert_eq!(result_text(&Ok(Tempo::Ambiguous)).0, "Ambiguous tempo");
    }

    #[test]
    fn workspace_reset_retires_the_worker_and_discards_a_late_result() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        let key = Key {
            epoch: state.epoch,
            asset: Id::new_v4(),
            source: InputIdentity::Compound {
                catalog: Weak::new(),
                failure_revision: 0,
            },
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        state.current = Some(key.clone());
        state.job = Some(Job {
            key,
            cancelled: cancelled.clone(),
            receiver,
            retried: false,
        });
        state.reset();
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(state.job.is_some());
        sender
            .send(Ok(Tempo::Estimate {
                bpm: 120.0,
                sampled: false,
            }))
            .unwrap();
        state.finish_frame(&ctx);
        assert!(state.job.is_none());
        assert!(state.entries.is_empty());
    }
}
