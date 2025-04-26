use std::time::SystemTime;

pub mod billboard;
pub mod shapes;

#[cfg(target_family = "wasm")]
pub fn time_now() -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs_f64(js_sys::Date::now() * 1e-3)
}

#[cfg(not(target_family = "wasm"))]
pub fn time_now() -> SystemTime { SystemTime::now() }
