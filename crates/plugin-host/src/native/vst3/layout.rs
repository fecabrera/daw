//! Negotiate bounded native layouts while the processor is inactive.
use super::*;
impl VstInstance {
    pub(super) fn negotiate_audio_layout(&self) -> Result<()> {
        let requested = |direction| -> Result<Vec<SpeakerArrangement>> {
            let count = unsafe { self.component.getBusCount(0, direction) };
            if !(0..=32).contains(&count) {
                return Err(Error::Invalid("VST3 audio bus count exceeds 32".into()));
            }
            let mut layouts = Vec::with_capacity(count as usize);
            for index in 0..count {
                let mut info: BusInfo = unsafe { std::mem::zeroed() };
                check(
                    unsafe { self.component.getBusInfo(0, direction, index, &mut info) },
                    "audio bus enumeration",
                )?;
                // Preserve native mono buses. Surround/default maximum layouts
                // must be offered the stereo format used by the DAW first.
                layouts.push(if info.channelCount == 1 {
                    SpeakerArr::kMono
                } else {
                    SpeakerArr::kStereo
                });
            }
            Ok(layouts)
        };
        let mut inputs = requested(0)?;
        let mut outputs = requested(1)?;
        let status = unsafe {
            self.processor.setBusArrangements(
                if inputs.is_empty() {
                    ptr::null_mut()
                } else {
                    inputs.as_mut_ptr()
                },
                inputs.len() as i32,
                if outputs.is_empty() {
                    ptr::null_mut()
                } else {
                    outputs.as_mut_ptr()
                },
                outputs.len() as i32,
            )
        };
        // A rejection can leave a supported alternative active. Re-enumerate
        // actual bus information in ports(); never size buffers from the request.
        if status != kResultFalse && status != kNotImplemented {
            check(status, "audio layout negotiation")?;
        }
        Ok(())
    }
}
