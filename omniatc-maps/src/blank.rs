use crate::demo;

/// A blank map for tests based on the demo map geometry, with no objects or tutorials.
#[must_use]
pub fn file() -> store::File {
    let mut map = demo::file();
    map.meta = store::Meta {
        id:          "omniatc.blank".into(),
        title:       "Blank".into(),
        description: "Blank map for tests".into(),
        authors:     ["omniatc".into()].into(),
        tags:        [("region", "fictional"), ("source", "builtin"), ("type", "scenario")]
            .into_iter()
            .map(|(k, v)| (String::from(k), String::from(v)))
            .collect(),
    };
    map.level.spawn_sets = [].into();
    map.level.spawn_trigger = store::SpawnTrigger::Disabled;
    map.quests = store::QuestTree::default();
    map.objects = [].into();
    map
}
