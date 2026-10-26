//! Borrow queues directly. At most seven cursors exist; point counts are uncapped.
use daw_drive_plugin::Parameter;
use daw_plugin_vst3::sys::{
    ComRef,
    Steinberg::{Vst::*, kResultOk},
};
#[derive(Clone, Copy)]
pub(crate) struct Cursor<'a> {
    queue: ComRef<'a, IParamValueQueue>,
    count: i32,
    index: i32,
    previous: (i32, f64),
    next: Option<(i32, f64)>,
    frames: i32,
}
impl<'a> Cursor<'a> {
    pub unsafe fn new(
        queue: ComRef<'a, IParamValueQueue>,
        value: f64,
        frames: i32,
    ) -> Result<Self, ()> {
        let count = unsafe { queue.getPointCount() };
        if count < 0 {
            return Err(());
        }
        let mut cursor = Self {
            queue,
            count,
            index: 0,
            previous: (-1, value),
            next: None,
            frames,
        };
        cursor.next = unsafe { cursor.read_next() }?;
        Ok(cursor)
    }
    unsafe fn read_next(&mut self) -> Result<Option<(i32, f64)>, ()> {
        if self.index >= self.count {
            return Ok(None);
        }
        let (mut offset, mut value) = (0, 0.0);
        let status = unsafe { self.queue.getPoint(self.index, &mut offset, &mut value) };
        self.index += 1;
        if status != kResultOk
            || offset < 0
            || (self.frames > 0 && offset >= self.frames)
            || (self.frames == 0 && offset != 0)
            || !value.is_finite()
            || !(0.0..=1.0).contains(&value)
            || offset < self.previous.0
        {
            return Err(());
        }
        Ok(Some((offset, value)))
    }
    pub unsafe fn at(&mut self, frame: i32) -> Result<f64, ()> {
        while let Some(next) = self.next {
            if next.0 > frame {
                break;
            }
            self.previous = next;
            self.next = unsafe { self.read_next() }?;
        }
        if let Some(next) = self.next {
            if self.previous.1 == next.1 {
                return Ok(next.1);
            }
            let fraction = f64::from(frame - self.previous.0) / f64::from(next.0 - self.previous.0);
            Ok(self.previous.1 + (next.1 - self.previous.1) * fraction)
        } else {
            Ok(self.previous.1)
        }
    }
}
pub(crate) struct Queues<'a> {
    pub cursors: [Option<(Parameter, Cursor<'a>)>; daw_drive_plugin::Parameter::COUNT],
    pub count: usize,
}
impl<'a> Queues<'a> {
    pub unsafe fn new(
        input: *mut IParameterChanges,
        previous: [f64; daw_drive_plugin::Parameter::COUNT],
        frames: i32,
    ) -> Result<Self, ()> {
        let mut result = Self {
            cursors: [None; daw_drive_plugin::Parameter::COUNT],
            count: 0,
        };
        let Some(input) = (unsafe { ComRef::from_raw(input) }) else {
            return Ok(result);
        };
        let count = unsafe { input.getParameterCount() };
        if count < 0 {
            return Err(());
        }
        let mut seen = [false; daw_drive_plugin::Parameter::COUNT];
        for index in 0..count {
            let Some(queue) = (unsafe { ComRef::from_raw(input.getParameterData(index)) }) else {
                return Err(());
            };
            let Ok(parameter) = Parameter::from_id(unsafe { queue.getParameterId() }) else {
                continue;
            };
            let slot = Parameter::ALL
                .iter()
                .position(|p| *p == parameter)
                .ok_or(())?;
            if seen[slot] {
                return Err(());
            }
            seen[slot] = true;
            let cursor = unsafe { Cursor::new(queue, previous[slot], frames) }?;
            result.cursors[result.count] = Some((parameter, cursor));
            result.count += 1;
        }
        Ok(result)
    }
}
