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
        std::env::var_os("DRIVE_VST3_TEST_LIBRARY").is_some(),
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
                        plugin.activate(48000, frames as i32, 0);
                        plugin
                    })
                    .collect();
                let mut drives: Vec<_> = (0..instances)
                    .map(|_| {
                        Drive::new(
                            ProcessingContext {
                                sample_rate_hz: 48000,
                                max_block_frames: frames as u32,
                            },
                            &state.to_settings(),
                        )
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
                let points = if automated {
                    vec![(
                        P::Gain,
                        (0..frames)
                            .map(|i| (i as i32, (i % 101) as f64 / 100.0))
                            .collect(),
                    )]
                } else {
                    vec![]
                };
                let queues = changes(&points);
                let mut left = vec![0.0; frames];
                let mut right = left.clone();
                let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
                let mut input = AudioBusBuffers {
                    numChannels: 2,
                    silenceFlags: 0,
                    __field0: AudioBusBuffers__type0 {
                        channelBuffers32: pointers.as_mut_ptr(),
                    },
                };
                let mut output = input;
                let mut data: ProcessData = unsafe { std::mem::zeroed() };
                data.numSamples = frames as i32;
                data.numInputs = 1;
                data.numOutputs = 1;
                data.inputs = &mut input;
                data.outputs = &mut output;
                data.inputParameterChanges = queues.as_ptr();
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
                                        states[index]
                                            .set_normalized(P::Gain, points[0].1[frame].1)
                                            .unwrap();
                                        drive.retarget(&states[index].to_settings());
                                    }
                                    sample = drive.process_sample(sample);
                                }
                                std::hint::black_box(sample);
                            }
                        } else {
                            for plugin in &plugins {
                                let status = unsafe { plugin.processor.process(&mut data) };
                                assert_eq!(status, kResultOk);
                            }
                        }
                        let elapsed = start.elapsed().as_nanos() as u64;
                        if block >= 1000 {
                            timing.push(elapsed);
                        }
                    }
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
                results.push(serde_json::json!({"frames":frames,"instances":instances,"automated":automated,"blocks":10000,"direct_median_ns":direct,"exported_median_ns":exported,"median_ceiling_ns":ceiling,"direct_p99_ns":direct_p99,"exported_p99_ns":exported_p99,"p99_ceiling_ns":p99_ceiling,"passed":exported as f64<=ceiling && exported_p99 as f64<=p99_ceiling && direct_p99 as f64<=p99_ceiling}));
            }
        }
    }
    let path = std::env::var_os("DRIVE_VST3_BENCH_REPORT")
        .expect("set DRIVE_VST3_BENCH_REPORT under ignored scratch space");
    std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    assert!(
        results.iter().all(|r| r["passed"] == true),
        "callback budget failed; inspect retained measurements"
    );
}
