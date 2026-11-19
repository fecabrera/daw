//! Owned synthetic CLAP/VST3 instrument and PCM sidechain acceptance.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::measured;
use daw_core::*;
use daw_engine::*;
use daw_plugin_host::*;
use std::{path::PathBuf, sync::Arc};
fn base_plan(rate: u32, block: u32, client: Arc<HostClient>, states: StateBank) -> RenderPlan {
    RenderPlan {
        external: ExternalPreparation {
            host: Some(client),
            states,
            offline: true,
            publish: false,
        },
        project: Project {
            sample_rate_hz: rate,
            buffer_size_frames: block,
            ..Default::default()
        },
        beat_tracks: Default::default(),
        audio: Default::default(),
        sampler_audio: Default::default(),
        stretched_audio: Default::default(),
    }
}
fn mapped(
    service: &mut HostService,
    candidate: &PluginCandidate,
    descriptor: &HostedPluginDescriptor,
    rate: u32,
    block: u32,
) -> std::result::Result<(HostedSettings, StateBank, NativeMetadata), Box<dyn std::error::Error>> {
    let id = unsafe {
        service
            .native
            .load(candidate, &descriptor.identity, rate, block, None)
    }?;
    let metadata = service.native.metadata(id)?.clone();
    let parameters = metadata
        .parameters
        .iter()
        .map(|p| {
            service
                .native
                .parameter_value(id, p.id)
                .map(|v| HostedParameterBinding::new(p.clone(), v))
        })
        .collect::<daw_plugin_host::Result<Vec<_>>>()?;
    let mut settings = HostedSettings::new(descriptor.clone(), parameters);
    settings.metadata_rate = rate;
    settings.latency_frames = metadata.latency_frames;
    settings.tail_frames = metadata.tail_frames;
    let mut states = StateBank::default();
    settings.state = Some(states.capture(service.native.save_state(id)?, 0)?);
    service.native.remove(id)?;
    Ok((settings, states, metadata))
}
fn render(plan: RenderPlan, count: usize) -> daw_plugin_host::Result<Vec<[f32; 2]>> {
    let mut renderer = Renderer::try_new(plan)?;
    renderer.try_seek(0)?;
    let mut output = vec![[0.0; 2]; count];
    let mut offset = 0;
    for size in [1, 17, 32, 513, 4096] {
        let size = size.min(count - offset);
        measured(|| renderer.render_offline(&mut output[offset..offset + size]));
        offset += size;
    }
    if offset < count {
        measured(|| renderer.render_offline(&mut output[offset..]));
    }
    assert!(renderer.hosted_fault().is_none());
    assert!(renderer.snapshot().is_none());
    // Backward requests reconstruct an independent instance and replay on this
    // worker. The main service continues running during this request.
    let mut sought = [[0.0; 2]; 32];
    renderer.try_seek(1000)?;
    measured(|| renderer.render_offline(&mut sought));
    assert_eq!(&output[1000..1032], &sought);
    Ok(output)
}
fn add_audio(plan: &mut RenderPlan, samples: Vec<[f32; 2]>) -> Id {
    let track = plan.project.add_track().unwrap();
    let asset = Id::new_v4();
    let frames = samples.len() as u64;
    let metadata = SourceMetadata {
        sample_rate_hz: plan.project.sample_rate_hz,
        channels: 2,
        sample_format: "ieee_float".into(),
        bits_per_sample: 32,
    };
    plan.project.assets.push(Asset {
        id: asset,
        name: "Fixture".into(),
        source: Source {
            kind: "external".into(),
            path: "fixture.wav".into(),
            path_kind: "absolute".into(),
            storage: None,
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: frames,
        midi_pattern: None,
        rendered_audio: None,
        compound_audio: None,
        beat_pattern: None,
    });
    plan.project.tracks.last_mut().unwrap().clips.push(Clip {
        id: Id::new_v4(),
        asset_id: asset,
        name: "Fixture".into(),
        color: None,
        start_frame: 0,
        length_frames: frames,
        source_offset_frame: 0,
        repeat: None,
        gain_db: 0.0,
        transpose: 0,
        channels: Default::default(),
        fades: Default::default(),
        muted: false,
        stretch: None,
    });
    plan.audio.insert(
        asset,
        daw_media::AudioData {
            disk: None,
            sample_rate_hz: plan.project.sample_rate_hz,
            samples: Arc::new(samples),
            metadata,
            peaks: Arc::new(vec![]),
        },
    );
    track
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dispatch_embedded_scan();
    let mut args = std::env::args_os().skip(1);
    let format = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("clap") => PluginFormat::Clap,
        Some("vst3") => PluginFormat::Vst3,
        _ => return Err("Expected format and an owned fixture bundle".into()),
    };
    let path = PathBuf::from(args.next().ok_or("Missing module")?).canonicalize()?;
    let discovery = discover(&[SearchRoot { format, path }]);
    let candidate = discovery
        .candidates
        .first()
        .ok_or("No fixture candidate")?
        .clone();
    let scratch = std::env::temp_dir().join(format!("daw-fixture-scan-{}", std::process::id()));
    let scan = scan_isolated(
        &std::env::current_exe()?,
        &candidate,
        &scratch,
        std::time::Duration::from_secs(10),
        || false,
    );
    let _ = std::fs::remove_dir(&scratch);
    let descriptors = scan?.plugins;
    assert_eq!(descriptors.len(), 2);
    let effect = descriptors
        .iter()
        .find(|p| p.role == HostedRole::Effect)
        .ok_or("Missing effect class")?
        .clone();
    let instrument = descriptors
        .iter()
        .find(|p| p.role == HostedRole::Instrument)
        .ok_or("Missing instrument class")?
        .clone();
    // An unimplemented controller sync must not mask an actual processor
    // restore failure. Both formats must name the failing native module.
    let mut host = NativeHost::new();
    let invalid = match format {
        PluginFormat::Clap => NativeState::Clap(vec![0]),
        PluginFormat::Vst3 => NativeState::Vst3 {
            component: vec![0],
            controller: Vec::new(),
        },
    };
    let error =
        unsafe { host.load(&candidate, &effect.identity, 48000, 512, Some(&invalid)) }.unwrap_err();
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains(&candidate.path.display().to_string()),
        "{diagnostic}"
    );
    assert!(diagnostic.contains(format.extension()), "{diagnostic}");
    drop(host);
    let mut registry = PluginRegistry::default();
    registry.entries.push(RegistryEntry {
        candidate: candidate.clone(),
        plugins: descriptors,
        state: ScanState::Ready,
        present: true,
    });
    let (mut service, client) = HostService::new(registry);
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for block in [32, 64, 128, 256, 512, 1024, 2048, 4096] {
            let (settings, states, _) = mapped(&mut service, &candidate, &instrument, rate, block)?;
            let mut plan = base_plan(rate, block, client.clone(), states);
            let track = plan.project.add_midi_track()?;
            let clip = plan.project.add_midi_pattern(track, 0)?;
            let layer = &mut plan.project.tracks[0].instrument.as_mut().unwrap().layers[0];
            layer.clear_sources();
            layer.hosted = Some(HostedSource::Known(settings));
            plan.project.edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![
                        MidiNote {
                            id: Id::new_v4(),
                            start_tick: 0,
                            length_ticks: 120,
                            pitch: 60,
                            velocity: 127,
                        },
                        MidiNote {
                            id: Id::new_v4(),
                            start_tick: 60,
                            length_ticks: 120,
                            pitch: 72,
                            velocity: 64,
                        },
                    ],
                },
            })?;
            plan.project.validate()?;
            let frames = (rate / 4).max(8192) as usize;
            let output = service.run(move || render(plan, frames))?;
            assert_eq!(
                output[0], [0.0; 2],
                "source clip fade must retain its latency-adjusted origin"
            );
            assert!(
                (output[1000][0] - 61.0 / 128.0).abs() < 1e-6,
                "{format:?} instrument rate {rate} block {block}: {:?}",
                output[1000]
            );
            assert!((output[1000][1] - 61.0 / 256.0).abs() < 1e-6);
            let overlap = (rate * 3 / 64) as usize;
            assert!(
                (output[overlap][0] - (61.0 / 128.0 + 73.0 / 128.0 * 64.0 / 127.0)).abs() < 1e-6,
                "independent polyphonic notes"
            );
            assert_eq!(output[frames - 1], [0.0; 2]);
            let (mut settings, states, metadata) =
                mapped(&mut service, &candidate, &effect, rate, block)?;
            let mut plan = base_plan(rate, block, client.clone(), states);
            let source = add_audio(&mut plan, vec![[0.1, -0.2]; 8192]);
            let destination = add_audio(&mut plan, vec![[0.2, 0.3]; 8192]);
            settings.audio_inputs.push(HostedAudioInput {
                id: Id::new_v4(),
                source_track: source,
                port: match format {
                    PluginFormat::Clap => HostedInputId::Clap { id: 11 },
                    PluginFormat::Vst3 => HostedInputId::Vst3 {
                        index: 1,
                        layout: metadata.audio_layout_signature(),
                    },
                },
                tap: daw_unit_core::SidechainTap::PreFader,
                channels: Default::default(),
                gain_db: 0.0,
                enabled: true,
            });
            plan.project
                .tracks
                .iter_mut()
                .find(|t| t.id == destination)
                .unwrap()
                .effects
                .push(EffectInstance {
                    id: Id::new_v4(),
                    unit: EffectSettings::Hosted(settings),
                });
            plan.project.validate()?;
            let bounce_plan = plan.clone();
            let output = service.run(move || render(plan, 8192))?;
            if rate == 48000 && block == 512 {
                let mut captured_plan = bounce_plan.clone();
                let original = captured_plan
                    .project
                    .tracks
                    .iter()
                    .find(|t| t.id == source)
                    .unwrap()
                    .clips[0]
                    .asset_id;
                captured_plan
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == source)
                    .unwrap()
                    .clips
                    .clear();
                add_audio(&mut captured_plan, vec![[0.1, -0.2]; 8192]);
                let carrier = captured_plan.project.tracks.last().unwrap().id;
                let carrier_asset = captured_plan.project.tracks.last().unwrap().clips[0].asset_id;
                let asset = captured_plan
                    .project
                    .assets
                    .iter_mut()
                    .find(|a| a.id == carrier_asset)
                    .unwrap();
                asset.source.kind = "rendered_audio".into();
                asset.rendered_audio = Some(RenderedAudio {
                    version: 1,
                    pre_send_asset: original,
                    feeds: vec![RenderedFeed {
                        provider: source,
                        raw_asset: original,
                        post_asset: original,
                    }],
                });
                captured_plan.project.validate()?;
                let pre = captured_plan.clone();
                let output = service.run(move || render(pre, 8192))?;
                assert!(
                    (output[1000][0] - 0.4).abs() < 1e-6,
                    "captured raw PCM provenance: {:?}",
                    output[1000]
                );
                captured_plan
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == carrier)
                    .unwrap()
                    .gain_db = -6.0;
                if let EffectSettings::Hosted(s) = &mut captured_plan
                    .project
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == destination)
                    .unwrap()
                    .effects[0]
                    .unit
                {
                    s.audio_inputs[0].tap = daw_unit_core::SidechainTap::PostFader;
                }
                let output = service.run(move || render(captured_plan, 8192))?;
                assert!(
                    (output[1000][0] - (0.2 + 0.2 * linear_gain(-6.0))).abs() < 1e-6,
                    "captured post PCM provenance: {:?}",
                    output[1000]
                );
                service.run(move || -> std::result::Result<(), String> {
                    let track = bounce_plan
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.id == destination)
                        .unwrap();
                    let target = BounceTarget {
                        track_id: destination,
                        clip_ids: track.clips.iter().map(|c| c.id).collect(),
                        start_frame: 0,
                        end_frame: 8192,
                    };
                    let mut capture = BounceCapture::new(bounce_plan, &target)?;
                    let mut out = Vec::new();
                    while out.len() < 2048 {
                        if let Some(frame) = capture.next_frame()? {
                            out.push(frame.main);
                        }
                    }
                    assert!(
                        (out[1000][0] - 0.3).abs() < 1e-6,
                        "native isolated bounce: {:?}",
                        out[1000]
                    );
                    assert!((out[1000][1] - 0.1).abs() < 1e-6);
                    Ok(())
                })?;
            }
            assert!(
                (output[1000][0] - 0.4).abs() < 1e-6,
                "{format:?} raw sidechain rate {rate} block {block}: {:?}",
                output[1000]
            );
            assert!((output[1000][1] + 0.1).abs() < 1e-6);
        }
    }
    // Strict rendering rejects unavailable units. Explicit degraded rendering
    // keeps the saved latency and produces dry audio without loading a module.
    let (settings, states, _) = mapped(&mut service, &candidate, &effect, 48000, 512)?;
    let (mut missing_service, missing_client) = HostService::new(PluginRegistry::default());
    let mut missing_plan = base_plan(48000, 512, missing_client.clone(), states);
    add_audio(&mut missing_plan, vec![[0.2, -0.1]; 8192]);
    missing_plan.project.tracks[0].effects.push(EffectInstance {
        id: Id::new_v4(),
        unit: EffectSettings::Hosted(settings.clone()),
    });
    let strict = missing_plan.clone();
    assert!(
        missing_service
            .run(move || Renderer::try_new(strict).map(|_| ()))
            .is_err()
    );
    missing_plan.external.host = Some(Arc::new(
        missing_client.allow_missing([settings.plugin.identity]),
    ));
    let output = missing_service.run(move || render(missing_plan, 8192))?;
    assert!((output[1000][0] - 0.2).abs() < 1e-6);
    assert!((output[1000][1] + 0.1).abs() < 1e-6);
    // Live block consumption, pause/resume and loop wrap use the same graph.
    let (settings, states, _) = mapped(&mut service, &candidate, &effect, 48000, 512)?;
    let mut live_plan = base_plan(48000, 512, client.clone(), states);
    live_plan.external.offline = false;
    add_audio(&mut live_plan, vec![[0.2, -0.1]; 8192]);
    live_plan.project.tracks[0].effects.push(EffectInstance {
        id: Id::new_v4(),
        unit: EffectSettings::Hosted(settings),
    });
    let inherit_plan = live_plan.clone();
    service.run(move || -> daw_plugin_host::Result<()> {
        let mut original = Renderer::try_new(inherit_plan.clone())?;
        let mut reference = Renderer::try_new(inherit_plan.clone())?;
        let mut replacement = Renderer::try_new(inherit_plan)?;
        original.play();
        reference.play();
        for _ in 0..1003 {
            measured(|| original.next_sample());
            measured(|| reference.next_sample());
        }
        assert!(replacement.can_inherit_without_preroll(&original));
        measured(|| replacement.inherit(&mut original));
        for _ in 0..2048 {
            assert_eq!(
                measured(|| replacement.next_sample()),
                measured(|| reference.next_sample()),
                "partial native FIFO handoff"
            );
        }
        Ok(())
    })?;
    live_plan.project.transport.r#loop.enabled = true;
    live_plan.project.transport.r#loop.start_frame = 1000;
    live_plan.project.transport.r#loop.end_frame = 2000;
    service.run(move || -> daw_plugin_host::Result<()> {
        let mut renderer = Renderer::try_new(live_plan)?;
        renderer.play();
        for _ in 0..1750 {
            let output = measured(|| renderer.next_sample());
            assert!(output.iter().all(|v| v.is_finite()));
        }
        assert_eq!(renderer.transport_state().frame, 1750);
        renderer.pause();
        for _ in 0..300 {
            measured(|| renderer.next_sample());
        }
        assert_eq!(measured(|| renderer.next_sample()), [0.0; 2]);
        assert_eq!(renderer.transport_state().frame, 1750);
        renderer.resume();
        for _ in 0..1000 {
            let output = measured(|| renderer.next_sample());
            assert!(output[0] > 0.0);
        }
        assert_eq!(renderer.transport_state().frame, 1750);
        assert!(renderer.hosted_fault().is_none());
        Ok(())
    })?;
    // A loop can start inside the same held note. Repeated stable note IDs
    // still require a fresh native attack, including clip-loop retriggers.
    let (settings, states, _) = mapped(&mut service, &candidate, &instrument, 48000, 512)?;
    let mut repeated = base_plan(48000, 512, client.clone(), states);
    repeated.external.offline = false;
    let track = repeated.project.add_midi_track()?;
    let layer = &mut repeated.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.clear_sources();
    layer.hosted = Some(HostedSource::Known(settings));
    let clip = repeated.project.add_midi_pattern(track, 0)?;
    repeated.project.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: MidiPattern {
            length_ticks: 3840,
            notes: vec![MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 1920,
                pitch: 60,
                velocity: 127,
            }],
        },
    })?;
    repeated.project.transport.r#loop.enabled = true;
    repeated.project.transport.r#loop.start_frame = 1000;
    repeated.project.transport.r#loop.end_frame = 2000;
    service.run(move || -> daw_plugin_host::Result<()> {
        let mut renderer = Renderer::try_new(repeated)?;
        renderer.play();
        let mut output = vec![[0.0; 2]; 2501];
        for sample in &mut output {
            *sample = measured(|| renderer.next_sample());
        }
        assert!(output[500][0] > 0.4);
        assert_eq!(
            output[1000], [0.0; 2],
            "looped held note must retrigger its native attack"
        );
        assert_eq!(output[2000], [0.0; 2]);
        Ok(())
    })?;
    // A bounded preview sends each native voice an off at its content-clock gate.
    let (settings, states, _) = mapped(&mut service, &candidate, &instrument, 48000, 512)?;
    let mut preview = base_plan(48000, 512, client.clone(), states);
    let track = preview.project.add_midi_track()?;
    let layer = &mut preview.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.clear_sources();
    layer.hosted = Some(HostedSource::Known(settings));
    let clip_id = preview.project.add_midi_pattern(track, 0)?;
    preview.project.edit(Edit::ReplaceMidiPattern {
        clip_id,
        pattern: Chord {
            root: 0,
            kind: ChordKind::Major,
        }
        .pattern()?,
    })?;
    service.run(move || -> daw_plugin_host::Result<()> {
        let mut renderer = Renderer::try_new(preview)?;
        let gate = 3001;
        renderer.begin_finite_audition(track, gate as u64);
        let mut samples = [[0.0; 2]; 3701];
        for block in samples.chunks_mut(127) {
            measured(|| renderer.render_offline(block));
        }
        assert!(samples[gate - 1][0] > 0.1);
        assert!(samples[gate..].iter().all(|sample| *sample == [0.0; 2]));
        assert!(renderer.hosted_fault().is_none());
        Ok(())
    })?;
    // One shared Macro drives the common typed destination. Plain CLAP deltas
    // and normalized VST3 automation produce the same synthetic gain.
    let (mut settings, states, _) = mapped(&mut service, &candidate, &instrument, 48000, 512)?;
    let mut plan = base_plan(48000, 512, client.clone(), states);
    let track = plan.project.add_midi_track()?;
    let instrument_settings = plan.project.tracks[0].instrument.as_mut().unwrap();
    instrument_settings.main.macro_module.add();
    instrument_settings.main.macro_module.controls[0]
        .settings
        .value = 0.5;
    let mut route = Modulation::new(ModulationParameter::HostedValue);
    route.target = Some(settings.parameters[0].target);
    route.source = Some(ModulationSourceId {
        layer: instrument_settings.main.id,
        unit: instrument_settings.main.macro_module.controls[0].id,
    });
    route.amount = if format == PluginFormat::Clap {
        0.5
    } else {
        0.25
    };
    settings.modulations.push(route);
    let layer = &mut instrument_settings.layers[0];
    layer.clear_sources();
    layer.hosted = Some(HostedSource::Known(settings));
    let clip = plan.project.add_midi_pattern(track, 0)?;
    plan.project.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: MidiPattern {
            length_ticks: 3840,
            notes: vec![MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 1920,
                pitch: 60,
                velocity: 127,
            }],
        },
    })?;
    let output = service.run(move || render(plan, 8192))?;
    assert!(
        (output[1000][0] - 61.0 / 128.0 * 1.25).abs() < 1e-5,
        "shared native modulation: {:?}",
        output[1000]
    );
    drop(client);
    service.pump(|_, _| {});
    println!(
        "PASS: {format:?} polyphonic instruments, raw PCM sidechains, shared modulation, independent seeks and callback heap guard at six rates/all eight project buffers, with bounce, live loops, pause/resume, degraded rendering and partial FIFO handoff"
    );
    Ok(())
}
