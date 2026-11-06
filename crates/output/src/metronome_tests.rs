use super::*;

#[test]
fn metronome_follows_looped_transport_and_stays_out_of_offline_audio() {
    let mut plan = tests::transport_plan();
    plan.project.master.gain_db = daw_core::SILENT_GAIN_DB;
    plan.project.transport.r#loop = daw_core::Loop {
        enabled: true,
        start_frame: 0,
        end_frame: 24_000,
    };
    let meters = track_meters(&plan, &HashMap::new());
    let (_, consumer) = RingBuffer::new(32);
    let (retire, _retired) = RingBuffer::new(64);
    let status = Arc::new(Status::default());
    status.metronome_enabled.store(true, Ordering::Relaxed);
    let mut callback = Callback::new(
        plan.clone(),
        &meters,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.seek(0);
    callback.renderer.play();
    callback.update();
    let mut reference = metronome::Metronome::new(48_000);
    reference.set_enabled(true);
    let mut audible = false;
    for index in 0..60_000 {
        let sample = callback.next();
        let expected = reference.next(Some(index % 24_000), 120.0);
        assert_eq!(sample, [expected; 2]);
        audible |= sample[0] != 0.0;
    }
    assert!(audible);
    let mut offline = vec![[0.0; 2]; 48_000];
    Renderer::new(plan).render_offline(&mut offline);
    assert!(offline.iter().all(|frame| *frame == [0.0; 2]));
    callback.renderer.pause();
    for _ in 0..242 {
        callback.next();
    }
    assert_eq!(callback.next(), [0.0; 2]);
}

#[test]
fn metronome_reaches_device_conversion_and_stops_after_power_off() {
    for rate in [44_100, 48_000, 96_000] {
        let mut plan = tests::transport_plan();
        plan.project.master.gain_db = daw_core::SILENT_GAIN_DB;
        let meters = track_meters(&plan, &HashMap::new());
        let (_, consumer) = RingBuffer::new(32);
        let (retire, _retired) = RingBuffer::new(64);
        let status = Arc::new(Status::default());
        status.metronome_enabled.store(true, Ordering::Relaxed);
        let mut callback =
            Callback::new(plan, &meters, consumer, retire, status.clone(), rate).unwrap();
        callback.renderer.seek(0);
        callback.renderer.play();
        callback.update();
        assert!((0..4096).any(|_| callback.next()[0].abs() > 0.00001));
        status.metronome_enabled.store(false, Ordering::Relaxed);
        callback.update();
        for _ in 0..rate / 10 {
            callback.next();
        }
        for _ in 0..128 {
            assert_eq!(callback.next(), [0.0; 2]);
        }
    }
}
