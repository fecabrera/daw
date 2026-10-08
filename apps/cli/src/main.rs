use clap::{Parser, Subcommand};
use daw_media::{ExportFormat, ExportSettings, Mp3Bitrate, WavCodec};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Validate DAW projects and render stereo WAV or MP3 without graphics or audio devices"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Validate {
        #[arg(long)]
        project: PathBuf,
    },
    Render {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
        #[arg(long, value_parser = ["wav", "mp3"])]
        format: Option<String>,
        #[arg(long, value_parser = ["pcm16", "pcm24", "float32"])]
        codec: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u16))]
        bitrate: Option<u16>,
    },
}
fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let result = match Args::parse().command {
        Command::Validate { project } => daw_project::Session::open(&project).map(|session| {
            for warning in &session.warnings {
                eprintln!("Warning: {warning}");
            }
            println!(
                "Valid project: {} ({} tracks)",
                session.project.name,
                session.project.tracks.len()
            );
        }),
        Command::Render {
            project,
            output,
            overwrite,
            format,
            codec,
            bitrate,
        } => {
            let settings = export_settings(&output, format.as_deref(), codec.as_deref(), bitrate)
                .unwrap_or_else(|error| {
                    clap::Error::raw(clap::error::ErrorKind::InvalidValue, error.to_string()).exit()
                });
            daw_project::Session::open(&project)
                .and_then(|session| {
                    for warning in &session.warnings {
                        eprintln!("Warning: {warning}");
                    }
                    session
                        .export_with_settings(&output, overwrite, settings)
                        .map(|warnings| {
                            warnings
                                .into_iter()
                                .filter(|w| !session.warnings.contains(w))
                                .collect::<Vec<_>>()
                        })
                })
                .map(|warnings| {
                    for warning in warnings {
                        eprintln!("Warning: {warning}");
                    }
                    println!("Exported {}", output.display());
                })
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn export_settings(
    output: &std::path::Path,
    format: Option<&str>,
    codec: Option<&str>,
    bitrate: Option<u16>,
) -> daw_project::Result<ExportSettings> {
    let mut settings = ExportSettings::default();
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    settings.format = match format.unwrap_or(if extension == "mp3" { "mp3" } else { "wav" }) {
        "mp3" => ExportFormat::Mp3,
        _ => ExportFormat::Wav,
    };
    if ["wav", "mp3"].contains(&extension.as_str()) && extension != settings.format.extension() {
        return Err(daw_project::Error(
            "Output extension does not match the export format".into(),
        ));
    }
    if settings.format == ExportFormat::Mp3 {
        if codec.is_some() {
            return Err(daw_project::Error(
                "--codec applies to WAV; MP3 uses MPEG Layer III (LAME)".into(),
            ));
        }
        settings.mp3_bitrate = Mp3Bitrate::ALL
            .into_iter()
            .find(|value| value.kbps() == bitrate.unwrap_or(192))
            .ok_or_else(|| {
                daw_project::Error("MP3 bitrate must be 128, 192, 256, or 320 kbps".into())
            })?;
    } else {
        if bitrate.is_some() {
            return Err(daw_project::Error("--bitrate applies to MP3".into()));
        }
        settings.wav_codec = match codec.unwrap_or("pcm24") {
            "pcm16" => WavCodec::Pcm16,
            "float32" => WavCodec::Float32,
            _ => WavCodec::Pcm24,
        };
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_arguments_use_matching_formats_and_reject_incompatible_options() {
        assert_eq!(
            export_settings(std::path::Path::new("mix.wav"), None, None, None).unwrap(),
            ExportSettings::default()
        );
        let mp3 = export_settings(std::path::Path::new("mix.MP3"), None, None, Some(320)).unwrap();
        assert_eq!(mp3.format, ExportFormat::Mp3);
        assert_eq!(mp3.mp3_bitrate, Mp3Bitrate::Kbps320);
        assert_eq!(
            export_settings(std::path::Path::new("mix.wav"), None, Some("float32"), None)
                .unwrap()
                .wav_codec,
            WavCodec::Float32
        );
        for (path, format, codec, bitrate) in [
            ("mix.mp3", Some("wav"), None, None),
            ("mix.mp3", None, Some("pcm16"), None),
            ("mix.wav", None, None, Some(320)),
            ("mix.mp3", None, None, Some(123)),
        ] {
            assert!(export_settings(std::path::Path::new(path), format, codec, bitrate).is_err());
        }
        assert!(
            Args::try_parse_from([
                "daw-cli",
                "render",
                "--project",
                "song",
                "--output",
                "mix.wav",
                "--codec",
                "aac"
            ])
            .is_err()
        );
    }
}
