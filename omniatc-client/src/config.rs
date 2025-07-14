use std::any::{type_name, Any, TypeId};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::{mem, ops};

use bevy::app::App;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Local, Res, SystemParam};
use bevy::utils::TypeIdMap;
use bevy_egui::egui;
use serde::de::DeserializeOwned;
use serde::Serialize;

mod bool_impl;
mod color_impl;
mod enum_impls;
pub use enum_impls::EnumField;
mod float_impls;

pub use omniatc_macros::Config;

use crate::render;

pub trait AppExt {
    fn init_config<C: Config>(&mut self);
}

impl AppExt for App {
    fn init_config<C: Config>(&mut self) {
        self.init_resource::<Registry>();
        self.init_resource::<Values>();
        self.world_mut().resource_mut::<Registry>().0.push(RegisteredType::new::<C>());
        self.world_mut().resource_mut::<Values>().0.insert(
            TypeId::of::<C>(),
            StoredConfig { value: Box::new(C::default()), generation: 0 },
        );
    }
}

/// Registry of all initialized configs.
#[derive(Default, Resource)]
pub struct Registry(pub Vec<RegisteredType>);

/// Stores the actual config values.
#[derive(Default, Resource)]
pub struct Values(pub TypeIdMap<StoredConfig>);

pub struct StoredConfig {
    pub value:      Box<dyn Any + Send + Sync>,
    pub generation: u64,
}

/// Type-erased data of a registered [`Config`] implementation.
pub struct RegisteredType {
    pub id:   &'static str,
    pub name: &'static str,
    pub draw: fn(&mut Values, &mut egui::Ui, &mut String),
}

impl RegisteredType {
    fn new<C: Config>() -> Self {
        Self { id: C::save_id(), name: C::name(), draw: render::config_editor::draw::<C> }
    }
}

pub trait Config: Default + Resource {
    fn save_id() -> &'static str;
    fn name() -> &'static str;

    fn for_each_field<V: FieldVisitor>(&mut self, visitor: &mut V, ctx: &mut FieldEguiContext);
}

pub trait FieldVisitor {
    fn visit_field<F: Field>(
        &mut self,
        meta: FieldMeta<F::Opts>,
        field: &mut F,
        ctx: &mut FieldEguiContext,
    );
}

pub struct FieldMeta<Opts> {
    pub group: Cow<'static, str>,
    pub id:    &'static str,
    pub doc:   &'static str,
    pub opts:  Opts,
}

/// A type that can be used in a Config implementor struct.
pub trait Field: Sized {
    /// A struct of constant values that can be specified in `#[config(...)]` attributes.
    type Opts: Default;

    fn show_egui(
        &mut self,
        meta: FieldMeta<Self::Opts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    );

    fn as_serialize(&self) -> impl Serialize + '_;

    type Deserialize: DeserializeOwned;
    fn from_deserialize(de: Self::Deserialize) -> Self;
}

pub struct FieldEguiContext<'a> {
    pub doc:     &'a mut String,
    pub changed: &'a mut bool,
}

#[derive(SystemParam)]
pub struct Read<'w, 's, T: Config> {
    values:        Res<'w, Values>,
    last_observed: Local<'s, u64>,
    _ph:           PhantomData<T>,
}

impl<T: Config> Read<'_, '_, T> {
    pub fn consume_change(&mut self) -> Option<&T> {
        match self.values.0.get(&TypeId::of::<T>()) {
            Some(v) => {
                if mem::replace(&mut *self.last_observed, v.generation) == v.generation {
                    None
                } else {
                    Some(v.value.downcast_ref().expect("TypeId mismatch"))
                }
            }
            None => panic!("Config type {} has not been initialized yet", type_name::<T>()),
        }
    }
}

impl<T: Config> ops::Deref for Read<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self.values.0.get(&TypeId::of::<T>()) {
            Some(v) => v.value.downcast_ref().expect("TypeId mismatch"),
            None => panic!("Config type {} has not been initialized yet", type_name::<T>()),
        }
    }
}
