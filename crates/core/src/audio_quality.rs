//! Concrete project audio quality, independent of audio devices and their backend.
use crate::{BUFFER_SIZES, Error, Project, Result, SAMPLE_RATES};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleFormat {
    I8,
    I16,
    I24,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    #[default]
    F32,
    F64,
}

impl SampleFormat {
    pub const ALL: [Self; 11] = [
        Self::I8,
        Self::I16,
        Self::I24,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::F32,
        Self::F64,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::I8 => "8-bit signed integer",
            Self::I16 => "16-bit signed integer",
            Self::I24 => "24-bit signed integer",
            Self::I32 => "32-bit signed integer",
            Self::I64 => "64-bit signed integer",
            Self::U8 => "8-bit unsigned integer",
            Self::U16 => "16-bit unsigned integer",
            Self::U32 => "32-bit unsigned integer",
            Self::U64 => "64-bit unsigned integer",
            Self::F32 => "32-bit float",
            Self::F64 => "64-bit float",
        }
    }
}

/// Defaults for new projects and the saved quality of an existing project.
/// Sample format requests the output stream type; internal DSP remains float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioQuality {
    pub sample_rate_hz: u32,
    pub output_sample_format: SampleFormat,
    pub buffer_size_frames: u32,
}
impl Default for AudioQuality {
    fn default() -> Self {
        Self {
            sample_rate_hz: crate::SAMPLE_RATE,
            output_sample_format: SampleFormat::F32,
            buffer_size_frames: crate::default_buffer_size(),
        }
    }
}
impl AudioQuality {
    pub fn validate(self) -> Result<()> {
        if !SAMPLE_RATES.contains(&self.sample_rate_hz)
            || !BUFFER_SIZES.contains(&self.buffer_size_frames)
        {
            return Err(Error("Unsupported project audio settings".into()));
        }
        Ok(())
    }
}
impl Project {
    pub fn new_with_audio_quality(quality: AudioQuality) -> Result<Self> {
        quality.validate()?;
        Ok(Self {
            sample_rate_hz: quality.sample_rate_hz,
            output_sample_format: quality.output_sample_format,
            buffer_size_frames: quality.buffer_size_frames,
            ..Self::default()
        })
    }
    pub fn audio_quality(&self) -> AudioQuality {
        AudioQuality {
            sample_rate_hz: self.sample_rate_hz,
            output_sample_format: self.output_sample_format,
            buffer_size_frames: self.buffer_size_frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_is_concrete_validated_and_preserved_by_project_round_trip() {
        for output_sample_format in SampleFormat::ALL {
            let quality = AudioQuality {
                sample_rate_hz: 96000,
                output_sample_format,
                buffer_size_frames: 256,
            };
            let project = Project::new_with_audio_quality(quality).unwrap();
            project.validate().unwrap();
            let loaded: Project =
                serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
            assert_eq!(loaded.audio_quality(), quality);
        }
        assert!(
            Project::new_with_audio_quality(AudioQuality {
                sample_rate_hz: 123,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            Project::new_with_audio_quality(AudioQuality {
                buffer_size_frames: 17,
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn legacy_projects_keep_processing_values_and_receive_a_stable_output_format() {
        let mut value = serde_json::to_value(Project::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("output_sample_format");
        value["sample_rate_hz"] = 44100.into();
        value["buffer_size_frames"] = 128.into();
        let project: Project = serde_json::from_value(value).unwrap();
        assert_eq!(
            project.audio_quality(),
            AudioQuality {
                sample_rate_hz: 44100,
                output_sample_format: SampleFormat::F32,
                buffer_size_frames: 128
            }
        );
    }
}
