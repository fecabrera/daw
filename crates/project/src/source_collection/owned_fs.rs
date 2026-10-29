//! Track only entries created by this operation. Cleanup checks file identity,
//! never recurses through a directory and never removes a pre-existing entry.
use super::execute::IoStep;
use super::{Result, error, paths};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity(u64, u64);
impl Identity {
    pub fn read(file: &File) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = file.metadata()?;
            if metadata.ino() == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Filesystem has no stable file identity",
                ));
            }
            Ok(Self(metadata.dev(), metadata.ino()))
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
            };
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
            // The borrowed File owns a valid handle through the synchronous call;
            // info is writable storage for the SDK's plain output structure.
            if unsafe { GetFileInformationByHandle(file.as_raw_handle() as isize, &mut info) } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
            if index == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Filesystem has no stable file identity",
                ));
            }
            Ok(Self(u64::from(info.dwVolumeSerialNumber), index))
        }
    }
    pub fn same_volume(self, other: Self) -> bool {
        self.0 == other.0
    }
}

fn open_entry(path: &Path, directory: bool) -> io::Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if crate::file_publish::is_reparse(&metadata)
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Entry is not an ordinary file/directory",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        options.custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        );
    }
    options.open(path)
}

pub(crate) struct OwnedFile {
    pub path: PathBuf,
    pub file: File,
    identity: Identity,
    temporary: bool,
}
struct OwnedDir {
    path: PathBuf,
    _file: File,
    identity: Identity,
    temporary: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupMode {
    Rollback,
    Committed,
    KeepAll,
}

pub(crate) struct Store {
    pub root: PathBuf,
    _root_file: File,
    root_identity: Identity,
    pub files: Vec<OwnedFile>,
    dirs: Vec<OwnedDir>,
    pub mode: CleanupMode,
    finished: bool,
}
impl Store {
    pub fn new(root: &Path) -> Result<Self> {
        let root_file = open_entry(root, true).map_err(error)?;
        let root_identity = Identity::read(&root_file).map_err(error)?;
        Ok(Self {
            root: root.to_owned(),
            _root_file: root_file,
            root_identity,
            files: Vec::new(),
            dirs: Vec::new(),
            mode: CleanupMode::Rollback,
            finished: false,
        })
    }
    pub fn check_root(&self) -> Result<()> {
        if Identity::read(&open_entry(&self.root, true).map_err(error)?).map_err(error)?
            != self.root_identity
        {
            return Err(error("Project folder changed during collection"));
        }
        Ok(())
    }
    fn track_directory(&mut self, path: PathBuf, temporary: bool) -> Result<()> {
        let file = open_entry(&path, true).map_err(error)?;
        let identity = Identity::read(&file).map_err(error)?;
        if !identity.same_volume(self.root_identity) {
            return Err(error(
                "Collection directories must use the project filesystem",
            ));
        }
        self.dirs.push(OwnedDir {
            path,
            _file: file,
            identity,
            temporary,
        });
        Ok(())
    }
    pub fn ensure_directory(
        &mut self,
        relative: &Path,
        hook: &mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
    ) -> Result<()> {
        self.check_root()?;
        if relative.as_os_str().is_empty() {
            return Ok(());
        }
        paths::DestinationTree::default().check(&self.root, relative, false, &|| false)?;
        let mut path = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(error("Invalid collection directory"));
            };
            path.push(name);
            match fs::symlink_metadata(&path) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    hook(IoStep::CreateDirectory, &path).map_err(error)?;
                    match fs::create_dir(&path) {
                        Ok(()) => {
                            self.track_directory(path.clone(), false)?;
                        }
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(error(e)),
                    }
                }
                Err(e) => return Err(error(e)),
            }
            let identity =
                Identity::read(&open_entry(&path, true).map_err(error)?).map_err(error)?;
            if !identity.same_volume(self.root_identity) {
                return Err(error(
                    "Collection directories must use the project filesystem",
                ));
            }
        }
        Ok(())
    }
    /// No entry was reserved by preflight. This is the exclusive directory reservation.
    pub fn reserve_directory(
        &mut self,
        relative: &Path,
        private: bool,
        hook: &mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
    ) -> Result<bool> {
        self.ensure_directory(relative.parent().unwrap_or_else(|| Path::new("")), hook)?;
        if self.occupied(relative)? {
            return Ok(false);
        }
        let path = self.root.join(relative);
        hook(IoStep::CreateDirectory, &path).map_err(error)?;
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            if private {
                builder.mode(0o700);
            }
        }
        match builder.create(&path) {
            Ok(()) => {
                self.track_directory(path, private)?;
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(error(e)),
        }
    }
    pub fn occupied(&self, relative: &Path) -> Result<bool> {
        self.check_root()?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        if !parent.as_os_str().is_empty() {
            paths::DestinationTree::default().check(&self.root, parent, false, &|| false)?;
        }
        let name = relative
            .file_name()
            .ok_or_else(|| error("Missing collection filename"))?
            .to_string_lossy()
            .to_lowercase();
        for entry in fs::read_dir(self.root.join(parent)).map_err(error)? {
            if entry
                .map_err(error)?
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                == name
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn reserve_file(
        &mut self,
        relative: &Path,
        temporary: bool,
        hook: &mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
    ) -> Result<Option<usize>> {
        self.ensure_directory(relative.parent().unwrap_or_else(|| Path::new("")), hook)?;
        if self.occupied(relative)? {
            return Ok(None);
        }
        // Validate the leaf as well as its parents before opening it.
        paths::ordinary(&self.root, relative, true)?;
        let path = self.root.join(relative);
        hook(IoStep::ReserveFile, &path).map_err(error)?;
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                let identity = Identity::read(&file).map_err(error)?;
                if !identity.same_volume(self.root_identity) {
                    return Err(error("Collection files must use the project filesystem"));
                }
                let index = self.files.len();
                self.files.push(OwnedFile {
                    path,
                    file,
                    identity,
                    temporary,
                });
                Ok(Some(index))
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(None),
            Err(e) => Err(error(e)),
        }
    }
    pub fn check_file(&self, index: usize) -> Result<()> {
        self.check_root()?;
        let entry = &self.files[index];
        let relative = entry.path.strip_prefix(&self.root).map_err(error)?;
        paths::ordinary(&self.root, relative, true)?;
        let current =
            Identity::read(&open_entry(&entry.path, false).map_err(error)?).map_err(error)?;
        if current != entry.identity {
            return Err(error(format!(
                "Collection file was replaced: {}",
                entry.path.display()
            )));
        }
        Ok(())
    }
    pub fn sync_directories(
        &self,
        hook: &mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let mut paths = std::collections::BTreeSet::from([self.root.clone()]);
        for dir in &self.dirs {
            paths.insert(dir.path.clone());
            if let Some(parent) = dir.path.parent() {
                paths.insert(parent.to_owned());
            }
        }
        for file in &self.files {
            if let Some(parent) = file.path.parent() {
                paths.insert(parent.to_owned());
            }
        }
        for path in paths {
            hook(IoStep::DirectorySync, &path).map_err(error)?;
            #[cfg(unix)]
            {
                match open_entry(&path, true).and_then(|f| f.sync_all()) {
                    Ok(()) => {}
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput
                        ) =>
                    {
                        warnings.push(format!(
                            "Directory sync is unsupported for {}; crash durability is limited",
                            path.display()
                        ))
                    }
                    Err(e) => return Err(error(format!("{}: {e}", path.display()))),
                }
            }
            #[cfg(windows)]
            {
                let _ = path;
            }
        }
        #[cfg(windows)] warnings.push("Windows directory flush is unavailable; file sync and native write-through replacement do not guarantee power-loss durability".into());
        Ok(())
    }
    pub fn cleanup(&mut self) -> Vec<String> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        if self.mode == CleanupMode::KeepAll {
            return Vec::new();
        }
        let mut warnings = Vec::new();
        if self.check_root().is_err() {
            return vec!["Cleanup retained files because the project folder changed".into()];
        }
        for index in (0..self.files.len()).rev() {
            let entry = &self.files[index];
            if self.mode == CleanupMode::Committed && !entry.temporary {
                continue;
            }
            match fs::symlink_metadata(&entry.path) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                _ => {}
            }
            if let Err(e) = self.check_file(index) {
                warnings.push(format!("Cleanup retained {}: {e}", entry.path.display()));
                continue;
            }
            if let Err(e) = fs::remove_file(&entry.path) {
                warnings.push(format!(
                    "Cleanup could not remove {}: {e}",
                    entry.path.display()
                ));
            }
        }
        for dir in self.dirs.iter().rev() {
            if self.mode == CleanupMode::Committed && !dir.temporary {
                continue;
            }
            let relative = match dir.path.strip_prefix(&self.root) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let identity = paths::ordinary(&self.root, relative, false).and_then(|_| {
                Identity::read(&open_entry(&dir.path, true).map_err(error)?).map_err(error)
            });
            if !matches!(identity,Ok(id) if id==dir.identity) {
                warnings.push(format!(
                    "Cleanup retained changed directory {}",
                    dir.path.display()
                ));
                continue;
            }
            // Nonempty directories can contain unrelated files; never recurse.
            if let Err(e) = fs::remove_dir(&dir.path)
                && e.kind() != io::ErrorKind::DirectoryNotEmpty
            {
                warnings.push(format!(
                    "Cleanup could not remove {}: {e}",
                    dir.path.display()
                ));
            }
        }
        warnings
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
