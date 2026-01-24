//! When a quest with these components is the first active quest,
//! highlight corresponding UI elements.

use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;

#[derive(Bundle)]
pub struct All(RadarView, SetAltitude, SetSpeed, SetHeading);

/// The main radar viewport.
#[derive(Component)]
pub struct RadarView;

/// Camera rotation control.
#[derive(Component)]
pub struct SetCameraRotation;

/// Camera zoom control.
#[derive(Component)]
pub struct SetCameraZoom;

/// UI for setting the altitude target.
#[derive(Component)]
pub struct SetAltitude;

/// UI for setting the speed target.
#[derive(Component)]
pub struct SetSpeed;

/// UI for setting the heading target.
#[derive(Component)]
pub struct SetHeading;
