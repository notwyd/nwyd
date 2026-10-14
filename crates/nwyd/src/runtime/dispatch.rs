use crate::net::Transition;
use protocol::transport::Event;
use std::ops::ControlFlow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum PacketFlow {
    Continue,
    RetireSocket,
    Exit,
}

#[derive(Default)]

pub struct BatchEffects {
    needs_upload: bool,
}

impl BatchEffects {
    pub fn request_upload(&mut self) {
        self.needs_upload = true;
    }
}

pub trait DispatchHost {
    fn flush_outbound(&mut self) {}

    fn receive_event(&mut self, event: Event) -> Option<Transition>;

    fn before_transition(&mut self);

    fn apply_transition(
        &mut self,
        transition: Transition,
        effects: &mut BatchEffects,
    ) -> PacketFlow;

    fn upload(&mut self);
}

pub fn drain(
    host: &mut impl DispatchHost,
    events: impl IntoIterator<Item = Event>,
) -> ControlFlow<()> {
    let mut effects = BatchEffects::default();

    host.flush_outbound();

    for event in events {
        let Some(transition) = host.receive_event(event) else {
            continue;
        };

        host.before_transition();

        let flow = host.apply_transition(transition, &mut effects);

        host.flush_outbound();

        match flow {
            PacketFlow::Continue => {}
            PacketFlow::RetireSocket => break,
            PacketFlow::Exit => return ControlFlow::Break(()),
        }
    }

    if effects.needs_upload {
        host.upload();
    }

    ControlFlow::Continue(())
}
