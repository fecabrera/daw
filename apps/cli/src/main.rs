use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Validate DAW projects and render stereo WAV without graphics or audio devices"
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
        } => daw_project::Session::open(&project)
            .and_then(|session| {
                for warning in &session.warnings {
                    eprintln!("Warning: {warning}");
                }
                session.export(&output, overwrite).map(|warnings| {
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
            }),
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
