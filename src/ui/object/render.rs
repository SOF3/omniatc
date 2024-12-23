use std::fmt;

use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::color::{Color, Mix};
use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::math::{Vec3, Vec3Swizzles};
use bevy::prelude::{
    BuildChildren, ChildBuild, Children, Commands, Component, DespawnRecursiveExt, Entity,
    EventReader, IntoSystemConfigs, Parent, Query, Res, Transform, Visibility, With, Without,
};
use bevy::sprite::{Anchor, Sprite};
use bevy::text::{Text2d, TextSpan};

use super::{ColorScheme, Config, LabelElement, LabelLine};
use crate::level::{nav, object, plane};
use crate::math::{TurnDirection, TROPOPAUSE_ALTITUDE};
use crate::ui::{billboard, SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        app.add_systems(app::Update, spawn_plane_viewable_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            (maintain_sprite_system, maintain_label_system).in_set(SystemSets::RenderMove),
        );
    }
}

/// Marker component indicating that the entity is the viewable entity showing a sprite for the
/// object.
#[derive(Component)]
struct SpriteViewable;

/// Marker component indicating that the entity is the viewable entity showing a label text.
#[derive(Component)]
struct LabelViewable;

fn spawn_plane_viewable_system(
    mut commands: Commands,
    mut events: EventReader<plane::SpawnEvent>,
    config: Res<Config>,
    asset_server: Res<AssetServer>,
) {
    for &plane::SpawnEvent(entity) in events.read() {
        commands.entity(entity).insert((Transform::IDENTITY, Visibility::Visible)).with_children(
            |b| {
                b.spawn((
                    Transform::from_translation(Vec3::ZERO.with_z(Zorder::Object.to_z())),
                    Sprite::from_image(asset_server.load("sprites/plane.png")),
                    billboard::MaintainScale { size: config.plane_sprite_size },
                    SpriteViewable,
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
#[query_data(mutable)]
struct SpriteParentQueryData {
    rotation:    &'static object::Rotation,
    position:    &'static object::Position,
    transform:   &'static mut Transform,
    destination: &'static object::Destination,
}

fn maintain_sprite_system(
    mut parent_query: Query<SpriteParentQueryData, Without<SpriteViewable>>,
    mut sprite_query: Query<(Entity, &Parent, &mut Transform, &mut Sprite), With<SpriteViewable>>,
    config: Res<Config>,
) {
    sprite_query.iter_mut().for_each(|(entity, parent_ref, mut sprite_tf, mut sprite)| {
        let Ok(mut parent) = parent_query.get_mut(parent_ref.get()) else {
            bevy::log::warn_once!(
                "sprite entity {entity:?} parent {parent_ref:?} is not an object"
            );
            return;
        };

        parent.transform.translation = parent.position.0.into();
        sprite_tf.rotation = parent.rotation.0;

        sprite.color = resolve_color(&parent, &config.color_scheme);
    });
}

fn resolve_color(data: &SpriteParentQueryDataItem, color_scheme: &ColorScheme) -> Color {
    match color_scheme {
        ColorScheme::Mixed { a, b, factor } => {
            resolve_color(data, a).mix(&resolve_color(data, b), *factor)
        }
        ColorScheme::Altitude(scale) => {
            scale.get((data.position.0.z / TROPOPAUSE_ALTITUDE).clamp(0., 1.))
        }
        ColorScheme::Destination { departure, arrival, ferry } => match data.destination {
            object::Destination::Departure(id) => {
                departure[(id.0 as usize).min(departure.len() - 1)]
            }
            object::Destination::Arrival(id) => arrival[(id.0 as usize).min(arrival.len() - 1)],
            object::Destination::Ferry { to, .. } => ferry[(to.0 as usize).min(ferry.len() - 1)],
        },
    }
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
    fn write_label(&self, element: Option<&LabelElement>, writer: &mut DynamicTextWriterForEntity) {
        let Some(element) = element else {
            writer.set_text("");
            return;
        };

        match *element {
            LabelElement::Const(ref str) => writer.set_text(str),
            LabelElement::IfEmpty {
                ref main,
                ref prefix_if_filled,
                ref suffix_if_filled,
                ref if_empty,
            } => {
                let mut writer = writer.with_child_count(3);

                let mut is_empty = true;
                writer.set_child(1, |writer| {
                    self.write_label(Some(main), writer);
                    is_empty = writer.get_text().is_empty();
                });

                if is_empty {
                    writer.set_child(0, |writer| self.write_label(if_empty.as_deref(), writer));
                } else {
                    writer.set_child(0, |writer| {
                        self.write_label(prefix_if_filled.as_deref(), writer);
                    });
                    writer.set_child(2, |writer| {
                        self.write_label(suffix_if_filled.as_deref(), writer);
                    });
                }
            }
            LabelElement::Name => writer.set_text(&self.display.name),
            LabelElement::CurrentIndicatedAirspeed(unit) => {
                if let Some(airborne) = self.airborne {
                    writer.set_display(unit.convert(airborne.airspeed.xy().length()));
                } else {
                    writer.set_text("");
                }
            }
            LabelElement::CurrentGroundSpeed(unit) => {
                writer.set_display(unit.convert(self.ground_speed.0.xy().length()));
            }
            LabelElement::CurrentAltitude(unit) => {
                writer.set_display(unit.convert(self.position.0.z));
            }
            LabelElement::CurrentHeading => {
                if let Some(plane_control) = self.plane_control {
                    writer
                        .set_display(format_args!("{:.0}\u{b0}", plane_control.heading.degrees()));
                } else {
                    writer.set_text("");
                }
            }
            LabelElement::TargetAirspeed(unit) => {
                if let Some(nav) = self.nav_velocity {
                    writer.set_display(unit.convert(nav.horiz_speed));
                } else {
                    writer.set_text("");
                }
            }
            LabelElement::TargetAltitude(unit) => {
                if let Some(nav) = self.nav_altitude {
                    writer.set_display(unit.convert(nav.0));
                } else {
                    writer.set_text("");
                }
            }
            LabelElement::TargetClimbRate(unit) => {
                if let Some(nav) = self.nav_velocity {
                    writer.set_display(unit.convert(nav.vert_rate));
                } else {
                    writer.set_text("");
                }
            }
            LabelElement::TargetHeading => {
                if let Some(nav) = self.nav_velocity {
                    match nav.yaw {
                        nav::YawTarget::Heading(heading) => {
                            writer.set_display(format_args!("{:.0}\u{B0}", heading.degrees()));
                        }
                        nav::YawTarget::TurnHeading { heading, remaining_crosses, direction } => {
                            writer.set_display(format_args!(
                                "({degrees:.0}\u{B0}{dir}{remain}",
                                degrees = heading.degrees(),
                                dir = match direction {
                                    TurnDirection::Clockwise => 'R',
                                    TurnDirection::CounterClockwise => 'L',
                                },
                                remain = if remaining_crosses > 0 {
                                    format!("\u{D7}{remaining_crosses}")
                                } else {
                                    String::new()
                                },
                            ));
                        }
                        nav::YawTarget::Speed(speed) => writer.set_text(match speed {
                            0.0 => "",
                            0.0.. => "R",
                            _ => "L",
                        }),
                    }
                }
            }
        }
    }
}

#[derive(SystemParam)]
struct DynamicTextWriter<'w, 's> {
    commands:        Commands<'w, 's>,
    children_query:  Query<'w, 's, &'static Children>,
    text_span_query: Query<'w, 's, &'static mut TextSpan>,
}

impl<'w, 's> DynamicTextWriter<'w, 's> {
    fn borrow_for(&mut self, entity: Entity) -> DynamicTextWriterForEntity<'_, 'w, 's> {
        DynamicTextWriterForEntity {
            entity,
            commands: &mut self.commands,
            children_query: &self.children_query,
            text_span_query: &mut self.text_span_query,
        }
    }
}

struct DynamicTextWriterForEntity<'a, 'w, 's> {
    entity:          Entity,
    commands:        &'a mut Commands<'w, 's>,
    children_query:  &'a Query<'w, 's, &'static Children>,
    text_span_query: &'a mut Query<'w, 's, &'static mut TextSpan>,
}

impl<'w, 's> DynamicTextWriterForEntity<'_, 'w, 's> {
    fn set_display(&mut self, text: impl fmt::Display) { self.set_text(text.to_string()) }

    fn set_text(&mut self, text: impl AsRef<str>) {
        let mut span = self
            .text_span_query
            .get_mut(self.entity)
            .expect("descendent of the label entity must have TextSpan component");
        if span.0.as_str() != text.as_ref() {
            span.0.clear();
            span.0.push_str(text.as_ref());
        }
    }

    fn get_text(&self) -> &str {
        let span = self
            .text_span_query
            .get(self.entity)
            .expect("descendent of the label entity must have TextSpan component");
        span.0.as_str()
    }

    fn with_child_count(&mut self, expected_children_count: usize) -> ChildWriter<'_, '_, 'w, 's> {
        let children: &[Entity] = if let Ok(children) = self.children_query.get(self.entity) {
            for &extra_child in children.get(expected_children_count..).into_iter().flatten() {
                self.commands.entity(extra_child).despawn_recursive();
            }
            children
        } else {
            &[]
        };

        self.commands.entity(self.entity).with_children(|b| {
            for _ in children.len()..expected_children_count {
                // just wait for the next tick to populate this span for simplicity.
                b.spawn(TextSpan::new(String::new()));
            }
        });

        ChildWriter(
            children,
            DynamicTextWriterForEntity {
                entity:          self.entity,
                commands:        self.commands,
                children_query:  self.children_query,
                text_span_query: self.text_span_query,
            },
        )
    }
}

struct ChildWriter<'e, 'a, 'w, 's>(&'e [Entity], DynamicTextWriterForEntity<'a, 'w, 's>);

impl<'w, 's> ChildWriter<'_, '_, 'w, 's> {
    fn set_child(
        &mut self,
        index: usize,
        mut mutator: impl for<'a> FnMut(&mut DynamicTextWriterForEntity<'a, 'w, 's>),
    ) {
        if let Some(&entity) = self.0.get(index) {
            self.1.entity = entity;
            mutator(&mut self.1);
        }
    }
}

fn maintain_label_system(
    parent_query: Query<LabelParentQuery>,
    mut label_query: Query<(Entity, &Parent), With<LabelViewable>>,
    config: Res<Config>,
    mut writer: DynamicTextWriter,
) {
    label_query.iter_mut().for_each(|(label_entity, parent_ref)| {
        let Ok(parent) = parent_query.get(parent_ref.get()) else {
            bevy::log::warn_once!(
                "label entity {label_entity:?} parent {parent_ref:?} is not an object"
            );
            return;
        };

        let mut label_writer = writer.borrow_for(label_entity);
        let mut label_writer = label_writer.with_child_count(config.label_elements.len());

        for (line, LabelLine { elements }) in config.label_elements.iter().enumerate() {
            label_writer.set_child(line, |line_writer| {
                let newline_offset = usize::from(line != 0);
                let mut line_writer = line_writer.with_child_count(elements.len() + newline_offset);
                if line != 0 {
                    line_writer.set_child(0, |writer| writer.set_text("\n"));
                }

                for (index, element) in elements.iter().enumerate() {
                    line_writer.set_child(index + newline_offset, |writer| {
                        parent.write_label(Some(element), writer);
                    });
                }
            });
        }
    });
}
