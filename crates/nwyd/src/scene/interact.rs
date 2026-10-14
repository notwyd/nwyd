use super::actors::Actor;
use super::field::FieldScene;
use super::hover::PLAYER_ID_LIMIT;
use super::mission;

pub const CONTENT_PGT_MENU: crate::frame::PanelId = 0x24;

pub const TARGET_INTERVAL_MS: u32 = 500;

pub const NPC_BODIES: [i32; 5] = [0x36, 0x37, 0x38, 0x44, 0x39];

pub const NPC_BODY_33: i32 = 0x33;

pub const NPC_BODY_43: i32 = 0x43;

pub const MOB_GRADES_LOW: [u8; 3] = [1, 2, 3];

pub const MOB_GRADES_MID: u8 = 4;

pub const MOB_GRADES_HIGH: [u8; 2] = [6, 7];

pub const ANY_GRADES: std::ops::RangeInclusive<u8> = 8..=0xF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Where {
    pub zone_x: i32,
    pub row: i32,
    pub block: (u8, u8),
}

impl Where {
    pub const INERT_ZONES: (std::ops::RangeInclusive<i32>, i32) = (2..=10, 0x280);

    pub const BODY_33_ZONES: ([i32; 2], i32) = ([0xD, 0xE], 0xE00);

    pub const BODY_43_BLOCKS: [(u8, u8); 3] = [(0x0D, 0x0D), (0x1C, 0x18), (0x18, 0x1F)];

    pub const PORTAL_BLOCK: (u8, u8) = (0x10, 0x10);

    pub fn of(scene: &FieldScene) -> Where {
        let at = scene.target();

        Where {
            zone_x: (at.x as i32) >> 7,
            row: (at.z as i32) & !0x7F,
            block: scene.world.current,
        }
    }
}

pub fn consumes_a_click(store_open: bool, target: Option<&Actor>, at: Where) -> bool {
    if store_open {
        return true;
    }

    let Some(target) = target.filter(|t| t.highlight) else {
        return false;
    };

    let (zones, row) = Where::INERT_ZONES;

    if zones.contains(&at.zone_x) && at.row < row {
        return true;
    }

    let mob = target.entity >= PLAYER_ID_LIMIT;

    let grade = u8::try_from(target.status.flags & 0xF).unwrap_or(0);

    let (body_33_zones, body_33_row) = Where::BODY_33_ZONES;

    if mob
        && target.body == NPC_BODY_33
        && body_33_zones.contains(&at.zone_x)
        && at.row == body_33_row
    {
        return true;
    }

    if !target.shop_title.is_empty() {
        return true;
    }

    if mob && MOB_GRADES_LOW.contains(&grade) {
        return true;
    }

    if mob && target.body == NPC_BODY_43 && Where::BODY_43_BLOCKS.contains(&at.block) {
        return true;
    }

    if mob && NPC_BODIES.contains(&target.body) {
        return true;
    }

    if mob && grade == MOB_GRADES_MID {
        return true;
    }

    if ANY_GRADES.contains(&grade) {
        return true;
    }

    mob && MOB_GRADES_HIGH.contains(&grade)
}

pub const INTERACT_INTERVAL_MS: u32 = 1_000;

pub const OPEN_PERSONAL_SHOP: u16 = 0x39A;

pub const OPEN_ITEM_SHOP: u16 = 0x27B;

pub const GRADE_3_INTERACT: u16 = 0x28B;

pub const ETERNAL_STONE_SLOT: usize = 10;

pub const ETERNAL_STONE: u16 = 0x6CE;

pub const ETERNAL_STONE_BODY_STRDEF: usize = 152;

pub const ETERNAL_STONE_CONTEXT: u32 = 0x35;

pub const GRADE_6_INTERACT: u16 = 0x28E;

pub const GUILD_APPLICATION_REPLY: u16 = 0x28F;

pub const GRADE_4_BODY_6: u16 = 0x2F4;

pub const PORTAL_BODY: i32 = 0x3F;

pub const PORTAL_GRADE: u8 = 7;

pub const TALK_BODY_10F: i32 = 0x10F;

pub const TALK_BODY_33_GRADE: u8 = 9;

