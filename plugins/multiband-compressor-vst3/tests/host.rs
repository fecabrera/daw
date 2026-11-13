#![allow(
    non_snake_case,
    dead_code,
    unused_imports,
    clippy::undocumented_unsafe_blocks
)]
struct Drive(daw_multiband_compressor::MultibandCompressor);
impl Drive {
    fn new(c: daw_unit_core::ProcessingContext, s: &daw_drive_plugin::Settings) -> Self {
        let present: [_; 64] =
            std::array::from_fn(|i| s.bands.iter().any(|b| b.id == s.slots[i].id));
        Self(
            daw_multiband_compressor::MultibandCompressor::prepare_slots(
                c, &s.slots, &present, s.enabled, s.level_db, s.dry_wet,
            ),
        )
    }
    fn retarget(&mut self, s: &daw_drive_plugin::Settings) {
        for i in 0..64 {
            self.0.retarget_slot(
                i,
                &s.slots[i],
                s.bands.iter().any(|b| b.id == s.slots[i].id),
            );
        }
        self.0.retarget_output(s.enabled, s.level_db, s.dry_wet);
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        daw_unit_core::EffectUnit::process_sample(&mut self.0, input)
    }
    fn reset(&mut self) {
        daw_unit_core::EffectUnit::reset(&mut self.0);
    }
    fn latency_frames(&self) -> u32 {
        0
    }
}
use daw_multiband_compressor_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;

mod spectrum_wire {
    use super::*;
    use daw_plugin_vst3::sys::ComRef;
    use std::ffi::{CStr, c_char};
    struct Request(&'static CStr);
    impl Class for Request {
        type Interfaces = (IMessage,);
    }
    impl IMessageTrait for Request {
        unsafe fn getMessageID(&self) -> *const c_char {
            self.0.as_ptr()
        }
        unsafe fn setMessageID(&self, _: *const c_char) {}
        unsafe fn getAttributes(&self) -> *mut IAttributeList {
            ptr::null_mut()
        }
    }
    struct Receiver {
        controller: ComPtr<IConnectionPoint>,
        frame: RefCell<Vec<f32>>,
    }
    impl Class for Receiver {
        type Interfaces = (IConnectionPoint,);
    }
    impl IConnectionPointTrait for Receiver {
        unsafe fn connect(&self, _: *mut IConnectionPoint) -> tresult {
            kResultOk
        }
        unsafe fn disconnect(&self, _: *mut IConnectionPoint) -> tresult {
            kResultOk
        }
        unsafe fn notify(&self, message: *mut IMessage) -> tresult {
            let m = unsafe { ComRef::from_raw(message) }.unwrap();
            let a = unsafe { ComRef::from_raw(m.getAttributes()) }.unwrap();
            let mut count = 0;
            assert_eq!(
                unsafe { a.getInt(c"value-count".as_ptr(), &mut count) },
                kResultOk
            );
            let mut frame = Vec::new();
            for i in 0..count {
                let key = match i {
                    0 => std::ffi::CString::new("current").unwrap(),
                    1 => std::ffi::CString::new("peak").unwrap(),
                    _ => std::ffi::CString::new(format!("meter.{i}")).unwrap(),
                };
                let mut value = 0.0;
                assert_eq!(unsafe { a.getFloat(key.as_ptr(), &mut value) }, kResultOk);
                frame.push(value as f32);
            }
            self.frame.replace(frame);
            unsafe { self.controller.notify(message) }
        }
    }
    #[test]
    fn actual_factory_transports_full_range_spectrum_and_masks_legacy_and_stopped_frames() {
        let _serial = SERIAL.lock().unwrap();
        let mut plugin = Plugin::new();
        let component = plugin.component.cast::<IConnectionPoint>().unwrap();
        let receiver = ComWrapper::new(Receiver {
            controller: plugin.controller.cast::<IConnectionPoint>().unwrap(),
            frame: RefCell::default(),
        });
        let peer = receiver.to_com_ptr::<IConnectionPoint>().unwrap();
        assert_eq!(unsafe { component.connect(peer.as_ptr()) }, kResultOk);
        for rate in SUPPORTED_RATES {
            plugin.activate(rate, 512, 0);
            let mut frames = 0;
            while frames < rate as usize {
                let length = 512.min(rate as usize - frames);
                let mut left: Vec<_> = (frames..frames + length)
                    .map(|i| {
                        (std::f64::consts::TAU * 80.0 * i as f64 / f64::from(rate)).sin() as f32
                            * 0.5
                    })
                    .collect();
                let mut right: Vec<_> = left.iter().map(|v| -*v).collect();
                run(&plugin, &mut left, &mut right, &[], 0, false, 0);
                frames += length;
            }
            let request = ComWrapper::new(Request(c"daw.output-meter.request.v2"))
                .to_com_ptr::<IMessage>()
                .unwrap();
            assert_eq!(unsafe { component.notify(request.as_ptr()) }, kResultOk);
            let frame = receiver.frame.borrow();
            assert_eq!(frame.len(), 796);
            assert_eq!(frame[269], rate as f32);
            assert_eq!(frame[782], 2.0);
            assert_eq!(frame[783], 1.0);
            assert!(frame[270..782].iter().any(|v| *v > 0.01));
            let output = daw_drive_plugin::OutputMeter::default();
            daw_drive_plugin::receive_meter(&output, frame.as_slice().try_into().unwrap());
            let r = output.spectrum.reading();
            assert!(r.upper_end_frame <= frames as u64);
            assert!((frames as u64 - r.upper_end_frame) as f64 / f64::from(rate) < 0.05);
            assert_eq!(r.bass_end_frame, 0);
            drop(frame);
            let legacy = ComWrapper::new(Request(c"daw.output-meter.request.v1"))
                .to_com_ptr::<IMessage>()
                .unwrap();
            assert_eq!(unsafe { component.notify(legacy.as_ptr()) }, kResultOk);
            let frame = receiver.frame.borrow();
            assert_eq!(frame.len(), 269);
            assert!(frame[205..269].iter().any(|v| *v > 0.0));
            drop(frame);
            plugin.stop();
            assert_eq!(unsafe { component.notify(request.as_ptr()) }, kResultOk);
            let frame = receiver.frame.borrow();
            assert_eq!(frame[783], 0.0);
            assert!(frame[270..782].iter().all(|v| *v == 0.0));
            drop(frame);
            plugin.deactivate();
        }
        assert_eq!(unsafe { component.disconnect(peer.as_ptr()) }, kResultOk);
    }
}
