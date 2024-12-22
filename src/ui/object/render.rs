use std::fmt;

use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::color::Color;
use bevy::ecs::query::QueryData;
use bevy::math::{Vec3, Vec3Swizzles};
use bevy::prelude::{
    BuildChildren, ChildBuild, Commands, Component, Entity, EventReader, IntoSystemConfigs, Parent,
    Query, Res, Transform, Visibility, With, Without,
};
use bevy::sprite::{Anchor, Sprite};
use bevy::text::Text2d;

use super::{DisplayConfig, LabelElement};
use crate::level::{nav, object, plane};
use crate::math::{TurnDirection, TROPOPAUSE_ALTITUDE};
use crate::ui::{billboard, SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisplayConfig>();

        app.add_systems(app::Update, spawn_plane_viewable_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            (maintain_target_system, maintain_label_system).in_set(SystemSets::RenderMove),
        );
    }
}

/// Marker component indicating that the entity is the viewable entity showing a target sprite.
#[derive(Component)]
struct TargetViewable;

/// Marker component indicating that the entity is the viewable entity showing a label text.
#[derive(Component)]
struct LabelViewable;

fn spawn_plane_viewable_system(
    mut commands: Commands,
    mut events: EventReader<plane::SpawnEvent>,
    config: Res<DisplayConfig>,
    asset_server: Res<AssetServer>,
) {
    for &plane::SpawnEvent(entity) in events.read() {
        commands.entity(entity).insert((Transform::IDENTITY, Visibility::Visible)).with_children(
            |b| {
                b.spawn((
                    Transform::from_translation(Vec3::ZERO.with_z(Zorder::Object.to_z())),
                    Sprite::from_image(asset_server.load("sprites/plane.png")),
                    billboard::MaintainScale { size: config.plane_sprite_size },
                    TargetViewable,
                ));
                b.spawn((
                    Transform::from_translation(Vec3::ZERO.with_z(Zorder::Object.to_z())),
                    Text2d::new(""),
                    billboard::MaintainScale { size: config.label_size },
                    billboard::MaintainRotation,
                    billboard::Label { distance: 50. },
                    Anchor::TopRight,
                    LabelViewable,
                ));
            },
        );
    }
}

#[derive(QueryData)]
struct TargetParentQuery {
    rotation: &'static object::Rotation,
    position: &'static object::Position,
}

fn maintain_target_system(
    mut parent_query: Query<
        (&object::Rotation, &object::Position, &mut Transform),
        Without<TargetViewable>,
    >,
    mut target_query: Query<(Entity, &Parent, &mut Transform, &mut Sprite), With<TargetViewable>>,
) {
    target_query.iter_mut().for_each(|(entity, parent_ref, mut target_tf, mut sprite)| {
        let Ok((rotation, position, mut parent_tf)) = parent_query.get_mut(parent_ref.get()) else {
            bevy::log::warn_once!(
                "target entity {entity:?} parent {parent_ref:?} is not an object"
            );
            return;
        };

        parent_tf.translation = position.0.into();
        target_tf.rotation = rotation.0;

        // TODO by color scheme
        sprite.color = Color::srgb((position.0.z / TROPOPAUSE_ALTITUDE).clamp(0., 1.), 1., 1.);
    });
}

#[derive(QueryData)]
struct LabelParentQuery {
    display:      &'static object::Display,
    ground_speed: &'static object::GroundSpeed,
    position:     &'static object::Position,
    airborne:     Option<&'static object::Airborne>,

    plane_control: Option<&'static plane::Control>,

    nav_velocity: Option<&'static nav::VelocityTarget>,
    nav_altitude: Option<&'static nav::TargetAltitude>,
}

