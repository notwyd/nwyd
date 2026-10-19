use crate::{
    clock::GameTime,
    net::Session,
    scene::{self, Scene, audio::SceneAudio},
};
use std::time::Duration;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum EntryEpoch {
    #[default]
    Running,
    AwaitingUpload,
}

pub struct Retained {
    pub(crate) selection_body: i16,
    pub(crate) effect_options: Option<scene::lod::EffectOptions>,
    pub(crate) view_distance: scene::view_distance::ViewDistance,
    pub(crate) memo_inbox: scene::memo::Inbox,
    pub(crate) saved_tab: scene::chat::SavedTab,
    pub(crate) item_prices: scene::item_prices::Retained,
    pub(crate) name_prompt_required: bool,
    pub(crate) status_timeout: Option<Duration>,
    pub(crate) account_lock: u8,
    attack_cycles: [usize; 3],
    entry_epoch: EntryEpoch,
}

impl Retained {
    pub fn new(initial: &Scene) -> Self {
        Self {
            selection_body: match initial {
                Scene::SelectChar(scene) => scene.selection_body_write.unwrap_or(0),
                _ => 0,
            },
            effect_options: initial.effect_options(),
            view_distance: initial.view_distance().unwrap_or_default(),
            memo_inbox: Default::default(),
            saved_tab: Default::default(),
            item_prices: Default::default(),
            name_prompt_required: false,
            status_timeout: initial.status_timeout(),
            account_lock: match initial {
                Scene::SelectChar(scene) => scene.lock,
                _ => scene::lock::LOCKED,
            },
            attack_cycles: match initial {
                Scene::Field(field) => {
                    [field.attack.cycle, field.attack.auto_cycle, field.cc.cycle]
                }
                _ => [0; 3],
            },
            entry_epoch: EntryEpoch::Running,
        }
    }

    pub(crate) fn remember_selection_body(&mut self, scene: &Scene) {
        if let Scene::SelectChar(scene) = scene
            && let Some(body) = scene.selection_body_write
        {
            self.selection_body = body;
        }
    }

    pub(crate) fn prepare_selection(&self, session: &mut Session) {
        if let Some(block) = session.block.as_deref_mut() {
            scene::select_char::restore_selection_bodies(block, self.selection_body);
        }
    }

    pub fn presentation_ready(&mut self, scene: &mut Scene, now_ms: GameTime) {
        if self.entry_epoch == EntryEpoch::AwaitingUpload {
            self.entry_epoch = EntryEpoch::Running;

            scene.shown_at(now_ms.as_millis());
        }
    }
}

pub struct SceneState<'a> {
    pub scene: &'a mut Scene,
    pub retained: &'a mut Retained,
    pub session: &'a mut Session,
    pub ime: &'a mut scene::ime::Bridge,
    pub pointer: &'a mut scene::cursor::PointerState,
    pub audio: Option<&'a mut SceneAudio>,
    pub launcher_login: bool,
    pub launcher_handoff: bool,
}

pub trait SceneHost {
    fn state(&mut self) -> SceneState<'_>;

    fn disable_host_ime(&mut self);

    fn describe_scene(&mut self, next: &Scene);

    fn upload_scene(&mut self) -> bool;

    fn game_time(&self) -> GameTime;

    fn redraw(&mut self);
}

