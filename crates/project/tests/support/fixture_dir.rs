use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

pub struct FixtureDir(PathBuf);

impl FixtureDir {
    pub fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", daw_core::Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self::adopt(path)
    }
    pub fn adopt(path: PathBuf) -> Self {
        Self(path)
    }
}

impl AsRef<Path> for FixtureDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Deref for FixtureDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
