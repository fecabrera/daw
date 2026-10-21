use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StretchEngineId {
    #[default]
    #[serde(rename = "rubberband-r3")]
    RubberBand,
    #[serde(rename = "timestretch")]
    Integrated,
}
fn initial_revision() -> u32 {
    1
}
/// Saved processing identity. Unsupported revisions must never silently fall back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StretchEngine {
    pub id: StretchEngineId,
    #[serde(default = "initial_revision")]
    pub revision: u32,
}
impl Default for StretchEngine {
    fn default() -> Self {
        Self::RUBBER_BAND
    }
}
impl StretchEngine {
    pub const RUBBER_BAND: Self = Self {
        id: StretchEngineId::RubberBand,
        revision: 1,
    };
    pub const INTEGRATED: Self = Self {
        id: StretchEngineId::Integrated,
        revision: 4,
    };
    pub fn label(self) -> &'static str {
        match self.id {
            StretchEngineId::RubberBand => "Rubber Band",
            StretchEngineId::Integrated => "Integrated (experimental)",
        }
    }
    pub fn validate(self) -> crate::Result<()> {
        if self != Self::RUBBER_BAND && self != Self::INTEGRATED {
            return Err(crate::Error(format!(
                "Unsupported time stretch revision {} for {}",
                self.revision,
                self.label()
            )));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_defaults_and_invalid_identity() {
        let mut saved = serde_json::to_value(crate::Project::default()).unwrap();
        saved.as_object_mut().unwrap().remove("stretch_engine");
        let legacy: crate::Project = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(legacy.stretch_engine, StretchEngine::RUBBER_BAND);
        for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
            saved["stretch_engine"] = serde_json::to_value(engine).unwrap();
            let project: crate::Project = serde_json::from_value(saved.clone()).unwrap();
            assert_eq!(project.stretch_engine, engine);
            project.validate().unwrap();
            for revision in [0, 1, 2, 3, 4, 5, u32::MAX] {
                if revision == engine.revision {
                    continue;
                }
                saved["stretch_engine"]["revision"] = serde_json::json!(revision);
                assert!(
                    serde_json::from_value::<crate::Project>(saved.clone())
                        .unwrap()
                        .validate()
                        .is_err()
                );
            }
        }
        saved["stretch_engine"]["id"] = serde_json::json!("unknown");
        assert!(serde_json::from_value::<crate::Project>(saved).is_err());
    }
}
