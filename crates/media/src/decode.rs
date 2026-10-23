//! Worker-side source decoding. Compressed stream lengths are never allocation hints.
use crate::*;
use std::io::{Read, Seek, SeekFrom};
use symphonia::core::codecs::{CODEC_TYPE_AAC, CODEC_TYPE_MP3};

pub const IMPORT_EXTENSIONS: &[&str] = &["wav", "mp3", "aac", "m4a", "mp4"];

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
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(Box::new(file), Default::default()),
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| err(&e))?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| matches!(t.codec_params.codec, CODEC_TYPE_MP3 | CODEC_TYPE_AAC))
        .ok_or_else(|| err(&"Supported audio formats are WAV, MP3 and AAC-LC"))?;
    let params = &track.codec_params;
    let sample_format = if params.codec == CODEC_TYPE_MP3 {
        "mp3"
    } else {
        "aac"
    };
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
    let mut decoder = symphonia::default::get_codecs()
        .make(params, &DecoderOptions::default())
        .map_err(|e| err(&e))?;
    let mut reservations = vec![AudioReservation::new(0, 1024 * 1024)?];
    let mut source_spec = None;
    let mut samples = Vec::new();
    let mut admitted = 0;
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
        let required = samples
            .len()
            .checked_add(audio.frames())
            .ok_or_else(|| err(&"Audio length overflow"))?;
        if required > admitted {
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
        let mut buffer = SampleBuffer::<f32>::new(audio.frames() as u64, *audio.spec());
        buffer.copy_interleaved_ref(audio);
        // MP3 applies packet trims in its decoder; AAC does not.
        let (trim_start, trim_end) = if sample_format == "aac" {
            (packet.trim_start() as usize, packet.trim_end() as usize)
        } else {
            (0, 0)
        };
        let frame_count = buffer.samples().len() / channels;
        let keep = frame_count
            .checked_sub(trim_start)
            .and_then(|n| n.checked_sub(trim_end))
            .ok_or_else(|| err(&"Invalid encoder delay or padding"))?;
        for frame in buffer
            .samples()
            .chunks_exact(channels)
            .skip(trim_start)
            .take(keep)
        {
            let l = frame[0];
            let r = if channels == 1 { l } else { frame[1] };
            if !l.is_finite() || !r.is_finite() {
                return Err(err(&"Source contains non-finite samples"));
            }
            samples.push([l, r]);
        }
    }
    if expected_end.is_some_and(|end| packet_end < end) {
        return Err(err(&"Incomplete compressed audio data"));
    }
    if samples.is_empty() {
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
    if source_rate != rate {
        samples = convert_samples(&samples, source_rate, rate, cancelled)?;
    }
    let peaks = prepare_peaks(&samples, cancelled)?;
    let audio = AudioData {
        sample_rate_hz: rate,
        samples: Arc::new(samples),
        metadata,
        peaks: Arc::new(peaks),
    };
    register_audio(&audio);
    Ok(audio)
}
