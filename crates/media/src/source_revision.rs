//! Worker-only content receipts. Paths and modification times are not content identities.
use crate::{AudioData, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Original encoded bytes, independent of source path and project processing rate.
/// Runtime-only: manifests do not assert that cached PCM still matches disk contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceRevision {
    pub byte_length: u64,
    pub sha256: [u8; 32],
}

/// Private worker-owned encoded input. Streaming to disk bounds scratch memory and
/// prevents pathname mutations from changing the bytes passed to either decoder.
pub(crate) struct SourceSnapshot {
    directory: PathBuf,
    pub path: PathBuf,
    pub revision: SourceRevision,
}
impl Drop for SourceSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
impl SourceSnapshot {
    pub fn read(path: &Path, cancelled: &impl Fn() -> bool) -> Result<Self> {
        check_cancelled(cancelled)?;
        let expected = stamp(fs::metadata(path).map_err(|e| Error(e.to_string()))?)?;
        let mut input = File::open(path).map_err(|e| Error(e.to_string()))?;
        if stamp(input.metadata().map_err(|e| Error(e.to_string()))?)? != expected {
            return Err(Error("Source changed before preparation".into()));
        }
        let directory =
            std::env::temp_dir().join(format!("daw-encoded-{}", daw_core::Id::new_v4()));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder
            .create(&directory)
            .map_err(|e| Error(format!("Cannot stage source: {e}")))?;
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|value| {
                crate::IMPORT_EXTENSIONS
                    .iter()
                    .copied()
                    .find(|known| value.eq_ignore_ascii_case(known))
            })
            .unwrap_or("audio");
        let mut snapshot = Self {
            path: directory.join(format!("source.{extension}")),
            directory,
            revision: SourceRevision {
                byte_length: 0,
                sha256: [0; 32],
            },
        };
        let mut output = File::create_new(&snapshot.path).map_err(|e| Error(e.to_string()))?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            check_cancelled(cancelled)?;
            let count = input.read(&mut buffer).map_err(|e| Error(e.to_string()))?;
            if count == 0 {
                break;
            }
            snapshot.revision.byte_length = snapshot
                .revision
                .byte_length
                .checked_add(count as u64)
                .ok_or_else(|| Error("Source byte length overflow".into()))?;
            if snapshot.revision.byte_length > expected.0 {
                return Err(Error("Source changed during preparation".into()));
            }
            hash.update(&buffer[..count]);
            output
                .write_all(&buffer[..count])
                .map_err(|e| Error(format!("Cannot stage source: {e}")))?;
        }
        snapshot.revision.sha256 = hash.finalize().into();
        if snapshot.revision.byte_length != expected.0
            || stamp(input.metadata().map_err(|e| Error(e.to_string()))?)? != expected
            || stamp(fs::metadata(path).map_err(|e| Error(e.to_string()))?)? != expected
        {
            return Err(Error("Source changed during preparation".into()));
        }
        check_cancelled(cancelled)?;
        Ok(snapshot)
    }
}

fn stamp(metadata: Metadata) -> Result<(u64, std::time::SystemTime)> {
    if !metadata.is_file() {
        return Err(Error("Source must be a regular file".into()));
    }
    Ok((
        metadata.len(),
        metadata.modified().map_err(|e| Error(e.to_string()))?,
    ))
}

fn check_cancelled(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err(Error("Source verification cancelled".into()))
    } else {
        Ok(())
    }
}

