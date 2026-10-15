#[derive(Clone, Default)]
pub struct Analytics;

impl Analytics {
    pub(crate) fn identify(&mut self, _account: &str) {}

    pub(crate) fn login(&mut self, _account: &str) {}

    pub(crate) fn ended(&self) {}
}
