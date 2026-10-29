//! Lexical relative references retain Windows volume identity across canonical prefixes.
use std::path::{Component, Path, PathBuf};
fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    #[cfg(windows)]
    if let (Component::Prefix(a), Component::Prefix(b)) = (a, b) {
        use std::path::Prefix;
        return match (a.kind(), b.kind()) {
            (
                Prefix::Disk(a) | Prefix::VerbatimDisk(a),
                Prefix::Disk(b) | Prefix::VerbatimDisk(b),
            ) => a.eq_ignore_ascii_case(&b),
            (
                Prefix::UNC(a, b) | Prefix::VerbatimUNC(a, b),
                Prefix::UNC(c, d) | Prefix::VerbatimUNC(c, d),
            ) => a == c && b == d,
            _ => a == b,
        };
    }
    a == b
}
pub(crate) fn relative_path(base: &Path, path: &Path) -> Option<PathBuf> {
    let a = base.components().collect::<Vec<_>>();
    let b = path.components().collect::<Vec<_>>();
    if !a
        .first()
        .zip(b.first())
        .is_some_and(|(x, y)| same_component(*x, *y))
    {
        return None;
    }
    let common = a
        .iter()
        .zip(&b)
        .take_while(|(x, y)| same_component(**x, **y))
        .count();
    let mut out = PathBuf::new();
    for _ in common..a.len() {
        out.push("..");
    }
    for c in &b[common..] {
        out.push(c.as_os_str());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn relative_paths_preserve_siblings_and_ancestor_components() {
        assert_eq!(
            relative_path(Path::new("/project/save"), Path::new("/project/audio.wav")),
            Some(PathBuf::from("../audio.wav"))
        );
        assert_eq!(
            relative_path(Path::new("/project"), Path::new("/project")),
            Some(PathBuf::new())
        );
    }
    #[cfg(windows)]
    #[test]
    fn ordinary_and_verbatim_drive_prefixes_share_one_volume() {
        for (base, source) in [
            (r"\\?\C:\project\save", r"C:\project\source.wav"),
            (r"C:\project\save", r"\\?\c:\project\source.wav"),
        ] {
            assert_eq!(
                relative_path(Path::new(base), Path::new(source)),
                Some(PathBuf::from(r"..\source.wav"))
            );
        }
        assert!(relative_path(Path::new(r"\\?\C:\project"), Path::new(r"D:\source.wav")).is_none());
    }
    #[cfg(windows)]
    #[test]
    fn ordinary_and_verbatim_unc_prefixes_share_one_volume() {
        assert_eq!(
            relative_path(
                Path::new(r"\\?\UNC\server\share\project"),
                Path::new(r"\\server\share\source.wav")
            ),
            Some(PathBuf::from(r"..\source.wav"))
        );
        assert!(
            relative_path(
                Path::new(r"\\?\UNC\server\share\project"),
                Path::new(r"\\server\other\source.wav")
            )
            .is_none()
        );
    }
}