impl SourceRevision {
    /// Bounded streaming read. Never call from painting or the audio callback.
    /// A receipt identifies the bytes read; final collection must verify its staged file too.
    pub fn read(path: &Path, cancelled: &impl Fn() -> bool) -> Result<Self> {
        check_cancelled(cancelled)?;
        let io_error = |e| Error(format!("{}: {e}", path.display()));
        // Reject devices/FIFOs before opening them: opening a FIFO can block.
        let expected = stamp(fs::metadata(path).map_err(io_error)?)?;
        let mut file = File::open(path).map_err(io_error)?;
        let before = stamp(file.metadata().map_err(io_error)?)?;
        if before != expected {
            return Err(Error(format!(
                "{}: source changed before verification",
                path.display()
            )));
        }
        let mut digest = Sha256::new();
        let mut byte_length = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            check_cancelled(cancelled)?;
            let count = file.read(&mut buffer).map_err(io_error)?;
            if count == 0 {
                break;
            }
            byte_length = byte_length
                .checked_add(count as u64)
                .ok_or_else(|| Error("Source byte length overflow".into()))?;
            digest.update(&buffer[..count]);
        }
        if before.0 != byte_length
            || before != stamp(file.metadata().map_err(io_error)?)?
            || before != stamp(fs::metadata(path).map_err(io_error)?)?
        {
            return Err(Error(format!(
                "{}: source changed during verification",
                path.display()
            )));
        }
        check_cancelled(cancelled)?;
        Ok(Self {
            byte_length,
            sha256: digest.finalize().into(),
        })
    }
}

