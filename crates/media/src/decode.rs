//! Worker-side source decoding. Compressed stream lengths are never allocation hints.
use crate::*;
use std::io::{Read, Seek, SeekFrom};
use symphonia::core::codecs::{CODEC_TYPE_AAC, CODEC_TYPE_MP3, CODEC_TYPE_OPUS};

pub const IMPORT_EXTENSIONS: &[&str] = &["wav", "mp3", "aac", "m4a", "mp4", "opus", "ogg"];

pub fn decode_audio(path: &Path) -> Result<AudioData> {
    decode_audio_at_rate(path, SAMPLE_RATE)
}

pub fn decode_audio_at_rate(path: &Path, rate: u32) -> Result<AudioData> {
    decode_audio_cancellable(path, rate, &|| false)
}

/// Preserve strict WAV validation, including declared sample lengths and PCM formats.
/// Probe compressed content instead of trusting a filename's extension.
pub fn decode_audio_cancellable(
    path: &Path,
    rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    decode_with_streaming(path, rate, cancelled, crate::stream_decode::should_stream)
}

fn decode_with_streaming(
    path: &Path,
    rate: u32,
    cancelled: &impl Fn() -> bool,
    should_stream: impl Fn(u64) -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Audio preparation cancelled".into()));
    }
    if !daw_core::SAMPLE_RATES.contains(&rate) {
        return Err(Error("Unsupported processing sample rate".into()));
    }
    let err = |e: &dyn std::fmt::Display| Error(format!("{}: {e}", path.display()));
    let mut file = File::open(path).map_err(|e| err(&e))?;
    let mut header = [0; 12];
    let count = file.read(&mut header).map_err(|e| err(&e))?;
    if count == header.len() && &header[..4] == b"RIFF" && &header[8..] == b"WAVE" {
        return decode_wav_cancellable(path, rate, cancelled);
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| err(&e))?;
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(extension);
    }
    // The patched MP4 demuxer enforces this bounded metadata/packet policy before
    // allocation. Reserve it before probing, including for content with misleading
    // extensions; formats are selected from bytes, not the path suffix.
    let _container_admission = AudioReservation::new(
        0,
        symphonia::default::formats::IsoMp4Reader::MAX_WORKING_BYTES,
    )?;
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(Box::new(file), Default::default()),
            &FormatOptions {
                // Opus uses explicit RFC 7845 bounds below.
                enable_gapless: &header[..4] != b"OggS",
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| err(&e))?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            matches!(
                t.codec_params.codec,
                CODEC_TYPE_MP3 | CODEC_TYPE_AAC | CODEC_TYPE_OPUS
            )
        })
        .ok_or_else(|| err(&"Supported audio formats are WAV, MP3, AAC-LC and Ogg Opus"))?;
    let params = &track.codec_params;
    let sample_format = match params.codec {
        CODEC_TYPE_MP3 => "mp3",
        CODEC_TYPE_OPUS => "opus",
        _ => "aac",
    };
    let mut opus = if params.codec == CODEC_TYPE_OPUS {
        let head = params
            .extra_data
            .as_deref()
            .ok_or_else(|| err(&"Missing Opus header"))?;
        Some(
            crate::opus_import::OpusImport::read(path, head, params.start_ts, cancelled)
                .map_err(|e| err(&e))?,
        )
    } else {
        None
    };
    let gain = opus.as_ref().map_or(1.0, |opus| opus.gain);
    // ADTS duration is a bitrate estimate. MP3 gapless and MP4 sample tables
    // supply exact packet bounds, so reject premature EOF for those sources.
    let expected_end =
        if (params.codec == CODEC_TYPE_MP3 && params.delay.is_some()) || &header[4..8] == b"ftyp" {
            params.n_frames.and_then(|n| params.start_ts.checked_add(n))
        } else {
            None
        };
    let mut packet_end = 0;
    let track_id = track.id;
    let mut codecs = symphonia::core::codecs::CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut codecs);
    codecs.register_all::<symphonia_adapter_libopus::OpusDecoder>();
    let mut decoder_params = params.clone();
    if opus.is_some() {
        // The worker owns pre-skip across packets and final-granule trimming.
        // Disable the adapter's first-packet-only pre-skip to apply it once.
        decoder_params.extra_data.as_mut().unwrap()[10..12].fill(0);
    }
    let mut decoder = codecs
        .make(&decoder_params, &DecoderOptions::default())
        .map_err(|e| err(&e))?;
    let mut reservations = Vec::new();
    let mut source_spec = None;
    let mut samples = Vec::new();
    let mut admitted = 0;
    let mut disk: Option<DiskAudioWriter> = None;
    loop {
        if cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(err(&e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        packet_end = packet
            .ts
            .checked_add(packet.dur)
            .ok_or_else(|| err(&"Packet timestamp overflow"))?;
        let audio = decoder.decode(&packet).map_err(|e| err(&e))?;
        let source_rate = audio.spec().rate;
        let channels = audio.spec().channels.count();
        if !(1..=2).contains(&channels) || source_rate == 0 {
            return Err(err(
                &"Only mono/stereo audio with a valid sample rate is supported",
            ));
        }
        if let Some(spec) = source_spec {
            if spec != (source_rate, channels) {
                return Err(err(
                    &"Source sample rate or channels change within the file",
                ));
            }
        } else {
            reservations.push(AudioReservation::new(
                0,
                resample_scratch(source_rate, rate)?,
            )?);
            source_spec = Some((source_rate, channels));
        }
        let mut buffer = SampleBuffer::<f32>::new(audio.frames() as u64, *audio.spec());
        let frame_count = audio.frames();
        buffer.copy_interleaved_ref(audio);
        // MP3 trims inside its decoder; AAC and Opus use the worker bounds.
        let (trim_start, trim_end) = if sample_format == "aac" {
            (packet.trim_start() as usize, packet.trim_end() as usize)
        } else if let Some(opus) = &mut opus {
            opus.trim(frame_count)
        } else {
            (0, 0)
        };
        let frame_count = buffer.samples().len() / channels;
        let keep = frame_count
            .checked_sub(trim_start)
            .and_then(|n| n.checked_sub(trim_end))
            .ok_or_else(|| err(&"Invalid encoder delay or padding"))?;
        let required = samples
            .len()
            .checked_add(keep)
            .ok_or_else(|| err(&"Audio length overflow"))?;
        if disk.is_none() && should_stream(required as u64) {
            let mut writer = DiskAudioWriter::incremental()?;
            for block in samples.chunks(8192) {
                if cancelled() {
                    return Err(Error("Audio preparation cancelled".into()));
                }
                writer.write(block)?;
            }
            samples = Vec::new();
            reservations.clear();
            reservations.push(AudioReservation::new(0, 1024 * 1024)?);
            disk = Some(writer);
        }
        if disk.is_none() && required > admitted {
            let additional = (required - admitted).max(admitted).max(16_384);
            reservations.push(AudioReservation::new(
                resampled_frames(additional as u64, source_rate, rate)?,
                if source_rate == rate {
                    0
                } else {
                    additional as u64 * 8
                },
            )?);
            admitted += additional;
            // Reallocation can hold the previous and new PCM blocks together.
            let _growth = AudioReservation::new(0, samples.capacity() as u64 * 8)?;
            samples
                .try_reserve_exact(admitted - samples.len())
                .map_err(|e| err(&e))?;
        }
        let mut packet_frames = Vec::with_capacity(keep);
        for frame in buffer
            .samples()
            .chunks_exact(channels)
            .skip(trim_start)
            .take(keep)
        {
            let l = frame[0] * gain;
            let r = if channels == 1 { l } else { frame[1] * gain };
            if !l.is_finite() || !r.is_finite() {
                return Err(err(&"Source contains non-finite samples"));
            }
            if disk.is_some() {
                packet_frames.push([l, r]);
            } else {
                samples.push([l, r]);
            }
        }
        if let Some(writer) = &mut disk {
            writer.write(&packet_frames)?;
        }
    }
    if opus.as_ref().is_some_and(|opus| !opus.complete())
        || expected_end.is_some_and(|end| packet_end < end)
    {
        return Err(err(&"Incomplete compressed audio data"));
    }
    if disk.is_none() && samples.is_empty() {
        return Err(err(&"Source contains no audio samples"));
    }
    let (source_rate, channels) =
        source_spec.ok_or_else(|| err(&"Source contains no audio samples"))?;
    let metadata = SourceMetadata {
        sample_rate_hz: source_rate,
        channels: channels as u16,
        sample_format: sample_format.into(),
        // Lossy bitstreams do not have a PCM bit depth.
        bits_per_sample: 0,
    };
    if let Some(writer) = disk {
        let audio = writer.finish(source_rate, metadata)?;
        return if source_rate == rate {
            Ok(audio)
        } else {
            crate::stream_decode::resample_audio(&audio, rate, cancelled)
        };
    }
    if crate::stream_decode::should_stream(resampled_frames(
        samples.len() as u64,
        source_rate,
        rate,
    )?) {
        let source = AudioData {
            disk: None,
            sample_rate_hz: source_rate,
            samples: Arc::new(samples),
            peaks: Default::default(),
            metadata,
        };
        return crate::stream_decode::resample_audio(&source, rate, cancelled);
    }
    if source_rate != rate {
        samples = convert_samples(&samples, source_rate, rate, cancelled)?;
    }
    let peaks = prepare_peaks(&samples, cancelled)?;
    let audio = AudioData {
        disk: None,
        sample_rate_hz: rate,
        samples: Arc::new(samples),
        metadata,
        peaks: Arc::new(peaks),
    };
    register_audio(&audio);
    Ok(audio)
}

#[cfg(test)]
mod compressed_storage_tests {
    use super::*;
    #[test]
    fn bounded_mp4_demuxing_preserves_resident_and_disk_pcm() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-2.m4a");
        for rate in [44100, 48000, 192000] {
            let resident = decode_with_streaming(&path, rate, &|| false, |_| false).unwrap();
            let disk = decode_with_streaming(&path, rate, &|| false, |_| true).unwrap();
            assert!(disk.disk.is_some());
            let materialized = disk.materialized(&|| false).unwrap();
            assert_eq!(resident.metadata, materialized.metadata);
            assert_eq!(resident.samples, materialized.samples);
            assert_eq!(resident.peaks, materialized.peaks);
        }
    }
}
