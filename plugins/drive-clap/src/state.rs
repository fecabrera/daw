use crate::instance::instance;
use daw_drive_plugin::PluginState;
use daw_plugin_clap::{
    boundary,
    sys::{
        ext::{params::CLAP_PARAM_RESCAN_VALUES, state::*},
        plugin::*,
        stream::*,
    },
};
use std::{
    io::{self, Read, Write},
    sync::atomic::Ordering,
};
struct Input(*const clap_istream);
impl Read for Input {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // SAFETY: save/load own the host stream through this control-thread call.
        let read = unsafe { (*self.0).read }.ok_or(io::ErrorKind::Unsupported)?;
        let count = unsafe { read(self.0, buffer.as_mut_ptr().cast(), buffer.len() as u64) };
        if count < 0 || count as u64 > buffer.len() as u64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(count as usize)
    }
}
struct Output(*const clap_ostream);
impl Write for Output {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let write = unsafe { (*self.0).write }.ok_or(io::ErrorKind::Unsupported)?;
        let count = unsafe { write(self.0, buffer.as_ptr().cast(), buffer.len() as u64) };
        if count < 0 || count as u64 > buffer.len() as u64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(count as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
unsafe extern "C" fn save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    boundary(false, || {
        if stream.is_null() {
            return false;
        }
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        let Some(state) = i.current() else {
            return false;
        };
        state.write_to(&mut Output(stream)).is_ok()
    })
}
unsafe extern "C" fn load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    boundary(false, || {
        if stream.is_null() {
            return false;
        }
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        let Ok(state) = PluginState::read_from(&mut Input(stream)) else {
            return false;
        };
        // Main-thread loads coalesce in a fixed mailbox. Inactive publication is
        // immediate. Active publication is consumed at the next process/flush/reset;
        // block events then take precedence in their existing stream order.
        i.exchange.recall(state);
        if i.lifecycle.load(Ordering::Acquire) == 1 {
            let audio = unsafe { i.audio() };
            i.apply_pending(audio);
            i.publish(audio);
        }
        // Host callbacks are legal here on main; no callback is made by the factory.
        let params = i.host_params.load(Ordering::Acquire);
        if let Some(params) = unsafe { params.as_ref() }
            && let Some(rescan) = params.rescan
        {
            unsafe {
                rescan(i.host, CLAP_PARAM_RESCAN_VALUES);
            }
        }
        if i.lifecycle.load(Ordering::Acquire) >= 2
            && let Some(request) = unsafe { (*i.host).request_process }
        {
            unsafe {
                request(i.host);
            }
        }
        true
    })
}
pub(crate) static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(save),
    load: Some(load),
};
