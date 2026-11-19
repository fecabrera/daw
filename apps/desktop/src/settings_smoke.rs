//! Packaged debug smoke test for real presentation, close and fallback.
use daw_ui::{DawUi, settings::TitlebarStyle};
use eframe::native_viewport::{self, StartupFault, Status};
use egui::{Context, ViewportCommand, ViewportId};
use std::{
    ffi::OsStr,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Step {
    Open,
    Opening,
    SwitchTitlebar(ViewportId),
    Closing,
    Finished,
}

pub(super) struct Smoke {
    started: Instant,
    step: Step,
    fault: Option<StartupFault>,
}

impl Smoke {
    pub fn from_arg(arg: &OsStr) -> Option<Self> {
        let fault = match arg.to_str()? {
            "--settings-smoke-test" => None,
            "--settings-smoke-test=window" => Some(StartupFault::Window),
            "--settings-smoke-test=redraw" => Some(StartupFault::Redraw),
            _ => return None,
        };
        Some(Self {
            started: Instant::now(),
            step: Step::Open,
            fault,
        })
    }

    pub fn update(&mut self, ctx: &Context, daw: &mut DawUi, host: &super::settings_host::Host) {
        if matches!(self.step, Step::Finished) {
            return;
        }
        assert!(
            self.started.elapsed() < Duration::from_secs(15),
            "Settings smoke test timed out"
        );
        match self.step {
            Step::Open => {
                daw.open_settings();
                assert!(daw.application_settings_open());
                if let Some(fault) = self.fault {
                    let generation = daw.native_settings_generation().unwrap();
                    let id = ViewportId::from_hash_of(("application-settings", generation));
                    native_viewport::inject_fault(ctx, id, fault);
                }
                self.step = Step::Opening;
            }
            Step::Opening if self.fault.is_some() => {
                if daw.native_settings_generation().is_none() {
                    assert!(
                        daw.application_settings_open(),
                        "fallback keeps Settings open"
                    );
                    assert!(
                        !daw.settings_enabled(),
                        "the fallback keeps main editing blocked"
                    );
                    daw.close_application_settings();
                    self.step = Step::Closing;
                }
            }
            Step::Opening => {
                if let Some(id) = host.viewport_id()
                    && let Some(Status::Presented(window)) = native_viewport::status(ctx, id)
                {
                    assert!(window.upgrade().is_some_and(|window| window.is_decorated()));
                    assert!(!daw.settings_enabled(), "Settings blocks main input");
                    let mut preferences = daw.settings();
                    preferences.titlebar = TitlebarStyle::Native;
                    daw.set_settings(ctx, preferences);
                    self.step = Step::SwitchTitlebar(id);
                }
            }
            Step::SwitchTitlebar(id) => {
                assert_eq!(host.viewport_id(), Some(id));
                ctx.send_viewport_cmd_to(id, ViewportCommand::Close);
                self.step = Step::Closing;
            }
            Step::Closing => {
                if !daw.application_settings_open() && host.viewport_id().is_none() {
                    println!("PASS: native Settings presentation, close, or embedded fallback");
                    self.step = Step::Finished;
                    daw.request_desktop_close();
                }
            }
            Step::Finished => {}
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}
