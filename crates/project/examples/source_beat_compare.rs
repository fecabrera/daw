//! Headless comparison of the production source BPM detector and offline OBTAIN.
use daw_media::{
    BeatAnalysis, BeatMode, BeatProfile, BeatSummary, Tempo, audio_beats, audio_beats_mode,
    audio_beats_profile, audio_tempo, audio_tempo_without_percussion, decode_audio_bounded,
};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    time::Instant,
};

struct Options {
    input: Option<PathBuf>,
    rate: u32,
    repeats: usize,
    experiment: Option<BeatMode>,
    profile: bool,
    percussion_recovery: bool,
}

fn options(args: impl IntoIterator<Item = OsString>) -> Option<Options> {
    let mut args = args.into_iter();
    let mut options = Options {
        input: None,
        rate: 48_000,
        repeats: 0,
        experiment: None,
        profile: false,
        percussion_recovery: false,
    };
    while let Some(arg) = args.next() {
        if arg == "--profile" && !options.profile {
            options.profile = true;
            continue;
        }
        if arg == "--percussion-recovery" && !options.percussion_recovery {
            options.percussion_recovery = true;
            continue;
        }
        let value = args.next()?;
        match arg.to_str()? {
            "--input" if options.input.is_none() => options.input = Some(value.into()),
            "--rate" => options.rate = value.to_str()?.parse().ok()?,
            "--repeats" => options.repeats = value.to_str()?.parse().ok()?,
            "--experiment" if options.experiment.is_none() => {
                options.experiment = Some(match value.to_str()? {
                    "legacy" => BeatMode::Legacy,
                    "reuse" => BeatMode::Reuse,
                    "onsets" => BeatMode::Onsets,
                    "rivals" => BeatMode::Rivals,
                    "combined" => BeatMode::Combined,
                    "selection" => BeatMode::Selection,
                    _ => return None,
                });
            }
            _ => return None,
        }
    }
    (daw_core::SAMPLE_RATES.contains(&options.rate)
        && options.repeats <= 9
        && !(options.percussion_recovery && options.experiment.is_some()))
    .then_some(options)
}

fn timed<T>(operation: impl FnOnce() -> T) -> (T, u64) {
    let start = Instant::now();
    let result = operation();
    (
        result,
        start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX),
    )
}

fn source_tempo(value: Tempo) -> Value {
    match value {
        Tempo::Estimate { bpm, sampled } => {
            json!({"status": "estimate", "bpm": bpm, "sampled": sampled})
        }
        Tempo::OctaveAmbiguous { bpm, sampled } => {
            json!({"status": "octave", "bpm": bpm, "sampled": sampled})
        }
        Tempo::Weak => json!({"status": "weak"}),
        Tempo::Ambiguous => json!({"status": "ambiguous"}),
    }
}

fn beat_track(value: &BeatAnalysis) -> Value {
    let mut result = match value.summary {
        BeatSummary::Stable {
            bpm,
            nearest_100_bpm,
        } => json!({"status":"estimate", "bpm":bpm, "nearest_100_bpm": nearest_100_bpm}),
        BeatSummary::Weak => json!({"status":"weak"}),
        BeatSummary::Ambiguous => json!({"status":"ambiguous"}),
        BeatSummary::Variable => json!({"status":"variable"}),
        BeatSummary::OutOfRange => json!({"status":"out_of_range"}),
    };
    result["sampled"] = json!(value.sampled);
    result["rate"] = json!(value.sample_rate);
    result["analyzed"] = json!(value.analyzed);
    result["runs"] = Value::Array(
        value
            .runs
            .iter()
            .map(|run| {
                let beats: Vec<_> = run
                    .beats
                    .iter()
                    .map(|beat| json!({"frame":beat.frame,"inferred":beat.inferred}))
                    .collect();
                json!({"range":run.range,"bpm":run.bpm,"octave_bpm":run.octave_bpm,"beats":beats})
            })
            .collect(),
    );
    result
}

#[derive(PartialEq)]
enum Outcome {
    Tempo(Tempo),
    Beats(BeatAnalysis),
}

#[derive(Default)]
struct Measurements {
    outcome: Option<Result<Outcome, String>>,
    first: u64,
    warm: Vec<u64>,
}

