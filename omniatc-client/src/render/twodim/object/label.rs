use bevy::color::Color;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{self, QueryData, QueryEntityError};
use bevy::ecs::system::{Commands, EntityCommands, Query, SystemParam};
use bevy::text::{TextColor, TextSpan};
use omniatc_core::level::object::{self};

use super::Conf;

#[derive(Component)]
#[relationship(relationship_target = HasLabel)]
pub struct IsLabelOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = IsLabelOf, linked_spawn)]
pub struct HasLabel(Entity);

#[derive(Component)]
struct Span;

#[derive(QueryData)]
pub struct ObjectData {
    label_entity: &'static HasLabel,
    display:      &'static object::Display,
}

impl ObjectDataItem<'_> {
    pub fn write_label(&self, _conf: &Conf, label_writer: &mut Writer) {
        label_writer.rewrite(self.label_entity.0, |mut s| {
            s.write(&self.display.name);
        });
    }
}

#[derive(SystemParam)]
pub struct Writer<'w, 's> {
    owner_query: Query<'w, 's, &'static Children, query::With<IsLabelOf>>,
    span_query:  Query<'w, 's, SpanData, query::With<Span>>,
    commands:    Commands<'w, 's>,
}

impl Writer<'_, '_> {
    fn rewrite(&mut self, owner_entity: Entity, writer: impl FnOnce(WriterScope)) {
        let mut children = match self.owner_query.get(owner_entity) {
            Ok(children) => &children[..],
            Err(QueryEntityError::QueryDoesNotMatch(..)) => &[],
            Err(err) => {
                bevy::log::error!("Dangling HasLabel reference: {err:?}");
                return;
            }
        };

        {
            let scope = WriterScope {
                parent_entity: owner_entity,
                children:      &mut children,
                span_query:    &mut self.span_query,
                commands:      &mut self.commands,
            };
            writer(scope);
        }
        for &child in children {
            self.commands.entity(child).despawn();
        }
    }
}

struct WriterScope<'w, 's, 'local, 'children> {
    parent_entity: Entity,
    children:      &'local mut &'children [Entity],
    span_query:    &'local mut Query<'w, 's, SpanData, query::With<Span>>,
    commands:      &'local mut Commands<'w, 's>,
}

#[derive(QueryData)]
#[query_data(mutable)]
struct SpanData {
    span:  &'static mut TextSpan,
    color: &'static mut TextColor,
}

enum TextStyler<'w, 'a> {
    FromQuery(SpanDataItem<'w>),
    NewBundle { bundle: Option<SpanBundle>, commands: EntityCommands<'a> },
}

impl TextStyler<'_, '_> {
    fn color(mut self, color: Color) -> Self {
        match self {
            Self::FromQuery(ref mut data) => &mut *data.color,
            Self::NewBundle { ref mut bundle, .. } => {
                &mut bundle.as_mut().expect("only removed on drop").color
            }
        }
        .0 = color;
        self
    }
}

#[derive(Bundle)]
struct SpanBundle {
    span:    TextSpan,
    color:   TextColor,
    _marker: Span,
}

impl Drop for TextStyler<'_, '_> {
    fn drop(&mut self) {
        if let Self::NewBundle { bundle, commands } = self {
            commands.with_child(bundle.take().expect("only removed on drop"));
        }
    }
}

impl WriterScope<'_, '_, '_, '_> {
    fn write(&mut self, text: impl AsRef<str> + Into<String>) -> TextStyler {
        if let Some(&child) = self.children.split_off_first() {
            let mut data =
                self.span_query.get_mut(child).expect("invalid reference to label child span");
            text.as_ref().clone_into(&mut data.span.0);
            TextStyler::FromQuery(data)
        } else {
            TextStyler::NewBundle {
                bundle:   Some(SpanBundle {
                    span:    TextSpan::new(text),
                    color:   TextColor::default(),
                    _marker: Span,
                }),
                commands: self.commands.entity(self.parent_entity),
            }
        }
    }
}
