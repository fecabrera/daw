mod audio;
mod backend;
mod metrics;
use audio::Result;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{Duration, Instant},
};
use timestretch::{Pitch, Ratio};
#[derive(Parser)]
#[command(
    version,
    about = "Offline time stretching, pitch shifting, and three-engine comparison"
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    Process(Process),
    Compare(Compare),
}
fn rate(text: &str) -> std::result::Result<u32, String> {
    text.parse::<u32>()
        .ok()
        .filter(|r| timestretch::SAMPLE_RATES.contains(r))
        .ok_or_else(|| "unsupported sample rate".into())
}
#[derive(clap::Args)]
struct Input {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    ratio: Ratio,
    /// Constant pitch shift in semitones, independent of duration (-24..=24).
    #[arg(long, default_value = "0", allow_hyphen_values = true)]
    pitch_semitones: Pitch,
    #[arg(long,default_value="48000",value_parser=rate)]
    sample_rate: u32,
}
#[derive(clap::Args)]
struct Process {
    #[command(flatten)]
    input: Input,
    #[arg(long)]
    output: PathBuf,
    #[arg(long,value_parser=backend::ENGINES)]
    engine: String,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    fixture_manifest: Option<PathBuf>,
}
#[derive(clap::Args)]
struct Compare {
    #[command(flatten)]
    input: Input,
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long)]
    fixture_manifest: Option<PathBuf>,
    #[arg(long,default_value="600",value_parser=clap::value_parser!(u64).range(1..))]
    timeout_seconds: u64,
}
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn cancelled() -> bool {
    CANCELLED.load(std::sync::atomic::Ordering::Relaxed)
}
fn main() -> ExitCode {
    let args = Cli::parse();
    if let Err(error) =
        ctrlc::set_handler(|| CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed))
    {
        eprintln!("Cannot install cancellation handler: {error}");
        return ExitCode::FAILURE;
    }
    let result = match args.command {
        Action::Process(p) => process(p),
        Action::Compare(c) => compare(c),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            if e.to_string().starts_with("arguments:") {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
fn resolved(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        Ok(fs::canonicalize(path)?)
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(fs::canonicalize(parent)?
            .join(path.file_name().ok_or("arguments: path needs a filename")?))
    }
}
fn same(a: &Path, b: &Path) -> Result<bool> {
    Ok(
        resolved(a)? == resolved(b)?
            || (a.exists() && b.exists() && same_file::is_same_file(a, b)?),
    )
}
fn validate_paths(p: &Process) -> Result<()> {
    for destination in [&p.output, &p.report] {
        if same(&p.input.input, destination)? {
            return Err("arguments: source cannot be an output/report destination".into());
        }
        if let Some(fixture) = &p.fixture_manifest
            && same(fixture, destination)?
        {
            return Err("arguments: fixture manifest cannot be replaced".into());
        }
        if destination.exists() && (!p.overwrite || !destination.is_file()) {
            return Err(format!("arguments: destination exists: {}", destination.display()).into());
        }
    }
    if same(&p.output, &p.report)? {
        return Err("arguments: output and report must be distinct files".into());
    }
    Ok(())
}
fn stage(path: &Path) -> Result<tempfile::NamedTempFile> {
    Ok(tempfile::NamedTempFile::new_in(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?)
}
fn publish(temp: tempfile::NamedTempFile, path: &Path, overwrite: bool) -> Result<()> {
    if overwrite {
        temp.persist(path).map_err(|e| e.error)?;
    } else {
        temp.persist_noclobber(path).map_err(|e| e.error)?;
    }
    Ok(())
}
fn process(p: Process) -> Result<()> {
    let wall = Instant::now();
    let revision = if p.engine == "timestretch" {
        timestretch::REVISION
    } else {
        1
    };
    validate_paths(&p)?;
    let source_hash = audio::file_hash(&p.input.input)?;
    let (input, decode, conversion) = audio::read(&p.input.input, p.input.sample_rate)?;
    let prepared_hash = input.hash();
    let engine_start = Instant::now();
    let (output, details) =
        backend::process(&input, p.input.ratio, p.input.pitch_semitones, &p.engine)?;
    let engine_seconds = engine_start.elapsed().as_secs_f64();
    let analysis_start = Instant::now();
    let diagnostics = metrics::diagnostics(&output);
    let fixture = metrics::fixture(
        p.fixture_manifest.as_deref(),
        &input,
        &output,
        p.input.ratio,
        p.input.pitch_semitones,
    )?;
    let analysis = analysis_start.elapsed().as_secs_f64();
    let audio_temp = stage(&p.output)?;
    let encoding_start = Instant::now();
    audio::write(audio_temp.path(), &output)?;
    let encoding = encoding_start.elapsed().as_secs_f64();
    let output_hash = audio::file_hash(audio_temp.path())?;
    let report = json!({"schema_version":1,"status":"complete","engine":p.engine,"revision":revision,"backend":details,
        "file_sha256":source_hash,"prepared_sha256":prepared_hash,"output_file_sha256":output_hash,"output_sample_sha256":output.hash(),
        "sample_rate":input.rate,"channels":input.channels,"input_frames":input.frames(),"output_frames":output.frames(),"expected_frames":p.input.ratio.output_frames(input.frames())?,
        "ratio":{"numerator":p.input.ratio.numerator(),"denominator":p.input.ratio.denominator()},
        "pitch":{"semitones":p.input.pitch_semitones.semitones(),"factor":p.input.pitch_semitones.factor(),"revision":if p.engine=="timestretch" {Some(timestretch::PITCH_REVISION)}else{None}},
        "build":{"package_version":env!("CARGO_PKG_VERSION"),"revision":env!("BUILD_REVISION"),"dirty":env!("BUILD_DIRTY"),"target":env!("BUILD_TARGET"),"profile":env!("BUILD_PROFILE")},
        "host":{"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"available_parallelism":std::thread::available_parallelism().map(|n|n.get()).ok(),"cpu_model":std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_|"unavailable".into())},
        "timings_seconds":{"decode":decode,"conversion":conversion,"engine":engine_seconds,"encoding":encoding,"analysis":analysis,"command_before_publication":wall.elapsed().as_secs_f64()},
        "processing_factor":engine_seconds/(input.frames().max(1) as f64/input.rate as f64),
        "peak_process_rss":{"status":"unavailable","units":"bytes","method":null},"diagnostics":diagnostics,"fixture_metrics":fixture});
    let mut report_temp = stage(&p.report)?;
    report_temp.write_all(&serde_json::to_vec_pretty(&report)?)?;
    report_temp.as_file().sync_all()?;
    // Recheck source identities immediately before replacing any existing file.
    validate_paths(&p)?;
    if cancelled() {
        return Err("processing cancelled before publication".into());
    }
    publish(audio_temp, &p.output, p.overwrite)?;
    publish(report_temp, &p.report, p.overwrite).map_err(|e| {
        format!(
            "partial publication: audio at {}; report failed: {e}",
            p.output.display()
        )
    })?;
    println!(
        "{}: {} -> {} frames",
        p.engine,
        input.frames(),
        output.frames()
    );
    Ok(())
}
struct RunningChild(std::process::Child);
impl Drop for RunningChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn compare(c: Compare) -> Result<()> {
    // Validate source before creating a destination.
    fs::metadata(&c.input.input)?;
    if c.output_dir.exists() {
        if !c.output_dir.is_dir() || fs::read_dir(&c.output_dir)?.next().is_some() {
            return Err("arguments: comparison directory must be new or empty".into());
        }
    } else {
        fs::create_dir_all(&c.output_dir)?;
    }
    let executable = std::env::current_exe()?;
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    let mut prepared: Option<Value> = None;
    let mut source: Option<Value> = None;
    for engine in backend::ENGINES {
        if cancelled() {
            failures.push(json!({"engine":engine,"error":"comparison cancelled"}));
            continue;
        }
        let output = c.output_dir.join(format!("{engine}.wav"));
        let report = c.output_dir.join(format!("{engine}.json"));
        let log = c.output_dir.join(format!("{engine}.log"));
        let file = fs::File::create(&log)?;
        let mut command = Command::new(&executable);
        command
            .arg("process")
            .arg("--input")
            .arg(&c.input.input)
            .arg("--output")
            .arg(&output)
            .arg("--report")
            .arg(&report)
            .arg("--engine")
            .arg(engine)
            .arg("--ratio")
            .arg(c.input.ratio.to_string())
            .arg("--pitch-semitones")
            .arg(c.input.pitch_semitones.semitones().to_string())
            .arg("--sample-rate")
            .arg(c.input.sample_rate.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::from(file.try_clone()?))
            .stderr(Stdio::from(file));
        if let Some(manifest) = &c.fixture_manifest {
            command.arg("--fixture-manifest").arg(manifest);
        }
        let started = Instant::now();
        let run = (|| -> Result<Value> {
            let mut child = RunningChild(command.spawn()?);
            let status = loop {
                if let Some(status) = child.0.try_wait()? {
                    break status;
                }
                if cancelled() || started.elapsed() >= Duration::from_secs(c.timeout_seconds) {
                    let _ = child.0.kill();
                    let _ = child.0.wait();
                    return Err(if cancelled() {
                        "comparison cancelled"
                    } else {
                        "engine timeout"
                    }
                    .into());
                }
                std::thread::sleep(Duration::from_millis(20));
            };
            if !status.success() {
                return Err(
                    format!("engine failed ({status}); {}", fs::read_to_string(&log)?).into(),
                );
            }
            let mut result: Value = serde_json::from_slice(&fs::read(&report)?)?;
            if result["status"] != "complete"
                || result["engine"] != engine
                || result["output_frames"] != result["expected_frames"]
                || result["pitch"]["semitones"].as_f64()
                    != Some(c.input.pitch_semitones.semitones())
                || result["pitch"]["factor"].as_f64() != Some(c.input.pitch_semitones.factor())
                || result["output_file_sha256"] != audio::file_hash(&output)?
            {
                return Err("invalid result identity/length/hash".into());
            }
            if prepared
                .as_ref()
                .is_some_and(|h| h != &result["prepared_sha256"])
                || source.as_ref().is_some_and(|h| h != &result["file_sha256"])
            {
                return Err("unequal source/prepared hashes".into());
            }
            prepared = Some(result["prepared_sha256"].clone());
            source = Some(result["file_sha256"].clone());
            result["child_wall_seconds"] = json!(started.elapsed().as_secs_f64());
            Ok(result)
        })();
        match run { Ok(report)=>reports.push(report), Err(e)=>failures.push(json!({"engine":engine,"error":e.to_string(),"log":log.file_name().unwrap().to_string_lossy()})) }
    }
    let pairs: Vec<_>=reports.iter().filter(|r|r["engine"]!="timestretch").map(|r| {
        let integrated=reports.iter().find(|r|r["engine"]=="timestretch");
        json!({"reference":r["engine"],"engine_time_ratio":integrated.and_then(|i|Some(i["timings_seconds"]["engine"].as_f64()? / r["timings_seconds"]["engine"].as_f64()?)),"quality_acceptance":"unassessed"})
    }).collect();
    let summary = json!({"schema_version":1,"status":if failures.is_empty(){"complete"}else{"failed"},"reports":reports,"comparisons":pairs,"failures":failures,"acceptance_baseline":"rubberband-r3"});
    let path = c.output_dir.join("comparison.json");
    let mut temp = stage(&path)?;
    temp.write_all(&serde_json::to_vec_pretty(&summary)?)?;
    publish(temp, &path, false)?;
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("comparison incomplete; see {}", path.display()).into())
    }
}
