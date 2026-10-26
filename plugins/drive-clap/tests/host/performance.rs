use super::*;

#[test]
#[ignore = "explicit release-only direct-DSP/exported-binary CPU comparison"]
// This ignored test must reject an explicit debug-profile invocation at runtime.
#[allow(clippy::assertions_on_constants)]
fn callback_cpu_budget() {
    assert!(
        !cfg!(debug_assertions),
        "use --release for callback budgets"
    );
    assert!(
        std::env::var_os("DRIVE_CLAP_TEST_LIBRARY").is_some(),
        "benchmark the loaded artifact"
    );
    let _serial = SERIAL.lock().unwrap();
    let budgets: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/drive_plugin_phase0/budgets.json"
    ))
    .unwrap();
    let mut results = vec![];
    for automated in [false, true] {
        for frames in [32_usize, 512] {
            for instances in [1, 8] {
                let mut state = PluginState::default();
                state
                    .set(P::ColorShape, P::ColorShape.info().maximum)
                    .unwrap();
                let plugins: Vec<_> = (0..instances)
                    .map(|_| {
                        let mut plugin = Plugin::new();
                        plugin.load(state);
                        plugin.activate(48000, frames as u32);
                        plugin
                    })
                    .collect();
                let direct_meters: Vec<std::sync::Arc<daw_unit_core::BandMeter<3>>> =
                    (0..instances).map(|_| std::sync::Arc::default()).collect();
                let mut drives: Vec<_> = (0..instances)
                    .map(|index| {
                        let mut drive = Drive::new(
                            ProcessingContext {
                                sample_rate_hz: 48000,
                                max_block_frames: frames as u32,
                            },
                            &state.to_settings(),
                        );
                        // Match the exported processor's current/held output monitoring.
                        // The exported meter escapes to its UI consumer. Keep the direct
                        // handle equally observable instead of benchmarking private atomics.
                        drive.set_output_peak_meter(std::hint::black_box(
                            direct_meters[index].clone(),
                        ));
                        drive
                    })
                    .collect();
                let mut states = vec![state; instances];
                let pcm: Vec<_> = (0..frames)
                    .map(|i| {
                        [
                            (i as f32 * 0.073).sin() * 0.3,
                            (i as f32 * 0.097).cos() * 0.2,
                        ]
                    })
                    .collect();
                let events = EventList::new(&if automated {
                    (0..frames)
                        .map(|i| (i as u32, P::Gain, (i % 101) as f64 / 100.0))
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                });
                let events_abi = events.abi();
                let mut left = vec![0.0; frames];
                let mut right = left.clone();
                let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
                let input = clap_audio_buffer {
                    data32: pointers.as_mut_ptr(),
                    data64: ptr::null_mut(),
                    channel_count: 2,
                    latency: 0,
                    constant_mask: 0,
                };
                let mut output = input;
                let process = clap_process {
                    steady_time: -1,
                    frames_count: frames as u32,
                    transport: ptr::null(),
                    audio_inputs: &input,
                    audio_outputs: &mut output,
                    audio_inputs_count: 1,
                    audio_outputs_count: 1,
                    in_events: &events_abi,
                    out_events: ptr::null(),
                };
                let mut timings = [Vec::with_capacity(10000), Vec::with_capacity(10000)];
                for (mode, timing) in timings.iter_mut().enumerate() {
                    for block in 0..11000 {
                        // Prepare identical host-owned input outside either timed region.
                        for (i, sample) in pcm.iter().enumerate() {
                            left[i] = sample[0];
                            right[i] = sample[1];
                        }
                        let start = std::time::Instant::now();
                        if mode == 0 {
                            for (frame, sample) in pcm.iter().enumerate() {
                                let mut sample = std::hint::black_box(*sample);
                                for (index, drive) in drives.iter_mut().enumerate() {
                                    if automated {
                                        states[index].set(P::Gain, events.0[frame].value).unwrap();
                                        drive.retarget(&states[index].to_settings());
                                    }
                                    sample = drive.process_sample(sample);
                                }
                                std::hint::black_box(sample);
                            }
                        } else {
                            for plugin in &plugins {
                                let status = unsafe {
                                    (*plugin.pointer).process.unwrap()(plugin.pointer, &process)
                                };
                                assert_eq!(status, CLAP_PROCESS_CONTINUE);
                            }
                        }
                        let elapsed = start.elapsed().as_nanos() as u64;
                        if block >= 1000 {
                            timing.push(elapsed);
                        }
                    }
                }
                for meter in &direct_meters {
                    assert!(std::hint::black_box(meter.boost_peaks()[0].peak()) > 0.0);
                }
                for timing in &mut timings {
                    timing.sort_unstable();
                }
                let direct = timings[0][5000];
                let exported = timings[1][5000];
                let direct_p99 = timings[0][9900];
                let exported_p99 = timings[1][9900];
                let ceiling = direct as f64
                    * budgets["drive_median_budget"]["direct_dsp_multiplier"].as_f64().unwrap()
                    + instances as f64
                        * budgets["drive_median_budget"]["additional_nanoseconds_per_instance_per_block"]
                            .as_f64()
                            .unwrap();
                let p99_ceiling = frames as f64 / 48000.0
                    * 1e9
                    * budgets["callback_p99_budget_fraction_of_period"]
                        .as_f64()
                        .unwrap();
                results.push(serde_json::json!({"frames":frames,"instances":instances,"automated":automated,"monitoring":"current_and_held_output_peak","blocks":10000,"direct_median_ns":direct,"exported_median_ns":exported,"median_ceiling_ns":ceiling,"direct_p99_ns":direct_p99,"exported_p99_ns":exported_p99,"p99_ceiling_ns":p99_ceiling,"passed":exported as f64<=ceiling && exported_p99 as f64<=p99_ceiling && direct_p99 as f64<=p99_ceiling}));
            }
        }
    }
    let path = std::env::var_os("DRIVE_CLAP_BENCH_REPORT")
        .expect("set DRIVE_CLAP_BENCH_REPORT under ignored scratch space");
    std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    assert!(
        results.iter().all(|r| r["passed"] == true),
        "callback budget failed; inspect retained measurements"
    );
}
