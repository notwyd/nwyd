use super::{Actor, ActorKey, Model, Snapshot};
use std::collections::HashMap;

#[derive(Debug, Default)]

pub(crate) struct OwnerIndex {
    keyed: HashMap<ActorKey, usize>,
    entities: HashMap<u16, usize>,
}

impl OwnerIndex {
    pub fn prepare<'a>(&'a mut self, actors: &'a [Actor], models: &'a [Model]) -> Owners<'a> {
        self.keyed.clear();

        self.entities.clear();

        for (index, actor) in actors.iter().enumerate() {
            self.keyed.entry(ActorKey::of(actor)).or_insert(index);

            self.entities.entry(actor.entity).or_insert(index);
        }

        Owners {
            index: self,
            actors,
            models,
        }
    }
}

pub(crate) struct Owners<'a> {
    index: &'a OwnerIndex,
    actors: &'a [Actor],
    models: &'a [Model],
}

impl Owners<'_> {
    pub fn actor(&self, key: ActorKey) -> Option<&Actor> {
        self.actors.get(*self.index.keyed.get(&key)?)
    }

    pub fn entity(&self, entity: u16) -> Option<&Actor> {
        self.actors.get(*self.index.entities.get(&entity)?)
    }

    pub fn snapshot(&self, key: ActorKey) -> Option<Snapshot> {
        self.actor(key)
            .map(|actor| super::snapshot(actor, self.models))
    }

    pub fn bone(&self, key: ActorKey, bone: usize) -> Option<render::camera::Matrix4> {
        super::actor_bone(self.actor(key)?, self.models, bone)
    }
}
