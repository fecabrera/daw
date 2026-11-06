use serde::{Deserialize, Serialize};

/// Tempo remains quarter notes per minute. The denominator defines one meter beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u16,
    pub denominator: u16,
}
impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}
impl TimeSignature {
    pub fn validate(self) -> crate::Result<()> {
        if self.numerator == 0 || self.denominator == 0 {
            Err(crate::Error(
                "Time signature numbers must be between 1 and 65535".into(),
            ))
        } else {
            Ok(())
        }
    }
    pub fn frames_per_beat(self, sample_rate_hz: u32, tempo_bpm: f64) -> f64 {
        f64::from(sample_rate_hz) * 60.0 / tempo_bpm * 4.0 / f64::from(self.denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edit, Project};

    #[test]
    fn signature_round_trip_legacy_default_and_transaction() {
        let mut project = Project::default();
        project.transport.playhead_frame = 12345;
        project.transport.r#loop.start_frame = 33;
        project.transport.r#loop.end_frame = 23456;
        let before = project.clone();
        let signature = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        project.edit(Edit::SetTimeSignature { signature }).unwrap();
        let mut expected = before.clone();
        expected.time_signature = signature;
        assert_eq!(project, expected);
        let json = serde_json::to_string(&project).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), project);
        let mut legacy = serde_json::to_value(&project).unwrap();
        legacy.as_object_mut().unwrap().remove("time_signature");
        assert_eq!(
            serde_json::from_value::<Project>(legacy)
                .unwrap()
                .time_signature,
            TimeSignature::default()
        );
        for signature in [
            TimeSignature {
                numerator: 0,
                denominator: 4,
            },
            TimeSignature {
                numerator: 4,
                denominator: 0,
            },
        ] {
            assert!(project.edit(Edit::SetTimeSignature { signature }).is_err());
            assert_eq!(project, expected);
        }
        assert_eq!(signature.frames_per_beat(48_000, 120.0), 12_000.0);
    }
}
