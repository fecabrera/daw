//! Literal dotted Clap leaves and namespace discovery from the same registry.
use crate::{MixOptions, collect_sources, expansion, plugins, report, tracks};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, error::ErrorKind};
use daw_project::{Error, Result};
use serde_json::json;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "daw-cli",
    version,
    about = "Work with saved DAW projects and local plugin catalogs"
)]
struct Args {
    /// Saved project folder. Required before a project command.
    #[arg(long, value_name = "FOLDER")]
    project: Option<PathBuf>,
    /// Write one versioned JSON result. Place this option before the command.
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List saved tracks in project order without preparing audio.
    #[command(name = "track.list")]
    TrackList {
        /// Filter by exact case-sensitive name. Include all matches.
        #[arg(long)]
        name: Option<String>,
    },
    /// Describe one saved track and its clip ranges in processing frames.
    #[command(name = "track.show")]
    TrackShow {
        /// Select one track by id:UUID or name:EXACT.
        #[arg(long, value_parser = tracks::parse_selector)]
        track: tracks::Selector,
    },
    /// Append and save one empty root track.
    #[command(name = "track.add")]
    TrackAdd {
        /// Type of the new track.
        #[arg(long = "type", value_enum)]
        track_type: tracks::AddType,
        /// Name of the new track. Duplicate names are allowed.
        #[arg(long, value_parser = tracks::parse_name)]
        name: String,
    },
    /// Rename and save one track. Its ID, clips and routing stay intact.
    #[command(name = "track.rename")]
    TrackRename {
        /// Select one track by id:UUID or name:EXACT.
        #[arg(long, value_parser = tracks::parse_selector)]
        track: tracks::Selector,
        /// New track name. Duplicate names are allowed.
        #[arg(long, value_parser = tracks::parse_name)]
        name: String,
    },
    /// Validate a saved project and prepare its referenced audio.
    #[command(name = "project.validate")]
    ProjectValidate,
    /// Bundle complete original sources and save the resulting references.
    #[command(name = "project.source.collect")]
    SourceCollect {
        /// Verify and report copy totals in bytes without copying or saving.
        #[arg(long)]
        dry_run: bool,
    },
    /// Render a saved stereo mix to one file.
    #[command(
        name = "project.export.mix",
        long_about = "Render a saved stereo mix to one file. Output is required. Format is inferred from the extension, with WAV as the fallback. WAV defaults to PCM24 at the project rate. MP3 and AAC default to 192 kbps and accept 128, 192, 256 or 320 kbps."
    )]
    ExportMix(MixOptions),
    /// Read the local plugin catalog without scanning or loading modules.
    #[command(name = "plugin.catalog.list")]
    CatalogList {
        /// Catalog file. Defaults to the platform cache.
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Refresh the local VST3/CLAP catalog with isolated scans.
    #[command(name = "plugin.catalog.scan")]
    CatalogScan {
        /// Scan changed modules, retry failures, force rescans or rebuild the catalog.
        #[arg(long, value_enum, default_value = "changed")]
        mode: plugins::ScanMode,
        /// Catalog file. Defaults to the platform cache.
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Scanner executable. Defaults to this executable's embedded scanner.
        #[arg(long)]
        scanner: Option<PathBuf>,
    },
}

const LEGACY: [&str; 5] = ["tracks", "validate", "render", "collect-sources", "plugins"];

fn token(args: &[OsString], index: usize) -> Option<&str> {
    args.get(index).and_then(|value| value.to_str())
}

pub(super) fn is_legacy(args: &[OsString]) -> bool {
    let first = token(args, 0);
    let target = if first == Some("help") {
        token(args, 1)
    } else {
        first
    };
    target.is_some_and(|name| LEGACY.contains(&name))
}

/// Inspect only recognized root options to identify the route. Clap owns
/// parsing the full invocation, including options, duplicates and operands.
fn path_position(args: &[OsString]) -> usize {
    let mut index = 0;
    while let Some(arg) = token(args, index) {
        match arg {
            "--project" => index += 2,
            "--json" => index += 1,
            _ if arg.starts_with("--project=") => index += 1,
            _ => return index,
        }
    }
    index
}

