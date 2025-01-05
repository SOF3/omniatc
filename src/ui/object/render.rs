use std::f32::consts::TAU;
use std::time::Duration;
use std::{fmt, str};

use bevy::app::{self, App, Plugin};
use bevy::asset::{AssetServer, Assets, Handle};
use bevy::color::{Color, Mix};
use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::math::{Vec3, Vec3Swizzles};
use bevy::prelude::{
    Annulus, BuildChildren, Camera2d, ChildBuild, Children, Commands, Component,
    DespawnRecursiveExt, DetectChangesMut, Entity, EventReader, GlobalTransform, IntoSystemConfigs,
    Mesh, Mesh2d, Mut, Parent, Query, Res, ResMut, Resource, Single, Transform, Visibility, With,
    Without,
};
use bevy::sprite::{Anchor, ColorMaterial, MeshMaterial2d, Sprite};
use bevy::text::{Text2d, TextColor, TextSpan};
use bevy::time::Time;

use super::{select, ColorScheme, Config, LabelElement, LabelLine};
use crate::level::{aerodrome, nav, object, plane};
use crate::math::{Heading, TurnDirection, TROPOPAUSE_ALTITUDE};
use crate::ui::{billboard, SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        app.init_resource::<SeparationAnnulusMesh>();
        app.add_systems(
            app::Startup,
            |mut store: ResMut<SeparationAnnulusMesh>, mut meshes: ResMut<Assets<Mesh>>| {
                store.0 = Some(meshes.add(Annulus::new(0., 1.5)));
            },
        );

        app.add_systems(app::Update, spawn_plane_viewable_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(app::Update, maintain_viewable_system.in_set(SystemSets::RenderMove));
        app.add_systems(app::Update, maintain_annulus_size_system.in_set(SystemSets::RenderMove));
    }
}

#[derive(Component)]
struct ObjectOwner;

/// Marker component indicating that the entity is the viewable entity showing a sprite for the
/// object.
#[derive(Component)]
struct SpriteViewable;

/// Marker component indicating that the entity is the viewable entity showing a label text.
#[derive(Component)]
struct LabelViewable;

/// Marker component indicating that the entity is the viewable entity showing the separation ring.
#[derive(Component)]
struct SeparationRing;

#[derive(Resource, Default)]
struct SeparationAnnulusMesh(Option<Handle<Mesh>>);

fn spawn_plane_viewable_system(
    mut commands: Commands,
    mut events: EventReader<plane::SpawnEvent>,
    config: Res<Config>,
    asset_server: Res<AssetServer>,
    separation_annulus_mesh: Res<SeparationAnnulusMesh>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for &plane::SpawnEvent(entity) in events.read() {
        let color = materials.add(ColorMaterial { color: Color::WHITE, ..Default::default() });

        commands
            .entity(entity)
            .insert((Transform::IDENTITY, Visibility::Visible, LastRender(None)))
            .with_children(|b| {
                b.spawn((
                    bevy::core::Name::new("ObjectOwner"),
                    ObjectOwner,
                    Transform::IDENTITY,
                    Visibility::Inherited,
                ))
                .with_children(|b| {
                    b.spawn((
                        bevy::core::Name::new("ObjectSprite"),
                        Transform::from_translation(Vec3::ZERO.with_z(Zorder::Object.into_z())),
                        Sprite::from_image(asset_server.load("sprites/plane.png")),
                        billboard::MaintainScale { size: config.plane_sprite_size },
                        SpriteViewable,
                    ));
                    b.spawn((
                        bevy::core::Name::new("SeparationRing"),
                        Transform::from_translation(
                            Vec3::ZERO.with_z(Zorder::ObjectSeparation.into_z()),
                        ),
                        Mesh2d(separation_annulus_mesh.0.clone().unwrap()),
                        MeshMaterial2d(color),
                        SeparationRing,
                    ));
                    b.spawn((
                        bevy::core::Name::new("ObjectLabel"),
                        Transform::from_translation(
                            Vec3::ZERO.with_z(Zorder::ObjectLabel.into_z()),
                        ),
                        Text2d::new(""),
                        billboard::MaintainScale { size: config.label_size },
                        billboard::MaintainRotation,
                        billboard::Label { distance: 50. },
                        Anchor::TopRight,
                        LabelViewable,
                        LabelSpan,
                    ));
                });
            });
    }
}

fn maintain_annulus_size_system(
    config: Res<Config>,
    camera_tf: Single<&GlobalTransform, With<Camera2d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    separation_annulus_mesh: Res<SeparationAnnulusMesh>,
) {
    let mesh = meshes
        .get_mut(separation_annulus_mesh.0.as_ref().unwrap())
        .expect("asset referenced by strong handle must exist");

    let width = camera_tf.scale().x * config.separation_ring_thickness;
    *mesh = Annulus::new(1.5 - width, 1.5).into();
}

