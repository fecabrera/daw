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
mod waveforms;

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
    mode: ClipDragMode,
    origin: Pos2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipDragMode {
    Move,
    TrimLeft,
    TrimRight,
    LoopLeft,
    LoopRight,
    Body,
}

impl ClipDragMode {
    fn at(block: Rect, pointer: Pos2) -> Self {
        let left = (pointer.x - block.left()).abs();
        let right = (pointer.x - block.right()).abs();
        if left.min(right) < 8.0 {
            match (pointer.y < block.top() + 22.0, left <= right) {
                (true, true) => Self::LoopLeft,
                (true, false) => Self::LoopRight,
                (false, true) => Self::TrimLeft,
                (false, false) => Self::TrimRight,
            }
        } else if pointer.y < block.top() + 26.0 {
            Self::Move
        } else {
            Self::Body
        }
    }
}
struct ClipPreview {
    track: Id,
    clip: Clip,
    valid: bool,
}
struct FileDropTarget {
    track: Option<Id>,
    start: u64,
    lane: Rect,
}
#[derive(Clone, Copy)]
enum LoopDragMode {
    Create,
    Start,
    End,
    Move,
}
struct LoopDrag {
    origin: Pos2,
    mode: LoopDragMode,
    original: daw_core::Loop,
    dirty: bool,
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
                    self.loop_drag = None;
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
        self.timeline_swipe(
            ui,
            Rect::from_min_max(Pos2::new(timeline_left, bounds.top()), bounds.max),
        );
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
    fn timeline_extent(&self) -> f64 {
        seconds(self.session.project.end()).max(60.0) + 30.0
    }
    fn timeline_swipe(&mut self, ui: &egui::Ui, viewport: Rect) {
        if !ui.is_enabled()
            || !ui.input(|input| input.focused)
            || !ui.rect_contains_pointer(viewport)
        {
            return;
        }
        let delta = ui.input_mut(|input| std::mem::take(&mut input.smooth_scroll_delta.x));
        if delta == 0.0 {
            return;
        }
        let maximum =
            (self.timeline_extent() - f64::from(viewport.width()) / f64::from(self.zoom)).max(0.0);
        let scroll = (self.scroll - f64::from(delta) / f64::from(self.zoom)).clamp(0.0, maximum);
        if scroll != self.scroll {
            self.scroll = scroll;
            ui.ctx().request_repaint();
        }
    }
    fn timeline_scrollbar(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            let width = ui.available_width();
            let extent = self.timeline_extent();
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
    fn loop_pixels(&self, rect: Rect) -> (f32, f32) {
        let region = &self.session.project.transport.r#loop;
        let pixel = |frame| rect.left() + ((seconds(frame) - self.scroll) as f32) * self.zoom;
        (pixel(region.start_frame), pixel(region.end_frame))
    }
    fn loop_drag_mode(&self, rect: Rect, pointer: Pos2) -> LoopDragMode {
        let region = &self.session.project.transport.r#loop;
        if region.end_frame <= region.start_frame {
            return LoopDragMode::Create;
        }
        let (left, right) = self.loop_pixels(rect);
        let left_distance = (pointer.x - left).abs();
        let right_distance = (pointer.x - right).abs();
        if left_distance.min(right_distance) <= 5.0 {
            if left_distance <= right_distance {
                LoopDragMode::Start
            } else {
                LoopDragMode::End
            }
        } else if (left..right).contains(&pointer.x) {
            LoopDragMode::Move
        } else {
            LoopDragMode::Create
        }
    }
    fn ruler_frame(&self, rect: Rect, pointer: Pos2) -> u64 {
        frames(self.scroll + f64::from((pointer.x - rect.left()) / self.zoom))
    }
    fn ruler_interaction(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        selection: &egui::Response,
        playhead: &egui::Response,
    ) {
        if !ui.is_enabled() || !ui.input(|input| input.focused) {
            if let Some(drag) = self.loop_drag.take() {
                self.session.project.transport.r#loop = drag.original;
                self.dirty = drag.dirty;
                self.sync_needed = true;
            }
            return;
        }
        if selection.drag_started_by(egui::PointerButton::Primary) {
            let origin = ui.input(|input| input.pointer.press_origin()).unwrap();
            self.loop_drag = Some(LoopDrag {
                origin,
                mode: self.loop_drag_mode(rect, origin),
                original: self.session.project.transport.r#loop.clone(),
                dirty: self.dirty,
            });
        }
        if (selection.dragged_by(egui::PointerButton::Primary)
            || selection.drag_stopped_by(egui::PointerButton::Primary))
            && let Some(pointer) = selection.interact_pointer_pos()
            && let Some(drag) = &self.loop_drag
        {
            let at = self.ruler_frame(rect, pointer);
            let original = &drag.original;
            let unsnapped = ui.input(|input| input.modifiers.shift);
            let (start, end) = match drag.mode {
                LoopDragMode::Create => {
                    let origin = self.selection_frame(
                        i128::from(self.ruler_frame(rect, drag.origin)),
                        0..=u64::MAX,
                        unsnapped,
                    );
                    let at = self.selection_frame(i128::from(at), 0..=u64::MAX, unsnapped);
                    (origin.min(at), origin.max(at))
                }
                LoopDragMode::Start => (
                    self.selection_frame(
                        i128::from(at),
                        0..=original.end_frame - 1,
                        unsnapped || at == original.start_frame,
                    ),
                    original.end_frame,
                ),
                LoopDragMode::End => (
                    original.start_frame,
                    self.selection_frame(
                        i128::from(at),
                        original.start_frame + 1..=u64::MAX,
                        unsnapped || at == original.end_frame,
                    ),
                ),
                LoopDragMode::Move => {
                    let length = original.end_frame - original.start_frame;
                    let delta = (f64::from(pointer.x - drag.origin.x) / f64::from(self.zoom)
                        * f64::from(daw_core::SAMPLE_RATE))
                    .round() as i128;
                    let start = self.selection_frame(
                        i128::from(original.start_frame) + delta,
                        0..=u64::MAX - length,
                        unsnapped || delta == 0,
                    );
                    (start, start + length)
                }
            };
            let region = &mut self.session.project.transport.r#loop;
            if end > start && (region.start_frame, region.end_frame) != (start, end) {
                region.start_frame = start;
                region.end_frame = end;
                self.sync_needed = true;
            }
        }
        if selection.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(drag) = self.loop_drag.take()
        {
            let region = &self.session.project.transport.r#loop;
            if (region.start_frame, region.end_frame)
                != (drag.original.start_frame, drag.original.end_frame)
            {
                self.changed();
            }
        }
        if (playhead.clicked_by(egui::PointerButton::Primary)
            || playhead.dragged_by(egui::PointerButton::Primary)
            || playhead.drag_stopped_by(egui::PointerButton::Primary))
            && let Some(pointer) = playhead.interact_pointer_pos()
        {
            let at = self.selection_frame(
                i128::from(self.ruler_frame(rect, pointer)),
                0..=u64::MAX,
                ui.input(|input| input.modifiers.shift),
            );
            if at != self.session.project.transport.playhead_frame {
                self.seek(at);
            }
        }
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
            let selection_response = ui.interact(
                labels, response.id.with("selection"), Sense::click_and_drag(),
            ).on_hover_text("Drag empty space to select a loop range. Drag its edges to resize or its body to move. Hold Shift to bypass snapping.");
            let playhead_response = ui
                .interact(ticks, response.id.with("playhead"), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(
                    "Click or drag to position the playhead. Hold Shift to bypass snapping.",
                );
            self.ruler_interaction(ui, rect, &selection_response, &playhead_response);
            let painter = ui.painter().with_clip_rect(rect);
            painter.rect_filled(rect, 0.0, theme::BACKGROUND);
            painter.rect_filled(labels, 0.0, theme::PANEL);
            let l = &self.session.project.transport.r#loop;
            let (x1, x2) = self.loop_pixels(rect);
            if l.end_frame > l.start_frame && x2 > rect.left() && x1 < rect.right() {
                let band = Rect::from_min_max(
                    Pos2::new(x1.max(rect.left()), labels.top()),
                    Pos2::new(x2.min(rect.right()), labels.bottom()),
                );
                painter.rect_filled(
                    band,
                    0.0,
                    if l.enabled {
                        theme::RULER_SELECTION
                    } else {
                        theme::RULER_SELECTION_INACTIVE
                    },
                );
                // Draw handles at the real endpoints, including when the range is scrolled.
                for x in [x1, x2] {
                    if (rect.left()..=rect.right()).contains(&x) {
                        painter.rect_filled(
                            Rect::from_min_max(
                                Pos2::new(x - 1.5, labels.top() + 2.0),
                                Pos2::new(x + 1.5, labels.bottom() - 2.0),
                            ),
                            0.0,
                            theme::ACCENT,
                        );
                    }
                }
            }
            if let Some(pointer) = selection_response.hover_pos() {
                let mode = self
                    .loop_drag
                    .as_ref()
                    .map_or_else(|| self.loop_drag_mode(rect, pointer), |drag| drag.mode);
                ui.ctx().set_cursor_icon(match mode {
                    LoopDragMode::Create => egui::CursorIcon::Crosshair,
                    LoopDragMode::Start | LoopDragMode::End => egui::CursorIcon::ResizeHorizontal,
                    LoopDragMode::Move if selection_response.dragged() => {
                        egui::CursorIcon::Grabbing
                    }
                    LoopDragMode::Move => egui::CursorIcon::Grab,
                });
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
                let top = if tick.bar {
                    rect.top()
                } else if tick.whole_beat {
                    ticks.top()
                } else {
                    ticks.bottom() - 5.0
                };
                painter.line_segment(
                    [Pos2::new(x, top), Pos2::new(x, ticks.bottom())],
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
                        Pos2::new(playhead_x, ticks.top()),
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
                    self.lane(ui, &track);
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
    fn move_start(&self, drag: &Drag, pointer: Pos2, track: Id, unsnapped: bool) -> u64 {
        let raw = i128::from(drag.clip.start_frame) + i128::from(self.drag_delta(drag, pointer));
        let start = raw.clamp(0, i128::from(u64::MAX - drag.clip.length_frames)) as u64;
        if unsnapped || raw != i128::from(start) {
            return start;
        }
        let anchors = self
            .session
            .project
            .tracks
            .iter()
            .filter(|candidate| candidate.id == track)
            .flat_map(|track| &track.clips)
            .filter(|clip| clip.id != drag.clip.id)
            .flat_map(|clip| [clip.start_frame, clip.end()]);
        self.musical_timeline()
            .snap_range(start, drag.clip.length_frames, anchors)
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
    fn snap_frame(
        &self,
        raw: i128,
        limits: std::ops::RangeInclusive<u64>,
        anchors: impl IntoIterator<Item = u64>,
        unsnapped: bool,
    ) -> u64 {
        let frame = raw.clamp(i128::from(*limits.start()), i128::from(*limits.end())) as u64;
        if unsnapped || raw != i128::from(frame) {
            return frame;
        }
        self.musical_timeline().snap(frame, limits, anchors)
    }
    fn clip_edge_frame(
        &self,
        drag: &Drag,
        raw: i128,
        limits: std::ops::RangeInclusive<u64>,
        fixed: u64,
        unsnapped: bool,
    ) -> u64 {
        let clip = &drag.clip;
        let length = clip
            .repeat
            .map_or(clip.length_frames, |repeat| repeat.length_frames);
        let bounded = raw.clamp(0, i128::from(u64::MAX)) as u64;
        if !unsnapped
            && raw == i128::from(bounded)
            && limits.contains(&bounded)
            && matches!(drag.mode, ClipDragMode::LoopLeft | ClipDragMode::LoopRight)
            && let Some(frame) = self.musical_timeline().snap_anchor(
                bounded,
                limits.clone(),
                musical_time::size_boundaries(bounded, fixed, length),
            )
        {
            return frame;
        }
        let anchors = [clip.start_frame, clip.end(), *limits.start(), *limits.end()];
        self.snap_frame(raw, limits, anchors, unsnapped)
    }
    fn selection_frame(
        &self,
        raw: i128,
        limits: std::ops::RangeInclusive<u64>,
        unsnapped: bool,
    ) -> u64 {
        let bounded = raw.clamp(0, i128::from(u64::MAX)) as u64;
        let anchors = self.session.project.tracks.iter().flat_map(|track| {
            track.clips.iter().flat_map(|clip| {
                let length = clip
                    .repeat
                    .map_or(clip.length_frames, |repeat| repeat.length_frames);
                [clip.start_frame, clip.end()]
                    .into_iter()
                    .chain(musical_time::size_boundaries(
                        bounded,
                        clip.start_frame,
                        length,
                    ))
                    .filter(move |frame| *frame >= clip.start_frame && *frame <= clip.end())
            })
        });
        self.snap_frame(raw, limits, anchors, unsnapped)
    }
    fn clip_drag_preview(
        &self,
        drag: &Drag,
        pointer: Pos2,
        unsnapped: bool,
    ) -> Option<ClipPreview> {
        if drag.mode == ClipDragMode::Body {
            return None;
        }
        let track = if drag.mode == ClipDragMode::Move {
            self.drag_destination(drag, pointer)
        } else {
            drag.track
        };
        self.lane_bounds.get(&track)?;
        let mut clip = drag.clip.clone();
        if matches!(drag.mode, ClipDragMode::TrimLeft | ClipDragMode::TrimRight) {
            // Also restore trimming for clips saved with an unnecessary loop flag.
            clip.restore_source_range();
        }
        let delta = i128::from(self.drag_delta(drag, pointer));
        let start = i128::from(clip.start_frame);
        let offset = i128::from(clip.source_offset_frame);
        let length = i128::from(clip.length_frames);
        // Do not shift an unchanged edge to a nearby grid line.
        let unsnapped = unsnapped || delta == 0;
        // A split or body trim can already be shorter than its saved repeat base.
        // Keep that range intact rather than extending it on an inward header drag.
        let loop_minimum = clip.repeat.map_or(length, |repeat| {
            length.min(i128::from(repeat.length_frames))
        });
        match drag.mode {
            ClipDragMode::TrimLeft if clip.repeat.is_some() => {
                let next = self.clip_edge_frame(
                    drag,
                    start + delta,
                    clip.start_frame..=clip.end() - 1,
                    clip.end(),
                    unsnapped,
                );
                let delta = i128::from(next) - start;
                clip.start_frame = (start + delta) as u64;
                clip.length_frames = (length - delta) as u64;
                clip.repeat = clip.repeat.map(|repeat| repeat.shifted(delta));
            }
            ClipDragMode::TrimLeft => {
                let minimum = (start - offset.min(start)) as u64;
                let next = self.clip_edge_frame(
                    drag,
                    start + delta,
                    minimum..=clip.end() - 1,
                    clip.end(),
                    unsnapped,
                );
                let delta = i128::from(next) - start;
                clip.start_frame = (start + delta) as u64;
                clip.source_offset_frame = (offset + delta) as u64;
                clip.length_frames = (length - delta) as u64;
            }
            ClipDragMode::TrimRight => {
                let source_length = i128::from(
                    self.session
                        .project
                        .assets
                        .iter()
                        .find(|asset| asset.id == clip.asset_id)?
                        .decoded_frame_count,
                );
                let maximum = if clip.repeat.is_some() {
                    length
                } else {
                    (source_length - offset).min(i128::from(u64::MAX) - start)
                };
                let end = self.clip_edge_frame(
                    drag,
                    start + length + delta,
                    clip.start_frame + 1..=(start + maximum) as u64,
                    clip.start_frame,
                    unsnapped,
                );
                clip.length_frames = end - clip.start_frame;
            }
            ClipDragMode::LoopLeft => {
                let next = self.clip_edge_frame(
                    drag,
                    start + delta,
                    0..=(start + length - loop_minimum) as u64,
                    clip.end(),
                    unsnapped,
                );
                let delta = i128::from(next) - start;
                if delta != 0 {
                    let repeat = clip.repeat.unwrap_or(daw_core::ClipLoop {
                        length_frames: clip.length_frames,
                        phase_frame: 0,
                    });
                    clip.start_frame = (start + delta) as u64;
                    clip.length_frames = (length - delta) as u64;
                    clip.repeat = Some(repeat.shifted(delta));
                }
            }
            ClipDragMode::LoopRight => {
                let end = self.clip_edge_frame(
                    drag,
                    start + length + delta,
                    (start + loop_minimum) as u64..=u64::MAX,
                    clip.start_frame,
                    unsnapped,
                );
                let new_length = end - clip.start_frame;
                if new_length != clip.length_frames {
                    clip.repeat = Some(clip.repeat.unwrap_or(daw_core::ClipLoop {
                        length_frames: clip.length_frames,
                        phase_frame: 0,
                    }));
                    clip.length_frames = new_length;
                }
            }
            _ => clip.start_frame = self.move_start(drag, pointer, track, unsnapped),
        }
        if matches!(
            drag.mode,
            ClipDragMode::LoopLeft
                | ClipDragMode::LoopRight
                | ClipDragMode::TrimLeft
                | ClipDragMode::TrimRight
        ) && (clip.start_frame != drag.clip.start_frame
            || clip.length_frames != drag.clip.length_frames)
        {
            clip.restore_source_range();
        }
        let valid = self.placement_valid(
            Some(track),
            Some(clip.id),
            clip.start_frame,
            clip.length_frames,
        );
        Some(ClipPreview { track, clip, valid })
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
            color: None,
            start_frame: target.start,
            source_offset_frame: 0,
            length_frames: audio.map_or_else(
                || frames(120.0 / f64::from(self.zoom)),
                |audio| audio.samples.len() as u64,
            ),
            repeat: None,
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
        let track_color = target
            .track
            .and_then(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
            })
            .map_or_else(
                || daw_core::default_track_color(self.session.project.tracks.len()),
                |track| track.color,
            );
        let colors = theme::clip_colors(track_color, clip.color);
        if let Some(audio) = audio {
            self.paint_clip(&ghost, track_color, block, &clip, false, Some(audio));
        } else {
            ghost.rect_filled(block, 2.0, colors.body);
            let header =
                Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
            ghost.rect_filled(header, 2.0, colors.header);
            let text = ghost.with_clip_rect(block);
            text.text(
                Pos2::new(header.left() + 7.0, header.center().y),
                egui::Align2::LEFT_CENTER,
                &clip.name,
                egui::FontId::proportional(11.0),
                colors.text,
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
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let Some(preview) =
            self.clip_drag_preview(drag, pointer, ui.input(|input| input.modifiers.shift))
        else {
            return;
        };
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.id == preview.track)
        else {
            return;
        };
        let lane = self.lane_bounds[&preview.track];
        let block = self.clip_block(lane, &preview.clip, preview.clip.start_frame);
        let painter = ui.painter_at(viewport.intersect(lane));
        let mut ghost = painter.clone();
        ghost.multiply_opacity(0.75);
        self.paint_clip(&ghost, track.color, block, &preview.clip, false, None);
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
        ui.ctx()
            .set_cursor_icon(if drag.mode == ClipDragMode::Move {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
    }
    fn paint_clip(
        &self,
        painter: &egui::Painter,
        track_color: daw_core::RgbColor,
        block: Rect,
        clip: &Clip,
        selected: bool,
        prepared: Option<&daw_media::AudioData>,
    ) {
        let audio = prepared.or_else(|| self.session.audio.get(&clip.asset_id));
        let missing = audio.is_none();
        let lane = painter.clip_rect();
        let colors = if missing {
            theme::ClipColors::default()
        } else {
            theme::clip_colors(track_color, clip.color)
        };
        painter.rect_filled(
            block,
            2.0,
            if missing { theme::MISSING } else { colors.body },
        );
        let header = Rect::from_min_max(block.min, Pos2::new(block.right(), block.top() + 22.0));
        painter.rect_filled(
            header,
            2.0,
            if missing {
                theme::BORDER
            } else {
                colors.header
            },
        );
        let clip_painter = painter.with_clip_rect(block.intersect(lane));
        clip_painter.text(
            Pos2::new(header.left() + 7.0, header.center().y),
            egui::Align2::LEFT_CENTER,
            &clip.name,
            egui::FontId::proportional(11.0),
            colors.text,
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
                        colors.channel,
                    );
                }
                if channel == 1 {
                    clip_painter.line_segment(
                        [Pos2::new(block.left(), top), Pos2::new(block.right(), top)],
                        Stroke::new(1.0_f32, colors.header),
                    );
                }
                let left = body.left().max(lane.left());
                let right = body.right().min(lane.right());
                for pixel in (left as i32)..=(right as i32) {
                    let local = ((pixel as f32 - body.left()) / self.zoom
                        * f64::from(daw_core::SAMPLE_RATE) as f32)
                        .max(0.0) as u64;
                    let samples_per_pixel = f64::from(daw_core::SAMPLE_RATE) / f64::from(self.zoom);
                    let [min, max] = waveforms::extrema(
                        audio,
                        clip,
                        local,
                        samples_per_pixel.ceil() as u64 + 1,
                        channel,
                    );
                    clip_painter.line_segment(
                        [
                            Pos2::new(pixel as f32, center - max.clamp(-1.0, 1.0) * height * 0.42),
                            Pos2::new(pixel as f32, center - min.clamp(-1.0, 1.0) * height * 0.42),
                        ],
                        Stroke::new(1.0_f32, colors.waveform),
                    );
                }
            }
        }
        if let Some(repeat) = clip.repeat {
            let step = seconds(repeat.length_frames) as f32 * self.zoom;
            if step >= 4.0 {
                let first = repeat.length_frames - repeat.phase_frame;
                let visible = frames(f64::from((lane.left() - block.left()).max(0.0) / self.zoom));
                let skipped = visible.saturating_sub(first) / repeat.length_frames;
                let mut local = first.saturating_add(skipped.saturating_mul(repeat.length_frames));
                while local < clip.length_frames {
                    let x = block.left() + seconds(local) as f32 * self.zoom;
                    if x > lane.right() {
                        break;
                    }
                    if x >= lane.left() {
                        clip_painter.line_segment(
                            [Pos2::new(x, block.top()), Pos2::new(x, block.bottom())],
                            Stroke::new(1.0, colors.text.gamma_multiply(0.25)),
                        );
                    }
                    let Some(next) = local.checked_add(repeat.length_frames) else {
                        break;
                    };
                    local = next;
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
            && drag.mode != ClipDragMode::Body
        {
            let pointer = ui
                .input(|input| input.pointer.latest_pos())
                .unwrap_or(drag.origin);
            if let Some(preview) =
                self.clip_drag_preview(&drag, pointer, ui.input(|input| input.modifiers.shift))
            {
                let c = preview.clip;
                if preview.track == drag.track
                    && (
                        c.start_frame,
                        c.source_offset_frame,
                        c.length_frames,
                        c.repeat,
                    ) == (
                        drag.clip.start_frame,
                        drag.clip.source_offset_frame,
                        drag.clip.length_frames,
                        drag.clip.repeat,
                    )
                {
                    return;
                }
                self.edit(Edit::Place {
                    clip_id: c.id,
                    track_id: preview.track,
                    start: c.start_frame,
                    offset: c.source_offset_frame,
                    length: c.length_frames,
                    repeat: c.repeat,
                });
            }
        }
    }
    fn lane(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        let track_id = track.id;
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
        for clip in &track.clips {
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
                .is_some_and(|drag| drag.mode != ClipDragMode::Body && drag.clip.id == clip.id)
            {
                clip_painter.multiply_opacity(0.35);
            }
            self.paint_clip(&clip_painter, track.color, block, clip, selected, None);
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
                match ClipDragMode::at(block, pos) {
                    ClipDragMode::LoopLeft | ClipDragMode::LoopRight => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        response
                            .clone()
                            .on_hover_text("Drag to loop clip. Hold Shift to bypass snapping.");
                    }
                    ClipDragMode::TrimLeft | ClipDragMode::TrimRight => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        response
                            .clone()
                            .on_hover_text("Drag to trim clip. Hold Shift to bypass snapping.");
                    }
                    ClipDragMode::Move => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        response
                            .clone()
                            .on_hover_text("Drag to move clip. Hold Shift to bypass snapping.");
                    }
                    ClipDragMode::Body => {}
                }
            }
            if response.drag_started()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                let mode = ClipDragMode::at(block, origin);
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
            let at = self.selection_frame(
                i128::from(self.ruler_frame(rect, pos)),
                0..=u64::MAX,
                ui.input(|input| input.modifiers.shift),
            );
            self.seek(at);
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
            color: None,
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 480000,
            repeat: None,
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
    fn shift_frame(
        app: &mut DawUi,
        ctx: &egui::Context,
        mut events: Vec<egui::Event>,
        shift: bool,
    ) -> Vec<egui::epaint::ClippedShape> {
        let modifiers = egui::Modifiers {
            shift,
            ..Default::default()
        };
        for event in &mut events {
            if let egui::Event::PointerButton {
                modifiers: value, ..
            } = event
            {
                *value = modifiers;
            }
        }
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        frame_shapes(app, ctx, events, Vec2::new(1280.0, 800.0))
    }

    #[test]
    fn clip_edges_snap_preview_and_release_with_dynamic_shift_override() {
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 0.5)] {
            for (mode, target, snapped) in [
                (ClipDragMode::TrimLeft, 2.27, 2.25),
                (ClipDragMode::TrimRight, 4.251, 4.25),
                (ClipDragMode::LoopLeft, 0.8751, 0.87),
                (ClipDragMode::LoopRight, 4.251, 4.26),
            ] {
                for free_release in [false, true] {
                    let (mut app, track) = fixture();
                    app.zoom = zoom;
                    app.scroll = scroll;
                    let clip = &mut app.session.project.tracks[0].clips[0];
                    clip.start_frame = frames(2.0);
                    clip.source_offset_frame = frames(0.5);
                    clip.length_frames = frames(1.13);
                    let original = clip.clone();
                    let left = matches!(mode, ClipDragMode::TrimLeft | ClipDragMode::LoopLeft);
                    let looping = matches!(mode, ClipDragMode::LoopLeft | ClipDragMode::LoopRight);
                    let ctx = context();
                    frame(&mut app, &ctx, vec![]);
                    let lane = app.lane_bounds[&track];
                    let block = app.clip_block(lane, &original, original.start_frame);
                    let start = Pos2::new(
                        if left {
                            block.left() + 1.0
                        } else {
                            block.right() - 1.0
                        },
                        block.top() + if looping { 12.0 } else { 40.0 },
                    );
                    let original_edge = if left {
                        original.start_frame
                    } else {
                        original.end()
                    };
                    let end =
                        start + Vec2::new((target - seconds(original_edge)) as f32 * zoom, 0.0);
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(start), button(start, true)],
                    );
                    shift_frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(end)],
                        !free_release,
                    );
                    assert_eq!(app.drag.as_ref().unwrap().mode, mode);
                    let before = app
                        .clip_drag_preview(app.drag.as_ref().unwrap(), end, !free_release)
                        .unwrap();
                    // Change Shift at the same pointer position, then release with that state.
                    let shapes = shift_frame(&mut app, &ctx, vec![], free_release);
                    let preview = app
                        .clip_drag_preview(app.drag.as_ref().unwrap(), end, free_release)
                        .unwrap();
                    let edge = |clip: &Clip| if left { clip.start_frame } else { clip.end() };
                    assert_ne!(edge(&before.clip), edge(&preview.clip));
                    let expected = frames(if free_release { target } else { snapped });
                    assert!(
                        edge(&preview.clip).abs_diff(expected) <= 1,
                        "{mode:?}: {} != {expected}",
                        edge(&preview.clip)
                    );
                    assert!(preview.valid);
                    assert!(has_preview_outline(
                        &shapes,
                        app.clip_block(lane, &preview.clip, preview.clip.start_frame),
                        theme::ACCENT
                    ));
                    assert_eq!(
                        app.session.project.tracks[0].clips[0].length_frames,
                        original.length_frames
                    );
                    assert!(!app.dirty);
                    shift_frame(&mut app, &ctx, vec![button(end, false)], free_release);
                    let placed = &app.session.project.tracks[0].clips[0];
                    assert_eq!(
                        (
                            placed.start_frame,
                            placed.length_frames,
                            placed.source_offset_frame,
                            placed.repeat
                        ),
                        (
                            preview.clip.start_frame,
                            preview.clip.length_frames,
                            preview.clip.source_offset_frame,
                            preview.clip.repeat
                        )
                    );
                    if looping {
                        assert_eq!(placed.repeat.unwrap().length_frames, original.length_frames);
                    }
                    assert!(app.dirty);
                    assert!(app.error.is_none());
                    app.session.project.validate().unwrap();
                }
            }
        }
    }

