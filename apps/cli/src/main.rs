mod collect_sources;
mod tracks;
use clap::{Parser, Subcommand};
use daw_media::{ExportFormat, ExportSettings, Mp3Bitrate, WavCodec};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Inspect tracks, validate projects, collect sources and render audio without graphics or audio devices"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// List saved tracks or show every exact name match without loading audio.
    Tracks {
        /// Project folder; defaults to the current directory.
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Exact, case-sensitive track name. Quote names containing spaces.
        track_name: Option<String>,
    },
    /// Copy original media into a saved project and save its source locations.
    #[command(
        long_about = "Copy complete original media into a saved project using verified transactional collection. Save and close the desktop project first. The command runs without a confirmation prompt and uses only saved assets, including unused assets and sample inputs. Desktop Undo/Redo history is not available to this process."
    )]
    CollectSources {
        /// Project folder; defaults to the current directory.
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Check sources and report totals without copying files or saving changes.
        #[arg(long)]
        dry_run: bool,
    },
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
        #[arg(long, value_parser = ["wav", "mp3", "aac"])]
        format: Option<String>,
        #[arg(long, value_parser = ["pcm16", "pcm24", "float32"])]
        codec: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u16))]
        bitrate: Option<u16>,
    },
}
fn main() {
    daw_media::configure_preparation_cache(daw_media::preparation_cache_directory());
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let result = match Args::parse().command {
        Command::Tracks {
            project,
            track_name,
        } => daw_project::inspect_project(&project)
            .and_then(|project| tracks::describe(&project, track_name.as_deref()))
            .and_then(|text| {
                use std::io::Write;
                match std::io::stdout().lock().write_all(text.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
                    Err(error) => Err(daw_project::Error(format!("Cannot write tracks: {error}"))),
                }
            }),
        Command::CollectSources { project, dry_run } => collect_sources::run(&project, dry_run),
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
    settings.format = match format.unwrap_or(match extension.as_str() {
        "mp3" => "mp3",
        "aac" => "aac",
        _ => "wav",
    }) {
        "mp3" => ExportFormat::Mp3,
        "aac" => ExportFormat::Aac,
        _ => ExportFormat::Wav,
    };
    if ["wav", "mp3", "aac", "m4a", "mp4"].contains(&extension.as_str())
        && extension != settings.format.extension()
    {
        return Err(daw_project::Error(
            "Output extension does not match the export format".into(),
        ));
    }
    if settings.format != ExportFormat::Wav {
        if codec.is_some() {
            return Err(daw_project::Error(
                "--codec applies to WAV; MP3 and AAC use their own encoders".into(),
            ));
        }
        let selected = Mp3Bitrate::ALL
            .into_iter()
            .find(|value| value.kbps() == bitrate.unwrap_or(192))
            .ok_or_else(|| {
                daw_project::Error("MP3/AAC bitrate must be 128, 192, 256, or 320 kbps".into())
            })?;
        if settings.format == ExportFormat::Aac {
            settings.aac_bitrate = selected;
        } else {
            settings.mp3_bitrate = selected;
        }
    } else {
        if bitrate.is_some() {
            return Err(daw_project::Error("--bitrate applies to MP3 or AAC".into()));
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
    fn tracks_accepts_optional_name_and_project_folder() {
        let Command::Tracks {
            project,
            track_name,
        } = Args::try_parse_from(["daw-cli", "tracks"]).unwrap().command
        else {
            panic!("tracks command");
        };
        assert_eq!(project, PathBuf::from("."));
        assert_eq!(track_name, None);
        for arguments in [
            vec!["daw-cli", "tracks", "Bass synth", "--project", "song"],
            vec!["daw-cli", "tracks", "--project", "song", "Bass synth"],
        ] {
            let Command::Tracks {
                project,
                track_name,
            } = Args::try_parse_from(arguments).unwrap().command
            else {
                panic!("tracks command");
            };
            assert_eq!(project, PathBuf::from("song"));
            assert_eq!(track_name.as_deref(), Some("Bass synth"));
        }
        assert!(Args::try_parse_from(["daw-cli", "tracks", "Bass", "Synth"]).is_err());
    }

    #[test]
    fn collect_sources_accepts_default_project_and_read_only_preflight() {
        for (args, expected_project, expected_dry_run) in [
            (vec!["daw-cli", "collect-sources"], ".", false),
            (
                vec![
                    "daw-cli",
                    "collect-sources",
                    "--project",
                    "My song",
                    "--dry-run",
                ],
                "My song",
                true,
            ),
        ] {
            let Command::CollectSources { project, dry_run } =
                Args::try_parse_from(args).unwrap().command
            else {
                panic!("collect-sources command");
            };
            assert_eq!(project, PathBuf::from(expected_project));
            assert_eq!(dry_run, expected_dry_run);
        }
        assert!(Args::try_parse_from(["daw-cli", "collect-sources", "unexpected-folder"]).is_err());
    }

    #[test]
    fn export_arguments_use_matching_formats_and_reject_incompatible_options() {
        assert_eq!(
            export_settings(std::path::Path::new("mix.wav"), None, None, None).unwrap(),
            ExportSettings::default()
        );
        let mp3 = export_settings(std::path::Path::new("mix.MP3"), None, None, Some(320)).unwrap();
        assert_eq!(mp3.format, ExportFormat::Mp3);
        assert_eq!(mp3.mp3_bitrate, Mp3Bitrate::Kbps320);
        let aac = export_settings(std::path::Path::new("mix.AAC"), None, None, Some(256)).unwrap();
        assert_eq!(aac.format, ExportFormat::Aac);
        assert_eq!(aac.aac_bitrate, daw_media::AacBitrate::Kbps256);
        assert_eq!(
            export_settings(std::path::Path::new("mix.aac"), None, None, None)
                .unwrap()
                .aac_bitrate,
            daw_media::AacBitrate::Kbps192
        );
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
            ("mix.aac", Some("wav"), None, None),
            ("mix.aac", None, Some("pcm16"), None),
            ("mix.aac", None, None, Some(123)),
            ("mix.m4a", Some("aac"), None, None),
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
