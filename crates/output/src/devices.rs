//! Device discovery and stream preferences. Call discovery on a control/worker thread.
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// CPAL 0.16 has no portable persistent ID. The occurrence disambiguates equal names
/// within the selected direction; missing devices must never select a different name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId {
    pub name: String,
    pub occurrence: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    F32,
    F64,
}

impl SampleFormat {
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
    pub(super) fn from_cpal(format: cpal::SampleFormat) -> Option<Self> {
        Some(match format {
            cpal::SampleFormat::I8 => Self::I8,
            cpal::SampleFormat::I16 => Self::I16,
            cpal::SampleFormat::I24 => Self::I24,
            cpal::SampleFormat::I32 => Self::I32,
            cpal::SampleFormat::I64 => Self::I64,
            cpal::SampleFormat::U8 => Self::U8,
            cpal::SampleFormat::U16 => Self::U16,
            cpal::SampleFormat::U32 => Self::U32,
            cpal::SampleFormat::U64 => Self::U64,
            cpal::SampleFormat::F32 => Self::F32,
            cpal::SampleFormat::F64 => Self::F64,
            _ => return None,
        })
    }
}

/// Machine-local preferences. None retains automatic/project-controlled behavior.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub output_device: Option<DeviceId>,
    /// Reserved selection; no input stream is opened until capture is implemented.
    pub input_device: Option<DeviceId>,
    pub sample_rate_hz: Option<u32>,
    pub sample_format: Option<SampleFormat>,
    pub buffer_size_frames: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Configuration {
    pub channels: u16,
    pub format: SampleFormat,
    pub min_rate_hz: u32,
    pub max_rate_hz: u32,
    pub buffer_range: Option<(u32, u32)>,
}
impl Configuration {
    pub fn supports(&self, settings: &AudioSettings) -> bool {
        self.channels > 0
            && settings
                .sample_format
                .is_none_or(|format| format == self.format)
            && settings
                .sample_rate_hz
                .is_none_or(|rate| rate > 0 && rate >= self.min_rate_hz && rate <= self.max_rate_hz)
            && settings.buffer_size_frames.is_none_or(|size| {
                size > 0
                    && self
                        .buffer_range
                        .is_none_or(|(min, max)| size >= min && size <= max)
            })
    }
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub configurations: Vec<Configuration>,
}
#[derive(Clone, Debug, Default)]
pub struct Devices {
    pub outputs: Vec<DeviceInfo>,
    pub inputs: Vec<DeviceInfo>,
    pub default_output: Option<DeviceId>,
    pub default_input: Option<DeviceId>,
}

fn describe(config: cpal::SupportedStreamConfigRange) -> Option<Configuration> {
    Some(Configuration {
        channels: config.channels(),
        format: SampleFormat::from_cpal(config.sample_format())?,
        min_rate_hz: config.min_sample_rate().0,
        max_rate_hz: config.max_sample_rate().0,
        buffer_range: match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => Some((*min, *max)),
            cpal::SupportedBufferSize::Unknown => None,
        },
    })
}

fn device_ids(
    devices: impl Iterator<Item = cpal::Device>,
) -> Result<Vec<(DeviceId, cpal::Device)>, String> {
    let mut occurrences = std::collections::HashMap::new();
    devices
        .map(|device| {
            let name = device.name().map_err(|error| error.to_string())?;
            let occurrence = occurrences.entry(name.clone()).or_insert(0);
            let id = DeviceId {
                name,
                occurrence: *occurrence,
            };
            *occurrence += 1;
            Ok((id, device))
        })
        .collect()
}

pub fn discover_devices() -> Result<Devices, String> {
    #[cfg(target_os = "windows")]
    super::wasapi::prepare()?;
    let host = cpal::default_host();
    let default_output = host
        .default_output_device()
        .and_then(|device| device.name().ok());
    let default_input = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let mut devices = Devices::default();
    for (id, device) in device_ids(host.output_devices().map_err(|error| error.to_string())?)? {
        if devices.default_output.is_none() && Some(&id.name) == default_output.as_ref() {
            devices.default_output = Some(id.clone());
        }
        devices.outputs.push(DeviceInfo {
            id,
            configurations: device
                .supported_output_configs()
                .map_err(|error| error.to_string())?
                .filter_map(describe)
                .collect(),
        });
    }
    for (id, device) in device_ids(host.input_devices().map_err(|error| error.to_string())?)? {
        if devices.default_input.is_none() && Some(&id.name) == default_input.as_ref() {
            devices.default_input = Some(id.clone());
        }
        devices.inputs.push(DeviceInfo {
            id,
            configurations: device
                .supported_input_configs()
                .map_err(|error| error.to_string())?
                .filter_map(describe)
                .collect(),
        });
    }
    Ok(devices)
}

pub(super) fn output_device(selection: Option<&DeviceId>) -> Result<cpal::Device, String> {
    #[cfg(target_os = "windows")]
    super::wasapi::prepare()?;
    let host = cpal::default_host();
    let Some(selection) = selection else {
        return host
            .default_output_device()
            .ok_or("No default audio output device".into());
    };
    device_ids(host.output_devices().map_err(|error| error.to_string())?)?
        .into_iter()
        .find_map(|(id, device)| (id == *selection).then_some(device))
        .ok_or_else(|| {
            format!(
                "Output device '{}' is unavailable. Choose another output device in Settings.",
                selection.name
            )
        })
}

