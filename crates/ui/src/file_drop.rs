use daw_media::AudioData;
use std::{path::PathBuf, sync::mpsc};

/// A drag owns its worker and prepared audio; hovering never changes the project.
pub struct FileHover {
    pub path: PathBuf,
    pub name: String,
    pub audio: Option<Result<AudioData, String>>,
    receiver: Option<mpsc::Receiver<Result<AudioData, String>>>,
}

impl FileHover {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (sender, receiver) = mpsc::channel();
        let source = path.clone();
        std::thread::spawn(move || {
            let _ = sender.send(daw_media::decode_wav(&source).map_err(|error| error.to_string()));
        });
        Self {
            path,
            name,
            audio: None,
            receiver: Some(receiver),
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
    pub fn into_audio(self) -> Result<AudioData, String> {
        if let Some(audio) = self.audio {
            audio
        } else {
            self.receiver
                .ok_or("File preview worker is unavailable")?
                .recv()
                .map_err(|_| "File preview worker stopped".to_owned())?
        }
    }
}
