use super::*;
use std::time::{Duration, Instant};
fn ready(worker: &preparation::Worker) -> preparation::Ready {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(ready) = worker.take() {
            return ready;
        }
        assert!(Instant::now() < deadline, "preparation did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn worker_transport_activates_only_latest_revision_and_preserves_pause_resume_start() {
    let mut plan = tests::transport_plan();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::limiter());
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let worker = preparation::Worker::new().unwrap();
    let (mut producer, consumer) = RingBuffer::new(4);
    let (retire, mut retired) = RingBuffer::new(8);
    let status = Arc::new(Status::default());
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    let mut state = TransportState {
        frame: 8000,
        playing: true,
        paused: false,
        playback_start: Some(4800),
    };
    let stale = worker.submit(plan.clone(), bindings.clone(), Some(state));
    let stale_ready = ready(&worker);
    state.frame = 12_000;
    let revision = worker.submit(plan.clone(), bindings.clone(), Some(state));
    status.requested_revision.store(revision, Ordering::Release);
    producer.push(Command::Plan(stale_ready.renderer)).unwrap();
    callback.update();
    assert_eq!(retired.pop().unwrap().revision, stale);
    assert!(!callback.renderer.playing);
    let prepared = ready(&worker);
    assert!(prepared.renderer.prepared_transport);
    assert_eq!(prepared.renderer.transport_state(), state);
    producer.push(Command::Plan(prepared.renderer)).unwrap();
    callback.update();
    assert_eq!(callback.renderer.transport_state(), state);
    assert_eq!(status.applied_revision.load(Ordering::Acquire), revision);
    for _ in 0..1000 {
        callback.next();
    }
    let at = callback.renderer.playhead;
    status
        .requested_revision
        .store(revision + 1, Ordering::Release);
    producer.push(Command::PauseAt(revision + 1)).unwrap();
    callback.update();
    for _ in 0..300 {
        callback.next();
    }
    assert_eq!(callback.renderer.playhead, at);
    assert!(callback.renderer.transport_state().paused);
    status
        .requested_revision
        .store(revision + 2, Ordering::Release);
    producer.push(Command::ResumeAt(revision + 2)).unwrap();
    callback.update();
    assert_eq!(
        callback.renderer.transport_state().playback_start,
        Some(4800)
    );
    callback.next();
    assert_eq!(callback.renderer.playhead, at + 1);
}
#[test]
fn latency_changes_return_to_worker_and_full_retirement_queue_retains_pending_plan() {
    let mut plan = tests::transport_plan();
    let bindings = preparation::Bindings {
        tracks: track_meters(&plan, &HashMap::new()),
        ..Default::default()
    };
    let worker = preparation::Worker::new().unwrap();
    let (mut producer, consumer) = RingBuffer::new(2);
    let (retire, mut retired) = RingBuffer::new(1);
    let status = Arc::new(Status::default());
    let mut callback = Callback::new(
        plan.clone(),
        &bindings.tracks,
        consumer,
        retire,
        status.clone(),
        48_000,
    )
    .unwrap();
    callback.renderer.play();
    for _ in 0..512 {
        callback.next();
    }
    let captured = callback.renderer.transport_state();
    plan.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::limiter());
    let revision = worker.submit(plan.clone(), bindings.clone(), None);
    status.requested_revision.store(revision, Ordering::Release);
    producer
        .push(Command::Plan(ready(&worker).renderer))
        .unwrap();
    callback.update();
    let mut returned = retired.pop().unwrap();
    assert_eq!(returned.reprepare, Some(captured));
    assert_eq!(callback.renderer.playhead, captured.frame);
    assert!(!callback.renderer.playing);
    let transport = returned.reprepare.take().unwrap();
    worker.reprepare(
        preparation::Ready {
            revision,
            renderer: returned,
            bindings: bindings.clone(),
        },
        transport,
    );
    let prepared = ready(&worker);
    // Fill retirement storage before activation. The callback keeps both renderers alive.
    callback
        .retired
        .push(Box::new(PreparedRenderer::new(plan, &bindings.tracks)))
        .unwrap();
    producer.push(Command::Plan(prepared.renderer)).unwrap();
    callback.update();
    assert!(callback.pending.is_some());
    drop(retired.pop().unwrap());
    callback.update();
    assert!(callback.pending.is_none());
    assert_eq!(callback.renderer.transport_state(), captured);
    assert!(callback.renderer.latency_frames() > 0);
    assert!(retired.pop().is_ok());
}
