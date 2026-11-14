use super::*;
use daw_drive_editor::{Edit, Editor, HEIGHT, WIDTH};
use daw_plugin_window::{Parent, Window};
unsafe extern "C" {
    fn daw_au_register_view(
        name: *const std::ffi::c_char,
        create: unsafe extern "C" fn(AudioUnit, *mut c_void) -> *mut c_void,
        retire: unsafe extern "C" fn(*mut c_void),
        tick: unsafe extern "C" fn(*mut c_void),
    );
    fn daw_au_bundle_url() -> CFURLRef;
    fn daw_au_view_size(view: *mut c_void, width: f64, height: f64);
}
pub(super) fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Ok(name) = std::ffi::CString::new(daw_drive_plugin::AU_COCOA_CLASS) {
            unsafe {
                daw_au_register_view(name.as_ptr(), create, retire, tick);
            }
        }
    });
}
pub(super) unsafe fn cocoa_info() -> Option<AudioUnitCocoaViewInfo> {
    let url = unsafe { daw_au_bundle_url() };
    if url.is_null() {
        return None;
    }
    Some(AudioUnitCocoaViewInfo {
        mCocoaAUViewBundleLocation: url,
        mCocoaAUViewClass: [unsafe { properties::string(daw_drive_plugin::AU_COCOA_CLASS) }],
    })
}
struct View {
    window: Option<Window>,
    shared: Arc<Shared>,
    model: Arc<daw_drive_editor::Model>,
    pending: Option<Edit>,
    seen_recall: u64,
    active: [bool; Parameter::COUNT],
}
unsafe extern "C" fn create(unit: AudioUnit, parent: *mut c_void) -> *mut c_void {
    boundary::call(std::ptr::null_mut(), || {
        let mut address = 0usize;
        let mut size = std::mem::size_of::<usize>() as u32;
        if unsafe {
            AudioUnitGetProperty(
                unit,
                PRIVATE_MODEL,
                kAudioUnitScope_Global,
                0,
                (&mut address as *mut usize).cast(),
                &mut size,
            )
        } != 0
            || address == 0
        {
            return std::ptr::null_mut();
        }
        let shared = unsafe { Arc::from_raw(address as *const Shared) };
        if shared.closed.load(Ordering::Acquire) {
            return std::ptr::null_mut();
        }
        let Some(parent_handle) = (unsafe { Parent::new(parent) }) else {
            return std::ptr::null_mut();
        };
        unsafe {
            daw_au_view_size(parent, f64::from(WIDTH), f64::from(HEIGHT));
        }
        let mut state = shared.exchange.current().unwrap_or_default();
        for p in Parameter::ALL {
            let _ = state.set(p, shared.parameter(p));
        }
        let mut model = daw_drive_editor::Model::default();
        model.output_meter = shared.model.output_meter.clone();
        model.copy_view_preferences(&shared.model);
        let model = Arc::new(model);
        model.publish(state);
        // View owns model and child; callbacks never borrow the AU instance.
        let app = Editor::new(model.clone(), || {});
        let Ok(window) = daw_plugin_window::create(
            app,
            &parent_handle,
            f64::from(WIDTH),
            f64::from(HEIGHT),
            1.0,
            daw_plugin_window::host(|| {}),
        ) else {
            return std::ptr::null_mut();
        };
        if window.show().is_err() {
            return std::ptr::null_mut();
        }
        let seen_recall = shared.recall_revision.load(Ordering::Acquire);
        Box::into_raw(Box::new(View {
            window: Some(window),
            shared,
            model,
            pending: None,
            seen_recall,
            active: [false; Parameter::COUNT],
        }))
        .cast()
    })
}
// Caller holds host_gate through every native call, including reentrant host code.
unsafe fn notify(shared: &Shared, p: Parameter, kind: u32) {
    let unit = shared.handle.load(Ordering::Acquire) as AudioUnit;
    if unit.is_null() || shared.closed.load(Ordering::Acquire) {
        return;
    }
    let mut event: AudioUnitEvent = unsafe { std::mem::zeroed() };
    event.mEventType = kind;
    event.mArgument.mParameter = AudioUnitParameter {
        mAudioUnit: unit,
        mParameterID: p as u32,
        mScope: kAudioUnitScope_Global,
        mElement: 0,
    };
    unsafe {
        AUEventListenerNotify(std::ptr::null_mut(), std::ptr::null_mut(), &event);
    }
}
unsafe extern "C" fn tick(pointer: *mut c_void) {
    boundary::call((), || {
        let Some(view) = (unsafe { pointer.cast::<View>().as_mut() }) else {
            return;
        };
        let shared = view.shared.clone();
        let Some(_host_lease) = token(&shared.host_gate) else {
            return;
        };
        if view.shared.closed.load(Ordering::Acquire) {
            view.window.take();
            return;
        }
        let revision = view.shared.recall_revision.load(Ordering::Acquire);
        if revision != view.seen_recall {
            view.model.discard_presentation();
            if matches!(view.pending, Some(Edit::Presentation(_))) {
                view.pending = None;
            }
            view.seen_recall = revision;
        }
        while let Some(edit) = view.pending.take().or_else(|| view.model.pop()) {
            match edit {
                Edit::Begin(p) => {
                    if !view.active[index(p)] {
                        view.active[index(p)] = true;
                        if view.shared.gestures[index(p)].fetch_add(1, Ordering::AcqRel) == 0 {
                            unsafe {
                                notify(
                                    &view.shared,
                                    p,
                                    kAudioUnitEvent_BeginParameterChangeGesture,
                                );
                            }
                        }
                    }
                }
                Edit::End(p) => {
                    if view.active[index(p)] {
                        view.active[index(p)] = false;
                        end_gesture(&view.shared, p);
                    }
                }
                Edit::Value(p, v) => {
                    let result = view.shared.set(p, v);
                    if result == BUSY {
                        view.pending = Some(edit);
                        break;
                    }
                    if result != 0 {
                        continue;
                    }
                    unsafe {
                        notify(&view.shared, p, kAudioUnitEvent_ParameterValueChange);
                    }
                }
                Edit::Presentation(order) => {
                    let Some(guard) = token(&view.shared.control) else {
                        view.pending = Some(edit);
                        break;
                    };
                    let Some(mut state) = view.shared.exchange.current() else {
                        view.pending = Some(edit);
                        break;
                    };
                    for p in Parameter::ALL {
                        let _ = state.set(p, view.shared.parameter(p));
                    }
                    let changed = state.set_presentation(order);
                    if changed {
                        view.shared.exchange.recall(state);
                    }
                    drop(guard);
                    if changed {
                        unsafe {
                            let mut event: AudioUnitEvent = std::mem::zeroed();
                            event.mEventType = kAudioUnitEvent_PropertyChange;
                            event.mArgument.mProperty = AudioUnitProperty {
                                mAudioUnit: view.shared.handle.load(Ordering::Acquire) as AudioUnit,
                                mPropertyID: kAudioUnitProperty_ClassInfo,
                                mScope: kAudioUnitScope_Global,
                                mElement: 0,
                            };
                            AUEventListenerNotify(
                                std::ptr::null_mut(),
                                std::ptr::null_mut(),
                                &event,
                            );
                        }
                    }
                }
            }
        }
        view.shared.model.meter_reset.fetch_or(
            view.model.meter_reset.swap(0, Ordering::AcqRel),
            Ordering::Release,
        );
        if view.model.output_reset.swap(false, Ordering::AcqRel) {
            view.shared
                .model
                .output_reset
                .store(true, Ordering::Release);
        }
        if let Some(mut state) = view.shared.exchange.current() {
            for p in Parameter::ALL {
                let _ = state.set(p, view.shared.parameter(p));
            }
            view.model.publish(state);
        }
        if let Some(window) = &view.window {
            window.host_main_thread_callback();
        }
        flush_pending_ends(&shared);
    });
}
unsafe extern "C" fn retire(pointer: *mut c_void) {
    boundary::call((), || {
        if pointer.is_null() {
            return;
        }
        let mut view = unsafe { Box::from_raw(pointer.cast::<View>()) };
        view.shared.model.copy_view_preferences(&view.model);
        // Child retirement ends gestures before the parent NSView deallocates.
        view.window.take();
        use daw_drive_plugin::BandAudition;
        for flag in view.shared.model.output_meter.band_solos() {
            flag.store(false, Ordering::Relaxed);
        }
        let shared = view.shared.clone();
        let Some(_host_lease) = token(&shared.host_gate) else {
            for (slot, active) in view.active.iter().enumerate() {
                if *active {
                    shared.pending_ends[slot].fetch_add(1, Ordering::Release);
                }
            }
            return;
        };
        for (slot, p) in Parameter::ALL.into_iter().enumerate() {
            if view.active[slot] {
                end_gesture(&view.shared, p);
            }
        }
        flush_pending_ends(&shared);
    });
}

