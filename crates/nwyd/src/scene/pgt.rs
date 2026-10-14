use crate::frame::PanelId;
use crate::scene::msgbox;

pub const CONTENT: PanelId = super::interact::CONTENT_PGT_MENU;

pub const TAGS: [&str; 4] = ["1", "2", "3", "4"];

pub const PAGES: [[&str; 4]; 4] = [
    ["Party", "Guild", "Trade", "Challenge"],
    ["vs1", "vs5", "vs10", "vsall"],
    ["guilddrop", "guildwar", "guildally", "guildinvite"],
    ["gicommon", "gichief1", "gichief2", "gichief3"],
];

pub const CLOSE_BUTTON: &str = "Close";

pub const PARTY: u16 = 0x37F;

pub const CHALLENGE: u16 = 0x39F;

pub const GUILD_INVITE: u16 = 0x3D5;

pub const TRADE: u16 = 0x383;

pub const GUILD_DROP_REPLY: u16 = 0x28C;

pub const GUILD_WAR_REPLY: u16 = 0xE0E;

pub const GUILD_ALLY_REPLY: u16 = 0xE12;

pub const PARTY_LEN: usize = 0x2C;

pub const SHORT_LEN: usize = 0x14;

pub const PARTY_NAME_AT: usize = 0x16;

pub const PARTY_NAME_LEN: usize = 18;

pub const DUEL_MODES: [(&str, u32); 4] = [("vs1", 0), ("vs5", 1), ("vs10", 2), ("vsall", 3)];

pub const ALREADY_INVITED: usize = 156;

pub const ALREADY_TRADING: usize = 35;

pub const DROP_QUESTION: usize = 36;

pub const WAR_QUESTION: usize = 158;

pub const ALLY_QUESTION: usize = 221;

pub const ALREADY_IN_A_GUILD: usize = 364;

pub const IN_ANOTHER_GUILD: usize = 365;

pub const ALREADY_A_CHIEF: usize = 366;

pub const NOT_IN_A_GUILD: usize = 367;

pub const NOT_ENOUGH_GOLD: usize = 155;

pub const TRADE_EXCLUSION: [i32; 4] = [0xA29, 0x6A6, 0xA5C, 0x6D6];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyGate {
    Missing,
    Refused,
    AlreadyInvited,
    Send,
}

