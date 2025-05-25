use bevy_egui::egui;
use serde::Serialize;

use super::{Field, FieldEguiContext, FieldMeta};

#[derive(Default)]
pub struct BoolOpts {}

impl Field for bool {
    type Opts = BoolOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<BoolOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let prev = *self;
        ui.checkbox(self, meta.id);
        if prev != *self {
            *ctx.changed = true;
        }
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}