/// Stores the last time an object has been rendered.
#[derive(Component)]
struct LastRender(Option<Duration>);

#[derive(QueryData)]
struct ParentQueryData {
    entity: Entity,

    display:      &'static object::Display,
    destination:  &'static object::Destination,
    ground_speed: &'static object::GroundSpeed,
    position:     &'static object::Position,
    rotation:     &'static object::Rotation,
    airborne:     Option<&'static object::Airborne>,

    plane_control: Option<&'static plane::Control>,

    nav_velocity: Option<&'static nav::VelocityTarget>,
    nav_altitude: Option<&'static nav::TargetAltitude>,
}

#[allow(clippy::too_many_arguments)] // TODO we need to split up this system a bit
fn maintain_viewable_system(
    time: Res<Time>,
    config: Res<Config>,
    mut parent_query: Query<(&mut LastRender, ParentQueryData)>,
    mut owner_query: Query<
        (&mut Transform, &Parent, &Children),
        (With<ObjectOwner>, Without<SpriteViewable>, Without<LabelViewable>),
    >,
    mut sprite_query: Query<(&mut Transform, &mut Sprite), With<SpriteViewable>>,
    separation_ring_query: Query<&MeshMaterial2d<ColorMaterial>, With<SeparationRing>>,
    label_query: Query<&LabelViewable>,
    mut writer: DynamicTextWriter,
    resolve_color_params: ResolveColorParams,
    write_label_params: WriteLabelParams,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
    owner_query.iter_mut().for_each(|(mut owner_tf, object_entity, owner_children)| {
        let Ok((mut last_render, parent)) = parent_query.get_mut(object_entity.get()) else {
            bevy::log::warn!("OwnerEntity parent is invalid");
            return;
        };

        let need_move = last_render.0.is_none_or(|last| {
            let Some(freq) = config.scan_frequency else {
                return true;
            };

            let scan_radial = Heading::from_vec3(parent.position.0).radians_nonnegative() / TAU;
            let scan_offset = freq.mul_f32(scan_radial);
            let current_cycles =
                time.elapsed().saturating_sub(scan_offset).as_nanos() / freq.as_nanos();
            let last_render_cycles = last.saturating_sub(scan_offset).as_nanos() / freq.as_nanos();
            current_cycles != last_render_cycles
        });

        if need_move {
            last_render.0 = Some(time.elapsed());
        }

        let color = parent.resolve_color(&config.color_scheme, &resolve_color_params);

        owner_tf.translation = parent.position.0.into();

        owner_children.iter().for_each(|&child_entity| {
            if let Ok((mut sprite_tf, mut sprite)) = sprite_query.get_mut(child_entity) {
                if need_move {
                    sprite_tf.rotation = parent.rotation.0;
                }
                sprite.color = color;
            }

            if let Ok(color_handle) = separation_ring_query.get(child_entity) {
                let material = color_materials
                    .get_mut(color_handle)
                    .expect("assets referenced by strong handle must exist");
                material.color = color;
            }

            if label_query.get(child_entity).is_ok() {
                maintain_label(
                    &config,
                    &write_label_params,
                    &mut writer.borrow_for(child_entity),
                    &parent,
                    color,
                );
            }
        });
    });
}

fn maintain_label(
    config: &Config,
    write_label_params: &WriteLabelParams,
    label_writer: &mut DynamicTextWriterForEntity,
    parent: &ParentQueryDataItem,
    color: Color,
) {
    label_writer.set_color(color);

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
                    parent.write_label(Some(element), writer, write_label_params, color);
                });
            }
        });
    }
}

#[derive(SystemParam)]
struct ResolveColorParams<'w, 's> {
    aerodrome_query: Query<'w, 's, &'static aerodrome::Display>,
}