pub fn party_gate(
    target: Option<&Party>,
    player_dead: bool,
    first_row: Option<super::party::RowState>,
) -> PartyGate {
    let Some(target) = target else {
        return PartyGate::Missing;
    };

    if target.tracked || player_dead {
        return PartyGate::Refused;
    }

    if first_row == Some(super::party::RowState::InvitedPending) {
        return PartyGate::AlreadyInvited;
    }

    PartyGate::Send
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeGate {
    TooFar,
    Dead,
    Excluded,
    AlreadyTrading,
    Send,
}

impl TradeGate {
    pub fn clears_target(self) -> bool {
        self != TradeGate::TooFar
    }
}

pub fn trade_gate(
    player: (f32, f32),
    target: (f32, f32),
    max_distance: f32,
    dead: bool,
    engaged_and_open: bool,
) -> TradeGate {
    let player = (player.0 as i32, player.1 as i32);

    let target = (target.0 as i32, target.1 as i32);

    if formats::world::path::tile_distance(player, target) as f32 > max_distance - 5.0 {
        return TradeGate::TooFar;
    }

    if dead {
        return TradeGate::Dead;
    }

    let [left, top, right, bottom] = TRADE_EXCLUSION;

    if (left..right).contains(&player.0) && (top..bottom).contains(&player.1) {
        return TradeGate::Excluded;
    }

    if engaged_and_open {
        return TradeGate::AlreadyTrading;
    }

    TradeGate::Send
}

pub const INVITE_GOLD: u32 = 1_000_000;

pub const CHIEF_GOLD: u32 = 50_000_000;

pub const GUILD_PAGE_RANK: i16 = 2;

pub const INVITE_RANK: std::ops::Range<i16> = 2..5;

pub const CHIEF_REFUSED_RANK: std::ops::Range<i16> = 3..9;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Party {
    pub entity: u16,
    pub guild: u16,
    pub rank: i16,
    pub dead: bool,
    pub tracked: bool,
    pub name: String,
    pub model_id: u8,
    pub score: (u16, u16, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Page(usize),
    GuildOptions([bool; 4]),
    Send(Vec<u8>),
    Ask {
        request: msgbox::Request,
        payload: u16,
    },
    Toast(usize),
    Message(usize),
    ClearTarget,
    ClearPgtId,
    Close,
    OpenChiefPrompt,
    TradeRequest,
}

pub fn command(player: &Party, target: Option<&Party>, gold: u32, name: &str) -> Vec<Action> {
    command_with_state(player, target, target.is_some(), None, gold, name)
}

pub fn command_with_state(
    player: &Party,
    retained_target: Option<&Party>,
    target_present: bool,
    first_row: Option<super::party::RowState>,
    gold: u32,
    name: &str,
) -> Vec<Action> {
    command_with_ally(
        player,
        retained_target,
        target_present,
        first_row,
        gold,
        name,
        0xFFFF,
    )
}

pub fn command_with_ally(
    player: &Party,
    retained_target: Option<&Party>,
    target_present: bool,
    first_row: Option<super::party::RowState>,
    gold: u32,
    name: &str,
    ally: u16,
) -> Vec<Action> {
    let context = Context::snapshot(
        retained_target,
        0,
        player.dead,
        target_present,
        first_row,
        ally,
    );

    command_with_context(player, retained_target, &context, gold, name)
}

#[derive(Debug, Clone)]
pub struct Context {
    pub player_present: bool,
    pub player_model: u8,
    pub player_dead: bool,
    pub target_present: bool,
    pub target: Option<Party>,
    pub party_target: Option<Party>,
    pub first_row: Option<super::party::RowState>,
    pub ally: u16,
}

impl Context {
    pub fn snapshot(
        target: Option<&Party>,
        player_model: u8,
        player_dead: bool,
        target_present: bool,
        first_row: Option<super::party::RowState>,
        ally: u16,
    ) -> Self {
        Self {
            player_present: true,
            player_model,
            player_dead,
            target_present,
            target: target.filter(|_| target_present).cloned(),
            party_target: target.cloned(),
            first_row,
            ally,
        }
    }
}

pub fn command_with_context(
    player: &Party,
    retained_target: Option<&Party>,
    context: &Context,
    gold: u32,
    name: &str,
) -> Vec<Action> {
    if !context.player_present {
        return Vec::new();
    }

    let eq = |what: &str| name.eq_ignore_ascii_case(what);

    let target = context.target.as_ref();

    if eq("Party") {
        return match party_gate(
            context.party_target.as_ref(),
            player.dead,
            context.first_row,
        ) {
            PartyGate::Missing => vec![Action::ClearTarget],
            PartyGate::Refused => vec![Action::ClearTarget],
            PartyGate::AlreadyInvited => {
                vec![Action::Toast(ALREADY_INVITED), Action::ClearTarget]
            }
            PartyGate::Send => {
                vec![
                    Action::Send(party_frame(
                        player,
                        context
                            .party_target
                            .as_ref()
                            .expect("the Send gate has a target"),
                    )),
                    Action::ClearPgtId,
                    Action::ClearTarget,
                    Action::Close,
                ]
            }
        };
    }

    if eq("Guild") {
        if player.rank < GUILD_PAGE_RANK {
            return Vec::new();
        }

        let visible = target.map_or([false; 4], |target| {
            if target.guild == 0 {
                [false, false, false, INVITE_RANK.contains(&player.rank)]
            } else if target.guild == player.guild {
                [true, false, false, false]
            } else {
                [false, false, target.guild != context.ally, false]
            }
        });

        return vec![Action::Page(2), Action::GuildOptions(visible)];
    }

    if eq("Challenge") {
        return vec![Action::Page(1)];
    }

    if eq("Trade") {
        return context
            .target_present
            .then_some(Action::TradeRequest)
            .into_iter()
            .collect();
    }

    if let Some(&(_, mode)) = DUEL_MODES.iter().find(|(button, _)| eq(button)) {
        let Some(target) = target else {
            return vec![Action::Close];
        };

        return vec![
            Action::Send(short_frame(CHALLENGE, player, target, mode)),
            Action::ClearTarget,
            Action::Close,
        ];
    }

    for (button, context, body) in [
        ("guilddrop", 0x38u32, DROP_QUESTION),
        ("guildwar", 0x39, WAR_QUESTION),
        ("guildally", 0x3A, ALLY_QUESTION),
    ] {
        if !eq(button) {
            continue;
        }

        if target.is_none() && button != "guilddrop" {
            return Vec::new();
        }

        let payload = match button {
            "guilddrop" => retained_target.map_or(0, |target| target.entity),
            _ => target.map_or(0, |target| target.guild),
        };

        return vec![
            Action::Ask {
                request: guild_box(context, body),
                payload,
            },
            Action::ClearTarget,
            Action::Close,
        ];
    }

    if eq("guildinvite") || eq("gicommon") {
        let Some(target) = target else {
            return Vec::new();
        };

        if target.guild != 0 {
            return vec![Action::Message(ALREADY_IN_A_GUILD)];
        }

        if (gold as i32) < INVITE_GOLD as i32 {
            return vec![Action::Message(NOT_ENOUGH_GOLD)];
        }

        return vec![
            Action::Send(short_frame(GUILD_INVITE, player, target, 0)),
            Action::ClearTarget,
            Action::Close,
        ];
    }

    if ["gichief1", "gichief2", "gichief3"].iter().any(|b| eq(b)) {
        if let Some(target) = target {
            if target.guild == 0 {
                return vec![Action::Toast(NOT_IN_A_GUILD)];
            }

            if target.guild != player.guild {
                return vec![Action::Toast(IN_ANOTHER_GUILD)];
            }

            if CHIEF_REFUSED_RANK.contains(&target.rank) {
                return vec![Action::Toast(ALREADY_A_CHIEF)];
            }

            if (gold as i32) < CHIEF_GOLD as i32 {
                return vec![Action::Toast(NOT_ENOUGH_GOLD)];
            }
        }

        return vec![Action::Close, Action::OpenChiefPrompt];
    }

    if eq(CLOSE_BUTTON) {
        return vec![Action::Close];
    }

    Vec::new()
}

pub fn party_frame(player: &Party, target: &Party) -> Vec<u8> {
    let mut out = vec![0u8; PARTY_LEN];

    out[4..6].copy_from_slice(&PARTY.to_le_bytes());

    out[6..8].copy_from_slice(&player.entity.to_le_bytes());

    out[0x0C] = player.model_id.wrapping_sub(1);

    let (level, max_hp, hp) = player.score;

    out[0x0E..0x10].copy_from_slice(&level.to_le_bytes());

    out[0x10..0x12].copy_from_slice(&max_hp.to_le_bytes());

    out[0x12..0x14].copy_from_slice(&hp.to_le_bytes());

    out[0x14..0x16].copy_from_slice(&player.entity.to_le_bytes());

    let name = formats::text::codepage::encode(&player.name, formats::text::Codepage::Windows1252);

    let take = name.len().min(PARTY_NAME_LEN - 1);

    out[PARTY_NAME_AT..PARTY_NAME_AT + take].copy_from_slice(&name[..take]);

    out[0x28..0x2C].copy_from_slice(&u32::from(target.entity).to_le_bytes());

    out
}

pub fn short_frame(opcode: u16, player: &Party, target: &Party, mode: u32) -> Vec<u8> {
    let mut out = vec![0u8; SHORT_LEN];

    out[4..6].copy_from_slice(&opcode.to_le_bytes());

    out[6..8].copy_from_slice(&player.entity.to_le_bytes());

    out[0x0C..0x10].copy_from_slice(&u32::from(target.entity).to_le_bytes());

    out[0x10..0x14].copy_from_slice(&mode.to_le_bytes());

    out
}

pub fn guild_box(context: u32, body: usize) -> msgbox::Request {
    msgbox::Request {
        caption: Some(msgbox::DEFAULT_CAPTION_STRDEF),
        kind: super::interact::TALK_BOX_KIND,
        context,
        body: msgbox::Body::StrDef(body),
        flags: msgbox::FLAGS,
    }
}

pub fn guild_answer(
    context: u32,
    client_id: u16,
    local_guild: u16,
    payload: u32,
) -> Option<Vec<u8>> {
    let (opcode, len) = match context {
        0x38 => (GUILD_DROP_REPLY, 16),
        0x39 | 0x3D => (GUILD_WAR_REPLY, 20),
        0x3A => (GUILD_ALLY_REPLY, 20),
        _ => return None,
    };

    let mut frame = vec![0; len];

    frame[4..6].copy_from_slice(&opcode.to_le_bytes());

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    if context == 0x38 {
        frame[0x0C..0x10].copy_from_slice(&payload.to_le_bytes());
    } else {
        frame[0x0C..0x10].copy_from_slice(&u32::from(local_guild).to_le_bytes());

        let target = if context == 0x3D {
            payload.wrapping_neg()
        } else {
            payload
        };

        frame[0x10..0x14].copy_from_slice(&target.to_le_bytes());
    }

    Some(frame)
}

pub fn opened(panels: &mut super::panels::Panels) {
    panels.tooltip.hide();

    panels.hover_tip.hide();

    page(panels, 0);
}

pub fn page(panels: &mut super::panels::Panels, shown: usize) {
    let Some(screen) = panels.screen_mut(CONTENT) else {
        return;
    };

    for (index, tag) in TAGS.iter().enumerate() {
        screen
            .placed
            .ui
            .panel
            .set_visible_by_tagname(tag, index == shown);
    }
}

pub fn carry_out(panels: &mut super::panels::Panels, actions: &[Action]) {
    for action in actions {
        match action {
            Action::Page(shown) => page(panels, *shown),
            Action::GuildOptions(visible) => {
                if let Some(screen) = panels.screen_mut(CONTENT) {
                    for (name, &shown) in PAGES[2].iter().zip(visible) {
                        if let Some(button) =
                            screen.element_mut(name, formats::ui::pane::ElementType::Button)
                        {
                            button.visible = shown;
                        }
                    }
                }
            }
            Action::Send(frame) => panels.wire.push(frame.clone()),
            Action::Ask { request, payload } => {
                panels.raise_entity_box(request, *payload);
            }
            Action::Toast(record) => {
                panels.pgt_toasts.push(*record);
            }
            Action::Message(record) => panels.pgt_messages.push(*record),
            Action::ClearTarget => panels.pgt_clear_target = true,
            Action::ClearPgtId => panels.pgt_target = None,
            Action::Close => {
                panels.close_by_toggle(CONTENT);
            }
            Action::OpenChiefPrompt => {
                panels.raise_box_with_edit_limit(&chief_prompt(), 10);
            }
            Action::TradeRequest => panels.pgt_trade_requested = true,
        }
    }
}

pub fn chief_prompt() -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: 13,
        context: 36,
        body: msgbox::Body::StrDef(363),
        flags: 0x2200_0001,
    }
}