pub const TALK_BODY_3A: i32 = 0x3A;

pub const TALK_BODY_3A_STATUS: u32 = 0x4C;

pub mod talk_box {

    pub const GRADE_D: u32 = 0x2F;

    pub const GRADE_E: u32 = 0x30;

    pub const GRADE_F: u32 = 0x31;

    pub const GRADE_A: u32 = 0x32;

    pub const BODY_10F: u32 = 0x33;

    pub const BODY_3A: u32 = 0x34;

    pub const ETERNAL_STONE: u32 = 0x35;

    pub const GUILD_FAME: u32 = 0x2D;
}

pub const TALK_BOX_KIND: i32 = 0xD;

pub const TALK_BOX_CAPTION_STRDEF: usize = 237;

pub const GUILD_FAME_BODY_STRDEF: usize = 437;

pub const GUILD_APPLICATION_CONTEXT: u32 = 0x36;

pub const GUILD_APPLICATION_BODY_STRDEF: usize = 407;

pub fn guild_application_box() -> super::msgbox::Request {
    super::msgbox::Request {
        caption: Some(TALK_BOX_CAPTION_STRDEF),
        kind: TALK_BOX_KIND,
        context: GUILD_APPLICATION_CONTEXT,
        body: super::msgbox::Body::StrDef(GUILD_APPLICATION_BODY_STRDEF),
        flags: super::msgbox::FLAGS,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct GuildApplicationRaised {
    pub slot: Option<usize>,
}

pub fn raise_guild_application(scene: &mut FieldScene) -> GuildApplicationRaised {
    let slot = scene
        .panels
        .as_mut()
        .and_then(|panels| panels.raise_box(&guild_application_box(), None));

    GuildApplicationRaised { slot }
}

pub fn describe_guild_application(raised: &GuildApplicationRaised) -> &'static str {
    if raised.slot.is_some() {
        "NWYD: 0x18D — raised the guild-application confirmation"
    } else {
        "NWYD: 0x18D — no free field message-box slot"
    }
}

pub const TALK_BOX_BODY: [(u32, usize); 6] = [
    (talk_box::GRADE_D, 131),
    (talk_box::GRADE_E, 233),
    (talk_box::GRADE_F, 152),
    (talk_box::GRADE_A, 152),
    (talk_box::BODY_10F, 348),
    (talk_box::BODY_3A, 152),
];

pub fn talk_confirm(context: u32) -> Option<u32> {
    match context {
        talk_box::GUILD_FAME | talk_box::GRADE_D | talk_box::BODY_10F => Some(0),
        talk_box::GRADE_E
        | talk_box::GRADE_F
        | talk_box::GRADE_A
        | talk_box::BODY_3A
        | talk_box::ETERNAL_STONE => Some(1),
        _ => None,
    }
}

pub fn confirm_frame(entity: u16, flag: u32) -> Vec<u8> {
    let mut out = frame(GRADE_3_INTERACT, GRADE_3_LEN, u32::from(entity));

    let at = protocol::header::LEN + 4;

    out[at..at + 4].copy_from_slice(&flag.to_le_bytes());

    out
}

pub const COIN_KIND: i32 = 0x1C;

pub const COIN_ACTION_TALK: i32 = 0x23;

pub const INTERACT_LEN: usize = 0x10;

pub const GRADE_3_LEN: usize = 0x14;

pub const GRADE_4_LEN: usize = 0x18;

pub fn frame(opcode: u16, len: usize, payload: u32) -> Vec<u8> {
    let mut out = vec![0u8; len];

    protocol::header::set_opcode(&mut out, opcode);

    out[protocol::header::LEN..protocol::header::LEN + 4].copy_from_slice(&payload.to_le_bytes());

    out
}

pub fn guild_application_reply(entity: u32) -> Vec<u8> {
    frame(GUILD_APPLICATION_REPLY, GRADE_3_LEN, entity)
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Release {
    Nothing,
    Sent {
        opcode: u16,
        entity: u16,
    },
    Opened {
        content: crate::frame::PanelId,
        entity: u16,
    },
    Message {
        context: u32,
        entity: u16,
    },
    Notice {
        record: usize,
        entity: u16,
    },
    Browser {
        entity: u16,
    },
    Unbuilt {
        what: &'static str,
        entity: u16,
    },
}

pub fn release(scene: &mut FieldScene, now_ms: u32) -> Release {
    let store_open = scene
        .panels
        .as_ref()
        .is_some_and(|screens| screens.is_open(super::panels::CONTENT_STORE));

    if store_open {
        return Release::Nothing;
    }

    let Some(entity) = scene.hovered else {
        return Release::Nothing;
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return Release::Nothing;
    };

    let at = Where::of(scene);

    let target = &scene.world.actors.actors[index];

    if !target.highlight {
        return Release::Nothing;
    }

    let (zones, row) = Where::INERT_ZONES;

    if zones.contains(&at.zone_x) && at.row < row {
        return Release::Nothing;
    }

    if now_ms.wrapping_sub(scene.last_interact_ms) < INTERACT_INTERVAL_MS {
        return Release::Nothing;
    }

    let mob = target.entity >= PLAYER_ID_LIMIT;

    let status = target.status.flags;

    let grade = u8::try_from(status & 0xF).unwrap_or(0);

    let body = target.body;

    let shop = !target.shop_title.is_empty();

    let sent = if shop {
        Some((OPEN_PERSONAL_SHOP, INTERACT_LEN, u32::from(entity)))
    } else if mob && body == NPC_BODY_33 && {
        let (zones, row) = Where::BODY_33_ZONES;

        zones.contains(&at.zone_x) && at.row == row
    } {
        return Release::Nothing;
    } else if mob && grade == MOB_GRADES_MID && body == 6 {
        Some((GRADE_4_BODY_6, GRADE_4_LEN, 1))
    } else if mob && grade == 1 {
        return open_item_shop(scene, entity, now_ms);
    } else if mob && grade == 2 && body != NPC_BODY_33 {
        return open_cargo(scene, entity, now_ms);
    } else if mob && grade == 3 {
        return open_skill_master(scene, entity, now_ms);
    } else if mob
        && body == NPC_BODY_33
        && at.block == (0x10, 0x10)
        && (0x864..=0x86c).contains(&target.mover.last_dest.0)
        && (0x813..=0x81c).contains(&target.mover.last_dest.1)
    {
        raise_talk_box(scene, talk_box::GUILD_FAME, GUILD_FAME_BODY_STRDEF, entity);

        return Release::Message {
            context: talk_box::GUILD_FAME,
            entity,
        };
    } else if mob && body == NPC_BODY_43 && target.status.mastery[3] == mission::MASTERY_GUARD {
        let key = i32::from(target.status.mastery[0]);

        return open_mission(scene, entity, key, now_ms);
    } else if let Some(filter) = super::mix::npc_filter(entity, body, grade, at.block) {
        if let Some(filter) = filter
            && let Some(panels) = scene.panels.as_mut()
        {
            if filter.0 == 55 && super::merge_item::open(panels, &scene.entered) {
                return Release::Opened {
                    content: super::merge_item::CONTENT,
                    entity,
                };
            }

            if panels.is_open(super::mix::CONTENT) {
                panels.set_open(super::mix::CONTENT, false);
            } else if panels.open_mix(filter, false) {
                return Release::Opened {
                    content: super::mix::CONTENT,
                    entity,
                };
            }
        }

        return Release::Nothing;
    } else if mob && body == 0x39 {
        return notice_host(scene, entity);
    } else if mob && body == PORTAL_BODY && grade == PORTAL_GRADE && at.block == Where::PORTAL_BLOCK
    {
        return open_portal(scene, entity);
    } else if mob && grade == MOB_GRADES_MID {
        return talk(scene, entity, mob, status, body, now_ms);
    } else if mob && MOB_GRADES_HIGH.contains(&grade) {
        Some((GRADE_6_INTERACT, INTERACT_LEN, u32::from(entity)))
    } else if ANY_GRADES.contains(&grade) {
        return talk(scene, entity, mob, status, body, now_ms);
    } else {
        None
    };

    let Some((opcode, len, payload)) = sent else {
        return Release::Nothing;
    };

    if opcode == GRADE_6_INTERACT {
        scene.guild_application_entity = payload;
    }

    scene.outbox.push(frame(opcode, len, payload));

    if opcode != GRADE_4_BODY_6 {
        scene.last_interact_ms = now_ms;
    }

    Release::Sent { opcode, entity }
}

pub fn open_item_shop(scene: &mut FieldScene, entity: u16, now_ms: u32) -> Release {
    let known = scene
        .panels
        .as_ref()
        .and_then(|p| p.screen(super::panels::CONTENT_ITEM_SHOP));

    match known {
        Some(screen) if screen.open => return Release::Nothing,
        Some(_) => {}
        None => return Release::Nothing,
    }

    scene
        .outbox
        .push(frame(OPEN_ITEM_SHOP, INTERACT_LEN, u32::from(entity)));

    scene.last_interact_ms = now_ms;

    scene.shop_npc = Some(entity);

    if scene.world.actors.entity(entity).is_some_and(|i| {
        let actor = &scene.world.actors.actors[i];

        actor.body == 0x3b && actor.helmet_item == 0x4ec
    }) {
        scene.shop_coupon_pending = true;
    }

    scene.shop_price_mode = 0;

    if let Some(panels) = scene.panels.as_mut() {
        panels.shop_id = entity;
    }

    super::shop::sync_prices(scene);

    Release::Sent {
        opcode: OPEN_ITEM_SHOP,
        entity,
    }
}

pub fn open_skill_master(scene: &mut FieldScene, entity: u16, now_ms: u32) -> Release {
    let stone = scene.entered.equipment.items[ETERNAL_STONE_SLOT].index == ETERNAL_STONE;

    if stone {
        raise_talk_box(
            scene,
            ETERNAL_STONE_CONTEXT,
            ETERNAL_STONE_BODY_STRDEF,
            entity,
        );

        scene
            .outbox
            .push(frame(GRADE_3_INTERACT, GRADE_3_LEN, u32::from(entity)));

        scene.last_interact_ms = now_ms;

        return Release::Message {
            context: ETERNAL_STONE_CONTEXT,
            entity,
        };
    }

    let Some(panels) = scene.panels.as_mut() else {
        return Release::Nothing;
    };

    if panels.screen(super::panels::CONTENT_SKILL_MASTER).is_none() {
        return Release::Nothing;
    }

    scene.outbox.push(super::skill_master::open_request(
        &mut panels.master,
        entity,
    ));

    scene.last_interact_ms = now_ms;

    Release::Sent {
        opcode: super::skill_master::OPEN,
        entity,
    }
}

pub fn open_cargo(scene: &mut FieldScene, entity: u16, now_ms: u32) -> Release {
    use super::panels::{CONTENT_CARGO, CONTENT_ITEM_SHOP, CONTENT_SKILL};

    let Some(panels) = scene.panels.as_mut() else {
        return Release::Nothing;
    };

    if panels
        .screen(CONTENT_CARGO)
        .is_none_or(|screen| screen.open)
    {
        return Release::Nothing;
    }

    panels.set_open(CONTENT_ITEM_SHOP, false);

    panels.set_open(CONTENT_SKILL, false);

    super::inventory::open(scene, CONTENT_CARGO, false);

    if let Some(panels) = scene.panels.as_mut() {
        panels.set_open(CONTENT_CARGO, true);
    }

    scene.cargo_npc = Some(entity);

    scene.last_interact_ms = now_ms;

    Release::Opened {
        content: CONTENT_CARGO,
        entity,
    }
}

pub fn open_portal(scene: &mut FieldScene, entity: u16) -> Release {
    use super::panels::CONTENT_PORTAL;

    let Some(panels) = scene.panels.as_mut() else {
        return Release::Nothing;
    };

    if panels.screen(CONTENT_PORTAL).is_none() {
        return Release::Nothing;
    }

    close_others(panels);

    panels.set_open(CONTENT_PORTAL, true);

    let quests = panels.quests.clone();

    let strings = panels.strings.clone();

    if let Some(screen) = panels.screen_mut(CONTENT_PORTAL) {
        super::screens::portal_air_move(screen, &quests, &strings);
    }

    panels.portal.mode = Some(super::portal::Mode::AirMove);

    Release::Opened {
        content: CONTENT_PORTAL,
        entity,
    }
}

pub fn open_mission(scene: &mut FieldScene, entity: u16, key: i32, now_ms: u32) -> Release {
    use super::panels::{CONTENT_MISSION_MIX, CONTENT_STORE};

    let _ = now_ms;

    let Some(panels) = scene.panels.as_mut() else {
        return Release::Nothing;
    };

    let Some(screen) = panels.screen(CONTENT_MISSION_MIX) else {
        return Release::Nothing;
    };

    if screen.open {
        panels.set_open(CONTENT_MISSION_MIX, false);

        scene.mission = None;

        return Release::Opened {
            content: CONTENT_MISSION_MIX,
            entity,
        };
    }

    panels.set_open(CONTENT_STORE, false);

    panels.set_open(CONTENT_MISSION_MIX, true);

    scene.mission = panels
        .missions
        .as_ref()
        .map(|tables| super::mission::State::open(tables, key, entity));

    if let Some(state) = scene.mission.as_ref() {
        panels.refresh_mission(state);
    }

    Release::Opened {
        content: CONTENT_MISSION_MIX,
        entity,
    }
}

pub fn close_others(panels: &mut super::panels::Panels) {
    use super::panels::{
        CONTENT_CARGO, CONTENT_CHARACTER, CONTENT_INVENTORY, CONTENT_ITEM_SHOP, CONTENT_SKILL,
        CONTENT_SKILL_MASTER,
    };

    for content in [
        CONTENT_CHARACTER,
        CONTENT_INVENTORY,
        CONTENT_CARGO,
        CONTENT_SKILL,
        CONTENT_SKILL_MASTER,
        CONTENT_ITEM_SHOP,
    ] {
        panels.set_open(content, false);
    }
}

pub fn talk(
    scene: &mut FieldScene,
    entity: u16,
    mob: bool,
    status: u32,
    body: i32,
    now_ms: u32,
) -> Release {
    let grade = u8::try_from(status & 0xF).unwrap_or(0);

    let boxed = match grade {
        0xD => Some(talk_box::GRADE_D),
        0xE => None,
        0xF => Some(talk_box::GRADE_F),
        0xA => Some(talk_box::GRADE_A),
        MOB_GRADES_MID if body == TALK_BODY_10F => Some(talk_box::BODY_10F),
        _ => None,
    };

    if let Some(context) = boxed {
        let body = TALK_BOX_BODY
            .iter()
            .find(|(id, _)| *id == context)
            .map_or(ETERNAL_STONE_BODY_STRDEF, |(_, record)| *record);

        raise_talk_box(scene, context, body, entity);

        scene.last_interact_ms = now_ms;

        return Release::Message { context, entity };
    }

    let coin = mob && grade == TALK_BODY_33_GRADE && body == NPC_BODY_33;

    if coin && let Some(panels) = scene.panels.as_mut() {
        panels.begin_coin_money(7, 136);

        scene.coin_prompt = Some((COIN_KIND, COIN_ACTION_TALK));
    }

    if !coin
        && mob
        && body == TALK_BODY_3A
        && (grade == 0xB || status & 0xFF == TALK_BODY_3A_STATUS)
    {
        raise_talk_box(scene, talk_box::BODY_3A, ETERNAL_STONE_BODY_STRDEF, entity);

        scene.last_interact_ms = now_ms;

        return Release::Message {
            context: talk_box::BODY_3A,
            entity,
        };
    }

    scene
        .outbox
        .push(frame(GRADE_3_INTERACT, GRADE_3_LEN, u32::from(entity)));

    scene.last_interact_ms = now_ms;

    Release::Sent {
        opcode: GRADE_3_INTERACT,
        entity,
    }
}

fn raise_talk_box(scene: &mut FieldScene, context: u32, body: usize, entity: u16) {
    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    let request = super::msgbox::Request {
        caption: Some(TALK_BOX_CAPTION_STRDEF),
        kind: TALK_BOX_KIND,
        context,
        body: super::msgbox::Body::StrDef(body),
        flags: super::msgbox::FLAGS,
    };

    panels.raise_entity_box(&request, entity);
}

fn notice_host(scene: &mut FieldScene, entity: u16) -> Release {
    if scene.nell_browser.visible {
        return Release::Browser { entity };
    }

    let empty = scene.entered.warehouse.cargo[..120]
        .iter()
        .filter(|item| item.index == 0)
        .count();

    let record = if empty < 5 {
        Some(278)
    } else if scene.layout.width < 800 {
        Some(271)
    } else {
        None
    };

    if let Some(record) = record {
        if let Some(panels) = scene.panels.as_mut() {
            let text = panels.strings.translate(&format!("ss_{record}"));

            super::notice::post(panels, &text, 3000, true, scene.time.now_ms);
        }

        Release::Notice { record, entity }
    } else if scene.nell_browser.supported {
        scene.nell_browser.visible = true;

        scene.nell_browser.pending = Some(empty);

        Release::Browser { entity }
    } else {
        Release::Unbuilt {
            what: crate::internal_reason!("NWYD-E1012", "NPC browser unavailable on this platform"),
            entity,
        }
    }
}

pub fn describe_release(release: &Release) -> Option<String> {
    match release {
        Release::Nothing => None,
        Release::Browser { .. } => None,
        Release::Notice { record, entity } => Some(format!(
            "NWYD: entity {entity} posts notice strdef[{record}]"
        )),
        Release::Sent { opcode, entity } => {
            Some(format!("NWYD: sent {opcode:#05X} for entity {entity}"))
        }
        Release::Opened { content, entity } => Some(format!(
            "NWYD: entity {entity} opened {} (content {content:#04X})",
            super::panels::screen_name(*content),
        )),
        Release::Message { context, entity } => Some(format!(
            "NWYD: entity {entity} raises message box {context:#04X}{}",
            match talk_confirm(*context) {
                Some(flag) => format!(" — a Yes sends {GRADE_3_INTERACT:#05X}({entity}, {flag})"),
                None => String::new(),
            },
        )),
        Release::Unbuilt { what, entity } => Some(format!("NWYD: entity {entity} wants {what}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Latched {
    pub entity: Option<u16>,
    pub menu: bool,
}

pub fn latch_target(scene: &mut FieldScene, now_ms: u32) -> Latched {
    let Some(entity) = scene.hovered else {
        return Latched::default();
    };

    if entity >= PLAYER_ID_LIMIT {
        return Latched::default();
    }

    let highlighted = scene
        .world
        .actors
        .entity(entity)
        .is_some_and(|index| scene.world.actors.actors[index].highlight);

    if !highlighted {
        return Latched::default();
    }

    if now_ms.wrapping_sub(scene.last_target_ms) <= TARGET_INTERVAL_MS {
        return Latched::default();
    }

    scene.target = Some(entity);

    scene.last_target_ms = now_ms;

    let party = actor_party(scene, entity);

    if let Some(panels) = scene.panels.as_mut() {
        panels.pgt_target = party;

        if !panels.is_open(CONTENT_PGT_MENU) {
            panels.set_open(CONTENT_PGT_MENU, true);
        }
    }

    Latched {
        entity: Some(entity),
        menu: true,
    }
}

pub(super) fn actor_party(scene: &FieldScene, entity: u16) -> Option<super::pgt::Party> {
    let index = scene.world.actors.entity(entity)?;

    let actor = &scene.world.actors.actors[index];

    Some(super::pgt::Party {
        entity: actor.entity,
        guild: actor.guild,
        rank: actor.guild_rank as i16,
        dead: actor.is_dead,
        tracked: actor.tracked,
        name: actor.name.clone(),
        model_id: actor.model_id as u8,
        score: (
            actor.status.level,
            actor.status.max_hp as u16,
            actor.status.hp as u16,
        ),
    })
}

pub fn describe(latched: &Latched) -> Option<String> {
    let entity = latched.entity?;

    Some(format!(
        "NWYD: entity {entity} targeted — PanelPGTMenu \
         (content {CONTENT_PGT_MENU:#04X}) opened on page 1"
    ))
}
