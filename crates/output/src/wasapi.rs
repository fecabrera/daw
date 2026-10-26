//! CPAL 0.16 caches one Windows device enumerator for the whole process.
//! Seed it in a process-lived MTA before any discovery or stream construction.
use cpal::traits::HostTrait;
use std::{marker::PhantomData, sync::OnceLock};
use windows_sys::Win32::System::Com::{
    CO_MTA_USAGE_COOKIE, COINIT_MULTITHREADED, CoDecrementMTAUsage, CoIncrementMTAUsage,
    CoInitializeEx, CoUninitialize,
};

static PREPARED: OnceLock<Result<(), String>> = OnceLock::new();

/// Control/worker thread only. The callback never enters COM or this initializer.
pub(super) fn prepare() -> Result<(), String> {
    PREPARED
        .get_or_init(|| {
            // The caller can be the window's STA. A fresh thread avoids changing its apartment.
            std::thread::Builder::new()
                .name("wasapi-initialize".into())
                .spawn(initialize)
                .map_err(|error| format!("Cannot start Windows audio initialization: {error}"))?
                .join()
                .map_err(|_| "Windows audio initialization worker panicked".to_owned())?
        })
        .clone()
}

// CoUninitialize must run on the thread that initialized this apartment.
struct MtaApartment(PhantomData<*mut ()>);
impl Drop for MtaApartment {
    fn drop(&mut self) {
        // SAFETY: this thread's unique guard balances one successful CoInitializeEx call.
        unsafe {
            CoUninitialize();
        }
    }
}

struct MtaUsage(CO_MTA_USAGE_COOKIE);
impl Drop for MtaUsage {
    fn drop(&mut self) {
        // SAFETY: this cookie came from one successful CoIncrementMTAUsage call,
        // and this unique owner balances it only on initialization failure/unwind.
        unsafe {
            CoDecrementMTAUsage(self.0);
        }
    }
}
fn initialize() -> Result<(), String> {
    // Explicit MTA initialization is required: an implicit MTA from a usage reference
    // alone would still let CPAL's later STA initialization change this thread's apartment.
    // SAFETY: this fresh worker has no application COM state; the reserved pointer is null.
    let status = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    if status < 0 {
        return Err(format!(
            "Cannot initialize Windows audio COM apartment: HRESULT 0x{:08X}",
            status as u32
        ));
    }
    let _apartment = MtaApartment(PhantomData);
    let mut cookie = 0;
    // SAFETY: cookie points to writable storage; this thread has explicitly joined the MTA.
    // The call creates/joins the MTA and keeps its resources alive independently of threads.
    let status = unsafe { CoIncrementMTAUsage(&mut cookie) };
    if status < 0 {
        return Err(format!(
            "Cannot retain Windows audio COM apartment: HRESULT 0x{:08X}",
            status as u32
        ));
    }
    let usage = MtaUsage(cookie);
    // CPAL's OnceLock must first create its enumerator here, never in a temporary STA.
    // A missing default device is valid; the call still initializes the cached enumerator.
    let _ = cpal::default_host().default_output_device();
    // CPAL's static enumerator is never dropped. Match its process lifetime with one
    // MTA reference; dropping it when this worker exits would invalidate the cache.
    std::mem::forget(usage);
    Ok(())
}
