use super::*;
use std::cell::UnsafeCell;
#[derive(Clone, Copy, Default)]
struct Point {
    offset: i32,
    value: f64,
}
struct ChangeData {
    points: Vec<Point>,
    heads: Vec<usize>,
    counts: Vec<usize>,
    write: Vec<usize>,
    active: Vec<usize>,
}
struct Queue {
    slot: usize,
    id: u32,
    data: *const ChangeData,
    output: *const Shared,
}
impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> u32 {
        self.id
    }
    unsafe fn getPointCount(&self) -> i32 {
        unsafe { &*self.data }.counts[self.slot] as i32
    }
    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        if offset.is_null() || value.is_null() || index < 0 {
            return kInvalidArgument;
        }
        let data = unsafe { &*self.data };
        if index as usize >= data.counts[self.slot] {
            return kResultFalse;
        }
        let p = data.points[data.heads[self.slot] + index as usize];
        unsafe {
            offset.write(p.offset);
            value.write(p.value);
        }
        kResultOk
    }
    unsafe fn addPoint(&self, offset: i32, value: f64, index: *mut i32) -> tresult {
        if self.output.is_null()
            || offset < 0
            || !value.is_finite()
            || !(0.0..=1.0).contains(&value)
        {
            return kInvalidArgument;
        }
        // The plugin borrows this fixed output bank exclusively during process.
        let data = unsafe { &mut *self.data.cast_mut() };
        let first = data.heads[self.slot];
        let count = data.counts[self.slot];
        let limit = MAX_HOSTED_EVENTS / data.counts.len().max(1);
        let points = &mut data.points[first..first + limit];
        let at = points[..count].partition_point(|p| p.offset < offset);
        if at < count && points[at].offset == offset {
            points[at].value = value;
        } else {
            if count == limit {
                return kResultFalse;
            }
            points.copy_within(at..count, at + 1);
            points[at] = Point { offset, value };
            data.counts[self.slot] += 1;
        }
        unsafe { &*self.output }.emit(NativeNotification::Telemetry(self.id, value));
        if !index.is_null() {
            unsafe {
                index.write(at as i32);
            }
        }
        kResultOk
    }
}
struct Changes {
    queues: Vec<ComPtr<IParamValueQueue>>,
    ids: Vec<u32>,
    data: *const ChangeData,
    output: bool,
}
impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        unsafe { &*self.data }.active.len() as i32
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index < 0 {
            return ptr::null_mut();
        }
        unsafe { &*self.data }
            .active
            .get(index as usize)
            .map_or(ptr::null_mut(), |slot| self.queues[*slot].as_ptr())
    }
    unsafe fn addParameterData(&self, id: *const u32, index: *mut i32) -> *mut IParamValueQueue {
        if !self.output || id.is_null() {
            return ptr::null_mut();
        }
        let id = unsafe { *id };
        match self.ids.binary_search(&id) {
            Ok(n) => {
                let data = unsafe { &mut *self.data.cast_mut() };
                let at = data
                    .active
                    .iter()
                    .position(|slot| *slot == n)
                    .unwrap_or_else(|| {
                        let at = data.active.len();
                        data.active.push(n);
                        at
                    });
                if !index.is_null() {
                    unsafe {
                        index.write(at as i32);
                    }
                }
                self.queues[n].as_ptr()
            }
            Err(_) => ptr::null_mut(),
        }
    }
}
pub(super) struct ParameterChanges {
    data: Box<UnsafeCell<ChangeData>>,
    ids: Vec<u32>,
    output_data: Box<UnsafeCell<ChangeData>>,
    pub input: ComPtr<IParameterChanges>,
    pub output: ComPtr<IParameterChanges>,
}
impl ParameterChanges {
    pub fn new(parameters: &[HostedParameter], shared: &Shared) -> Self {
        let mut ids: Vec<_> = parameters.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        let data = Box::new(UnsafeCell::new(ChangeData {
            points: Vec::with_capacity(MAX_HOSTED_EVENTS),
            heads: vec![0; n],
            counts: vec![0; n],
            write: vec![0; n],
            active: Vec::with_capacity(n),
        }));
        let output_data = Box::new(UnsafeCell::new(ChangeData {
            points: vec![Point::default(); MAX_HOSTED_EVENTS],
            heads: (0..n)
                .map(|slot| slot * (MAX_HOSTED_EVENTS / n.max(1)))
                .collect(),
            counts: vec![0; n],
            write: vec![0; n],
            active: Vec::with_capacity(n),
        }));
        let make = |output: bool| {
            let storage = if output {
                output_data.get()
            } else {
                data.get()
            };
            let queues = ids
                .iter()
                .enumerate()
                .map(|(slot, id)| {
                    ComWrapper::new(Queue {
                        slot,
                        id: *id,
                        data: storage,
                        output: if output { shared } else { ptr::null() },
                    })
                    .to_com_ptr::<IParamValueQueue>()
                    .expect("declared host queue")
                })
                .collect();
            ComWrapper::new(Changes {
                queues,
                ids: ids.clone(),
                data: storage,
                output,
            })
            .to_com_ptr::<IParameterChanges>()
            .expect("declared parameter changes")
        };
        let input = make(false);
        let output = make(true);
        Self {
            data,
            output_data,
            ids,
            input,
            output,
        }
    }
    pub fn prepare(&mut self, events: &[NativeEvent]) -> bool {
        // All SDK readers borrow immutable data only during process(). This unique
        // audio owner writes the preallocated storage before entering the plugin.
        let output = unsafe { &mut *self.output_data.get() };
        output.counts.fill(0);
        output.active.clear();
        let data = unsafe { &mut *self.data.get() };
        data.counts.fill(0);
        data.active.clear();
        for event in events {
            match event {
                NativeEvent::Parameter { id, value, .. } => {
                    let Ok(slot) = self.ids.binary_search(id) else {
                        return false;
                    };
                    if !(0.0..=1.0).contains(value) {
                        return false;
                    }
                    data.counts[slot] += 1;
                }
                NativeEvent::Modulation { .. } => return false,
                _ => {}
            }
        }
        let mut total = 0;
        for slot in 0..self.ids.len() {
            data.heads[slot] = total;
            data.write[slot] = total;
            total += data.counts[slot];
            if data.counts[slot] != 0 {
                data.active.push(slot);
            }
        }
        if total > data.points.capacity() {
            return false;
        }
        data.points.resize(total, Point::default());
        for event in events {
            if let NativeEvent::Parameter { offset, id, value } = *event {
                let Ok(slot) = self.ids.binary_search(&id) else {
                    return false;
                };
                let at = data.write[slot];
                data.points[at] = Point {
                    offset: offset as i32,
                    value,
                };
                data.write[slot] += 1;
            }
        }
        true
    }
}
pub(super) struct Events {
    data: Box<UnsafeCell<Vec<Event>>>,
    pub pointer: ComPtr<IEventList>,
}
struct EventList(*const Vec<Event>);
impl Class for EventList {
    type Interfaces = (IEventList,);
}
impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> i32 {
        unsafe { &*self.0 }.len() as i32
    }
    unsafe fn getEvent(&self, index: i32, out: *mut Event) -> tresult {
        if index < 0 || out.is_null() {
            return kInvalidArgument;
        }
        if let Some(event) = unsafe { &*self.0 }.get(index as usize) {
            unsafe {
                out.write(*event);
            }
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn addEvent(&self, _: *mut Event) -> tresult {
        kNotImplemented
    }
}
impl Events {
    pub fn new() -> Self {
        let data = Box::new(UnsafeCell::new(Vec::with_capacity(MAX_HOSTED_EVENTS)));
        let pointer = ComWrapper::new(EventList(data.get()))
            .to_com_ptr::<IEventList>()
            .expect("declared host event list");
        Self { data, pointer }
    }
    pub fn prepare(&mut self, events: &[NativeEvent], notes: bool) -> bool {
        let data = unsafe { &mut *self.data.get() };
        data.clear();
        for event in events {
            let note = match *event {
                NativeEvent::Note {
                    offset,
                    on,
                    id,
                    channel,
                    key,
                    velocity,
                } => Some((offset, on, id, channel, key, velocity)),
                NativeEvent::Midi { offset, data } if matches!(data[0] & 0xf0, 0x80 | 0x90) => {
                    Some((
                        offset,
                        data[0] & 0xf0 == 0x90 && data[2] != 0,
                        -1,
                        i16::from(data[0] & 15),
                        i16::from(data[1]),
                        f64::from(data[2]) / 127.0,
                    ))
                }
                NativeEvent::Midi {
                    offset,
                    data: bytes,
                } if bytes[0] & 0xf0 == 0xa0 => {
                    if !notes || data.len() == data.capacity() {
                        return false;
                    }
                    let mut e: Event = unsafe { std::mem::zeroed() };
                    e.busIndex = 0;
                    e.sampleOffset = offset as i32;
                    e.r#type = 3;
                    e.__field0 = Event__type0 {
                        polyPressure: PolyPressureEvent {
                            channel: i16::from(bytes[0] & 15),
                            pitch: i16::from(bytes[1] & 127),
                            pressure: f32::from(bytes[2] & 127) / 127.0,
                            noteId: -1,
                        },
                    };
                    data.push(e);
                    None
                }
                NativeEvent::Midi { .. } => None,
                _ => None,
            };
            if let Some((offset, on, id, channel, key, velocity)) = note {
                if !notes || data.len() == data.capacity() {
                    return false;
                }
                let mut e: Event = unsafe { std::mem::zeroed() };
                e.busIndex = 0;
                e.sampleOffset = offset as i32;
                e.r#type = if on { 0 } else { 1 };
                e.__field0 = if on {
                    Event__type0 {
                        noteOn: NoteOnEvent {
                            channel,
                            pitch: key,
                            tuning: 0.0,
                            velocity: velocity as f32,
                            length: 0,
                            noteId: id,
                        },
                    }
                } else {
                    Event__type0 {
                        noteOff: NoteOffEvent {
                            channel,
                            pitch: key,
                            velocity: velocity as f32,
                            noteId: id,
                            tuning: 0.0,
                        },
                    }
                };
                data.push(e);
            }
        }
        true
    }
}