fn compare(path: &Path, options: &Options, index: usize) -> Result<Value, (&'static str, String)> {
    let cancelled = || false;
    let (audio, prepare) = timed(|| decode_audio_bounded(path, options.rate, &cancelled));
    let audio = audio.map_err(|error| ("decode_error", error.0))?;
    let mut results: [Measurements; 4] = std::array::from_fn(|_| Measurements::default());
    let (names, errors, count) = if options.percussion_recovery {
        (
            ["baseline", "obtain", "ellis_recovery", "obtain_recovery"],
            [
                "baseline_error",
                "obtain_error",
                "ellis_recovery_error",
                "obtain_recovery_error",
            ],
            4,
        )
    } else {
        (
            ["baseline", "obtain", "experiment", ""],
            ["baseline_error", "obtain_error", "experiment_error", ""],
            if options.experiment.is_some() { 3 } else { 2 },
        )
    };
    let obtain_mode = if options.percussion_recovery {
        BeatMode::Selection
    } else {
        BeatMode::default()
    };
    for round in 0..=options.repeats {
        let first = (index + round) % count;
        for offset in 0..count {
            let version = (first + offset) % count;
            let (outcome, elapsed) = timed(|| match version {
                0 if options.percussion_recovery => {
                    audio_tempo_without_percussion(&audio, &cancelled).map(Outcome::Tempo)
                }
                1 if options.percussion_recovery => {
                    audio_beats_mode(&audio, obtain_mode, &cancelled).map(Outcome::Beats)
                }
                0 => audio_tempo(&audio, &cancelled).map(Outcome::Tempo),
                1 => audio_beats(&audio, &cancelled).map(Outcome::Beats),
                2 if options.percussion_recovery => {
                    audio_tempo(&audio, &cancelled).map(Outcome::Tempo)
                }
                3 => audio_beats(&audio, &cancelled).map(Outcome::Beats),
                _ => audio_beats_mode(&audio, options.experiment.unwrap(), &cancelled)
                    .map(Outcome::Beats),
            });
            let outcome = outcome.map_err(|error| error.0);
            let result = &mut results[version];
            if round == 0 {
                result.first = elapsed;
                result.outcome = Some(outcome);
            } else {
                if result.outcome.as_ref() != Some(&outcome) {
                    return Err((
                        errors[version],
                        "Detector result changed between calls".into(),
                    ));
                }
                result.warm.push(elapsed);
            }
        }
    }
    let mut output = json!({
        "v": 1, "id": index, "frames": audio.frame_count(), "disk": audio.disk.is_some(),
        "prepare_ns": prepare,
    });
    for (i, measurement) in results.into_iter().take(count).enumerate() {
        let name = names[i];
        output[name] = match measurement.outcome.unwrap() {
            Ok(Outcome::Tempo(value)) => source_tempo(value),
            Ok(Outcome::Beats(value)) => beat_track(&value),
            Err(message) if count == 2 => return Err((errors[i], message)),
            Err(_) => json!({"status":errors[i]}),
        };
        output[format!("{name}_first_ns")] = json!(measurement.first);
        output[format!("{name}_repeat_ns")] = json!(measurement.warm);
    }
    if options.profile {
        for (name, mode) in std::iter::once(("obtain", obtain_mode))
            .chain(options.experiment.map(|mode| ("experiment", mode)))
            .chain(
                options
                    .percussion_recovery
                    .then_some(("obtain_recovery", BeatMode::default())),
            )
        {
            let mut profile = BeatProfile::default();
            if let Ok(value) = audio_beats_profile(&audio, mode, &cancelled, &mut profile) {
                if beat_track(&value) != output[name] {
                    return Err(("profile_error", "Diagnostic result changed".into()));
                }
                let contexts: Vec<_> = profile
                    .contexts
                    .iter()
                    .map(|trace| {
                        json!({
                            "landmarks":trace.landmarks, "proposed":trace.proposed_periods,
                            "qualified":trace.qualified_periods, "kept":trace.kept_periods,
                            "guard":trace.guard, "selected":trace.selected_period,
                        })
                    })
                    .collect();
                output[format!("{name}_profile")] = json!({
                    "stage_ns":profile.stage_time.map(|stage| stage.as_nanos() as u64),
                    "read_calls":profile.read_calls,"read_frames":profile.read_frames,
                    "transforms":profile.transforms,"visits":profile.visits,"contexts":contexts,
                });
            }
        }
    }
    Ok(output)
}

fn run(options: &Options) -> io::Result<u8> {
    let mut output = io::BufWriter::new(io::stdout().lock());
    if let Some(path) = &options.input {
        match compare(path, options, 0) {
            Ok(result) => writeln!(output, "{result}")?,
            Err((_, error)) => {
                eprintln!("{error}");
                return Ok(1);
            }
        }
    } else {
        // One UTF-8 path per line on stdin. Each line yields one complete record.
        let mut input = io::stdin().lock();
        let mut line = Vec::new();
        let mut index = 0;
        loop {
            use std::io::Read;
            line.clear();
            let read = input.by_ref().take(16_385).read_until(b'\n', &mut line)?;
            if read == 0 {
                break;
            }
            if read > 16_384 || index >= 100_000 {
                return Err(io::Error::other("Comparison manifest exceeds limits"));
            }
            let path = std::str::from_utf8(&line)
                .map_err(io::Error::other)?
                .trim_end_matches(['\r', '\n']);
            let result = compare(Path::new(path), options, index)
                .unwrap_or_else(|(status, _)| json!({"v":1,"id":index,"error":status}));
            writeln!(output, "{result}")?;
            index += 1;
        }
    }
    output.flush()?;
    Ok(0)
}

fn main() -> std::process::ExitCode {
    let Some(options) = options(std::env::args_os().skip(1)) else {
        eprintln!(
            "Usage: source_beat_compare [--input PATH] [--rate HZ] [--repeats 0..9] [--experiment legacy|reuse|onsets|rivals|combined|selection | --percussion-recovery] [--profile]"
        );
        return 2.into();
    };
    match run(&options) {
        Ok(code) => code.into(),
        Err(error) => {
            eprintln!("{error}");
            1.into()
        }
    }
}
