use super::*;

const BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn stream(
    input: &Path,
    index: usize,
    length: u64,
    staging: bool,
    store: &mut Store,
    control: &mut Control<'_>,
) -> Result<()> {
    control.check_current()?;
    store.check_file(index)?;
    if !fs::metadata(input).map_err(error)?.is_file() {
        return Err(error("Copy source must be a regular file"));
    }
    let mut source = fs::File::open(input).map_err(error)?;
    let before = source.metadata().map_err(error)?;
    if before.len() != length {
        return Err(error("Source size changed before copying"));
    }
    let stamp = (before.len(), before.modified().map_err(error)?);
    let output = store.files[index].path.clone();
    let mut buffer = [0_u8; BUFFER_BYTES];
    let mut copied = 0_u64;
    while copied < length {
        control.check_cancel()?;
        let limit = (length - copied).min(BUFFER_BYTES as u64) as usize;
        (control.hook)(IoStep::Read, input).map_err(error)?;
        let count = source.read(&mut buffer[..limit]).map_err(error)?;
        control.check_cancel()?;
        if count == 0 {
            return Err(error("Source was truncated while copying"));
        }
        (control.hook)(IoStep::Write, &output).map_err(error)?;
        store.files[index]
            .file
            .write_all(&buffer[..count])
            .map_err(error)?;
        control.check_cancel()?;
        copied = copied
            .checked_add(count as u64)
            .ok_or_else(|| error("Copied byte count overflow"))?;
        let total = if staging {
            &mut control.state.staged_bytes
        } else {
            &mut control.state.copied_bytes
        };
        *total = total
            .checked_add(count as u64)
            .ok_or_else(|| error("Collection byte progress overflow"))?;
        control.notify(CollectionPhase::Staging)?;
    }
    control.check_cancel()?;
    (control.hook)(IoStep::Read, input).map_err(error)?;
    if source.read(&mut buffer[..1]).map_err(error)? != 0 {
        return Err(error("Source grew while copying"));
    }
    let after = source.metadata().map_err(error)?;
    if stamp != (after.len(), after.modified().map_err(error)?) {
        return Err(error("Source changed while copying"));
    }
    finish_file(index, store, control)?;
    control.check_current()?;
    Ok(())
}

fn finish_file(index: usize, store: &mut Store, control: &mut Control<'_>) -> Result<()> {
    control.check_cancel()?;
    store.check_file(index)?;
    let path = store.files[index].path.clone();
    (control.hook)(IoStep::Flush, &path).map_err(error)?;
    store.files[index].file.flush().map_err(error)?;
    control.check_cancel()?;
    (control.hook)(IoStep::FileSync, &path).map_err(error)?;
    store.files[index].file.sync_all().map_err(error)?;
    control.check_cancel()?;
    Ok(())
}

pub(super) fn write_manifest(
    bytes: &[u8],
    index: usize,
    store: &mut Store,
    control: &mut Control<'_>,
) -> Result<()> {
    let path = store.files[index].path.clone();
    for chunk in bytes.chunks(BUFFER_BYTES) {
        control.check_cancel()?;
        (control.hook)(IoStep::Write, &path).map_err(error)?;
        store.files[index].file.write_all(chunk).map_err(error)?;
        control.check_cancel()?;
    }
    finish_file(index, store, control)
}

fn directory(store: &mut Store, control: &mut Control<'_>, preferred: &Path) -> Result<PathBuf> {
    let mut relative = preferred.to_owned();
    loop {
        control.check_current()?;
        if store.reserve_directory(&relative, false, control.hook)? {
            return Ok(relative);
        }
        relative = PathBuf::from(format!("stems/{}", Id::new_v4()));
    }
}

pub(super) fn reserve_target(
    target: &CollectionTarget,
    store: &mut Store,
    control: &mut Control<'_>,
    groups: &mut BTreeMap<String, PathBuf>,
    names: &mut BTreeSet<String>,
) -> Result<(String, usize)> {
    let proposal = Path::new(&target.relative_path);
    let mut relative = if target.kind == CollectionTargetKind::GeneratedStem {
        let parent = proposal
            .parent()
            .ok_or_else(|| error("Missing stem group folder"))?;
        let key = parent.to_string_lossy().into_owned();
        let actual = if let Some(path) = groups.get(&key) {
            path.clone()
        } else {
            let path = directory(store, control, parent)?;
            groups.insert(key, path.clone());
            path
        };
        actual.join(
            proposal
                .file_name()
                .ok_or_else(|| error("Missing stem filename"))?,
        )
    } else {
        proposal.to_owned()
    };
    loop {
        control.check_current()?;
        let name = paths::relative(&store.root, &store.root.join(&relative))?;
        if !names.contains(&name.to_lowercase())
            && let Some(index) = store.reserve_file(&relative, false, control.hook)?
        {
            names.insert(name.to_lowercase());
            return Ok((name, index));
        }
        relative = match target.kind {
            CollectionTargetKind::IncludedRecording => PathBuf::from(format!(
                "assets/sources/{}.{}",
                Id::new_v4(),
                proposal
                    .extension()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| error("Missing source extension"))?
            )),
            CollectionTargetKind::GeneratedStem => directory(
                store,
                control,
                &PathBuf::from(format!("stems/{}", Id::new_v4())),
            )?
            .join(
                proposal
                    .file_name()
                    .ok_or_else(|| error("Missing stem filename"))?,
            ),
        };
    }
}