impl LabelParentQueryItem<'_> {
    fn write_label(&self, element: &LabelElement, out: &mut LabelWriter) {
        match *element {
            LabelElement::Const(ref str) => out.push_str(str),
            LabelElement::IfEmpty {
                ref main,
                ref prefix_if_filled,
                ref suffix_if_filled,
                ref if_empty,
            } => {
                let prefix_writer = &mut |buf: &mut String| {
                    if let Some(prefix) = prefix_if_filled {
                        self.write_label(
                            prefix,
                            &mut LabelWriter { buf, before_first_write: None },
                        );
                    }
                };
                let mut out_wrapped = out.with_before_first_write(prefix_writer);

                self.write_label(main, &mut out_wrapped);

                #[allow(clippy::collapsible_else_if)] // these are two symmetric cases
                if out_wrapped.before_first_write.is_some() {
                    // empty
                    if let Some(if_empty) = if_empty {
                        self.write_label(if_empty, out);
                    }
                } else {
                    if let Some(suffix) = suffix_if_filled {
                        self.write_label(suffix, out);
                    }
                }
            }
            LabelElement::Name => out.push_str(&self.display.name),
            LabelElement::CurrentIndicatedAirspeed(unit) => {
                if let Some(airborne) = self.airborne {
                    out.push_display(&unit.convert(airborne.airspeed.xy().length()));
                }
            }
            LabelElement::CurrentGroundSpeed(unit) => {
                out.push_display(&unit.convert(self.ground_speed.0.xy().length()));
            }
            LabelElement::CurrentAltitude(unit) => {
                out.push_display(&unit.convert(self.position.0.z));
            }
            LabelElement::CurrentHeading => {
                if let Some(plane_control) = self.plane_control {
                    out.push_display(&format_args!("{:.0}\u{b0}", plane_control.heading.degrees()));
                }
            }
            LabelElement::TargetAirspeed(unit) => {
                if let Some(nav) = self.nav_velocity {
                    out.push_display(&unit.convert(nav.horiz_speed));
                }
            }
            LabelElement::TargetAltitude(unit) => {
                if let Some(nav) = self.nav_altitude {
                    out.push_display(&unit.convert(nav.0));
                }
            }
            LabelElement::TargetClimbRate(unit) => {
                if let Some(nav) = self.nav_velocity {
                    out.push_display(&unit.convert(nav.vert_rate));
                }
            }
            LabelElement::TargetHeading => {
                if let Some(nav) = self.nav_velocity {
                    match nav.yaw {
                        nav::YawTarget::Heading(heading) => {
                            out.push_display(&format_args!("{:.0}\u{B0}", heading.degrees()));
                        }
                        nav::YawTarget::TurnHeading { heading, remaining_crosses, direction } => {
                            out.push_display(&format_args!("({:.0}\u{B0}", heading.degrees()));
                            out.push_char(match direction {
                                TurnDirection::Clockwise => 'R',
                                TurnDirection::CounterClockwise => 'L',
                            });
                            if remaining_crosses > 0 {
                                out.push_display(&format_args!("\u{D7}{remaining_crosses}"));
                            }
                        }
                        nav::YawTarget::Speed(speed) => {
                            match speed {
                                0.0 => {}
                                0.0.. => out.push_char('R'),
                                _ => out.push_char('L'),
                            };
                        }
                    }
                }
            }
        }
    }
}

struct LabelWriter<'a, 'b> {
    buf:                &'a mut String,
    before_first_write: Option<&'b mut dyn FnMut(&mut String)>,
}

impl LabelWriter<'_, '_> {
    fn push_char(&mut self, char: char) {
        self.consume_before_write();
        self.buf.push(char);
    }

    fn push_str(&mut self, str: &str) {
        self.consume_before_write();
        self.buf.push_str(str);
    }

    fn push_display(&mut self, args: &impl fmt::Display) {
        use fmt::Write;

        self.consume_before_write();
        write!(self.buf, "{args}").unwrap();
    }

    fn consume_before_write(&mut self) {
        if let Some(closure) = self.before_first_write.take() {
            closure(self.buf);
        }
    }

    fn with_before_first_write<'a, 'b>(
        &'a mut self,
        handler: &'b mut dyn FnMut(&mut String),
    ) -> LabelWriter<'a, 'b> {
        LabelWriter { buf: self.buf, before_first_write: Some(handler) }
    }
}

fn maintain_label_system(
    parent_query: Query<LabelParentQuery>,
    mut label_query: Query<(Entity, &Parent, &mut Text2d), With<LabelViewable>>,
    config: Res<DisplayConfig>,
) {
    label_query.iter_mut().for_each(|(entity, parent_ref, mut label)| {
        let Ok(parent) = parent_query.get(parent_ref.get()) else {
            bevy::log::warn_once!(
                "target entity {entity:?} parent {parent_ref:?} is not an object"
            );
            return;
        };

        label.0.clear();

        let mut last_newline = 0;

        for line in &config.label_elements {
            if label.0.len() > last_newline {
                label.0.push('\n');
            }
            last_newline = label.0.len();

            for element in &line.elements {
                parent.write_label(
                    element,
                    &mut LabelWriter { buf: &mut label.0, before_first_write: None },
                );
            }
        }
    });
}
