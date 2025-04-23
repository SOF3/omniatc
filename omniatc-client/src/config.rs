use std::ops;

use bevy::app::App;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, SystemParam};

pub trait AppExt {
    fn init_config<C: Config>(&mut self);
}

impl AppExt for App {
    fn init_config<C: Config>(&mut self) { self.init_resource::<C>(); }
}

pub trait Config: Default + Resource {}

// TODO: make a derive macro for Config
impl<T: Default + Resource> Config for T {}

#[derive(SystemParam)]
pub struct Read<'w, T: Config>(Res<'w, T>);

impl<T: Config> ops::Deref for Read<'_, T> {
    type Target = T;

    fn deref(&self) -> &T { &self.0 }
}