    #[test]
    fn loop_selection_snaps_creation_edges_and_movement_with_shift_override() {
        for (press, target, snapped, raw) in [
            (1.03, 3.27, (1.0, 3.25), (1.03, 3.27)),
            (2.0, 2.27, (2.25, 4.0), (2.27, 4.0)),
            (4.0, 4.27, (2.0, 4.25), (2.0, 4.27)),
            (3.0, 3.27, (2.25, 4.25), (2.27, 4.27)),
        ] {
            for free_release in [false, true] {
                let (mut app, track) = fixture();
                let region = &mut app.session.project.transport.r#loop;
                region.start_frame = frames(2.0);
                region.end_frame = frames(4.0);
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let point =
                    |time: f64| Pos2::new(lane.left() + time as f32 * 70.0, lane.top() - 29.0);
                let start = point(press);
                let end = point(target);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(end)],
                    !free_release,
                );
                shift_frame(&mut app, &ctx, vec![], free_release);
                let region = &app.session.project.transport.r#loop;
                let live = (region.start_frame, region.end_frame);
                let expected = if free_release { raw } else { snapped };
                assert!(live.0.abs_diff(frames(expected.0)) <= 1);
                assert!(live.1.abs_diff(frames(expected.1)) <= 1);
                assert!(!app.dirty);
                shift_frame(&mut app, &ctx, vec![button(end, false)], free_release);
                let region = &app.session.project.transport.r#loop;
                assert_eq!((region.start_frame, region.end_frame), live);
                assert!(!region.enabled);
                assert!(app.dirty);
                assert!(app.error.is_none());
                app.session.project.validate().unwrap();
            }
        }
    }

    #[test]
    fn source_bounds_and_clip_size_are_fallbacks_to_grid_snapping() {
        let (mut app, track) = fixture();
        app.session.project.tracks[0].clips[0].length_frames = frames(3.15);
        assert_eq!(
            app.selection_frame(i128::from(frames(3.16)), 0..=u64::MAX, false),
            frames(3.15)
        );
        app.session.project.tracks[0].clips[0].length_frames = frames(3.26);
        assert_eq!(
            app.selection_frame(i128::from(frames(3.26)), 0..=u64::MAX, false),
            frames(3.25)
        );
        let clip = &mut app.session.project.tracks[0].clips[0];
        clip.start_frame = frames(2.0);
        clip.source_offset_frame = frames(0.5);
        clip.length_frames = frames(1.13);
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let drag = Drag {
            clip: app.session.project.tracks[0].clips[0].clone(),
            track,
            mode: ClipDragMode::LoopRight,
            origin: Pos2::ZERO,
        };
        let preview = app
            .clip_drag_preview(&drag, Pos2::new((4.251 - 3.13) * 70.0, 0.0), false)
            .unwrap();
        let repeated = Drag {
            clip: preview.clip,
            ..drag
        };
        let returned = app
            .clip_drag_preview(&repeated, Pos2::new((3.15 - 4.26) * 70.0, 0.0), false)
            .unwrap();
        assert_eq!(returned.clip.length_frames, frames(1.13));
        assert_eq!(returned.clip.repeat, None);
        let trim = Drag {
            mode: ClipDragMode::TrimRight,
            clip: returned.clip,
            ..repeated
        };
        let restored = app
            .clip_drag_preview(&trim, Pos2::new(20.0 * 70.0, 0.0), false)
            .unwrap();
        assert_eq!(restored.clip.length_frames, frames(9.5));
        assert_eq!(restored.clip.repeat, None);
        assert_eq!(app.snap_frame(-1, 0..=u64::MAX, [1], false), 0);
        assert_eq!(app.snap_frame(100, 0..=99, [98], false), 99);
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
    fn trackpad_scroll(pointer: Pos2, delta: Vec2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::ZERO,
                modifiers: Default::default(),
                phase: egui::TouchPhase::Start,
            },
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                modifiers: Default::default(),
                phase: egui::TouchPhase::Move,
            },
        ]
    }
    #[test]
    fn horizontal_trackpad_swipes_scroll_the_timeline_and_clamp_without_editing() {
        for zoom in [70.0, 140.0] {
            for y in [-29.0, 40.0, 90.0] {
                let (mut app, track) = fixture();
                app.zoom = zoom;
                app.session.project.tracks[0].clips[0].start_frame = frames(2.0);
                let original = app.session.project.tracks[0].clips[0].clone();
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let pointer = lane.min + Vec2::new(80.0, y);
                let maximum = app.timeline_extent()
                    - f64::from(app.scrollbar_bounds.width()) / f64::from(zoom);
                for (delta, expected) in [
                    (-0.5 * zoom, 0.5),
                    (0.25 * zoom, 0.25),
                    (zoom, 0.0),
                    (-90_000.0, maximum),
                    (90_000.0, 0.0),
                ] {
                    let shapes = frame_shapes(
                        &mut app,
                        &ctx,
                        trackpad_scroll(pointer, Vec2::new(delta, 0.0)),
                        Vec2::new(1280.0, 800.0),
                    );
                    assert!(
                        (app.scroll - expected).abs() < 0.00001,
                        "{} != {expected}",
                        app.scroll
                    );
                    assert_eq!(app.lane_bounds[&track], lane);
                    if expected <= 0.5 {
                        let block = app.clip_block(lane, &original, original.start_frame);
                        assert!(
                            shapes
                                .iter()
                                .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                            if rect.rect == block && rect.fill == theme::MISSING))
                        );
                    }
                    // The scrollbar retains the swipe position on the next frame.
                    frame(&mut app, &ctx, vec![]);
                    assert!((app.scroll - expected).abs() < 0.00001);
                    assert!(!app.dirty);
                    assert!(app.error.is_none());
                }
                let clip = &app.session.project.tracks[0].clips[0];
                assert_eq!(
                    (
                        clip.start_frame,
                        clip.source_offset_frame,
                        clip.length_frames,
                        clip.repeat
                    ),
                    (
                        original.start_frame,
                        original.source_offset_frame,
                        original.length_frames,
                        original.repeat
                    )
                );
                assert_eq!(app.session.project.transport.playhead_frame, 0);
                assert_eq!(app.session.project.transport.r#loop.start_frame, 0);
                assert_eq!(app.session.project.transport.r#loop.end_frame, 0);
            }
        }
        // When the full extent fits, swiping must keep the offset at zero.
        let mut app = DawUi {
            zoom: 1.0,
            ..Default::default()
        };
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let pointer = app.scrollbar_bounds.center() - Vec2::new(0.0, 100.0);
        frame(
            &mut app,
            &ctx,
            trackpad_scroll(pointer, Vec2::new(-200.0, 0.0)),
        );
        assert_eq!(app.scroll, 0.0);
    }
    #[test]
    fn timeline_swipes_preserve_vertical_scrolling_and_respect_hover_and_disabled_ui() {
        let (mut app, track) = fixture();
        for _ in 0..7 {
            app.session.project.add_track().unwrap();
        }
        app.scroll = 1.0;
        let ctx = context();
        let size = Vec2::new(900.0, 450.0);
        frame_sized(&mut app, &ctx, vec![], size);
        let lane = app.lane_bounds[&track];
        let master = app.master_bounds;
        let pointer = lane.min + Vec2::new(80.0, 40.0);
        // A diagonal swipe keeps its vertical component for the track scroll area.
        frame_sized(
            &mut app,
            &ctx,
            trackpad_scroll(pointer, Vec2::new(-35.0, -80.0)),
            size,
        );
        frame_sized(&mut app, &ctx, vec![], size);
        assert!((app.scroll - 1.5).abs() < 0.00001);
        assert!(app.lane_bounds[&track].top() < lane.top());
        assert_eq!(app.master_bounds, master);
        for pointer in [
            Pos2::new(30.0, pointer.y),
            master.center(),
            Pos2::new(pointer.x, 50.0),
            Pos2::new(pointer.x, app.scrollbar_bounds.bottom() + 10.0),
        ] {
            frame_sized(
                &mut app,
                &ctx,
                trackpad_scroll(pointer, Vec2::new(-70.0, 0.0)),
                size,
            );
            assert!((app.scroll - 1.5).abs() < 0.00001);
        }
        let (_sender, receiver) = mpsc::channel();
        app.job = Some(receiver);
        frame_sized(
            &mut app,
            &ctx,
            trackpad_scroll(pointer, Vec2::new(-70.0, 0.0)),
            size,
        );
        assert!((app.scroll - 1.5).abs() < 0.00001);
        assert_eq!(app.master_bounds, master);
        assert!(!app.dirty);
        assert!(app.error.is_none());
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
        let label = Pos2::new(lane.left() + bar_two + 3.0, lane.top() - 10.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(label), button(label, true)],
        );
        frame(&mut app, &ctx, vec![button(label, false)]);
        assert_eq!(app.session.project.transport.playhead_frame, frames(4.0));
        assert_eq!(
            musical_time::monitor(app.session.project.transport.playhead_frame, 60.0),
            "0002.01"
        );
        let start = Pos2::new(lane.left() + bar_two, lane.top() - 29.0);
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
        let start = Pos2::new(lane.left() + 70.0, lane.top() - 29.0);
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
        app.dirty = false;
        let start = Pos2::new(lane.left() + 70.0, lane.top() - 29.0);
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
        assert!(app.error.is_none());
        assert!(!app.dirty);
    }

    #[test]
    fn ruler_upper_selection_resizes_moves_and_paints_only_in_the_label_band() {
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            let (mut app, track) = fixture();
            app.zoom = zoom;
            app.scroll = scroll;
            app.session.project.transport.playhead_frame = frames(8.0);
            let region = &mut app.session.project.transport.r#loop;
            region.start_frame = frames(2.0);
            region.end_frame = frames(5.0);
            region.enabled = true;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let point = |seconds: f64| {
                Pos2::new(
                    lane.left() + (seconds - scroll) as f32 * zoom,
                    lane.top() - 29.0,
                )
            };
            for (start, end, expected) in [
                (point(2.0), point(3.0), (3.0, 5.0)),
                (point(5.0), point(6.0), (3.0, 6.0)),
                (point(4.0), point(5.0), (4.0, 7.0)),
                (point(5.0), point(-10.0), (0.0, 3.0)),
            ] {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                let region = &app.session.project.transport.r#loop;
                assert_eq!(
                    (region.start_frame, region.end_frame),
                    (frames(expected.0), frames(expected.1))
                );
                frame(&mut app, &ctx, vec![button(end, false)]);
                assert!(app.loop_drag.is_none());
                assert!(app.dirty);
                app.session.project.validate().unwrap();
            }
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let band = Rect::from_min_max(
                Pos2::new(lane.left(), lane.top() - RULER_HEIGHT),
                Pos2::new(point(3.0).x, lane.top() - 20.0),
            );
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == band && rect.fill == theme::RULER_SELECTION)));
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == theme::RULER_SELECTION && rect.rect.bottom() > lane.top() - 20.0)));
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == theme::ACCENT && rect.rect.width() == 3.0 && rect.rect.height() == 14.0)));
            assert_eq!(app.session.project.transport.playhead_frame, frames(8.0));
            assert!(app.session.project.transport.r#loop.enabled);
            assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
            assert_eq!(app.session.project.tracks[0].clips[0].length_frames, 480000);
            assert!(app.error.is_none());
        }
    }
    #[test]
    fn ruler_resizing_clamps_and_interrupted_selection_drags_restore_the_range() {
        for left in [true, false] {
            let (mut app, track) = fixture();
            let region = &mut app.session.project.transport.r#loop;
            region.start_frame = frames(1.0);
            region.end_frame = frames(3.0);
            region.enabled = true;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let start = Pos2::new(
                lane.left() + if left { 70.0 } else { 210.0 },
                lane.top() - 29.0,
            );
            let end = start + Vec2::new(if left { 500.0 } else { -500.0 }, 25.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            let region = &app.session.project.transport.r#loop;
            assert_eq!(region.end_frame - region.start_frame, 1);
            app.session.project.validate().unwrap();
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.error.is_none());
        }
        for focus_loss in [false, true] {
            let (mut app, track) = fixture();
            let region = &mut app.session.project.transport.r#loop;
            region.start_frame = frames(1.0);
            region.end_frame = frames(3.0);
            region.enabled = true;
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let start = Pos2::new(lane.left() + 280.0, lane.top() - 29.0);
            let end = start + Vec2::new(140.0, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert_eq!(
                app.session.project.transport.r#loop.start_frame,
                frames(4.0)
            );
            assert!(!app.dirty);
            let (sender, receiver) = mpsc::channel();
            if focus_loss {
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
                app.job = Some(receiver);
                frame(&mut app, &ctx, vec![]);
            }
            let region = &app.session.project.transport.r#loop;
            assert_eq!(
                (region.start_frame, region.end_frame),
                (frames(1.0), frames(3.0))
            );
            assert!(region.enabled);
            assert!(app.loop_drag.is_none());
            assert!(!app.dirty);
            assert!(app.error.is_none());
            drop(sender);
        }
    }
    #[test]
    fn ruler_playhead_marker_drags_live_and_clamps_at_the_track_start() {
        let (mut app, track) = fixture();
        app.scroll = 1.0;
        app.session.project.transport.playhead_frame = frames(2.0);
        app.session.project.transport.r#loop.start_frame = frames(1.0);
        app.session.project.transport.r#loop.end_frame = frames(3.0);
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let marker = Pos2::new(lane.left() + 70.0, lane.top() - 10.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(marker), button(marker, true)],
        );
        let left = marker - Vec2::new(500.0, 0.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(left)]);
        assert_eq!(app.session.project.transport.playhead_frame, 0);
        let right = marker + Vec2::new(140.0, -25.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(right)]);
        assert_eq!(app.session.project.transport.playhead_frame, frames(4.0));
        frame(&mut app, &ctx, vec![button(right, false)]);
        let region = &app.session.project.transport.r#loop;
        assert_eq!(
            (region.start_frame, region.end_frame),
            (frames(1.0), frames(3.0))
        );
        assert!(app.loop_drag.is_none());
        assert!(app.error.is_none());
    }
    #[test]
    fn ruler_lower_band_seeks_without_editing_selection() {
        let (mut app, track) = fixture();
        app.session.project.transport.r#loop.start_frame = 48000;
        app.session.project.transport.r#loop.end_frame = 144000;
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let lane = app.lane_bounds[&track];
        let start = Pos2::new(lane.left() + 280.0, lane.top() - 10.0);
        // A playhead drag must retain its role when it enters the upper band.
        let end = start + Vec2::new(140.0, -25.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let region = &app.session.project.transport.r#loop;
        assert_eq!((region.start_frame, region.end_frame), (48000, 144000));
        assert_eq!(app.session.project.transport.playhead_frame, 288000);
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
            if stroke.color == theme::clip_colors(app.session.project.tracks[1].color, None).waveform.gamma_multiply(0.75))
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
        for index in 0..2 {
            let point = Pos2::new(
                TRACK_WIDTH + 140.0,
                if index == 0 {
                    140.0
                } else {
                    app.lane_bounds[&app.session.project.tracks[index - 1].id].bottom() + 20.0
                },
            );
            let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
            let target = app.file_drop_target.as_ref().unwrap();
            assert_eq!(target.track, None);
            assert_eq!(target.start, frames(2.0));
            let block = Rect::from_min_size(
                Pos2::new(point.x, target.lane.top()),
                Vec2::new(70.0, ROW_HEIGHT),
            );
            let colors = theme::clip_colors(daw_core::default_track_color(index), None);
            assert!(has_preview_outline(&shapes, block, theme::ACCENT));
            assert!(
                shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.rect == block && rect.fill == colors.body.gamma_multiply(0.75)))
            );
            assert_eq!(app.session.project.tracks.len(), index);
            file_frame(&mut app, &ctx, &source.0, point, true);
            finish_import(&mut app, &ctx);
            assert!(app.error.is_none());
            assert_eq!(app.session.project.tracks.len(), index + 1);
            assert_eq!(
                app.session.project.tracks[index].color,
                daw_core::default_track_color(index)
            );
            assert_eq!(app.session.project.tracks[index].clips[0].color, None);
            assert_eq!(
                app.session.project.tracks[index].clips[0].start_frame,
                frames(2.0)
            );
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            assert!(
                shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.rect == block && rect.fill == colors.body))
            );
        }
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
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            assert_eq!(preview.track, second);
            assert_eq!(preview.clip.start_frame, frames(3.25));
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
            assert_eq!(placed.start_frame, preview.clip.start_frame);
            assert_eq!(placed.source_offset_frame, original.source_offset_frame);
            assert_eq!(placed.length_frames, original.length_frames);
            assert!(app.error.is_none());
        }
    }

    #[test]
    fn clip_moves_snap_either_edge_and_match_release_with_dynamic_shift() {
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            for across_tracks in [false, true] {
                for (target, length, expected) in
                    [(3.02, 1.13, 3.0), (3.13, 0.89, 3.11), (3.26, 0.72, 3.28)]
                {
                    for free_release in [false, true] {
                        let (mut app, track) = fixture();
                        let destination = if across_tracks {
                            app.session.project.add_track().unwrap()
                        } else {
                            track
                        };
                        app.zoom = zoom;
                        app.scroll = scroll;
                        let clip = &mut app.session.project.tracks[0].clips[0];
                        clip.start_frame = frames(2.0);
                        clip.source_offset_frame = frames(0.5);
                        clip.length_frames = frames(length);
                        if length == 0.72 {
                            clip.repeat = Some(daw_core::ClipLoop {
                                length_frames: frames(0.36),
                                phase_frame: frames(0.07),
                            });
                        }
                        let original = clip.clone();
                        let ctx = context();
                        frame(&mut app, &ctx, vec![]);
                        let lane = app.lane_bounds[&track];
                        let start = app.clip_block(lane, &original, original.start_frame).min
                            + Vec2::new(20.0, 12.0);
                        let end = Pos2::new(
                            start.x + (target - 2.0) as f32 * zoom,
                            app.lane_bounds[&destination].top() + 12.0,
                        );
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(start), button(start, true)],
                        );
                        shift_frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end)],
                            !free_release,
                        );
                        let before = app
                            .clip_drag_preview(app.drag.as_ref().unwrap(), end, !free_release)
                            .unwrap();
                        let shapes = shift_frame(&mut app, &ctx, vec![], free_release);
                        let preview = app
                            .clip_drag_preview(app.drag.as_ref().unwrap(), end, free_release)
                            .unwrap();
                        assert_ne!(before.clip.start_frame, preview.clip.start_frame);
                        assert!(
                            preview.clip.start_frame.abs_diff(frames(if free_release {
                                target
                            } else {
                                expected
                            })) <= 1
                        );
                        assert_eq!(preview.track, destination);
                        assert!(preview.valid);
                        let block = app.clip_block(
                            app.lane_bounds[&destination],
                            &preview.clip,
                            preview.clip.start_frame,
                        );
                        assert!(has_preview_outline(&shapes, block, theme::ACCENT));
                        assert_eq!(
                            app.session.project.tracks[0].clips[0].start_frame,
                            original.start_frame
                        );
                        assert!(!app.dirty);
                        shift_frame(&mut app, &ctx, vec![button(end, false)], free_release);
                        let placed = app
                            .session
                            .project
                            .tracks
                            .iter()
                            .find(|track| track.id == destination)
                            .unwrap()
                            .clips
                            .iter()
                            .find(|clip| clip.id == original.id)
                            .unwrap();
                        assert_eq!(placed.start_frame, preview.clip.start_frame);
                        assert_eq!(
                            (
                                placed.length_frames,
                                placed.source_offset_frame,
                                placed.repeat
                            ),
                            (
                                original.length_frames,
                                original.source_offset_frame,
                                original.repeat
                            )
                        );
                        assert!(app.dirty);
                        assert!(app.error.is_none());
                        app.session.project.validate().unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn playhead_clicks_and_drags_snap_to_grid_with_dynamic_shift() {
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            for free_release in [false, true] {
                let (mut app, track) = fixture();
                app.zoom = zoom;
                app.scroll = scroll;
                app.session.project.tracks[0].clips[0].length_frames = frames(3.26);
                app.session.project.transport.r#loop.start_frame = frames(1.0);
                app.session.project.transport.r#loop.end_frame = frames(3.0);
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let point = |time: f64| {
                    Pos2::new(
                        lane.left() + (time - scroll) as f32 * zoom,
                        lane.top() - 10.0,
                    )
                };
                let click = point(2.27);
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(click), button(click, true)],
                    free_release,
                );
                shift_frame(&mut app, &ctx, vec![button(click, false)], free_release);
                let position = if free_release { 2.27 } else { 2.25 };
                assert!(
                    app.session
                        .project
                        .transport
                        .playhead_frame
                        .abs_diff(frames(position))
                        <= 1
                );
                let marker = point(position);
                let end = point(3.27);
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(marker), button(marker, true)],
                    !free_release,
                );
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(end)],
                    !free_release,
                );
                let before = app.session.project.transport.playhead_frame;
                shift_frame(&mut app, &ctx, vec![], free_release);
                let live = app.session.project.transport.playhead_frame;
                assert_ne!(live, before);
                assert!(live.abs_diff(frames(if free_release { 3.27 } else { 3.25 })) <= 1);
                shift_frame(&mut app, &ctx, vec![button(end, false)], free_release);
                assert_eq!(app.session.project.transport.playhead_frame, live);
                let background = point(4.27) + Vec2::new(0.0, 50.0);
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(background),
                        button(background, true),
                    ],
                    free_release,
                );
                shift_frame(
                    &mut app,
                    &ctx,
                    vec![button(background, false)],
                    free_release,
                );
                assert!(
                    app.session
                        .project
                        .transport
                        .playhead_frame
                        .abs_diff(frames(if free_release { 4.27 } else { 4.25 }))
                        <= 1
                );
                let selection = &app.session.project.transport.r#loop;
                assert_eq!(
                    (selection.start_frame, selection.end_frame),
                    (frames(1.0), frames(3.0))
                );
                assert!(!selection.enabled);
                assert!(app.loop_drag.is_none());
                assert_eq!(app.session.project.tracks[0].clips[0].start_frame, 0);
                assert!(app.error.is_none());
            }
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
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            assert_eq!(preview.track, target);
            assert_eq!(preview.clip.start_frame, 0);
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
        let preview = app
            .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
            .unwrap();
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
    fn saved_colors_render_on_clips_and_drag_previews_with_neutral_track_panels() {
        let source = TestWav::new();
        let mut audio = daw_media::decode_wav(&source.0).unwrap();
        audio.metadata.channels = 2;
        for custom in [
            None,
            Some(daw_core::RgbColor {
                r: 160,
                g: 70,
                b: 35,
            }),
            Some(daw_core::RgbColor {
                r: 235,
                g: 225,
                b: 190,
            }),
        ] {
            let (mut app, track) = fixture();
            let track_color = daw_core::RgbColor {
                r: 45,
                g: 31,
                b: 55,
            };
            app.edit(Edit::SetTrackColor {
                track_id: track,
                color: track_color,
            });
            let clip_id = app.session.project.tracks[0].clips[0].id;
            app.edit(Edit::SetClipColor {
                clip_id,
                color: custom,
            });
            assert!(app.dirty);
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.length_frames = 48000;
            let clip = clip.clone();
            app.session.audio.insert(clip.asset_id, audio.clone());
            let colors = theme::clip_colors(track_color, custom);
            let ctx = context();
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let has_fill = |shapes: &[egui::epaint::ClippedShape], color| {
                shapes.iter().any(
                    |shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == color),
                )
            };
            let lane = app.lane_bounds[&track];
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == theme::PANEL && rect.rect.top() == lane.top() && rect.rect.right() <= lane.left())));
            assert!(
                !shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == colors.body && rect.rect.right() <= lane.left()))
            );
            for color in [colors.body, colors.header, colors.channel] {
                assert!(has_fill(&shapes, color));
            }
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment {stroke, ..} if stroke.color == colors.waveform)));
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Test" && text.fallback_color == colors.text)));
            let lane = app.lane_bounds[&track];
            let start = lane.min + Vec2::new(20.0, 12.0);
            let end = start + Vec2::new(140.0, 0.0);
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
            assert!(has_fill(&shapes, colors.body.gamma_multiply(0.75)));
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment {stroke, ..} if stroke.color == colors.waveform.gamma_multiply(0.75))));
            assert!(has_preview_outline(
                &shapes,
                app.clip_block(lane, &clip, frames(2.0)),
                theme::ACCENT
            ));
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert_eq!(app.session.project.tracks[0].clips[0].color, custom);
            app.edit(Edit::SetTrackColor {
                track_id: track,
                color: daw_core::LEGACY_CLIP_COLOR,
            });
            app.edit(Edit::SetClipColor {
                clip_id,
                color: None,
            });
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            for color in [theme::CLIP, theme::CLIP_HEADER, theme::CLIP_CHANNEL] {
                assert!(has_fill(&shapes, color));
            }
            app.session.audio.clear();
            app.edit(Edit::SetClipColor {
                clip_id,
                color: custom,
            });
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            assert!(has_fill(&shapes, theme::MISSING));
        }
    }

    #[test]
    fn clip_color_inheritance_follows_track_changes_moves_and_file_drops() {
        let source = TestWav::new();
        let audio = daw_media::decode_wav(&source.0).unwrap();
        let first_color = daw_core::RgbColor {
            r: 120,
            g: 55,
            b: 80,
        };
        let second_color = daw_core::RgbColor {
            r: 40,
            g: 105,
            b: 62,
        };
        let changed_color = daw_core::RgbColor {
            r: 85,
            g: 80,
            b: 145,
        };
        for override_color in [
            None,
            Some(daw_core::RgbColor {
                r: 160,
                g: 75,
                b: 30,
            }),
        ] {
            let (mut app, first) = fixture();
            let second = app.session.project.add_track().unwrap();
            app.session.project.tracks[0].color = first_color;
            app.session.project.tracks[1].color = second_color;
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.color = override_color;
            clip.length_frames = 48000;
            let clip = clip.clone();
            app.session.audio.insert(clip.asset_id, audio.clone());
            let ctx = context();
            let size = Vec2::new(1280.0, 800.0);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let has_clip_fill = |shapes: &[egui::epaint::ClippedShape], block, color| {
                shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect)
                    if rect.rect == block && rect.fill == color)
                })
            };
            let first_lane = app.lane_bounds[&first];
            let second_lane = app.lane_bounds[&second];
            let start = first_lane.min + Vec2::new(20.0, 12.0);
            let end = Pos2::new(start.x + 140.0, second_lane.top() + 12.0);
            assert!(has_clip_fill(
                &shapes,
                app.clip_block(first_lane, &clip, 0),
                theme::clip_colors(first_color, override_color).body
            ));
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            let shapes = frame_shapes(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], size);
            let block = app.clip_block(second_lane, &clip, frames(2.0));
            assert!(has_clip_fill(
                &shapes,
                block,
                theme::clip_colors(second_color, override_color)
                    .body
                    .gamma_multiply(0.75)
            ));
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(app.session.project.tracks[0].clips.is_empty());
            assert_eq!(app.session.project.tracks[1].clips[0].color, override_color);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            assert!(has_clip_fill(
                &shapes,
                block,
                theme::clip_colors(second_color, override_color).body
            ));
            app.edit(Edit::SetTrackColor {
                track_id: second,
                color: changed_color,
            });
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            assert!(has_clip_fill(
                &shapes,
                block,
                theme::clip_colors(changed_color, override_color).body
            ));
            assert_eq!(app.session.project.tracks[1].clips[0].color, override_color);

            let point = Pos2::new(second_lane.left() - 20.0, second_lane.center().y);
            let new_block = app.clip_block(second_lane, &clip, 0);
            let inherited = theme::clip_colors(changed_color, None);
            let loading = file_frame(&mut app, &ctx, &source.0, point, false);
            assert!(loading.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.rect.left_top() == second_lane.left_top() && rect.fill == inherited.body.gamma_multiply(0.75))));
            let shapes = ready_file_hover(&mut app, &ctx, &source.0, point);
            assert!(has_clip_fill(
                &shapes,
                new_block,
                inherited.body.gamma_multiply(0.75)
            ));
            file_frame(&mut app, &ctx, &source.0, point, true);
            finish_import(&mut app, &ctx);
            assert!(app.error.is_none());
            let imported = app.session.project.tracks[1]
                .clips
                .iter()
                .find(|c| c.id != clip.id)
                .unwrap();
            assert_eq!(imported.color, None);
            assert_eq!(imported.start_frame, 0);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            assert!(has_clip_fill(&shapes, new_block, inherited.body));
        }
    }

    #[test]
    fn header_edge_drags_preview_and_commit_repeats_of_the_trimmed_clip() {
        let source = TestWav::new();
        let audio = daw_media::decode_wav(&source.0).unwrap();
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            for left in [true, false] {
                let (mut app, track) = fixture();
                app.zoom = zoom;
                app.scroll = scroll;
                app.session.project.assets[0].decoded_frame_count = 48000;
                app.session.project.assets[0].source_metadata.channels = 1;
                let clip = &mut app.session.project.tracks[0].clips[0];
                clip.start_frame = frames(2.0);
                clip.source_offset_frame = frames(0.25);
                clip.length_frames = frames(0.5);
                let original = clip.clone();
                app.session.audio.insert(clip.asset_id, audio.clone());
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let block = app.clip_block(lane, &original, original.start_frame);
                let start = Pos2::new(
                    if left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    block.top() + 12.0,
                );
                // Cross into the body after pressing: the initial header hit must keep looping.
                let end = start + Vec2::new(if left { -0.75 * zoom } else { 0.75 * zoom }, 28.0);
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
                let preview = app
                    .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                    .unwrap();
                assert!(preview.valid);
                assert_eq!(
                    preview.clip.start_frame,
                    frames(if left { 1.25 } else { 2.0 })
                );
                assert_eq!(
                    preview.clip.source_offset_frame,
                    original.source_offset_frame
                );
                assert_eq!(preview.clip.length_frames, frames(1.25));
                assert_eq!(
                    preview.clip.repeat,
                    Some(daw_core::ClipLoop {
                        length_frames: frames(0.5),
                        phase_frame: frames(if left { 0.25 } else { 0.0 }),
                    })
                );
                if left {
                    for local in [0, 1, frames(0.25), frames(0.5) - 1] {
                        assert_eq!(
                            preview.clip.source_frame(local + frames(0.75)),
                            original.source_frame(local)
                        );
                    }
                }
                let block = app.clip_block(lane, &preview.clip, preview.clip.start_frame);
                assert!(has_preview_outline(&shapes, block, theme::ACCENT));
                let colors = theme::clip_colors(app.session.project.tracks[0].color, None);
                let extension_x = if left {
                    block.left() + 5.0
                } else {
                    block.right() - 5.0
                };
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke } if stroke.color == colors.waveform.gamma_multiply(0.75)
                        && (points[0].x - extension_x).abs() < 2.0 && points[0].y != points[1].y)));
                assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::LineSegment { points, stroke } if stroke.color == colors.text.gamma_multiply(0.25).gamma_multiply(0.75)
                        && points[0].y == block.top() && points[1].y == block.bottom())));
                assert_eq!(app.session.project.tracks[0].clips[0].repeat, None);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![button(end, false)]);
                let placed = &app.session.project.tracks[0].clips[0];
                assert_eq!(placed.start_frame, preview.clip.start_frame);
                assert_eq!(placed.length_frames, preview.clip.length_frames);
                assert_eq!(placed.repeat, preview.clip.repeat);
                assert!(std::sync::Arc::ptr_eq(
                    &app.session.audio[&placed.asset_id].samples,
                    &audio.samples
                ));
                assert!(app.dirty);
                assert!(app.error.is_none());
                app.session.project.validate().unwrap();

                let again = Drag {
                    clip: placed.clone(),
                    track,
                    mode: ClipDragMode::LoopRight,
                    origin: end,
                };
                let resized = app
                    .clip_drag_preview(&again, end + Vec2::new(zoom, 0.0), false)
                    .unwrap();
                assert_eq!(resized.clip.length_frames, frames(2.25));
                assert_eq!(resized.clip.repeat, placed.repeat);
                let trim = Drag {
                    mode: ClipDragMode::TrimLeft,
                    ..again
                };
                let trimmed = app
                    .clip_drag_preview(&trim, end + Vec2::new(0.25 * zoom, 0.0), false)
                    .unwrap();
                assert_eq!(trimmed.clip.source_offset_frame, placed.source_offset_frame);
                assert_eq!(trimmed.clip.repeat.unwrap().length_frames, frames(0.5));
                assert_eq!(
                    trimmed.clip.source_frame(0),
                    placed.source_frame(frames(0.25))
                );
                assert_eq!(trimmed.clip.length_frames, frames(1.0));
            }
        }
    }

    #[test]
    fn interrupted_header_loop_drags_leave_the_saved_range_unchanged() {
        for focus_loss in [false, true] {
            let (mut app, track) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = frames(2.0);
            clip.length_frames = frames(1.0);
            let original = clip.clone();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let start = lane.min + Vec2::new(3.0 * app.zoom - 1.0, 12.0);
            let end = start + Vec2::new(app.zoom, 0.0);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert!(app.drag.is_some());
            let (sender, receiver) = mpsc::channel();
            if focus_loss {
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
                app.job = Some(receiver);
                frame(&mut app, &ctx, vec![]);
            }
            assert!(app.drag.is_none());
            frame(&mut app, &ctx, vec![button(end, false)]);
            let clip = &app.session.project.tracks[0].clips[0];
            assert_eq!(
                (
                    clip.start_frame,
                    clip.source_offset_frame,
                    clip.length_frames,
                    clip.repeat
                ),
                (
                    original.start_frame,
                    original.source_offset_frame,
                    original.length_frames,
                    original.repeat
                )
            );
            assert!(!app.dirty);
            assert!(app.error.is_none());
            drop(sender);
        }
    }

    #[test]
    fn header_loops_clamp_timeline_and_minimum_length_without_dirtying_no_ops() {
        for (mode, start, delta, expected_start, expected_length) in [
            (ClipDragMode::LoopLeft, frames(2.0), -20.0, 0, frames(2.5)),
            (
                ClipDragMode::LoopLeft,
                frames(2.0),
                20.0,
                frames(2.0),
                frames(0.5),
            ),
            (
                ClipDragMode::LoopRight,
                frames(2.0),
                -20.0,
                frames(2.0),
                frames(0.5),
            ),
            (ClipDragMode::LoopLeft, 0, -20.0, 0, frames(0.5)),
            (
                ClipDragMode::LoopRight,
                frames(2.0),
                0.0,
                frames(2.0),
                frames(0.5),
            ),
            (
                ClipDragMode::LoopRight,
                u64::MAX - frames(1.0),
                20.0,
                u64::MAX - frames(1.0),
                frames(1.0),
            ),
        ] {
            let (mut app, track) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = start;
            clip.source_offset_frame = frames(3.0);
            clip.length_frames = frames(0.5);
            let original = clip.clone();
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let origin = app.lane_bounds[&track].center();
            let end = origin + Vec2::new(delta * app.zoom, 0.0);
            app.drag = Some(Drag {
                clip: original.clone(),
                track,
                mode,
                origin,
            });
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            assert!(preview.valid);
            assert_eq!(preview.clip.start_frame, expected_start);
            assert_eq!(preview.clip.length_frames, expected_length);
            assert_eq!(
                preview.clip.source_offset_frame,
                original.source_offset_frame
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(end), button(end, false)],
            );
            let placed = &app.session.project.tracks[0].clips[0];
            assert_eq!(placed.start_frame, expected_start);
            assert_eq!(placed.length_frames, expected_length);
            assert_eq!(placed.repeat, preview.clip.repeat);
            let changed = expected_start != start || expected_length != original.length_frames;
            assert_eq!(app.dirty, changed);
            assert_eq!(placed.repeat.is_some(), changed);
            app.session.project.validate().unwrap();
        }
    }

    #[test]
    fn repeated_clip_header_edges_stop_at_the_base_length_in_preview_and_release() {
        for left in [true, false] {
            for initial in [frames(0.25), frames(0.5), frames(1.25)] {
                let (mut app, track) = fixture();
                let clip = &mut app.session.project.tracks[0].clips[0];
                clip.start_frame = frames(2.0);
                clip.source_offset_frame = frames(3.0);
                clip.length_frames = initial;
                clip.repeat = Some(daw_core::ClipLoop {
                    length_frames: frames(0.5),
                    phase_frame: frames(0.25),
                });
                let original = clip.clone();
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let block = app.clip_block(lane, &original, original.start_frame);
                let start = Pos2::new(
                    if left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    block.top() + 12.0,
                );
                let end = start
                    + Vec2::new(
                        if left {
                            20.0 * app.zoom
                        } else {
                            -20.0 * app.zoom
                        },
                        0.0,
                    );
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
                let preview = app
                    .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                    .unwrap();
                let minimum = initial.min(frames(0.5));
                assert!(preview.valid);
                assert_eq!(preview.clip.length_frames, minimum);
                assert_eq!(
                    preview.clip.source_offset_frame,
                    original.source_offset_frame
                );
                if left {
                    assert_eq!(preview.clip.end(), original.end());
                    let repeat = original
                        .repeat
                        .unwrap()
                        .shifted(i128::from(initial - minimum));
                    assert_eq!(
                        preview.clip.repeat,
                        if initial > minimum
                            && (minimum == repeat.length_frames
                                || minimum <= repeat.length_frames - repeat.phase_frame)
                        {
                            None
                        } else {
                            Some(repeat)
                        }
                    );
                } else {
                    assert_eq!(preview.clip.start_frame, original.start_frame);
                    assert_eq!(
                        preview.clip.repeat,
                        if initial > minimum {
                            None
                        } else {
                            original.repeat
                        }
                    );
                }
                assert!(has_preview_outline(
                    &shapes,
                    app.clip_block(lane, &preview.clip, preview.clip.start_frame),
                    theme::ACCENT
                ));
                frame(&mut app, &ctx, vec![button(end, false)]);
                let placed = &app.session.project.tracks[0].clips[0];
                assert_eq!(placed.start_frame, preview.clip.start_frame);
                assert_eq!(placed.length_frames, minimum);
                assert_eq!(placed.repeat, preview.clip.repeat);
                assert_eq!(app.dirty, initial > minimum);
                assert!(app.error.is_none());
                app.session.project.validate().unwrap();
            }
        }
    }

    #[test]
    fn trimming_looping_and_returning_to_the_base_restores_full_source_extension() {
        fn drag_edge(
            app: &mut DawUi,
            ctx: &egui::Context,
            track: Id,
            left: bool,
            header: bool,
            delta: f32,
        ) {
            frame(app, ctx, vec![]);
            let clip = app.session.project.tracks[0].clips[0].clone();
            let lane = app.lane_bounds[&track];
            let block = app.clip_block(lane, &clip, clip.start_frame);
            let start = Pos2::new(
                if left {
                    block.left() + 1.0
                } else {
                    block.right() - 1.0
                },
                block.top() + if header { 12.0 } else { 40.0 },
            );
            let end = start + Vec2::new(delta * app.zoom, 0.0);
            frame(
                app,
                ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            assert!(preview.valid);
            frame(app, ctx, vec![button(end, false)]);
            let placed = &app.session.project.tracks[0].clips[0];
            assert_eq!(placed.start_frame, preview.clip.start_frame);
            assert_eq!(placed.source_offset_frame, preview.clip.source_offset_frame);
            assert_eq!(placed.length_frames, preview.clip.length_frames);
            assert_eq!(placed.repeat, preview.clip.repeat);
            assert!(app.error.is_none());
            app.session.project.validate().unwrap();
        }

        let source = TestWav::new();
        let audio = daw_media::decode_wav(&source.0).unwrap();
        for (left, return_left) in [(false, false), (true, true), (false, true), (true, false)] {
            let (mut app, track) = fixture();
            app.session.project.assets[0].decoded_frame_count = 48000;
            app.session.project.assets[0].source_metadata.channels = 1;
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = frames(2.0);
            clip.length_frames = frames(1.0);
            let original = clip.clone();
            app.session.audio.insert(clip.asset_id, audio.clone());
            let ctx = context();
            drag_edge(&mut app, &ctx, track, true, false, 0.25);
            drag_edge(&mut app, &ctx, track, false, false, -0.25);
            let trimmed = app.session.project.tracks[0].clips[0].clone();
            assert_eq!(trimmed.source_offset_frame, frames(0.25));
            assert_eq!(trimmed.length_frames, frames(0.5));
            drag_edge(
                &mut app,
                &ctx,
                track,
                left,
                true,
                if left { -0.75 } else { 0.75 },
            );
            let looped = app.session.project.tracks[0].clips[0].clone();
            assert!(looped.repeat.is_some());
            drag_edge(
                &mut app,
                &ctx,
                track,
                return_left,
                true,
                if return_left { 0.75 } else { -0.75 },
            );
            let returned = &app.session.project.tracks[0].clips[0];
            assert_eq!(returned.repeat, None);
            let returned_start = if return_left {
                looped.end() - trimmed.length_frames
            } else {
                looped.start_frame
            };
            assert_eq!(returned.start_frame, returned_start);
            assert_eq!(returned.source_offset_frame, trimmed.source_offset_frame);
            assert_eq!(returned.length_frames, trimmed.length_frames);
            drag_edge(&mut app, &ctx, track, true, false, -2.0);
            drag_edge(&mut app, &ctx, track, false, false, 2.0);
            let restored = &app.session.project.tracks[0].clips[0];
            assert_eq!(restored.repeat, None);
            assert_eq!(
                restored.start_frame,
                returned_start - trimmed.source_offset_frame
            );
            assert_eq!(restored.source_offset_frame, original.source_offset_frame);
            assert_eq!(restored.length_frames, original.length_frames);
            assert!(std::sync::Arc::ptr_eq(
                &app.session.audio[&restored.asset_id].samples,
                &audio.samples
            ));
        }

        // Previously saved base-length loops must also allow source extension.
        for phase in [0, frames(0.25)] {
            let (mut app, track) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = frames(2.0);
            clip.source_offset_frame = frames(3.0);
            clip.length_frames = frames(1.0);
            clip.repeat = Some(daw_core::ClipLoop {
                length_frames: frames(1.0),
                phase_frame: phase,
            });
            let ctx = context();
            drag_edge(&mut app, &ctx, track, false, false, 20.0);
            let restored = &app.session.project.tracks[0].clips[0];
            assert_eq!(restored.repeat, None);
            assert_eq!(restored.source_offset_frame, frames(3.0));
            assert_eq!(restored.length_frames, frames(7.0));
        }
    }

    #[test]
    fn clip_trim_preview_shows_the_waveform_and_matches_the_committed_range() {
        let source = TestWav::new();
        let audio = daw_media::decode_wav(&source.0).unwrap();
        for (zoom, scroll) in [(70.0, 0.0), (140.0, 1.0)] {
            for left in [true, false] {
                let (mut app, track) = fixture();
                app.zoom = zoom;
                app.scroll = scroll;
                app.session.project.assets[0].decoded_frame_count = 48000;
                app.session.project.assets[0].source_metadata.channels = 1;
                let clip = &mut app.session.project.tracks[0].clips[0];
                clip.start_frame = frames(2.0);
                clip.source_offset_frame = frames(0.25);
                clip.length_frames = frames(0.5);
                let original = clip.clone();
                app.session.audio.insert(clip.asset_id, audio.clone());
                let ctx = context();
                frame(&mut app, &ctx, vec![]);
                let lane = app.lane_bounds[&track];
                let block = app.clip_block(lane, &original, original.start_frame);
                let start = Pos2::new(
                    if left {
                        block.left() + 1.0
                    } else {
                        block.right() - 1.0
                    },
                    lane.top() + 40.0,
                );
                let end = start + Vec2::new(if left { 0.25 * zoom } else { -0.25 * zoom }, 0.0);
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
                let preview = app
                    .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                    .unwrap();
                assert!(preview.valid);
                assert_eq!(preview.track, track);
                assert_eq!(
                    preview.clip.start_frame,
                    if left { frames(2.25) } else { frames(2.0) }
                );
                assert_eq!(
                    preview.clip.source_offset_frame,
                    if left { frames(0.5) } else { frames(0.25) }
                );
                assert_eq!(preview.clip.length_frames, frames(0.25));
                let block = app.clip_block(lane, &preview.clip, preview.clip.start_frame);
                assert!(has_preview_outline(&shapes, block, theme::ACCENT));
                assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == theme::clip_colors(app.session.project.tracks[0].color, None).waveform.gamma_multiply(0.75))));
                let unchanged = &app.session.project.tracks[0].clips[0];
                assert_eq!(
                    (
                        unchanged.start_frame,
                        unchanged.source_offset_frame,
                        unchanged.length_frames
                    ),
                    (
                        original.start_frame,
                        original.source_offset_frame,
                        original.length_frames
                    )
                );
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![button(end, false)]);
                let placed = &app.session.project.tracks[0].clips[0];
                assert_eq!(
                    (
                        placed.start_frame,
                        placed.source_offset_frame,
                        placed.length_frames
                    ),
                    (
                        preview.clip.start_frame,
                        preview.clip.source_offset_frame,
                        preview.clip.length_frames
                    )
                );
                assert!(std::sync::Arc::ptr_eq(
                    &app.session.audio[&placed.asset_id].samples,
                    &audio.samples
                ));
                assert!(app.dirty);
                assert!(app.error.is_none());
                app.session.project.validate().unwrap();
            }
        }
    }
    #[test]
    fn clip_trim_preview_and_release_clamp_source_timeline_and_minimum_length() {
        for (left, start_seconds, offset_seconds, length_seconds, delta, expected) in [
            (true, 5.0, 3.0, 4.0, -20.0, (frames(2.0), 0, frames(7.0))),
            (true, 1.0, 3.0, 4.0, -20.0, (0, frames(2.0), frames(5.0))),
            (
                false,
                2.0,
                3.0,
                4.0,
                20.0,
                (frames(2.0), frames(3.0), frames(7.0)),
            ),
            (
                true,
                2.0,
                3.0,
                4.0,
                20.0,
                (frames(6.0) - 1, frames(7.0) - 1, 1),
            ),
            (false, 2.0, 3.0, 4.0, -20.0, (frames(2.0), frames(3.0), 1)),
            (true, 1.0, 0.0, 4.0, -20.0, (frames(1.0), 0, frames(4.0))),
            (false, 1.0, 0.0, 10.0, 20.0, (frames(1.0), 0, frames(10.0))),
        ] {
            let (mut app, track) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = frames(start_seconds);
            clip.source_offset_frame = frames(offset_seconds);
            clip.length_frames = frames(length_seconds);
            let original = (
                clip.start_frame,
                clip.source_offset_frame,
                clip.length_frames,
            );
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let block = app.clip_block(
                lane,
                &app.session.project.tracks[0].clips[0],
                frames(start_seconds),
            );
            let start = Pos2::new(
                if left {
                    block.left() + 1.0
                } else {
                    block.right() - 1.0
                },
                lane.top() + 40.0,
            );
            let end = start + Vec2::new(delta * app.zoom, 0.0);
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
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            let clip = &preview.clip;
            assert_eq!(
                (
                    clip.start_frame,
                    clip.source_offset_frame,
                    clip.length_frames
                ),
                expected
            );
            assert!(preview.valid);
            assert!(has_preview_outline(
                &shapes,
                app.clip_block(lane, clip, clip.start_frame),
                theme::ACCENT
            ));
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![button(end, false)]);
            let clip = &app.session.project.tracks[0].clips[0];
            assert_eq!(
                (
                    clip.start_frame,
                    clip.source_offset_frame,
                    clip.length_frames
                ),
                expected
            );
            assert_eq!(app.dirty, expected != original);
            assert!(app.error.is_none());
            app.session.project.validate().unwrap();
        }
    }
    #[test]
    fn overlapping_clip_trim_and_loop_previews_are_red_and_retain_the_original() {
        for (left, header) in [(true, false), (false, false), (true, true), (false, true)] {
            let (mut app, track) = fixture();
            let clip = &mut app.session.project.tracks[0].clips[0];
            clip.start_frame = frames(2.0);
            clip.source_offset_frame = frames(3.0);
            clip.length_frames = frames(4.0);
            let original = clip.clone();
            let mut neighbor = clip.clone();
            neighbor.id = Id::new_v4();
            neighbor.start_frame = frames(if left { 0.0 } else { 7.0 });
            neighbor.source_offset_frame = 0;
            neighbor.length_frames = frames(1.0);
            app.session.project.tracks[0].clips.push(neighbor);
            let ctx = context();
            frame(&mut app, &ctx, vec![]);
            let lane = app.lane_bounds[&track];
            let block = app.clip_block(lane, &original, original.start_frame);
            let start = Pos2::new(
                if left {
                    block.left() + 1.0
                } else {
                    block.right() - 1.0
                },
                lane.top() + if header { 12.0 } else { 40.0 },
            );
            let end = start
                + Vec2::new(
                    if left {
                        -1.5 * app.zoom
                    } else {
                        2.0 * app.zoom
                    },
                    0.0,
                );
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
            let preview = app
                .clip_drag_preview(app.drag.as_ref().unwrap(), end, false)
                .unwrap();
            assert!(!preview.valid);
            assert!(has_preview_outline(
                &shapes,
                app.clip_block(lane, &preview.clip, preview.clip.start_frame),
                theme::ERROR
            ));
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(
                app.error
                    .as_ref()
                    .is_some_and(|error| error.contains("overlap"))
            );
            assert!(!app.dirty);
            let clip = app.session.project.tracks[0]
                .clips
                .iter()
                .find(|clip| clip.id == original.id)
                .unwrap();
            assert_eq!(
                (
                    clip.start_frame,
                    clip.source_offset_frame,
                    clip.length_frames
                ),
                (
                    original.start_frame,
                    original.source_offset_frame,
                    original.length_frames
                )
            );
            assert_eq!(clip.repeat, original.repeat);
            app.session.project.validate().unwrap();
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
