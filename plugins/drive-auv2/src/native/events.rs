use super::*;
const CAPACITY: usize = 65536;
#[derive(Clone, Copy)]
pub(super) struct Event {
    pub parameter: Parameter,
    pub start: i64,
    pub duration: u32,
    pub from: f64,
    pub to: f64,
    pub order: usize,
}
/// Independent bounded producer storage. Pre-render notifications may schedule
/// without borrowing the processor. One CAS, never a mutex or a retry loop.
pub(super) struct Events {
    gate: AtomicBool,
    values: UnsafeCell<Vec<Event>>,
}
impl Events {
    pub fn new() -> Self {
        Self {
            gate: AtomicBool::new(false),
            values: UnsafeCell::new(Vec::with_capacity(CAPACITY)),
        }
    }
    pub fn clear(&self) {
        if let Some(_g) = token(&self.gate) {
            unsafe {
                (*self.values.get()).clear();
            }
        }
    }
    pub fn push_immediate(&self, parameter: Parameter, value: f64, offset: u32) -> OSStatus {
        let Some(_g) = token(&self.gate) else {
            return BUSY;
        };
        let values = unsafe { &mut *self.values.get() };
        if values.len() == CAPACITY {
            return BUSY;
        }
        values.push(Event {
            parameter,
            start: i64::from(offset),
            duration: 0,
            from: value,
            to: value,
            order: values.len(),
        });
        0
    }
    pub fn take(&self, target: &mut Vec<Event>) -> bool {
        let Some(_g) = token(&self.gate) else {
            return false;
        };
        let values = unsafe { &mut *self.values.get() };
        target.clear();
        target.extend_from_slice(values);
        values.clear();
        // Unstable sort allocates no scratch. Explicit call order makes ties stable.
        target.sort_unstable_by_key(|e| (e.start, e.order));
        true
    }
}
/// AU parameters are f32. Clamp only rounding at an endpoint, not invalid input.
pub(super) fn plain(p: Parameter, v: f32) -> Option<f64> {
    if !v.is_finite() {
        return None;
    }
    let info = p.info();
    if v < info.minimum as f32 || v > info.maximum as f32 {
        return None;
    }
    let value = f64::from(v).clamp(info.minimum, info.maximum);
    // AU indexed/boolean controls are transported as floats. Hosts and auval
    // may send intermediate values; quantize at the boundary, retaining the
    // product contract's strict integer validation.
    let value = if info.steps > 0 {
        value.round().clamp(info.minimum, info.maximum)
    } else {
        value
    };
    p.validate(value).ok()
}
unsafe fn decode(e: AudioUnitParameterEvent) -> Result<Event, OSStatus> {
    if e.scope != kAudioUnitScope_Global || e.element != 0 {
        return Err(kAudioUnitErr_InvalidScope);
    }
    let p = Parameter::from_id(e.parameter).map_err(|_| kAudioUnitErr_InvalidParameter)?;
    if e.eventType == kParameterEvent_Immediate {
        let v = unsafe { e.eventValues.immediate };
        let value = plain(p, v.value).ok_or(kAudioUnitErr_InvalidParameterValue)?;
        Ok(Event {
            parameter: p,
            start: i64::from(v.bufferOffset),
            duration: 0,
            from: value,
            to: value,
            order: 0,
        })
    } else if e.eventType == kParameterEvent_Ramped && p.info().steps == 0 {
        let v = unsafe { e.eventValues.ramp };
        Ok(Event {
            parameter: p,
            start: i64::from(v.startBufferOffset),
            duration: v.durationInFrames,
            from: plain(p, v.startValue).ok_or(kAudioUnitErr_InvalidParameterValue)?,
            to: plain(p, v.endValue).ok_or(kAudioUnitErr_InvalidParameterValue)?,
            order: 0,
        })
    } else {
        Err(kAudioUnitErr_InvalidParameterValue)
    }
}
pub(super) unsafe extern "C" fn schedule(
    pointer: *mut c_void,
    input: *const AudioUnitParameterEvent,
    count: u32,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(pointer) }) else {
            return BAD;
        };
        if i.phase.load(Ordering::Acquire) >= 2 {
            return BUSY;
        }
        if count as usize > CAPACITY || (input.is_null() && count != 0) {
            return BAD;
        }
        let Some(_g) = token(&i.events.gate) else {
            return BUSY;
        };
        let values = unsafe { &mut *i.events.values.get() };
        if count as usize > CAPACITY - values.len() {
            return BUSY;
        }
        // Complete validation before the first write. Read packed/untrusted C data by value.
        for n in 0..count as usize {
            if let Err(error) = unsafe { decode(input.add(n).read_unaligned()) } {
                return error;
            }
        }
        for n in 0..count as usize {
            if let Ok(mut event) = unsafe { decode(input.add(n).read_unaligned()) } {
                event.order = values.len();
                values.push(event);
            }
        }
        0
    })
}
pub(super) fn storage() -> Vec<Event> {
    Vec::with_capacity(CAPACITY)
}