/// Select a compatible advertised range; prefer default channel layout/format and
/// project rate when the user leaves those choices automatic.
pub(super) fn choose_config(
    ranges: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
    default: cpal::SupportedStreamConfig,
    settings: &AudioSettings,
    project_rate_hz: u32,
) -> Result<cpal::SupportedStreamConfig, String> {
    let mut best = None;
    let mut best_score = 0;
    for range in ranges {
        let Some(description) = describe(range) else {
            continue;
        };
        if !description.supports(settings) {
            continue;
        }
        let rate = settings.sample_rate_hz.unwrap_or_else(|| {
            if (range.min_sample_rate().0..=range.max_sample_rate().0).contains(&project_rate_hz) {
                project_rate_hz
            } else {
                default
                    .sample_rate()
                    .0
                    .clamp(range.min_sample_rate().0, range.max_sample_rate().0)
            }
        });
        let score = u8::from(range.channels() == default.channels()) * 4
            + u8::from(range.sample_format() == default.sample_format()) * 2
            + u8::from(rate == project_rate_hz);
        if best.is_none() || score > best_score {
            best_score = score;
            best = Some(range.with_sample_rate(cpal::SampleRate(rate)));
        }
    }
    best.ok_or_else(|| "The output device does not support this sample rate, bit depth and buffer size combination. Choose supported settings.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_discovery_survives_initializer_worker_exit() {
        const CHILD: &str = "DAW_WASAPI_DISCOVERY_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            use windows_sys::Win32::System::Com::{
                APTTYPE_STA, COINIT_APARTMENTTHREADED, CoGetApartmentType, CoInitializeEx,
                CoUninitialize,
            };
            // Simulate the window thread's STA while temporary discovery workers come and go.
            // SAFETY: this isolated test thread owns and balances its COM initialization.
            let status =
                unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
            assert!(status >= 0, "initialize simulated window STA: {status:#X}");
            struct Sta;
            impl Drop for Sta {
                fn drop(&mut self) {
                    // SAFETY: balances the successful initialization on this test thread.
                    unsafe {
                        CoUninitialize();
                    }
                }
            }
            let _sta = Sta;
            for _ in 0..3 {
                std::thread::spawn(|| {
                    super::super::wasapi::prepare().expect("initialize Windows audio COM lifetime");
                    // Device absence/driver rejection is allowed. A dead COM vtable is not.
                    let _ = discover_devices();
                    let _ = output_device(None);
                })
                .join()
                .expect("device discovery worker panicked");
            }
            // The cached MTA enumerator must also remain callable from the window's STA.
            let _ = discover_devices();
            let _ = output_device(None);
            let (mut apartment, mut qualifier) = (0, 0);
            // SAFETY: both output pointers refer to writable stack values.
            assert!(unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) } >= 0);
            assert_eq!(
                apartment, APTTYPE_STA,
                "audio initialization changed the window apartment"
            );
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "devices::tests::windows_discovery_survives_initializer_worker_exit",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("start isolated device discovery regression");
        assert!(
            output.status.success(),
            "device discovery child failed: {}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn range(format: cpal::SampleFormat, min: u32, max: u32) -> cpal::SupportedStreamConfigRange {
        cpal::SupportedStreamConfigRange::new(
            2,
            cpal::SampleRate(min),
            cpal::SampleRate(max),
            cpal::SupportedBufferSize::Range { min: 64, max: 1024 },
            format,
        )
    }
    #[test]
    fn automatic_prefers_project_rate_and_default_format() {
        let ranges = [
            range(cpal::SampleFormat::I16, 44100, 96000),
            range(cpal::SampleFormat::F32, 44100, 96000),
        ];
        let default = ranges[1].with_sample_rate(cpal::SampleRate(44100));
        let selected = choose_config(
            ranges.into_iter(),
            default,
            &AudioSettings::default(),
            48000,
        )
        .unwrap();
        assert_eq!(selected.sample_format(), cpal::SampleFormat::F32);
        assert_eq!(selected.sample_rate().0, 48000);
    }
    #[test]
    fn explicit_choices_are_never_silently_substituted() {
        let ranges = [
            range(cpal::SampleFormat::F32, 44100, 48000),
            range(cpal::SampleFormat::I16, 44100, 96000),
        ];
        let default = ranges[0].with_sample_rate(cpal::SampleRate(48000));
        let mut settings = AudioSettings {
            sample_rate_hz: Some(96000),
            sample_format: Some(SampleFormat::I16),
            buffer_size_frames: Some(256),
            ..Default::default()
        };
        let selected =
            choose_config(ranges.into_iter(), default.clone(), &settings, 48000).unwrap();
        assert_eq!(selected.sample_format(), cpal::SampleFormat::I16);
        assert_eq!(selected.sample_rate().0, 96000);
        settings.sample_format = Some(SampleFormat::F32);
        assert!(choose_config(ranges.into_iter(), default.clone(), &settings, 48000).is_err());
        settings.sample_rate_hz = Some(48000);
        settings.buffer_size_frames = Some(32);
        assert!(choose_config(ranges.into_iter(), default, &settings, 48000).is_err());
    }
    #[test]
    fn automatic_can_convert_project_rate_to_a_device_rate() {
        let range = range(cpal::SampleFormat::F32, 44100, 44100);
        let default = range.with_sample_rate(cpal::SampleRate(44100));
        assert_eq!(
            choose_config(
                [range].into_iter(),
                default,
                &AudioSettings::default(),
                192000
            )
            .unwrap()
            .sample_rate()
            .0,
            44100
        );
    }
}
