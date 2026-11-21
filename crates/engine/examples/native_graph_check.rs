//! Only load the repository's own Drive products, in disposable test bundles.
use daw_core::*;
use daw_engine::*;
use daw_plugin_host::*;

#[path = "support/allocations.rs"]
mod allocations;

fn check_live_insert(
    mut plan: RenderPlan,
    settings: &HostedSettings,
    already_hosted: bool,
) -> daw_plugin_host::Result<()> {
    let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.effects.push(EffectInstance::plate_reverb());
    plan.project.add_beat_track().unwrap();
    // The note ends while the new graph is running. Its reverb tail must be
    // identical to uninterrupted playback, as must the first held samples.
    let hosted = EffectInstance {
        id: Id::new_v4(),
        unit: EffectSettings::Hosted(settings.clone()),
    };
    if already_hosted {
        plan.project.master.effects.push(hosted.clone());
    }
    let mut new_effect = hosted.duplicate();
    let EffectSettings::Hosted(new_settings) = &mut new_effect.unit else {
        unreachable!();
    };
    new_settings.enabled = false;
    let inserted = new_settings.id;
    let mut edited = plan.clone();
    let track = &edited.project.tracks[0];
    let target = EffectChainTarget::Layer {
        track: track.id,
        layer: track.instrument.as_ref().unwrap().main.id,
    };
    let (effects, compact) = edited.project.effect_chain(target).unwrap();
    let mut effects = effects.to_vec();
    effects.push(new_effect);
    edited.project.edit(target.edit(effects, compact)).unwrap();
    assert_eq!(plan.hosted_effect_inserted(&edited), Some(inserted));
    let mut changed = edited.clone();
    changed.project.master.gain_db -= 1.0;
    assert_eq!(plan.hosted_effect_inserted(&changed), None);

    let mut reference = Renderer::try_new(plan.clone())?;
    let mut playing = Renderer::try_new(plan)?;
    let mut candidate = Renderer::try_new(edited)?;
    reference.play();
    playing.play();
    for _ in 0..4099 {
        assert_eq!(reference.next_sample(), playing.next_sample());
    }
    allocations::measured(|| {
        assert!(candidate.try_insert_hosted_effect(&mut playing, inserted));
    });
    assert_eq!(candidate.transport_state(), reference.transport_state());
    let mut peak = 0.0f32;
    let mut tail = 0.0f32;
    for i in 0..72_000 {
        let a = reference.next_sample();
        let b = if i < 1024 {
            let mut sample = [0.0; 2];
            allocations::measured(|| sample = candidate.next_sample());
            sample
        } else {
            candidate.next_sample()
        };
        assert!(
            (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) < 1e-5,
            "existing audio changed at frame {i} (existing plugin: {already_hosted}): {a:?} vs {b:?}"
        );
        if i < 2048 {
            peak = peak.max(a[0].abs());
        }
        if i >= 57_000 {
            tail = tail.max(a[0].abs());
        }
    }
    assert!(peak > 0.001 && tail > 0.00001, "held {peak}, tail {tail}");
    assert!(candidate.hosted_fault().is_none());
    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let format = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("clap") => PluginFormat::Clap,
        Some("vst3") => PluginFormat::Vst3,
        _ => return Err("Expected format and owned module path".into()),
    };
    let path = std::path::PathBuf::from(args.next().ok_or("Missing module")?).canonicalize()?;
    let discovery = discover(&[SearchRoot { format, path }]);
    let candidate = discovery.candidates.first().ok_or("No candidate")?.clone();
    let identity = match format {
        PluginFormat::Clap => PluginIdentity::Clap(daw_drive_plugin::IDENTITY.clap_id.into()),
        PluginFormat::Vst3 => PluginIdentity::Vst3(daw_drive_plugin::IDENTITY.processor_id),
    };
    let descriptor = HostedPluginDescriptor {
        identity: identity.clone(),
        name: "Drive".into(),
        vendor: "DAW".into(),
        version: "1".into(),
        role: HostedRole::Effect,
    };
    let mut registry = PluginRegistry::default();
    registry.entries.push(RegistryEntry {
        candidate: candidate.clone(),
        plugins: vec![descriptor.clone()],
        state: ScanState::Ready,
        present: true,
    });
    let (mut service, client) = HostService::new(registry);
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for block in [32, 512, 4096] {
            eprintln!("Checking {format:?} graph at {rate} Hz with a {block}-frame buffer");
            let mut state = daw_drive_plugin::PluginState::default();
            state.set(daw_drive_plugin::Parameter::Gain, 0.6)?;
            state.set(daw_drive_plugin::Parameter::Volume, -3.0)?;
            let opaque = match format {
                PluginFormat::Clap => NativeState::Clap(state.encode().to_vec()),
                PluginFormat::Vst3 => NativeState::Vst3 {
                    component: state.encode().to_vec(),
                    controller: vec![],
                },
            };
            let native = unsafe {
                service
                    .native
                    .load(&candidate, &identity, rate, block, Some(&opaque))
            }?;
            let metadata = service.native.metadata(native)?.clone();
            let bindings = metadata
                .parameters
                .iter()
                .map(|p| {
                    service
                        .native
                        .parameter_value(native, p.id)
                        .map(|value| HostedParameterBinding::new(p.clone(), value))
                })
                .collect::<daw_plugin_host::Result<Vec<_>>>()?;
            service.native.remove(native)?;
            let mut settings = HostedSettings::new(descriptor.clone(), bindings);
            settings.metadata_rate = rate;
            settings.latency_frames = metadata.latency_frames;
            settings.tail_frames = metadata.tail_frames;
            let mut states = StateBank::default();
            settings.state = Some(states.capture(opaque, 0)?);
            let mut project = Project {
                sample_rate_hz: rate,
                buffer_size_frames: block,
                ..Default::default()
            };
            let track = project.add_midi_track()?;
            let oscillator = OscillatorSettings {
                unison: OscillatorUnison::Eight,
                ..Default::default()
            };
            project.tracks[0].instrument = Some(oscillator.into());
            let clip = project.add_midi_pattern(track, 0)?;
            project.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 1920,
                        pitch: 60,
                        velocity: 100,
                    }],
                },
            })?;
            let mut plan = RenderPlan {
                external: ExternalPreparation {
                    host: Some(client.clone()),
                    states,
                    offline: false,
                    publish: false,
                },
                project,
                beat_tracks: Default::default(),
                audio: Default::default(),
                sampler_audio: Default::default(),
                stretched_audio: Default::default(),
            };
            if rate == 48000 && block == 512 {
                assert_eq!(settings.latency_frames, 0);
                let live = plan.clone();
                let settings = settings.clone();
                service.run(move || {
                    check_live_insert(live.clone(), &settings, false)?;
                    check_live_insert(live, &settings, true)
                })?;
            }
            let portable = EffectInstance {
                id: Id::new_v4(),
                unit: EffectSettings::Drive(state.to_settings()),
            };
            plan.project.tracks[0].effects.push(portable);
            let mut reference = Renderer::new(plan.clone());
            let mut expected = vec![[0.0; 2]; 8192];
            reference.seek(0);
            reference.render_offline(&mut expected);
            plan.project.tracks[0].effects[0].unit = EffectSettings::Hosted(settings);
            plan.project.validate()?;
            let actual = service.run(move || -> daw_plugin_host::Result<Vec<[f32; 2]>> {
                let mut renderer = Renderer::try_new(plan)?;
                assert!(renderer.snapshot().is_none());
                renderer.seek(0);
                let mut actual = vec![[0.0; 2]; 8192];
                let mut offset = 0;
                for count in [1, 17, 32, 513, 4096, 3533] {
                    renderer.render_offline(&mut actual[offset..offset + count]);
                    offset += count;
                }
                Ok(actual)
            })?;
            assert!(expected.iter().any(|s| s[0].abs() > 0.001));
            for (frame, (a, b)) in expected.iter().zip(&actual).enumerate() {
                for ch in 0..2 {
                    assert!(
                        (a[ch] - b[ch]).abs() < 1e-6,
                        "{format:?} rate {rate} block {block} frame {frame}: {a:?} {b:?}"
                    );
                }
            }
        }
    }
    drop(client);
    service.pump(|_, _| {});
    println!(
        "PASS: {format:?} full MIDI source → native effect → master graph, six rates, three blocks and irregular output slices"
    );
    Ok(())
}
