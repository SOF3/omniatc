//! PID control systems.

use std::mem;

pub struct State {
    pub params: Params,
    prev_error: f32,
}

impl Default for State {
    fn default() -> Self { Self::new(Params { p_gain: 1.0, i_gain: 0.0, d_gain: 0.0 }) }
}

impl State {
    #[must_use]
    pub fn new(params: Params) -> Self { Self { params, prev_error: 0. } }
}

#[allow(clippy::struct_field_names)]
pub struct Params {
    pub p_gain: f32,
    pub i_gain: f32,
    pub d_gain: f32,
}

pub fn control(state: &mut State, error: f32, dt: f32) -> f32 {
    let prev_error = mem::replace(&mut state.prev_error, error);
    let p_term = state.params.p_gain * error;
    let i_term = state.params.i_gain * (error + prev_error) * dt;
    let d_term = state.params.d_gain * (error - prev_error) * dt;
    p_term + i_term + d_term
}