fn children<'a>(
    paths: impl IntoIterator<Item = (&'a str, String)>,
    prefix: &str,
) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}.")
    };
    for (path, about) in paths {
        if let Some(rest) = path.strip_prefix(&prefix) {
            if let Some((family, _)) = rest.split_once('.') {
                result.insert(format!("{prefix}{family}"), "Namespace".into());
            } else {
                result.insert(path.into(), about);
            }
        }
    }
    result
}

fn paths(registry: &clap::Command) -> impl Iterator<Item = (&str, String)> {
    registry
        .get_subcommands()
        .filter(|command| command.get_name().contains('.'))
        .map(|command| {
            let about = command
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default();
            (command.get_name(), about)
        })
}

fn discovery(registry: &clap::Command, prefix: &str) -> Result<()> {
    let tree = children(paths(registry), prefix);
    let mut stdout = io::stdout().lock();
    let written = if prefix.is_empty() {
        writeln!(stdout, "Work with saved DAW projects and local plugin catalogs.\n\nUsage: daw-cli [--project FOLDER] [--json] COMMAND [OPTIONS]\n\nNamespaces:")
    } else {
        writeln!(stdout, "Commands under {prefix}:")
    }.and_then(|()| {
        for (child, about) in tree {
            writeln!(stdout, "  {child:<26} {about}")?;
        }
        if prefix.is_empty() {
            writeln!(stdout, "\nLegacy commands: {}\nUse COMMAND --help for details, or --version for the version.", LEGACY.join(", "))?;
        }
        Ok(())
    });
    match written {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(|e| Error(e.to_string())),
    }
}

pub(super) fn run(argv: &[OsString]) -> Result<()> {
    let args = &argv[1..];
    let registry = registry().mut_subcommands(|command| {
        let notes = if command.get_name().starts_with("plugin.catalog.") || expansion::route(command.get_name()).is_some_and(expansion::Route::is_local) {
            "Machine-local command. Place --json before the command to request JSON."
        } else {
            "Place --project FOLDER before the command. Use --project . for the current folder. Place --json before the command to request JSON."
        };
        command.after_help(notes)
    });
    if args.is_empty()
        || matches!(args, [arg] if ["--help", "-h", "help"].iter().any(|flag| arg == *flag))
        || matches!(args, [help, flag] if help == "help" && (flag == "--help" || flag == "-h"))
    {
        return discovery(&registry, "");
    }
    let index = path_position(args);
    let path = token(args, index);
    let help_alias = path == Some("help") && index == 0 && args.len() == 2;
    if let Some(target) = token(args, index + usize::from(help_alias)) {
        let prefix = !children(paths(&registry), target).is_empty();
        if prefix {
            let tail = &args[index + usize::from(help_alias) + 1..];
            if index != 0
                || !(tail.is_empty() || matches!(tail, [flag] if flag == "--help" || flag == "-h"))
            {
                usage(
                    "A namespace accepts only --help. Select a complete command path to run a command.",
                );
            }
            return discovery(&registry, target);
        }
        if index != 0 && LEGACY.contains(&target) {
            usage(
                "Use the legacy command first and place its options after it, or use a dotted command.",
            );
        }
    }
    let matches = registry
        .try_get_matches_from(argv)
        .unwrap_or_else(|error| error.exit());
    let path = matches.subcommand_name().expect("Clap requires a command");
    if let Some(route) = expansion::route(path) {
        let project = matches.get_one::<PathBuf>("project").map(PathBuf::as_path);
        match (route.is_local(), project) {
            (true, Some(_)) => usage("This command does not take --project"),
            (false, None) => {
                usage("This command requires --project FOLDER before the command path")
            }
            _ => {}
        }
        let leaf = matches.subcommand_matches(path).expect("selected command");
        return route
            .execute(leaf, project)?
            .write(path, matches.get_flag("json"));
    }
    let parsed = Args::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let folder = match (&parsed.command, parsed.project.as_deref()) {
        (Command::CatalogList { .. } | Command::CatalogScan { .. }, Some(_)) => {
            usage("Plugin catalog commands do not take --project")
        }
        (Command::CatalogList { .. } | Command::CatalogScan { .. }, None) => None,
        (_, Some(folder)) => Some(folder),
        (_, None) => usage("This command requires --project FOLDER before the command path"),
    };
    let export_settings = match &parsed.command {
        Command::ExportMix(options) => Some(options.settings()),
        _ => None,
    };
    crate::initialize(matches!(
        parsed.command,
        Command::ProjectValidate | Command::SourceCollect { .. } | Command::ExportMix(_)
    ));
    execute(parsed.command, folder, export_settings)?.write(path, parsed.json)
}