pub fn replace(host: &mut impl SceneHost, mut next: Scene) {
    {
        let state = host.state();

        state.retained.remember_selection_body(state.scene);

        if let Scene::Field(field) = &state.scene {
            state.retained.attack_cycles =
                [field.attack.cycle, field.attack.auto_cycle, field.cc.cycle];
        }

        if let Scene::Field(field) = &mut next {
            [field.attack.cycle, field.attack.auto_cycle, field.cc.cycle] =
                state.retained.attack_cycles;
        }

        state.retained.effect_options = state
            .scene
            .effect_options()
            .or(state.retained.effect_options);

        if let Some(options) = state.retained.effect_options {
            next.set_effect_options(options);
        }

        state.retained.view_distance = state
            .scene
            .view_distance()
            .unwrap_or(state.retained.view_distance);

        next.set_view_distance(state.retained.view_distance);

        state.retained.item_prices.apply(&mut next);

        if let Scene::Field(field) = &state.scene
            && let Some(panels) = &field.panels
        {
            state.retained.saved_tab.remember(&panels.chat);
        }

        if let Scene::Field(field) = &mut next
            && let Some(packet) = state
                .retained
                .saved_tab
                .entry_packet(field.entered.client_id, &field.entered.name())
        {
            field.outbox.push(packet);
        }

        if matches!(next, Scene::ServerSelect(_) | Scene::LoginPanels(_))
            && state.session.take_server_return_notice()
        {
            next.server_return_notice(true);
        }
    }

    {
        let state = host.state();

        match &mut next {
            Scene::LoginPanels(scene) => scene.launcher_handoff = state.launcher_handoff,
            Scene::ServerSelect(scene) => {
                scene.panels.launcher_handoff = state.launcher_handoff;
            }
            _ => {}
        }

        if let Scene::Field(field) = &state.scene {
            state
                .session
                .remember_evolution_tier(scene::cc::evolution_tier(field));
        }

        state.ime.cancel(state.scene);
    }

    host.disable_host_ime();

    {
        let state = host.state();

        next.set_launcher_login(state.launcher_login);

        if let Scene::SelectChar(character) = &mut next {
            character.name_dialog.prompt_required = state.retained.name_prompt_required;
        }

        if let Scene::Field(field) = &mut next
            && let Some(panels) = field.panels.as_mut()
        {
            state.retained.memo_inbox.enter(panels);
        }

        if let Some(timeout) = state.retained.status_timeout {
            next.allow_status_fetch(timeout);
        }
    }

    host.describe_scene(&next);

    {
        let mut state = host.state();

        next.inherit_message(state.scene);

        if let Some(block) = state.session.block.as_deref_mut() {
            state.scene.retire_account_experience(block);
        }

        leave_scene_state(state.scene, state.audio.as_deref_mut());

        finish_field_music(state.scene, &next, state.audio.as_deref());

        *state.scene = next;

        if matches!(state.scene, Scene::ServerSelect(_)) {
            state.retained.selection_body = 0;
        }

        enter_scene_state(
            state.scene,
            state.audio,
            &mut state.retained.account_lock,
            state.session,
        );

        state.pointer.scene_changed();

        state.retained.entry_epoch = EntryEpoch::AwaitingUpload;
    }

    let ready = host.upload_scene();

    let now = host.game_time();

    {
        let state = host.state();

        state.scene.set_now_ms(now.as_millis());

        if ready {
            state.retained.presentation_ready(state.scene, now);
        }
    }

    host.redraw();
}

pub(crate) fn enter_scene_state(
    scene: &Scene,
    audio: Option<&mut SceneAudio>,
    account_lock: &mut u8,
    session: &mut Session,
) {
    if matches!(scene, Scene::ServerSelect(_)) {
        session.entered_server_select();

        *account_lock = scene::lock::LOCKED;

        if let Some(audio) = audio {
            audio.enter();
        }
    }
}

pub(crate) fn leave_scene_state(scene: &Scene, audio: Option<&mut SceneAudio>) {
    if let Some(audio) = audio {
        audio.cancel_pending_sounds();

        match scene {
            Scene::ServerSelect(_) => audio.leave(),
            Scene::Field(_) => audio.leave_field(),
            _ => {}
        }
    }
}

pub(crate) fn finish_field_music(previous: &Scene, next: &Scene, audio: Option<&SceneAudio>) {
    if matches!(previous, Scene::Field(_))
        && matches!(next, Scene::SelectChar(_))
        && let Some(audio) = audio
    {
        audio.engine().stop_music();
    }
}
