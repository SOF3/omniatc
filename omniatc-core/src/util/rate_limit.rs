use std::num::NonZero;
use std::time::Duration;

use bevy::ecs::system::{Local, Res, SystemParam};
use bevy::time::{self, Time};

#[derive(SystemParam)]
pub struct RateLimit<'w, 's> {
    time:     Res<'w, Time<time::Virtual>>,
    last_run: Local<'s, Option<u128>>,
}

impl RateLimit<'_, '_> {
    pub fn should_run(&mut self, min_period: Duration) -> Option<NonZero<u128>> {
        if self.time.is_paused() || min_period.is_zero() {
            return None;
        }

        let now = self.time.elapsed().as_nanos() / min_period.as_nanos();

        match self.last_run.replace(now) {
            None => NonZero::new(1),
            Some(last) => NonZero::new(now - last),
        }
    }
}
