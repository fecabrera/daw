//! Headless collection over the same verified plan and commit boundary as desktop.
use daw_project::{
    Error, ProjectWriteGuard, Result, Session,
    source_collection::{CollectionPreflight, CollectionStatus},
};
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    io::{self, Write},
    path::Path,
};

pub fn run(project: &Path, dry_run: bool) -> Result<()> {
    collect(project, dry_run, true).map(|_| ())
}

pub(super) fn collect(
    project: &Path,
    dry_run: bool,
    legacy: bool,
) -> Result<crate::report::Outcome> {
    use serde_json::json;
    let prepare = |folder: &Path| {
        let session = Session::open(folder)?;
        let report = session
            .source_collection_snapshot(None, 0)?
            .preflight(None, &|| false)?;
        Ok::<_, Error>((session, report))
    };
    let (mut session, mut report) = prepare(project)?;
    let guard = if dry_run || !report.is_ready() {
        None
    } else {
        let guard = ProjectWriteGuard::acquire(project)?;
        (session, report) = prepare(guard.folder())?;
        Some(guard)
    };
    let warn = |message: &str| {
        if legacy {
            warn(message);
        } else {
            crate::report::warning(message);
        }
    };
    for warning in &session.warnings {
        warn(warning);
    }
    let preflight = summary(&session, &report);
    if legacy {
        write_output(&preflight)
            .map_err(|e| Error(format!("Cannot write collection preflight: {e}")))?;
    }
    if !report.is_ready() {
        if !legacy {
            let _ = write!(io::stderr().lock(), "{preflight}");
        }
        for diagnostic in report.diagnostics() {
            let owners = diagnostic
                .owners
                .iter()
                .map(|&index| {
                    let owner = &report.snapshot().owners()[index];
                    format!("{:?} ({})", owner.asset_name, owner.asset_id)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                io::stderr().lock(),
                "Source problem: {:?}: {}\nAssets: {}",
                diagnostic.path,
                if legacy {
                    diagnostic.message.clone()
                } else {
                    crate::report::escaped(&diagnostic.message)
                },
                owners
            );
        }
        return Err(Error(format!(
            "Collection blocked by {} source problem(s); project unchanged. Totals cover verified sources only.",
            report.diagnostics().len()
        )));
    }
    let stats = report.summary();
    let result = json!({
        "dry_run": dry_run,
        "files_to_copy": stats.files_to_copy,
        "files_already_included": stats.files_already_included,
        "additional_bytes": stats.additional_bytes,
    });
    if dry_run {
        let complete = "Dry run complete; project unchanged.\n";
        if legacy {
            write_output(complete)
                .map_err(|e| Error(format!("Cannot write collection result: {e}")))?;
        }
        return Ok(crate::report::Outcome::read(preflight + complete, result));
    }
    let plan = report.into_plan()?;
    let outcome = plan.execute(
        &|snapshot| {
            guard.as_ref().is_some_and(|guard| guard.check().is_ok())
                && snapshot.is_current(&session, None, 0)
        },
        &|| false,
        &mut |_| Ok(()),
    )?;
    for warning in outcome.warnings() {
        warn(warning);
    }
    if outcome.status() == CollectionStatus::RecoveryRequired {
        return Err(Error(format!(
            "uncertain-commit: Collection commit could not be confirmed. Keep all project media and candidate manifest {:?} for recovery. No collection success was confirmed.",
            outcome
                .recovery_manifest()
                .map(|path| path.as_os_str())
                .unwrap_or_default()
        )));
    }
    let progress = outcome.progress();
    // The manifest is already committed. A reporting error cannot turn that into
    // a failed collection or trigger another write/rollback attempt.
    let complete = format!(
        "Collected files: {}\nCopied bytes: {}\nProject sources collected and saved.\n",
        progress.files_copied, progress.copied_bytes
    );
    if legacy && let Err(e) = write_output(&complete) {
        warn(&format!(
            "Project sources were collected and saved, but result output failed: {e}"
        ));
    }
    let mut result = result;
    result["files_copied"] = json!(progress.files_copied);
    result["copied_bytes"] = json!(progress.copied_bytes);
    result["committed"] = json!(true);
    Ok(crate::report::Outcome::committed(
        preflight + &complete,
        result,
    ))
}
fn summary(session: &Session, report: &CollectionPreflight) -> String {
    let s = report.summary();
    let mut text = String::new();
    // Formatting into a String has no I/O failure path.
    let _ = writeln!(text, "Project: {:?}", session.project.name);
    if let Some(folder) = &session.folder {
        let _ = writeln!(text, "Folder: {folder:?}");
    }
    let _ = writeln!(
        text,
        "Files to copy: {}\nAlready included: {}\nAdditional bytes: {}",
        s.files_to_copy, s.files_already_included, s.additional_bytes
    );
    text.push_str("Temporary space is also required; free space was not measured.\n");
    text
}
fn write_output(text: &str) -> io::Result<()> {
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}
fn warn(message: &str) {
    let _ = writeln!(io::stderr().lock(), "Warning: {message}");
}
