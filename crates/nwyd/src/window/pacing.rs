use web_time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Pacing {
    last_redraw: Option<Instant>,
    rendered_this_turn: bool,
    retry_after: Option<Instant>,
    loading_after: Option<Instant>,
}

pub(super) struct Conditions {
    pub wayland: bool,
    pub focused: bool,
    pub occluded: bool,
    pub minimized: bool,
    pub drawable: bool,
    pub mode: wgpu::PresentMode,
}

impl Pacing {
    pub fn loading_wait(&self, now: Instant, pending: bool) -> Option<Instant> {
        self.loading_after.filter(|&at| pending && now < at)
    }

    pub fn loading_completed(&mut self, now: Instant, pending: bool) {
        self.loading_after = pending.then_some(now + Duration::from_millis(2));
    }

    pub fn new_turn(&mut self) {
        self.rendered_this_turn = false;
    }

    pub fn redraw(&mut self, now: Instant) {
        self.last_redraw = Some(now);
    }

    pub fn presented(&mut self, now: Instant, success: bool) {
        self.rendered_this_turn = true;

        self.retry_after = (!success).then_some(now + Duration::from_millis(16));
    }

    pub fn invalidate_visibility(&mut self) {
        self.last_redraw = None;
    }

    pub fn uncapped(&self, now: Instant, c: &Conditions) -> bool {
        c.wayland
            && c.focused
            && !c.occluded
            && !c.minimized
            && c.drawable
            && matches!(
                c.mode,
                wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox
            )
            && self
                .last_redraw
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(100))
            && self.retry_after.is_none_or(|retry| now >= retry)
    }

    pub fn needs_frame(&self) -> bool {
        !self.rendered_this_turn
    }
}

pub(super) fn backend(window: &winit::window::Window) -> &'static str {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

    match window.display_handle().map(|h| h.as_raw()) {
        Ok(RawDisplayHandle::Wayland(_)) => "wayland",
        Ok(RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_)) => "x11",
        Ok(RawDisplayHandle::Windows(_)) => "windows",
        Ok(RawDisplayHandle::AppKit(_)) => "appkit",
        _ => "other",
    }
}
