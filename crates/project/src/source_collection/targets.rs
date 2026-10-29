use super::preflight::{cancelled, issue};
use super::*;
use std::{collections::BTreeMap, fs};

pub(super) struct TargetPlanner<'a> {
    pub root: &'a Path,
    pub snapshot: &'a SourceCollectionSnapshot,
    pub tree: paths::DestinationTree,
    stem_directories: BTreeMap<PathBuf, Id>,
    destinations: BTreeMap<String, (PathBuf, Vec<usize>)>,
}
impl<'a> TargetPlanner<'a> {
    pub fn new(root: &'a Path, snapshot: &'a SourceCollectionSnapshot) -> Self {
        Self {
            root,
            snapshot,
            tree: Default::default(),
            stem_directories: BTreeMap::new(),
            destinations: BTreeMap::new(),
        }
    }
    pub fn plan(
        &mut self,
        path: &Path,
        owners: &[usize],
        extension: &str,
        diagnostics: &mut Vec<CollectionDiagnostic>,
        check: &impl Fn() -> bool,
    ) -> Result<Vec<CollectionTarget>> {
        let mut targets = Vec::new();
        for kind in [
            CollectionTargetKind::IncludedRecording,
            CollectionTargetKind::GeneratedStem,
        ] {
            let members = owners
                .iter()
                .copied()
                .filter(|&i| {
                    (self.snapshot.owners[i].source.kind == "generated")
                        == (kind == CollectionTargetKind::GeneratedStem)
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let first = &self.snapshot.owners[members[0]];
            let included = match kind {
                CollectionTargetKind::IncludedRecording => {
                    members
                        .iter()
                        .any(|&i| self.snapshot.owners[i].source.is_included_recording())
                        && path.starts_with(self.root.join("assets/sources"))
                }
                CollectionTargetKind::GeneratedStem => path.starts_with(self.root.join("stems")),
            };
            let relative_path = if included {
                match paths::relative(self.root, path) {
                    Ok(relative) => relative,
                    Err(e) => {
                        issue(
                            diagnostics,
                            SourceIssue::UnsafeDestination,
                            path,
                            &members,
                            e,
                        );
                        continue;
                    }
                }
            } else {
                match kind {
                    CollectionTargetKind::IncludedRecording => {
                        format!("assets/sources/{}.{}", first.proposal, extension)
                    }
                    CollectionTargetKind::GeneratedStem => {
                        let parent = path
                            .parent()
                            .ok_or_else(|| error("Source folder is missing"))?
                            .to_path_buf();
                        let group = self
                            .stem_directories
                            .entry(parent)
                            .or_insert(first.proposal);
                        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                            issue(
                                diagnostics,
                                SourceIssue::UnsafeDestination,
                                path,
                                &members,
                                "Stem filename is not UTF-8",
                            );
                            continue;
                        };
                        let preferred = format!("stems/{group}/{name}");
                        // Distinct case-sensitive source names must remain distinct on Windows.
                        if self.destinations.contains_key(&preferred.to_lowercase()) {
                            format!("stems/{}/{name}", first.proposal)
                        } else {
                            preferred
                        }
                    }
                }
            };
            let target = self.root.join(&relative_path);
            if let Err(e) = self
                .tree
                .check(self.root, Path::new(&relative_path), true, check)
            {
                cancelled(check)?;
                issue(
                    diagnostics,
                    SourceIssue::UnsafeDestination,
                    &target,
                    &members,
                    e,
                );
                continue;
            }
            if !included && fs::symlink_metadata(&target).is_ok() {
                issue(
                    diagnostics,
                    SourceIssue::DestinationOccupied,
                    &target,
                    &members,
                    "Destination proposal is occupied; capture a fresh snapshot to propose a new UUID",
                );
                continue;
            }
            let key = relative_path.to_lowercase();
            if let Some((previous, previous_owners)) = self.destinations.get(&key)
                && previous != path
            {
                let all = previous_owners
                    .iter()
                    .chain(&members)
                    .copied()
                    .collect::<Vec<_>>();
                issue(
                    diagnostics,
                    SourceIssue::UnsafeDestination,
                    &target,
                    &all,
                    "Source destinations collide on a case-insensitive filesystem",
                );
                continue;
            }
            self.destinations
                .insert(key, (path.to_path_buf(), members.clone()));
            targets.push(CollectionTarget {
                relative_path,
                kind,
                needs_copy: !included,
                owners: members,
            });
        }
        Ok(targets)
    }
}