/// Verify a source against the original project-rate PCM, never a stretched/pitched buffer.
/// Bypass the weak decode cache: its length/mtime key is not proof of content identity.
/// This also handles sessions opened before content receipts were available. The collection
/// backend must run this check on private staged bytes before publishing them (Stage 2).
pub fn verify_prepared_source(
    path: &Path,
    prepared: &AudioData,
    cancelled: &impl Fn() -> bool,
) -> Result<SourceRevision> {
    let check = check_prepared_source(path, prepared, cancelled)?;
    if !check.matches_audio {
        return Err(Error(format!(
            "{}: source differs from prepared audio; collection requires the original recording",
            path.display()
        )));
    }
    Ok(check.revision)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedSourceCheck {
    pub revision: SourceRevision,
    pub matches_audio: bool,
}

/// Like verification, but exposes a mismatch without parsing diagnostic text.
pub fn check_prepared_source(
    path: &Path,
    prepared: &AudioData,
    cancelled: &impl Fn() -> bool,
) -> Result<PreparedSourceCheck> {
    let snapshot = SourceSnapshot::read(path, cancelled)?;
    let before = snapshot.revision;
    let current =
        crate::decode_audio_cancellable(&snapshot.path, prepared.sample_rate_hz, cancelled)?;
    let mut matches =
        prepared.metadata == current.metadata && prepared.samples.len() == current.samples.len();
    if matches {
        for (old, new) in prepared
            .samples
            .chunks(4096)
            .zip(current.samples.chunks(4096))
        {
            check_cancelled(cancelled)?;
            if !old
                .iter()
                .zip(new)
                .all(|(a, b)| a[0].to_bits() == b[0].to_bits() && a[1].to_bits() == b[1].to_bits())
            {
                matches = false;
                break;
            }
        }
    }
    let after = SourceRevision::read(path, cancelled)?;
    if before != after {
        return Err(Error(format!(
            "{}: source changed during verification",
            path.display()
        )));
    }
    Ok(PreparedSourceCheck {
        revision: after,
        matches_audio: matches,
    })
}

/// Worker-side naming hint for an already verified source. Preserve MP4 containers
/// even when they contain AAC/MP3 or the original filename has a misleading extension.
pub fn source_file_extension(
    path: &Path,
    metadata: &daw_core::SourceMetadata,
) -> Result<&'static str> {
    stamp(fs::metadata(path).map_err(|e| Error(e.to_string()))?)?;
    let mut header = [0_u8; 12];
    let mut file = File::open(path).map_err(|e| Error(e.to_string()))?;
    file.read_exact(&mut header)
        .map_err(|e| Error(e.to_string()))?;
    match metadata.sample_format.as_str() {
        "pcm_int" | "ieee_float" if &header[..4] == b"RIFF" && &header[8..] == b"WAVE" => Ok("wav"),
        "mp3" | "aac" if &header[4..8] == b"ftyp" => Ok("mp4"),
        "mp3" => Ok("mp3"),
        "aac" => Ok("aac"),
        _ => Err(Error(
            "Cannot determine a supported source container".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, path::PathBuf};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("daw-source-revision-{}", daw_core::Id::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn wav(&self, value: i16) -> PathBuf {
            let path = self.0.join("source.wav");
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 48000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..480 {
                writer.write_sample(value).unwrap();
            }
            writer.finalize().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn encoded_snapshot_binds_decoder_input_and_removes_private_scratch() {
        let fixture = Fixture::new();
        let path = fixture.wav(1000);
        let snapshot = SourceSnapshot::read(&path, &|| false).unwrap();
        let directory = snapshot.directory.clone();
        assert_eq!(
            snapshot.revision,
            SourceRevision::read(&path, &|| false).unwrap()
        );
        fixture.wav(2000);
        let decoded = crate::decode_audio_cancellable(&snapshot.path, 48000, &|| false).unwrap();
        assert_eq!(decoded.samples[0], [1000.0 / 32768.0; 2]);
        assert_ne!(
            snapshot.revision,
            SourceRevision::read(&path, &|| false).unwrap()
        );
        drop(snapshot);
        assert!(!directory.exists());
    }

    #[test]
    fn receipt_matches_sha256_vector_and_ignores_path() {
        let fixture = Fixture::new();
        let path = fixture.0.join("bytes");
        fs::write(&path, b"abc").unwrap();
        let revision = SourceRevision::read(&path, &|| false).unwrap();
        assert_eq!(revision.byte_length, 3);
        assert_eq!(
            revision.sha256,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
        let copy = fixture.0.join("copy");
        fs::copy(&path, &copy).unwrap();
        assert_eq!(revision, SourceRevision::read(&copy, &|| false).unwrap());
    }

    #[test]
    fn same_length_and_mtime_replacement_cannot_match_prepared_audio() {
        let fixture = Fixture::new();
        let path = fixture.wav(1000);
        let original = fs::metadata(&path).unwrap();
        let revision = SourceRevision::read(&path, &|| false).unwrap();
        let prepared = crate::decode_audio_cached(&path, 48000, &|| false).unwrap();
        assert_eq!(
            verify_prepared_source(&path, &prepared, &|| false).unwrap(),
            revision
        );
        fixture.wav(2000);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(original.modified().unwrap()))
            .unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), original.len());
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            original.modified().unwrap()
        );
        assert_ne!(SourceRevision::read(&path, &|| false).unwrap(), revision);
        // Content verification also invalidates the ordinary live cache.
        let cached = crate::decode_audio_cached(&path, 48000, &|| false).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&prepared.samples, &cached.samples));
        assert_ne!(prepared.samples, cached.samples);
        assert!(
            verify_prepared_source(&path, &prepared, &|| false)
                .unwrap_err()
                .0
                .contains("differs from prepared")
        );
    }

    #[test]
    fn verification_accepts_project_rate_audio_and_checks_metadata() {
        let fixture = Fixture::new();
        let path = fixture.wav(500);
        let mut prepared = crate::decode_audio_cancellable(&path, 96000, &|| false).unwrap();
        verify_prepared_source(&path, &prepared, &|| false).unwrap();
        prepared.metadata.channels = 2;
        assert!(verify_prepared_source(&path, &prepared, &|| false).is_err());
    }

    #[test]
    fn streaming_cancellation_mutation_and_missing_sources_fail() {
        let fixture = Fixture::new();
        let path = fixture.0.join("bytes");
        fs::write(&path, vec![0; 150_000]).unwrap();
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == 4
        };
        assert!(
            SourceRevision::read(&path, &cancelled)
                .unwrap_err()
                .0
                .contains("cancelled")
        );
        let calls = Cell::new(0);
        let mutate = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                fs::write(&path, b"replaced").unwrap();
            }
            false
        };
        assert!(SourceRevision::read(&path, &mutate).is_err());
        assert!(SourceRevision::read(&fixture.0, &|| false).is_err());
        fs::remove_file(&path).unwrap();
        assert!(SourceRevision::read(&path, &|| false).is_err());
    }
}
