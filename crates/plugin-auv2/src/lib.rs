//! Native AUv2 C ABI generated from the locally installed Apple SDK.
#![cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[cfg(target_os = "macos")]
#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::all
)]
pub mod sys {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
#[cfg(target_os = "macos")]
pub mod boundary;
#[cfg(target_os = "macos")]
pub mod buffers;
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::sys::*;
    use std::mem::{align_of, offset_of, size_of};
    unsafe extern "C" {
        fn daw_au_abi(index: usize) -> usize;
        fn daw_au_c_render(
            p: *mut std::ffi::c_void,
            f: *mut u32,
            t: *const AudioTimeStamp,
            b: u32,
            n: u32,
            a: *mut AudioBufferList,
        ) -> i32;
    }
    #[test]
    fn c_and_rust_layouts_and_constants_agree() {
        let mut values = Vec::new();
        macro_rules! layout {($($t:ty),*)=>{$(values.extend([size_of::<$t>(),align_of::<$t>()]);)*};}
        layout!(
            AudioComponentDescription,
            AudioComponentPlugInInterface,
            AudioUnitParameterEvent,
            AudioBufferList,
            AudioBuffer,
            AudioTimeStamp,
            AudioStreamBasicDescription,
            HostCallbackInfo,
            AudioUnitCocoaViewInfo,
            AudioUnitParameterInfo,
            AURenderCallbackStruct,
            AudioUnitConnection,
            AudioUnitEvent,
            AUPreset,
            AudioUnitParameterStringFromValue,
            AudioUnitParameterValueFromString
        );
        values.extend([
            offset_of!(AudioBufferList, mBuffers),
            offset_of!(AudioUnitParameterEvent, eventValues),
            offset_of!(AudioUnitCocoaViewInfo, mCocoaAUViewClass),
            offset_of!(AudioComponentPlugInInterface, Lookup),
            kAudioUnitRenderSelect as usize,
            kAudioUnitProperty_StreamFormat as usize,
            kAudioFormatFlagIsNonInterleaved as usize,
            kAudioUnitParameterFlag_CanRamp as usize,
            kAudioUnitErr_CannotDoInCurrentContext as usize,
            size_of::<i32>(),
            1,
        ]);
        for (index, value) in values.into_iter().enumerate() {
            assert_eq!(value, unsafe { daw_au_abi(index) }, "ABI entry {index}");
        }
    }
    #[test]
    fn generated_callback_calls_c_and_handles_trailing_array() {
        let callback: AURenderCallback = std::hint::black_box(Some(daw_au_c_render));
        let mut left = [0.0; 17];
        let mut right = [0.0; 17];
        let mut list = super::buffers::StereoList::new(left.as_mut_ptr(), right.as_mut_ptr(), 17);
        let mut flags = 0;
        let stamp = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                callback.unwrap()(
                    std::ptr::null_mut(),
                    &mut flags,
                    &stamp,
                    0,
                    17,
                    list.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(left, [0.125; 17]);
        assert_eq!(right, [0.125; 17]);
    }
}

/// Reproducible local generation inventory. Contains no SDK source or machine paths.
#[cfg(target_os = "macos")]
pub const PROVENANCE: &str = include_str!(concat!(env!("OUT_DIR"), "/provenance.json"));
