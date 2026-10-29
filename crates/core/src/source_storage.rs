//! Source ownership is independent of provenance and path representation.
use crate::{Error, Result, Source};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStorage {
    External,
    /// Includes collected recordings and generated stems (including temporary stems).
    Project,
    Embedded,
}

impl Source {
    /// Interpret omitted storage exactly as the legacy source lifecycle did.
    /// Validation rejects unsupported provenance/storage combinations.
    pub fn storage_kind(&self) -> SourceStorage {
        self.storage.unwrap_or(match self.kind.as_str() {
            "generated" => SourceStorage::Project,
            "midi_pattern" => SourceStorage::Embedded,
            _ => SourceStorage::External,
        })
    }

    pub fn validate_storage(&self) -> Result<()> {
        match (self.kind.as_str(), self.storage_kind()) {
            ("external", SourceStorage::External)
            | ("generated", SourceStorage::Project)
            | ("midi_pattern", SourceStorage::Embedded) => Ok(()),
            ("external", SourceStorage::Project) if self.path_kind == "relative" => {
                validate_included_source_path(&self.path)
            }
            _ => Err(Error(
                "Invalid source storage or included source path".into(),
            )),
        }
    }

    /// Generated stems retain their existing managed/temporary lifecycle.
    pub fn is_included_recording(&self) -> bool {
        self.kind == "external" && self.storage_kind() == SourceStorage::Project
    }
}

/// Lexical validation only. The project boundary also checks filesystem containment.
/// New collectors generate UUID filenames; compatible manifests may use other portable names.
pub fn validate_included_source_path(path: &str) -> Result<()> {
    let invalid = || Error("Included recordings require assets/sources/<portable filename>".into());
    let name = path.strip_prefix("assets/sources/").ok_or_else(invalid)?;
    if name.is_empty()
        || name.len() > 255
        || name.starts_with(' ')
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err(invalid());
    }
    let base = name.split('.').next().unwrap().trim_end().to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str())
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn source(kind: &str, path_kind: &str, path: &str) -> serde_json::Value {
        json!({"kind": kind, "path_kind": path_kind, "path": path})
    }

    #[test]
    fn legacy_sources_keep_ownership_and_wire_representation() {
        for (kind, path_kind, path, expected) in [
            (
                "external",
                "relative",
                "../recording.wav",
                SourceStorage::External,
            ),
            (
                "external",
                "relative",
                "assets/sources/local.wav",
                SourceStorage::External,
            ),
            (
                "generated",
                "absolute",
                "/temporary/stem.wav",
                SourceStorage::Project,
            ),
            (
                "generated",
                "relative",
                "stems/id/vocals.wav",
                SourceStorage::Project,
            ),
            ("midi_pattern", "embedded", "", SourceStorage::Embedded),
        ] {
            let fixture = source(kind, path_kind, path);
            let decoded: Source = serde_json::from_value(fixture.clone()).unwrap();
            decoded.validate_storage().unwrap();
            assert_eq!(decoded.storage_kind(), expected);
            assert_eq!(serde_json::to_value(decoded).unwrap(), fixture);
        }
    }

    #[test]
    fn included_recording_round_trip_preserves_provenance() {
        let mut fixture = source("external", "relative", "assets/sources/recording.wav");
        fixture["storage"] = json!("project");
        let decoded: Source = serde_json::from_value(fixture.clone()).unwrap();
        decoded.validate_storage().unwrap();
        assert!(decoded.is_included_recording());
        assert_eq!(decoded.kind, "external");
        assert_eq!(serde_json::to_value(decoded).unwrap(), fixture);
    }

    #[test]
    fn invalid_storage_and_escaping_paths_are_rejected() {
        for (kind, storage, path_kind, path) in [
            ("external", "embedded", "embedded", ""),
            ("generated", "external", "relative", "stems/id/stem.wav"),
            ("midi_pattern", "project", "embedded", ""),
            ("external", "project", "absolute", "/assets/sources/a.wav"),
            ("external", "project", "relative", "../assets/sources/a.wav"),
            ("external", "project", "relative", "assets/sources/../a.wav"),
            (
                "external",
                "project",
                "relative",
                "assets/sources/sub/a.wav",
            ),
            ("external", "project", "relative", "assets\\sources\\a.wav"),
        ] {
            let mut fixture = source(kind, path_kind, path);
            fixture["storage"] = json!(storage);
            let decoded: Source = serde_json::from_value(fixture).unwrap();
            assert!(decoded.validate_storage().is_err(), "{decoded:?}");
        }
        let mut unknown = source("external", "relative", "source.wav");
        unknown["storage"] = json!("unknown");
        assert!(serde_json::from_value::<Source>(unknown).is_err());
    }

    #[test]
    fn included_names_are_portable_on_all_hosts() {
        for name in [
            "",
            ".",
            "..",
            "a/b.wav",
            "a\\b.wav",
            "D:a.wav",
            "a.wav ",
            "a.",
            "CON.wav",
            "aux",
            "COM1.wav",
            "lpt³.wav",
            "a\n.wav",
            "*.wav",
        ] {
            assert!(
                validate_included_source_path(&format!("assets/sources/{name}")).is_err(),
                "{name:?}"
            );
        }
        for name in ["a.wav", "音声.wav", "CONCERT.wav", "com10.wav"] {
            validate_included_source_path(&format!("assets/sources/{name}")).unwrap();
        }
    }
}