pub fn chief_frame(
    client_id: u16,
    target_name: &str,
    edit: &str,
    reserved: &super::create::Reserved,
) -> Option<Vec<u8>> {
    use formats::text::{Codepage, codepage};

    let cstr =
        |s: &str| codepage::encode(s.split('\0').next().unwrap_or(""), Codepage::Windows1252);

    let mut text = cstr(target_name);

    text.push(b' ');

    text.extend(cstr(edit));

    if text.windows(2).any(|pair| pair == [0xa1, 0xa1]) {
        return None;
    }

    reserved.substitute(&mut text, Codepage::Windows1252);

    let mut frame = vec![0; 160];

    frame[4..6].copy_from_slice(&0x334u16.to_le_bytes());

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..21].copy_from_slice(b"subcreate");

    let len = text.len().min(127);

    frame[28..28 + len].copy_from_slice(&text[..len]);

    Some(frame)
}

pub fn chief_accepts(scene: &mut super::field::FieldScene) {
    let Some(edit) = scene.panels.as_mut().and_then(|p| p.chief_accepted.take()) else {
        return;
    };

    if scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_none()
    {
        return;
    }

    let Some(target) = scene.target.and_then(|id| scene.world.actors.entity(id)) else {
        return;
    };

    let name = &scene.world.actors.actors[target].name;

    let panels = scene.panels.as_mut().unwrap();

    if let Some(frame) = chief_frame(scene.entered.client_id, name, &edit, &panels.reserved) {
        scene.outbox.push(frame);
    } else {
        let text = panels.strings.translate("ss_370");

        super::notice::post(panels, &text, 2000, true, scene.time.now_ms);
    }
}

