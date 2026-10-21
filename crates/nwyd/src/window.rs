use crate::clock::Clock;
use crate::frame::SceneRenderer;
use crate::net::{self, Session};
use crate::overlay::Overlay;
use crate::scene::Scene;
use crate::scene::audio::SceneAudio;
use crate::scene::field::{self, Transition};
use crate::scene::rng::Lcg;
use crate::scene::{char_listener, listener};
use anyhow::{Context, Result, anyhow};
use diagnostics::{FrameDiagnostics, FrameProbe};
use render::blit::{BlitSource, Blitter, Fit};
use render::device::Gpu;
use render::target::{COLOR_FORMAT, Offscreen};
use std::sync::Arc;
use ui::input::Mouse;
use ui::text_entry::{Key as EditKey, Mods};
use winit::application::ApplicationHandler;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Fullscreen;
use winit::window::{CustomCursor, Window, WindowId};

mod actor_packets;
mod combat_packets;
mod commerce_packets;
mod departure;
mod diagnostics;
mod dispatch;
mod entry;
mod environment_packets;
mod field_packets;
mod game_settings;
mod ime;
mod interaction_packets;
mod inventory_packets;
#[cfg(test)]
mod keyboard_tests;
mod lifecycle;
mod live_packets;
mod message_packets;
mod object_packets;
mod pacing;
#[cfg(not(target_arch = "wasm32"))]
mod release_updates;
mod selection;
mod session_packets;
mod social_packets;
mod surface;
#[cfg(any(target_arch = "wasm32", test))]
mod touch;
#[cfg(test)]
mod touch_tests;
mod update;

pub use crate::runtime::screens::Screens;

pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

#[derive(Debug, Clone)]
pub struct Settings {
    pub title: String,
    pub backbuffer: (u32, u32),
    pub fullscreen: bool,
    pub present_mode: wgpu::PresentMode,
    pub renderer: Option<render::device::RendererBackend>,
    pub overlay: bool,
    pub pointers: crate::scene::cursor::Pointers,
}

struct Live {
    window_backend: &'static str,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    back_buffer: Offscreen,
    renderer: SceneRenderer,
    timings: Option<render::timing::GpuTimings>,
    neural_failed: bool,
    blitter: Blitter,
    source: BlitSource,
    stale: bool,
    recovery_retry_at: Option<web_time::Instant>,
    normal_cursor: Option<CustomCursor>,
    alternate_cursor: Option<CustomCursor>,
    ime_cursor_area: ime::CursorAreaCache,
}

pub fn upload_scene_renderer(
    gpu: &Gpu,
    scene: &Scene,
    overlay: Option<&Overlay>,
    size: (u32, u32),
    neural: bool,
) -> SceneRenderer {
    let mut renderer =
        if cfg!(feature = "neural-rendering") && neural && gpu.supports_neural_rendering() {
            SceneRenderer::new_with_neural(gpu, COLOR_FORMAT, size)
        } else {
            SceneRenderer::new(gpu, COLOR_FORMAT)
        };

    scene.upload(gpu, &mut renderer);

    if let Some(overlay) = overlay {
        renderer.bind_overlay(gpu, overlay.atlas());
    }

    renderer
}

