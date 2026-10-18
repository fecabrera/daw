use crate::audio::{Audio, Result};
use serde_json::{Value, json};
use timestretch::{Pitch, Ratio, Request};
pub const ENGINES: [&str; 3] = ["timestretch", "rubberband-r2", "rubberband-r3"];
pub fn process(audio: &Audio, ratio: Ratio, pitch: Pitch, engine: &str) -> Result<(Audio, Value)> {
    let (samples, details) = if engine == "timestretch" {
        let output = Request::new(audio.rate, audio.channels, ratio)
            .with_pitch(pitch)
            .process_with_progress(&audio.samples, |_| {
                if crate::cancelled() {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            })?;
        (
            output,
            json!({"algorithm":"linked-phase-vocoder", "revision":timestretch::REVISION, "pitch_revision":timestretch::PITCH_REVISION,"pitch_scale":pitch.factor(),"pitch_stages":pitch.stages().map(|p| p.semitones()).collect::<Vec<_>>(),"threads":1}),
        )
    } else {
        reference(audio, ratio, pitch, engine)?
    };
    if samples.len() != ratio.output_frames(audio.frames())? * audio.channels
        || samples.iter().any(|s| !s.is_finite())
    {
        return Err("invalid output length or nonfinite samples".into());
    }
    Ok((
        Audio {
            samples,
            channels: audio.channels,
            rate: audio.rate,
        },
        details,
    ))
}
#[cfg(not(feature = "rubberband-reference"))]
fn reference(_: &Audio, _: Ratio, _: Pitch, _: &str) -> Result<(Vec<f32>, Value)> {
    Err("Rubber Band references unavailable: rebuild with rubberband-reference".into())
}
#[cfg(feature = "rubberband-reference")]
fn reference(audio: &Audio, ratio: Ratio, pitch: Pitch, engine: &str) -> Result<(Vec<f32>, Value)> {
    if pitch.stages().len() > 1 {
        let mut prepared: Option<Audio> = None;
        let mut stages = Vec::new();
        for (index, stage) in pitch.stages().enumerate() {
            let input = prepared.as_ref().unwrap_or(audio);
            let duration = if index == 0 { ratio } else { Ratio::new(1, 1)? };
            let (samples, details) = reference(input, duration, stage, engine)?;
            stages.push(details);
            prepared = Some(Audio {
                samples,
                rate: audio.rate,
                channels: audio.channels,
            });
        }
        let mut details = stages.last().unwrap().clone();
        details["pitch_scale"] = json!(pitch.factor());
        details["pitch_stages"] = json!(stages);
        return Ok((prepared.unwrap().samples, details));
    }
    use rubberband::{Options, Stretcher};
    let version = match engine {
        "rubberband-r2" => 2,
        "rubberband-r3" => 3,
        _ => return Err("unknown engine".into()),
    };
    let options = Options::PROCESS_OFFLINE
        | Options::CHANNELS_TOGETHER
        | Options::THREADING_NEVER
        | if version == 2 {
            Options::ENGINE_FASTER
        } else {
            Options::ENGINE_FINER
        };
    let expected = ratio.output_frames(audio.frames())?;
    let duration =
        u32::try_from(audio.frames()).map_err(|_| "input exceeds reference API frame limit")?;
    let mut processor = Stretcher::new(
        audio.rate,
        audio.channels as u32,
        options,
        ratio.as_f64(),
        pitch.factor(),
    );
    if processor.engine_version() != version {
        return Err("Rubber Band returned the wrong engine version".into());
    }
    const BLOCK: usize = 4096;
    processor.set_expected_input_duraction(duration);
    processor.set_max_process_size(BLOCK as u32);
    let mut planes = vec![vec![0.0; BLOCK]; audio.channels];
    let mut output_planes = vec![vec![0.0; BLOCK]; audio.channels];
    let mut output = Vec::new();
    output.try_reserve_exact(
        expected
            .checked_mul(audio.channels)
            .ok_or("reference output too large")?,
    )?;
    for study in [true, false] {
        if audio.frames() == 0 {
            break;
        }
        for (index, chunk) in audio.samples.chunks(BLOCK * audio.channels).enumerate() {
            if crate::cancelled() {
                return Err("reference cancelled".into());
            }
            let frames = chunk.len() / audio.channels;
            for (c, plane) in planes.iter_mut().enumerate() {
                for i in 0..frames {
                    plane[i] = chunk[i * audio.channels + c];
                }
            }
            let input: Vec<_> = planes.iter().map(|p| &p[..frames]).collect();
            let last = (index + 1) * BLOCK >= audio.frames();
            if study {
                processor.study(&input, last);
            } else {
                processor.process(&input, last);
                while let Some(available) = processor.available().filter(|n| *n > 0) {
                    let size = (available as usize).min(BLOCK);
                    let mut buffers: Vec<_> =
                        output_planes.iter_mut().map(|p| &mut p[..size]).collect();
                    let count = processor.retrieve(&mut buffers) as usize;
                    if count == 0 {
                        return Err("reference stalled while draining".into());
                    }
                    for i in 0..count {
                        for plane in &output_planes {
                            output.push(plane[i]);
                        }
                    }
                }
            }
        }
    }
    if audio.frames() > 0 && processor.available().is_some() {
        return Err("reference did not finish".into());
    }
    let raw_frames = output.len() / audio.channels;
    if raw_frames.abs_diff(expected) > 1 {
        return Err(format!("reference produced {raw_frames} frames; expected {expected}").into());
    }
    let raw = Audio {
        samples: output,
        channels: audio.channels,
        rate: audio.rate,
    };
    let raw_hash = raw.hash();
    let mut output = raw.samples;
    output.resize(expected * audio.channels, 0.0);
    Ok((
        output,
        json!({"native_version":"4.0.0", "binding_version":"0.1.5", "sys_version":"0.1.4", "engine_version":version,
        "options_bits":if version==2 {0x10010000u32} else {0x30010000u32}, "options":["offline","channels-together","threading-never","stretch-elastic","transients-crisp","detector-compound","phase-laminar","window-standard","smoothing-off","formant-shifted","pitch-high-speed"],
        "pitch_scale":pitch.factor(),"raw_frames":raw_frames,"raw_sample_sha256":raw_hash,"length_adjustment_frames":expected as i128-raw_frames as i128,"delay_compensation":"native offline", "threads":1}),
    ))
}
