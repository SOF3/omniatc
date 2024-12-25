use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::math::{Quat, Vec2, Vec3};
use bevy::prelude::{
    BuildChildren, Camera, Camera2d, ChildBuild, Commands, Component, GlobalTransform,
    IntoSystemConfigs, Mesh, Mesh2d, Rectangle, Res, ResMut, Single, Transform, Visibility, With,
    Without,
};
use bevy::render::camera::ViewportConversionError;
use bevy::sprite::{AlphaMode2d, ColorMaterial, MeshMaterial2d};
use bevy::text::Text2d;
use bevy::window::Window;

use super::Config;
use crate::ui::{SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Startup, setup_scale_ruler_system);
        app.add_systems(app::Update, maintain_scale_ruler_system.in_set(SystemSets::RenderMove));
    }
}

fn setup_scale_ruler_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let square = meshes.add(Rectangle::new(1., 1.));
    let material = materials.add(ColorMaterial {
        color:      Color::WHITE,
        alpha_mode: AlphaMode2d::Opaque,
        texture:    None,
    });
    commands
        .spawn((
            ScaleRulerBody,
            Transform::IDENTITY,
            Visibility::Visible,
            bevy::core::Name::new("ScaleRulerBody"),
        ))
        .with_children(|b| {
            b.spawn((
                Transform {
                    translation: Vec3::new(0.25, 0., 0.),
                    rotation:    Quat::IDENTITY,
                    scale:       Vec3::new(0.5, 1., 1.),
                },
                Mesh2d(square.clone()),
                MeshMaterial2d(material.clone()),
            ));
            b.spawn((
                Transform {
                    translation: Vec3::new(0.75, 0.5, 0.),
                    rotation:    Quat::IDENTITY,
                    scale:       Vec3::new(0.5, 0.1, 1.),
                },
                Mesh2d(square.clone()),
                MeshMaterial2d(material.clone()),
            ));
            b.spawn((
                Transform {
                    translation: Vec3::new(0.75, -0.5, 0.),
                    rotation:    Quat::IDENTITY,
                    scale:       Vec3::new(0.5, 0.1, 1.),
                },
                Mesh2d(square.clone()),
                MeshMaterial2d(material.clone()),
            ));
            b.spawn((
                Transform {
                    translation: Vec3::new(1., 0., 0.),
                    rotation:    Quat::IDENTITY,
                    scale:       Vec3::new(0.1, 1., 1.),
                },
                Mesh2d(square),
                MeshMaterial2d(material),
                ScaleRulerProximalEdge,
            ));
        });
    commands.spawn((
        ScaleRulerLeftText,
        Text2d::new("0"),
        bevy::core::Name::new("ScaleRulerLeftText"),
    ));
    commands.spawn((
        ScaleRulerRightText,
        Text2d::new(""),
        bevy::core::Name::new("ScaleRulerRightText"),
    ));
}

#[derive(Component)]
struct ScaleRulerBody;

#[derive(Component)]
struct ScaleRulerLeftText;

#[derive(Component)]
struct ScaleRulerRightText;

#[derive(Component)]
struct ScaleRulerProximalEdge;

fn maintain_scale_ruler_system(
    config: Res<Config>,
    camera_query: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    window: Single<&Window>,
    mut ruler_body: Single<
        (&mut Visibility, &mut Transform),
        (With<ScaleRulerBody>, Without<ScaleRulerLeftText>, Without<ScaleRulerRightText>),
    >,
    mut ruler_left: Single<
        (&mut Visibility, &mut Transform),
        (With<ScaleRulerLeftText>, Without<ScaleRulerBody>, Without<ScaleRulerRightText>),
    >,
    mut ruler_right: Single<
        (&mut Visibility, &mut Transform, &mut Text2d),
        (With<ScaleRulerRightText>, Without<ScaleRulerBody>, Without<ScaleRulerLeftText>),
    >,
    mut ruler_proximal_edge: Single<
        &mut Transform,
        (
            With<ScaleRulerProximalEdge>,
            Without<ScaleRulerRightText>,
            Without<ScaleRulerBody>,
            Without<ScaleRulerLeftText>,
        ),
    >,
) {
    let vis = if config.ruler.is_some() { Visibility::Visible } else { Visibility::Hidden };
    *ruler_body.0 = vis;
    *ruler_left.0 = vis;
    *ruler_right.0 = vis;

    let Some(ruler_config) = &config.ruler else { return };

    let (camera, camera_tf) = *camera_query;

    let distal_window_pos = Vec2::new(
        if ruler_config.pos.x >= 0. {
            ruler_config.pos.x
        } else {
            window.width() + ruler_config.pos.x
        },
        if ruler_config.pos.y >= 0. {
            ruler_config.pos.y
        } else {
            window.height() + ruler_config.pos.y
        },
    );
    let ruler_default_window_width = 100f32.max(window.width() * ruler_config.base_width_ratio);

    let ruler_width_window_offset =
        Vec2::new(ruler_default_window_width * ruler_config.pos.x.signum(), 0.);
    let ruler_text_height_offset = Vec2::new(
        0.,
        if ruler_config.pos.y > 0. { 1. } else { -1. }
            * (ruler_config.height + ruler_config.label_padding),
    );

    let [distal_world_pos, sample_world_pos, distal_world_text_pos] = match (|| {
        Ok::<_, ViewportConversionError>([
            camera.viewport_to_world_2d(camera_tf, distal_window_pos)?,
            camera
                .viewport_to_world_2d(camera_tf, distal_window_pos + ruler_width_window_offset)?,
            camera.viewport_to_world_2d(camera_tf, distal_window_pos + ruler_text_height_offset)?,
        ])
    })() {
        Ok(v) => v,
        Err(err) => {
            bevy::log::error!("get viewport scale: {err:?}");
            return;
        }
    };

    let sample_distance = distal_world_pos.distance(sample_world_pos);
    let log2 = sample_distance.log2();
    #[allow(clippy::cast_possible_truncation)] // log2 output is within bounds
    let ruler_distance = if (log2 % 1. + 1.) % 1. < 0.5 {
        2f32.powi(log2.floor() as i32)
    } else {
        2f32.powi(log2.ceil() as i32)
    };
    let proximal_pos = distal_world_pos
        + (sample_world_pos - distal_world_pos) * (ruler_distance / sample_distance);
    let proximal_text_pos = proximal_pos + (distal_world_text_pos - distal_world_pos);

    let (left_text_pos, right_text_pos) = if ruler_config.pos.x > 0. {
        (distal_world_text_pos, proximal_text_pos)
    } else {
        (proximal_text_pos, distal_world_text_pos)
    };

    for (tf, pos) in [(&mut *ruler_left.1, left_text_pos), (&mut *ruler_right.1, right_text_pos)] {
        tf.translation = (pos, Zorder::ScaleRulerLabel.to_z()).into();
        tf.rotation = camera_tf.rotation();
        tf.scale = camera_tf.scale() * 0.6;
    }

    ruler_right.2 .0 = format!("{ruler_distance} nm");

    ruler_body.1.translation = (distal_world_pos, Zorder::ScaleRuler.to_z()).into();
    ruler_body.1.rotation = camera_tf.rotation();
    ruler_body.1.scale.x = ruler_distance * ruler_config.pos.x.signum();
    ruler_body.1.scale.y = camera_tf.scale().y * ruler_config.height;

    ruler_proximal_edge.scale.x = ruler_body.1.scale.x.recip() * camera_tf.scale().x;
}
