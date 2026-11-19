//! Loaded-binary processing check against this repository's portable Drive.
use daw_drive_plugin::{IDENTITY, Parameter, PluginState};
use daw_plugin_host::*;
use daw_plugin_support::allocation_probe::{Allocator, assert_no_heap as measured};
use daw_unit_core::{EffectUnit, ProcessingContext};
use std::path::PathBuf;

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let format = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("clap") => PluginFormat::Clap,
        Some("vst3") => PluginFormat::Vst3,
        _ => return Err("Expected clap|vst3 and an owned Drive module path".into()),
    };
    let path = PathBuf::from(args.next().ok_or("Missing module path")?).canonicalize()?;
    let found = discover(&[SearchRoot { format, path }]);
    let candidate = found
        .candidates
        .first()
        .ok_or_else(|| format!("No native candidate: {:?}", found.warnings))?;
    let identity = match format {
        PluginFormat::Clap => PluginIdentity::Clap(IDENTITY.clap_id.into()),
        PluginFormat::Vst3 => PluginIdentity::Vst3(IDENTITY.processor_id),
    };
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut host = NativeHost::new();
        let mut state = PluginState::default();
        state.set(Parameter::Volume, -3.0)?;
        let saved = match format {
            PluginFormat::Clap => NativeState::Clap(state.encode().to_vec()),
            PluginFormat::Vst3 => NativeState::Vst3 {
                component: state.encode().to_vec(),
                controller: vec![],
            },
        };
        // SAFETY: this check loads only the repository's known Drive ABI build on main.
        let id = unsafe { host.load(candidate, &identity, rate, 4096, Some(&saved)) }?;
        assert!(!host.metadata(id)?.parameters.is_empty());
        assert_eq!(host.metadata(id)?.latency_frames, 0);
        let mut processor = host.take_processor(id)?;
        assert!(host.take_processor(id).is_err());
        std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let mut drive = daw_drive::Drive::new(
                        ProcessingContext {
                            sample_rate_hz: rate,
                            max_block_frames: 4096,
                        },
                        &state.to_settings(),
                    );
                    let mut frame = 0;
                    for frames in [1, 17, 32, 64, 128, 512, 4096, 31] {
                        let input: Vec<_> = (0..frames)
                            .map(|n| {
                                let x = ((frame + n) as f32 * 0.17).sin() * 0.8;
                                [x, x * -0.37 + 0.1]
                            })
                            .collect();
                        let mut output = vec![[0.0; 2]; frames];
                        let status = measured(|| {
                            processor.process(
                                &[&input],
                                &mut output,
                                &[],
                                NativeTransport {
                                    frame: frame as u64,
                                    playing: true,
                                    ..Default::default()
                                },
                            )
                        });
                        assert_eq!(status, NativeProcessStatus::Processed);
                        for (n, (source, actual)) in input.iter().zip(&output).enumerate() {
                            let expected = drive.process_sample(*source);
                            for channel in 0..2 {
                                assert!(
                                    (actual[channel] - expected[channel]).abs() < 2e-6,
                                    "{format:?} rate={rate} sample={n}: {actual:?} != {expected:?}"
                                );
                            }
                        }
                        frame += frames;
                    }
                    // Transient automation must not become the saved manual
                    // base when the exact state barrier flushes the processor.
                    let tail = [[0.0; 2]; 32];
                    let event = NativeEvent::Parameter {
                        offset: 0,
                        id: Parameter::Volume as u32,
                        value: if format == PluginFormat::Clap {
                            6.0
                        } else {
                            Parameter::Volume.normalize(6.0).unwrap()
                        },
                    };
                    assert_eq!(
                        measured(|| processor.process(
                            &[&tail],
                            &mut [[0.0; 2]; 32],
                            &[event],
                            NativeTransport::default()
                        )),
                        NativeProcessStatus::Processed
                    );
                    drop(processor);
                })
                .join()
        })
        .unwrap();
        let captured = host.save_state(id)?;
        match captured {
            NativeState::Clap(bytes) => {
                assert_eq!(PluginState::decode(&bytes)?.get(Parameter::Volume), -3.0)
            }
            NativeState::Vst3 { component, .. } => assert_eq!(
                PluginState::decode(&component)?.get(Parameter::Volume),
                -3.0
            ),
        }
        host.remove(id)?;
        host.pump(|_, _| {});
        assert!(host.metadata(id).is_err());
    }
    println!(
        "PASS: {format:?} loaded processing, six rates, irregular spans, state, exclusive lease, retirement, and zero host callback heap operations"
    );
    Ok(())
}