#[cfg(target_arch = "wasm32")]
type BrowserDevice = (wgpu::Surface<'static>, Gpu);

#[cfg(target_arch = "wasm32")]
type BrowserRecovery = std::rc::Rc<std::cell::RefCell<Option<Result<BrowserDevice>>>>;

pub struct Client {
    #[cfg(not(target_arch = "wasm32"))]
    updates: release_updates::State,
    #[cfg(target_arch = "wasm32")]
    browser_start: Option<(web_sys::HtmlCanvasElement, BrowserDevice)>,
    #[cfg(target_arch = "wasm32")]
    browser_recovery: Option<BrowserRecovery>,
    runtime: crate::runtime::state::RuntimeState,
    settings: Settings,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    nell_browser: Option<crate::nell_browser::Browser>,
    live: Option<Live>,
    frames: u64,
    diagnostics: FrameDiagnostics,
    fit: Option<Fit>,
    overlay: Option<Overlay>,
    pacing: pacing::Pacing,
    last_frame_at: Option<web_time::Instant>,
    render_rate: crate::scene::lod::FrameRate,
    render_quality: i16,
    fit_warned: bool,
    occluded: bool,
    frame_deadline: FrameDeadline,
    resize_attempts: u8,
    failure: Option<anyhow::Error>,
    error_dialogs: bool,
    audio: Option<SceneAudio>,
    pointer: (f64, f64),
    pointer_location: crate::scene::cursor::Location,
    #[cfg(any(target_arch = "wasm32", test))]
    primary_touch: Option<(winit::event::DeviceId, u64)>,
    screens: Option<Box<dyn Screens>>,
    launcher_login: bool,
    launcher_credentials: Option<formats::launcher::Credentials>,
    picked: Option<listener::Picked>,
    closing_since: Option<crate::clock::GameTime>,
}

pub const CLOSE_GRACE_MS: u32 = field::WINDOW_CLOSE_REPEAT_MS;

const RESIZE_ATTEMPTS: u8 = 2;

const HIDDEN_FRAME: std::time::Duration = std::time::Duration::from_millis(16);

struct FrameDeadline {
    next: web_time::Instant,
}

impl FrameDeadline {
    fn new(now: web_time::Instant) -> Self {
        Self { next: now }
    }

    fn due(&self, now: web_time::Instant) -> bool {
        now >= self.next
    }

    fn completed(&mut self, now: web_time::Instant) {
        self.next = now + HIDDEN_FRAME;
    }
}

impl Client {
    fn pointer_motion(&mut self, dx: f64, dy: f64) {
        self.runtime.input.mouse.dx =
            self.runtime.input.mouse.dx.saturating_add(
                (dx as i32).saturating_mul(invert(self.runtime.input.invert_mouse)),
            );

        self.runtime.input.mouse.dy = self.runtime.input.mouse.dy.saturating_add(dy as i32);
    }

    pub fn with_render_quality(mut self, quality: i16, adaptive: bool) -> Self {
        self.render_quality = quality.min(2);

        self.render_rate = if adaptive {
            crate::scene::lod::FrameRate::starting_at(self.runtime.clock.tick_ms())
        } else {
            crate::scene::lod::FrameRate::frozen()
        };

        self
    }

    pub fn new(settings: Settings, scene: Scene) -> Client {
        let overlay = match Overlay::bundled() {
            Ok(overlay) => Some(overlay),
            Err(error) => {
                tracing::info!(
                    "NWYD: the debug overlay's bundled face did not load ({error}); \
                           continuing without it"
                );

                None
            }
        };

        Client {
            #[cfg(not(target_arch = "wasm32"))]
            updates: release_updates::State::default(),
            #[cfg(target_arch = "wasm32")]
            browser_start: None,
            #[cfg(target_arch = "wasm32")]
            browser_recovery: None,
            render_rate: crate::scene::lod::FrameRate::starting_at(0),
            render_quality: 2,
            overlay,
            pacing: pacing::Pacing::default(),
            settings,
            live: None,
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            nell_browser: None,
            frames: 0,
            diagnostics: FrameDiagnostics::from_env(),
            fit: None,
            last_frame_at: None,
            fit_warned: false,
            occluded: false,
            frame_deadline: FrameDeadline::new(web_time::Instant::now()),
            resize_attempts: RESIZE_ATTEMPTS,
            failure: None,
            error_dialogs: true,
            audio: None,
            pointer: (0.0, 0.0),
            pointer_location: Default::default(),
            #[cfg(any(target_arch = "wasm32", test))]
            primary_touch: None,
            screens: None,
            launcher_login: false,
            launcher_credentials: None,
            picked: None,
            closing_since: None,
            runtime: crate::runtime::state::RuntimeState::new(
                scene,
                Clock::new(),
                Lcg::new(
                    web_time::SystemTime::now()
                        .duration_since(web_time::UNIX_EPOCH)
                        .map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32),
                ),
            ),
        }
    }

    pub fn with_inverted_mouse(mut self, invert: bool) -> Client {
        self.runtime.input.invert_mouse = invert;

        self
    }

    pub fn with_screens(mut self, screens: Box<dyn Screens>) -> Client {
        self.screens = Some(screens);

        self
    }

    pub fn with_session(mut self, session: Session) -> Client {
        self.runtime.net = session;

        self
    }

    pub fn with_launcher(mut self, credentials: formats::launcher::Credentials) -> Client {
        if credentials.enabled() {
            self.launcher_login = true;

            self.launcher_credentials = Some(credentials);

            self.runtime.scene.set_launcher_login(true);

            match &mut self.runtime.scene {
                Scene::LoginPanels(scene) => {
                    scene.launcher_login = true;

                    scene.launcher_handoff = true;
                }
                Scene::ServerSelect(scene) => {
                    scene.panels.launcher_login = true;

                    scene.panels.launcher_handoff = true;
                }
                _ => {}
            }
        }

        self
    }

    fn route_mouse(&mut self, message: Mouse) {
        if self.settings_visible() {
            return;
        }

        let Some((x, y)) = self.pointer_in_backbuffer() else {
            return;
        };

        let now = self.runtime.clock.game_time();

        if matches!(self.runtime.scene, Scene::SelectChar(_)) {
            self.route_character_mouse(message, x, y, now, self.runtime.clock.wire_tick());

            self.redraw();

            return;
        }

        match &mut self.runtime.scene {
            Scene::LoginPanels(scene) => scene.launcher_login = self.launcher_login,
            Scene::ServerSelect(scene) => {
                scene.panels.launcher_login = self.launcher_login;
            }
            _ => {}
        }

        let events = self.runtime.scene.mouse(message, x, y);

        if !events.is_empty() {
            for raised in self.runtime.scene.handle(&events, now) {
                self.raised(raised);
            }
        }

        self.open_requested_game_settings();

        self.redraw();
    }

    fn route_key(
        &mut self,
        physical: &winit::keyboard::PhysicalKey,
        logical: &winit::keyboard::Key,
        text: Option<&str>,
        state: ElementState,
        repeat: bool,
    ) {
        let text = match logical {
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => Some("\u{8}"),
            _ => text,
        };

        if matches!(
            logical, winit::keyboard::Key::Character(key) if key
            .eq_ignore_ascii_case("s")
        ) && self.runtime.input.mods.ctrl
            && self.runtime.input.mods.shift
            && !self.runtime.input.mods.alt
        {
            if state == ElementState::Pressed && !repeat {
                self.open_game_settings();
            }

            return;
        }

        if matches!(
            logical, winit::keyboard::Key::Character(key) if key
            .eq_ignore_ascii_case("d")
        ) && self.runtime.input.mods.ctrl
            && self.runtime.input.mods.shift
            && !self.runtime.input.mods.alt
        {
            if state == ElementState::Pressed && !repeat {
                self.settings.overlay = !self.settings.overlay;

                self.redraw();
            }

            return;
        }

        if self
            .runtime
            .game_settings
            .as_mut()
            .is_some_and(|p| p.key(logical, state == ElementState::Pressed))
        {
            self.redraw();

            return;
        }

        if state == ElementState::Pressed {}

        if self.runtime.scene.legacy_modal()
            && !matches!(self.runtime.scene, Scene::Field(_))
            && !self.runtime.scene.server_modern_keyboard()
        {
            self.runtime.scene.cancel_legacy_presses();

            self.sync_ime();

            if state == ElementState::Pressed {
                if *logical == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) {
                    self.runtime.scene.legacy_character('\r');
                } else if *logical == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    self.runtime.scene.legacy_character('\u{1b}');
                } else if let Some(text) = text {
                    for ch in text.chars() {
                        self.runtime.scene.legacy_character(ch);
                    }
                }

                self.redraw();
            }

            return;
        }

        if self.runtime.ime.composing {
            return;
        }

        if state == ElementState::Pressed {
            crate::scene::combo_box::key_prefix(&mut self.runtime.scene, pane_virtual_key(logical));
        }

        if state == ElementState::Pressed && matches!(self.runtime.scene, Scene::SelectChar(_)) {
            let mut consumed = false;

            self.route_character_ui(
                self.runtime.clock.game_time(),
                self.runtime.clock.wire_tick(),
                |scene, rng, _| match char_listener::modal_key(
                    scene,
                    pane_virtual_key(logical) as u16,
                    rng,
                ) {
                    Some(raised) => {
                        consumed = true;

                        raised
                    }
                    None => Vec::new(),
                },
            );

            if consumed {
                self.sync_ime();

                self.redraw();

                return;
            }
        }

        if *logical == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) {
            let down = state == ElementState::Pressed;

            let now_ms = self.runtime.clock.game_time();

            if matches!(self.runtime.scene, Scene::SelectChar(_)) {
                if let Some(events) =
                    crate::scene::combo_box::checkable_key(&mut self.runtime.scene, down)
                {
                    self.route_character_ui(
                        now_ms,
                        self.runtime.clock.wire_tick(),
                        |scene, rng, ctx| char_listener::widget_events(scene, &events, rng, ctx),
                    );

                    self.redraw();

                    return;
                }
            } else if self.runtime.scene.widget_space(down, now_ms) {
                self.redraw();

                return;
            }
        }

        if state != ElementState::Pressed {
            return;
        }

        let plan = key_plan(physical, logical, repeat);

        let combo_vk = if *logical == winit::keyboard::Key::Named(winit::keyboard::NamedKey::F4) {
            Some(0x73)
        } else {
            plan.edit.and_then(crate::scene::combo_box::virtual_key)
        };

        let now_ms = self.runtime.clock.game_time();

        if let Some(vk) = combo_vk {
            if matches!(self.runtime.scene, Scene::SelectChar(_)) {
                if let Some(events) =
                    crate::scene::combo_box::key(&mut self.runtime.scene, vk, repeat)
                {
                    self.route_character_ui(
                        now_ms,
                        self.runtime.clock.wire_tick(),
                        |scene, rng, ctx| char_listener::widget_events(scene, &events, rng, ctx),
                    );

                    self.redraw();

                    return;
                }
            } else if self.runtime.scene.widget_key(vk, repeat, now_ms) {
                self.redraw();

                return;
            }
        }

        if let Some(tab) = inventory_key_tab(logical, repeat)
            && !self.runtime.scene.legacy_modal()
            && let Scene::Field(scene) = &mut self.runtime.scene
        {
            crate::scene::field::inventory_key(scene, tab);
        }

        if let Some(key) = plan.edit {
            if matches!(self.runtime.scene, Scene::SelectChar(_)) && key == EditKey::Insert {
                self.route_character_ui(now_ms, self.runtime.clock.wire_tick(), |scene, _, _| {
                    char_listener::time_key(scene, key, repeat)
                });
            }

            let raised = self
                .runtime
                .scene
                .key_event(key, self.runtime.input.mods, repeat);

            if raised.contains(&ui::text_entry::Raised::Submit) {
                self.submit();
            }
        }

        if plan.recall && !self.runtime.scene.legacy_modal() {
            let _ = self.runtime.scene.recall();
        }

        if let Some(vk) = plan.numpad
            && !self.runtime.scene.legacy_modal()
        {
            self.runtime.scene.numpad(vk);
        }

        if plan.character
            && let Some(text) = text
        {
            for ch in text.chars() {
                self.runtime.scene.character(ch);
            }

            self.load_panel_icons();
        }

        self.redraw();
    }

    fn pointer_moved(&mut self, position: (f64, f64)) {
        self.pointer = (position.0, position.1);

        self.pointer_location = crate::scene::cursor::Location::Inside;

        if self.settings_blocks_pointer() {
            let point = self.pointer_in_backbuffer();

            if let Some(panel) = &mut self.runtime.game_settings {
                panel.motion(point);
            }

            self.apply_game_settings();

            self.redraw();

            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.update_notice_motion(self.pointer_in_backbuffer()) {
            return;
        }

        self.route_mouse(Mouse::Move);

        let (emissive, inside) = self.hover_inputs();

        let choice =
            self.runtime
                .input
                .pointer_state
                .on_move(self.settings.pointers.mode, emissive, inside);

        self.apply_pointer(choice);
    }

    fn pointer_left(&mut self) {
        self.pointer_location = crate::scene::cursor::Location::Outside;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(panel) = &mut self.updates.panel {
            panel.motion(None);
        }

        if let Scene::Field(scene) = &mut self.runtime.scene {
            field::cancel_pointer_hold(scene);
        }

        if let Scene::SelectChar(scene) = &mut self.runtime.scene {
            scene.pointer(None);
        }

        if let Some(scene) = self.scene_manager() {
            scene.hovered = None;
        }

        self.redraw();
    }

    fn pointer_button(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released
            && button == MouseButton::Left
            && let Scene::Field(scene) = &mut self.runtime.scene
        {
            scene.drag = None;
        }

        let point = self.pointer_in_backbuffer();

        let scale = self.panel_scale();

        if button == MouseButton::Right {
            tracing::debug!(
                target : "nwyd::equip_input", ? state, ? point, ctrl = self.runtime.input
                .mods.ctrl, shift = self.runtime.input.mods.shift, alt = self.runtime
                .input.mods.alt, settings = self.settings_visible(), overlay = self
                .panel_blocks_pointer(), "right-button window input"
            );
        }

        let index = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            _ => 3,
        };

        if self.runtime.game_settings.as_mut().is_some_and(|p| {
            p.button(
                index,
                state == ElementState::Pressed,
                point,
                self.settings.backbuffer,
                scale,
            )
        }) {
            self.apply_game_settings();

            self.redraw();

            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.update_notice_button(
            index,
            state == ElementState::Pressed,
            point,
            crate::notice_browser::open,
        ) {
            return;
        }

        if state == ElementState::Pressed {}

        if state == ElementState::Pressed && self.runtime.ime.composing {
            self.runtime.ime.cancel(&mut self.runtime.scene);
        }

        if state == ElementState::Released
            && matches!(button, MouseButton::Right | MouseButton::Middle)
        {
            let choice = self
                .runtime
                .input
                .pointer_state
                .on_select_only(self.settings.pointers.mode);

            self.apply_pointer(choice);
        }

        let plan = mouse_plan(button, state);

        if let Some(down) = plan.mmb {
            self.runtime.input.mouse.mmb = down;
        }

        if let Some(down) = plan.rmb {
            self.runtime.input.mouse.rmb = down;
        }

        if let Some(message) = plan.message {
            self.route_mouse(message);

            self.load_panel_icons();
        }
    }

    fn pointer_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        if self.settings_blocks_pointer() {
            return;
        }

        if self.panel_blocks_pointer() {
            self.redraw();

            return;
        }

        let counts = wheel_step(delta, false);

        let now_ms = self.runtime.clock.game_time();

        let consumed = if matches!(self.runtime.scene, Scene::SelectChar(_)) {
            if let Some(events) = crate::scene::combo_box::wheel(&mut self.runtime.scene, counts, 3)
            {
                self.route_character_ui(
                    now_ms,
                    self.runtime.clock.wire_tick(),
                    |scene, rng, ctx| char_listener::widget_events(scene, &events, rng, ctx),
                );

                true
            } else {
                false
            }
        } else {
            self.runtime.scene.widget_wheel(counts, 3, now_ms)
        };

        if !consumed {
            self.runtime.input.mouse.dz += wheel_step(delta, self.runtime.input.invert_mouse);
        }

        self.redraw();
    }

    fn submit(&mut self) {
        if let Scene::SelectChar(scene) = &mut self.runtime.scene
            && scene.name_dialog.visible
        {
            if let Some(mut body) = scene.submit_name(self.runtime.clock.game_time())
                && let Err(error) = self.runtime.net.send_selection(
                    &mut body,
                    "character name",
                    self.runtime.clock.wire_tick(),
                )
            {
                tracing::info!("NWYD: {error}");
            }

            self.redraw();

            return;
        }

        let events = self.runtime.scene.submit();

        if events.is_empty() {
            return;
        }

        let now = self.runtime.clock.tick_ms();

        for raised in self.runtime.scene.handle(&events, now) {
            self.raised(raised);
        }

        self.redraw();
    }

    fn raised(&mut self, raised: listener::Refusal) {
        if self.diagnostics.is_benchmark() {
            tracing::info!("NWYD: login is disabled during an offline frame benchmark");
            return;
        }

        let (account, password, launcher) = match raised {
            listener::Refusal::LoginAccepted { account, password } => (account, password, None),
            listener::Refusal::LauncherAccepted => {
                let Some(credentials) = self.launcher_credentials.clone() else {
                    tracing::warn!("NWYD: launcher login requested without a hand-off");

                    return;
                };

                (String::new(), String::new(), Some(credentials))
            }
            raised => {
                tracing::info!("NWYD: {raised:?}");

                return;
            }
        };

        let Scene::ServerSelect(scene) = &mut self.runtime.scene else {
            tracing::info!("NWYD: a login was raised by a screen that has no server picked");

            return;
        };

        let address = scene.panels.session.address.clone();

        if address.is_empty() {
            tracing::info!("NWYD: no server is selected — pick one with btnSelServer first");

            return;
        }

        self.picked = Some(listener::Picked {
            server: scene.panels.session.server.unwrap_or_default(),
            channel: scene.panels.session.channel.unwrap_or_default(),
            status: match scene.panels.status_for == scene.panels.session.server {
                true => scene.panels.status.clone(),
                false => crate::scene::status::Status::default(),
            },
        });

        let seed = self.runtime.rng.next();

        let tick = self.runtime.clock.wire_tick();

        let launcher_attempt = launcher.is_some();

        let sent = match launcher {
            Some(credentials) => {
                self.runtime
                    .net
                    .login_launcher_staged(&address, &credentials, seed, tick)
            }
            None => self
                .runtime
                .net
                .login_staged(&address, &account, &password, seed, tick),
        };

        if let Err(error) = sent {
            tracing::info!("NWYD: {error}");

            if error.fixed_launcher_owner(launcher_attempt) {
                self.runtime.scene.launcher_connection_failed();

                self.sync_ime();

                self.redraw();
            } else if let Scene::ServerSelect(scene) = &mut self.runtime.scene {
                scene.panels.login_connection_failed();

                scene.panels.message_panel.post(
                    listener::CONNECT_FAILED_STRDEF,
                    4000,
                    self.runtime.clock.game_time(),
                    true,
                );
            }

            self.show_message(listener::CONNECT_FAILED_STRDEF);
        } else {
            self.runtime.retained.name_prompt_required = false;
        }
    }

    fn route_character_mouse(&mut self, message: Mouse, x: i32, y: i32, now_ms: u32, tick: u32) {
        self.route_character_ui(now_ms, tick, |scene, rng, ctx| {
            char_listener::mouse(scene, message, x, y, rng, ctx)
        });
    }

    fn route_character_ui(
        &mut self,
        now_ms: u32,
        tick: u32,
        route: impl FnOnce(
            &mut crate::scene::select_char::SelectChar,
            &mut Lcg,
            &char_listener::Context<'_>,
        ) -> Vec<char_listener::Raised>,
    ) {
        let Client {
            screens,
            runtime:
                crate::runtime::state::RuntimeState {
                    scene,
                    net,
                    rng,
                    retained,
                    ..
                },
            ..
        } = self;

        let (Scene::SelectChar(scene), Some(screens)) = (scene, screens.as_ref()) else {
            return;
        };

        let ctx = char_listener::Context {
            game: screens.game(),
            items: screens.items(),
            backbuffer: (scene.layout.width, scene.layout.height),
            now_ms,
        };

        let raised = route(scene, rng, &ctx);

        retained.account_lock = scene.lock;

        let mut leaving = false;

        for one in raised {
            match one {
                char_listener::Raised::Send {
                    what, body, slot, ..
                } => {
                    let mut body = body;

                    let sent = match slot {
                        Some(slot) => net.select_character(slot, &mut body, tick),
                        None => net.send_selection(&mut body, what, tick),
                    };

                    if let Err(error) = sent {
                        tracing::info!("NWYD: {error}");
                    }
                }
                char_listener::Raised::Message(record) => {
                    tracing::info!(
                        "NWYD: {}",
                        scene.panels.strings.translate(&format!("ss_{record}"))
                    );
                }
                char_listener::Raised::Refused(why) => tracing::info!("NWYD: {why}"),
                char_listener::Raised::BackToServerSelect => leaving = true,
            }
        }

        if leaving {
            self.runtime.net.disconnect();

            self.show_server_select();
        }
    }

    fn show_message(&mut self, record: usize) {
        let strings = match &self.runtime.scene {
            Scene::LoginPanels(scene) => Some(&scene.strings),
            Scene::ServerSelect(scene) => Some(&scene.panels.strings),
            Scene::SelectChar(scene) => Some(&scene.panels.strings),
            Scene::Field(scene) => scene.panels.as_ref().map(|p| &p.strings),
            _ => None,
        };

        match strings.map(|s| s.translate(&format!("ss_{record}"))) {
            Some(text) if !text.starts_with("ss_") && !text.is_empty() => {
                tracing::info!("NWYD: {text}")
            }
            _ => tracing::info!("NWYD: system message {record} (no strdef.bin loaded)"),
        }
    }

    fn show(&mut self, scene: Scene) {
        crate::runtime::lifecycle::replace(self, scene);
    }

    fn stream_zones(&mut self) {
        let Client {
            runtime: crate::runtime::state::RuntimeState { scene, .. },
            screens,
            live,
            ..
        } = self;

        let game = screens.as_ref().map(|s| s.game());

        let loaded = match live.as_mut().filter(|live| !live.gpu.is_lost()) {
            Some(live) => live.renderer.stream_zones(&live.gpu, scene, game),
            None => scene.stream_zones_background(game),
        };

        if !loaded {
            return;
        }

        self.runtime.net.loaded_zone(self.runtime.clock.game_now());

        if let Some(report) = scene.zone_report() {
            tracing::info!("NWYD: {report}");
        }

        if let Some(live) = live.as_mut()
            && !live.gpu.is_lost()
        {
            scene.upload(&live.gpu, &mut live.renderer);
        }

        self.redraw();
    }

    fn flush_outbox(&mut self) {
        if let Scene::Field(scene) = &mut self.runtime.scene
            && let Some(panels) = &mut scene.panels
        {
            panels.pump_quest(&scene.entered);
        }

        self.flush_pending_outbox();
    }

    fn flush_pending_outbox(&mut self) {
        crate::runtime::outbound::flush(
            &mut self.runtime.scene,
            &mut self.runtime.net,
            &self.runtime.clock,
        );
    }

    fn flush_quick_slot_save(&mut self) {
        let Some(screens) = self.screens.as_ref() else {
            return;
        };

        let Some(bytes) = self.runtime.scene.take_quick_slot_save() else {
            return;
        };

        if let Err(error) = screens.write_quick_slots(&bytes) {
            tracing::info!("NWYD: NQuickSLot_01.bin was not saved: {error:#}");
        }
    }

    fn flush_notice_browser(&mut self) {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        self.flush_nell_browser();

        self.open_requested_game_settings();

        let community = match &mut self.runtime.scene {
            Scene::Field(scene) => scene.panels.as_mut().and_then(|p| {
                std::mem::take(&mut p.community_requested).then(|| p.strings.translate("ss_341"))
            }),
            _ => None,
        };

        if let Some(base) = community {
            let url = self.picked.as_ref().and_then(|picked| {
                crate::notice_browser::community_url(
                    &base,
                    &self.runtime.net.account,
                    picked.server,
                    self.launcher_credentials.as_ref().map_or(0, |c| c.kind),
                )
            });

            let result = url.as_deref().map_or(Ok(false), |url| {
                crate::notice_browser::dispatch_url(url, crate::notice_browser::open)
            });

            match result {
                Ok(accepted) => {
                    tracing::info!(
                        event = "browser_request",
                        source = "community",
                        accepted,
                        "NWYD: community browser request handled"
                    )
                }
                Err(_) => {
                    tracing::warn!(
                        event = "browser_request",
                        source = "community",
                        accepted = false,
                        "NWYD: could not dispatch community navigation"
                    )
                }
            }
        }

        for url in self.runtime.scene.take_registration_requests() {
            match crate::notice_browser::dispatch_url(&url, crate::notice_browser::open) {
                Ok(true) => {
                    tracing::info!(
                        event = "browser_request",
                        source = "registration",
                        accepted = true,
                        "NWYD: registration link handed to the browser adapter"
                    )
                }
                Ok(false) => {
                    tracing::warn!(
                        event = "browser_request",
                        source = "registration",
                        accepted = false,
                        "NWYD: registration link is not a permitted web URL"
                    )
                }
                Err(error) => {
                    tracing::warn!(
                        % error, event = "browser_request", source = "registration",
                        accepted = false, "NWYD: could not dispatch account registration"
                    )
                }
            }
        }

        let requested = match &mut self.runtime.scene {
            Scene::Field(scene) => scene
                .panels
                .as_mut()
                .is_some_and(|p| std::mem::take(&mut p.notice_requested)),
            _ => false,
        };

        if !requested {
            return;
        }

        let (Some(screens), Some(picked)) = (&self.screens, &self.picked) else {
            return;
        };

        let raw = match screens.game().read(b"WNotice.ini") {
            Ok(raw) => raw,
            Err(error) => {
                tracing::warn!(% error, "NWYD: could not read the manual notice");

                return;
            }
        };

        if let Err(error) =
            crate::notice_browser::dispatch(&raw, picked.server, crate::notice_browser::open)
        {
            tracing::warn!(% error, "NWYD: could not dispatch the manual notice");
        }
    }

    fn flush_music(&mut self) {
        let Some(slot) = self.runtime.scene.take_music() else {
            return;
        };

        let Some(audio) = self.audio.as_mut() else {
            return;
        };

        if !audio.switch_music(slot) {
            tracing::info!("NWYD: background music slot {slot} will not play; the field is quiet");
        }
    }

    fn show_server_select(&mut self) {
        crate::runtime::departure::show_server_select(self);
    }

    fn poll_net(&mut self) -> bool {
        let events = { self.runtime.net.drain_events() };

        if crate::runtime::dispatch::drain(self, events).is_break() {
            return true;
        }

        let idle = match self.runtime.scene {
            Scene::Field(_) => net::KEEPALIVE_FIELD_MS,
            _ => net::KEEPALIVE_SELECT_MS,
        };

        let now = self.runtime.clock.wire_tick();

        self.runtime.net.keepalive(idle, now);

        if let Scene::Field(field) = &self.runtime.scene
            && let Some(player) = field.player.and_then(|i| field.world.actors.actors.get(i))
            && let Err(error) =
                self.runtime
                    .net
                    .report_desync(player.mover.last_dest, field.wire_speed, now)
        {
            tracing::info!("NWYD: clock report not sent — {error}");
        }

        let field_body = matches!(
            & self.runtime.scene, Scene::Field(field) if field.player.and_then(| i |
            field.world.actors.actors.get(i)).is_some()
        );

        let sample = if self.runtime.clock.is_frozen() {
            None
        } else {
            crate::clock_drift::Sample::now(self.runtime.clock.tick_ms())
        };

        if let Err(error) = self
            .runtime
            .net
            .observe_clock_drift(field_body, sample, now)
        {
            tracing::info!("NWYD: local clock report not sent — {error}");
        }

        false
    }

    fn apply_packet(
        &mut self,
        transition: net::Transition,
        effects: &mut crate::runtime::dispatch::BatchEffects,
    ) -> crate::runtime::dispatch::PacketFlow {
        use crate::runtime::{
            actor_packets::{self, Packet as ActorPacket},
            combat_packets::{self, Packet as CombatPacket},
            commerce_packets::{self, Packet as CommercePacket},
            departure::{self, Packet as DeparturePacket},
            dispatch::PacketFlow,
            entry::Packet as EntryPacket,
            environment_packets::{self, Packet as EnvironmentPacket},
            field_packets::{self, Packet as FieldPacket},
            interaction_packets::{self, Packet as InteractionPacket},
            inventory_packets::{self, Packet as InventoryPacket},
            live_packets::{self, Packet as LivePacket},
            message_packets::{self, Packet as MessagePacket},
            object_packets::{self, Packet as ObjectPacket},
            selection::Packet as SelectionPacket,
            session_packets::{self, Packet as SessionPacket},
            social_packets::{self, Packet as SocialPacket},
        };

        match transition {
            net::Transition::QuestFlags(flags) => {
                session_packets::apply(self, SessionPacket::QuestFlags(flags));
            }
            net::Transition::ClientConfig(update) => {
                session_packets::apply(self, SessionPacket::ClientConfig(update));
            }
            net::Transition::ItemPrices(update) => {
                commerce_packets::apply(self, CommercePacket::Prices(update), effects);
            }
            net::Transition::DynamicKey(update) => {
                session_packets::apply(self, SessionPacket::DynamicKey(update));
            }
            net::Transition::CharacterNotice { opcode } => {
                self.apply_selection_packet(SelectionPacket::Notice { opcode });
            }
            net::Transition::SelectionMetadata(value) => {
                self.apply_selection_packet(SelectionPacket::Metadata(value));
            }
            net::Transition::CharacterDeleteHold(hold) => {
                self.apply_selection_packet(SelectionPacket::DeleteHold(hold));
            }
            net::Transition::CharacterNameNotice => {
                self.apply_selection_packet(SelectionPacket::NameNotice);
            }
            net::Transition::CharacterNameResult(reply) => {
                self.apply_selection_packet(SelectionPacket::NameResult(reply));
            }
            net::Transition::CharacterSelect { block, account } => {
                self.apply_entry_packet(EntryPacket::LoginAccepted { block, account });
            }
            net::Transition::CharacterListChanged { block } => {
                self.apply_selection_packet(SelectionPacket::AccountRefresh(block));
            }
            net::Transition::CharacterSlotEffect { slot } => {
                self.apply_selection_packet(SelectionPacket::SlotEffect { slot });
            }
            net::Transition::AccountLock { opcode } => {
                self.apply_selection_packet(SelectionPacket::AccountLock { opcode });
            }
            net::Transition::LeaveWorld { client_id } => {
                self.apply_entry_packet(EntryPacket::LeaveWorld { client_id });
            }
            net::Transition::EnterWorld(world) => {
                let received_raw_ms = self.runtime.clock.raw_now();

                self.apply_entry_packet(EntryPacket::EnterWorld {
                    world,
                    received_raw_ms,
                });
            }
            net::Transition::ServerReturnNotice => {
                session_packets::apply(self, SessionPacket::ServerReturnNotice);
            }
            net::Transition::LoginDenied { opcode } => {
                session_packets::apply(self, SessionPacket::LoginDenied { opcode });
            }
            net::Transition::Message(system) => {
                message_packets::apply(self, MessagePacket::Message(system));
            }
            net::Transition::Chat(line) => {
                message_packets::apply(self, MessagePacket::Chat(line));
            }
            net::Transition::Spawn { opcode, entity } => {
                actor_packets::apply(self, ActorPacket::Spawn { opcode, entity }, effects);
            }
            net::Transition::Action {
                opcode,
                entity,
                action,
            } => {
                actor_packets::apply(
                    self,
                    ActorPacket::Action {
                        opcode,
                        entity: crate::runtime::identity::EntityId::from_wire(entity),
                        action,
                    },
                    effects,
                );
            }
            net::Transition::Attack { opcode, attack } => {
                combat_packets::apply(self, CombatPacket::Attack { opcode, attack });
            }
            net::Transition::Kill(death) => {
                combat_packets::apply(self, CombatPacket::Kill(death));
            }
            net::Transition::Vanish { entity, vanish } => {
                actor_packets::apply(
                    self,
                    ActorPacket::Vanish {
                        entity: crate::runtime::identity::EntityId::from_wire(entity),
                        vanish,
                    },
                    effects,
                );
            }
            net::Transition::GroundItem(drop) => {
                object_packets::apply(self, ObjectPacket::GroundItem(drop), effects);
            }
            net::Transition::GateCommand(command) => {
                object_packets::apply(self, ObjectPacket::GateCommand(command), effects);
            }
            net::Transition::Remove { entity } => {
                actor_packets::apply(
                    self,
                    ActorPacket::Remove {
                        entity: crate::runtime::identity::EntityId::from_wire(entity),
                    },
                    effects,
                );
            }
            net::Transition::ShopClosed => {
                commerce_packets::apply(self, CommercePacket::ShopClosed, effects);
            }
            net::Transition::CraftingClosed => {
                interaction_packets::apply(self, InteractionPacket::CraftingClosed);
            }
            net::Transition::AnimationProbe(request) => {
                object_packets::apply(self, ObjectPacket::AnimationProbe(request), effects);
            }
            net::Transition::Question(question) => {
                interaction_packets::apply(self, InteractionPacket::Question(question));
            }
            net::Transition::LongMessage(message) => {
                message_packets::apply(self, MessagePacket::LongMessage(message));
            }
            net::Transition::ReelResult(result) => {
                interaction_packets::apply(self, InteractionPacket::ReelResult(result));
            }
            net::Transition::CloseCargo => {
                interaction_packets::apply(self, InteractionPacket::CloseCargo);
            }
            net::Transition::ItemMoved { dst, src } => {
                inventory_packets::apply(self, InventoryPacket::Moved { dst, src }, effects);
            }
            net::Transition::Sold(sale) => {
                commerce_packets::apply(self, CommercePacket::Sold(sale), effects);
            }
            net::Transition::ShopContents(contents) => {
                commerce_packets::apply(self, CommercePacket::Contents(contents), effects);
            }
            net::Transition::ShopSlotSold(sold) => {
                commerce_packets::apply(self, CommercePacket::SlotSold(sold), effects);
            }
            net::Transition::ShopStock(stock) => {
                commerce_packets::apply(self, CommercePacket::Stock(stock), effects);
            }
            net::Transition::Vitals { entity, vitals } => {
                live_packets::apply(self, LivePacket::Vitals { entity, vitals }, effects);
            }
            net::Transition::HpDelta { entity, delta } => {
                live_packets::apply(self, LivePacket::HpDelta { entity, delta }, effects);
            }
            net::Transition::Firework { entity, glyph } => {
                match self.runtime.scene.firework(entity, &glyph) {
                    Some(applied) => {
                        tracing::info!("{}", crate::scene::live::describe_firework(&applied));

                        if applied.raised {
                            self.redraw();
                        }
                    }
                    None => {
                        tracing::info!("NWYD: 0x3CA for entity {entity} arrived with no world up")
                    }
                }
            }
            net::Transition::Emote { entity, emote } => {
                match self.runtime.scene.emote(entity, &emote) {
                    Some(applied) => {
                        tracing::info!("{}", crate::scene::emote::describe(&applied));

                        if applied.played.is_some()
                            || applied.level_up
                            || matches!(
                                applied.outcome,
                                crate::scene::emote::Outcome::Firework { .. }
                            )
                        {
                            self.redraw();
                        }
                    }
                    None => {
                        tracing::info!("NWYD: 0x36A for entity {entity} arrived with no world up")
                    }
                }
            }
            net::Transition::NameMark { entity, mode } => {
                social_packets::apply(self, SocialPacket::NameMark { entity, mode });
            }
            net::Transition::GlowBurst { entity, mode } => {
                match self.runtime.scene.glow_burst(entity, mode) {
                    Some(applied) => {
                        tracing::info!("{}", crate::scene::glow_burst::describe(&applied));

                        if applied.raised {
                            self.redraw();
                        }
                    }
                    None => {
                        tracing::info!("NWYD: 0x3AD for entity {entity} arrived with no world up")
                    }
                }
            }
            net::Transition::GuildInvite { entity, invite } => {
                social_packets::apply(self, SocialPacket::Invite { entity, invite });
            }
            net::Transition::GuildApplication => {
                social_packets::apply(self, SocialPacket::Application);
            }
            net::Transition::Challenge { entity, challenge } => {
                social_packets::apply(self, SocialPacket::Challenge { entity, challenge });
            }
            net::Transition::Inventory { entity, bag } => {
                inventory_packets::apply(self, InventoryPacket::Inventory { entity, bag }, effects);
            }
            net::Transition::Collection { entity, progress } => {
                match self.runtime.scene.collection(entity, &progress) {
                    Some(applied) => {
                        tracing::info!("{}", crate::scene::collection::describe(&applied));

                        if applied.refreshed {
                            self.redraw();
                        }
                    }
                    None => {
                        tracing::info!(entity, "NWYD: 0x300 has no live Field actor owner")
                    }
                }
            }
            net::Transition::WarehouseGold(moved) => {
                inventory_packets::apply(self, InventoryPacket::WarehouseGold(moved), effects);
            }
            net::Transition::Weather(forecast) => {
                environment_packets::apply(self, EnvironmentPacket::Weather(forecast));
            }
            net::Transition::EventCountdown(countdown) => {
                environment_packets::apply(self, EnvironmentPacket::EventCountdown(countdown));
            }
            net::Transition::EventRemaining(remaining) => {
                environment_packets::apply(self, EnvironmentPacket::EventRemaining(remaining));
            }
            net::Transition::SummonRequest(request) => {
                field_packets::apply(self, FieldPacket::SummonRequest(request));
            }
            net::Transition::SummonCancel => {
                field_packets::apply(self, FieldPacket::SummonCancel);
            }
            net::Transition::EventPacked(packed) => {
                environment_packets::apply(self, EnvironmentPacket::EventPacked(packed));
            }
            net::Transition::EventTarget(target) => {
                environment_packets::apply(self, EnvironmentPacket::EventTarget(target));
            }
            net::Transition::EventScore(score) => {
                environment_packets::apply(self, EnvironmentPacket::EventScore(score));
            }
            net::Transition::ItemArrived(arrival) => {
                inventory_packets::apply(self, InventoryPacket::Arrived(arrival), effects);
            }
            net::Transition::PlateLevel(change) => {
                social_packets::apply(self, SocialPacket::Level(change));
            }
            net::Transition::PlateGuild(line) => {
                social_packets::apply(self, SocialPacket::GuildLine(line));
            }
            net::Transition::SiegeState(frame) => {
                field_packets::apply(self, FieldPacket::SiegeState(frame));
            }
            net::Transition::WarFlag(flag) => {
                field_packets::apply(self, FieldPacket::WarFlag(flag));
            }
            net::Transition::Kickout(k) => {
                return departure::apply(self, DeparturePacket::Kickout(k));
            }
            net::Transition::FieldCarpet(packet) => {
                field_packets::apply(self, FieldPacket::FieldCarpet(packet));
            }
            net::Transition::FieldSound(packet) => {
                field_packets::apply(self, FieldPacket::FieldSound(packet));
            }
            net::Transition::FieldLine(line) => {
                field_packets::apply(self, FieldPacket::FieldLine(line));
            }
            net::Transition::ArenaRound(packet) => {
                environment_packets::apply(self, EnvironmentPacket::ArenaRound(packet));
            }
            net::Transition::ArenaKill(packet) => {
                environment_packets::apply(self, EnvironmentPacket::ArenaKill(packet));
            }
            net::Transition::ArenaList(packet) => {
                environment_packets::apply(self, EnvironmentPacket::ArenaList(packet));
            }
            net::Transition::KeyedRecord(record) => {
                field_packets::apply(self, FieldPacket::KeyedRecord(record));
            }
            net::Transition::Guild(membership) => {
                social_packets::apply(self, SocialPacket::Guild(membership));
            }
            net::Transition::ItemCleared(clear) => {
                inventory_packets::apply(self, InventoryPacket::Cleared(clear), effects);
            }
            net::Transition::PartyMember { opcode, member } => {
                social_packets::apply(self, SocialPacket::Member { opcode, member });
            }
            net::Transition::PartyLeft(leave) => {
                social_packets::apply(self, SocialPacket::Left(leave));
            }
            net::Transition::PartyMarks(marks) => {
                social_packets::apply(self, SocialPacket::Marks(marks));
            }
            net::Transition::ShopSellList(list) => {
                commerce_packets::apply(self, CommercePacket::SellList(list), effects);
            }
            net::Transition::NpcLine { entity, line } => {
                message_packets::apply(self, MessagePacket::NpcLine { entity, line });
            }
            net::Transition::TradeOffer { entity, offer } => {
                commerce_packets::apply(self, CommercePacket::Offer { entity, offer }, effects);
            }
            net::Transition::TradeClosed { entity } => {
                commerce_packets::apply(self, CommercePacket::TradeClosed { entity }, effects);
            }
            net::Transition::TradeToggle { entity } => {
                commerce_packets::apply(self, CommercePacket::Toggle { entity }, effects);
            }
            net::Transition::ActorStatus { entity, score } => {
                live_packets::apply(self, LivePacket::ActorStatus { entity, score }, effects);
            }
            net::Transition::PlayerProgression { entity, score } => {
                live_packets::apply(
                    self,
                    LivePacket::PlayerProgression { entity, score },
                    effects,
                );
            }
            net::Transition::QuickBar { bar } => {
                live_packets::apply(self, LivePacket::QuickBar { bar }, effects);
            }
            net::Transition::Gold { entity, gold } => {
                live_packets::apply(self, LivePacket::Gold { entity, gold }, effects);
            }
            net::Transition::EquipChanged { entity, change } => {
                live_packets::apply(self, LivePacket::EquipChanged { entity, change }, effects);
            }
            net::Transition::SlotPatch { entity, patch } => {
                live_packets::apply(self, LivePacket::SlotPatch { entity, patch }, effects);
            }
            net::Transition::Affects { entity, affects } => {
                combat_packets::apply(self, CombatPacket::Affects { entity, affects });
            }
            net::Transition::Redirect(ticket) => {
                return departure::apply(self, DeparturePacket::Redirect(ticket));
            }
            net::Transition::Disconnected {
                reason,
                socket_closed,
            } => {
                return departure::apply(
                    self,
                    DeparturePacket::Disconnected {
                        reason,
                        socket_closed,
                    },
                );
            }
            net::Transition::Quit => {
                return departure::apply(self, DeparturePacket::Quit);
            }
        }

        PacketFlow::Continue
    }

    fn transition(&mut self, transition: Transition) -> bool {
        match transition {
            Transition::Quit => {
                tracing::info!("NWYD: the exit countdown finished — closing");

                true
            }
            Transition::ServerSelect => {
                self.runtime.net.disconnect();

                self.show_server_select();

                false
            }
        }
    }

    fn pointer_in_backbuffer(&self) -> Option<(i32, i32)> {
        let (bw, bh) = self.settings.backbuffer;

        let Some(fit) = self.fit else {
            return None;
        };

        let (px, py) = self.pointer;

        let dx = px - fit.x as f64;

        let dy = py - fit.y as f64;

        if dx < 0.0 || dy < 0.0 || dx >= fit.width as f64 || dy >= fit.height as f64 {
            return None;
        }

        let x = (dx * bw as f64 / fit.width as f64) as i32;

        let y = (dy * bh as f64 / fit.height as f64) as i32;

        Some((x.clamp(0, bw as i32 - 1), y.clamp(0, bh as i32 - 1)))
    }

    fn panel_scale(&self) -> f32 {
        self.fit.map_or_else(
            || {
                self.live.as_ref().map_or(1.0, |l| {
                    self.settings.backbuffer.0 as f32 / l.config.width.max(1) as f32
                })
            },
            |fit| self.settings.backbuffer.0 as f32 / fit.width.max(1) as f32,
        )
    }

    fn panel_blocks_pointer(&self) -> bool {
        if self.nell_browser_visible() {
            return true;
        }

        if self.settings_blocks_pointer() {
            return true;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.update_notice_blocks_pointer(self.pointer_in_backbuffer()) {
            return true;
        }

        false
    }

    fn hover_inputs(&self) -> (Option<[f32; 4]>, bool) {
        let emissive = match &self.runtime.scene {
            Scene::Field(scene) => crate::scene::cursor::hovered_emissive(scene),
            _ => crate::scene::cursor::NO_HOVER,
        };

        let inside = self.pointer_in_backbuffer().is_some_and(|at| {
            crate::scene::cursor::inside_client_rect(at, self.settings.backbuffer, (0, 0))
        });

        (emissive, inside)
    }

    fn apply_pointer(&mut self, choice: Option<crate::scene::cursor::Pointer>) {
        use crate::scene::cursor::Pointer;

        let Some(choice) = choice else {
            return;
        };

        let Some(live) = self.live.as_ref() else {
            return;
        };

        match choice {
            Pointer::Hidden => live.window.set_cursor_visible(false),
            Pointer::Normal | Pointer::Alternate => {
                let bitmap = match choice {
                    Pointer::Alternate => live.alternate_cursor.as_ref(),
                    _ => live.normal_cursor.as_ref(),
                };

                if let Some(bitmap) = bitmap {
                    live.window.set_cursor(bitmap.clone());
                }

                live.window.set_cursor_visible(true);
            }
            Pointer::Unchanged => {}
        }
    }

    fn scene_manager(&mut self) -> Option<&mut ui::input::Manager> {
        match &mut self.runtime.scene {
            Scene::LoginStatic(_) => None,
            Scene::LoginPanels(scene) => Some(if scene.msgbox.placed.ui.panel.visible {
                &mut scene.box_manager
            } else {
                &mut scene.manager
            }),
            Scene::ServerSelect(scene) => Some(if scene.panels.msgbox.placed.ui.panel.visible {
                &mut scene.panels.box_manager
            } else {
                &mut scene.panels.manager
            }),
            Scene::SelectChar(scene) => Some(&mut scene.panels.manager),
            Scene::Field(_) => None,
        }
    }

    fn camera_frame(&mut self) {
        if self.runtime.scene.legacy_modal() {
            self.runtime.input.mouse = crate::scene::camera::MouseInput {
                alt: self.runtime.input.mods.alt,
                ..Default::default()
            };

            return;
        }

        let mouse = camera_frame_input(&mut self.runtime.input.mouse);

        if self.runtime.scene.camera_input(mouse) {
            self.redraw();
        }
    }

    fn redraw(&self) {
        if self.destination_pending() {
            return;
        }

        if let Some(live) = self.live.as_ref() {
            live.window.request_redraw();
        }
    }

    fn sync_ime(&mut self) {
        if self.settings_visible() {
            return;
        }

        let reset = self.runtime.ime.synchronize(&mut self.runtime.scene);

        let area = self.runtime.ime.area(&mut self.runtime.scene);

        let Some(live) = &mut self.live else {
            return;
        };

        if let Some(allow) = reset {
            live.ime_cursor_area.clear();

            live.window.set_ime_allowed(false);

            if allow {
                live.window.set_ime_allowed(true);
            }
        }

        if let Some(area) = area {
            let fit = render::blit::fit(
                self.settings.backbuffer,
                (live.config.width, live.config.height),
            );

            let area = ime_surface_area(area, self.settings.backbuffer, fit);

            let Some([x, y, w, h]) = live
                .ime_cursor_area
                .update(area, live.window.scale_factor())
            else {
                return;
            };

            live.window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(x, y),
                PhysicalSize::new(w, h),
            );
        }
    }

    fn refresh_screens(&mut self) {
        self.runtime.sync_costume_settings();

        let Scene::Field(scene) = &mut self.runtime.scene else {
            return;
        };

        {
            let Some(panels) = scene.panels.as_mut() else {
                return;
            };

            crate::scene::screens::refresh_all(panels, &scene.entered);
        }

        crate::scene::field::refresh_sheet_magic(scene);
    }

    fn refresh_party(&mut self) {
        let Scene::Field(scene) = &mut self.runtime.scene else {
            return;
        };

        let party = &scene.party;

        let auto_join = scene.party_auto_join;

        let Some(panels) = scene.panels.as_mut() else {
            return;
        };

        let Some(screen) = panels.screen_mut(crate::scene::panels::CONTENT_PARTY) else {
            return;
        };

        crate::scene::screens::party_controls(screen, party.rows.len(), auto_join);

        crate::scene::screens::party_refresh_in_arena(screen, party, scene.arena.visible);
    }

    fn broadcast_panels(&mut self) {
        let Scene::Field(scene) = &mut self.runtime.scene else {
            return;
        };

        {
            let Some(panels) = scene.panels.as_mut() else {
                return;
            };

            panels.field_close_owner_present = scene
                .player
                .and_then(|i| scene.world.actors.actors.get(i))
                .is_some();

            crate::scene::screens::refresh_status_panels(panels, &scene.entered);
        }

        crate::scene::field::refresh_sheet_magic(scene);
    }

    fn load_panel_icons(&mut self) {
        let Client {
            runtime: crate::runtime::state::RuntimeState { scene, .. },
            screens,
            live,
            ..
        } = self;

        let Some(screens) = screens.as_ref() else {
            return;
        };

        let Scene::Field(scene) = scene else {
            return;
        };

        let Some(panels) = scene.panels.as_mut() else {
            return;
        };

        let before = panels.icon_sheets.len();

        panels.field_close_owner_present = scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .is_some();

        crate::scene::screens::refresh_opened(panels, &scene.entered);

        if let Err(error) = crate::scene::panels::load_icon_sheets(screens.game(), panels) {
            tracing::info!("NWYD: could not load an updated cell's icon sheet: {error:#}");
        }

        if panels.icon_sheets.len() != before {}

        if panels.icon_sheets.len() != before
            && let Some(live) = live.as_mut()
            && !live.gpu.is_lost()
        {
            crate::scene::panels::upload(panels, &live.gpu, &mut live.renderer);
        }
    }

    pub fn with_audio(mut self, audio: SceneAudio) -> Client {
        self.audio = Some(audio);

        self
    }

    pub fn with_error_dialogs(mut self, enabled: bool) -> Self {
        self.error_dialogs = enabled;

        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_frame_benchmark(
        mut self,
        warmup: u64,
        frames: u64,
        output: std::path::PathBuf,
    ) -> Self {
        self.diagnostics = FrameDiagnostics::benchmark(
            warmup,
            frames,
            output,
            !std::env::var("NWYD_GPU_TIMINGS").is_ok_and(|value| value == "0"),
        );
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<()> {
        self.validate_benchmark_session()?;

        let event_result = event_loop
            .run_app(&mut self)
            .context(crate::error_context::Operation::RunEventLoop);

        self.runtime.net.shutdown_transport();

        let report = self.finish_frame_report();

        let _ = self.runtime.scene.final_guild_offline();

        if let Some(audio) = self.audio.as_mut() {
            audio.leave();
        }

        event_result?;

        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }

        report
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn run_on_canvas(mut self, canvas: web_sys::HtmlCanvasElement) -> Result<()> {
        use winit::platform::web::EventLoopExtWebSys;

        let instance = Gpu::instance();

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .context(crate::error_context::Operation::Surface)?;

        let gpu = Gpu::open_with_backend(instance, Some(&surface), self.settings.renderer)
            .await
            .context(crate::error_context::Operation::Device)?;

        let event_loop = create_event_loop()?;

        self.error_dialogs = false;

        self.browser_start = Some((canvas, (surface, gpu)));

        event_loop.spawn_app(self);

        Ok(())
    }

    fn benchmark_session_is_offline(&self) -> bool {
        self.runtime.net.phase() == net::Phase::Offline && !self.runtime.net.is_connected()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn validate_benchmark_session(&self) -> Result<()> {
        anyhow::ensure!(
            !self.diagnostics.is_benchmark() || self.benchmark_session_is_offline(),
            "an offline frame benchmark cannot use an active network session"
        );
        Ok(())
    }

    fn tick(&mut self) -> crate::clock::GameTime {
        if let Some(time) = self.diagnostics.benchmark_time(self.frames)
            && self.benchmark_session_is_offline()
        {
            self.runtime.clock = Clock::frozen(time);
        }
        let inputs = if self.render_rate.is_sampling() {
            crate::scene::lod::Inputs::live(self.render_rate.rate(), self.render_quality)
        } else {
            crate::scene::lod::Inputs::default()
        };

        self.runtime.sample_time(inputs, self.render_quality)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn hold_size(&mut self) {
        let Some(live) = self.live.as_ref() else {
            return;
        };

        let (w, h) = self.settings.backbuffer;

        if self.resize_attempts == 0 || live.window.inner_size() == PhysicalSize::new(w, h) {
            return;
        }

        self.resize_attempts -= 1;

        let _ = live
            .window
            .request_inner_size(logical((w, h), live.window.scale_factor()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Live> {
        let icon = formats::texture::decode_image(include_bytes!("../resources/nwyd.png"))
            .context("decoding the application icon")?;
        let icon = winit::window::Icon::from_rgba(icon.data, icon.width, icon.height)
            .context("creating the application icon")?;
        let attributes = Window::default_attributes()
            .with_title(self.settings.title.clone())
            .with_window_icon(Some(icon))
            .with_inner_size(logical_backbuffer(event_loop, self.settings.backbuffer))
            .with_resizable(false)
            .with_fullscreen(
                self.settings
                    .fullscreen
                    .then(|| Fullscreen::Borderless(None)),
            );

        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context(crate::error_context::Operation::Window)?,
        );

        let instance = Gpu::instance_with_display(Box::new(window.clone()));

        let surface = instance
            .create_surface(window.clone())
            .context(crate::error_context::Operation::Surface)?;

        let gpu = pollster::block_on(Gpu::open_with_backend(
            instance,
            Some(&surface),
            self.settings.renderer,
        ))
        .context(crate::error_context::Operation::Device)?;

        self.start_live(event_loop, window, surface, gpu)
    }

    #[cfg(target_arch = "wasm32")]
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Live> {
        use winit::platform::web::WindowAttributesExtWebSys;

        let (canvas, (surface, gpu)) = self
            .browser_start
            .take()
            .ok_or_else(|| anyhow!("browser GPU must be initialized before starting the client"))?;

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(self.settings.title.clone())
                        .with_canvas(Some(canvas))
                        .with_focusable(true)
                        .with_prevent_default(true),
                )
                .context(crate::error_context::Operation::Window)?,
        );

        self.start_live(event_loop, window, surface, gpu)
    }

    fn start_live(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        gpu: Gpu,
    ) -> Result<Live> {
        let (width, height) = self.settings.backbuffer;

        let capabilities = surface.get_capabilities(&gpu.adapter);

        let format = surface_format(&capabilities).ok_or_else(|| {
            anyhow!(
                "adapter {} offers no 8-bit surface format (it offers {:?})",
                gpu.info().name,
                capabilities.formats,
            )
        })?;

        let size = window.inner_size();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_present_mode(self.settings.present_mode, &capabilities),
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha_mode(&capabilities),
            view_formats: vec![],
        };

        surface::configure(&surface, &gpu, &config)?;

        let window_backend = pacing::backend(&window);

        tracing::info!(
            requested = ? self.settings.present_mode, resolved = ? config.present_mode,
            available = ? capabilities.present_modes, window_backend,
            "NWYD: presentation mode resolved"
        );

        let back_buffer = Offscreen::new(&gpu, width, height);

        let neural_available =
            cfg!(feature = "neural-rendering") && gpu.supports_neural_rendering();
        let neural = self
            .runtime
            .game_settings
            .as_ref()
            .is_some_and(|panel| panel.applied.neural_rendering);
        let renderer = upload_scene_renderer(
            &gpu,
            &self.runtime.scene,
            self.overlay.as_ref(),
            (width, height),
            neural,
        );

        let blitter = Blitter::new(&gpu, format);

        let source = blitter.source(&gpu, &back_buffer);

        if let Some(panel) = &mut self.runtime.game_settings {
            panel.set_renderers(gpu.available_backends(), gpu.backend());
            panel.set_neural_rendering_available(neural_available);
        }

        tracing::info!(
            "NWYD: {} ({:?}) — back buffer {}x{}, surface {}x{} {:?}, {:?}",
            gpu.info().name,
            gpu.info().backend,
            width,
            height,
            config.width,
            config.height,
            format,
            config.present_mode,
        );

        let timings =
            self.diagnostics
                .configure_gpu(&gpu, &config, (width, height), window_backend);

        Ok(Live {
            window_backend,
            window,
            surface,
            config,
            gpu,
            back_buffer,
            renderer,
            timings,
            neural_failed: false,
            blitter,
            source,
            stale: false,
            recovery_retry_at: None,
            normal_cursor: custom_cursor(event_loop, self.settings.pointers.normal.as_ref()),
            alternate_cursor: custom_cursor(event_loop, self.settings.pointers.alternate.as_ref()),
            ime_cursor_area: Default::default(),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn recover_device(&mut self) -> Result<()> {
        let Some(live) = self.live.as_mut().filter(|live| live.gpu.is_lost()) else {
            return Ok(());
        };

        let now = web_time::Instant::now();

        if live.recovery_retry_at.is_some_and(|retry| now < retry) {
            return Ok(());
        }

        live.recovery_retry_at = Some(now + std::time::Duration::from_secs(1));

        tracing::warn!(
            adapter =% live.gpu.info().name,
            "NWYD: rebuilding lost GPU device and rendering resources"
        );

        let instance = Gpu::instance_with_display(Box::new(live.window.clone()));

        let surface = instance
            .create_surface(live.window.clone())
            .context(crate::error_context::Operation::RecoverSurface)?;

        let gpu = pollster::block_on(Gpu::open_with_backend(
            instance,
            Some(&surface),
            self.settings.renderer,
        ))
        .context(crate::error_context::Operation::RecoverDevice)?;

        self.restore_device(surface, gpu)
    }

    #[cfg(target_arch = "wasm32")]
    fn recover_device(&mut self) -> Result<()> {
        use winit::platform::web::WindowExtWebSys;

        if let Some(pending) = &self.browser_recovery {
            let result = pending.borrow_mut().take();

            if let Some(result) = result {
                self.browser_recovery = None;

                let (surface, gpu) = result?;

                return self.restore_device(surface, gpu);
            }

            return Ok(());
        }

        let Some(live) = self.live.as_mut().filter(|live| live.gpu.is_lost()) else {
            return Ok(());
        };

        let now = web_time::Instant::now();

        if live.recovery_retry_at.is_some_and(|retry| now < retry) {
            return Ok(());
        }

        live.recovery_retry_at = Some(now + std::time::Duration::from_secs(1));

        let canvas = live
            .window
            .canvas()
            .context("browser game canvas is unavailable")?;

        let pending = BrowserRecovery::default();

        self.browser_recovery = Some(pending.clone());

        let renderer = self.settings.renderer;

        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let instance = Gpu::instance();

                let surface = instance
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                    .context(crate::error_context::Operation::RecoverSurface)?;

                let gpu = Gpu::open_with_backend(instance, Some(&surface), renderer)
                    .await
                    .context(crate::error_context::Operation::RecoverDevice)?;

                Ok((surface, gpu))
            }
            .await;

            *pending.borrow_mut() = Some(result);
        });

        Ok(())
    }

    fn restore_device(&mut self, surface: wgpu::Surface<'static>, gpu: Gpu) -> Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };

        let capabilities = surface.get_capabilities(&gpu.adapter);

        let mut config = live.config.clone();

        config.format = surface_format(&capabilities)
            .ok_or_else(|| anyhow!("replacement GPU offers no 8-bit surface format"))?;

        config.alpha_mode = alpha_mode(&capabilities);

        config.present_mode = surface_present_mode(self.settings.present_mode, &capabilities);

        tracing::info!(
            requested = ? self.settings.present_mode, resolved = ? config.present_mode,
            available = ? capabilities.present_modes,
            "NWYD: recovered presentation mode resolved"
        );

        if config.width > 0 && config.height > 0 {
            surface::configure(&surface, &gpu, &config)?;
        }

        let (width, height) = self.settings.backbuffer;

        let back_buffer = Offscreen::new(&gpu, width, height);

        live.renderer.release_neural();
        let neural_available =
            cfg!(feature = "neural-rendering") && gpu.supports_neural_rendering();
        let neural = self
            .runtime
            .game_settings
            .as_ref()
            .is_some_and(|panel| panel.applied.neural_rendering);
        let renderer = upload_scene_renderer(
            &gpu,
            &self.runtime.scene,
            self.overlay.as_ref(),
            (width, height),
            neural,
        );

        let blitter = Blitter::new(&gpu, config.format);

        let source = blitter.source(&gpu, &back_buffer);

        live.surface = surface;

        live.config = config;

        live.back_buffer = back_buffer;

        live.renderer = renderer;
        live.neural_failed = false;

        live.blitter = blitter;

        live.source = source;

        if let Some(panel) = &mut self.runtime.game_settings {
            panel.set_renderers(gpu.available_backends(), gpu.backend());
            panel.set_neural_rendering_available(neural_available);
        }

        live.timings = self.diagnostics.configure_gpu(
            &gpu,
            &live.config,
            (width, height),
            live.window_backend,
        );

        live.gpu = gpu;

        live.stale = false;

        live.recovery_retry_at = None;

        self.last_frame_at = None;

        tracing::info!(
            adapter =% live.gpu.info().name, "NWYD: GPU rendering resources recovered"
        );

        Ok(())
    }

    fn draw(&mut self, mut probe: FrameProbe) -> Result<()> {
        if matches!(
            & self.runtime.scene, Scene::Field(scene) if scene.world.force_load.is_some()
        ) {
            return Ok(());
        }

        if let Err(error) = self.recover_device() {
            tracing::warn!(
                % error, "NWYD: GPU recovery not ready; retaining scene and retrying"
            );

            return Ok(());
        }

        if self.live.as_ref().is_some_and(|live| live.gpu.is_lost()) {
            return Ok(());
        }

        if self.live.is_some() {
            self.runtime
                .retained
                .presentation_ready(&mut self.runtime.scene, self.runtime.clock.game_now());
        }

        let engine_pointer = self
            .pointer_location
            .resolve(self.settings.backbuffer, self.pointer_in_backbuffer());

        let panel_scale = self.panel_scale();

        let settings_open = self.settings_visible();

        #[cfg(not(target_arch = "wasm32"))]
        let update_visible = self.update_notice_visible();
        #[cfg(target_arch = "wasm32")]
        let update_visible = false;

        #[cfg(not(target_arch = "wasm32"))]
        let update_hovered = self.update_notice_blocks_pointer(engine_pointer);
        #[cfg(target_arch = "wasm32")]
        let update_hovered = false;

        if self.diagnostics.enabled() {
            probe.sample.frame_id = self.frames;
            probe.sample.continuous = self.uncapped_wayland();
        }

        if self.overlay.is_some() {
            let now = web_time::Instant::now();

            if let Some(last) = self.last_frame_at.replace(now) {
                let ms = now.duration_since(last).as_secs_f32() * 1_000.0;

                if let Some(overlay) = self.overlay.as_mut() {
                    overlay.record(ms);
                }
            }
        }

        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };

        if live.config.width == 0 || live.config.height == 0 {
            return Ok(());
        }

        if let Some(timings) = &mut live.timings {
            for sample in timings.poll(&live.gpu) {
                self.diagnostics.record_gpu(sample);
            }
        }

        if self.diagnostics.enabled() {
            probe.sample.focused = live.window.has_focus();
            probe.sample.surface = [live.config.width, live.config.height];
        }

        if std::mem::take(&mut live.stale) {
            surface::configure(&live.surface, &live.gpu, &live.config)?;
        }

        probe.sample.gap_ms = probe.checkpoint();
        let acquired = live.surface.get_current_texture();
        probe.sample.acquire_ms = probe.checkpoint();

        if live.gpu.is_lost() {
            return Ok(());
        }

        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                live.stale = true;

                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                surface::configure(&live.surface, &live.gpu, &live.config)?;

                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                live.surface = live
                    .gpu
                    .instance
                    .create_surface(live.window.clone())
                    .context(crate::error_context::Operation::RecoverSurface)?;

                surface::configure(&live.surface, &live.gpu, &live.config)?;

                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                if !self.occluded {
                    tracing::warn!(
                        frames = self.frames,
                        "NWYD: surface is occluded; waiting for a visible window before presenting"
                    );
                }

                self.occluded = true;

                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
        };

        if settings_open || update_visible {
            self.runtime.scene.queue_frame_under_overlay(
                &live.gpu,
                &mut live.renderer,
                if settings_open || update_hovered {
                    None
                } else {
                    engine_pointer
                },
            );
        } else {
            self.runtime.scene.queue_frame_with_pointer(
                &live.gpu,
                &mut live.renderer,
                &self.settings.pointers,
                self.runtime.input.pointer_state.frame,
                engine_pointer,
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        if update_visible && let Some(panel) = &mut self.updates.panel {
            panel.queue(&live.gpu, &mut live.renderer);
            self.settings.pointers.queue(
                &live.gpu,
                &mut live.renderer,
                self.settings.backbuffer,
                if update_hovered {
                    crate::scene::cursor::FRAME_NORMAL
                } else {
                    self.runtime.input.pointer_state.frame
                },
                engine_pointer,
            );
        }

        finish_frame_audio(&mut self.runtime.scene, self.audio.as_ref());

        if (self.settings.overlay || self.diagnostics.overlay_enabled())
            && let Some(overlay) = self.overlay.as_ref()
        {
            let top = 6.0;

            let mut lines = overlay.lines();
            lines.extend(self.diagnostics.lines());
            live.renderer
                .push_overlay(&overlay.draws(&lines, (6.0, top)));
        }

        if let Some(panel) = &mut self.runtime.game_settings {
            panel.queue(
                &live.gpu,
                &mut live.renderer,
                self.settings.backbuffer,
                panel_scale,
                &self.settings.pointers,
                engine_pointer,
            );
        }

        let timing = live
            .timings
            .as_mut()
            .and_then(|timings| timings.begin_frame(self.frames));

        let prepared = live
            .renderer
            .prepare_frame(&live.gpu, self.runtime.scene.viewport());

        probe.sample.prepare_ms = probe.checkpoint();
        prepared.submit_timed(&live.back_buffer.view, timing.as_ref());
        probe.sample.submit_ms = probe.checkpoint();

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let fit = live.blitter.render_timed(
            &live.gpu,
            &live.source,
            &view,
            (live.config.width, live.config.height),
            timing.as_ref(),
        );

        probe.sample.blit_ms = probe.checkpoint();

        if let (Some(timings), Some(timing)) = (&mut live.timings, timing) {
            timings.finish_frame(timing);
        }

        self.fit = Some(fit);

        if !self.fit_warned
            && (fit.width != live.back_buffer.width || fit.height != live.back_buffer.height)
        {
            self.fit_warned = true;

            tracing::info!(
                "Rescaling the {}x{} back buffer to fit the {}x{} surface",
                live.back_buffer.width,
                live.back_buffer.height,
                live.config.width,
                live.config.height,
            );
        }

        live.window.pre_present_notify();

        live.gpu.queue.present(frame);

        probe.sample.present_ms = probe.checkpoint();
        probe.sample_material_uploads(|| live.renderer.material_upload_stats());
        self.diagnostics.record(probe.finish());

        self.render_rate.sample(self.runtime.clock.tick_ms());

        self.frames += 1;

        let neural_failure = live.renderer.take_neural_failure();
        if let Some(error) = neural_failure {
            live.neural_failed = true;
            live.renderer.release_neural();
            if !live.gpu.is_lost() {
                live.renderer = upload_scene_renderer(
                    &live.gpu,
                    &self.runtime.scene,
                    self.overlay.as_ref(),
                    self.settings.backbuffer,
                    false,
                );
            }
            if let Some(panel) = &mut self.runtime.game_settings {
                panel.set_neural_rendering_available(false);
            }
            self.report_neural_failure(&error);
        }

        Ok(())
    }

    fn should_render(&self) -> bool {
        !self.nell_browser_visible() && !self.occluded
    }

    fn nell_browser_visible(&self) -> bool {
        matches!(& self.runtime.scene, Scene::Field(scene) if scene.nell_browser.visible)
    }

    fn uncapped_wayland(&self) -> bool {
        if self.destination_pending() {
            return false;
        }

        let Some(live) = &self.live else {
            return false;
        };

        self.pacing.uncapped(
            web_time::Instant::now(),
            &pacing::Conditions {
                wayland: live.window_backend == "wayland",
                focused: live.window.has_focus(),
                occluded: self.occluded,
                minimized: live.window.is_minimized() == Some(true),
                drawable: live.config.width > 0 && live.config.height > 0 && !live.gpu.is_lost(),
                mode: live.config.present_mode,
            },
        )
    }

    fn advance(&mut self) -> Result<crate::runtime::UpdateOutcome> {
        crate::runtime::advance(self)
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop, attempt_draw: bool) -> bool {
        if self
            .pacing
            .loading_wait(web_time::Instant::now(), self.destination_pending())
            .is_some()
        {
            return false;
        }

        let frames_before = self.frames;

        if let Some(live) = &mut self.live {
            live.renderer.begin_upload_frame();
        }

        let mut probe = FrameProbe::new(self.diagnostics.enabled());
        probe.begin_material_uploads(|| {
            self.live
                .as_ref()
                .map(|live| live.renderer.material_upload_stats())
        });

        match self.advance() {
            Ok(crate::runtime::UpdateOutcome::Ready { .. }) => {}
            Ok(crate::runtime::UpdateOutcome::Exit) => {
                event_loop.exit();

                return true;
            }
            Err(error) => {
                self.fail(event_loop, error);

                return true;
            }
        };

        probe.sample.update_ms = probe.checkpoint();

        if attempt_draw
            && self.should_render()
            && let Err(error) = self.draw(probe)
        {
            self.fail(event_loop, error);

            return true;
        }

        if let Some(live) = &self.live {
            live.renderer.finish_upload_frame();
        }

        if self.diagnostics.done(self.frames) {
            event_loop.exit();
            return true;
        }

        if attempt_draw {
            self.pacing
                .presented(web_time::Instant::now(), self.frames != frames_before);
        }

        let completed = web_time::Instant::now();

        self.pacing
            .loading_completed(completed, self.destination_pending());

        self.frame_deadline.completed(completed);

        false
    }

    fn destination_pending(&self) -> bool {
        matches!(
            & self.runtime.scene, Scene::Field(scene) if scene.world.force_load.is_some()
        )
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        tracing::error!(error = % format_args!("{error:#}"), "NWYD window failed");

        #[cfg(target_arch = "wasm32")]
        crate::web::report_error(&format!("{error:#}"));

        self.runtime.net.disconnect();

        if let Some(audio) = self.audio.as_mut() {
            audio.leave();
        }

        self.failure = Some(if self.error_dialogs {
            crate::error_dialog::show(error, self.live.as_ref().map(|live| live.window.as_ref()))
        } else {
            error
        });

        event_loop.exit();
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[path = "window/nell_browser.rs"]
mod nell_browser;

fn finish_frame_audio(scene: &mut Scene, audio: Option<&SceneAudio>) {
    let sounds = scene.take_sounds();

    let damage_sounds = scene.take_damage_sounds();

    let unconditional_sounds = scene.take_unconditional_sounds();

    let mut damage_played = Vec::with_capacity(damage_sounds.len());

    if let Some(audio) = audio {
        for slot in sounds {
            audio.play_once(slot);
        }

        for request in unconditional_sounds {
            audio.play_unconditional(request);
        }

        damage_played.extend(
            damage_sounds
                .iter()
                .copied()
                .map(|request| audio.play_damage(request)),
        );
    } else {
        damage_played.resize(damage_sounds.len(), Some(false));
    }

    for (request, played) in damage_sounds.into_iter().zip(damage_played) {
        if let Some(played) = played {
            scene.finish_damage_sound(request, played);
        }
    }

    if let Some(audio) = audio {
        for (request, played) in audio.take_damage_results() {
            scene.finish_damage_sound(request, played);
        }
    }
}

impl Client {
    fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window: WindowId,
        event: WindowEvent,
    ) {
        self.sync_ime();

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        if self.nell_browser_visible()
            && matches!(
                & event, WindowEvent::KeyboardInput { event, .. } if event.state ==
                ElementState::Pressed && event.logical_key ==
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
            )
        {
            if let Some(host) = &self.nell_browser {
                host.close();
            }

            self.flush_nell_browser();

            return;
        }

        if self.nell_browser_visible()
            && matches!(
                &event,
                WindowEvent::KeyboardInput { .. }
                    | WindowEvent::Ime(_)
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::CursorMoved { .. }
                    | WindowEvent::ModifiersChanged(_)
            )
        {
            return;
        }

        match event {
            WindowEvent::Ime(event) => {
                if matches!(
                    event,
                    winit::event::Ime::Enabled | winit::event::Ime::Disabled
                ) && let Some(live) = &mut self.live
                {
                    live.ime_cursor_area.clear();
                }

                if self.settings_visible() {
                    return;
                }

                self.runtime.ime.event(&mut self.runtime.scene, event);

                self.load_panel_icons();

                self.redraw();
            }
            WindowEvent::CloseRequested => {
                let now = self.runtime.clock.game_now();

                let result = match &mut self.runtime.scene {
                    Scene::Field(scene) => field::request_window_close(scene, now.as_millis()),
                    _ => field::WindowClose::Exit,
                };

                match result {
                    field::WindowClose::Started => {
                        self.runtime.net.quit(self.runtime.clock.wire_tick());

                        self.closing_since = Some(now);
                    }
                    field::WindowClose::Waiting => {}
                    field::WindowClose::Exit => event_loop.exit(),
                }
            }
            WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(live) = self.live.as_mut() {
                    live.config.width = size.width;

                    live.config.height = size.height;

                    if size.width > 0 && size.height > 0 && !live.gpu.is_lost() {
                        if let Err(error) =
                            surface::configure(&live.surface, &live.gpu, &live.config)
                        {
                            return self.fail(event_loop, error);
                        }
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                self.hold_size();
            }
            #[cfg(not(target_arch = "wasm32"))]
            WindowEvent::ScaleFactorChanged {
                mut inner_size_writer,
                ..
            } => {
                let (w, h) = self.settings.backbuffer;

                let _ = inner_size_writer.request_inner_size(PhysicalSize::new(w, h));
            }
            WindowEvent::ModifiersChanged(state) => {
                self.runtime.input.mods = Mods {
                    shift: state.state().shift_key(),
                    ctrl: state.state().control_key(),
                    alt: state.state().alt_key(),
                };

                self.runtime.input.mouse.alt = self.runtime.input.mods.alt;

                self.runtime.scene.modifiers(self.runtime.input.mods);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.route_key(
                    &event.physical_key,
                    &event.logical_key,
                    event.text.as_deref(),
                    event.state,
                    event.repeat,
                );
            }
            #[cfg(target_arch = "wasm32")]
            WindowEvent::Touch(touch) => self.pointer_touch(touch),
            // A finger owns the shared game pointer until it lifts or is cancelled.
            #[cfg(target_arch = "wasm32")]
            WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
                if self.primary_touch.is_some() => {}
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_moved((position.x, position.y));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.pointer_button(state, button);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.pointer_wheel(delta);
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_left();
            }
            WindowEvent::Focused(focused) => {
                #[cfg(not(target_arch = "wasm32"))]
                if !focused && let Some(panel) = &mut self.updates.panel {
                    panel.cancel_input();
                }

                if !focused && let Some(panel) = &mut self.runtime.game_settings {
                    panel.cancel_input();
                }

                self.runtime.ime.focused = focused;

                if !focused {
                    #[cfg(target_arch = "wasm32")]
                    self.cancel_touch();

                    if let Scene::Field(scene) = &mut self.runtime.scene {
                        field::cancel_pointer_hold(scene);
                    }

                    self.runtime.input.mods = Mods::default();

                    self.runtime.scene.modifiers(self.runtime.input.mods);

                    self.runtime.input.mouse = Default::default();
                }

                if let Some(audio) = self.audio.as_ref() {
                    audio.set_active(focused);
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;

                if occluded {
                    #[cfg(target_arch = "wasm32")]
                    self.cancel_touch();

                    self.pacing.invalidate_visibility();
                }

                if !occluded && let Some(live) = self.live.as_ref() {
                    live.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.pacing.redraw(web_time::Instant::now());

                self.frame(event_loop, true);
            }
            _ => {}
        }

        self.sync_ime();
    }
}

impl ApplicationHandler for Client {
    #[cfg(target_arch = "wasm32")]
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        let _ = self.runtime.scene.final_guild_offline();

        self.runtime.net.disconnect();

        if let Some(audio) = self.audio.as_mut() {
            audio.leave();
        }

        crate::web::report_closed();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.pacing.new_turn();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_some() {
            return;
        }

        match self.start(event_loop) {
            Ok(live) => self.live = Some(live),
            Err(error) => return self.fail(event_loop, error),
        }

        self.sync_ime();

        if self.settings.pointers.mode == formats::config::CursorMode::System {
            let choice = self
                .runtime
                .input
                .pointer_state
                .on_select_only(self.settings.pointers.mode);

            self.apply_pointer(choice);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        self.handle_window_event(event_loop, window, event);

        self.flush_pending_outbox();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            if self.panel_blocks_pointer() {
                return;
            }

            self.pointer_motion(delta.0, delta.1);

            if self.runtime.input.mouse.rotating() || self.runtime.input.mouse.alt {
                self.redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);

            return;
        }

        if self.destination_pending() {
            if self
                .pacing
                .loading_wait(web_time::Instant::now(), true)
                .is_none()
            {
                if self.frame(event_loop, true) {
                    return;
                }
            }

            if self.destination_pending() {
                let now = web_time::Instant::now();

                let next = self.pacing.loading_wait(now, true).unwrap_or(now);

                event_loop.set_control_flow(ControlFlow::WaitUntil(next));

                return;
            }
        }

        if self.uncapped_wayland() {
            if self.pacing.needs_frame() {
                if self.frame(event_loop, true) {
                    return;
                }
            }
        } else if self.frame_deadline.due(web_time::Instant::now()) {
            if self.frame(event_loop, false) {
                return;
            }
        }

        if !self.nell_browser_visible() && !self.occluded {
            self.redraw();
        }

        event_loop.set_control_flow(if self.uncapped_wayland() {
            ControlFlow::Poll
        } else {
            ControlFlow::WaitUntil(self.frame_deadline.next)
        });
    }
}

fn ime_surface_area(area: [f64; 4], backbuffer: (u32, u32), fit: Fit) -> [f64; 4] {
    let sx = fit.width as f64 / backbuffer.0.max(1) as f64;

    let sy = fit.height as f64 / backbuffer.1.max(1) as f64;

    [
        fit.x as f64 + area[0] * sx,
        fit.y as f64 + area[1] * sy,
        (area[2] * sx).max(1.0),
        (area[3] * sy).max(1.0),
    ]
}

pub fn create_event_loop() -> Result<EventLoop<()>> {
    let mut builder = EventLoop::builder();

    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;

        builder.with_msg_hook(crate::nell_browser::native::message_hook);
    }

    builder
        .build()
        .context(crate::error_context::Operation::EventLoop)
}

#[cfg(not(target_arch = "wasm32"))]
fn logical_backbuffer(event_loop: &ActiveEventLoop, backbuffer: (u32, u32)) -> LogicalSize<f64> {
    let scale = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map_or(1.0, |m| m.scale_factor());

    logical(backbuffer, scale)
}

#[cfg(not(target_arch = "wasm32"))]
fn logical(backbuffer: (u32, u32), scale: f64) -> LogicalSize<f64> {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };

    LogicalSize::new(
        f64::from(backbuffer.0) / scale,
        f64::from(backbuffer.1) / scale,
    )
}

fn surface_format(capabilities: &wgpu::SurfaceCapabilities) -> Option<wgpu::TextureFormat> {
    if capabilities.formats.contains(&SURFACE_FORMAT) {
        return Some(SURFACE_FORMAT);
    }

    capabilities
        .formats
        .iter()
        .find(|f| !f.is_srgb())
        .or(capabilities.formats.first())
        .copied()
}

fn surface_present_mode(
    requested: wgpu::PresentMode,
    capabilities: &wgpu::SurfaceCapabilities,
) -> wgpu::PresentMode {
    use wgpu::PresentMode::*;

    let candidates: &[wgpu::PresentMode] = match requested {
        AutoNoVsync => &[Immediate, Mailbox, Fifo],
        AutoVsync => &[FifoRelaxed, Fifo],
        _ => std::slice::from_ref(&requested),
    };

    candidates
        .iter()
        .copied()
        .find(|mode| capabilities.present_modes.contains(mode))
        .unwrap_or(Fifo)
}

fn alpha_mode(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::CompositeAlphaMode {
    let modes = &capabilities.alpha_modes;

    for wanted in [
        wgpu::CompositeAlphaMode::Opaque,
        wgpu::CompositeAlphaMode::Inherit,
    ] {
        if modes.contains(&wanted) {
            return wanted;
        }
    }

    modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto)
}

pub const WHEEL_DELTA: i32 = 120;

pub const ALT_DRAG_ZOOM: i32 = 3;

fn invert(inverted: bool) -> i32 {
    match inverted {
        true => -1,
        false => 1,
    }
}

fn wheel_counts(delta: winit::event::MouseScrollDelta) -> i32 {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(_, lines) => (lines * WHEEL_DELTA as f32) as i32,
        winit::event::MouseScrollDelta::PixelDelta(position) => position.y as i32,
    }
}

fn numpad_vk(
    physical: &winit::keyboard::PhysicalKey,
    logical: &winit::keyboard::Key,
) -> Option<u8> {
    use winit::keyboard::{KeyCode, PhysicalKey};

    let PhysicalKey::Code(code) = physical else {
        return None;
    };

    let digit = match code {
        KeyCode::Numpad0 => 0u8,
        KeyCode::Numpad1 => 1,
        KeyCode::Numpad2 => 2,
        KeyCode::Numpad3 => 3,
        KeyCode::Numpad4 => 4,
        KeyCode::Numpad5 => 5,
        KeyCode::Numpad6 => 6,
        KeyCode::Numpad7 => 7,
        KeyCode::Numpad8 => 8,
        KeyCode::Numpad9 => 9,
        _ => return None,
    };

    let winit::keyboard::Key::Character(text) = logical else {
        return None;
    };

    (text.as_str() == char::from(b'0' + digit).to_string())
        .then_some(crate::scene::emote::VK_NUMPAD0 + digit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPlan {
    pub edit: Option<EditKey>,
    pub numpad: Option<u8>,
    pub recall: bool,
    pub character: bool,
}

fn pane_virtual_key(key: &winit::keyboard::Key) -> u32 {
    use winit::keyboard::{Key, NamedKey as N};

    match key {
        Key::Character(s) => s
            .chars()
            .next()
            .filter(|_| s.chars().count() == 1)
            .map(|c| u32::from(c.to_ascii_uppercase()))
            .unwrap_or(0x10000),
        Key::Named(n) => match n {
            N::Backspace => 8,
            N::Tab => 9,
            N::Enter => 13,
            N::Shift => 16,
            N::Control => 17,
            N::Alt => 18,
            N::Pause => 19,
            N::CapsLock => 20,
            N::Escape => 27,
            N::Space => 32,
            N::PageUp => 33,
            N::PageDown => 34,
            N::End => 35,
            N::Home => 36,
            N::ArrowLeft => 37,
            N::ArrowUp => 38,
            N::ArrowRight => 39,
            N::ArrowDown => 40,
            N::PrintScreen => 44,
            N::Insert => 45,
            N::Delete => 46,
            N::F1 => 112,
            N::F2 => 113,
            N::F3 => 114,
            N::F4 => 115,
            N::F5 => 116,
            N::F6 => 117,
            N::F7 => 118,
            N::F8 => 119,
            N::F9 => 120,
            N::F10 => 121,
            N::F11 => 122,
            N::F12 => 123,
            N::NumLock => 144,
            N::ScrollLock => 145,
            _ => 0x10000,
        },
        _ => 0x10000,
    }
}

pub fn key_plan(
    physical: &winit::keyboard::PhysicalKey,
    logical: &winit::keyboard::Key,
    repeat: bool,
) -> KeyPlan {
    let edit = edit_key(logical);

    let claimed = edit.is_some();

    let numpad = match claimed {
        true => None,
        false => numpad_vk(physical, logical),
    };

    KeyPlan {
        edit,
        numpad,
        recall: !claimed
            && !repeat
            && matches!(
                logical,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::F11)
            ),
        character: !claimed,
    }
}

pub fn inventory_key_tab(logical: &winit::keyboard::Key, repeat: bool) -> Option<usize> {
    use winit::keyboard::{Key, NamedKey};

    if repeat {
        return None;
    }

    match logical {
        Key::Named(NamedKey::F1) => Some(0),
        Key::Named(NamedKey::F2) => Some(1),
        Key::Named(NamedKey::F3) => Some(2),
        Key::Named(NamedKey::F4) => Some(3),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MousePlan {
    pub mmb: Option<bool>,
    pub rmb: Option<bool>,
    pub message: Option<Mouse>,
}

fn custom_cursor(
    event_loop: &ActiveEventLoop,
    cursor: Option<&formats::exe::Cursor>,
) -> Option<CustomCursor> {
    let cursor = cursor?;

    let width = u16::try_from(cursor.width).ok()?;

    let height = u16::try_from(cursor.height).ok()?;

    let source = CustomCursor::from_rgba(
        cursor.rgba.clone(),
        width,
        height,
        cursor.hotspot_x,
        cursor.hotspot_y,
    )
    .ok()?;

    Some(event_loop.create_custom_cursor(source))
}

pub fn mouse_plan(button: MouseButton, state: ElementState) -> MousePlan {
    let down = state == ElementState::Pressed;

    MousePlan {
        mmb: matches!(button, MouseButton::Middle).then_some(down),
        rmb: matches!(button, MouseButton::Right).then_some(down),
        message: match (button, down) {
            (MouseButton::Left, true) => Some(Mouse::LeftDown),
            (MouseButton::Left, false) => Some(Mouse::LeftUp),
            (MouseButton::Right, true) => Some(Mouse::RightDown),
            (MouseButton::Right, false) => Some(Mouse::RightUp),
            _ => None,
        },
    }
}

pub fn wheel_step(delta: winit::event::MouseScrollDelta, inverted: bool) -> i32 {
    wheel_counts(delta) * invert(inverted)
}

pub fn camera_frame_input(
    mouse: &mut crate::scene::camera::MouseInput,
) -> crate::scene::camera::MouseInput {
    let mut frame = *mouse;

    if frame.alt && !frame.rmb {
        frame.dz = frame.dy * ALT_DRAG_ZOOM;
    }

    mouse.dx = 0;

    mouse.dy = 0;

    mouse.dz = 0;

    frame
}

fn edit_key(key: &winit::keyboard::Key) -> Option<EditKey> {
    use winit::keyboard::{Key, NamedKey};

    let Key::Named(named) = key else {
        return None;
    };

    Some(match named {
        NamedKey::Tab => EditKey::Tab,
        NamedKey::Enter => EditKey::Return,
        NamedKey::Escape => EditKey::Escape,
        NamedKey::PageUp => EditKey::PageUp,
        NamedKey::PageDown => EditKey::PageDown,
        NamedKey::End => EditKey::End,
        NamedKey::Home => EditKey::Home,
        NamedKey::ArrowLeft => EditKey::Left,
        NamedKey::ArrowUp => EditKey::Up,
        NamedKey::ArrowRight => EditKey::Right,
        NamedKey::ArrowDown => EditKey::Down,
        NamedKey::Insert => EditKey::Insert,
        NamedKey::Delete => EditKey::Delete,
        _ => return None,
    })
}
