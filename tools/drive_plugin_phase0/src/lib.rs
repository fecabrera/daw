//! Disposable stereo gain probe. It is not Drive or a distributable product.
//! Stable public product identities and the real editor belong to later phases.
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize, prelude::*};
use nice_plug_egui::{
    EguiEditor, EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp, RepaintNotifier,
    create_egui_editor,
};
use std::sync::Arc;

#[derive(Params)]
struct Parameters {
    #[id = "probe-gain"]
    gain: FloatParam,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            // No smoothing: the host probe must observe the exact event sample.
            gain: FloatParam::new("Probe gain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
        }
    }
}
struct Editor {
    params: Arc<Parameters>,
    context: Option<GuiContext>,
}
impl NiceEguiApp for Editor {
    fn build(
        &mut self,
        _context: egui::Context,
        host: GuiContext,
        _frame: &mut Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        self.context = Some(host);
        Ok(())
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        ui.heading("Phase 0 feasibility probe");
        ui.label("This is not the Drive editor.");
        if let Some(context) = &self.context {
            ui.add(nice_plug_egui::widgets::ParamSlider::for_param(
                &self.params.gain,
                &context.param_setter(),
            ));
        }
    }
    fn editor_closed(&mut self) {
        self.context = None;
    }
}
struct Probe {
    params: Arc<Parameters>,
    editor_state: Arc<EguiEditorState>,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            params: Arc::new(Parameters::default()),
            editor_state: EguiEditorState::from_size(LogicalSize::new(360.0, 120.0), 1.0),
        }
    }
}
impl Plugin for Probe {
    const NAME: &'static str = "DAW Phase 0 Probe (not Drive)";
    const VENDOR: &'static str = "DAW feasibility tools";
    const URL: &'static str = "https://example.invalid/phase0";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type Editor = EguiEditor<Editor>;
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        create_egui_editor(
            self.editor_state.clone(),
            RepaintNotifier::new(),
            EguiNiceSettings::new(),
            Editor {
                params: self.params.clone(),
                context: None,
            },
        )
    }
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let gain = self.params.gain.value();
        for mut frame in buffer.iter_samples() {
            for sample in frame.iter_mut() {
                *sample *= gain;
            }
        }
        ProcessStatus::Normal
    }
}
impl ClapPlugin for Probe {
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Disposable Phase 0 stereo pass-through probe");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_ID: &'static str = "invalid.example.daw.phase0-probe";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}
impl Vst3Plugin for Probe {
    const VST3_CLASS_ID: [u8; 16] = *b"DAWPhaseZeroOnly";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
nice_export_clap!(Probe);
nice_export_vst3!(Probe);

// The probe counts allocations inside the loaded plugin, including framework code.
// Host queues and GUI work are deliberately outside the measured audio-thread region.
mod allocation_probe {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering},
    };
    thread_local! { static TRACK: Cell<bool> = const { Cell::new(false) }; }
    static ALLOCATED: AtomicU64 = AtomicU64::new(0);
    static DEALLOCATED: AtomicU64 = AtomicU64::new(0);
    struct Counter;
    fn count(counter: &AtomicU64) {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
    // Safety: every method forwards the identical layout and pointer to System.
    // The counters use no allocation or blocking synchronization.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count(&ALLOCATED);
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            count(&DEALLOCATED);
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            count(&ALLOCATED);
            count(&DEALLOCATED);
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: Counter = Counter;
    #[repr(C)]
    pub struct Counts {
        allocated: u64,
        deallocated: u64,
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn phase0_begin_allocations() {
        ALLOCATED.store(0, Ordering::Relaxed);
        DEALLOCATED.store(0, Ordering::Relaxed);
        TRACK.with(|track| track.set(true));
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn phase0_end_allocations() -> Counts {
        TRACK.with(|track| track.set(false));
        Counts {
            allocated: ALLOCATED.load(Ordering::Relaxed),
            deallocated: DEALLOCATED.load(Ordering::Relaxed),
        }
    }
}
