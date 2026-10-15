use super::Message;
use crate::scene::enter::Entered;

#[derive(Clone, Default)]
pub struct Analytics;

impl Analytics {
    pub(crate) fn identify(&mut self, _account: &str) {}

    pub(crate) fn login(&mut self, _account: &str) {}

    pub(crate) fn ended(&self) {}

    pub(crate) fn level_up(&self, _character: &Entered) {}

    pub(crate) fn message(&self, _character: &Entered, _message: &Message) {}

    pub(crate) fn item_actioned(&self, _character: &Entered, _id: i32, _name: &str) {}

    pub(crate) fn item_collected(&self, _character: &Entered, _name: &str, _amount: i32) {}
}
