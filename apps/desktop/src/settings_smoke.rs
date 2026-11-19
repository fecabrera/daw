//! Packaged debug smoke test for real presentation, close and fallback.
use daw_ui::{DawUi, FileAction, settings::TitlebarStyle};
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
    OpenProjectPrompt(FileAction),
    ProjectPromptOpening(FileAction),
    ProjectPromptClosing(FileAction),
    Finished,
}

pub(super) struct Smoke {
    started: Instant,
    step: Step,
    fault: Option<StartupFault>,
}

impl Smoke {
    pub fn from_arg(arg: &OsStr) -> Option<Self> {
        let (step, fault) = match arg.to_str()? {
            "--settings-smoke-test" => (Step::Open, None),
            "--settings-smoke-test=window" => (Step::Open, Some(StartupFault::Window)),
            "--settings-smoke-test=redraw" => (Step::Open, Some(StartupFault::Redraw)),
            "--project-prompts-smoke-test" => (Step::OpenProjectPrompt(FileAction::Save), None),
            _ => return None,
        };
        Some(Self {
            started: Instant::now(),
            step,
            fault,
        })
    }

    pub fn update(&mut self, ctx: &Context, daw: &mut DawUi, host: &super::settings_host::Host) {
        if matches!(self.step, Step::Finished) {
            return;
        }
        assert!(
            self.started.elapsed() < Duration::from_secs(15),
            "Native dialog smoke test timed out"
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
            Step::OpenProjectPrompt(action) => {
                if action == FileAction::Export {
                    let project = &mut daw.session.project;
                    let track = project.add_midi_track().unwrap();
                    project.add_midi_pattern(track, 0).unwrap();
                }
                daw.perform_file_action(action);
                assert!(daw.desktop_window().is_some());
                self.step = Step::ProjectPromptOpening(action);
            }
            Step::ProjectPromptOpening(action) => {
                if let Some(id) = host.viewport_id()
                    && let Some(Status::Presented(window)) = native_viewport::status(ctx, id)
                    && let Some(window) = window.upgrade()
                {
                    let key = daw.desktop_window().unwrap();
                    let [_, height] = daw.desktop_window_size(key);
                    let actual = f64::from(window.inner_size().height) / window.scale_factor();
                    assert!(window.is_decorated());
                    assert_eq!(
                        window.title(),
                        if action == FileAction::Save {
                            "Save project"
                        } else {
                            "Export audio"
                        },
                    );
                    if height < 145 && (actual - f64::from(height)).abs() <= 1.0 {
                        ctx.send_viewport_cmd_to(id, ViewportCommand::Close);
                        self.step = Step::ProjectPromptClosing(action);
                    }
                }
            }
            Step::ProjectPromptClosing(action) => {
                if daw.desktop_window().is_none() && host.viewport_id().is_none() {
                    if action == FileAction::Save {
                        self.step = Step::OpenProjectPrompt(FileAction::Export);
                    } else {
                        println!(
                            "PASS: native Save and Export title bars, fitted client heights and close"
                        );
                        self.step = Step::Finished;
                        daw.request_desktop_close();
                    }
                }
            }
            Step::Finished => {}
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}