pub fn panel_trades(scene: &mut super::field::FieldScene) -> Option<TradeGate> {
    super::trade::finish_panel_close(scene);

    let requested = scene
        .panels
        .as_mut()
        .is_some_and(|panels| std::mem::take(&mut panels.pgt_trade_requested));

    if !requested {
        return None;
    }

    let player_index = scene.player?;

    let target_entity = scene.target?;

    let target_index = scene.world.actors.entity(target_entity)?;

    let player = &scene.world.actors.actors[player_index];

    let target = &scene.world.actors.actors[target_index];

    let engaged_and_open = scene.trade.engaged()
        && scene
            .panels
            .as_ref()
            .is_some_and(|panels| panels.is_open(super::trade::CONTENT));

    let gate = trade_gate(
        (player.position[0], player.position[2]),
        (target.position[0], target.position[2]),
        scene.camera.max_dist,
        player.is_dead,
        engaged_and_open,
    );

    if !gate.clears_target() {
        return Some(gate);
    }

    scene.target = None;

    match gate {
        TradeGate::AlreadyTrading => {}
        TradeGate::Send => {
            let partner = scene
                .panels
                .as_ref()
                .and_then(|panels| panels.pgt_target.as_ref())
                .map_or(0, |target| target.entity);

            scene.trade.mine.partner = partner;

            scene.outbox.push(scene.trade.mine.to_frame().to_vec());

            if let Some(panels) = scene.panels.as_mut() {
                panels.pgt_target = None;

                panels.close_by_toggle(CONTENT);
            }
        }
        TradeGate::TooFar | TradeGate::Dead | TradeGate::Excluded => {}
    }

    if gate == TradeGate::AlreadyTrading
        && let Some(panels) = scene.panels.as_mut()
    {
        panels.pgt_toasts.push(ALREADY_TRADING);
    }

    Some(gate)
}
