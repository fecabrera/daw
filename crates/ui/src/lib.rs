use daw_core::{Clip, Edit, Id, frames, seconds};
use daw_output::AudioOutput;
use daw_project::Session;
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};

pub mod dialogs;
mod file_drop;
pub mod fonts;
pub mod icons;
pub mod knobs;
pub mod meters;
mod musical_time;
pub mod panels;
pub mod rows;
pub mod theme;
pub mod toolbars;

const TRACK_WIDTH: f32 = 250.0;
const ROW_HEIGHT: f32 = toolbars::CONTROL_HEIGHT + 4.0 + knobs::SIZE + 14.0;
const MASTER_HEIGHT: f32 = ROW_HEIGHT;
const RULER_HEIGHT: f32 = toolbars::HEIGHT;

fn numeric_input(ui: &mut egui::Ui, text: &mut String, tooltip: &str) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .text_color(theme::TEXT)
            .font(egui::TextStyle::Small)
            .horizontal_align(egui::Align::Center)
            .desired_width(36.0),
    )
    .on_hover_text(tooltip)
}

#[derive(PartialEq, Eq)]
enum InlineEditAction {
    Commit,
    Cancel,
}

fn inline_edit_action(ui: &mut egui::Ui, response: &egui::Response) -> Option<InlineEditAction> {
    if !ui.is_enabled() || !ui.input(|input| input.focused) {
        return Some(InlineEditAction::Cancel);
    }
    if response.has_focus() || response.lost_focus() {
        let (cancel, commit) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        });
        if cancel {
            return Some(InlineEditAction::Cancel);
        }
        // Single-line inputs also lose focus on Enter. Commit takes precedence.
        if commit {
            return Some(InlineEditAction::Commit);
        }
        if response.lost_focus() {
            return Some(InlineEditAction::Cancel);
        }
    }
    None
}

fn transport_time(frame: u64) -> String {
    let rate = u64::from(daw_core::SAMPLE_RATE);
    let total_seconds = frame / rate;
    let hours = total_seconds / 3600;
    let minutes = total_seconds / 60 % 60;
    let seconds = total_seconds % 60;
    let hundredths = frame % rate * 100 / rate;
    format!("{hours:02}h{minutes:02}m{seconds:02}.{hundredths:02}s")
}

struct MonitorResponse {
    response: egui::Response,
    digit_bounds: Vec<Rect>,
}

fn transport_monitor_control(
    ui: &mut egui::Ui,
    text: &str,
    tooltip: &str,
    minimum_digits: usize,
    sense: Sense,
) -> MonitorResponse {
    let font = fonts::semibold(13.0);
    // Digits share a fixed advance; unit letters and punctuation keep theirs.
    let digit_count = text.chars().filter(char::is_ascii_digit).count();
    let digit_slots = digit_count.max(minimum_digits);
    let (digit_width, field_width) = ui.fonts_mut(|fonts| {
        let digit_width = "0123456789"
            .chars()
            .map(|c| fonts.glyph_width(&font, c))
            .fold(0.0_f32, f32::max)
            .ceil();
        let unit_width: f32 = text
            .chars()
            .filter(|character| !character.is_ascii_digit())
            .map(|c| {
                fonts
                    .layout(c.to_string(), font.clone(), theme::TIME_UNIT, f32::INFINITY)
                    .size()
                    .x
            })
            .sum();
        (
            digit_width,
            digit_slots as f32 * digit_width + unit_width + 2.0 * f32::from(theme::TOOLBAR_PADDING),
        )
    });
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(field_width, ui.spacing().interact_size.y), sense);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    let response = response.on_hover_text(tooltip);
    let painter = ui.painter().with_clip_rect(rect);
    let mut x = rect.left()
        + f32::from(theme::TOOLBAR_PADDING)
        + (digit_slots - digit_count) as f32 * digit_width;
    let mut digit_bounds: Vec<Rect> = Vec::new();
    let mut in_digits = false;
    for character in text.chars() {
        let is_digit = character.is_ascii_digit();
        let color = if is_digit {
            theme::TEXT
        } else {
            theme::TIME_UNIT
        };
        let galley = painter.layout_no_wrap(character.to_string(), font.clone(), color);
        let width = if is_digit {
            digit_width
        } else {
            galley.size().x
        };
        if is_digit {
            if in_digits {
                digit_bounds.last_mut().unwrap().max.x = x + width;
            } else {
                digit_bounds.push(Rect::from_min_max(
                    Pos2::new(x, rect.top()),
                    Pos2::new(x + width, rect.bottom()),
                ));
            }
        }
        in_digits = is_digit;
        painter.galley(
            Pos2::new(
                x + (width - galley.size().x) / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        x += width;
    }
    if let Some(first) = digit_bounds.first_mut() {
        first.min.x = rect.left();
    }
    if let Some(last) = digit_bounds.last_mut() {
        last.max.x = rect.right();
    }
    MonitorResponse {
        response,
        digit_bounds,
    }
}

enum Job {
    Loaded(Session, bool),
    Saved(Session),
    Exported(Vec<String>),
}

enum Action {
    New,
    Open(PathBuf),
    CloseProject,
    CloseWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    New,
    Open,
    Import,
    Save,
    SaveAs,
    Export,
    CloseProject,
}
impl FileAction {
    pub const ALL: [Self; 7] = [
        Self::New,
        Self::Open,
        Self::Import,
        Self::Save,
        Self::SaveAs,
        Self::Export,
        Self::CloseProject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::New => "New project",
            Self::Open => "Open…",
            Self::Import => "Import WAV…",
            Self::Save => "Save",
            Self::SaveAs => "Save as…",
            Self::Export => "Export WAV…",
            Self::CloseProject => "Close project",
        }
    }

    pub fn shortcut(self) -> egui::KeyboardShortcut {
        let (key, shift) = match self {
            Self::New => (egui::Key::N, false),
            Self::Open => (egui::Key::O, false),
            Self::Import => (egui::Key::I, true),
            Self::Save => (egui::Key::S, false),
            Self::SaveAs => (egui::Key::S, true),
            Self::Export => (egui::Key::E, true),
            Self::CloseProject => (egui::Key::W, false),
        };
        egui::KeyboardShortcut::new(
            egui::Modifiers {
                shift,
                ..egui::Modifiers::COMMAND
            },
            key,
        )
    }
}

struct Inputs {
    gain: String,
    pan: String,
}
struct TrackNameEdit {
    track: Id,
    text: String,
    focus: bool,
}
struct TempoEdit {
    text: String,
    focus: bool,
    width: f32,
}
#[derive(Clone, Copy)]
enum PositionStep {
    Beats { count: i64, tempo: f32 },
    Frames(u64),
}

impl PositionStep {
    fn shift(self, frame: u64, steps: i64) -> u64 {
        match self {
            Self::Beats { count, tempo } => {
                musical_time::shift(frame, tempo, steps.saturating_mul(count))
            }
            Self::Frames(count) => (i128::from(frame) + i128::from(steps) * i128::from(count))
                .clamp(0, i128::from(u64::MAX)) as u64,
        }
    }
}

fn musical_steps(tempo: f32) -> [PositionStep; 2] {
    [
        PositionStep::Beats {
            count: musical_time::BEATS_PER_BAR as i64,
            tempo,
        },
        PositionStep::Beats { count: 1, tempo },
    ]
}

struct MonitorDrag {
    frame: u64,
    units: f64,
    applied: i64,
    step: PositionStep,
}

