use daw_plugin_host::{
    PluginRegistry, RegistryLock, ScanState, configured_search_roots, default_registry_path,
    discover, scan_isolated,
};
use std::{io::Write, path::PathBuf, time::Duration};

#[derive(clap::Args)]
pub struct Args {
    /// Read only the saved catalog; do not search directories or load plugin code.
    #[arg(long, conflicts_with_all = ["retry_failed", "rescan", "clear_cache"])]
    cached: bool,
    /// Rescan unchanged modules, including previously failed modules.
    #[arg(long)]
    rescan: bool,
    /// Rebuild the local catalog from installed files. Project data is unaffected.
    #[arg(long)]
    clear_cache: bool,
    /// Retry unchanged modules that failed a previous scan.
    #[arg(long)]
    retry_failed: bool,
    /// Write the catalog as JSON.
    #[arg(long)]
    json: bool,
    /// Catalog cache file; defaults to the platform cache directory.
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Optional standalone scanner executable; defaults to this CLI in scanner mode.
    #[arg(long)]
    scanner: Option<PathBuf>,
}
pub fn run(args: Args) -> daw_project::Result<()> {
    let json = args.json;
    let registry = catalog(args, |message| eprintln!("Warning: {message}"))
        .map_err(|e| daw_project::Error(e.to_string()))?;
    write_legacy(&registry, json).map_err(|e| daw_project::Error(e.to_string()))
}
fn catalog(args: Args, warn: impl Fn(&str)) -> Result<PluginRegistry, Box<dyn std::error::Error>> {
    let cache = args.cache.map(Ok).unwrap_or_else(default_registry_path)?;
    let _lock = if args.cached {
        None
    } else {
        Some(RegistryLock::acquire(&cache)?)
    };
    let mut registry = match PluginRegistry::load(&cache) {
        Ok(registry) => registry,
        Err(e) if !args.cached => {
            warn(&format!("Rebuilding plugin catalog: {e}"));
            PluginRegistry::default()
        }
        Err(e) => return Err(e.into()),
    };
    if args.clear_cache {
        registry = PluginRegistry::default();
    } else if args.rescan {
        registry.request_rescan();
    }
    if !args.cached {
        let scanner = match args.scanner {
            Some(p) => p,
            None => std::env::current_exe()?,
        };
        if !scanner.is_file() {
            return Err("The scanner executable does not exist. Check --scanner.".into());
        }
        let discovery = discover(&configured_search_roots()?);
        for warning in &discovery.warnings {
            warn(warning);
        }
        let scratch = cache
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(format!("scan-{}", std::process::id()));
        let result = registry.refresh(
            &discovery,
            &cache,
            args.retry_failed || args.rescan,
            || false,
            |candidate| {
                scan_isolated(
                    &scanner,
                    candidate,
                    &scratch,
                    Duration::from_secs(10),
                    || false,
                )
            },
        );
        let _ = std::fs::remove_dir(&scratch);
        result?;
    }
    Ok(registry)
}
fn write_legacy(registry: &PluginRegistry, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let output = if json {
        format!("{}\n", serde_json::to_string_pretty(registry)?)
    } else {
        describe(registry, false)
    };
    match std::io::stdout().lock().write_all(output.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn describe(registry: &PluginRegistry, escape: bool) -> String {
    let mut output = String::new();
    let text = |value: &str| {
        if escape {
            crate::report::escaped(value)
        } else {
            value.to_owned()
        }
    };
    for entry in &registry.entries {
        if !entry.present {
            continue;
        }
        if let ScanState::Quarantined(reason) = &entry.state {
            output.push_str(&format!(
                "Unavailable\t{}\t{}\n",
                text(&entry.candidate.path.to_string_lossy()),
                text(reason)
            ));
            continue;
        }
        if entry.state != ScanState::Ready {
            output.push_str(&format!(
                "Pending\t{}\n",
                text(&entry.candidate.path.to_string_lossy())
            ));
            continue;
        }
        for plugin in &entry.plugins {
            let active = registry
                .resolve(&plugin.identity)
                .is_some_and(|(c, _)| c.path == entry.candidate.path);
            output.push_str(&format!(
                "{}\t{:?}\t{}\t{}\t{}\n",
                plugin.identity.format().extension().to_ascii_uppercase(),
                plugin.role,
                text(&plugin.name),
                if active { "available" } else { "shadowed" },
                text(&entry.candidate.path.to_string_lossy())
            ));
        }
    }
    if output.is_empty() {
        output.push_str("No scanned VST3 or CLAP plugins.\n");
    }
    output
}

#[derive(Clone, Copy, Default, clap::ValueEnum)]
pub(super) enum ScanMode {
    #[default]
    Changed,
    RetryFailed,
    Rescan,
    Rebuild,
}

pub(super) fn namespaced_catalog(
    cache: Option<PathBuf>,
    scanner: Option<PathBuf>,
    mode: Option<ScanMode>,
) -> daw_project::Result<crate::report::Outcome> {
    use serde_json::json;
    let registry = catalog(
        Args {
            cached: mode.is_none(),
            rescan: matches!(mode, Some(ScanMode::Rescan)),
            clear_cache: matches!(mode, Some(ScanMode::Rebuild)),
            retry_failed: matches!(mode, Some(ScanMode::RetryFailed)),
            json: false,
            cache,
            scanner,
        },
        crate::report::warning,
    )
    .map_err(|e| daw_project::Error(e.to_string()))?;
    let modules: Vec<_> = registry.entries.iter().filter(|entry| entry.present).map(|entry| {
        let plugins: Vec<_> = entry.plugins.iter().map(|plugin| json!({
            "key": crate::expansion::identity_key(&plugin.identity), "identity": plugin.identity, "name": plugin.name, "vendor": plugin.vendor,
            "version": plugin.version, "role": plugin.role,
            "available": registry.resolve(&plugin.identity)
                .is_some_and(|(candidate, _)| candidate.path == entry.candidate.path)
        })).collect();
        let (status, diagnostic) = match &entry.state {
            ScanState::Ready => ("ready", None),
            ScanState::Pending => ("pending", None),
            ScanState::Scanning => ("scanning", None),
            ScanState::Quarantined(message) => ("quarantined", Some(message)),
        };
        json!({"path": entry.candidate.path.to_string_lossy(), "format": entry.candidate.format.extension(),
            "status": status, "diagnostic": diagnostic, "plugins": plugins})
    }).collect();
    let result = json!({"modules": modules});
    let text = describe(&registry, true);
    Ok(if mode.is_some() {
        crate::report::Outcome::committed(text, result)
    } else {
        crate::report::Outcome::read(text, result)
    })
}

pub fn render(
    session: daw_project::Session,
    output: &std::path::Path,
    overwrite: bool,
    settings: daw_media::ExportSettings,
    allow_missing: bool,
    warn: impl Fn(&str),
) -> daw_project::Result<Vec<String>> {
    render_hosted(session, allow_missing, warn, |session| {
        session.export_with_settings(output, overwrite, settings)
    })
}

pub(super) fn render_hosted<T: Send>(
    session: daw_project::Session,
    allow_missing: bool,
    warn: impl Fn(&str),
    task: impl FnOnce(daw_project::Session) -> daw_project::Result<T> + Send,
) -> daw_project::Result<T> {
    if session.project.hosted_units().is_empty() && !session.project.has_unsupported_hosted() {
        return task(session);
    }
    let scanner = std::env::current_exe().map_err(|e| daw_project::Error(e.to_string()))?;
    let (registry, warnings) = daw_plugin_host::refresh_default_registry(&scanner, || false)
        .map_err(|e| daw_project::Error(e.to_string()))?;
    for warning in warnings {
        warn(&warning);
    }
    if session.project.has_unsupported_hosted() {
        if !allow_missing {
            return Err(daw_project::Error(
                "The project contains an unsupported saved plugin version".into(),
            ));
        }
        warn(
            "unsupported saved plugin units use dry effects or silent sources in this degraded render",
        );
    }
    let missing: Vec<_> = session
        .project
        .hosted_units()
        .into_iter()
        .filter(|(_, s)| registry.resolve(&s.plugin.identity).is_none())
        .map(|(_, s)| s.plugin.identity.clone())
        .collect();
    if !allow_missing && !missing.is_empty() {
        return Err(daw_project::Error("Required plugins are missing or quarantined; use --allow-missing-plugins only for an explicit degraded render".into()));
    }
    let (mut service, client) = daw_plugin_host::HostService::new(registry);
    let client = if allow_missing {
        for (_, unit) in session
            .project
            .hosted_units()
            .into_iter()
            .filter(|(_, s)| missing.contains(&s.plugin.identity))
        {
            warn(&format!(
                "degraded render of {} ({:?}): delayed dry effect or silent instrument",
                unit.plugin.name, unit.id
            ));
        }
        std::sync::Arc::new(client.allow_missing(missing))
    } else {
        client
    };
    let mut session = session;
    session.plugin_host = Some(client);
    service.run(move || task(session))
}