pub(super) fn finish_gestures(shared: &Shared) {
    for (slot, p) in Parameter::ALL.into_iter().enumerate() {
        if shared.gestures[slot].swap(0, Ordering::AcqRel) != 0 {
            unsafe {
                notify(shared, p, kAudioUnitEvent_EndParameterChangeGesture);
            }
        }
    }
}

fn end_gesture(shared: &Shared, p: Parameter) {
    let count = shared.gestures[index(p)].load(Ordering::Acquire);
    if count != 0
        && shared.gestures[index(p)]
            .compare_exchange(count, count - 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        && count == 1
    {
        unsafe {
            notify(shared, p, kAudioUnitEvent_EndParameterChangeGesture);
        }
    }
}
/// Called with host_gate held on a control/UI thread. Never from rendering.
pub(super) fn flush_pending_ends(shared: &Shared) {
    for (slot, p) in Parameter::ALL.into_iter().enumerate() {
        let count = shared.pending_ends[slot].swap(0, Ordering::AcqRel);
        for _ in 0..count {
            end_gesture(shared, p);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn views_have_independent_queues_and_retain_only_an_inert_model_after_close() {
        let shared = Arc::new(Shared::new());
        let first = Arc::new(daw_drive_editor::Model::default());
        let second = Arc::new(daw_drive_editor::Model::default());
        first.appearance.store(2, Ordering::Relaxed);
        shared.model.copy_view_preferences(&first);
        second.copy_view_preferences(&shared.model);
        assert_eq!(second.appearance.load(Ordering::Relaxed), 2);
        assert!(first.push(&[Edit::Begin(Parameter::Bypass)]));
        assert!(second.pop().is_none());
        let mut view = Box::new(View {
            window: None,
            shared: shared.clone(),
            model: second,
            pending: None,
            seen_recall: 0,
            active: [false; Parameter::COUNT],
        });
        shared.closed.store(true, Ordering::Release);
        unsafe {
            tick((&mut *view as *mut View).cast());
        }
        assert!(view.window.is_none());
    }
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    #[test]
    fn reentrant_view_retirement_defers_gesture_ends_until_the_host_call_returns() {
        let shared = Arc::new(Shared::new());
        shared.gestures[0].store(1, Ordering::Release);
        let mut active = [false; Parameter::COUNT];
        active[0] = true;
        let view = Box::new(View {
            window: None,
            shared: shared.clone(),
            model: Arc::default(),
            pending: None,
            seen_recall: 0,
            active,
        });
        let _lease = token(&shared.host_gate).unwrap();
        unsafe {
            retire(Box::into_raw(view).cast());
        }
        assert_eq!(shared.pending_ends[0].load(Ordering::Acquire), 1);
        flush_pending_ends(&shared);
        assert_eq!(shared.pending_ends[0].load(Ordering::Acquire), 0);
        assert_eq!(shared.gestures[0].load(Ordering::Acquire), 0);
    }
}
