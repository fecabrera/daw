//! Optional cache work yields to editing, playback pressure and required requests.
use daw_engine::CacheProfile;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default)]
pub(super) struct CacheControls {
    pub profile: CacheProfile,
    pub editing: bool,
    pub last_activity: Option<Instant>,
    pub revision: u64,
}
#[derive(Default)]
pub(super) struct WarmSchedule {
    overruns: u64,
    pressure_until: Option<Instant>,
}
impl WarmSchedule {
    pub fn overrun_count(&self) -> u64 {
        self.overruns
    }
    pub fn delay(
        &mut self,
        controls: CacheControls,
        playing: bool,
        overruns: u64,
        now: Instant,
    ) -> Duration {
        if overruns != self.overruns {
            self.overruns = overruns;
            self.pressure_until = Some(now + Duration::from_secs(1));
        }
        let policy = controls.profile.policy();
        let mut delay = self
            .pressure_until
            .map_or(Duration::ZERO, |until| until.saturating_duration_since(now));
        if controls.editing || (playing && !policy.warm_while_playing) {
            delay = delay.max(Duration::from_millis(100));
        }
        if let Some(activity) = controls.last_activity {
            delay = delay.max((activity + policy.edit_quiet).saturating_duration_since(now));
        }
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_gate_optional_work_after_edits_during_playback_and_gestures() {
        let now = Instant::now();
        for profile in CacheProfile::ALL {
            let mut schedule = WarmSchedule::default();
            let mut controls = CacheControls {
                profile,
                last_activity: Some(now),
                ..Default::default()
            };
            assert!(schedule.delay(controls, false, 0, now) > Duration::ZERO);
            let settled = now + profile.policy().edit_quiet;
            assert!(schedule.delay(controls, false, 0, settled).is_zero());
            assert_eq!(
                schedule.delay(controls, true, 0, settled).is_zero(),
                profile != CacheProfile::Light
            );
            controls.editing = true;
            assert!(schedule.delay(controls, false, 0, settled) > Duration::ZERO);
            // Releasing a gesture starts a fresh quiet interval.
            controls.editing = false;
            controls.last_activity = Some(settled);
            assert!(schedule.delay(controls, false, 0, settled) > Duration::ZERO);
            assert!(
                schedule
                    .delay(controls, false, 0, settled + profile.policy().edit_quiet)
                    .is_zero()
            );
        }
    }
    #[test]
    fn callback_overruns_pause_optional_work_and_further_overruns_extend_the_pause() {
        let now = Instant::now();
        let controls = CacheControls::default();
        let mut schedule = WarmSchedule::default();
        assert!(schedule.delay(controls, true, 0, now).is_zero());
        assert!(!schedule.delay(controls, true, 1, now).is_zero());
        assert!(
            !schedule
                .delay(controls, true, 1, now + Duration::from_millis(999))
                .is_zero()
        );
        assert!(
            !schedule
                .delay(controls, true, 2, now + Duration::from_millis(999))
                .is_zero()
        );
        assert!(
            schedule
                .delay(controls, true, 2, now + Duration::from_millis(1999))
                .is_zero()
        );
    }
}
