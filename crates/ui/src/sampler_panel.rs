//! Sampler presentation and drop adapters. Audio preparation belongs to Session workers.
use super::*;
use daw_core::{
    InstrumentComponent, InstrumentLayerSettings, SamplerMode, SamplerSettings, SamplerTrigger,
};

const DURATION_INPUT_WIDTH: f32 = 120.0;
const DURATION_UNIT_WIDTH: f32 = 60.0;

#[derive(Clone)]
struct RangeDrag {
    original: SamplerSettings,
    origin: u64,
    mode: LoopDragMode,
    preview: [u64; 2],
}
#[derive(Clone)]
struct InputDraft<T> {
    saved: T,
    value: T,
    generation: u64,
}
pub(super) struct Bounds {
    pub track: Id,
    pub layer: Id,
    pub panel: Rect,
    pub waveform: Rect,
    pub range: Rect,
    pub ruler: Rect,
    pub root: Rect,
    pub root_group: Rect,
    pub sharp: Rect,
    pub duration: Rect,
    pub duration_value: Rect,
    pub duration_unit: Rect,
    pub gain: Rect,
    pub trigger: Rect,
    pub controls: Rect,
    pub grains: [Rect; 9],
    pub loop_toggle: Rect,
}
impl Bounds {
    fn new(track: Id, layer: Id) -> Self {
        Self {
            track,
            layer,
            panel: Rect::NOTHING,
            waveform: Rect::NOTHING,
            range: Rect::NOTHING,
            ruler: Rect::NOTHING,
            root: Rect::NOTHING,
            root_group: Rect::NOTHING,
            sharp: Rect::NOTHING,
            duration: Rect::NOTHING,
            duration_value: Rect::NOTHING,
            duration_unit: Rect::NOTHING,
            gain: Rect::NOTHING,
            trigger: Rect::NOTHING,
            controls: Rect::NOTHING,
            grains: [Rect::NOTHING; 9],
            loop_toggle: Rect::NOTHING,
        }
    }
}
impl DawUi {
    /// Hover Instrument while carrying one audio clip. Other view/drag guards stay intact.
    pub(super) fn sampler_drag_navigation(&mut self, ctx: &egui::Context) {
        if self.view_mode != ViewMode::Timeline
            || !self.edit_environment_available()
            || !ctx.input(|i| i.focused && i.pointer.primary_down())
            || !self.drag.as_ref().is_some_and(|d| {
                d.mode == ClipDragMode::Move
                    && self
                        .session
                        .project
                        .assets
                        .iter()
                        .any(|a| a.id == d.clip.asset_id && a.midi_pattern.is_none())
            })
        {
            return;
        }
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| {
                t.instrument
                    .as_ref()
                    .is_some_and(|i| i.layers.iter().any(|l| l.sampler.is_some()))
            })
            .map(|t| t.id)
        else {
            return;
        };
        let drag = self.drag.take();
        self.set_view_mode(ViewMode::Instrument, ctx);
        if self.view_mode == ViewMode::Instrument {
            self.selected_track = Some(track);
            self.master_selected = false;
        }
        self.drag = drag;
    }
    pub(super) fn sampler_unit(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        layer: &mut InstrumentLayerSettings,
        sources: &[modulation_panel::Source],
        height: f32,
        compact: bool,
    ) -> Rect {
        let audio_ready = self.media_ready();
        let layer_id = layer.id;
        let s = layer.sampler.as_mut().unwrap();
        let mut bounds = Bounds::new(track, layer_id);
        let mut enabled = s.enabled;
        let mut collapsed = s.collapsed;
        let margin = panels::PanelStyle::default().margin;
        let open = s.modulation_open;
        let live = self.output.as_ref().and_then(|o| {
            o.modulation_snapshot(daw_engine::ModulationTarget::Sampler {
                track,
                layer: layer_id,
            })
        });
        let marker = modulation_panel::marker(
            &s.modulations,
            sources,
            daw_core::ModulationParameter::SamplerGain,
            s.gain_db,
            s.enabled,
            None,
            [live.map(|v| v[1]), live.map(|v| v[2])],
        );
        let mut choose = false;
        let result = ui.scope_builder(egui::UiBuilder::new().ui_stack_info(
            egui::UiStackInfo::default().with_tag("unit-modulation-button")), |ui| {
            if !audio_ready { ui.disable(); }
            panels::PanelStyle {
            margin: 0,
            ..Default::default()
        }
        .unit(
            ui,
            (s.name(), compact),
            height,
            &mut enabled,
            &mut collapsed,
            |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing =
                        Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                    let preview = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                            ui.set_width(270.0);
                            ui.set_min_height(envelope_panel::body_height(compact));
                            if compact {
                                rows::centered(ui, envelope_panel::body_height(true), |ui| {
                                    rows::unit_label(
                                        ui,
                                        s.input
                                            .as_ref()
                                            .map_or("Drop audio here", |i| i.name.as_str()),
                                    )
                                });
                                return;
                            }
                            let (wave, _) =
                                ui.allocate_exact_size(Vec2::new(270.0, 154.0), Sense::hover());
                            bounds.waveform = wave;
                            let bar = Rect::from_min_max(
                                wave.min,
                                egui::pos2(wave.right(), wave.top() + 18.0),
                            );
                            bounds.range = bar;
                            let ruler = Rect::from_min_max(
                                wave.min,
                                bar.right_bottom() + Vec2::new(0.0, 20.0),
                            );
                            bounds.ruler = ruler;
                            let clip_rect = Rect::from_min_max(ruler.left_bottom(), wave.max);
                            let body = Rect::from_min_max(
                                clip_rect.min + Vec2::new(0.0, 22.0),
                                clip_rect.max,
                            );
                            let palette = theme::palette(ui.ctx());
                            ui.painter().rect_filled(wave, 0.0, palette.background);
                            if let Some(input) = s.input.clone() {
                                let clip = Clip::from(&input);
                                let colors = theme::clip_colors(
                                    self.session
                                        .project
                                        .tracks
                                        .iter()
                                        .find(|t| t.id == track)
                                        .map_or(daw_core::DEFAULT_TRACK_COLOR, |t| t.color),
                                    None,
                                );
                                ui.painter().rect_filled(body, 0.0, colors.body);
                                crate::clip_paint::paint_header(
                                    ui.painter(),
                                    clip_rect,
                                    &input.name,
                                    colors,
                                    false,
                                );
                                if let Some(audio) = self.session.audio_for_clip(&clip) {
                                    waveforms::paint(
                                        ui.painter(),
                                        body,
                                        audio,
                                        &clip,
                                        input.length_frames as f64 / f64::from(body.width()),
                                        colors,
                                        None,
                                    );
                                } else {
                                    ui.painter().text(
                                        body.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "Missing source",
                                        fonts::numeric(11.0),
                                        palette.warning,
                                    );
                                }
                                let preview = selection(
                                    ui,
                                    bar,
                                    s,
                                    self.session.project.sample_rate_hz,
                                    self.session.project.tempo_bpm,
                                );
                                let pixel = |f: u64| {
                                    wave.left()
                                        + f as f32 / input.length_frames as f32 * wave.width()
                                };
                                range_selection::paint(
                                    ui,
                                    bar,
                                    pixel(preview[0]),
                                    pixel(preview[1]),
                                    true,
                                );
                                let clock = preview_clock(
                                    s,
                                    wave.width(),
                                    self.session.project.sample_rate_hz,
                                    self.session.project.tempo_bpm,
                                );
                                clock.timeline.paint_grid(&ui.painter_at(body), body);
                                clock.timeline.paint_ruler_ticks(
                                    &ui.painter_at(ruler),
                                    ruler,
                                    bar,
                                    preview.map(|frame| clock.output_frame(frame)),
                                    true,
                                );
                                for r in [
                                    Rect::from_min_max(
                                        body.min,
                                        egui::pos2(pixel(preview[0]), body.bottom()),
                                    ),
                                    Rect::from_min_max(
                                        egui::pos2(pixel(preview[1]), body.top()),
                                        body.max,
                                    ),
                                ] {
                                    ui.painter_at(body).rect_filled(
                                        r,
                                        0.0,
                                        egui::Color32::from_black_alpha(100),
                                    );
                                }
                                if let Some(g) = &s.granulizer {
                                    let scan = live.as_ref().map_or(g.position * (s.end_frame - s.start_frame) as f32, |v| v[0]);
                                    let jitter = live.as_ref().map_or(g.position_jitter, |v| v[8]);
                                    let length = (s.end_frame - s.start_frame) as f32;
                                    let to_x = |p: f32| pixel(s.start_frame) + p / input.length_frames as f32 * wave.width();
                                    let range = Rect::from_min_max(egui::pos2(to_x((scan - jitter * length).max(0.0)), body.top()), egui::pos2(to_x((scan + jitter * length).min(length)), body.bottom()));
                                    ui.painter_at(body).rect_filled(range, 0.0, theme::accent(ui.ctx()).linear_multiply(0.15));
                                    if let Some(values) = &live {
                                        for p in values[11..27].iter().filter(|p| p.is_finite()) {
                                            ui.painter_at(body).vline(to_x(*p), body.y_range(), Stroke::new(1.0, palette.secondary));
                                        }
                                    }
                                }
                                if let Some(position) = self
                                    .output
                                    .as_ref()
                                    .and_then(|o| {
                                        o.modulation_snapshot(daw_engine::ModulationTarget::Sampler {
                                            track,
                                            layer: layer_id,
                                        })
                                    })
                                    .map(|v| v[0])
                                    .filter(|v| v.is_finite())
                                {
                                    let duration = s
                                        .output_frames(
                                            self.session.project.sample_rate_hz,
                                            self.session.project.tempo_bpm,
                                        )
                                        .unwrap_or(1);
                                    let source = s.start_frame as f32
                                        + position / duration as f32
                                            * (s.end_frame - s.start_frame) as f32;
                                    let x = wave.left()
                                        + source / input.length_frames as f32 * wave.width();
                                    ui.painter_at(body).vline(
                                        x,
                                        body.y_range(),
                                        Stroke::new(1.0, theme::accent(ui.ctx())),
                                    );
                                }
                            } else {
                                ui.painter().text(
                                    body.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Drop an audio file or clip",
                                    fonts::numeric(11.0),
                                    palette.secondary,
                                );
                                let picker = Rect::from_center_size(body.center() + egui::vec2(0.0, 24.0), egui::vec2(128.0, 22.0));
                                if ui.put(picker, egui::Button::new("Choose sample…")).clicked() { choose = true; }
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x =
                                    f32::from(theme::TOOLBAR_PADDING) * 0.5;
                                let (root, sharp, group) = root_note(ui, &mut s.root_note);
                                bounds.root = root;
                                bounds.sharp = sharp;
                                bounds.root_group = group;
                                if let Some(g) = &mut s.granulizer {
                                    bounds.trigger = trigger(ui, &mut s.trigger);
                                    bounds.loop_toggle = ui.checkbox(&mut g.looping, "Loop").on_hover_text("Wrap scan and grain reads inside the selected region").rect;
                                } else {
                                let (group, value, unit) = duration(
                                    ui,
                                    s,
                                    self.session.project.sample_rate_hz,
                                    self.session.project.tempo_bpm,
                                );
                                bounds.duration = group;
                                bounds.duration_value = value;
                                bounds.duration_unit = unit;
                                bounds.trigger = trigger(ui, &mut s.trigger);
                                }
                            });
                        })
                    });
                    panels::unit_divider(
                        ui,
                        preview.response.rect.right(),
                        preview.response.rect.y_range().into(),
                    );
                    if s.granulizer.is_some() {
                        bounds.grains = crate::granulizer_panel::controls(ui, s, sources, live.as_ref(), compact);
                    }
                    let strip = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                            ui.set_min_height(envelope_panel::body_height(compact));
                            ui.set_width(
                                midi_editor::UNIT_STRIP_WIDTH
                                    - 2.0 * f32::from(panels::PanelStyle::default().margin),
                            );
                            if !compact {
                                bounds.gain = knobs::row(
                                    ui,
                                    s,
                                    knobs::CHOICE_SIZE,
                                    |ui, s| ui.add(knobs::Knob::new(&mut s.gain_db,
                                        parameter_knobs::destination(daw_core::ModulationParameter::SamplerGain))
                                        .modulated(marker.as_ref(), &mut s.modulation_open)),
                                )
                                .rect;
                                for _ in 0..2 {
                                    ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), knobs::CHOICE_SIZE),
                                        Sense::hover(),
                                    );
                                }
                            }
                            rows::centered(
                                ui,
                                if compact {
                                    envelope_panel::body_height(true)
                                } else {
                                    icons::COMPACT_BUTTON_SIZE
                                },
                                |ui| rows::unit_label(ui, if s.granulizer.is_some() { "Out" } else { "Sample" }),
                            );
                        })
                    });
                    bounds.controls = strip.response.rect;
                    if open {
                        panels::unit_divider(ui, strip.response.rect.right(), strip.response.rect.y_range().into());
                        let parameters = s.parameters();
                        modulation_panel::body(ui, &mut s.modulations, sources,
                            parameters,
                            <SamplerSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                    }
                });
            },
        ) }).inner;
        s.enabled = enabled;
        s.collapsed = collapsed;
        if !compact && s.granulizer.is_some() && bounds.controls.is_positive() {
            let target = daw_engine::SourceOutputTarget {
                track,
                layer: layer_id,
                unit: s.id,
            };
            let reading = meters::OutputLevel::new(
                self.output
                    .as_ref()
                    .map_or(0.0, |o| o.source_output_level(target)),
                self.output
                    .as_ref()
                    .and_then(|o| o.source_output_peak(target)),
            );
            meters::effect_output_level(
                ui,
                bounds.controls,
                panels::PanelStyle::default().margin,
                reading,
                s.enabled,
            );
        }

        let toggle =
            modulation_panel::toggle(ui, &result.header, s.collapsed, &mut s.modulation_open);
        bounds.panel = result.response.rect.intersect(ui.clip_rect());
        let carrying = ui.input(|i| !i.raw.hovered_files.is_empty())
            || self
                .drag
                .as_ref()
                .is_some_and(|d| d.mode == ClipDragMode::Move);
        if carrying && ui.rect_contains_pointer(bounds.panel) {
            panels::selection_border(ui, bounds.panel);
        }
        self.start_installed_component_drag(
            ui,
            &result.header,
            &[result.power.rect, toggle],
            track,
            layer_id,
            InstrumentComponent::Sampler,
        );
        let mut remove = false;
        let mut replace = false;
        let mut switch = false;
        let power_menu = crate::unit_power::PowerMenu::from_ui(ui);
        crate::menus::ContextMenu::new(&result.header).show(|ui| {
            if !audio_ready {
                ui.disable();
            }
            if let Some(menu) = &power_menu {
                menu.show(ui);
            }
            if ui.button("Choose sample…").clicked() {
                choose = true;
                ui.close();
            }
            crate::menus::separator(ui);
            if ui
                .button(format!("Remove {}", s.name().to_lowercase()))
                .clicked()
            {
                remove = true;
                ui.close();
            }
            if ui
                .button(if s.granulizer.is_some() {
                    "Replace source with Sampler"
                } else {
                    "Replace source with Granulizer"
                })
                .clicked()
            {
                switch = true;
                ui.close();
            }
            if ui.button("Replace source with Oscillator").clicked() {
                replace = true;
                ui.close();
            }
        });
        if power_menu
            .as_ref()
            .is_some_and(|menu| menu.action().is_some())
        {
            crate::unit_power::schedule_focus(ui, &result.header);
        }
        if switch {
            s.granulizer = if s.granulizer.is_some() {
                None
            } else {
                Some(Default::default())
            };
            s.version = if s.granulizer.is_some() { 2 } else { 1 };
            s.modulations
                .retain(|r| r.parameter == daw_core::ModulationParameter::SamplerGain);
            s.mode = SamplerMode::Off;
            s.trigger = if s.granulizer.is_some() {
                SamplerTrigger::Gate
            } else {
                SamplerTrigger::OneShot
            };
        }
        if remove || replace {
            layer.sampler = None;
            layer.prune_component_order();
        }
        if replace {
            layer.oscillators.push(Default::default());
            layer.component_order.retain(|c| !c.is_source());
        }
        // The full expanded body is a drop target; compact/collapsed units retain a target.
        self.instrument_ui.samplers.push(bounds);
        if choose
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Audio samples", daw_media::IMPORT_EXTENSIONS)
                .pick_file()
        {
            self.load_sampler_file((track, layer_id), path);
        }
        result.response.rect
    }
    pub(super) fn sampler_drop_target(&self, pointer: Pos2) -> Option<(Id, Id)> {
        if !self.media_ready() {
            return None;
        }
        if self.view_mode != ViewMode::Instrument {
            return None;
        }
        self.instrument_ui
            .samplers
            .iter()
            .find(|b| Some(b.track) == self.selected_track && b.panel.contains(pointer))
            .map(|b| (b.track, b.layer))
    }
    pub(super) fn run_sampler_job(
        &mut self,
        label: &str,
        job: impl FnOnce(&std::sync::atomic::AtomicBool) -> Result<Session, String> + Send + 'static,
    ) {
        if !self.media_ready() {
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = cancel.clone();
        self.run_job(label, move || job(&flag).map(|s| Job::Loaded(s, true)));
        self.settings_cancel = Some(cancel);
    }
    pub(super) fn load_sampler_file(&mut self, target: (Id, Id), path: PathBuf) {
        let session = self.session.clone();
        let hover = self.file_hover.take().filter(|h| h.path == path);
        self.run_sampler_job("Preparing sample", move |cancel| {
            let audio = if let Some(hover) = hover {
                hover.into_audio()?
            } else {
                daw_media::decode_audio_cached(&path, session.project.sample_rate_hz, &|| {
                    cancel.load(Ordering::Relaxed)
                })
                .map_err(|e| e.to_string())?
            };
            session
                .with_sampler_decoded(target.0, target.1, &path, audio, &|| {
                    cancel.load(Ordering::Relaxed)
                })
                .map_err(|e| e.to_string())
        });
    }
}
/// Keep drag previews local and let the standard input own text entry and stepping.
fn staged_input<T: Copy + PartialEq + Send + Sync + 'static>(
    ui: &mut egui::Ui,
    key: &str,
    saved: &mut T,
    add: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
) -> (egui::Response, T) {
    let id = ui.make_persistent_id(key);
    let mut draft = ui
        .data_mut(|d| d.get_temp::<InputDraft<T>>(id))
        .unwrap_or(InputDraft {
            saved: *saved,
            value: *saved,
            generation: 0,
        });
    if draft.saved != *saved {
        draft.saved = *saved;
        draft.value = *saved;
        draft.generation = draft.generation.wrapping_add(1);
    }
    // Capture Escape before TextEdit consumes it. A new widget generation discards
    // its pending text, so losing focus on the next frame cannot commit that text.
    let cancelled =
        !ui.is_enabled() || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape));
    let response = ui
        .push_id(draft.generation, |ui| add(ui, &mut draft.value))
        .inner;
    if cancelled {
        draft.value = *saved;
        draft.generation = draft.generation.wrapping_add(1);
        response.surrender_focus();
    } else if response.drag_stopped()
        || (response.changed() && !ui.input(|i| i.pointer.primary_down()))
    {
        *saved = draft.value;
        draft.saved = *saved;
    } else if !response.dragged() && !response.has_focus() {
        draft.value = *saved;
    }
    let preview = draft.value;
    ui.data_mut(|d| d.insert_temp(id, draft));
    (response, preview)
}
fn small_input_text(ui: &mut egui::Ui) {
    let font = fonts::numeric(egui::TextStyle::Small.resolve(ui.style()).size);
    ui.style_mut().drag_value_text_style = egui::TextStyle::Small;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Small, font);
}
fn root_note(ui: &mut egui::Ui, note: &mut u8) -> (Rect, Rect, Rect) {
    let group = rows::input_group(ui, |ui| {
        small_input_text(ui);
        let sharp = matches!(*note % 12, 1 | 3 | 6 | 8 | 10);
        let (response, preview) = staged_input(ui, "root-note-draft", note, |ui, value| {
            ui.add_sized([NUMERIC_WIDTH, ui.spacing().interact_size.y],
            egui::DragValue::new(value)
                .speed(0.1)
                .range(0..=127)
                .custom_formatter(|value, _| daw_unit_core::note_name(value as u8).replace('#', ""))
                .custom_parser(|text| daw_unit_core::natural_note(text, sharp).map(f64::from))
                .update_while_editing(false),
        ).on_hover_text("Sample root note (C−1 to G9). Drag in semitone steps; Shift for fine control. Click to type a natural note and octave. All MIDI notes play relative to this root; duration stays unchanged.")
        });
        // The toggle follows the preview during a drag, including accidental crossings.
        let sharp = matches!(preview % 12, 1 | 3 | 6 | 8 | 10);
        let natural = daw_unit_core::note_name(preview).replace('#', "");
        let toggle = ui
            .add_enabled(
                daw_unit_core::natural_note(&natural, true).is_some() && ui.input(|i| i.focused),
                egui::Button::new(egui::RichText::new("#").small()).selected(sharp),
            )
            .on_hover_text("Sharp modifier");
        if toggle.clicked() {
            *note = daw_unit_core::natural_note(&natural, !sharp).unwrap();
        }
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(preview));
            info.label = Some("Sample root note".into());
            info
        });
        (response, toggle)
    });
    let (response, toggle) = group.inner;
    rows::input_group_outline(
        ui,
        group.response.rect,
        toggle.rect.left(),
        &[&response, &toggle],
    );
    (response.rect, toggle.rect, group.response.rect)
}
/// Keep the compact numeric segment fixed even for unusually long saved values.
fn duration_value(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    text: String,
    widget: impl egui::Widget,
) -> egui::Response {
    ui.scope(|ui| {
        let mut font = egui::TextStyle::Small.resolve(ui.style());
        let available = (width - 2.0 * ui.spacing().button_padding.x).max(1.0);
        let needed = ui
            .painter()
            .layout_no_wrap(text, font.clone(), theme::palette(ui.ctx()).text)
            .size()
            .x;
        if needed > available {
            font.size *= available / needed;
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Small, font);
        }
        ui.add_sized([width, height], widget)
    })
    .inner
}
fn choice_dropdown<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    key: &str,
    name: &str,
    value: &mut T,
    selected: &str,
    options: &[(T, &str)],
    width: f32,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            small_input_text(ui);
            ui.spacing_mut().button_padding.x = 3.0;
            ui.spacing_mut().icon_width = 12.0;
            ui.spacing_mut().icon_spacing = 4.0;
            crate::menus::Dropdown::from_id_salt(key)
                .small_items()
                .selected_text(egui::RichText::new(selected).small())
                .width(width)
                .wrap_mode(egui::TextWrapMode::Truncate)
                .show_ui(ui, |ui| {
                    for (choice, label) in options {
                        ui.selectable_value(value, *choice, *label);
                    }
                })
                .response
        })
        .inner;
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, response.enabled(), name);
        info.current_text_value = Some(selected.into());
        info
    });
    response
}
fn trigger(ui: &mut egui::Ui, value: &mut SamplerTrigger) -> Rect {
    let selected = if *value == SamplerTrigger::OneShot {
        "One-shot"
    } else {
        "Gate"
    };
    choice_dropdown(ui, "trigger-mode", "Sample trigger mode", value, selected,
        &[(SamplerTrigger::OneShot, "One-shot"), (SamplerTrigger::Gate, "Gate")], 76.0)
        .on_hover_text("Trigger mode: One-shot continues after note-off; Gate follows the note gate and Amplitude release.")
        .rect
}
fn duration(
    ui: &mut egui::Ui,
    s: &mut SamplerSettings,
    rate: u32,
    tempo: f32,
) -> (Rect, Rect, Rect) {
    let unit = match s.mode {
        SamplerMode::Off => "Off",
        SamplerMode::Time => "ms",
        SamplerMode::Bars => "bars",
    };
    let previous_mode = s.mode;
    let previous_frames = s.output_frames(rate, tempo);
    let group =
        rows::input_group(ui, |ui| {
            small_input_text(ui);
            ui.spacing_mut().button_padding.x = 6.0;
            let height = ui.spacing().interact_size.y;
            let border = (toolbars::CONTROL_HEIGHT - height) * 0.5;
            let value_width = DURATION_INPUT_WIDTH - DURATION_UNIT_WIDTH - 2.0 * border;
            let response =
                if s.mode == SamplerMode::Bars {
                    staged_input(ui, "bars-draft", &mut s.bars, |ui, value| {
                        duration_value(ui, value_width, height, value.to_string(),
                    egui::DragValue::new(value)
                        .speed(0.1)
                        .range(1..=u32::MAX)
                        .update_while_editing(false),
                )
                .on_hover_text(
                    "Drag in whole-bar steps, minimum 1 bar. Click to type a whole-bar duration.",
                )
                    })
                    .0
                } else if s.mode == SamplerMode::Off {
                    let mut original_ms =
                        (s.end_frame - s.start_frame) as f64 * 1000.0 / f64::from(rate);
                    ui.add_enabled_ui(false, |ui| {
                duration_value(ui, value_width, height,
                    egui::emath::format_with_decimals_in_range(original_ms, 0..=2),
                    egui::DragValue::new(&mut original_ms))
            })
            .inner
            .on_hover_text(
                "Original selected duration in milliseconds. Select Time or Bars to change it.",
            )
                } else {
                    staged_input(ui, "time-draft", &mut s.time_ms, |ui, value| {
                        duration_value(ui, value_width, height,
                    egui::emath::format_with_decimals_in_range(*value, 0..=2),
                    egui::DragValue::new(value)
                        .speed(1.0)
                        .range(0.000001..=1e12)
                        .update_while_editing(false),
                )
                .on_hover_text(
                    "Drag to adjust; Shift for fine adjustment. Click to type milliseconds.",
                )
                    })
                    .0
                };
            let label = choice_dropdown(
                ui,
                "stretch-mode",
                "Sample duration mode",
                &mut s.mode,
                unit,
                &[
                    (SamplerMode::Off, "Off"),
                    (SamplerMode::Time, "Time (ms)"),
                    (SamplerMode::Bars, "Bars"),
                ],
                DURATION_UNIT_WIDTH,
            )
            .on_hover_text("Stretch mode: Off, Time (ms), or Bars");
            if s.mode != previous_mode
                && let Some(frames) = previous_frames
            {
                s.set_duration_from_frames(frames, rate, tempo);
            }
            response.widget_info(|| {
                let value = match previous_mode {
                    SamplerMode::Bars => f64::from(s.bars),
                    SamplerMode::Time => s.time_ms,
                    SamplerMode::Off => {
                        (s.end_frame - s.start_frame) as f64 * 1000.0 / f64::from(rate)
                    }
                };
                let mut info = egui::WidgetInfo::drag_value(response.enabled(), value);
                info.label = Some(format!("Sample duration ({unit})"));
                info
            });
            (response, label)
        });
    let (response, label) = group.inner;
    rows::input_group_outline(
        ui,
        group.response.rect,
        label.rect.left(),
        &[&response, &label],
    );
    (group.response.rect, response.rect, label.rect)
}
/// The waveform stays in input frames; the ruler and snapping share the output clock.
struct PreviewClock {
    timeline: musical_time::Timeline,
    ratio: daw_core::ClipStretch,
    length: u64,
}
fn preview_clock(s: &SamplerSettings, width: f32, rate: u32, tempo: f32) -> PreviewClock {
    let selected = s.end_frame - s.start_frame;
    let ratio =
        daw_core::ClipStretch::new(selected, s.output_frames(rate, tempo).unwrap_or(selected))
            .unwrap();
    let length = s.input.as_ref().unwrap().length_frames;
    let zoom = (f64::from(width) * f64::from(rate) / (length as f64 * ratio.ratio())) as f32;
    PreviewClock {
        timeline: musical_time::Timeline::at_sample_rate(tempo, zoom, 0.0, rate),
        ratio,
        length,
    }
}
impl PreviewClock {
    fn output_frame(&self, input: u64) -> u64 {
        self.ratio.scale(input).unwrap_or(u64::MAX)
    }
    fn snap(&self, raw: i128, bounds: std::ops::RangeInclusive<u64>, bypass: bool) -> u64 {
        let frame = raw.clamp(i128::from(*bounds.start()), i128::from(*bounds.end())) as u64;
        if bypass || raw != i128::from(frame) {
            return frame;
        }
        let output = self.output_frame(frame);
        let snapped = self.timeline.snap(
            output,
            self.output_frame(*bounds.start())..=self.output_frame(*bounds.end()),
            [0, self.output_frame(self.length)],
        );
        if snapped == output {
            frame
        } else {
            self.ratio
                .original_frame(snapped)
                .clamp(*bounds.start(), *bounds.end())
        }
    }
}
fn selection(ui: &egui::Ui, bar: Rect, s: &mut SamplerSettings, rate: u32, tempo: f32) -> [u64; 2] {
    let length = s.input.as_ref().unwrap().length_frames;
    let id = ui.make_persistent_id("sample-selection");
    let response = ui.interact(bar, id, Sense::click_and_drag()).on_hover_text("Drag the selection edges or body. Drag outside to select a new region. Shift bypasses snapping.");
    let at = |x: f32| {
        (((x - bar.left()) / bar.width()).clamp(0.0, 1.0) as f64 * length as f64).round() as u64
    };
    let pixel = |f: u64| bar.left() + f as f32 / length as f32 * bar.width();
    let mut drag = ui.data_mut(|d| d.get_temp::<RangeDrag>(id));
    if drag.as_ref().is_some_and(|d| &d.original != s) || drag::cancelled(ui) || !ui.is_enabled() {
        drag = None;
    }
    if response.drag_started() && !drag::cancelled(ui) {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap();
        drag = Some(RangeDrag {
            original: s.clone(),
            origin: at(origin.x),
            mode: range_selection::mode(pixel(s.start_frame), pixel(s.end_frame), origin),
            preview: [s.start_frame, s.end_frame],
        });
    }
    let mut preview = [s.start_frame, s.end_frame];
    if let Some(d) = &mut drag {
        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
            let clock = preview_clock(&d.original, bar.width(), rate, tempo);
            let pos = at(pointer.x);
            d.preview = range_selection::preview(
                range_selection::Gesture {
                    mode: d.mode,
                    origin_frame: d.origin,
                    at_frame: pos,
                    delta_frames: i128::from(pos) - i128::from(d.origin),
                },
                [d.original.start_frame, d.original.end_frame],
                length,
                musical_time::snap_bypassed(ui),
                |raw, bounds, bypass| clock.snap(raw, bounds, bypass),
            );
        }
        preview = d.preview;
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            if preview[1] > preview[0] {
                s.start_frame = preview[0];
                s.end_frame = preview[1];
            }
            drag = None;
        }
    }
    ui.data_mut(|d| {
        if let Some(drag) = drag {
            d.insert_temp(id, drag);
        } else {
            d.remove::<RangeDrag>(id);
        }
    });
    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampler_preview_clock_follows_output_duration_and_snaps_back_to_input_frames() {
        for rate in daw_core::SAMPLE_RATES {
            for tempo in [60.0, 120.0, 180.0] {
                let mut settings = SamplerSettings {
                    input: Some(daw_core::SamplerInput {
                        asset_id: Id::new_v4(),
                        name: "Sample".into(),
                        source_offset_frame: 0,
                        length_frames: u64::from(rate) * 4,
                        repeat: None,
                        stretch: None,
                    }),
                    start_frame: u64::from(rate),
                    end_frame: u64::from(rate) * 3,
                    time_ms: 1000.0,
                    bars: 2,
                    ..Default::default()
                };
                for mode in [SamplerMode::Off, SamplerMode::Time, SamplerMode::Bars] {
                    settings.mode = mode;
                    let clock = preview_clock(&settings, 270.0, rate, tempo);
                    let target = settings.output_frames(rate, tempo).unwrap();
                    let start = clock.output_frame(settings.start_frame);
                    let end = clock.output_frame(settings.end_frame);
                    assert!(end.abs_diff(start + target) <= 1);
                    assert!((clock.timeline.pixel_at_frame(start) - 67.5).abs() < 0.01);
                    assert!((clock.timeline.pixel_at_frame(end) - 202.5).abs() < 0.01);
                    if mode == SamplerMode::Bars {
                        assert!(
                            (clock.timeline.beats(end) - clock.timeline.beats(start) - 8.0).abs()
                                < 0.001
                        );
                    }
                    let beat_frame = (f64::from(rate) * 60.0 / f64::from(tempo)).ceil() as u64;
                    let anchor = clock.ratio.original_frame(beat_frame);
                    let raw = anchor + u64::from(rate) / 1000;
                    let bounds = 0..=settings.input.as_ref().unwrap().length_frames;
                    assert!(
                        clock
                            .snap(i128::from(raw), bounds.clone(), false)
                            .abs_diff(anchor)
                            <= 1
                    );
                    assert_eq!(clock.snap(i128::from(raw), bounds.clone(), true), raw);
                    assert_eq!(clock.snap(-1, bounds.clone(), false), 0);
                    assert_eq!(
                        clock.snap(i128::from(*bounds.end()) + 1, bounds.clone(), false),
                        *bounds.end()
                    );
                }
            }
        }
    }
}
