//! Prepare private, blinded real-recording comparisons; never writes to inputs.
#[path = "../audio.rs"]
mod audio;
use audio::{Audio, Result};
use audio_analysis::{TransientDetector as Mode, TransientRequest};
use clap::Parser;
use serde_json::{Value, json};
use std::{fs, ops::ControlFlow, path::PathBuf, time::Instant};
use timestretch::{Ratio, Request};

#[derive(Parser)]
#[command(
    about = "Prepare private Energy/Combined A/B listening files from a WAV manifest. No listener scores are inferred."
)]
struct Args {
    #[arg(long)]
    manifest: PathBuf,
    /// Must not exist, and must not be inside a recording source directory.
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value = "48000")]
    sample_rate: u32,
    #[arg(long, default_value = "3/4,3/2,2/1", value_delimiter = ',')]
    ratios: Vec<Ratio>,
    #[arg(long, default_value = "10")]
    seconds: u32,
    #[arg(long, default_value = "106391")]
    seed: u64,
}
fn main() -> Result<()> {
    run(Args::parse())
}
fn run(args: Args) -> Result<()> {
    if !timestretch::SAMPLE_RATES.contains(&args.sample_rate) || !(1..=30).contains(&args.seconds) {
        return Err("use a supported sample rate and 1–30 seconds".into());
    }
    if args.ratios.is_empty() {
        return Err("at least one ratio is required".into());
    }
    let entries: Value = serde_json::from_slice(&fs::read(&args.manifest)?)?;
    let entries = entries.as_array().ok_or("manifest must be an array")?;
    if entries.is_empty() {
        return Err("manifest has no recordings".into());
    }
    let parent = args
        .output_dir
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let parent = fs::canonicalize(parent)?;
    let output = parent.join(
        args.output_dir
            .file_name()
            .ok_or("output directory needs a name")?,
    );
    if output.exists() {
        return Err("output directory already exists; choose a fresh directory".into());
    }
    let mut sources = Vec::new();
    for entry in entries {
        let path = PathBuf::from(entry["path"].as_str().ok_or("entry requires path")?);
        let path = if path.is_absolute() {
            path
        } else {
            args.manifest
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(path)
        };
        let path = fs::canonicalize(path)?;
        if output.starts_with(path.parent().ok_or("source has no parent")?) {
            return Err("output must be outside source directories".into());
        }
        if entry["rights"].as_str().is_none_or(|s| s.trim().is_empty()) {
            return Err("entry requires a rights/use note".into());
        }
        sources.push((entry, audio::file_hash(&path)?, path));
    }
    fs::create_dir(&output)?;
    fs::create_dir(output.join("raw"))?;
    fs::create_dir(output.join("audition"))?;
    fs::create_dir(output.join("private"))?;
    let mut report = Vec::new();
    let mut key = Vec::new();
    let mut ratings = String::from(
        "case,listener,label,overall_1_to_5,transients,tonal_stability,vocals,stereo,boundaries,notes\n",
    );
    let mut state = args.seed.max(1);
    for (index, (entry, original_hash, path)) in sources.iter().enumerate() {
        let (mut input, _, _) = audio::read(path, args.sample_rate)?;
        let offset = entry["start_seconds"].as_f64().unwrap_or(0.0);
        if !offset.is_finite() || offset < 0.0 {
            return Err("invalid excerpt start".into());
        }
        let first = (offset * f64::from(input.rate)).round() as usize;
        let frames =
            (args.seconds as usize * input.rate as usize).min(input.frames().saturating_sub(first));
        if frames == 0 {
            return Err("empty excerpt".into());
        }
        input.samples =
            input.samples[first * input.channels..(first + frames) * input.channels].to_vec();
        audio::write(
            &output.join("raw").join(format!("source-{index:02}.wav")),
            &input,
        )?;
        let mut cases = Vec::new();
        for (r, ratio) in args.ratios.iter().copied().enumerate() {
            let id = format!("case-{index:02}-{r:02}");
            let request = Request::new(input.rate, input.channels, ratio);
            let mut renders = Vec::new();
            let mut metrics = Vec::new();
            for mode in [Mode::Energy, Mode::Combined] {
                let start = Instant::now();
                let samples =
                    request.process_with_transient_detector(&input.samples, mode, |_| {
                        ControlFlow::Continue(())
                    })?;
                let elapsed = start.elapsed().as_secs_f64();
                if samples.len() != ratio.output_frames(input.frames())? * input.channels
                    || !samples.iter().all(|x| x.is_finite())
                {
                    return Err("invalid render".into());
                }
                let render = Audio {
                    samples,
                    channels: input.channels,
                    rate: input.rate,
                };
                let onsets = TransientRequest {
                    detector: mode,
                    ..TransientRequest::new(input.rate, input.channels)
                }
                .detect_with_progress(&input.samples, |_| ControlFlow::Continue(()))?;
                audio::write(
                    &output
                        .join("raw")
                        .join(format!("{id}-{}.wav", mode.revision())),
                    &render,
                )?;
                metrics.push(json!({"detector":mode.revision(),"seconds":elapsed,"frames":render.frames(),"pcm_hash":render.hash(),"detected_source_frames":onsets}));
                renders.push(render);
            }
            // One common attenuation for reference and both renders. No limiting,
            // per-file RMS matching or claim of formal LUFS matching.
            let peak = input
                .samples
                .iter()
                .chain(renders.iter().flat_map(|a| a.samples.iter()))
                .fold(0.0_f32, |a, &b| a.max(b.abs()));
            let gain = if peak > 0.95 { 0.95 / peak } else { 1.0 };
            let audition = |audio: &Audio| Audio {
                samples: audio.samples.iter().map(|s| s * gain).collect(),
                channels: audio.channels,
                rate: audio.rate,
            };
            audio::write(
                &output.join("audition").join(format!("{id}-original.wav")),
                &audition(&input),
            )?;
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let order = if state & 1 == 0 { [0, 1] } else { [1, 0] };
            for (label, which) in ["A", "B"].into_iter().zip(order) {
                audio::write(
                    &output.join("audition").join(format!("{id}-{label}.wav")),
                    &audition(&renders[which]),
                )?;
                ratings.push_str(&format!("{id},,{label},,,,,,,\n"));
                key.push(json!({"case":id,"label":label,"detector":metrics[which]["detector"]}));
            }
            cases.push(json!({"case":id,"ratio":ratio.to_string(),"common_audition_gain":gain,"renders":metrics}));
        }
        if audio::file_hash(path)? != *original_hash {
            return Err("source changed during run; discard this comparison".into());
        }
        report.push(json!({"id":entry["id"],"source":path,"source_sha256":original_hash,"rights":entry["rights"],"start_seconds":offset,"frames":frames,"channels":input.channels,"cases":cases}));
    }
    fs::write(
        output.join("private/key.json"),
        serde_json::to_vec_pretty(&json!({"seed":args.seed,"mapping":key}))?,
    )?;
    fs::write(output.join("ratings.csv"), ratings)?;
    fs::write(
        output.join("private/report.json"),
        serde_json::to_vec_pretty(
            &json!({"engine_revision":timestretch::REVISION,"package_version":env!("CARGO_PKG_VERSION"),"status":"awaiting human listening","normalization":"shared attenuation only; not LUFS matched","sources":report}),
        )?,
    )?;
    fs::write(
        output.join("LISTENING.md"),
        "# Detector A/B listening\n\nStatus: awaiting human listening. No ratings have been supplied.\n\nUse audition/ only; keep private/ hidden until ratings are complete. Compare A and B against the original. Rate 1 (unusable) to 5 (no objectionable artifact), noting transients, sustained tones, vocals, stereo and boundaries. Use NA where inapplicable. Complete ratings.csv with a listener name; repeat independently for at least three listeners before unblinding private/key.json.\n\nBoth modes use one shared attenuation with the original, no limiter or per-file loudness normalization. This two-detector comparison is not the complete three-engine/12-excerpt acceptance panel. Sources and renders are private evaluation material; do not commit or redistribute licensed samples. Source hashes were checked before/after rendering.\n",
    )?;
    println!(
        "Prepared {} recordings in {}. Human listening remains pending.",
        sources.len(),
        output.display()
    );
    Ok(())
}
