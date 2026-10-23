use daw_media::AudioData;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

/// A drag owns its cancellation token and prepared audio; hovering never changes the project.
pub struct FileHover {
    pub path: PathBuf,
    pub name: String,
    pub audio: Option<Result<AudioData, String>>,
    receiver: Option<mpsc::Receiver<Result<AudioData, String>>>,
    cancelled: Arc<AtomicBool>,
}

impl FileHover {
    pub fn at_sample_rate(path: PathBuf, sample_rate_hz: u32) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (sender, receiver) = mpsc::channel();
        let source = path.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancelled.clone();
        let request = PreviewRequest {
            source,
            sample_rate_hz,
            sender,
            cancelled: worker_cancel,
        };
        if let Err(error) = preview_queue().try_send(request) {
            let (request, message) = match error {
                mpsc::TrySendError::Full(r) => {
                    (r, "File preview queue is full; drop the file again")
                }
                mpsc::TrySendError::Disconnected(r) => (r, "File preview worker is unavailable"),
            };
            let _ = request.sender.send(Err(message.into()));
        }
        Self {
            path,
            name,
            audio: None,
            receiver: Some(receiver),
            cancelled,
        }
    }

    pub fn poll(&mut self) {
        let result = self
            .receiver
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("File preview worker stopped".into()))
                }
            });
        if let Some(result) = result {
            self.audio = Some(result);
            self.receiver = None;
        }
    }

    /// Finish preparation on the import worker if the file was dropped before it was ready.
    pub fn into_audio(mut self) -> Result<AudioData, String> {
        if let Some(audio) = self.audio.take() {
            audio
        } else {
            self.receiver
                .take()
                .ok_or("File preview worker is unavailable")?
                .recv()
                .map_err(|_| "File preview worker stopped".to_owned())?
        }
    }
}

impl Drop for FileHover {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct PreviewRequest {
    source: PathBuf,
    sample_rate_hz: u32,
    sender: mpsc::Sender<Result<AudioData, String>>,
    cancelled: Arc<AtomicBool>,
}
fn preview_queue() -> &'static mpsc::SyncSender<PreviewRequest> {
    static QUEUE: OnceLock<mpsc::SyncSender<PreviewRequest>> = OnceLock::new();
    QUEUE.get_or_init(|| {
        // Bound both concurrent PCM preparation and waiting thread/request count.
        let (sender, receiver) = mpsc::sync_channel::<PreviewRequest>(64);
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..2 {
            let receiver = receiver.clone();
            let _ = std::thread::Builder::new()
                .name("file-preview".into())
                .spawn(move || {
                    loop {
                        let Ok(request) = receiver.lock().unwrap().recv() else {
                            return;
                        };
                        let audio = daw_media::decode_audio_cached(
                            &request.source,
                            request.sample_rate_hz,
                            &|| request.cancelled.load(Ordering::Relaxed),
                        )
                        .map_err(|e| e.to_string());
                        let _ = request.sender.send(audio);
                    }
                });
        }
        sender
    })
}
