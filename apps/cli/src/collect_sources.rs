//! Headless collection over the same verified plan and commit boundary as desktop.
use daw_project::{
    Error, Result, Session,
    source_collection::{CollectionPreflight, CollectionStatus},
};
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    io::{self, Write},
    path::Path,
};

pub fn run(project: &Path, dry_run: bool) -> Result<()> {
    let session = Session::open(project)?;
    for warning in &session.warnings {
        warn(warning);
    }
    // The CLI owns one immutable loaded session and has no in-memory edit history.
    // External filesystem/manifest changes are rechecked by the shared executor.
    let snapshot = session.source_collection_snapshot(None, 0)?;
    let report = snapshot.preflight(None, &|| false)?;
    write_output(&summary(&session, &report))
        .map_err(|e| Error(format!("Cannot write collection preflight: {e}")))?;
    if !report.is_ready() {
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
                diagnostic.message,
                owners
            );
        }
        return Err(Error(format!(
            "Collection blocked by {} source problem(s); project unchanged. Totals cover verified sources only.",
            report.diagnostics().len()
        )));
    }
    if dry_run {
        return write_output("Dry run complete; project unchanged.\n")
            .map_err(|e| Error(format!("Cannot write collection result: {e}")));
    }
    let plan = report.into_plan()?;
    let outcome = plan.execute(
        &|snapshot| snapshot.is_current(&session, None, 0),
        &|| false,
        &mut |_| Ok(()),
    )?;
    for warning in outcome.warnings() {
        warn(warning);
    }
    if outcome.status() == CollectionStatus::RecoveryRequired {
        return Err(Error(format!(
            "Collection commit could not be confirmed. Keep all project media and candidate manifest {:?} for recovery. No collection success was confirmed.",
            outcome
                .recovery_manifest()
                .map(|path| path.as_os_str())
                .unwrap_or_default()
        )));
    }
    let progress = outcome.progress();
    // The manifest is already committed. A reporting error cannot turn that into
    // a failed collection or trigger another write/rollback attempt.
    if let Err(e) = write_output(&format!(
        "Collected files: {}\nCopied bytes: {}\nProject sources collected and saved.\n",
        progress.files_copied, progress.copied_bytes
    )) {
        warn(&format!(
            "Project sources were collected and saved, but result output failed: {e}"
        ));
    }
    Ok(())
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
