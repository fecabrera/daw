#![allow(non_snake_case)]
// SDK test-host pointers stay live through every synchronous COM call. Owned
// interfaces are released before module unload; queue and stream borrows do not
// change reference counts during processing.
include!("host/support.rs");
#[path = "host/lifecycle.rs"]
mod lifecycle;
#[path = "host/parity.rs"]
mod parity;
#[path = "host/performance.rs"]
mod performance;
