#![allow(unused_imports)]
use std::time::{Duration, SystemTime};

#[cfg(target_arch = "wasm32")]
pub fn current_time() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs_f64(js_sys::Date::now() * 1e-3)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn current_time() -> SystemTime { SystemTime::now() }
