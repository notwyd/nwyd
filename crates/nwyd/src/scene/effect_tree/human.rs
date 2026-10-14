use super::{Effect, OwnedGroup};
use crate::scene::effect_context::ActorKey;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Control {
    OwnerHidden,
    QualityHidden,
    DrawVisibility(bool),
}

#[derive(Debug, Default)]
pub(super) struct Controls {
    actors: HashMap<ActorKey, Control>,
    rotors: HashMap<u16, Control>,
}

impl Controls {
    pub fn fill(&mut self, controls: impl IntoIterator<Item = (ActorKey, Control)>) {
        self.actors.clear();

        self.rotors.clear();

        for (key, control) in controls {
            self.actors.insert(key, control);

            self.rotors.insert(key.entity, control);
        }
    }

    pub fn apply(&self, nodes: &mut [Box<dyn Effect>]) {
        if self.actors.is_empty() {
            return;
        }

        for node in nodes {
            let control = match node.owned_group() {
                Some(OwnedGroup::HumanRotors(id)) => self.rotors.get(&id),
                Some(OwnedGroup::HumanAffect(key, 0x858 | 0x85C)) => self.actors.get(&key),
                Some(OwnedGroup::HumanAffect(key, 0x830 | 0x834)) => self
                    .actors
                    .get(&key)
                    .filter(|c| matches!(c, Control::QualityHidden)),
                _ => None,
            };

            match control {
                Some(Control::OwnerHidden | Control::QualityHidden) => node.human_owner_hidden(),
                Some(Control::DrawVisibility(visible)) => node.human_draw_visibility(*visible),
                None => {}
            }
        }
    }
}