fn musical_sections(
    ui: &egui::Ui,
    monitor: &MonitorResponse,
    offset: usize,
) -> [egui::Response; 2] {
    std::array::from_fn(|index| {
        ui.interact(
            monitor.digit_bounds[offset + index],
            monitor.response.id.with(offset + index),
            Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .on_hover_text(if index == 0 {
            "Drag up or down by bars. Hold Shift for slower adjustment."
        } else {
            "Drag up or down by beats. Beats carry across bars. Hold Shift for slower adjustment."
        })
    })
}

fn monitor_drag_position(
    ui: &egui::Ui,
    responses: &[egui::Response],
    drag: &mut Option<MonitorDrag>,
    frame: u64,
    steps: &[PositionStep],
    limits: std::ops::RangeInclusive<u64>,
) -> Option<u64> {
    let active = responses.iter().enumerate().find(|(_, response)| {
        response.dragged_by(egui::PointerButton::Primary)
            && ui.is_enabled()
            && ui.input(|input| input.focused)
    });
    let Some((index, response)) = active else {
        *drag = None;
        return None;
    };
    let state = drag.get_or_insert(MonitorDrag {
        frame,
        units: 0.0,
        applied: 0,
        step: steps[index],
    });
    let fine = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
    state.units -= f64::from(response.drag_delta().y) * fine;
    // Accumulated tenths can fall a rounding error short of a whole unit.
    let steps = (state.units + state.units.signum() * 1e-9).trunc() as i64;
    if steps == state.applied {
        return None;
    }
    let destination = state
        .step
        .shift(state.frame, steps)
        .clamp(*limits.start(), *limits.end());
    state.applied = steps;
    if (destination == *limits.start() && steps < 0) || (destination == *limits.end() && steps > 0)
    {
        state.frame = destination;
        state.units = 0.0;
        state.applied = 0;
    }
    Some(destination)
}
struct Drag {
    clip: Clip,
    track: Id,
    mode: u8,
    origin: Pos2,
}
struct MovePreview {
    track: Id,
    start: u64,
    valid: bool,
}
struct FileDropTarget {
    track: Option<Id>,
    start: u64,
    lane: Rect,
}
struct LoopDrag {
    start: u64,
    original: daw_core::Loop,
}

pub struct DawUi {
    pub session: Session,
    output: Option<AudioOutput>,
    selected_track: Option<Id>,
    selected_clip: Option<Id>,
    inputs: HashMap<Id, Inputs>,
    track_name_edit: Option<TrackNameEdit>,
    master_gain_input: String,
    tempo_edit: Option<TempoEdit>,
    time_drag: Option<MonitorDrag>,
    musical_drag: Option<MonitorDrag>,
    selection_drag: [Option<MonitorDrag>; 2],
    zoom: f32,
    scroll: f64,
    drag: Option<Drag>,
    file_hover: Option<file_drop::FileHover>,
    file_drop_target: Option<FileDropTarget>,
    loop_drag: Option<LoopDrag>,
    lane_bounds: HashMap<Id, Rect>,
    #[cfg(test)]
    scrollbar_bounds: Rect,
    #[cfg(test)]
    master_bounds: Rect,
    #[cfg(test)]
    tempo_bounds: Rect,
    #[cfg(test)]
    musical_bounds: [Rect; 2],
    #[cfg(test)]
    time_bounds: [Rect; 3],
    #[cfg(test)]
    selection_bounds: [Rect; 4],
    dirty: bool,
    sync_needed: bool,
    job: Option<mpsc::Receiver<Result<Job, String>>>,
    busy: String,
    notices: Vec<String>,
    error: Option<String>,
    pending: Option<Action>,
    unsaved_prompt: bool,
    overwrite: Option<PathBuf>,
    allow_close: bool,
    native_file_menu: bool,
    window_title: String,
}
impl Default for DawUi {
    fn default() -> Self {
        Self {
            session: Session::default(),
            output: None,
            selected_track: None,
            selected_clip: None,
            inputs: HashMap::new(),
            track_name_edit: None,
            master_gain_input: "0.0".into(),
            tempo_edit: None,
            musical_drag: None,
            time_drag: None,
            selection_drag: [None, None],
            zoom: 70.0,
            scroll: 0.0,
            drag: None,
            file_hover: None,
            file_drop_target: None,
            loop_drag: None,
            lane_bounds: HashMap::new(),
            #[cfg(test)]
            scrollbar_bounds: Rect::NOTHING,
            #[cfg(test)]
            master_bounds: Rect::NOTHING,
            #[cfg(test)]
            tempo_bounds: Rect::NOTHING,
            #[cfg(test)]
            musical_bounds: [Rect::NOTHING; 2],
            #[cfg(test)]
            time_bounds: [Rect::NOTHING; 3],
            #[cfg(test)]
            selection_bounds: [Rect::NOTHING; 4],
            dirty: false,
            sync_needed: false,
            job: None,
            busy: String::new(),
            notices: vec![],
            error: None,
            pending: None,
            unsaved_prompt: false,
            overwrite: None,
            allow_close: false,
            native_file_menu: false,
            window_title: String::new(),
        }
    }
}
impl DawUi {
    /// Let the desktop adapter provide file menus and their keyboard shortcuts.
    pub fn use_native_file_menu(&mut self) {
        self.native_file_menu = true;
    }

    pub fn new(project: Option<PathBuf>) -> Self {
        let mut app = Self::default();
        if let Some(folder) = project {
            app.open(folder);
        }
        app
    }
    fn fail(&mut self, error: impl ToString) {
        self.error = Some(error.to_string());
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.sync_needed = true;
    }
    fn run_job(&mut self, label: &str, job: impl FnOnce() -> Result<Job, String> + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.busy = label.into();
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
    }
    fn open(&mut self, folder: PathBuf) {
        self.run_job("Opening project", move || {
            Session::open(&folder)
                .map(|s| Job::Loaded(s, false))
                .map_err(|e| e.to_string())
        });
    }
    fn request(&mut self, action: Action) {
        self.pending = Some(action);
        if self.dirty {
            self.unsaved_prompt = true;
        } else {
            self.perform_pending();
        }
    }
    fn perform_pending(&mut self) {
        match self.pending.take() {
            Some(Action::New | Action::CloseProject) => {
                self.output = None;
                self.session = Session::default();
                self.dirty = false;
                self.inputs.clear();
                self.track_name_edit = None;
                self.master_gain_input = format!("{:.1}", self.session.project.master.gain_db);
                self.tempo_edit = None;
                self.musical_drag = None;
                self.time_drag = None;
                self.selection_drag = [None, None];
                self.selected_track = None;
                self.selected_clip = None;
                self.notices.clear();
                self.scroll = 0.0;
                self.drag = None;
                self.file_hover = None;
                self.file_drop_target = None;
                self.loop_drag = None;
                self.lane_bounds.clear();
                self.sync_needed = false;
            }
            Some(Action::Open(p)) => self.open(p),
            Some(Action::CloseWindow) => self.allow_close = true,
            None => {}
        }
    }
    fn save(&mut self, save_as: bool) {
        let folder = if save_as || self.session.folder.is_none() {
            rfd::FileDialog::new()
                .set_title("Save project folder")
                .pick_folder()
        } else {
            self.session.folder.clone()
        };
        let Some(folder) = folder else {
            return;
        };
        let mut session = self.session.clone();
        self.run_job("Saving project", move || {
            session
                .save(&folder)
                .map(|()| Job::Saved(session))
                .map_err(|e| e.to_string())
        });
    }
    fn import(&mut self, path: PathBuf) {
        let track = self
            .selected_track
            .or_else(|| self.session.project.tracks.first().map(|track| track.id));
        self.import_at(
            path,
            track,
            self.session.project.transport.playhead_frame,
            None,
        );
    }
    fn import_at(
        &mut self,
        path: PathBuf,
        track: Option<Id>,
        at: u64,
        prepared: Option<file_drop::FileHover>,
    ) {
        let mut session = self.session.clone();
        self.run_job("Importing WAV and building waveform", move || {
            let audio = match prepared {
                Some(prepared) => prepared.into_audio()?,
                None => daw_media::decode_wav(&path).map_err(|error| error.to_string())?,
            };
            session
                .import_decoded(&path, audio, track, at)
                .map(|_| Job::Loaded(session, true))
                .map_err(|error| error.to_string())
        });
    }
    fn update_file_hover(&mut self, ctx: &egui::Context) {
        self.file_drop_target = None;
        let path = if self.file_action_enabled(FileAction::Import) {
            ctx.input(|input| {
                input
                    .raw
                    .hovered_files
                    .first()
                    .and_then(|file| file.path.clone())
                    .or_else(|| {
                        input
                            .raw
                            .dropped_files
                            .first()
                            .map(|file| file.path().to_path_buf())
                    })
            })
        } else {
            None
        };
        match path {
            Some(path) => {
                if self
                    .file_hover
                    .as_ref()
                    .is_none_or(|hover| hover.path != path)
                {
                    self.file_hover = Some(file_drop::FileHover::new(path));
                }
                self.file_hover.as_mut().unwrap().poll();
            }
            None => self.file_hover = None,
        }
    }
    fn export(&mut self, path: PathBuf, overwrite: bool) {
        let session = self.session.clone();
        self.notices = session.warnings.clone();
        self.run_job("Exporting stereo WAV", move || {
            session
                .export(&path, overwrite)
                .map(Job::Exported)
                .map_err(|e| e.to_string())
        });
    }
    fn poll(&mut self) {
        let message = self.job.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(m) => Some(m),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Background operation stopped unexpectedly".into()))
            }
            Err(_) => None,
        });
        if let Some(message) = message {
            self.job = None;
            self.busy.clear();
            match message {
                Ok(Job::Loaded(session, dirty)) => {
                    if !dirty {
                        self.output = None;
                        self.inputs.clear();
                        self.track_name_edit = None;
                        self.master_gain_input = format!("{:.1}", session.project.master.gain_db);
                        self.tempo_edit = None;
                        self.musical_drag = None;
                        self.time_drag = None;
                        self.selection_drag = [None, None];
                        self.scroll = 0.0;
                    }
                    self.notices = session.warnings.clone();
                    self.session = session;
                    self.selected_track = self
                        .selected_track
                        .filter(|id| self.session.project.tracks.iter().any(|t| t.id == *id))
                        .or_else(|| self.session.project.tracks.first().map(|t| t.id));
                    self.dirty = dirty;
                    self.sync_needed = true;
                }
                Ok(Job::Saved(session)) => {
                    self.session = session;
                    self.dirty = false;
                    self.unsaved_prompt = false;
                    self.perform_pending();
                }
                Ok(Job::Exported(warnings)) => {
                    self.notices = warnings;
                    self.notices.push("Export complete".into());
                }
                Err(e) => {
                    self.fail(e);
                    self.pending = None;
                    self.unsaved_prompt = false;
                }
            }
        }
    }
    fn seek(&mut self, frame: u64) {
        self.session.project.transport.playhead_frame = frame;
        self.dirty = true;
        if let Some(output) = &mut self.output
            && let Err(e) = output.seek(frame)
        {
            self.fail(e);
        }
    }
    fn play(&mut self) {
        if self.session.project.end() == 0 {
            return;
        }
        if self
            .output
            .as_ref()
            .is_some_and(|o| o.status.failed.load(Ordering::Relaxed))
        {
            self.output = None;
        }
        if self.output.is_none() {
            match AudioOutput::new(self.session.plan()) {
                Ok(o) => self.output = Some(o),
                Err(e) => {
                    self.fail(e);
                    return;
                }
            }
        }
        if let Some(output) = &mut self.output
            && let Err(e) = output.play()
        {
            self.fail(e);
        }
    }
    fn is_playing(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|o| o.status.playing.load(Ordering::Relaxed))
    }
    fn toggle_playback(&mut self) {
        if self.is_playing() {
            if let Some(output) = &mut self.output
                && let Err(e) = output.pause()
            {
                self.fail(e);
            }
        } else {
            self.play();
        }
    }
    fn stop(&mut self) {
        if let Some(output) = &mut self.output
            && let Err(e) = output.stop()
        {
            self.fail(e);
        }
    }
    fn edit(&mut self, command: Edit) {
        match self.session.project.edit(command) {
            Ok(()) => {
                if self.track_name_edit.as_ref().is_some_and(|edit| {
                    !self
                        .session
                        .project
                        .tracks
                        .iter()
                        .any(|t| t.id == edit.track)
                }) {
                    self.track_name_edit = None;
                }
                self.changed();
            }
            Err(e) => self.fail(e),
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll();
        self.update_file_hover(&ctx);
        if !self.native_file_menu
            && let Some(action) = self.take_file_shortcut(&ctx)
        {
            self.perform_file_action(action);
        }
        if let Some(output) = &mut self.output {
            output.drain();
            self.session.project.transport.playhead_frame =
                output.status.playhead.load(Ordering::Relaxed);
            if output.status.failed.load(Ordering::Relaxed) {
                self.error=Some("Audio output failed or disconnected. Playback stopped. Editing and export remain available.".into());
                self.output = None;
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.job.is_none() {
                self.request(Action::CloseWindow);
            }
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show_separator_line(true)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            rows::centered(ui, toolbars::CONTROL_HEIGHT, |ui| {
                if self.job.is_some() {
                    ui.spinner();
                    ui.label(&self.busy);
                } else {
                    ui.label(if self.dirty {
                        "Unsaved changes"
                    } else {
                        "Ready"
                    });
                }
                ui.separator();
                ui.label("48 kHz · WAV");
                if let Some(output) = &self.output {
                    ui.separator();
                    ui.label(&output.description);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(self.job.is_none(), |ui| self.selection_control(ui));
                    ui.label("Selection");
                });
            });
            for notice in &self.notices {
                ui.colored_label(theme::WARNING, notice);
            }
            if !self.notices.is_empty() && ui.small_button("Dismiss notices").clicked() {
                self.notices.clear();
            }
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND))
            .show(ui, |ui| {
                ui.add_enabled_ui(self.job.is_none(), |ui| self.workspace(ui));
            });
        if self.sync_needed {
            let plan = self.session.plan();
            if let Some(output) = &mut self.output {
                if let Ok(()) = output.sync(plan) {
                    self.sync_needed = false
                }
            } else {
                self.sync_needed = false;
            }
        }
        self.dialogs(&ctx);
        if self.job.is_none() {
            if self.file_action_enabled(FileAction::Import)
                && let Some(dropped) = ctx.input(|i| i.raw.dropped_files.first().cloned())
                && let Some(target) = self.file_drop_target.take()
            {
                let path = dropped.path().to_path_buf();
                let prepared = self.file_hover.take().filter(|hover| hover.path == path);
                self.selected_track = target.track;
                self.import_at(path, target.track, target.start, prepared);
            }
            if !ctx.egui_wants_keyboard_input() {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.toggle_playback();
                }
                if ctx.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                }) && let Some(id) = self.selected_clip
                {
                    self.edit(Edit::DeleteClip(id));
                    self.selected_clip = None;
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
        for action in FileAction::ALL {
            if matches!(
                action,
                FileAction::Import
                    | FileAction::Save
                    | FileAction::Export
                    | FileAction::CloseProject
            ) {
                ui.separator();
            }
            let button = egui::Button::new(action.label())
                .shortcut_text(ui.ctx().format_shortcut(&action.shortcut()));
            if ui
                .add_enabled(self.file_action_enabled(action), button)
                .clicked()
            {
                ui.close();
                self.perform_file_action(action);
            }
        }
    }

    pub fn file_action_enabled(&self, action: FileAction) -> bool {
        self.job.is_none()
            && !self.unsaved_prompt
            && self.overwrite.is_none()
            && self.error.is_none()
            && (action != FileAction::Export || self.session.project.end() > 0)
    }

    fn take_file_shortcut(&self, ctx: &egui::Context) -> Option<FileAction> {
        // Match Save as before Save because egui permits extra Shift modifiers.
        FileAction::ALL.into_iter().rev().find(|action| {
            self.file_action_enabled(*action)
                && ctx.input_mut(|input| input.consume_shortcut(&action.shortcut()))
        })
    }

    pub fn perform_file_action(&mut self, action: FileAction) {
        if !self.file_action_enabled(action) {
            return;
        }
        match action {
            FileAction::New => self.request(Action::New),
            FileAction::CloseProject => self.request(Action::CloseProject),
            FileAction::Open => {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.request(Action::Open(p));
                }
            }
            FileAction::Import => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("WAV audio", &["wav"])
                    .pick_file()
                {
                    self.import(p);
                }
            }
            FileAction::Save => self.save(false),
            FileAction::SaveAs => self.save(true),
            FileAction::Export => {
                let Some(p) = rfd::FileDialog::new()
                    .set_file_name("mix.wav")
                    .add_filter("WAV audio", &["wav"])
                    .save_file()
                else {
                    return;
                };
                if p.exists() {
                    self.overwrite = Some(p);
                } else {
                    self.export(p, false);
                }
            }
        }
    }
    fn tempo_control(&mut self, ui: &mut egui::Ui) {
        let previous = self.session.project.tempo_bpm;
        if let Some(mut edit) = self.tempo_edit.take() {
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(ui.make_persistent_id("tempo"))
                .font(fonts::semibold(13.0))
                .text_color(theme::TEXT)
                .horizontal_align(egui::Align::Center)
                .desired_width(edit.width)
                .min_size(Vec2::new(edit.width, toolbars::CONTROL_HEIGHT))
                .show(ui);
            output.response.widget_info(|| {
                egui::WidgetInfo::text_edit(
                    ui.is_enabled(),
                    previous.to_string(),
                    &edit.text,
                    "Tempo (BPM)",
                )
            });
            #[cfg(test)]
            {
                self.tempo_bounds = output.response.rect;
            }
            if edit.focus && ui.is_enabled() {
                output.response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(edit.text.chars().count()),
                    )));
                output.state.store(ui.ctx(), output.response.id);
                edit.focus = false;
            }
            if let Some(action) = inline_edit_action(ui, &output.response) {
                output.response.surrender_focus();
                if action == InlineEditAction::Commit {
                    match edit.text.parse::<f32>() {
                        Ok(value) if value.is_finite() && value > 0.0 => {
                            self.session.project.tempo_bpm = value;
                            if value != previous {
                                self.changed();
                            }
                        }
                        _ => self.fail("Tempo must be a positive finite BPM value"),
                    }
                }
            } else {
                self.tempo_edit = Some(edit);
            }
        } else {
            let response = transport_monitor_control(
                ui,
                &format!("{previous}bpm"),
                "Tempo (BPM): drag up or down to adjust; hold Shift for fine adjustment. Double-click to edit.",
                3,
                Sense::click_and_drag(),
            )
            .response
            .on_hover_cursor(egui::CursorIcon::ResizeVertical);
            #[cfg(test)]
            {
                self.tempo_bounds = response.rect;
            }
            if response.double_clicked() {
                self.tempo_edit = Some(TempoEdit {
                    text: previous.to_string(),
                    focus: true,
                    width: response.rect.width(),
                });
            } else if response.dragged_by(egui::PointerButton::Primary)
                && ui.is_enabled()
                && ui.input(|input| input.focused)
            {
                let delta = response.drag_delta().y;
                if delta != 0.0 {
                    let step = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
                    // Round drag changes to hundredths to avoid displaying float noise.
                    let adjusted =
                        ((f64::from(previous) - f64::from(delta) * step) * 100.0).round() / 100.0;
                    let value = adjusted.clamp(0.01, f64::from(f32::MAX)) as f32;
                    if value != previous {
                        self.session.project.tempo_bpm = value;
                        self.changed();
                    }
                }
            }
        }
    }
    fn time_control(&mut self, ui: &mut egui::Ui) {
        let frame = self.session.project.transport.playhead_frame;
        let monitor =
            transport_monitor_control(ui, &transport_time(frame), "Time", 0, Sense::hover());
        let responses: [egui::Response; 3] = std::array::from_fn(|index| {
            // Seconds include the decimal fraction; each unit includes its suffix.
            let right = if index < 2 {
                monitor.digit_bounds[index + 1].left()
            } else {
                monitor.response.rect.right()
            };
            let bounds = Rect::from_min_max(
                monitor.digit_bounds[index].min,
                Pos2::new(right, monitor.response.rect.bottom()),
            );
            ui.interact(bounds, monitor.response.id.with(index), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                .on_hover_text(format!(
                    "Drag up or down by {}. Hold Shift for slower adjustment.",
                    ["hours", "minutes", "seconds"][index],
                ))
        });
        #[cfg(test)]
        {
            self.time_bounds = responses.each_ref().map(|response| response.rect);
        }
        let steps = [3600, 60, 1]
            .map(|seconds| PositionStep::Frames(seconds * u64::from(daw_core::SAMPLE_RATE)));
        if let Some(destination) = monitor_drag_position(
            ui,
            &responses,
            &mut self.time_drag,
            frame,
            &steps,
            0..=u64::MAX,
        ) && destination != frame
        {
            self.seek(destination);
        }
    }
    fn musical_control(&mut self, ui: &mut egui::Ui) {
        let frame = self.session.project.transport.playhead_frame;
        let tempo = self.session.project.tempo_bpm;
        let monitor = transport_monitor_control(
            ui,
            &musical_time::monitor(frame, tempo),
            "Bars and beats (4/4)",
            0,
            Sense::hover(),
        );
        let responses = musical_sections(ui, &monitor, 0);
        #[cfg(test)]
        {
            self.musical_bounds = responses.each_ref().map(|response| response.rect);
        }
        if let Some(destination) = monitor_drag_position(
            ui,
            &responses,
            &mut self.musical_drag,
            frame,
            &musical_steps(tempo),
            0..=u64::MAX,
        ) && destination != frame
        {
            self.seek(destination);
        }
    }
    fn selection_control(&mut self, ui: &mut egui::Ui) {
        let tempo = self.session.project.tempo_bpm;
        let region = &self.session.project.transport.r#loop;
        let text = format!(
            "{}–{}",
            musical_time::monitor(region.start_frame, tempo),
            musical_time::monitor(region.end_frame, tempo),
        );
        let monitor = transport_monitor_control(
            ui,
            &text,
            "Selection range in bars and beats (4/4)",
            0,
            Sense::hover(),
        );
        for endpoint in 0..2 {
            let responses = musical_sections(ui, &monitor, endpoint * 2);
            #[cfg(test)]
            {
                for (section, response) in responses.iter().enumerate() {
                    self.selection_bounds[endpoint * 2 + section] = response.rect;
                }
            }
            let region = &self.session.project.transport.r#loop;
            let gap = u64::from(region.enabled);
            let (frame, limits) = if endpoint == 0 {
                (region.start_frame, 0..=region.end_frame.saturating_sub(gap))
            } else {
                (
                    region.end_frame,
                    region.start_frame.saturating_add(gap)..=u64::MAX,
                )
            };
            if let Some(destination) = monitor_drag_position(
                ui,
                &responses,
                &mut self.selection_drag[endpoint],
                frame,
                &musical_steps(tempo),
                limits,
            ) && destination != frame
            {
                let region = &mut self.session.project.transport.r#loop;
                if endpoint == 0 {
                    region.start_frame = destination;
                } else {
                    region.end_frame = destination;
                }
                self.changed();
            }
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let window_title = format!(
            "{}{} - DAW",
            self.session.project.name,
            if self.dirty { " *" } else { "" }
        );
        if self.window_title != window_title {
            self.window_title = window_title;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(self.window_title.clone()));
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        toolbars::Toolbar::row(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // Keep the native macOS window buttons clear of application controls.
                #[cfg(target_os = "macos")]
                ui.add_space(78.0);
                if !self.native_file_menu {
                    ui.menu_button("File", |ui| self.file_menu(ui));
                    ui.separator();
                }
                let title = ui.label(&self.window_title);
                #[cfg(target_os = "macos")]
                Self::drag_window(title.interact(Sense::drag()));
                #[cfg(not(target_os = "macos"))]
                let _ = title;
                #[cfg(target_os = "macos")]
                {
                    let (_, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width().max(0.0), ui.spacing().interact_size.y),
                        Sense::drag(),
                    );
                    Self::drag_window(response);
                }
            });
        });
        let (divider, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        let divider_y = divider.center().y;
        let bounds = ui.clip_rect();
        ui.painter().line_segment(
            [
                Pos2::new(bounds.left(), divider_y),
                Pos2::new(bounds.right(), divider_y),
            ],
            Stroke::new(1.0_f32, theme::BORDER),
        );
        toolbars::Toolbar::row(ui, |ui| {
            let playing = self.is_playing();
            if icons::button(
                ui,
                if playing {
                    icons::Lucide::Pause
                } else {
                    icons::Lucide::Play
                },
                if playing { "Pause" } else { "Play" },
                playing || self.session.project.end() > 0,
                theme::ACCENT,
            )
            .clicked()
            {
                self.toggle_playback();
            }
            if icons::button(ui, icons::Lucide::Square, "Stop", true, theme::TEXT).clicked() {
                self.stop();
            }
            let enabled = self.session.project.transport.r#loop.enabled;
            if icons::toggle_button(
                ui,
                icons::Lucide::Repeat,
                "Loop",
                self.job.is_none(),
                enabled,
            )
            .clicked()
            {
                let enabled = !enabled;
                if enabled
                    && self.session.project.transport.r#loop.end_frame
                        <= self.session.project.transport.r#loop.start_frame
                {
                    self.fail("Drag on the time ruler to select a loop region");
                } else {
                    self.session.project.transport.r#loop.enabled = enabled;
                    self.changed();
                }
            }
            ui.add_enabled_ui(self.job.is_none(), |ui| self.time_control(ui));
            ui.add_enabled_ui(self.job.is_none(), |ui| self.musical_control(ui));
            ui.add_enabled_ui(self.job.is_none(), |ui| self.tempo_control(ui));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.scope(|ui| {
                    ui.spacing_mut().slider_width = 56.0;
                    ui.spacing_mut().slider_rail_height = 4.0;
                    ui.spacing_mut().interact_size.y = 14.0;
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Body, egui::FontId::proportional(11.0));
                    let response = ui.add(
                        egui::Slider::new(&mut self.zoom, 1.0..=3000.0)
                            .logarithmic(true)
                            .show_value(false)
                            .trailing_fill(false)
                            .handle_shape(egui::style::HandleShape::Circle),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::slider(
                            ui.is_enabled(),
                            f64::from(self.zoom),
                            "Horizontal zoom",
                        )
                    });
                    response
                        .on_hover_text("Horizontal zoom: drag left to zoom out, right to zoom in");
                });
                ui.add(
                    icons::Lucide::MoveHorizontal
                        .size(16.0)
                        .stroke_width(2.0)
                        .color(theme::TEXT)
                        .image()
                        .alt_text("Horizontal zoom"),
                )
                .on_hover_text("Horizontal zoom");
            });
        });
    }
    #[cfg(target_os = "macos")]
    fn drag_window(response: egui::Response) {
        if response.drag_started_by(egui::PointerButton::Primary) {
            response
                .ctx
                .send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
    fn workspace(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let bounds = ui.max_rect();
        let timeline_left = bounds.left() + TRACK_WIDTH;
        let painter = ui.painter().clone();
        let track_surface =
            Rect::from_min_max(bounds.min, Pos2::new(timeline_left, bounds.bottom()));
        painter.rect_filled(track_surface, 0.0, theme::PANEL);
        painter.line_segment(
            [track_surface.right_top(), track_surface.right_bottom()],
            Stroke::new(1.0_f32, theme::BORDER),
        );

        let master_rect = Rect::from_min_max(
            Pos2::new(
                bounds.left(),
                (bounds.bottom() - MASTER_HEIGHT).max(bounds.top()),
            ),
            Pos2::new(timeline_left, bounds.bottom()),
        );
        #[cfg(test)]
        {
            self.master_bounds = master_rect;
        }
        let mut master_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("master_track")
                .max_rect(master_rect),
        );
        master_ui.set_clip_rect(master_rect.intersect(ui.clip_rect()));
        self.master_track(&mut master_ui);

        // The scrollbar belongs to the timeline viewport, not a window-wide panel.
        let scrollbar_rect = Rect::from_min_max(
            Pos2::new(timeline_left, (bounds.bottom() - 15.0).max(bounds.top())),
            bounds.max,
        );
        #[cfg(test)]
        {
            self.scrollbar_bounds = scrollbar_rect;
        }
        let mut scrollbar_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("timeline_scrollbar")
                .max_rect(scrollbar_rect),
        );
        scrollbar_ui.set_clip_rect(scrollbar_rect.intersect(ui.clip_rect()));
        self.timeline_scrollbar(&mut scrollbar_ui);

        ui.set_max_height((scrollbar_rect.top() - bounds.top()).max(0.0));
        self.ruler(ui);
        let timeline_rect = Rect::from_min_max(
            Pos2::new(timeline_left, ui.cursor().top()),
            Pos2::new(bounds.right(), scrollbar_rect.top()),
        );
        self.paint_grid(&painter.with_clip_rect(timeline_rect), timeline_rect);
        self.lane_bounds.clear();
        let tracks = egui::ScrollArea::vertical()
            .max_height((master_rect.top() - ui.cursor().top()).max(0.0))
            .auto_shrink([false, false])
            .show(ui, |ui| self.tracks(ui));
        self.finish_clip_drag(ui);
        self.paint_drag_preview(ui, tracks.inner_rect.intersect(timeline_rect));
        self.paint_file_hover(ui, tracks.inner_rect.intersect(timeline_rect));
        // One shared border separates the entire header from the track workspace.
        let header_bottom = timeline_rect.top() - 0.5;
        for (left, right, color) in [
            (bounds.left(), timeline_left, theme::BORDER),
            (timeline_left, bounds.right(), theme::BORDER),
        ] {
            painter.line_segment(
                [
                    Pos2::new(left, header_bottom),
                    Pos2::new(right, header_bottom),
                ],
                Stroke::new(1.0, color),
            );
        }

        let playhead_x = timeline_rect.left()
            + ((seconds(self.session.project.transport.playhead_frame) - self.scroll) as f32)
                * self.zoom;
        if (timeline_rect.left()..=timeline_rect.right()).contains(&playhead_x) {
            painter.line_segment(
                [
                    Pos2::new(playhead_x, timeline_rect.top()),
                    Pos2::new(playhead_x, timeline_rect.bottom()),
                ],
                Stroke::new(1.0_f32, theme::ACCENT),
            );
        }
    }
    fn master_track(&mut self, ui: &mut egui::Ui) {
        let panel = panels::PanelStyle {
            margin: 6,
            ..Default::default()
        }
        .track_frame()
        .show(ui, |ui| {
            ui.set_min_width(TRACK_WIDTH - 14.0);
            ui.set_min_height(MASTER_HEIGHT - 14.0);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            let clipped = self
                .output
                .as_ref()
                .is_some_and(|output| output.status.clipped.load(Ordering::Relaxed));
            let levels = std::array::from_fn(|ch| {
                let peak = self.output.as_ref().map_or(0.0, |output| {
                    f32::from_bits(output.status.peaks[ch].load(Ordering::Relaxed))
                });
                (peak, clipped)
            });
            let cleared = meters::stereo_panel(ui, MASTER_HEIGHT - 14.0, levels, |ui| {
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::Label::new(
                        egui::RichText::new("Master").font(egui::FontId::proportional(13.0)),
                    ),
                );
                rows::centered(ui, knobs::SIZE, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let previous = self.session.project.master.gain_db;
                    let knob = knobs::gain(ui, &mut self.session.project.master.gain_db);
                    if knob.changed() {
                        self.master_gain_input =
                            format!("{:.1}", self.session.project.master.gain_db);
                    }
                    let gain = numeric_input(ui, &mut self.master_gain_input, "Gain (dB)");
                    let cancel = (gain.has_focus() || gain.lost_focus())
                        && ui.input(|i| i.key_pressed(egui::Key::Escape));
                    if cancel {
                        self.master_gain_input =
                            format!("{:.1}", self.session.project.master.gain_db);
                        gain.surrender_focus();
                    } else if gain.lost_focus()
                        || (gain.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        match self.master_gain_input.parse::<f32>() {
                            Ok(value) if value.is_finite() => {
                                self.session.project.master.gain_db = value.clamp(-60.0, 12.0);
                            }
                            _ => self.fail("Gain must be a finite number"),
                        }
                        self.master_gain_input =
                            format!("{:.1}", self.session.project.master.gain_db);
                    }
                    if self.session.project.master.gain_db != previous {
                        self.changed();
                    }
                });
            });
            if cleared.into_iter().any(|clicked| clicked)
                && let Some(output) = &self.output
            {
                output.status.clipped.store(false, Ordering::Relaxed);
            }
        });
        // The status panel supplies the shared bottom divider.
        let rect = panel.response.rect.shrink(0.5);
        for edge in [
            [rect.left_top(), rect.right_top()],
            [rect.left_top(), rect.left_bottom()],
            [rect.right_top(), rect.right_bottom()],
        ] {
            ui.painter()
                .line_segment(edge, Stroke::new(1.0, theme::BORDER));
        }
    }
    fn timeline_scrollbar(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            let width = ui.available_width();
            let extent = seconds(self.session.project.end()).max(60.0) + 30.0;
            let offset = self.scroll * f64::from(self.zoom);
            let output = ui
                .scope(|ui| {
                    ui.spacing_mut().scroll = egui::style::ScrollStyle {
                        bar_width: 10.0,
                        ..egui::style::ScrollStyle::solid()
                    };
                    // The timeline paints only its visible range. This scroll area supplies
                    // the standard scrollbar and measures the full range in pixels.
                    egui::ScrollArea::horizontal()
                        .id_salt("timeline_horizontal_scroll")
                        .max_width(width)
                        .max_height(1.0)
                        .min_scrolled_height(0.0)
                        .auto_shrink([false, true])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR)
                        .horizontal_scroll_offset(offset as f32)
                        .show(ui, |ui| {
                            ui.allocate_space(Vec2::new(
                                (extent as f32 * self.zoom).max(width),
                                1.0,
                            ));
                        })
                })
                .inner;
            let scroll = f64::from(output.state.offset.x) / f64::from(self.zoom);
            if scroll != self.scroll {
                self.scroll = scroll;
                ui.ctx().request_repaint();
            }
        });
    }
    fn ruler(&mut self, ui: &mut egui::Ui) {
        rows::centered(ui, RULER_HEIGHT, |ui| {
            ui.allocate_ui(Vec2::new(TRACK_WIDTH, RULER_HEIGHT), |ui| {
                ui.set_min_width(TRACK_WIDTH);
                toolbars::Toolbar::row(ui, |ui| {
                    ui.label("Tracks");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icons::button(ui, icons::Lucide::Plus, "Add track", true, theme::TEXT)
                            .clicked()
                        {
                            match self.session.project.add_track() {
                                Ok(id) => {
                                    self.selected_track = Some(id);
                                    self.changed();
                                }
                                Err(e) => self.fail(e),
                            }
                        }
                    });
                });
            });
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), RULER_HEIGHT),
                Sense::hover(),
            );
            let labels =
                Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - 20.0));
            let ticks = Rect::from_min_max(labels.left_bottom(), rect.max);
            let label_response = ui.interact(labels, response.id.with("labels"), Sense::click());
            let response = ui
                .interact(
                    ticks,
                    response.id.with("selection"),
                    Sense::click_and_drag(),
                )
                .on_hover_text("Drag to select a loop range. Click to seek.");
            let painter = ui.painter().with_clip_rect(rect);
            painter.rect_filled(rect, 0.0, theme::BACKGROUND);
            painter.rect_filled(labels, 0.0, theme::PANEL);
            let l = &self.session.project.transport.r#loop;
            if l.end_frame > l.start_frame {
                let x1 = rect.left() + ((seconds(l.start_frame) - self.scroll) as f32) * self.zoom;
                let x2 = rect.left() + ((seconds(l.end_frame) - self.scroll) as f32) * self.zoom;
                if x2 > rect.left() && x1 < rect.right() {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(x1.max(rect.left()), ticks.top()),
                            Pos2::new(x2.min(rect.right()), ticks.bottom()),
                        ),
                        0.0,
                        if l.enabled {
                            theme::RULER_SELECTION
                        } else {
                            theme::RULER_SELECTION_INACTIVE
                        },
                    );
                }
            }
            painter.line_segment(
                [ticks.left_top(), ticks.right_top()],
                Stroke::new(1.0_f32, theme::BORDER),
            );
            let timeline = self.musical_timeline();
            for tick in timeline.ticks(rect.width()) {
                let x = rect.left() + tick.x;
                if x < rect.left() || x > rect.right() {
                    continue;
                }
                let height = if tick.bar {
                    11.0
                } else if tick.whole_beat {
                    8.0
                } else {
                    5.0
                };
                painter.line_segment(
                    [
                        Pos2::new(x, ticks.bottom() - height),
                        Pos2::new(x, ticks.bottom()),
                    ],
                    Stroke::new(1.0_f32, theme::SECONDARY),
                );
                if let Some(label) = tick.label {
                    let selected = tick.beat >= timeline.beats(l.start_frame)
                        && tick.beat < timeline.beats(l.end_frame);
                    painter.text(
                        Pos2::new(x + 3.0, labels.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(11.0),
                        if selected {
                            theme::CLIP_TEXT
                        } else {
                            theme::TEXT
                        },
                    );
                }
            }
            let playhead_x = rect.left()
                + ((seconds(self.session.project.transport.playhead_frame) - self.scroll) as f32)
                    * self.zoom;
            if (rect.left()..=rect.right()).contains(&playhead_x) {
                let painter = painter.with_clip_rect(rect);
                painter.line_segment(
                    [
                        Pos2::new(playhead_x, rect.top()),
                        Pos2::new(playhead_x, rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, theme::ACCENT),
                );
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(playhead_x - 4.0, rect.bottom() - 13.0),
                        Pos2::new(playhead_x + 4.0, rect.bottom() - 13.0),
                        Pos2::new(playhead_x + 4.0, rect.bottom() - 7.0),
                        Pos2::new(playhead_x, rect.bottom() - 3.0),
                        Pos2::new(playhead_x - 4.0, rect.bottom() - 7.0),
                    ],
                    theme::ACCENT,
                    Stroke::NONE,
                ));
            }
            if label_response.clicked()
                && let Some(pointer) = label_response.interact_pointer_pos()
            {
                self.seek(frames(
                    self.scroll + f64::from((pointer.x - rect.left()) / self.zoom),
                ));
            }
            if let Some(pointer) = response.interact_pointer_pos() {
                let at = frames(self.scroll + f64::from((pointer.x - rect.left()) / self.zoom));
                if response.drag_started() {
                    let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer);
                    self.loop_drag = Some(LoopDrag {
                        start: frames(
                            self.scroll + f64::from((origin.x - rect.left()) / self.zoom),
                        ),
                        original: self.session.project.transport.r#loop.clone(),
                    });
                }
                if response.dragged()
                    && let Some(drag) = &self.loop_drag
                {
                    self.session.project.transport.r#loop.start_frame = drag.start.min(at);
                    self.session.project.transport.r#loop.end_frame = drag.start.max(at);
                }
                if response.clicked() {
                    self.seek(at);
                }
            }
            if response.drag_stopped()
                && let Some(drag) = self.loop_drag.take()
            {
                let region = &self.session.project.transport.r#loop;
                if region.end_frame <= region.start_frame {
                    self.session.project.transport.r#loop = drag.original;
                    self.fail("Loop end must be after its start");
                } else {
                    self.changed();
                }
            }
        });
    }
    fn track_name(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        let width = ui.available_width();
        if self
            .track_name_edit
            .as_ref()
            .is_some_and(|edit| edit.track == track.id)
        {
            let mut edit = self.track_name_edit.take().unwrap();
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(ui.make_persistent_id(("track_name", track.id)))
                .font(egui::FontId::proportional(13.0))
                .text_color(theme::TEXT)
                .horizontal_align(egui::Align::Center)
                .desired_width(width)
                .min_size(Vec2::new(width, toolbars::CONTROL_HEIGHT))
                .show(ui);
            output.response.widget_info(|| {
                egui::WidgetInfo::text_edit(ui.is_enabled(), &track.name, &edit.text, "Track name")
            });
            if edit.focus {
                output.response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(edit.text.chars().count()),
                    )));
                output.state.store(ui.ctx(), output.response.id);
                edit.focus = false;
            }
            if let Some(action) = inline_edit_action(ui, &output.response) {
                output.response.surrender_focus();
                if action == InlineEditAction::Commit
                    && edit.text != track.name
                    && let Some(track) = self
                        .session
                        .project
                        .tracks
                        .iter_mut()
                        .find(|t| t.id == edit.track)
                {
                    track.name = edit.text;
                    self.changed();
                }
            } else {
                self.track_name_edit = Some(edit);
            }
        } else {
            let response = ui
                .add_sized(
                    [width, toolbars::CONTROL_HEIGHT],
                    egui::Button::new(
                        egui::RichText::new(&track.name)
                            .font(egui::FontId::proportional(13.0))
                            .color(theme::TEXT),
                    )
                    .selected(self.selected_track == Some(track.id))
                    .frame(false)
                    .truncate(),
                )
                .on_hover_text(&track.name);
            if response.clicked() {
                self.selected_track = Some(track.id);
            }
            if response.double_clicked() && self.track_name_edit.is_none() {
                self.track_name_edit = Some(TrackNameEdit {
                    track: track.id,
                    text: track.name.clone(),
                    focus: true,
                });
            }
        }
    }
    fn tracks(&mut self, ui: &mut egui::Ui) {
        let tracks = self.session.project.tracks.clone();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let count = tracks.len();
            for (index, track) in tracks.into_iter().enumerate() {
                rows::centered(ui, ROW_HEIGHT, |ui| {
                    let inner = ui.allocate_ui_with_layout(
                        Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(TRACK_WIDTH);
                            panels::PanelStyle {
                                margin: 6,
                                ..Default::default()
                            }
                            .track_frame()
                            .show(ui, |ui| {
                                ui.set_min_width(TRACK_WIDTH - 14.0);
                                ui.set_min_height(ROW_HEIGHT - 14.0);
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                                let levels = std::array::from_fn(|ch| {
                                    let meter = self
                                        .output
                                        .as_ref()
                                        .and_then(|output| output.track_meter(track.id));
                                    let peak = meter.map_or(0.0, |m| {
                                        f32::from_bits(m.peaks[ch].load(Ordering::Relaxed))
                                    });
                                    let clipped = meter
                                        .is_some_and(|m| m.clipped[ch].load(Ordering::Relaxed));
                                    (peak, clipped)
                                });
                                let cleared =
                                    meters::stereo_panel(ui, ROW_HEIGHT - 14.0, levels, |ui| {
                                        rows::centered(ui, toolbars::CONTROL_HEIGHT, |ui| {
                                            self.track_name(ui, &track);
                                        });
                                        let inputs =
                                            self.inputs.entry(track.id).or_insert_with(|| Inputs {
                                                gain: format!("{:.1}", track.gain_db),
                                                pan: format!("{:.2}", track.pan),
                                            });
                                        let mut gain = None;
                                        let mut pan = None;
                                        let mut input_error = None;
                                        let mut mute = track.muted;
                                        let mut solo = track.soloed;
                                        rows::centered(ui, knobs::SIZE, |ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            let mute_button = ui
                                                .add(
                                                    egui::Button::new("M")
                                                        .selected(mute)
                                                        .min_size(Vec2::new(28.0, 22.0)),
                                                )
                                                .on_hover_text("Mute");
                                            mute_button.widget_info(|| {
                                                egui::WidgetInfo::selected(
                                                    egui::WidgetType::Button,
                                                    ui.is_enabled(),
                                                    mute,
                                                    "Mute",
                                                )
                                            });
                                            if mute_button.clicked() {
                                                mute = !mute;
                                            }
                                            let solo_button = ui
                                                .add(
                                                    egui::Button::new("S")
                                                        .selected(solo)
                                                        .min_size(Vec2::new(28.0, 22.0)),
                                                )
                                                .on_hover_text("Solo");
                                            solo_button.widget_info(|| {
                                                egui::WidgetInfo::selected(
                                                    egui::WidgetType::Button,
                                                    ui.is_enabled(),
                                                    solo,
                                                    "Solo",
                                                )
                                            });
                                            if solo_button.clicked() {
                                                solo = !solo;
                                            }

                                            let mut value = track.gain_db;
                                            if knobs::gain(ui, &mut value).changed() {
                                                gain = Some(value);
                                                inputs.gain = format!("{value:.1}");
                                            }
                                            let r =
                                                numeric_input(ui, &mut inputs.gain, "Gain (dB)");
                                            if r.lost_focus()
                                                || (r.has_focus()
                                                    && ui
                                                        .input(|i| i.key_pressed(egui::Key::Enter)))
                                            {
                                                match inputs.gain.parse::<f32>() {
                                                    Ok(v)
                                                        if v.is_finite()
                                                            && daw_core::linear_gain(v)
                                                                .is_finite() =>
                                                    {
                                                        gain = Some(v)
                                                    }
                                                    _ => {
                                                        inputs.gain =
                                                            format!("{:.1}", track.gain_db);
                                                        input_error =
                                                            Some("Gain must be a finite number");
                                                    }
                                                }
                                            }
                                            if r.has_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                inputs.gain = format!("{:.1}", track.gain_db);
                                                r.surrender_focus();
                                            }
                                            let mut value = track.pan;
                                            if knobs::pan(ui, &mut value).changed() {
                                                pan = Some(value);
                                                inputs.pan = format!("{value:.2}");
                                            }
                                            let r = numeric_input(
                                                ui,
                                                &mut inputs.pan,
                                                "Pan (-1 left, 0 center, +1 right)",
                                            );
                                            if r.lost_focus()
                                                || (r.has_focus()
                                                    && ui
                                                        .input(|i| i.key_pressed(egui::Key::Enter)))
                                            {
                                                match inputs.pan.parse::<f32>() {
                                                    Ok(v)
                                                        if v.is_finite()
                                                            && (-1.0..=1.0).contains(&v) =>
                                                    {
                                                        pan = Some(v)
                                                    }
                                                    _ => {
                                                        inputs.pan = format!("{:.2}", track.pan);
                                                        input_error =
                                                            Some("Pan must be between -1 and 1");
                                                    }
                                                }
                                            }
                                            if r.has_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                inputs.pan = format!("{:.2}", track.pan);
                                                r.surrender_focus();
                                            }
                                        });
                                        if let Some(e) = input_error {
                                            self.fail(e);
                                        }
                                        let updated = gain.is_some_and(|v| v != track.gain_db)
                                            || pan.is_some_and(|v| v != track.pan)
                                            || mute != track.muted
                                            || solo != track.soloed;
                                        if updated {
                                            if let Some(t) = self
                                                .session
                                                .project
                                                .tracks
                                                .iter_mut()
                                                .find(|t| t.id == track.id)
                                            {
                                                t.gain_db = gain.unwrap_or(t.gain_db);
                                                t.pan = pan.unwrap_or(t.pan);
                                                t.muted = mute;
                                                t.soloed = solo;
                                            }
                                            self.changed();
                                        }
                                    });
                                if let Some(meter) = self
                                    .output
                                    .as_ref()
                                    .and_then(|output| output.track_meter(track.id))
                                {
                                    for (ch, clicked) in cleared.into_iter().enumerate() {
                                        if clicked {
                                            meter.clipped[ch].store(false, Ordering::Relaxed);
                                        }
                                    }
                                }
                            });
                        },
                    );
                    let rect = inner.response.rect.shrink(0.5);
                    let selected = self.selected_track == Some(track.id);
                    let painter = ui.painter();
                    if index > 0 {
                        painter.line_segment(
                            [rect.left_top(), rect.right_top()],
                            Stroke::new(1.0, theme::BORDER),
                        );
                    }
                    for edge in [
                        [rect.left_top(), rect.left_bottom()],
                        [rect.right_top(), rect.right_bottom()],
                    ] {
                        painter.line_segment(edge, Stroke::new(1.0, theme::BORDER));
                    }
                    if index + 1 == count {
                        painter.line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            Stroke::new(1.0, theme::BORDER),
                        );
                    }
                    if selected {
                        painter.rect_stroke(
                            inner.response.rect,
                            2.0,
                            theme::SELECTION_OUTLINE,
                            StrokeKind::Inside,
                        );
                    }
                    inner.response.context_menu(|ui| {
                        if ui.button("Delete track").clicked() {
                            self.edit(Edit::DeleteTrack(track.id));
                            self.inputs.remove(&track.id);
                            ui.close();
                        }
                    });
                    self.lane(ui, track.id, &track.clips);
                });
            }
        });
    }
    fn musical_timeline(&self) -> musical_time::Timeline {
        musical_time::Timeline::new(self.session.project.tempo_bpm, self.zoom, self.scroll)
    }
    fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        for tick in self.musical_timeline().ticks(rect.width()) {
            let x = rect.left() + tick.x;
            if x < rect.left() || x > rect.right() {
                continue;
            }
            let color = if tick.bar {
                theme::BORDER
            } else if tick.whole_beat {
                theme::GRID
            } else {
                theme::GRID_SUBDIVISION
            };
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0_f32, color),
            );
        }
    }
    fn clip_block(&self, lane: Rect, clip: &Clip, start: u64) -> Rect {
        let left = lane.left() + ((seconds(start) - self.scroll) as f32) * self.zoom;
        let right = left + seconds(clip.length_frames) as f32 * self.zoom;
        Rect::from_min_max(
            Pos2::new(left, lane.top()),
            Pos2::new(right.max(left + 2.0), lane.bottom()),
        )
    }
    fn drag_delta(&self, drag: &Drag, pointer: Pos2) -> i64 {
        (f64::from(pointer.x - drag.origin.x) / f64::from(self.zoom)
            * f64::from(daw_core::SAMPLE_RATE))
        .round() as i64
    }
    fn move_start(&self, drag: &Drag, pointer: Pos2) -> u64 {
        (i128::from(drag.clip.start_frame) + i128::from(self.drag_delta(drag, pointer)))
            .clamp(0, i128::from(u64::MAX)) as u64
    }
    fn drag_destination(&self, drag: &Drag, pointer: Pos2) -> Id {
        self.session
            .project
            .tracks
            .iter()
            .find(|track| {
                self.lane_bounds
                    .get(&track.id)
                    .is_some_and(|lane| lane.contains(pointer))
            })
            .map_or(drag.track, |track| track.id)
    }
    fn move_preview(&self, pointer: Pos2) -> Option<MovePreview> {
        let drag = self.drag.as_ref().filter(|drag| drag.mode == 0)?;
        let track = self.drag_destination(drag, pointer);
        self.lane_bounds.get(&track)?;
        let start = self.move_start(drag, pointer);
        let valid = self.placement_valid(
            Some(track),
            Some(drag.clip.id),
            start,
            drag.clip.length_frames,
        );
        Some(MovePreview {
            track,
            start,
            valid,
        })
    }
    fn placement_valid(
        &self,
        track: Option<Id>,
        ignore: Option<Id>,
        start: u64,
        length: u64,
    ) -> bool {
        length > 0
            && start.checked_add(length).is_some_and(|end| {
                track.is_none_or(|id| {
                    self.session
                        .project
                        .tracks
                        .iter()
                        .find(|track| track.id == id)
                        .is_some_and(|track| {
                            track.clips.iter().all(|clip| {
                                Some(clip.id) == ignore
                                    || end <= clip.start_frame
                                    || start >= clip.end()
                            })
                        })
                })
            })
    }
    fn file_target(&self, pointer: Pos2, viewport: Rect) -> Option<FileDropTarget> {
        let drop_viewport = Rect::from_min_max(
            Pos2::new(viewport.left() - TRACK_WIDTH, viewport.top()),
            viewport.max,
        );
        if !drop_viewport.contains(pointer) {
            return None;
        }
        // Track controls always target frame zero, even when the timeline is scrolled.
        let start = if pointer.x < viewport.left() {
            0
        } else {
            frames((self.scroll + f64::from((pointer.x - viewport.left()) / self.zoom)).max(0.0))
        };
        if let Some(track) = self.session.project.tracks.iter().find(|track| {
            self.lane_bounds
                .get(&track.id)
                .is_some_and(|lane| pointer.y >= lane.top() && pointer.y <= lane.bottom())
        }) {
            return Some(FileDropTarget {
                track: Some(track.id),
                start,
                lane: self.lane_bounds[&track.id],
            });
        }
        let top = self
            .lane_bounds
            .values()
            .map(Rect::bottom)
            .fold(viewport.top(), f32::max);
        if pointer.y < top || top >= viewport.bottom() {
            return None;
        }
        Some(FileDropTarget {
            track: None,
            start,
            lane: Rect::from_min_size(
                Pos2::new(viewport.left(), top),
                Vec2::new(viewport.width(), ROW_HEIGHT),
            ),
        })
    }
    fn paint_file_hover(&mut self, ui: &egui::Ui, viewport: Rect) {
        if !ui.is_enabled()
            || !self.file_action_enabled(FileAction::Import)
            || self.file_hover.is_none()
        {
            return;
        }
        let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) else {
            return;
        };
        let Some(target) = self.file_target(pointer, viewport) else {
            return;
        };
        let hover = self.file_hover.as_ref().unwrap();
        let audio = hover.audio.as_ref().and_then(|result| result.as_ref().ok());
        let clip = Clip {
            id: Id::nil(),
            asset_id: Id::nil(),
            name: hover.name.clone(),
            start_frame: target.start,
            source_offset_frame: 0,
            length_frames: audio.map_or_else(
                || frames(120.0 / f64::from(self.zoom)),
                |audio| audio.samples.len() as u64,
            ),
        };
        let valid = match &hover.audio {
            Some(Ok(_)) => {
                self.placement_valid(target.track, None, target.start, clip.length_frames)
            }
            Some(Err(_)) => false,
            None => true,
        };
        let block = self.clip_block(target.lane, &clip, target.start);
        let painter = ui.painter_at(viewport.intersect(target.lane));
        let mut ghost = painter.clone();
        ghost.multiply_opacity(0.75);
        if let Some(audio) = audio {
            self.paint_clip(&ghost, target.lane, block, &clip, false, Some(audio));
        } else {
            ghost.rect_filled(block, 2.0, theme::CLIP);
            let header =
                Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
            ghost.rect_filled(header, 2.0, theme::CLIP_HEADER);
            let text = ghost.with_clip_rect(block);
            text.text(
                Pos2::new(header.left() + 7.0, header.center().y),
                egui::Align2::LEFT_CENTER,
                &clip.name,
                egui::FontId::proportional(11.0),
                theme::CLIP_TEXT,
            );
            text.text(
                Pos2::new(block.center().x, block.top() + 44.0),
                egui::Align2::CENTER_CENTER,
                if hover.audio.is_some() {
                    "Unsupported WAV"
                } else {
                    "Loading waveform…"
                },
                egui::FontId::proportional(11.0),
                theme::TEXT,
            );
        }
        painter.rect_stroke(
            block,
            2.0,
            Stroke::new(2.0, if valid { theme::ACCENT } else { theme::ERROR }),
            StrokeKind::Inside,
        );
        self.file_drop_target = Some(target);
    }
    fn paint_drag_preview(&self, ui: &egui::Ui, viewport: Rect) {
        let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) else {
            return;
        };
        let Some(preview) = self.move_preview(pointer) else {
            return;
        };
        let drag = self.drag.as_ref().unwrap();
        let lane = self.lane_bounds[&preview.track];
        let block = self.clip_block(lane, &drag.clip, preview.start);
        let painter = ui.painter_at(viewport.intersect(lane));
        let mut ghost = painter.clone();
        ghost.multiply_opacity(0.75);
        self.paint_clip(&ghost, lane, block, &drag.clip, false, None);
        painter.rect_stroke(
            block,
            2.0,
            Stroke::new(
                2.0,
                if preview.valid {
                    theme::ACCENT
                } else {
                    theme::ERROR
                },
            ),
            StrokeKind::Inside,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    fn paint_clip(
        &self,
        painter: &egui::Painter,
        lane: Rect,
        block: Rect,
        clip: &Clip,
        selected: bool,
        prepared: Option<&daw_media::AudioData>,
    ) {
        let audio = prepared.or_else(|| self.session.audio.get(&clip.asset_id));
        let missing = audio.is_none();
        painter.rect_filled(
            block,
            2.0,
            if missing { theme::MISSING } else { theme::CLIP },
        );
        let header = Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
        painter.rect_filled(
            header,
            2.0,
            if missing {
                theme::BORDER
            } else {
                theme::CLIP_HEADER
            },
        );
        let clip_painter = painter.with_clip_rect(block.intersect(lane));
        clip_painter.text(
            Pos2::new(header.left() + 7.0, header.center().y),
            egui::Align2::LEFT_CENTER,
            &clip.name,
            egui::FontId::proportional(11.0),
            theme::CLIP_TEXT,
        );
        if missing {
            clip_painter.text(
                block.center(),
                egui::Align2::CENTER_CENTER,
                "Missing source",
                egui::FontId::proportional(13.0),
                theme::WARNING,
            );
        } else if let Some(audio) = audio {
            let channels = usize::from(audio.metadata.channels);
            let body = Rect::from_min_max(Pos2::new(block.left(), block.top() + 22.0), block.max);
            let height = body.height() / channels as f32;
            for channel in 0..channels {
                let top = body.top() + height * channel as f32;
                let center = top + height * 0.5;
                if channel == 1 {
                    clip_painter.rect_filled(
                        Rect::from_min_max(Pos2::new(body.left(), top), body.max),
                        0.0,
                        theme::CLIP_CHANNEL,
                    );
                }
                if channel == 1 {
                    clip_painter.line_segment(
                        [Pos2::new(block.left(), top), Pos2::new(block.right(), top)],
                        Stroke::new(1.0_f32, theme::CLIP_HEADER),
                    );
                }
                let left = body.left().max(lane.left());
                let right = body.right().min(lane.right());
                for pixel in (left as i32)..=(right as i32) {
                    let local = ((pixel as f32 - body.left()) / self.zoom
                        * f64::from(daw_core::SAMPLE_RATE) as f32)
                        .max(0.0) as u64;
                    let offset = clip.source_offset_frame + local;
                    let samples_per_pixel = f64::from(daw_core::SAMPLE_RATE) / f64::from(self.zoom);
                    let lo = (offset / 256) as usize;
                    let hi = ((offset + samples_per_pixel.ceil() as u64) / 256) as usize;
                    let mut min = 0.0_f32;
                    let mut max = 0.0_f32;
                    if samples_per_pixel < 256.0 {
                        for s in audio
                            .samples
                            .iter()
                            .skip(offset as usize)
                            .take(samples_per_pixel.ceil() as usize + 1)
                        {
                            min = min.min(s[channel]);
                            max = max.max(s[channel]);
                        }
                    } else {
                        for p in audio
                            .peaks
                            .iter()
                            .skip(lo)
                            .take((hi - lo + 1).min(audio.peaks.len()))
                        {
                            min = min.min(p[channel][0]);
                            max = max.max(p[channel][1]);
                        }
                    }
                    clip_painter.line_segment(
                        [
                            Pos2::new(pixel as f32, center - max.clamp(-1.0, 1.0) * height * 0.42),
                            Pos2::new(pixel as f32, center - min.clamp(-1.0, 1.0) * height * 0.42),
                        ],
                        Stroke::new(1.0_f32, theme::WAVEFORM),
                    );
                }
            }
        }
        painter.rect_stroke(
            block,
            2.0,
            if selected {
                theme::SELECTION_OUTLINE
            } else {
                Stroke::new(1.0, theme::DIVIDER)
            },
            StrokeKind::Inside,
        );
    }
    fn finish_clip_drag(&mut self, ui: &mut egui::Ui) {
        if !ui.is_enabled() || !ui.input(|input| input.focused) {
            self.drag = None;
            return;
        }
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary))
            && let Some(drag) = self.drag.take()
            && drag.mode < 3
        {
            let pointer = ui
                .input(|input| input.pointer.latest_pos())
                .unwrap_or(drag.origin);
            let delta = self.drag_delta(&drag, pointer);
            let destination = if drag.mode == 0 {
                self.drag_destination(&drag, pointer)
            } else {
                drag.track
            };
            let move_start = self.move_start(&drag, pointer);
            let c = drag.clip;
            let position = c.start_frame as i128 + delta as i128;
            let length = c.length_frames as i128;
            let offset = c.source_offset_frame as i128;
            let (start, source, len) = match drag.mode {
                1 => (position, offset + delta as i128, length - delta as i128),
                2 => (c.start_frame as i128, offset, length + delta as i128),
                _ => (i128::from(move_start), offset, length),
            };
            if start < 0
                || source < 0
                || len <= 0
                || start > u64::MAX as i128
                || source > u64::MAX as i128
                || len > u64::MAX as i128
            {
                self.fail("Clip edit exceeds valid source or timeline bounds");
            } else {
                self.edit(Edit::Place {
                    clip_id: c.id,
                    track_id: destination,
                    start: start as u64,
                    offset: source as u64,
                    length: len as u64,
                });
            }
        }
    }
    fn lane(&mut self, ui: &mut egui::Ui, track_id: Id, clips: &[Clip]) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
        self.lane_bounds.insert(track_id, rect);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::BACKGROUND);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, theme::BORDER),
        );
        self.paint_grid(&painter, rect);
        for clip in clips {
            let block = self.clip_block(rect, clip, clip.start_frame);
            if !block.intersects(rect) {
                continue;
            }
            let response = ui.interact(
                block.intersect(rect),
                egui::Id::new(clip.id),
                Sense::click_and_drag(),
            );
            let selected = self.selected_clip == Some(clip.id);
            let mut clip_painter = painter.clone();
            if self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.mode == 0 && drag.clip.id == clip.id)
            {
                clip_painter.multiply_opacity(0.35);
            }
            self.paint_clip(&clip_painter, rect, block, clip, selected, None);
            response.context_menu(|ui| {
                if ui.button("Split at playhead").clicked() {
                    self.edit(Edit::Split {
                        clip_id: clip.id,
                        at: self.session.project.transport.playhead_frame,
                    });
                    ui.close();
                }
                if ui.button("Delete clip").clicked() {
                    self.edit(Edit::DeleteClip(clip.id));
                    ui.close();
                }
            });
            if response.clicked() {
                self.selected_clip = Some(clip.id);
                self.selected_track = Some(track_id);
            }
            if response.hovered()
                && let Some(pos) = response.hover_pos()
            {
                if (pos.x - block.left()).abs() < 7.0 || (pos.x - block.right()).abs() < 7.0 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                } else if pos.y < block.top() + 26.0 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }
            if response.drag_started()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                let mode = if (origin.x - block.left()).abs() < 8.0 {
                    1
                } else if (origin.x - block.right()).abs() < 8.0 {
                    2
                } else if origin.y < block.top() + 26.0 {
                    0
                } else {
                    3
                };
                self.drag = Some(Drag {
                    clip: clip.clone(),
                    track: track_id,
                    mode,
                    origin,
                });
                self.selected_clip = Some(clip.id);
            }
        }
        let x = rect.left()
            + ((seconds(self.session.project.transport.playhead_frame) - self.scroll) as f32)
                * self.zoom;
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0_f32, theme::ACCENT),
            );
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.selected_track = Some(track_id);
            self.selected_clip = None;
            self.seek(frames(
                self.scroll + f64::from((pos.x - rect.left()) / self.zoom),
            ));
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        #[derive(Clone, Copy)]
        enum UnsavedChoice {
            Save,
            Discard,
            Cancel,
        }

        if self.unsaved_prompt {
            let choice = dialogs::Dialog::new(
                "Unsaved changes",
                "Save changes before continuing?",
                &[
                    ("Save", UnsavedChoice::Save),
                    ("Discard", UnsavedChoice::Discard),
                    ("Cancel", UnsavedChoice::Cancel),
                ],
            )
            .actions_enabled(self.job.is_none())
            .show(ctx);
            match choice {
                Some(UnsavedChoice::Save) => self.save(false),
                Some(UnsavedChoice::Discard) => {
                    self.dirty = false;
                    self.unsaved_prompt = false;
                    self.perform_pending();
                }
                Some(UnsavedChoice::Cancel) => {
                    self.pending = None;
                    self.unsaved_prompt = false;
                }
                None => {}
            }
        }
        if let Some(path) = self.overwrite.clone() {
            let message = format!("Replace {}?", path.display());
            if let Some(replace) = dialogs::Dialog::new(
                "Replace export?",
                &message,
                &[("Replace", true), ("Cancel", false)],
            )
            .show(ctx)
            {
                self.overwrite = None;
                if replace {
                    self.export(path, true);
                }
            }
        }
        if let Some(error) = self.error.clone()
            && dialogs::Dialog::new("Error", &error, &[("OK", ())])
                .show(ctx)
                .is_some()
        {
            self.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (DawUi, Id) {
        let mut app = DawUi::default();
        let track = app.session.project.add_track().unwrap();
        let asset = Id::new_v4();
        app.session.project.assets.push(daw_core::Asset {
            id: asset,
            name: "Missing".into(),
            source: daw_core::Source {
                kind: "external".into(),
                path: "/missing.wav".into(),
                path_kind: "absolute".into(),
            },
            source_metadata: daw_core::SourceMetadata {
                sample_rate_hz: 48000,
                channels: 2,
                sample_format: "pcm_int".into(),
                bits_per_sample: 16,
            },
            decoded_frame_count: 480000,
        });
        app.session.project.tracks[0].clips.push(Clip {
            id: Id::new_v4(),
            asset_id: asset,
            name: "Test".into(),
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 480000,
        });
        (app, track)
    }
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        fonts::configure(&ctx);
        icons::configure(&ctx);
        ctx
    }
    fn start_tempo_edit(app: &mut DawUi, ctx: &egui::Context) {
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![]);
        let monitor = app.tempo_bounds;
        let point = monitor.center();
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(app, ctx, vec![button(point, false)]);
        assert!(
            app.tempo_edit.is_none(),
            "single click must retain the monitor"
        );
        frame(app, ctx, vec![button(point, true)]);
        frame(app, ctx, vec![button(point, false)]);
        assert!(app.tempo_edit.is_some());
        frame(app, ctx, vec![]);
        assert!(ctx.egui_wants_keyboard_input());
        assert_eq!(app.tempo_bounds.width(), monitor.width());
    }
    #[test]
    fn toolbar_tempo_double_click_commits_cancels_and_rejects_invalid_values() {
        let mut app = DawUi::default();
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        for (text, action, expected, changed, invalid) in [
            ("135.5", egui::Key::Enter, 135.5, true, false),
            ("80", egui::Key::Escape, 135.5, false, false),
            ("135.5", egui::Key::Enter, 135.5, false, false),
            ("0", egui::Key::Enter, 135.5, false, true),
            ("-12", egui::Key::Enter, 135.5, false, true),
            ("NaN", egui::Key::Enter, 135.5, false, true),
            ("inf", egui::Key::Enter, 135.5, false, true),
            ("", egui::Key::Enter, 135.5, false, true),
        ] {
            app.dirty = false;
            app.error = None;
            let ctx = context();
            start_tempo_edit(&mut app, &ctx);
            let input = if text.is_empty() {
                key(egui::Key::Backspace)
            } else {
                egui::Event::Text(text.into())
            };
            frame(&mut app, &ctx, vec![input]);
            assert_eq!(app.tempo_edit.as_ref().unwrap().text, text);
            frame(&mut app, &ctx, vec![key(action)]);
            assert_eq!(app.session.project.tempo_bpm, expected);
            assert!(app.tempo_edit.is_none());
            assert_eq!(app.dirty, changed);
            assert_eq!(app.error.is_some(), invalid);
        }
        app.error = None;
        app.dirty = false;
        let ctx = context();
        start_tempo_edit(&mut app, &ctx);
        frame(&mut app, &ctx, vec![egui::Event::Text("99.25".into())]);
        let outside = Pos2::new(30.0, app.tempo_bounds.bottom() + 20.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(outside), button(outside, true)],
        );
        frame(&mut app, &ctx, vec![button(outside, false)]);
        assert!(app.tempo_edit.is_none());
        assert_eq!(app.session.project.tempo_bpm, 135.5);
        assert!(!app.dirty, "focus loss must cancel without committing");
        assert!(app.error.is_none());

        app.pending = Some(Action::CloseProject);
        app.perform_pending();
        assert!(app.tempo_edit.is_none());
        assert_eq!(app.session.project.tempo_bpm, 120.0);
        let ctx = context();
        start_tempo_edit(&mut app, &ctx);
        let (sender, receiver) = mpsc::channel();
        let mut session = Session::default();
        session.project.tempo_bpm = 87.5;
        sender.send(Ok(Job::Loaded(session, false))).unwrap();
        app.job = Some(receiver);
        app.poll();
        assert!(app.tempo_edit.is_none());
        assert_eq!(app.session.project.tempo_bpm, 87.5);
    }
    #[test]
    fn tempo_drag_uses_vertical_motion_and_shift_for_fine_adjustment() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let start = app.tempo_bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let right = start + Vec2::new(20.0, 0.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
        assert_eq!(app.session.project.tempo_bpm, 120.0);
        assert!(!app.dirty, "horizontal motion must not change tempo");

        let up = right - Vec2::new(0.0, 10.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
        assert_eq!(app.session.project.tempo_bpm, 130.0);
        assert!(app.dirty);
        app.dirty = false;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.session.project.tempo_bpm, 130.0);
        assert!(!app.dirty, "stationary frames must not repeat the change");

        let fine = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let fine_up = up - Vec2::new(0.0, 10.0);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(fine),
                egui::Event::PointerMoved(fine_up),
            ],
        );
        assert_eq!(app.session.project.tempo_bpm, 131.0);
        let fine_down = fine_up + Vec2::new(0.0, 3.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(fine_down)]);
        assert_eq!(app.session.project.tempo_bpm, 130.7);

        let down = fine_down + Vec2::new(0.0, 10.0);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(Default::default()),
                egui::Event::PointerMoved(down),
            ],
        );
        assert_eq!(app.session.project.tempo_bpm, 120.7);
        frame(&mut app, &ctx, vec![button(down, false)]);
        assert!(app.tempo_edit.is_none());
        assert!(app.error.is_none());
        let clip = &app.session.project.tracks[0].clips[0];
        assert_eq!(app.session.project.tracks[0].id, track);
        assert_eq!(clip.start_frame, 0);
        assert_eq!(clip.length_frames, 480000);
        assert_eq!(clip.source_offset_frame, 0);
        app.session.project.validate().unwrap();
    }
    #[test]
    fn tempo_drag_stays_positive_and_is_disabled_during_background_jobs() {
        let mut app = DawUi::default();
        app.session.project.tempo_bpm = 2.0;
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let start = app.tempo_bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let down = start + Vec2::new(0.0, 20.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(down)]);
        assert_eq!(app.session.project.tempo_bpm, 0.01);
        app.session.project.validate().unwrap();
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(down + Vec2::new(0.0, 10.0))],
        );
        assert_eq!(app.session.project.tempo_bpm, 0.01);
        assert!(!app.dirty);

        let (sender, receiver) = mpsc::channel();
        app.job = Some(receiver);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
        assert_eq!(app.session.project.tempo_bpm, 0.01);
        assert!(!app.dirty);
        frame(&mut app, &ctx, vec![button(start, false)]);
        drop(sender);
    }
    #[test]
    fn musical_monitor_drags_bars_and_beats_with_carry_and_borrow() {
        for tempo in [60.0, 123.5] {
            for section in 0..2 {
                let (mut app, _) = fixture();
                app.session.project.tempo_bpm = tempo;
                let initial = musical_time::shift(0, tempo, 3) + frames(0.1);
                app.session.project.transport.playhead_frame = initial;
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let start = app.musical_bounds[section].center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                // Moving over the other section must retain the original drag unit.
                let right = start + Vec2::new(100.0, 0.0);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
                assert_eq!(app.session.project.transport.playhead_frame, initial);
                assert!(!app.dirty);
                let up = right - Vec2::new(0.0, 1.0);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
                let moved = app.session.project.transport.playhead_frame;
                assert_eq!(
                    musical_time::monitor(moved, tempo),
                    if section == 0 { "0002.04" } else { "0002.01" }
                );
                let expected_seconds =
                    if section == 0 { 4.0 } else { 1.0 } * 60.0 / f64::from(tempo);
                assert!((seconds(moved - initial) - expected_seconds).abs() <= 1.0 / 48000.0);
                assert!(app.dirty);
                app.dirty = false;
                frame(&mut app, &ctx, vec![]);
                assert_eq!(app.session.project.transport.playhead_frame, moved);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
                assert_eq!(app.session.project.transport.playhead_frame, initial);
                assert_eq!(musical_time::monitor(initial, tempo), "0001.04");
                frame(&mut app, &ctx, vec![button(right, false)]);
                assert!(app.musical_drag.is_none());
                assert_eq!(app.session.project.tempo_bpm, tempo);
                assert_eq!(app.session.project.tracks[0].clips[0].length_frames, 480000);
                assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
                assert!(app.error.is_none());
            }
        }
    }
    #[test]
    fn musical_monitor_shift_drag_keeps_whole_units_and_clamps_at_start() {
        for section in 0..2 {
            let mut app = DawUi::default();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let start = app.musical_bounds[section].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let fine = egui::Modifiers {
                shift: true,
                ..Default::default()
            };
            let halfway = start - Vec2::new(0.0, 5.0);
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(fine),
                    egui::Event::PointerMoved(halfway),
                ],
            );
            assert_eq!(app.session.project.transport.playhead_frame, 0);
            assert!(!app.dirty);
            let up = halfway - Vec2::new(0.0, 5.0);
            for point in 1..=5 {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(
                        halfway - Vec2::new(0.0, point as f32),
                    )],
                );
            }
            assert_eq!(
                musical_time::monitor(app.session.project.transport.playhead_frame, 120.0),
                if section == 0 { "0002.01" } else { "0001.02" }
            );
            let down = up + Vec2::new(0.0, 20.0);
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(Default::default()),
                    egui::Event::PointerMoved(down),
                ],
            );
            assert_eq!(app.session.project.transport.playhead_frame, 0);
            let reversed = down - Vec2::new(0.0, 1.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(reversed)]);
            let moved = app.session.project.transport.playhead_frame;
            assert_eq!(
                musical_time::monitor(moved, 120.0),
                if section == 0 { "0002.01" } else { "0001.02" }
            );
            app.dirty = false;
            let (sender, receiver) = mpsc::channel();
            app.job = Some(receiver);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(reversed - Vec2::new(0.0, 20.0))],
            );
            assert_eq!(app.session.project.transport.playhead_frame, moved);
            assert!(!app.dirty);
            assert!(app.musical_drag.is_none());
            frame(&mut app, &ctx, vec![button(reversed, false)]);
            drop(sender);
            app.session.project.validate().unwrap();
        }
    }
    #[test]
    fn selection_monitor_drags_each_endpoint_by_bars_and_beats() {
        for target in 0..4 {
            let (mut app, _) = fixture();
            let tempo = 123.5;
            app.session.project.tempo_bpm = tempo;
            let start_frame = musical_time::shift(0, tempo, 3);
            let end_frame = musical_time::shift(0, tempo, 15);
            let region = &mut app.session.project.transport.r#loop;
            region.start_frame = start_frame;
            region.end_frame = end_frame;
            region.enabled = true;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let start = app.selection_bounds[target].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let left = start - Vec2::new(100.0, 0.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(left)]);
            assert!(!app.dirty);
            let up = left - Vec2::new(0.0, 1.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
            let region = &app.session.project.transport.r#loop;
            assert_eq!(
                musical_time::monitor(
                    if target < 2 {
                        region.start_frame
                    } else {
                        region.end_frame
                    },
                    tempo
                ),
                ["0002.04", "0002.01", "0005.04", "0005.01"][target],
            );
            assert_eq!(
                if target < 2 {
                    region.end_frame
                } else {
                    region.start_frame
                },
                if target < 2 { end_frame } else { start_frame }
            );
            assert!(region.enabled);
            assert!(app.dirty);
            app.dirty = false;
            frame(&mut app, &ctx, vec![]);
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(left)]);
            let region = &app.session.project.transport.r#loop;
            assert_eq!(
                (region.start_frame, region.end_frame),
                (start_frame, end_frame)
            );
            frame(&mut app, &ctx, vec![button(left, false)]);
            assert!(app.selection_drag.iter().all(Option::is_none));
            assert_eq!(app.session.project.transport.playhead_frame, 0);
            assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
            assert_eq!(app.session.project.tracks[0].clips[0].length_frames, 480000);
            assert!(app.error.is_none());
            app.session.project.validate().unwrap();
        }
    }
    #[test]
    fn selection_monitor_shift_drag_can_create_a_range_without_enabling_loop() {
        for target in 2..4 {
            let mut app = DawUi::default();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let start = app.selection_bounds[target].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let left = start - Vec2::new(100.0, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(left),
                    egui::Event::ModifiersChanged(egui::Modifiers {
                        shift: true,
                        ..Default::default()
                    }),
                ],
            );
            for point in 1..=9 {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(
                        left - Vec2::new(0.0, point as f32),
                    )],
                );
                assert_eq!(app.session.project.transport.r#loop.end_frame, 0);
                assert!(!app.dirty);
            }
            let up = left - Vec2::new(0.0, 10.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
            let region = &app.session.project.transport.r#loop;
            assert_eq!(
                musical_time::monitor(region.end_frame, 120.0),
                if target == 2 { "0002.01" } else { "0001.02" }
            );
            assert_eq!(region.start_frame, 0);
            assert!(!region.enabled);
            assert!(app.dirty);
            frame(&mut app, &ctx, vec![button(up, false)]);
        }
    }
    #[test]
    fn selection_monitor_clamps_endpoints_and_blocks_dragging_during_jobs() {
        for enabled in [false, true] {
            for endpoint in 0..2 {
                let mut app = DawUi::default();
                let region = &mut app.session.project.transport.r#loop;
                region.start_frame = frames(2.0);
                region.end_frame = frames(4.0);
                region.enabled = enabled;
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let start = app.selection_bounds[endpoint * 2 + 1].center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                let direction = if endpoint == 0 { -1.0 } else { 1.0 };
                let end = start + Vec2::new(0.0, direction * 20.0);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                let region = &app.session.project.transport.r#loop;
                let expected = if endpoint == 0 {
                    frames(4.0) - u64::from(enabled)
                } else {
                    frames(2.0) + u64::from(enabled)
                };
                assert_eq!(
                    if endpoint == 0 {
                        region.start_frame
                    } else {
                        region.end_frame
                    },
                    expected
                );
                assert_eq!(region.enabled, enabled);
                app.session.project.validate().unwrap();
                app.dirty = false;
                let farther = end + Vec2::new(0.0, direction * 10.0);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(farther)]);
                assert!(!app.dirty);
                let reverse = farther - Vec2::new(0.0, direction);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(reverse)]);
                assert!(
                    app.dirty,
                    "reversing at a range limit must adjust immediately"
                );
                let original = app.session.project.transport.r#loop.clone();
                app.dirty = false;
                let (sender, receiver) = mpsc::channel();
                app.job = Some(receiver);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
                let region = &app.session.project.transport.r#loop;
                assert_eq!(
                    (region.start_frame, region.end_frame),
                    (original.start_frame, original.end_frame)
                );
                assert!(!app.dirty);
                assert!(app.selection_drag.iter().all(Option::is_none));
                frame(&mut app, &ctx, vec![button(start, false)]);
                drop(sender);
                app.session.project.validate().unwrap();
            }
        }
    }
    #[test]
    fn time_monitor_drags_hours_minutes_and_seconds_with_carry_and_borrow() {
        for (initial_seconds, direction, expected) in [
            (
                3599.25,
                -1.0,
                ["01h59m59.25s", "01h00m59.25s", "01h00m00.25s"],
            ),
            (
                3600.25,
                1.0,
                ["00h00m00.25s", "00h59m00.25s", "00h59m59.25s"],
            ),
            (
                359999.25,
                -1.0,
                ["100h59m59.25s", "100h00m59.25s", "100h00m00.25s"],
            ),
        ] {
            for (section, expected) in expected.into_iter().enumerate() {
                let (mut app, _) = fixture();
                let initial = frames(initial_seconds) + 17;
                app.session.project.transport.playhead_frame = initial;
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let start = app.time_bounds[section].center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                let right = start + Vec2::new(100.0, 0.0);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
                assert_eq!(app.session.project.transport.playhead_frame, initial);
                assert!(!app.dirty);
                let moved_pointer = right + Vec2::new(0.0, direction);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(moved_pointer)],
                );
                let moved = app.session.project.transport.playhead_frame;
                assert_eq!(transport_time(moved), expected);
                assert_eq!(moved % 48000, initial % 48000);
                assert!(app.dirty);
                app.dirty = false;
                frame(&mut app, &ctx, vec![]);
                assert_eq!(app.session.project.transport.playhead_frame, moved);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
                assert_eq!(app.session.project.transport.playhead_frame, initial);
                frame(&mut app, &ctx, vec![button(right, false)]);
                assert!(app.time_drag.is_none());
                assert_eq!(app.session.project.tempo_bpm, 120.0);
                assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
                assert_eq!(app.session.project.tracks[0].clips[0].length_frames, 480000);
                assert!(!app.session.project.transport.r#loop.enabled);
                assert!(app.error.is_none());
                app.session.project.validate().unwrap();
            }
        }
    }
    #[test]
    fn time_monitor_shift_drag_clamps_and_stops_when_disabled() {
        for section in 0..3 {
            let mut app = DawUi::default();
            let initial = frames(0.25) + 17;
            app.session.project.transport.playhead_frame = initial;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let start = app.time_bounds[section].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let right = start + Vec2::new(100.0, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(right),
                    egui::Event::ModifiersChanged(egui::Modifiers {
                        shift: true,
                        ..Default::default()
                    }),
                ],
            );
            for point in 1..=9 {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(
                        right - Vec2::new(0.0, point as f32),
                    )],
                );
                assert_eq!(app.session.project.transport.playhead_frame, initial);
                assert!(!app.dirty);
            }
            let up = right - Vec2::new(0.0, 10.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(up)]);
            let step = [3600, 60, 1][section] * 48000;
            assert_eq!(app.session.project.transport.playhead_frame, initial + step);
            let down = up + Vec2::new(0.0, 20.0);
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(Default::default()),
                    egui::Event::PointerMoved(down),
                ],
            );
            assert_eq!(app.session.project.transport.playhead_frame, 0);
            let reverse = down - Vec2::new(0.0, 1.0);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(reverse)]);
            assert_eq!(app.session.project.transport.playhead_frame, step);
            app.dirty = false;
            let (sender, receiver) = mpsc::channel();
            app.job = Some(receiver);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]);
            assert_eq!(app.session.project.transport.playhead_frame, step);
            assert!(!app.dirty);
            assert!(app.time_drag.is_none());
            frame(&mut app, &ctx, vec![button(start, false)]);
            drop(sender);
        }
        let hour = PositionStep::Frames(3600 * 48000);
        assert_eq!(hour.shift(u64::MAX - 10, 1), u64::MAX);
        assert_eq!(hour.shift(0, i64::MIN), 0);
    }
    #[test]
    fn tempo_monitor_reserves_equal_digit_slots() {
        let mut app = DawUi::default();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let default = app.tempo_bounds;
        for tempo in [111.0, 888.0, 60.0, 9.0] {
            app.session.project.tempo_bpm = tempo;
            frame(&mut app, &ctx, vec![]);
            assert_eq!(app.tempo_bounds, default);
        }
        app.session.project.tempo_bpm = 120.25;
        frame(&mut app, &ctx, vec![]);
        let fractional = app.tempo_bounds;
        assert!(fractional.width() > default.width());
        app.session.project.tempo_bpm = 888.88;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.tempo_bounds, fractional);
    }
    #[test]
    fn track_name_double_click_commits_with_enter_and_cancels_with_escape() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let name = Pos2::new(110.0, app.lane_bounds[&track].top() + 18.0);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(name), button(name, true)],
        );
        frame(&mut app, &ctx, vec![button(name, false)]);
        assert!(app.track_name_edit.is_none());
        assert_eq!(app.selected_track, Some(track));
        frame(&mut app, &ctx, vec![button(name, true)]);
        frame(&mut app, &ctx, vec![button(name, false)]);
        assert_eq!(app.track_name_edit.as_ref().unwrap().track, track);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![egui::Event::Text("Bajo 🎸".into())]);
        assert_eq!(app.track_name_edit.as_ref().unwrap().text, "Bajo 🎸");
        assert_eq!(app.session.project.tracks[0].name, "Track 1");
        assert!(!app.dirty);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
        assert!(app.track_name_edit.is_none());
        assert_eq!(app.session.project.tracks[0].name, "Bajo 🎸");
        assert!(app.dirty);

        app.dirty = false;
        for _ in 0..2 {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(name), button(name, true)],
            );
            frame(&mut app, &ctx, vec![button(name, false)]);
        }
        frame(&mut app, &ctx, vec![]);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Text("Discard this".into())],
        );
        frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
        assert!(app.track_name_edit.is_none());
        assert_eq!(app.session.project.tracks[0].name, "Bajo 🎸");
        assert!(!app.dirty);
        app.track_name_edit = Some(TrackNameEdit {
            track,
            text: "Bajo 🎸".into(),
            focus: true,
        });
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
        assert!(app.track_name_edit.is_none());
        assert!(!app.dirty);
    }
    #[test]
    fn track_name_focus_loss_cancels_and_project_changes_clear_the_draft() {
        let (mut app, track) = fixture();
        let ctx = context();
        app.track_name_edit = Some(TrackNameEdit {
            track,
            text: "Draft".into(),
            focus: true,
        });
        frame(&mut app, &ctx, vec![]);
        let outside = app.lane_bounds[&track].min + Vec2::new(40.0, 40.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(outside), button(outside, true)],
        );
        frame(&mut app, &ctx, vec![button(outside, false)]);
        assert!(app.track_name_edit.is_none());
        assert_eq!(app.session.project.tracks[0].name, "Track 1");
        assert!(!app.dirty);
        app.track_name_edit = Some(TrackNameEdit {
            track,
            text: "Draft".into(),
            focus: true,
        });
        app.edit(Edit::DeleteTrack(track));
        assert!(app.track_name_edit.is_none());

        let track = app.session.project.add_track().unwrap();
        app.track_name_edit = Some(TrackNameEdit {
            track,
            text: "Draft".into(),
            focus: true,
        });
        app.pending = Some(Action::CloseProject);
        app.perform_pending();
        assert!(app.track_name_edit.is_none());
    }
    #[test]
    fn inline_label_edits_cancel_on_tab_and_window_focus_loss() {
        for tempo in [false, true] {
            for window_focus_loss in [false, true] {
                let (mut app, track) = fixture();
                let ctx = context();
                if tempo {
                    start_tempo_edit(&mut app, &ctx);
                } else {
                    app.track_name_edit = Some(TrackNameEdit {
                        track,
                        text: "Track 1".into(),
                        focus: true,
                    });
                    frame(&mut app, &ctx, vec![]);
                }
                frame(&mut app, &ctx, vec![egui::Event::Text("90".into())]);
                if window_focus_loss {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            focused: false,
                            events: vec![egui::Event::WindowFocused(false)],
                            ..Default::default()
                        },
                        |ui| app.show(ui),
                    );
                    output.textures_delta.clear();
                } else {
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::Key {
                            key: egui::Key::Tab,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Default::default(),
                        }],
                    );
                    frame(&mut app, &ctx, vec![]);
                }
                assert!(app.track_name_edit.is_none());
                assert!(app.tempo_edit.is_none());
                assert_eq!(app.session.project.tracks[0].name, "Track 1");
                assert_eq!(app.session.project.tempo_bpm, 120.0);
                assert!(!app.dirty);
                assert!(app.error.is_none());
            }
        }
    }
    #[test]
    fn master_numeric_gain_commits_clamps_and_cancels() {
        let mut app = DawUi::default();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let input = app.master_bounds.min + Vec2::new(55.0, 47.0);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        for (text, action, expected) in [
            ("-6.0", egui::Key::Enter, -6.0),
            ("-3.0", egui::Key::Escape, -6.0),
            ("99", egui::Key::Enter, 12.0),
            ("NaN", egui::Key::Enter, 12.0),
        ] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(input), button(input, true)],
            );
            frame(&mut app, &ctx, vec![button(input, false)]);
            app.master_gain_input = text.into();
            frame(&mut app, &ctx, vec![key(action)]);
            assert_eq!(app.session.project.master.gain_db, expected);
            assert_eq!(app.master_gain_input, format!("{expected:.1}"));
        }
        assert!(app.error.is_some());
        app.pending = Some(Action::CloseProject);
        app.perform_pending();
        assert_eq!(app.master_gain_input, "0.0");
    }
    fn frame(app: &mut DawUi, ctx: &egui::Context, events: Vec<egui::Event>) {
        frame_sized(app, ctx, events, Vec2::new(1280.0, 800.0));
    }
    fn frame_sized(app: &mut DawUi, ctx: &egui::Context, events: Vec<egui::Event>, size: Vec2) {
        frame_shapes(app, ctx, events, size);
    }
    fn frame_shapes(
        app: &mut DawUi,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        size: Vec2,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        // These input tests have no GPU renderer. Discard texture updates explicitly.
        output.textures_delta.clear();
        output.shapes
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn shortcut_event(action: FileAction) -> egui::Event {
        let shortcut = action.shortcut();
        egui::Event::Key {
            key: shortcut.logical_key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: shortcut.modifiers,
        }
    }

    #[test]
    fn new_shortcut_preserves_unsaved_project_and_is_blocked_during_jobs() {
        let (mut app, track) = fixture();
        let ctx = context();
        app.dirty = true;
        frame(&mut app, &ctx, vec![shortcut_event(FileAction::New)]);
        assert!(app.unsaved_prompt);
        assert!(matches!(app.pending, Some(Action::New)));
        assert_eq!(app.session.project.tracks[0].id, track);

        app.unsaved_prompt = false;
        app.pending = None;
        let (_tx, rx) = mpsc::channel();
        app.job = Some(rx);
        frame(&mut app, &ctx, vec![shortcut_event(FileAction::New)]);
        assert!(!app.unsaved_prompt);
        assert!(app.pending.is_none());
        assert_eq!(app.session.project.tracks[0].id, track);
    }

    #[test]
    fn close_project_preserves_unsaved_changes_and_is_blocked_during_jobs() {
        let (mut app, track) = fixture();
        let ctx = context();
        app.dirty = true;
        frame(
            &mut app,
            &ctx,
            vec![shortcut_event(FileAction::CloseProject)],
        );
        assert!(app.unsaved_prompt);
        assert!(matches!(app.pending, Some(Action::CloseProject)));
        assert_eq!(app.session.project.tracks[0].id, track);
        assert!(!app.allow_close);

        // Cancelling retains the original project and its dirty state.
        app.pending = None;
        app.unsaved_prompt = false;
        let (_tx, rx) = mpsc::channel();
        app.job = Some(rx);
        app.perform_file_action(FileAction::CloseProject);
        assert!(app.pending.is_none());
        assert_eq!(app.session.project.tracks[0].id, track);
        assert!(app.dirty);

        app.job = None;
        app.perform_file_action(FileAction::CloseProject);
        app.dirty = false;
        app.unsaved_prompt = false;
        app.perform_pending();
        assert!(app.session.project.tracks.is_empty());
        assert!(app.session.project.assets.is_empty());
        assert!(app.session.folder.is_none());
        assert!(!app.allow_close);
    }

    #[test]
    fn close_project_after_save_resets_the_workspace_without_closing_the_window() {
        let (mut app, _) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        app.scroll = 20.0;
        app.dirty = true;
        app.perform_file_action(FileAction::CloseProject);
        let saved = app.session.clone();
        let (tx, rx) = mpsc::channel();
        app.job = Some(rx);
        tx.send(Ok(Job::Saved(saved))).unwrap();
        app.poll();
        assert!(!app.unsaved_prompt);
        assert!(!app.dirty);
        assert!(!app.allow_close);
        assert!(app.pending.is_none());
        assert!(app.session.project.tracks.is_empty());
        assert_eq!(app.session.project.name, "Untitled");
        assert!(app.selected_track.is_none());
        assert!(app.selected_clip.is_none());
        assert!(app.inputs.is_empty());
        assert!(app.lane_bounds.is_empty());
        assert_eq!(app.scroll, 0.0);
    }

    #[test]
    fn save_as_shortcut_takes_precedence_and_empty_export_is_disabled() {
        let app = DawUi::default();
        let ctx = context();
        let mut selected = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![shortcut_event(FileAction::SaveAs)],
                ..Default::default()
            },
            |ui| selected = app.take_file_shortcut(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(selected, Some(FileAction::SaveAs));
        assert!(!app.file_action_enabled(FileAction::Export));
        assert!(app.file_action_enabled(FileAction::Import));
    }

    #[test]
    fn native_menu_owns_shortcuts_and_uses_guarded_file_actions() {
        let (mut app, track) = fixture();
        let ctx = context();
        app.use_native_file_menu();
        frame(&mut app, &ctx, vec![shortcut_event(FileAction::New)]);
        assert_eq!(app.session.project.tracks[0].id, track);

        app.error = Some("Pending error".into());
        app.perform_file_action(FileAction::New);
        assert_eq!(app.session.project.tracks[0].id, track);
        app.error = None;
        app.perform_file_action(FileAction::New);
        assert!(app.session.project.tracks.is_empty());
    }

    #[test]
    fn timeline_scrollbar_moves_lanes_without_changing_tracks() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let start = app.scrollbar_bounds.left_center() + Vec2::new(30.0, -1.0);
        let end = start + Vec2::new(80.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(
            app.scroll > 0.0,
            "scrollbar did not move: {:?}",
            app.scrollbar_bounds
        );
        assert_eq!(app.lane_bounds[&track], lane);
        assert!(!app.dirty);
    }
    #[test]
    fn add_track_button_remains_enabled_beyond_four_tracks() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let add = Pos2::new(
            TRACK_WIDTH - 19.0,
            app.lane_bounds[&track].top() - RULER_HEIGHT / 2.0,
        );
        for expected in 2..=8 {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(add), button(add, true)],
            );
            frame(&mut app, &ctx, vec![button(add, false)]);
            assert_eq!(app.session.project.tracks.len(), expected);
            assert_eq!(
                app.selected_track,
                Some(app.session.project.tracks.last().unwrap().id)
            );
            assert!(app.dirty);
        }
    }
    #[test]
    fn master_stays_pinned_while_tracks_scroll_and_window_resizes() {
        let (mut app, track) = fixture();
        for _ in 0..7 {
            app.session.project.add_track().unwrap();
        }
        let ctx = context();
        // Eight compact tracks must overflow to exercise scrolling.
        let size = Vec2::new(900.0, 450.0);
        frame_sized(&mut app, &ctx, vec![], size);
        let master = app.master_bounds;
        let lane = app.lane_bounds[&track];
        assert_eq!(master.width(), TRACK_WIDTH);
        assert_eq!(master.height(), MASTER_HEIGHT);
        assert_eq!(master.bottom(), app.scrollbar_bounds.bottom());
        frame_sized(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(lane.min + Vec2::new(30.0, 40.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -200.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
            size,
        );
        frame_sized(&mut app, &ctx, vec![], size);
        assert!(app.lane_bounds[&track].top() < lane.top());
        assert_eq!(app.master_bounds, master);
        assert!(!app.dirty);
        assert_eq!(app.session.project.tracks.len(), 8);

        frame_sized(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert_eq!(app.master_bounds.bottom(), app.scrollbar_bounds.bottom());
        assert!(app.master_bounds.top() > master.top());
        assert_eq!(app.master_bounds.height(), MASTER_HEIGHT);
    }
    #[test]
    fn clip_header_drag_moves_by_total_distance() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let start = app.lane_bounds[&track].min + Vec2::new(20.0, 12.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let middle = start + Vec2::new(35.0, 0.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(middle)]);
        let end = start + Vec2::new(70.0, 0.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 48000);
        assert!(app.dirty);
    }
    #[test]
    fn musical_ruler_seek_and_loop_match_the_project_tempo() {
        let (mut app, track) = fixture();
        app.session.project.tempo_bpm = 60.0;
        let original_clip = app.session.project.tracks[0].clips[0].clone();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let bar_two = app
            .musical_timeline()
            .ticks(lane.width())
            .find(|tick| tick.beat == 4.0)
            .unwrap()
            .x;
        let label = Pos2::new(lane.left() + bar_two + 3.0, lane.top() - 29.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(label), button(label, true)],
        );
        frame(&mut app, &ctx, vec![button(label, false)]);
        assert_eq!(
            app.session.project.transport.playhead_frame,
            frames(4.0 + 3.0 / 70.0)
        );
        assert_eq!(
            musical_time::monitor(app.session.project.transport.playhead_frame, 60.0),
            "0002.01"
        );
        let start = Pos2::new(lane.left() + bar_two, lane.top() - 10.0);
        let end = start + Vec2::new(280.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let region = &app.session.project.transport.r#loop;
        assert_eq!(region.start_frame, frames(4.0));
        assert_eq!(region.end_frame, frames(8.0));
        assert_eq!(musical_time::monitor(region.end_frame, 60.0), "0003.01");
        app.session.project.tempo_bpm = 120.0;
        assert_eq!(musical_time::monitor(region.start_frame, 120.0), "0003.01");
        let clip = &app.session.project.tracks[0].clips[0];
        assert_eq!(clip.start_frame, original_clip.start_frame);
        assert_eq!(clip.length_frames, original_clip.length_frames);
        assert_eq!(clip.source_offset_frame, original_clip.source_offset_frame);
    }
    #[test]
    fn ruler_selects_a_loop_and_track_controls_align() {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let start = Pos2::new(lane.left() + 70.0, lane.top() - 10.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let end = start + Vec2::new(140.0, 0.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let region = &app.session.project.transport.r#loop;
        assert_eq!(region.start_frame, 48000);
        assert_eq!(region.end_frame, 144000);
        assert!(app.dirty);
        app.session.project.transport.r#loop.enabled = true;
        let start = Pos2::new(lane.left() + 70.0, lane.top() - 10.0);
        let end = start + Vec2::new(0.0, 15.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        app.session.project.validate().unwrap();
        assert_eq!(app.session.project.transport.r#loop.end_frame, 144000);
        assert!(app.error.is_some());
    }

    #[test]
    fn ruler_label_band_cannot_edit_selection() {
        let (mut app, track) = fixture();
        app.session.project.transport.r#loop.start_frame = 48000;
        app.session.project.transport.r#loop.end_frame = 144000;
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let start = Pos2::new(lane.left() + 280.0, lane.top() - 35.0);
        // Starting in the labels must not edit a selection, even when the
        // pointer later enters the lower tick band.
        let end = start + Vec2::new(140.0, 30.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let region = &app.session.project.transport.r#loop;
        assert_eq!((region.start_frame, region.end_frame), (48000, 144000));
        assert!(!app.dirty);
        assert!(app.error.is_none());

        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![button(start, false)]);
        assert_eq!(app.session.project.transport.playhead_frame, 192000);
        let region = &app.session.project.transport.r#loop;
        assert_eq!((region.start_frame, region.end_frame), (48000, 144000));
    }
    #[derive(Debug)]
    struct TestDrop(std::path::PathBuf);
    impl egui::DroppedFile for TestDrop {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|error| error.to_string())
        }
    }
    struct TestWav(std::path::PathBuf);
    impl TestWav {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("daw-file-hover-{}.wav", Id::new_v4()));
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 48000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for index in 0..48000 {
                writer
                    .write_sample(if index % 128 < 64 {
                        8000_i16
                    } else {
                        -8000_i16
                    })
                    .unwrap();
            }
            writer.finalize().unwrap();
            Self(path)
        }
    }
    impl Drop for TestWav {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn file_frame(
        app: &mut DawUi,
        ctx: &egui::Context,
        path: &std::path::Path,
        pointer: Pos2,
        dropped: bool,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                events: vec![egui::Event::PointerMoved(pointer)],
                hovered_files: if dropped {
                    vec![]
                } else {
                    vec![egui::HoveredFile {
                        path: Some(path.to_path_buf()),
                        ..Default::default()
                    }]
                },
                dropped_files: if dropped {
                    vec![std::sync::Arc::new(TestDrop(path.to_path_buf()))]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        output.shapes
    }
    fn ready_file_hover(
        app: &mut DawUi,
        ctx: &egui::Context,
        path: &std::path::Path,
        pointer: Pos2,
    ) -> Vec<egui::epaint::ClippedShape> {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let shapes = file_frame(app, ctx, path, pointer, false);
            if app
                .file_hover
                .as_ref()
                .is_some_and(|hover| hover.audio.is_some())
            {
                return shapes;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "file preview worker timed out"
            );
            std::thread::yield_now();
        }
    }
    fn finish_import(app: &mut DawUi, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.job.is_some() {
            frame(app, ctx, vec![]);
            assert!(
                std::time::Instant::now() < deadline,
                "file import worker timed out"
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn file_hover_previews_the_cursor_target_and_drop_reuses_prepared_audio() {
        let source = TestWav::new();
        let (mut app, first) = fixture();
        let second = app.session.project.add_track().unwrap();
        app.selected_track = Some(first);
        app.session.project.transport.playhead_frame = frames(9.0);
        app.zoom = 140.0;
        app.scroll = 1.0;
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&second];
        let point = lane.min + Vec2::new(280.0, 12.0);
        let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
        let target = app.file_drop_target.as_ref().unwrap();
        assert_eq!(target.track, Some(second));
        assert_eq!(target.start, frames(3.0));
        let block = Rect::from_min_size(point - Vec2::new(0.0, 12.0), Vec2::new(140.0, ROW_HEIGHT));
        assert!(has_preview_outline(&shapes, block, theme::ACCENT));
        assert!(shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::LineSegment {stroke, ..}
            if stroke.color == theme::WAVEFORM.gamma_multiply(0.75))
        ));
        assert!(app.session.project.tracks[1].clips.is_empty());
        assert_eq!(app.session.project.assets.len(), 1);
        assert!(!app.dirty);
        let audio = app
            .file_hover
            .as_ref()
            .unwrap()
            .audio
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .samples
            .clone();
        let moved = point + Vec2::new(140.0, 0.0);
        file_frame(&mut app, &ctx, &source.0, moved, false);
        assert_eq!(app.file_drop_target.as_ref().unwrap().start, frames(4.0));
        file_frame(&mut app, &ctx, &source.0, moved, true);
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none());
        let clip = &app.session.project.tracks[1].clips[0];
        assert_eq!(clip.start_frame, frames(4.0));
        assert_eq!(clip.length_frames, 48000);
        assert!(std::sync::Arc::ptr_eq(
            &audio,
            &app.session.audio[&clip.asset_id].samples
        ));
        assert!(app.dirty);
        assert!(app.file_hover.is_none());
    }
    #[test]
    fn file_drag_over_track_controls_clamps_preview_and_drop_to_track_start() {
        let source = TestWav::new();
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 0.5)] {
            let (mut app, first) = fixture();
            let second = app.session.project.add_track().unwrap();
            app.selected_track = Some(first);
            app.zoom = zoom;
            app.scroll = scroll;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&second];
            let point = lane.min + Vec2::new(140.0, 12.0);
            ready_file_hover(&mut app, &ctx, &source.0, point);
            assert!(app.file_drop_target.as_ref().unwrap().start > 0);

            let controls = Pos2::new(lane.left() - 120.0, point.y);
            let shapes = file_frame(&mut app, &ctx, &source.0, controls, false);
            let target = app.file_drop_target.as_ref().unwrap();
            assert_eq!(target.track, Some(second));
            assert_eq!(target.start, 0);
            let block = Rect::from_min_size(
                lane.min - Vec2::new(scroll as f32 * zoom, 0.0),
                Vec2::new(zoom, ROW_HEIGHT),
            );
            assert!(has_preview_outline(&shapes, block, theme::ACCENT));
            assert!(app.session.project.tracks[1].clips.is_empty());
            assert!(!app.dirty);

            file_frame(&mut app, &ctx, &source.0, controls, true);
            finish_import(&mut app, &ctx);
            assert!(app.error.is_none());
            let clip = &app.session.project.tracks[1].clips[0];
            assert_eq!(clip.start_frame, 0);
            assert_eq!(clip.source_offset_frame, 0);
            assert_eq!(clip.length_frames, 48000);
            assert_eq!(app.session.project.tracks[0].clips.len(), 1);
            assert!(app.dirty);
        }
    }
    #[test]
    fn file_drop_on_empty_timeline_previews_and_creates_a_track() {
        let source = TestWav::new();
        let mut app = DawUi::default();
        let ctx = context();
        let point = Pos2::new(TRACK_WIDTH + 140.0, 140.0);
        let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
        let target = app.file_drop_target.as_ref().unwrap();
        assert_eq!(target.track, None);
        assert_eq!(target.start, frames(2.0));
        let block = Rect::from_min_size(
            Pos2::new(point.x, target.lane.top()),
            Vec2::new(70.0, ROW_HEIGHT),
        );
        assert!(has_preview_outline(&shapes, block, theme::ACCENT));
        assert!(app.session.project.tracks.is_empty());
        file_frame(&mut app, &ctx, &source.0, point, true);
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none());
        assert_eq!(app.session.project.tracks.len(), 1);
        assert_eq!(
            app.session.project.tracks[0].clips[0].start_frame,
            frames(2.0)
        );
    }
    #[test]
    fn rejected_file_drop_and_cancelled_hover_leave_the_project_unchanged() {
        let source = TestWav::new();
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let point = lane.min + Vec2::new(70.0, 12.0);
        let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
        let block =
            Rect::from_min_size(lane.min + Vec2::new(70.0, 0.0), Vec2::new(70.0, ROW_HEIGHT));
        assert!(has_preview_outline(&shapes, block, theme::ERROR));
        frame(&mut app, &ctx, vec![]);
        assert!(app.file_hover.is_none());
        assert!(app.file_drop_target.is_none());
        assert!(app.job.is_none());
        assert!(!app.dirty);
        ready_file_hover(&mut app, &ctx, &source.0, point);
        file_frame(&mut app, &ctx, &source.0, point, true);
        finish_import(&mut app, &ctx);
        assert!(
            app.error
                .as_ref()
                .is_some_and(|error| error.contains("overlap"))
        );
        assert_eq!(app.session.project.assets.len(), 1);
        assert_eq!(app.session.project.tracks[0].clips.len(), 1);
        assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
        assert!(!app.dirty);
    }
    #[test]
    fn direct_file_drop_imports_at_the_cursor_and_drops_outside_the_track_workspace_are_ignored() {
        let source = TestWav::new();
        let mut app = DawUi::default();
        let ctx = context();
        let point = Pos2::new(TRACK_WIDTH + 210.0, 140.0);
        file_frame(&mut app, &ctx, &source.0, point, true);
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none());
        assert_eq!(
            app.session.project.tracks[0].clips[0].start_frame,
            frames(3.0)
        );
        app.scroll = 2.0;
        let lane = app.lane_bounds[&app.session.project.tracks[0].id];
        file_frame(
            &mut app,
            &ctx,
            &source.0,
            Pos2::new(30.0, lane.center().y),
            true,
        );
        finish_import(&mut app, &ctx);
        assert!(app.error.is_none());
        assert_eq!(app.session.project.tracks[0].clips.len(), 2);
        assert!(
            app.session.project.tracks[0]
                .clips
                .iter()
                .any(|clip| clip.start_frame == 0)
        );
        app.dirty = false;
        let lane = app.lane_bounds[&app.session.project.tracks[0].id];
        let master = app.master_bounds.center();
        for point in [
            Pos2::new(30.0, 20.0),
            Pos2::new(30.0, lane.top() - 12.0),
            Pos2::new(-10.0, lane.center().y),
            master,
        ] {
            file_frame(&mut app, &ctx, &source.0, point, true);
            assert!(app.file_drop_target.is_none());
            assert!(app.job.is_none());
            assert_eq!(app.session.project.assets.len(), 2);
            assert!(!app.dirty);
        }
        frame(&mut app, &ctx, vec![]);
        assert!(app.file_hover.is_none());
    }
    #[test]
    fn invalid_file_hover_is_red_and_import_failure_does_not_create_a_track() {
        let source = TestWav::new();
        std::fs::write(&source.0, b"invalid WAV").unwrap();
        let mut app = DawUi::default();
        let ctx = context();
        let point = Pos2::new(TRACK_WIDTH + 140.0, 140.0);
        let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
        assert!(
            app.file_hover
                .as_ref()
                .unwrap()
                .audio
                .as_ref()
                .unwrap()
                .is_err()
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.stroke == Stroke::new(2.0, theme::ERROR)))
        );
        assert!(!app.dirty);
        file_frame(&mut app, &ctx, &source.0, point, true);
        finish_import(&mut app, &ctx);
        assert!(app.error.is_some());
        assert!(app.session.project.tracks.is_empty());
        assert!(app.session.project.assets.is_empty());
        assert!(!app.dirty);
    }
    fn has_preview_outline(
        shapes: &[egui::epaint::ClippedShape],
        block: Rect,
        color: egui::Color32,
    ) -> bool {
        shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.rect == block && rect.stroke == Stroke::new(2.0, color))
        })
    }
    #[test]
    fn clip_move_preview_is_painted_at_the_drop_position_without_editing_the_project() {
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            let (mut app, first) = fixture();
            let second = app.session.project.add_track().unwrap();
            app.zoom = zoom;
            app.scroll = scroll;
            app.session.project.tracks[0].clips[0].start_frame = frames(2.0);
            let original = app.session.project.tracks[0].clips[0].clone();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let start =
                app.lane_bounds[&first].min + Vec2::new((2.0 - scroll as f32) * zoom + 20.0, 12.0);
            let end = Pos2::new(start.x + 1.25 * zoom, app.lane_bounds[&second].top() + 12.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end)],
                Vec2::new(1280.0, 800.0),
            );
            let preview = app.move_preview(end).unwrap();
            assert_eq!(preview.track, second);
            assert_eq!(preview.start, frames(3.25));
            assert!(preview.valid);
            let lane = app.lane_bounds[&second];
            let block = Rect::from_min_size(
                lane.min + Vec2::new((3.25 - scroll as f32) * zoom, 0.0),
                Vec2::new(10.0 * zoom, ROW_HEIGHT),
            );
            assert!(has_preview_outline(&shapes, block, theme::ACCENT));
            assert_eq!(
                app.session.project.tracks[0].clips[0].start_frame,
                original.start_frame
            );
            assert!(app.session.project.tracks[1].clips.is_empty());
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.drag.is_none());
            assert!(app.session.project.tracks[0].clips.is_empty());
            let placed = &app.session.project.tracks[1].clips[0];
            assert_eq!(placed.start_frame, preview.start);
            assert_eq!(placed.source_offset_frame, original.source_offset_frame);
            assert_eq!(placed.length_frames, original.length_frames);
            assert!(app.error.is_none());
        }
    }
    #[test]
    fn moving_left_clamps_the_preview_and_drop_to_the_track_start() {
        for across_tracks in [false, true] {
            let (mut app, first) = fixture();
            let second = app.session.project.add_track().unwrap();
            app.session.project.tracks[0].clips[0].start_frame = frames(2.0);
            let original = app.session.project.tracks[0].clips[0].clone();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let start = app.lane_bounds[&first].min + Vec2::new(180.0, 12.0);
            let target = if across_tracks { second } else { first };
            let lane = app.lane_bounds[&target];
            let end = lane.min + Vec2::new(if across_tracks { 10.0 } else { -100.0 }, 12.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end)],
                Vec2::new(1280.0, 800.0),
            );
            let preview = app.move_preview(end).unwrap();
            assert_eq!(preview.track, target);
            assert_eq!(preview.start, 0);
            assert!(preview.valid);
            let block = Rect::from_min_size(lane.min, Vec2::new(700.0, ROW_HEIGHT));
            assert!(has_preview_outline(&shapes, block, theme::ACCENT));
            assert_eq!(
                app.session.project.tracks[0].clips[0].start_frame,
                frames(2.0)
            );
            frame(&mut app, &ctx, vec![button(end, false)]);
            let placed = &app
                .session
                .project
                .tracks
                .iter()
                .find(|track| track.id == target)
                .unwrap()
                .clips[0];
            assert_eq!(placed.start_frame, 0);
            assert_eq!(placed.source_offset_frame, original.source_offset_frame);
            assert_eq!(placed.length_frames, original.length_frames);
            assert!(app.error.is_none());
        }
    }
    #[test]
    fn overlapping_move_preview_is_red_and_rejected_drop_preserves_the_clips() {
        let (mut app, first) = fixture();
        let second = app.session.project.add_track().unwrap();
        let mut occupied = app.session.project.tracks[0].clips[0].clone();
        occupied.id = Id::new_v4();
        app.session.project.tracks[1].clips.push(occupied);
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let start = app.lane_bounds[&first].min + Vec2::new(20.0, 12.0);
        let lane = app.lane_bounds[&second];
        let end = lane.min + Vec2::new(90.0, 12.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let shapes = frame_shapes(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end)],
            Vec2::new(1280.0, 800.0),
        );
        let preview = app.move_preview(end).unwrap();
        assert!(!preview.valid);
        let block = Rect::from_min_size(
            lane.min + Vec2::new(70.0, 0.0),
            Vec2::new(700.0, ROW_HEIGHT),
        );
        assert!(has_preview_outline(&shapes, block, theme::ERROR));
        assert!(app.error.is_none());
        assert!(!app.dirty);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(app.drag.is_none());
        assert!(
            app.error
                .as_ref()
                .is_some_and(|error| error.contains("overlap"))
        );
        assert!(!app.dirty);
        for track in &app.session.project.tracks {
            assert_eq!(track.clips.len(), 1);
            assert_eq!(track.clips[0].start_frame, 0);
        }
    }
    #[test]
    fn clips_move_between_tracks_and_trim_without_changing_sources() {
        let (mut app, first) = fixture();
        let second = app.session.project.add_track().unwrap();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let start = app.lane_bounds[&first].min + Vec2::new(20.0, 12.0);
        let end = app.lane_bounds[&second].min + Vec2::new(90.0, 12.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(app.session.project.tracks[0].clips.is_empty());
        assert_eq!(app.session.project.tracks[1].clips[0].start_frame, 48000);
        frame(&mut app, &ctx, vec![]);
        let start = app.lane_bounds[&second].min + Vec2::new(71.0, 40.0);
        let end = start + Vec2::new(70.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let clip = &app.session.project.tracks[1].clips[0];
        assert_eq!(
            (
                clip.start_frame,
                clip.source_offset_frame,
                clip.length_frames
            ),
            (96000, 48000, 432000)
        );
        assert_eq!(app.session.project.assets[0].decoded_frame_count, 480000);
    }
}