fn registry() -> clap::Command {
    expansion::register(Args::command())
}

pub(super) fn usage(message: &str) -> ! {
    clap::Error::raw(ErrorKind::InvalidValue, message).exit()
}

fn execute(
    command: Command,
    folder: Option<&Path>,
    settings: Option<daw_media::ExportSettings>,
) -> Result<report::Outcome> {
    let project = || folder.expect("Project context validated before dispatch");
    match command {
        Command::TrackList { name } => tracks::list(project(), name.as_deref()),
        Command::TrackShow { track } => tracks::show(project(), &track),
        Command::TrackAdd { track_type, name } => {
            let (id, warnings) = tracks::create(project(), &name, track_type)?;
            for warning in warnings {
                report::warning(&warning);
            }
            Ok(report::Outcome::committed(
                format!("Added track: {name:?}\nID: {id}\n"),
                json!({"id": id, "name": name, "kind": track_type.as_str(), "changed": true}),
            ))
        }
        Command::TrackRename { track, name } => tracks::rename(project(), &track, &name),
        Command::ProjectValidate => {
            let session = daw_project::Session::open(project())?;
            for warning in &session.warnings {
                report::warning(warning);
            }
            Ok(report::Outcome::read(
                format!(
                    "Valid project: {:?} ({} tracks)\n",
                    session.project.name,
                    session.project.tracks.len()
                ),
                json!({"project_id": session.project.project_id, "name": session.project.name,
                    "track_count": session.project.tracks.len(), "warnings": session.warnings}),
            ))
        }
        Command::SourceCollect { dry_run } => collect_sources::collect(project(), dry_run, false),
        Command::ExportMix(options) => {
            let settings = settings.expect("Export options validated before dispatch");
            options.export(project(), settings, report::warning)?;
            Ok(report::Outcome::committed(
                format!("Exported {:?}\n", options.output),
                json!({"output": options.output.to_string_lossy(), "format": settings.format.extension()}),
            ))
        }
        Command::CatalogList { cache } => plugins::namespaced_catalog(cache, None, None),
        Command::CatalogScan {
            mode,
            cache,
            scanner,
        } => plugins::namespaced_catalog(cache, scanner, Some(mode)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_registry_has_valid_non_conflicting_leaf_paths() {
        let registry = registry();
        let leaves: Vec<_> = paths(&registry).map(|(name, _)| name).collect();
        assert!(!leaves.is_empty());
        for leaf in &leaves {
            assert!(!LEGACY.contains(leaf));
            let help = registry
                .clone()
                .try_get_matches_from(["daw-cli", leaf, "--help"]);
            assert!(matches!(help, Err(error) if error.kind() == ErrorKind::DisplayHelp));
            assert!(leaf.split('.').all(|segment| {
                segment
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_lowercase)
                    && segment.split('-').all(|word| {
                        !word.is_empty()
                            && word
                                .bytes()
                                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                    })
            }));
            assert!(children(paths(&registry), leaf).is_empty());
            assert_eq!(leaves.iter().filter(|other| *other == leaf).count(), 1);
        }
        let deep = [(
            "instrument.layer.effect.chain.list",
            String::from("List effects"),
        )];
        assert!(
            children(deep.clone(), "instrument.layer.effect")
                .contains_key("instrument.layer.effect.chain")
        );
        assert_eq!(
            children(deep, "instrument.layer.effect.chain")["instrument.layer.effect.chain.list"],
            "List effects"
        );
    }
}
