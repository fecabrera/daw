//! Manual, project-free native state recall check for one explicitly supplied module.
//! Run in a disposable process: arbitrary native plugin code is not contained here.
use daw_plugin_host::*;
use std::path::PathBuf;
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let format = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("clap") => PluginFormat::Clap,
        Some("vst3") => PluginFormat::Vst3,
        _ => return Err("Expected clap|vst3 and one explicit module path".into()),
    };
    let path = PathBuf::from(args.next().ok_or("Missing module path")?).canonicalize()?;
    let discovery = discover(&[SearchRoot { format, path }]);
    if discovery.candidates.len() != 1 {
        return Err("Expected exactly one native module".into());
    }
    let candidate = &discovery.candidates[0];
    // SAFETY: the caller explicitly chooses the native module. This disposable
    // command owns all foreign calls on its main thread and never opens a project.
    let descriptors = unsafe { inspect_native(format, &candidate.path) }?;
    for descriptor in descriptors {
        let mut host = NativeHost::new();
        let id = unsafe { host.load(candidate, &descriptor.identity, 48000, 512, None) }?;
        let parameters = host.metadata(id)?.parameters.clone();
        let values = parameters
            .iter()
            .map(|p| host.parameter_value(id, p.id))
            .collect::<Result<Vec<_>>>()?;
        let state = host.save_state(id)?;
        host.remove(id)?;
        let restored =
            unsafe { host.load(candidate, &descriptor.identity, 48000, 512, Some(&state)) }?;
        for (parameter, expected) in parameters.iter().zip(values) {
            if !parameter.read_only {
                let actual = host.parameter_value(restored, parameter.id)?;
                if (actual - expected).abs() > 1e-6 {
                    return Err(format!(
                        "{}: parameter {} changed after recall: {expected} -> {actual}",
                        descriptor.name, parameter.name
                    )
                    .into());
                }
            }
        }
        let inputs = vec![vec![[0.05; 2]; 512]; host.metadata(restored)?.inputs.len()];
        let inputs: Vec<&[[f32; 2]]> = inputs.iter().map(Vec::as_slice).collect();
        let mut processor = host.take_processor(restored)?;
        std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let mut output = [[0.0; 2]; 512];
                    for block in 0..4 {
                        assert_eq!(
                            processor.process(
                                &inputs,
                                &mut output,
                                &[],
                                NativeTransport {
                                    frame: block * 512,
                                    playing: true,
                                    ..Default::default()
                                }
                            ),
                            NativeProcessStatus::Processed
                        );
                        assert!(output.iter().flatten().all(|value| value.is_finite()));
                    }
                })
                .join()
                .expect("native processing check failed");
        });
        host.pump(|_, _| {});
        host.save_state(restored)?;
        host.remove(restored)?;
        println!(
            "PASS: {} ({format:?}) capture, fresh-instance restore, parameter values, four stereo processing blocks and recapture",
            descriptor.name
        );
    }
    Ok(())
}