impl ParentQueryDataItem<'_> {
    fn resolve_color(&self, color_scheme: &ColorScheme, params: &ResolveColorParams) -> Color {
        match color_scheme {
            ColorScheme::Mixed { a, b, factor } => {
                self.resolve_color(a, params).mix(&self.resolve_color(b, params), *factor)
            }
            ColorScheme::Altitude(scale) => {
                scale.get((self.position.0.z / TROPOPAUSE_ALTITUDE).clamp(0., 1.))
            }
            ColorScheme::Destination { departure, arrival, ferry } => match *self.destination {
                object::Destination::Departure { aerodrome } => {
                    let id = match params.aerodrome_query.get(aerodrome) {
                        Ok(&aerodrome::Display { id, .. }) => id,
                        _ => 0,
                    };
                    departure[(id as usize).min(departure.len() - 1)]
                }
                object::Destination::Arrival { aerodrome } => {
                    let id = match params.aerodrome_query.get(aerodrome) {
                        Ok(&aerodrome::Display { id, .. }) => id,
                        _ => 0,
                    };
                    arrival[(id as usize).min(arrival.len() - 1)]
                }
                object::Destination::Ferry { to_aerodrome, .. } => {
                    let id = match params.aerodrome_query.get(to_aerodrome) {
                        Ok(&aerodrome::Display { id, .. }) => id,
                        _ => 0,
                    };
                    ferry[(id as usize).min(ferry.len() - 1)]
                }
            },
        }
    }

    #[allow(clippy::too_many_lines)] // all the clutter are in separate functions
    fn write_label(
        &self,
        element: Option<&LabelElement>,
        writer: &mut DynamicTextWriterForEntity,
        params: &WriteLabelParams,
        color: Color,
    ) {
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
                    self.write_label(Some(main), writer, params, color);
                    is_empty = writer.get_text().is_empty();
                });

                if is_empty {
                    writer.set_child(0, |writer| {
                        self.write_label(if_empty.as_deref(), writer, params, color);
                    });
                } else {
                    writer.set_child(0, |writer| {
                        self.write_label(prefix_if_filled.as_deref(), writer, params, color);
                    });
                    writer.set_child(2, |writer| {
                        self.write_label(suffix_if_filled.as_deref(), writer, params, color);
                    });
                }
            }
            LabelElement::Name => match &params.search_stack.chars {
                None => writer.with_child_count(1).set_child(0, |writer| {
                    writer.set_text(&self.display.name);
                    writer.set_color(match params.selected.object_entity {
                        Some(entity) if entity == self.entity => params.config.selected_color,
                        _ => color,
                    });
                }),
                Some(chars) => {
                    let mut chars = chars.chars().peekable();

                    let mut writer = writer.with_child_count(self.display.name.chars().count());

                    for (index, name_char) in self.display.name.chars().enumerate() {
                        let color = match chars.peek().copied() {
                            Some(c) if name_char.eq_ignore_ascii_case(&c) => {
                                chars.next().unwrap();
                                params.config.search_matched_color
                            }
                            Some(_) => params.config.search_skipped_color,
                            None => params.config.search_remaining_color,
                        };

                        writer.set_child(index, |writer| {
                            let mut buf = [0u8; 4];
                            writer.set_text(name_char.encode_utf8(&mut buf));
                            writer.set_color(color);
                        });
                    }
                }
            },
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
                    writer.set_display(unit.convert(nav.altitude));
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
struct WriteLabelParams<'w> {
    search_stack: Res<'w, select::SearchStack>,
    config:       Res<'w, Config>,
    selected:     Res<'w, select::Selected>,
}

#[derive(Component)]
struct LabelSpan;

#[derive(SystemParam)]
struct DynamicTextWriter<'w, 's> {
    commands:         Commands<'w, 's>,
    children_query:   Query<'w, 's, &'static Children, With<LabelSpan>>,
    text_span_query:  Query<'w, 's, &'static mut TextSpan, With<LabelSpan>>,
    text_color_query: Query<'w, 's, &'static mut TextColor, With<LabelSpan>>,
}

impl<'w, 's> DynamicTextWriter<'w, 's> {
    fn borrow_for(&mut self, entity: Entity) -> DynamicTextWriterForEntity<'_, 'w, 's> {
        DynamicTextWriterForEntity {
            entity,
            commands: &mut self.commands,
            children_query: &self.children_query,
            text_span_query: &mut self.text_span_query,
            text_color_query: &mut self.text_color_query,
        }
    }
}

struct DynamicTextWriterForEntity<'a, 'w, 's> {
    entity:           Entity,
    commands:         &'a mut Commands<'w, 's>,
    children_query:   &'a Query<'w, 's, &'static Children, With<LabelSpan>>,
    text_span_query:  &'a mut Query<'w, 's, &'static mut TextSpan, With<LabelSpan>>,
    text_color_query: &'a mut Query<'w, 's, &'static mut TextColor, With<LabelSpan>>,
}

impl<'w, 's> DynamicTextWriterForEntity<'_, 'w, 's> {
    fn set_display(&mut self, text: impl fmt::Display) { self.set_text(text.to_string()) }

    fn set_text(&mut self, text: impl AsRef<str>) {
        let mut span = self
            .text_span_query
            .get_mut(self.entity)
            .expect("descendent of the label entity must have TextSpan component");
        if span.0 != text.as_ref() {
            text.as_ref().clone_into(&mut span.0);
        }
    }

    fn set_color(&mut self, color: Color) {
        let span = self
            .text_color_query
            .get_mut(self.entity)
            .expect("descendent of the label entity must have TextColor component");

        Mut::map_unchanged(span, |TextColor(v)| v).set_if_neq(color);
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
                b.spawn((TextSpan::new(String::new()), LabelSpan));
            }
        });

        ChildWriter(
            children,
            DynamicTextWriterForEntity {
                entity:           self.entity,
                commands:         self.commands,
                children_query:   self.children_query,
                text_span_query:  self.text_span_query,
                text_color_query: self.text_color_query,
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
