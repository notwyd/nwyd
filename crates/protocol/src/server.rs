use crate::codec::Packet;
use formats::net::firework::{self, CELLS};
use formats::net::{
    account, action, affect, animation_probe, arena, attack, autotrade, backpack, body_mark,
    challenge, collection, emote, enter, event_hud, field_fx, gate_command, ground_item, guild,
    guild_invite, inventory, kept, kickout, kill, live, long_message, npc_line, party, plate,
    question, reel, shop, siege, spawn, trade, warehouse, weather,
};

pub const SYSTEM_MESSAGE: u16 = 0x105;

pub const SYSTEM_MESSAGE_ARG: u16 = 0x106;

pub const LOGIN_ACCEPTED: u16 = 0x10A;

pub const ACCOUNT_AFTER_DELETE: u16 = 0x110;

pub const ACCOUNT_AFTER_CREATE: u16 = 0x112;

pub const CHARACTER_SLOT_EFFECT: u16 = 0x3B4;

pub const ENTER_WORLD: u16 = 0x114;

pub const LEAVE_WORLD: u16 = 0x116;

pub const GUILD_APPLICATION: u16 = 0x18D;

pub const LOCK: u16 = 0xFDE;

pub const LOCK_RESET: u16 = 0xFDF;

pub const SLOT_PATCH: u16 = 0x182;

pub const VITALS: u16 = 0x181;

pub const ACTOR_STATUS: u16 = 0x336;

pub const PLAYER_PROGRESSION: u16 = 0x337;

pub const EQUIP_CHANGED: u16 = 0x36B;

pub const GOLD: u16 = 0x3AF;

pub const HP_DELTA: u16 = 0x18A;

pub const EMOTE: u16 = 0x36A;

pub const LIVE_STATE: [u16; 9] = [
    VITALS,
    SLOT_PATCH,
    ACTOR_STATUS,
    PLAYER_PROGRESSION,
    EQUIP_CHANGED,
    GOLD,
    HP_DELTA,
    EMOTE,
    NAME_MARK,
];

pub const FIREWORK: u16 = 0x3CA;

pub const NAME_MARK: u16 = 0x193;

pub const GLOW_BURST: u16 = 0x3AD;

pub const GUILD_INVITE: u16 = 0x3D5;

pub const CHALLENGE: u16 = 0x39F;

pub const INVENTORY: u16 = 0x185;

pub const COLLECTION: u16 = 0x300;

pub const NPC_LINE: u16 = 0x107;

pub const TRADE_OFFER: u16 = 0x383;

pub const TRADE_CLOSED: u16 = 0x384;

pub const TRADE_TOGGLE: u16 = 0x386;

pub const ITEM_MOVED: u16 = 0x376;

pub const SHOP_CLOSED: u16 = 0x196;

pub const CRAFTING_CLOSED: u16 = 0x3A7;

pub const CLOSE_CARGO: u16 = 0x19C;

pub const QUICK_BAR: u16 = enter::QUICK_BAR;

pub const SHOP_STOCK: u16 = 0x17C;

pub const SOLD: u16 = 0x37A;

pub const SHOP_CONTENTS: u16 = autotrade::CONTENTS;

pub const SHOP_SLOT_SOLD: u16 = autotrade::SLOT_SOLD;

pub const QUIT: u16 = 0x3AE;

pub const REDIRECT: u16 = 0x52A;

pub const REDIRECT_PAYLOAD_LEN: usize = 68;

pub const CHAT_CONSOLE: u16 = 0x101;

pub const CHAT_SAY: u16 = 0x333;

pub const CHAT_CHANNEL: u16 = 0x334;

pub const CHAT_SHOUT: u16 = 0xD1D;

pub const SPAWN: u16 = 0x364;

pub const SPAWN_SHOP: u16 = 0x363;

pub const AFFECTS: u16 = 0x3B9;

pub const MOVE: u16 = 0x36C;

pub const MOVE_RUN: u16 = 0x366;

pub const ACT: u16 = 0x368;

pub const ATTACK: u16 = attack::ATTACK;

pub const ATTACK_AREA: u16 = attack::ATTACK_AREA;

pub const ATTACK_MANY: u16 = attack::ATTACK_MANY;

pub const ATTACK_FAMILY: [u16; 3] = [ATTACK, ATTACK_AREA, ATTACK_MANY];

pub const ACTION_FAMILY: [u16; 3] = [MOVE, MOVE_RUN, ACT];

pub const ASKS_WHO: [u16; 2] = [MOVE, MOVE_RUN];

pub const REMOVE: u16 = 0x16F;

pub const GROUND_ITEM: u16 = ground_item::DROP;

pub const GATE_COMMAND: u16 = gate_command::OPCODE;

pub const WAREHOUSE_DEPOSIT: u16 = warehouse::DEPOSIT;

pub const WAREHOUSE_WITHDRAW: u16 = warehouse::WITHDRAW;

pub const WAREHOUSE_SET: u16 = warehouse::SET;

pub const WAREHOUSE_FAMILY: [u16; 3] = warehouse::FAMILY;

pub const WEATHER: u16 = weather::SET;

pub const EVENT_COUNTDOWN: u16 = event_hud::COUNTDOWN;

pub const EVENT_REMAINING: u16 = event_hud::REMAINING;

pub const EVENT_PACKED: u16 = event_hud::PACKED;

pub const EVENT_TARGET: u16 = event_hud::TARGET;

pub const EVENT_SCORE: u16 = event_hud::SCORE;

pub const EVENT_HUD_FAMILY: [u16; 5] = event_hud::FAMILY;

pub const SUMMON_REQUEST: u16 = 0x3B2;

pub const SUMMON_CANCEL: u16 = 0x3B3;

pub const SUMMON_NAME_OFF: usize = 4;

pub const SUMMON_NAME_LEN: usize = 16;

pub const SUMMON_REQUEST_LEN: usize = SUMMON_NAME_OFF + SUMMON_NAME_LEN;

pub const PLATE_LEVEL: u16 = 0x2F7;

pub const PLATE_GUILD: u16 = 0x2F8;

pub const GUILD: u16 = guild::OPCODE;

pub const SIEGE_STATE: u16 = siege::STATE;

pub const WAR_FLAG: u16 = siege::WAR;

pub const SIEGE_FAMILY: [u16; 2] = [WAR_FLAG, SIEGE_STATE];

pub const KICKOUT: u16 = kickout::OPCODE;

pub const FIELD_CARPET: u16 = field_fx::CARPET;

pub const FIELD_SOUND: u16 = field_fx::SOUND;

pub const FIELD_LINE: u16 = kept::LINES;

pub const KEYED_RECORD: u16 = kept::RECORD;

pub const ARENA_ROUND: u16 = arena::ROUND;

pub const ARENA_KILL: u16 = arena::KILL;

pub const ARENA_LIST: u16 = arena::LIST;

pub const ARENA_FAMILY: [u16; 3] = arena::FAMILY;

pub const ANIMATION_PROBE: u16 = animation_probe::REQUEST;

pub const QUESTION: u16 = question::QUESTION;

pub const LONG_MESSAGE: u16 = long_message::INBOUND;

pub const REEL_RESULT: u16 = reel::RESULT;

pub const CONSUMED: [u16; 3] = [0x1C5, 0x379, 0x3A4];

pub const ITEM_ARRIVED: u16 = backpack::ARRIVED;

pub const ITEM_CLEARED: u16 = backpack::CLEARED;

pub const BACKPACK_FAMILY: [u16; 2] = backpack::FAMILY;

pub const PARTY_JOINED: u16 = party::JOINED;

pub const PARTY_LEFT: u16 = party::LEFT;

pub const PARTY_INVITED: u16 = party::INVITED;

pub const PARTY_MARKED: u16 = party::MARKED;

pub const PARTY_FAMILY: [u16; 4] = party::FAMILY;

pub const SHOP_SELL_LIST: u16 = shop::SELL_LIST;

pub const VANISH: u16 = 0x165;

pub const KILL: u16 = kill::KILL;

pub const STRDEF_BASE: usize = 1000;

pub const KIND_PLAIN: u8 = 0;

pub const KIND_MODAL: u8 = 4;

pub const KIND_CHAT: u8 = 1;

pub const SAY_TEXT: usize = 0x00;

pub const SAY_TEXT_END: usize = 0x7E;

pub const CHANNEL_NAME: usize = 0x00;

pub const CHANNEL_NAME_END: usize = 0x0F;

pub const CHANNEL_TEXT: usize = 0x10;

pub const CHANNEL_TEXT_END: usize = 0x8E;

pub const CHANNEL_KIND: usize = 0x90;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemMessage {
    pub kind: u8,
    pub code: i16,
    pub argument: String,
    pub has_argument: bool,
}

impl SystemMessage {
    pub fn strdef(&self) -> usize {
        (self.code as isize + STRDEF_BASE as isize).max(0) as usize
    }

    fn parse(payload: &[u8], has_argument: bool) -> SystemMessage {
        let kind = payload.first().copied().unwrap_or(0);

        let code = payload
            .get(2..4)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .unwrap_or(0);

        let argument = match has_argument {
            true => text_at(payload, 4),
            false => String::new(),
        };

        SystemMessage {
            kind,
            code,
            argument,
            has_argument,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatLine {
    pub opcode: u16,
    pub speaker: u16,
    pub name: String,
    pub text: String,
    pub kind: u16,
    pub strdef_code: Option<i16>,
    pub argument: String,
}

impl ChatLine {
    pub fn strdef(&self) -> Option<usize> {
        Some((self.strdef_code? as isize + STRDEF_BASE as isize).max(0) as usize)
    }

    fn say(opcode: u16, speaker: u16, payload: &[u8]) -> ChatLine {
        let end = payload.len().min(SAY_TEXT_END);

        ChatLine {
            opcode,
            speaker,
            text: text_at(&payload[..end], SAY_TEXT),
            ..ChatLine::default()
        }
    }

    fn channel(speaker: u16, payload: &[u8]) -> ChatLine {
        let cut = |from: usize, to: usize| match payload.get(from..payload.len().min(to)) {
            Some(field) => text_at(field, 0),
            None => String::new(),
        };

        ChatLine {
            opcode: CHAT_CHANNEL,
            speaker,
            name: cut(CHANNEL_NAME, CHANNEL_NAME_END),
            text: cut(CHANNEL_TEXT, CHANNEL_TEXT_END),
            kind: payload
                .get(CHANNEL_KIND..CHANNEL_KIND + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .unwrap_or(0),
            ..ChatLine::default()
        }
    }

    fn canned(opcode: u16, speaker: u16, payload: &[u8]) -> ChatLine {
        let system = SystemMessage::parse(payload, opcode == SYSTEM_MESSAGE_ARG);

        ChatLine {
            opcode,
            speaker,
            strdef_code: Some(system.code),
            argument: system.argument,
            ..ChatLine::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub client_id: u16,
    pub payload: [u8; REDIRECT_PAYLOAD_LEN],
    pub account: String,
    pub token: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummonRequest {
    pub requester: String,
}

fn summon_request(payload: &[u8]) -> Result<SummonRequest, String> {
    let Some(field) = payload.get(SUMMON_NAME_OFF..SUMMON_REQUEST_LEN) else {
        return Err(format!(
            "a summon request needs {SUMMON_REQUEST_LEN} payload bytes, got {}",
            payload.len(),
        ));
    };

    let field = &field[..SUMMON_NAME_LEN - 1];

    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());

    Ok(SummonRequest {
        requester: formats::text::codepage::decode(
            &field[..end],
            formats::text::Codepage::Windows1252,
        ),
    })
}

impl Redirect {
    pub fn channel(&self) -> Option<i32> {
        let bytes = self.token.strip_prefix(b"*")?;

        let mut at = bytes
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(bytes.len());

        let number_start = at;

        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }

        let digits_start = at;

        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }

        if at == digits_start {
            return None;
        }

        std::str::from_utf8(&bytes[number_start..at])
            .ok()?
            .parse()
            .ok()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    QuestFlags([u8; 11]),
    ClientConfig(formats::net::client_config::Update),
    ServerReturnNotice,
    LoginDenied {
        opcode: u16,
    },
    CharacterNotice {
        opcode: u16,
    },
    CharacterDeleteHold(account::DeleteHold),
    SelectionMetadata(account::SelectionMetadata),
    CharacterNameNotice,
    CharacterNameResult(account::NameResult),
    CharacterSlotEffect {
        slot: u32,
    },
    LoginAccepted(Box<account::Accept>),
    AccountBlock {
        opcode: u16,
        block: Box<account::Block>,
    },
    EnterWorld(Box<enter::EnterWorld>),
    LeaveWorld {
        client_id: u16,
    },
    System(SystemMessage),
    Chat(Box<ChatLine>),
    Spawn {
        opcode: u16,
        entity: Box<spawn::Spawn>,
    },
    Affects {
        entity: u16,
        affects: Box<affect::Affects>,
    },
    Action {
        opcode: u16,
        entity: u16,
        action: Box<action::Action>,
    },
    Remove {
        entity: u16,
    },
    GroundItem(Box<ground_item::Drop>),
    GateCommand(gate_command::Command),
    WarehouseGold(warehouse::Move),
    Weather(weather::Forecast),
    EventCountdown(event_hud::Countdown),
    EventRemaining(event_hud::Remaining),
    EventPacked(event_hud::Packed),
    EventTarget(event_hud::Target),
    EventScore(event_hud::Score),
    SummonRequest(SummonRequest),
    SummonCancel,
    PlateLevel(plate::LevelChange),
    PlateGuild(plate::GuildLine),
    Guild(guild::Membership),
    SiegeState(siege::Siege),
    WarFlag(siege::WarFlag),
    Kickout(kickout::Kickout),
    Consumed {
        opcode: u16,
    },
    ItemPrices(formats::net::item_prices::Update),
    FieldCarpet(field_fx::Carpet),
    FieldSound(field_fx::Sound),
    FieldLine(Box<kept::Line>),
    KeyedRecord(Box<kept::Record>),
    ArenaRound(arena::Round),
    ArenaKill(Box<arena::Kill>),
    ArenaList(Box<arena::List>),
    ItemArrived(backpack::Arrival),
    ItemCleared(backpack::Clear),
    PartyMember {
        opcode: u16,
        member: party::Member,
    },
    PartyLeft(party::Leave),
    PartyMarks(party::Marks),
    ShopSellList(Box<shop::SellList>),
    Vanish {
        entity: u16,
        vanish: live::Vanish,
    },
    Kill(kill::Kill),
    Attack {
        opcode: u16,
        attack: Box<attack::Attack>,
    },
    SlotPatch {
        entity: u16,
        patch: live::SlotPatch,
    },
    Vitals {
        entity: u16,
        vitals: live::Vitals,
    },
    HpDelta {
        entity: u16,
        delta: live::HpDelta,
    },
    Firework {
        entity: u16,
        glyph: firework::Glyph,
    },
    Emote {
        entity: u16,
        emote: emote::Emote,
    },
    ActorStatus {
        entity: u16,
        score: Box<live::ActorStatus>,
    },
    PlayerProgression {
        entity: u16,
        score: live::PlayerProgression,
    },
    EquipChanged {
        entity: u16,
        change: Box<live::EquipChange>,
    },
    Gold {
        entity: u16,
        gold: live::Gold,
    },
    NameMark {
        entity: u16,
        mode: i32,
    },
    GlowBurst {
        entity: u16,
        mode: i32,
    },
    GuildInvite {
        entity: u16,
        invite: guild_invite::Invite,
    },
    GuildApplication,
    Challenge {
        entity: u16,
        challenge: challenge::Challenge,
    },
    Inventory {
        entity: u16,
        bag: Box<inventory::Bag>,
    },
    Collection {
        entity: u16,
        progress: collection::Progress,
    },
    NpcLine {
        entity: u16,
        line: npc_line::NpcLine,
    },
    TradeOffer {
        entity: u16,
        offer: trade::Offer,
    },
    TradeClosed {
        entity: u16,
    },
    TradeToggle {
        entity: u16,
    },
    ItemMoved {
        dst: (u8, u8),
        src: (u8, u8),
    },
    Sold(shop::Sale),
    ShopClosed,
    CraftingClosed,
    AnimationProbe(animation_probe::Request),
    Question(Box<question::Question>),
    LongMessage(Box<long_message::LongMessage>),
    ReelResult(Box<reel::ResultMessage>),
    CloseCargo,
    QuickBar {
        bar: [u8; enter::QUICK_SLOTS],
    },
    ShopStock(Box<shop::Stock>),
    ShopContents(Box<autotrade::Contents>),
    ShopSlotSold(autotrade::Sold),
    Lock {
        opcode: u16,
    },
    Quit,
    Redirect(Redirect),
    DynamicKey {
        opcode: u16,
        update: formats::net::dynamic_key::Update,
    },
    Other {
        opcode: u16,
        payload: Vec<u8>,
    },
    Malformed {
        opcode: u16,
        reason: String,
    },
}

impl ServerMessage {
    pub fn opcode(&self) -> u16 {
        match self {
            Self::QuestFlags(_) => 0x4307,
            Self::ClientConfig(_) => 0x4302,
            ServerMessage::ServerReturnNotice => 0x194,
            ServerMessage::LoginDenied { opcode } => *opcode,
            ServerMessage::CharacterNotice { opcode } => *opcode,
            ServerMessage::CharacterDeleteHold(_) => 0x11E,
            ServerMessage::SelectionMetadata(_) => 0x1D6,
            ServerMessage::CharacterNameNotice => 0x7A9,
            ServerMessage::CharacterNameResult(_) => 0xFAA,
            ServerMessage::LoginAccepted(_) => LOGIN_ACCEPTED,
            ServerMessage::CharacterSlotEffect { .. } => CHARACTER_SLOT_EFFECT,
            ServerMessage::AccountBlock { opcode, .. } => *opcode,
            ServerMessage::EnterWorld(_) => ENTER_WORLD,
            ServerMessage::LeaveWorld { .. } => LEAVE_WORLD,
            ServerMessage::System(message) => match message.has_argument {
                true => SYSTEM_MESSAGE_ARG,
                false => SYSTEM_MESSAGE,
            },
            ServerMessage::Chat(line) => line.opcode,
            ServerMessage::Spawn { opcode, .. } => *opcode,
            ServerMessage::Affects { .. } => AFFECTS,
            ServerMessage::Action { opcode, .. } => *opcode,
            ServerMessage::Remove { .. } => REMOVE,
            ServerMessage::GroundItem(_) => GROUND_ITEM,
            ServerMessage::GateCommand(_) => GATE_COMMAND,
            ServerMessage::WarehouseGold(moved) => moved.change.opcode(),
            ServerMessage::Weather(_) => WEATHER,
            ServerMessage::EventCountdown(_) => EVENT_COUNTDOWN,
            ServerMessage::EventRemaining(_) => EVENT_REMAINING,
            ServerMessage::EventPacked(_) => EVENT_PACKED,
            ServerMessage::EventTarget(_) => EVENT_TARGET,
            ServerMessage::EventScore(_) => EVENT_SCORE,
            ServerMessage::SummonRequest(_) => SUMMON_REQUEST,
            ServerMessage::SummonCancel => SUMMON_CANCEL,
            ServerMessage::PlateLevel(_) => PLATE_LEVEL,
            ServerMessage::PlateGuild(_) => PLATE_GUILD,
            ServerMessage::Guild(_) => GUILD,
            ServerMessage::SiegeState(_) => SIEGE_STATE,
            ServerMessage::WarFlag(_) => WAR_FLAG,
            ServerMessage::Kickout(_) => KICKOUT,
            ServerMessage::Consumed { opcode } => *opcode,
            ServerMessage::ItemPrices(_) => 0x1c2,
            ServerMessage::FieldCarpet(_) => FIELD_CARPET,
            ServerMessage::FieldSound(_) => FIELD_SOUND,
            ServerMessage::FieldLine(_) => FIELD_LINE,
            ServerMessage::KeyedRecord(_) => KEYED_RECORD,
            ServerMessage::ArenaRound(_) => ARENA_ROUND,
            ServerMessage::ArenaKill(_) => ARENA_KILL,
            ServerMessage::ArenaList(_) => ARENA_LIST,
            ServerMessage::ItemArrived(_) => ITEM_ARRIVED,
            ServerMessage::ItemCleared(_) => ITEM_CLEARED,
            ServerMessage::PartyMember { opcode, .. } => *opcode,
            ServerMessage::PartyLeft(_) => PARTY_LEFT,
            ServerMessage::PartyMarks(_) => PARTY_MARKED,
            ServerMessage::ShopSellList(_) => SHOP_SELL_LIST,
            ServerMessage::Vanish { .. } => VANISH,
            ServerMessage::Kill(_) => KILL,
            ServerMessage::Attack { opcode, .. } => *opcode,
            ServerMessage::SlotPatch { .. } => SLOT_PATCH,
            ServerMessage::Vitals { .. } => VITALS,
            ServerMessage::HpDelta { .. } => HP_DELTA,
            ServerMessage::Firework { .. } => FIREWORK,
            ServerMessage::Emote { .. } => EMOTE,
            ServerMessage::ActorStatus { .. } => ACTOR_STATUS,
            ServerMessage::PlayerProgression { .. } => PLAYER_PROGRESSION,
            ServerMessage::EquipChanged { .. } => EQUIP_CHANGED,
            ServerMessage::Gold { .. } => GOLD,
            ServerMessage::NameMark { .. } => NAME_MARK,
            ServerMessage::GlowBurst { .. } => GLOW_BURST,
            ServerMessage::GuildInvite { .. } => GUILD_INVITE,
            ServerMessage::GuildApplication => GUILD_APPLICATION,
            ServerMessage::Challenge { .. } => CHALLENGE,
            ServerMessage::Inventory { .. } => INVENTORY,
            ServerMessage::Collection { .. } => COLLECTION,
            ServerMessage::NpcLine { .. } => NPC_LINE,
            ServerMessage::TradeOffer { .. } => TRADE_OFFER,
            ServerMessage::TradeClosed { .. } => TRADE_CLOSED,
            ServerMessage::TradeToggle { .. } => TRADE_TOGGLE,
            ServerMessage::ShopStock(_) => SHOP_STOCK,
            ServerMessage::ShopContents(_) => SHOP_CONTENTS,
            ServerMessage::ShopSlotSold(_) => SHOP_SLOT_SOLD,
            ServerMessage::ShopClosed => SHOP_CLOSED,
            ServerMessage::CraftingClosed => CRAFTING_CLOSED,
            ServerMessage::AnimationProbe(_) => ANIMATION_PROBE,
            ServerMessage::Question(_) => QUESTION,
            ServerMessage::LongMessage(_) => LONG_MESSAGE,
            ServerMessage::ReelResult(_) => REEL_RESULT,
            ServerMessage::CloseCargo => CLOSE_CARGO,
            ServerMessage::QuickBar { .. } => QUICK_BAR,
            ServerMessage::ItemMoved { .. } => ITEM_MOVED,
            ServerMessage::Sold(_) => SOLD,
            ServerMessage::Lock { opcode } => *opcode,
            ServerMessage::Quit => QUIT,
            ServerMessage::Redirect(_) => REDIRECT,
            ServerMessage::DynamicKey { opcode, .. } => *opcode,
            ServerMessage::Other { opcode, .. } => *opcode,
            ServerMessage::Malformed { opcode, .. } => *opcode,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::QuestFlags(_) => "0x4307 quest flags".into(),
            Self::ClientConfig(update) => {
                format!("0x4302 client configuration — {} entries", update.0.len())
            }
            ServerMessage::ServerReturnNotice => "0x194 server-return notice".into(),
            ServerMessage::LoginDenied { opcode } => {
                format!("{opcode:#05x} login denied")
            }
            ServerMessage::CharacterNotice { opcode } => {
                format!("{opcode:#05x} character-selection notice")
            }
            ServerMessage::CharacterDeleteHold(_) => "0x11E character deletion hold".into(),
            ServerMessage::SelectionMetadata(_) => "0x1D6 account selection metadata".into(),
            ServerMessage::DynamicKey { opcode, .. } => {
                format!("{opcode:#05x} dynamic-key update")
            }
            ServerMessage::CharacterNameNotice => "0x7A9 character-name notice flag".into(),
            ServerMessage::CharacterNameResult(reply) => {
                format!(
                    "0xFAA character-name result {} — slot {}",
                    reply.result, reply.slot
                )
            }
            ServerMessage::CharacterSlotEffect { slot } => {
                format!("0x3B4 character-slot effect — slot {slot}")
            }
            ServerMessage::LoginAccepted(accept) => {
                format!(
                    "0x10A login accepted — account {:?}, {} of 4 slot(s) used, key {:02X?}",
                    accept.account(),
                    accept.block.occupied(),
                    &accept.key[..4],
                )
            }
            ServerMessage::AccountBlock { opcode, block } => {
                format!(
                    "{opcode:#05x} character list — {} of 4 slot(s) used",
                    block.occupied(),
                )
            }
            ServerMessage::EnterWorld(world) => {
                let (zone_x, zone_y) = world.zone();

                format!(
                    "0x114 enter world — {:?} as client id {}, zone ({zone_x}, {zone_y})",
                    world.mob.name(),
                    world.client_id,
                )
            }
            ServerMessage::LeaveWorld { client_id } => {
                format!("0x116 back to the character list for client id {client_id}")
            }
            ServerMessage::Spawn { opcode, entity } => {
                format!(
                    "{opcode:#05x} spawn — entity {} {:?} ({}), tile ({}, {}), body {}{}",
                    entity.id,
                    entity.name(),
                    match entity.is_player() {
                        true => format!("level {}", entity.level()),
                        false => "a mob".to_string(),
                    },
                    entity.x,
                    entity.z,
                    entity.equip[0],
                    match entity.motto().is_empty() {
                        true => String::new(),
                        false => format!(", shop {:?}", entity.motto()),
                    },
                )
            }
            ServerMessage::Affects { entity, affects } => {
                let live: Vec<String> = affects
                    .details
                    .iter()
                    .filter(|detail| detail.is_set())
                    .map(|detail| format!("{:#04x}/{}", detail.kind, detail.level))
                    .collect();

                format!(
                    "0x3B9 affects — entity {entity}, {} active{}",
                    live.len(),
                    match live.is_empty() {
                        true => String::new(),
                        false => format!(" [{}]", live.join(", ")),
                    },
                )
            }
            ServerMessage::Action {
                opcode,
                entity,
                action,
            } => {
                format!(
                    "{opcode:#05x} {} — entity {entity}, ({}, {}) → ({}, {}){}{}",
                    match action.order() {
                        action::Order::Walk => "move".to_string(),
                        action::Order::Teleport => "teleport".to_string(),
                        action::Order::Act(kind) => format!("action {kind}"),
                        action::Order::Nothing => "no-op".to_string(),
                    },
                    action.x,
                    action.z,
                    action.dest_x,
                    action.dest_z,
                    match action.dirs().is_empty() {
                        true => String::new(),
                        false => format!(", path {:?}", String::from_utf8_lossy(action.dirs())),
                    },
                    match action.out_of_range() {
                        true => " — off the map, which the original only logs".to_string(),
                        false => String::new(),
                    },
                )
            }
            ServerMessage::Remove { entity } => {
                format!("0x16F remove — entity {entity} left the world")
            }
            ServerMessage::GateCommand(command) => {
                format!(
                    "0x374 gate command — entity {} to state {} (commanded {}, fill {})",
                    command.entity,
                    command.state,
                    command.commanded(),
                    command.fill,
                )
            }
            ServerMessage::WarehouseGold(moved) => {
                format!(
                    "{:#06X} warehouse — {} {} coins",
                    moved.change.opcode(),
                    match moved.change {
                        warehouse::Change::Set => "now holds",
                        warehouse::Change::Deposit => "deposit",
                        warehouse::Change::Withdraw => "withdraw",
                    },
                    moved.amount,
                )
            }
            ServerMessage::Weather(forecast) => {
                format!(
                    "0x18B weather — {}",
                    match forecast.code {
                        weather::CLEAR => "clear, the day cycle runs".to_string(),
                        weather::RAIN => "rain".to_string(),
                        weather::SNOW => "snow".to_string(),
                        weather::HEAVY_SNOW => "heavy snow".to_string(),
                        code => format!("code {code} — no emitter, and the day cycle stops"),
                    },
                )
            }
            ServerMessage::EventCountdown(countdown) => {
                format!("0x3A1 event clock — {} s", countdown.seconds)
            }
            ServerMessage::EventRemaining(remaining) => {
                format!("0x3B0 event counter — {:?}", remaining.text())
            }
            ServerMessage::EventPacked(packed) => {
                format!(
                    "0x3BB event counter — {:?}{}",
                    packed.text(),
                    match packed.unread_byte() {
                        0 => String::new(),
                        byte => format!(" (byte 1 = {byte:#04X}, read by neither half)"),
                    },
                )
            }
            ServerMessage::EventTarget(target) => match target.text() {
                Some(text) => {
                    format!("0x3CF event counter — {text:?}, shown and NOT stamped")
                }
                None => "0x3CF event counter — hidden".to_string(),
            },
            ServerMessage::EventScore(score) => {
                format!("0x3D0 event counter — {:?}, no show", score.text())
            }
            ServerMessage::SummonRequest(request) => {
                format!("0x3B2 summon request — {:?}", request.requester)
            }
            ServerMessage::SummonCancel => "0x3B3 summon notice cancelled".to_string(),
            ServerMessage::PlateLevel(change) => {
                let (level, name_level) = change.stored();

                format!(
                    "0x2F7 level change — entity {} stores level {level}, name level {name_level}{}",
                    change.entity,
                    match change.is_mob() {
                        true => " (the mob arm; the payload's own fields are discarded)",
                        false => "",
                    },
                )
            }
            ServerMessage::PlateGuild(line) => {
                format!(
                    "0x2F8 guild line — entity {} is {:?}",
                    line.entity,
                    line.displayed(),
                )
            }
            ServerMessage::Guild(m) => {
                format!(
                    "0x3E1C guild membership — guild {}, rank {}, charidx {}, sub-command {} ({}){}",
                    m.guild,
                    m.rank,
                    m.charidx,
                    m.command,
                    match m.command() {
                        guild::Command::None =>
                            "the writes and the connection report, nothing else",
                        guild::Command::Refresh => "refresh the guild window",
                        guild::Command::Close => "close the guild window",
                        guild::Command::Other(_) => "no panel arm",
                    },
                    match m.raises_effect() {
                        true => ", with a level-up burst",
                        false => "",
                    },
                )
            }
            ServerMessage::SiegeState(siege::Siege::Ignored { size }) => {
                format!("0x3A8 castle siege — size {size:#06x} has no arm; consumed and dropped")
            }
            ServerMessage::SiegeState(siege::Siege::Set(state)) => {
                format!(
                    "0x3A8 castle siege — war guild {}{}, kingdom {}, ally {} (arm {:#06x})",
                    state.war_guild,
                    match state.has_zero_guild_id() {
                        true => " (D-508: the 0xFFFF sentinel was skipped)",
                        false => "",
                    },
                    match state.kingdom {
                        Some(kingdom) => kingdom.to_string(),
                        None => "unchanged".to_string(),
                    },
                    match state.ally {
                        Some(ally) => ally.to_string(),
                        None => "unchanged".to_string(),
                    },
                    state.size,
                )
            }
            ServerMessage::WarFlag(war) => {
                format!(
                    "0x3AC war flag — {} ({:#010x}); clears the arena music slot{}",
                    match war.raised() {
                        true => "raised",
                        false => "lowered",
                    },
                    war.flag,
                    match war.clears_field_music() {
                        true => " and the field one",
                        false => "",
                    },
                )
            }
            ServerMessage::Kickout(k) => {
                format!(
                    "0x292 session teardown — code {} ({})",
                    k.code(),
                    match k {
                        kickout::Kickout::Characters { .. } =>
                            "back to the character list (ChangeScene 5)".to_string(),
                        kickout::Kickout::Death { .. } =>
                            "die where you stand, if your level is under 1000".to_string(),
                        kickout::Kickout::Disconnected { .. } => format!(
                            "strdef {} for {} ms, then ChangeScene 7 unless the launcher logged in",
                            kickout::MESSAGE_STRDEF,
                            kickout::MESSAGE_LIFE_MS,
                        ),
                    },
                )
            }
            ServerMessage::Consumed { opcode } => {
                format!(
                    "{opcode:#06x} is consumed and does nothing — the image's own deliberate silence",
                )
            }
            ServerMessage::ItemPrices(update) => {
                format!(
                    "0x01c2 item prices 412={}, 413={}",
                    update.prices[0], update.prices[1]
                )
            }
            ServerMessage::FieldCarpet(c) => {
                format!(
                    "0x3a2 explosion carpet — ({}, {})..({}, {}), kind {:#x}: {} cell(s), {}",
                    c.x0,
                    c.z0,
                    c.x1,
                    c.z1,
                    c.kind,
                    c.cells().count(),
                    match c.builds() {
                        true => "an explosion on each",
                        false => "the ground sampled and nothing built",
                    },
                )
            }
            ServerMessage::FieldSound(s) => {
                format!(
                    "0x3a3 field sound — slot {}, unless it is already playing",
                    s.slot
                )
            }
            ServerMessage::FieldLine(line) => {
                format!(
                    "0x2c8 field text line — slot {}{}, {:?}",
                    line.slot,
                    match line.clamped {
                        true => " (clamped)",
                        false => "",
                    },
                    line.text,
                )
            }
            ServerMessage::KeyedRecord(r) => {
                format!(
                    "0xdc3 keyed record — key {}, +0x10 = {} ({}.{})",
                    r.key(),
                    r.tenths(),
                    r.split_tenths().0,
                    r.split_tenths().1.abs(),
                )
            }
            ServerMessage::ArenaRound(round) => {
                format!(
                    "0x2f1 arena round — {}, scores {:?}",
                    match (round.is_active(), round.draw()) {
                        (true, _) => "the board up, the clock seeded if it was down".to_string(),
                        (_, true) => "a draw, board down".to_string(),
                        _ => format!("team {} wins, board down", round.winning_team()),
                    },
                    round.scores,
                )
            }
            ServerMessage::ArenaKill(kill) => {
                format!(
                    "0x2f2 arena kill — {:?} (team {}) over {:?} (team {}), strdef {}, scores {:?}",
                    kill.killer,
                    kill.killer_team.saturating_add(1),
                    kill.victim,
                    kill.victim_team.saturating_add(1),
                    match kill.record() {
                        Some(record) => record.to_string(),
                        None => "none — the line is suppressed".to_string(),
                    },
                    kill.scores,
                )
            }
            ServerMessage::ArenaList(list) => {
                let slot = list.chosen();

                format!(
                    "0x2f5 arena list — slot {} of {}, state {}, {}/{}, {} gold",
                    slot,
                    arena::SLOTS,
                    list.state[slot],
                    list.current[slot],
                    list.max[slot],
                    list.cost[slot],
                )
            }
            ServerMessage::ItemArrived(arrival) => {
                format!(
                    "0x171 item arrived — item {} for carry slot {}{}",
                    arrival.item.index,
                    arrival.slot,
                    match arrival.slot as usize >= backpack::Container::Carry.slots() {
                        true => " — out of the sixty-four the array has",
                        false => "",
                    },
                )
            }
            ServerMessage::ItemCleared(clear) => {
                format!(
                    "0x175 slot cleared — {} slot {}",
                    match clear.container {
                        Some(backpack::Container::Equipment) => "equipment".to_string(),
                        Some(backpack::Container::Carry) => "carry".to_string(),
                        Some(backpack::Container::Cargo) => "warehouse".to_string(),
                        None => format!("container {} (no arm)", clear.raw_container),
                    },
                    clear.slot,
                )
            }
            ServerMessage::PartyMember { opcode, member } => {
                format!(
                    "{opcode:#05x} party {} — {} (id {}, class {}, level {}, {}/{} hp)",
                    match *opcode {
                        PARTY_INVITED => "invitation, and the roster is replaced by it",
                        _ => "roster row",
                    },
                    member.name,
                    member.id,
                    member.class,
                    member.level,
                    member.hp,
                    member.max_hp,
                )
            }
            ServerMessage::PartyLeft(leave) => match leave {
                party::Leave::Dissolved => "0x37E party dissolved".to_string(),
                party::Leave::Member(id) => {
                    format!("0x37E party — member {id} left")
                }
            },
            ServerMessage::PartyMarks(marks) => {
                format!(
                    "0x3EA party marks — {} of {} ids for member {}",
                    marks.marked(),
                    party::MARKS,
                    marks.owner,
                )
            }
            ServerMessage::ShopSellList(list) => {
                format!(
                    "0x3E8 shop sell list — {} of {} rows, {} of them drawable",
                    list.filled(),
                    shop::SELL_SLOTS,
                    list.drawn(),
                )
            }
            ServerMessage::GroundItem(drop) => {
                format!(
                    "0x26E ground item — entity {} is item {} at ({}, {}){}",
                    drop.entity,
                    drop.item.index,
                    drop.x,
                    drop.z,
                    match drop.plays_sound() {
                        true => ", with the drop sound",
                        false => "",
                    },
                )
            }
            ServerMessage::Vanish { entity, vanish } => {
                format!(
                    "0x165 vanish — entity {entity}, mode {} ({})",
                    vanish.mode,
                    match vanish.mode {
                        0 => "armed, and still standing",
                        1 => "killed",
                        2 => "vanishing",
                        _ => "gone, behind a portal column",
                    },
                )
            }
            ServerMessage::Kill(death) => {
                format!(
                    "0x338 kill — entity {} killed entity {}; {} exp, {} cp",
                    death.killer, death.victim, death.exp, death.cp,
                )
            }
            ServerMessage::Attack { opcode, attack } => {
                format!(
                    "{opcode:#05x} attack — entity {} at ({}, {}), action {}{} → {}",
                    attack.attacker,
                    attack.to.0,
                    attack.to.1,
                    attack.action,
                    match attack.skill_id {
                        attack::SKILL_NONE => String::new(),
                        id => format!(", skill {id}"),
                    },
                    attack.describe_hits(),
                )
            }
            ServerMessage::System(message) => {
                format!(
                    "{:#05x} system message — kind {}, code {} (strdef {}){}",
                    self.opcode(),
                    message.kind,
                    message.code,
                    message.strdef(),
                    match message.argument.is_empty() {
                        true => String::new(),
                        false => format!(", {:?}", message.argument),
                    },
                )
            }
            ServerMessage::Chat(line) => {
                format!(
                    "{:#05x} chat — from client id {}{}, {}{}",
                    line.opcode,
                    line.speaker,
                    match line.name.is_empty() {
                        true => String::new(),
                        false => format!(" ({:?})", line.name),
                    },
                    match line.strdef() {
                        Some(record) => format!("strdef {record}"),
                        None => format!("{:?}", line.text),
                    },
                    match line.opcode == CHAT_CHANNEL {
                        true => format!(", channel {}", line.kind),
                        false => String::new(),
                    },
                )
            }
            ServerMessage::SlotPatch { entity, patch } => {
                format!(
                    "0x182 slot patch — entity {entity}, {} slot {} {}",
                    match patch.container {
                        live::Container::Equipment => "equipment".to_string(),
                        live::Container::Carry => "backpack".to_string(),
                        live::Container::Cargo => "warehouse".to_string(),
                        live::Container::Other(n) =>
                            format!("container {n}, which writes nothing,"),
                    },
                    patch.slot,
                    match patch.item.is_empty() {
                        true => "cleared".to_string(),
                        false => format!("← item {}", patch.item.index),
                    },
                )
            }
            ServerMessage::Vitals { entity, vitals } => {
                format!(
                    "0x181 vitals — entity {entity}, hp {}, mp {}{}",
                    vitals.hp,
                    vitals.mp,
                    match vitals.is_dead() {
                        true => " — dead",
                        false => "",
                    },
                )
            }
            ServerMessage::HpDelta { entity, delta } => {
                format!(
                    "0x18A hp delta — entity {entity}, hp {}, {}",
                    delta.hp,
                    match delta.text() {
                        Some(text) => format!("draws \"{text}\""),
                        None => "draws nothing".to_string(),
                    },
                )
            }
            ServerMessage::Emote { entity, emote } => {
                format!(
                    "0x36A emote — entity {entity}, code {}, {}",
                    emote.code,
                    match (emote.code, emote.mode) {
                        (emote::FIREWORK, pattern) => format!("a firework, pattern {pattern}"),
                        (_, emote::MODE_APPEARANCE) => "mode 1, the appearance ladder".to_string(),
                        (_, emote::MODE_UNDIE) => "mode 2, clears the death byte".to_string(),
                        (_, emote::MODE_LEVEL_UP) => "mode 3, a level-up burst".to_string(),
                        (code, _) if code >= emote::CODE_LIMIT =>
                            "refused, code >= 0x100".to_string(),
                        _ => "plays the code".to_string(),
                    },
                )
            }
            ServerMessage::Firework { entity, glyph } => {
                format!(
                    "0x3CA firework — entity {entity}, {} of {CELLS} cells lit{}",
                    glyph.cells.iter().filter(|c| **c).count(),
                    match glyph.forced() {
                        true => " (an empty mask, so one cell is forced on)",
                        false => "",
                    },
                )
            }
            ServerMessage::ActorStatus { entity, score } => {
                format!(
                    "0x336 score — entity {entity}, level {}, hp {}/{}, mp {}/{}, {} affect(s)",
                    score.score.shown_level(),
                    score.score.hp,
                    score.score.max_hp,
                    score.score.mp,
                    score.score.max_mp,
                    score
                        .affects
                        .iter()
                        .filter(|affect| affect.is_set())
                        .count(),
                )
            }
            ServerMessage::PlayerProgression { entity, score } => {
                format!(
                    "0x337 score 2 — entity {entity}, {} exp, {} gold",
                    score.exp, score.gold,
                )
            }
            ServerMessage::EquipChanged { entity, change } => {
                format!(
                    "0x36B equip — entity {entity}, body {}, hands {:?}, {} slot(s) worn",
                    change.equip[0],
                    change.hands(),
                    change.equip.iter().filter(|id| **id != 0).count(),
                )
            }
            ServerMessage::Gold { entity, gold } => {
                format!("0x3AF gold — entity {entity}, {}", gold.gold)
            }
            ServerMessage::NameMark { entity, mode } => {
                format!("0x193 name mark — entity {entity}, mode {mode}")
            }
            ServerMessage::GlowBurst { entity, mode } => {
                format!("0x3AD glow burst — entity {entity}, mode {mode}")
            }
            ServerMessage::GuildInvite { entity, invite } => {
                format!(
                    "0x3D5 guild invitation — entity {entity}, subject {}, {:?}",
                    invite.subject, invite.guild,
                )
            }
            ServerMessage::GuildApplication => "0x18D guild-application confirmation".to_string(),
            ServerMessage::Challenge { entity, challenge } => {
                format!(
                    "0x39F ranking-match challenge — entity {entity}, subject {}, stake {} ({})",
                    challenge.subject,
                    challenge.stake,
                    challenge.ratio().unwrap_or("off the table"),
                )
            }
            ServerMessage::Inventory { entity, bag } => {
                format!(
                    "0x185 inventory replaced — entity {entity}, {} of {} slots filled, {} gold",
                    bag.filled(),
                    inventory::SLOTS,
                    bag.gold,
                )
            }
            ServerMessage::Collection { entity, progress } => {
                format!(
                    "0x300 collection book — entity {entity}, {} of {} stated records complete",
                    progress.completed(),
                    collection::FLAG_BYTES,
                )
            }
            ServerMessage::NpcLine { entity, line } => {
                format!(
                    "0x107 NPC line — entity {entity}, kind {}, index {}",
                    line.kind, line.index,
                )
            }
            ServerMessage::TradeOffer { entity, offer } => {
                format!(
                    "0x383 trade offer — entity {entity}, partner {}, {} item(s), {} gold, ready {}",
                    offer.partner,
                    offer.filled(),
                    offer.gold,
                    offer.ready,
                )
            }
            ServerMessage::TradeClosed { entity } => {
                format!("0x384 trade closed — entity {entity}")
            }
            ServerMessage::TradeToggle { .. } => "0x386 trade toggle".to_string(),
            ServerMessage::ItemMoved { dst, src } => {
                format!(
                    "0x376 item moved — container {} slot {} ⇄ container {} slot {}",
                    src.0, src.1, dst.0, dst.1,
                )
            }
            ServerMessage::Sold(sale) => {
                format!(
                    "0x37A sold — shop {}, container {}, slot {}",
                    sale.shop, sale.container, sale.slot,
                )
            }
            ServerMessage::ShopClosed => "0x196 close the NPC item shop".to_string(),
            ServerMessage::CraftingClosed => "0x3A7 close the crafting panels".to_string(),
            ServerMessage::AnimationProbe(request) => {
                format!(
                    "0x1C1 animation probe — model {}, byte offset {}",
                    request.model, request.byte_offset,
                )
            }
            ServerMessage::Question(question) => {
                format!(
                    "0x1C6 question — {:?}, {} answer(s)",
                    question.text,
                    question.answers.len(),
                )
            }
            ServerMessage::LongMessage(message) => {
                format!(
                    "0x7B1 long field message — mode {}, {} non-empty line(s)",
                    message.mode,
                    message.lines.iter().filter(|line| !line.is_empty()).count(),
                )
            }
            ServerMessage::ReelResult(result) => {
                format!(
                    "0x1BF reel result — stops {:?}, payout {}, counter {}",
                    result.stops, result.payout, result.counter,
                )
            }
            ServerMessage::CloseCargo => "0x19C close the warehouse".to_string(),
            ServerMessage::ShopStock(stock) => {
                format!(
                    "0x17C {} — {} of {} slot(s) stocked",
                    match stock.kind {
                        shop::Kind::Shop => "shop stock".to_string(),
                        shop::Kind::SkillMaster => "the skill master".to_string(),
                        shop::Kind::Other(n) => format!("arm {n}, which does nothing"),
                    },
                    stock.items.iter().filter(|item| !item.is_empty()).count(),
                    shop::STOCK_SLOTS,
                )
            }
            ServerMessage::ShopContents(contents) => {
                format!(
                    "0x397 personal shop — owner {}, \"{}\", {} of {} slot(s) stocked",
                    contents.owner,
                    contents.title(),
                    contents.filled(),
                    autotrade::PANEL_SLOTS,
                )
            }
            ServerMessage::ShopSlotSold(sold) => {
                format!(
                    "0x39B personal shop slot {} of owner {} is gone",
                    sold.slot, sold.owner,
                )
            }
            ServerMessage::QuickBar { bar } => {
                format!(
                    "0x378 quick-slot bar — {} of {} slot(s) filled, pre-rebase",
                    bar.iter().filter(|b| **b != enter::QUICK_EMPTY).count(),
                    enter::QUICK_SLOTS,
                )
            }
            ServerMessage::Lock { opcode } => format!("{opcode:#05x} account lock"),
            ServerMessage::Quit => "0x3AE the server asked the client to quit".to_string(),
            ServerMessage::Redirect(redirect) => {
                format!(
                    "0x52A redirect — account {:?}, channel {:?}",
                    redirect.account,
                    redirect.channel(),
                )
            }
            ServerMessage::Other { opcode, payload } => {
                format!(
                    "{opcode:#05x} not built — {} payload byte(s)",
                    payload.len()
                )
            }
            ServerMessage::Malformed { opcode, reason } => {
                format!("{opcode:#05x} malformed — {reason}")
            }
        }
    }
}

fn text_at(payload: &[u8], at: usize) -> String {
    let Some(rest) = payload.get(at..) else {
        return String::new();
    };

    let end = rest.iter().position(|b| *b == 0).unwrap_or(rest.len());

    formats::text::codepage::decode(&rest[..end], formats::text::Codepage::Windows1252)
}

pub fn classify(packet: Packet) -> ServerMessage {
    let Packet {
        header, payload, ..
    } = packet;

    let malformed = |reason: String| ServerMessage::Malformed {
        opcode: header.opcode,
        reason,
    };

    match header.opcode {
        0x4307 => match formats::net::quest::parse(&payload) {
            Some(flags) => ServerMessage::QuestFlags(flags),
            None => malformed("invalid quest flag marker or truncated payload".into()),
        },
        0x4302 => ServerMessage::ClientConfig(formats::net::client_config::parse(&payload)),
        0x194 => ServerMessage::ServerReturnNotice,
        0x11C | 0x11D => ServerMessage::LoginDenied {
            opcode: header.opcode,
        },
        0x119 | 0x11A | 0x11B | 0x120 => ServerMessage::CharacterNotice {
            opcode: header.opcode,
        },
        0x11E => match account::parse_delete_hold(&payload) {
            Ok(hold) => ServerMessage::CharacterDeleteHold(hold),
            Err(error) => malformed(error.to_string()),
        },
        0x1D6 => match account::parse_selection_metadata(&payload) {
            Ok(value) => ServerMessage::SelectionMetadata(value),
            Err(error) => malformed(error.to_string()),
        },
        0xBBF | 0xFBC | 0x13BD | 0x7BE => {
            match formats::net::dynamic_key::parse(header.opcode, &payload) {
                Ok(update) => ServerMessage::DynamicKey {
                    opcode: header.opcode,
                    update,
                },
                Err(error) => malformed(error.to_string()),
            }
        }
        0x7A9 => ServerMessage::CharacterNameNotice,
        0xFAA => match account::parse_name_result(&payload) {
            Ok(reply) => ServerMessage::CharacterNameResult(reply),
            Err(error) => malformed(error.to_string()),
        },
        CHARACTER_SLOT_EFFECT => match account::parse_slot_effect(&payload) {
            Ok(slot) => ServerMessage::CharacterSlotEffect { slot },
            Err(error) => malformed(error.to_string()),
        },
        LOGIN_ACCEPTED => match account::parse_accept(&payload) {
            Ok(accept) => ServerMessage::LoginAccepted(Box::new(accept)),
            Err(error) => malformed(error.to_string()),
        },
        ACCOUNT_AFTER_DELETE | ACCOUNT_AFTER_CREATE => match account::parse_refresh(&payload) {
            Ok(block) => ServerMessage::AccountBlock {
                opcode: header.opcode,
                block: Box::new(block),
            },
            Err(error) => malformed(error.to_string()),
        },
        ENTER_WORLD => match enter::parse(&payload) {
            Ok(world) => ServerMessage::EnterWorld(Box::new(world)),
            Err(error) => malformed(error.to_string()),
        },
        LEAVE_WORLD => ServerMessage::LeaveWorld {
            client_id: header.client_id,
        },
        SYSTEM_MESSAGE | SYSTEM_MESSAGE_ARG if header.client_id != 0 => {
            match payload.first() == Some(&KIND_CHAT) {
                true => ServerMessage::Chat(Box::new(ChatLine::canned(
                    header.opcode,
                    header.client_id,
                    &payload,
                ))),
                false => ServerMessage::Other {
                    opcode: header.opcode,
                    payload,
                },
            }
        }
        SYSTEM_MESSAGE => ServerMessage::System(SystemMessage::parse(&payload, false)),
        SYSTEM_MESSAGE_ARG => ServerMessage::System(SystemMessage::parse(&payload, true)),
        CHAT_CONSOLE if header.client_id == 0 => {
            ServerMessage::Chat(Box::new(ChatLine::say(header.opcode, 0, &payload)))
        }
        CHAT_SAY | CHAT_SHOUT => ServerMessage::Chat(Box::new(ChatLine::say(
            header.opcode,
            header.client_id,
            &payload,
        ))),
        CHAT_CHANNEL => {
            ServerMessage::Chat(Box::new(ChatLine::channel(header.client_id, &payload)))
        }
        SPAWN | SPAWN_SHOP => match spawn::parse(&payload) {
            Ok(entity) => ServerMessage::Spawn {
                opcode: header.opcode,
                entity: Box::new(entity),
            },
            Err(error) => malformed(error.to_string()),
        },
        ATTACK | ATTACK_AREA | ATTACK_MANY => match attack::parse(header.opcode, &payload) {
            Ok(swing) => ServerMessage::Attack {
                opcode: header.opcode,
                attack: Box::new(swing),
            },
            Err(error) => malformed(error.to_string()),
        },
        MOVE | MOVE_RUN | ACT => match action::parse(&payload) {
            Ok(action) => ServerMessage::Action {
                opcode: header.opcode,
                entity: header.client_id,
                action: Box::new(action),
            },
            Err(error) => malformed(error.to_string()),
        },
        REMOVE => match payload.get(..2) {
            Some(id) => ServerMessage::Remove {
                entity: u16::from_le_bytes([id[0], id[1]]),
            },
            None => malformed("a removal needs two payload bytes".to_string()),
        },
        GATE_COMMAND => match gate_command::parse(&payload) {
            Ok(command) => ServerMessage::GateCommand(command),
            Err(error) => malformed(error.to_string()),
        },
        GROUND_ITEM => match ground_item::parse(&payload) {
            Ok(drop) => ServerMessage::GroundItem(Box::new(drop)),
            Err(error) => malformed(error.to_string()),
        },
        WAREHOUSE_SET | WAREHOUSE_DEPOSIT | WAREHOUSE_WITHDRAW => {
            match warehouse::parse(header.opcode, &payload) {
                Ok(moved) => ServerMessage::WarehouseGold(moved),
                Err(error) => malformed(error.to_string()),
            }
        }
        WEATHER => match weather::parse(&payload) {
            Ok(forecast) => ServerMessage::Weather(forecast),
            Err(error) => malformed(error.to_string()),
        },
        EVENT_COUNTDOWN => match event_hud::parse_countdown(&payload) {
            Ok(countdown) => ServerMessage::EventCountdown(countdown),
            Err(error) => malformed(error.to_string()),
        },
        EVENT_REMAINING => match event_hud::parse_remaining(&payload) {
            Ok(remaining) => ServerMessage::EventRemaining(remaining),
            Err(error) => malformed(error.to_string()),
        },
        EVENT_PACKED => match event_hud::parse_packed(&payload) {
            Ok(packed) => ServerMessage::EventPacked(packed),
            Err(error) => malformed(error.to_string()),
        },
        EVENT_TARGET => match event_hud::parse_target(&payload) {
            Ok(target) => ServerMessage::EventTarget(target),
            Err(error) => malformed(error.to_string()),
        },
        EVENT_SCORE => match event_hud::parse_score(&payload) {
            Ok(score) => ServerMessage::EventScore(score),
            Err(error) => malformed(error.to_string()),
        },
        SUMMON_REQUEST => match summon_request(&payload) {
            Ok(request) => ServerMessage::SummonRequest(request),
            Err(error) => malformed(error),
        },
        SUMMON_CANCEL => ServerMessage::SummonCancel,
        PLATE_LEVEL => match plate::parse_level(&payload) {
            Ok(change) => ServerMessage::PlateLevel(change),
            Err(error) => malformed(error.to_string()),
        },
        PLATE_GUILD => match plate::parse_guild(&payload) {
            Ok(line) => ServerMessage::PlateGuild(line),
            Err(error) => malformed(error.to_string()),
        },
        GUILD => match guild::parse(&payload) {
            Ok(membership) => ServerMessage::Guild(membership),
            Err(error) => malformed(error.to_string()),
        },
        SIEGE_STATE => match siege::parse(header.size, &payload) {
            Ok(state) => ServerMessage::SiegeState(state),
            Err(error) => malformed(error.to_string()),
        },
        WAR_FLAG => match siege::parse_war(&payload) {
            Ok(flag) => ServerMessage::WarFlag(flag),
            Err(error) => malformed(error.to_string()),
        },
        KICKOUT => match kickout::parse(&payload) {
            Ok(k) => ServerMessage::Kickout(k),
            Err(error) => malformed(error.to_string()),
        },
        0x1c2 => match formats::net::item_prices::parse(&payload) {
            Some(update) => ServerMessage::ItemPrices(update),
            None => malformed("item prices require eight payload bytes".into()),
        },
        opcode if CONSUMED.contains(&opcode) => ServerMessage::Consumed { opcode },
        FIELD_CARPET => match field_fx::parse_carpet(&payload) {
            Ok(carpet) => ServerMessage::FieldCarpet(carpet),
            Err(error) => malformed(error.to_string()),
        },
        FIELD_SOUND => match field_fx::parse_sound(&payload) {
            Ok(sound) => ServerMessage::FieldSound(sound),
            Err(error) => malformed(error.to_string()),
        },
        FIELD_LINE => match kept::parse_line(&payload) {
            Ok(line) => ServerMessage::FieldLine(Box::new(line)),
            Err(error) => malformed(error.to_string()),
        },
        KEYED_RECORD => {
            let mut frame = header.to_bytes().to_vec();

            frame.extend_from_slice(&payload);

            match kept::parse_record(&frame) {
                Ok(record) => ServerMessage::KeyedRecord(Box::new(record)),
                Err(error) => malformed(error.to_string()),
            }
        }
        ARENA_ROUND => match arena::parse_round(&payload) {
            Ok(round) => ServerMessage::ArenaRound(round),
            Err(error) => malformed(error.to_string()),
        },
        ARENA_KILL => match arena::parse_kill(&payload) {
            Ok(kill) => ServerMessage::ArenaKill(Box::new(kill)),
            Err(error) => malformed(error.to_string()),
        },
        ARENA_LIST => match arena::parse_list(&payload) {
            Ok(list) => ServerMessage::ArenaList(Box::new(list)),
            Err(error) => malformed(error.to_string()),
        },
        ITEM_ARRIVED => match backpack::parse_arrival(&payload) {
            Ok(arrival) => ServerMessage::ItemArrived(arrival),
            Err(error) => malformed(error.to_string()),
        },
        ITEM_CLEARED => match backpack::parse_clear(&payload) {
            Ok(clear) => ServerMessage::ItemCleared(clear),
            Err(error) => malformed(error.to_string()),
        },
        PARTY_JOINED | PARTY_INVITED => match party::parse_member(header.opcode, &payload) {
            Ok(member) => ServerMessage::PartyMember {
                opcode: header.opcode,
                member,
            },
            Err(error) => malformed(error.to_string()),
        },
        PARTY_LEFT => match party::parse_leave(&payload) {
            Ok(leave) => ServerMessage::PartyLeft(leave),
            Err(error) => malformed(error.to_string()),
        },
        PARTY_MARKED => match party::parse_marks(&payload) {
            Ok(marks) => ServerMessage::PartyMarks(marks),
            Err(error) => malformed(error.to_string()),
        },
        SHOP_SELL_LIST => match shop::parse_sell_list(&payload) {
            Ok(list) => ServerMessage::ShopSellList(Box::new(list)),
            Err(error) => malformed(error.to_string()),
        },
        VANISH => match live::parse_vanish(&payload) {
            Ok(vanish) => ServerMessage::Vanish {
                entity: header.client_id,
                vanish,
            },
            Err(error) => malformed(error.to_string()),
        },
        KILL => match kill::parse(&payload) {
            Ok(death) => ServerMessage::Kill(death),
            Err(error) => malformed(error.to_string()),
        },
        AFFECTS => match affect::parse(&payload) {
            Ok(affects) => ServerMessage::Affects {
                entity: header.client_id,
                affects: Box::new(affects),
            },
            Err(error) => malformed(error.to_string()),
        },
        SLOT_PATCH => match live::parse_slot_patch(&payload) {
            Ok(patch) => ServerMessage::SlotPatch {
                entity: header.client_id,
                patch,
            },
            Err(error) => malformed(error.to_string()),
        },
        VITALS => match live::parse_vitals(&payload) {
            Ok(vitals) => ServerMessage::Vitals {
                entity: header.client_id,
                vitals,
            },
            Err(error) => malformed(error.to_string()),
        },
        HP_DELTA => match live::parse_hp_delta(&payload) {
            Ok(delta) => ServerMessage::HpDelta {
                entity: header.client_id,
                delta,
            },
            Err(error) => malformed(error.to_string()),
        },
        FIREWORK => match firework::parse(&payload) {
            Ok(glyph) => ServerMessage::Firework {
                entity: header.client_id,
                glyph,
            },
            Err(error) => malformed(error.to_string()),
        },
        EMOTE => match emote::parse(&payload) {
            Ok(emote) => ServerMessage::Emote {
                entity: header.client_id,
                emote,
            },
            Err(error) => malformed(error.to_string()),
        },
        ACTOR_STATUS => match live::parse_actor_status(&payload) {
            Ok(score) => ServerMessage::ActorStatus {
                entity: header.client_id,
                score: Box::new(score),
            },
            Err(error) => malformed(error.to_string()),
        },
        PLAYER_PROGRESSION => match live::parse_player_progression(&payload) {
            Ok(score) => ServerMessage::PlayerProgression {
                entity: header.client_id,
                score,
            },
            Err(error) => malformed(error.to_string()),
        },
        EQUIP_CHANGED => match live::parse_equip(&payload) {
            Ok(change) => ServerMessage::EquipChanged {
                entity: header.client_id,
                change: Box::new(change),
            },
            Err(error) => malformed(error.to_string()),
        },
        GOLD => match live::parse_gold(&payload) {
            Ok(gold) => ServerMessage::Gold {
                entity: header.client_id,
                gold,
            },
            Err(error) => malformed(error.to_string()),
        },
        NAME_MARK => match body_mark::parse_mode(&payload) {
            Ok(mode) => ServerMessage::NameMark {
                entity: header.client_id,
                mode,
            },
            Err(error) => malformed(error.to_string()),
        },
        GLOW_BURST => match body_mark::parse_mode(&payload) {
            Ok(mode) => ServerMessage::GlowBurst {
                entity: header.client_id,
                mode,
            },
            Err(error) => malformed(error.to_string()),
        },
        GUILD_INVITE => match guild_invite::parse(&payload) {
            Ok(invite) => ServerMessage::GuildInvite {
                entity: header.client_id,
                invite,
            },
            Err(error) => malformed(error.to_string()),
        },
        GUILD_APPLICATION => ServerMessage::GuildApplication,
        CHALLENGE => match challenge::parse(&payload) {
            Ok(challenge) => ServerMessage::Challenge {
                entity: header.client_id,
                challenge,
            },
            Err(error) => malformed(error.to_string()),
        },
        INVENTORY => match inventory::parse(&payload) {
            Ok(bag) => ServerMessage::Inventory {
                entity: header.client_id,
                bag: Box::new(bag),
            },
            Err(error) => malformed(error.to_string()),
        },
        COLLECTION => match collection::parse(&payload) {
            Ok(progress) => ServerMessage::Collection {
                entity: header.client_id,
                progress,
            },
            Err(error) => malformed(error.to_string()),
        },
        NPC_LINE => match npc_line::parse(&payload) {
            Ok(line) => ServerMessage::NpcLine {
                entity: header.client_id,
                line,
            },
            Err(error) => malformed(error.to_string()),
        },
        TRADE_OFFER => match trade::parse(&payload) {
            Ok(offer) => ServerMessage::TradeOffer {
                entity: header.client_id,
                offer,
            },
            Err(error) => malformed(error.to_string()),
        },
        TRADE_CLOSED => ServerMessage::TradeClosed {
            entity: header.client_id,
        },
        TRADE_TOGGLE => ServerMessage::TradeToggle {
            entity: header.client_id,
        },
        ITEM_MOVED => match payload.get(..4) {
            Some(at) => ServerMessage::ItemMoved {
                dst: (at[0], at[1]),
                src: (at[2], at[3]),
            },
            None => malformed(format!(
                "an item move needs 4 payload bytes, got {}",
                payload.len()
            )),
        },
        SOLD => match shop::parse_sale(&payload) {
            Ok(sale) => ServerMessage::Sold(sale),
            Err(error) => malformed(error.to_string()),
        },
        SHOP_STOCK => match shop::parse(&payload) {
            Ok(stock) => ServerMessage::ShopStock(Box::new(stock)),
            Err(error) => malformed(error.to_string()),
        },
        SHOP_CONTENTS => match autotrade::parse(&payload) {
            Ok(contents) => ServerMessage::ShopContents(Box::new(contents)),
            Err(error) => malformed(error.to_string()),
        },
        SHOP_SLOT_SOLD => match autotrade::parse_sold(&payload) {
            Ok(sold) => ServerMessage::ShopSlotSold(sold),
            Err(error) => malformed(error.to_string()),
        },
        SHOP_CLOSED => ServerMessage::ShopClosed,
        CRAFTING_CLOSED => ServerMessage::CraftingClosed,
        ANIMATION_PROBE => match animation_probe::parse(&payload) {
            Ok(request) => ServerMessage::AnimationProbe(request),
            Err(error) => malformed(error.to_string()),
        },
        QUESTION => match question::parse(&payload) {
            Ok(question) => ServerMessage::Question(Box::new(question)),
            Err(error) => malformed(error.to_string()),
        },
        LONG_MESSAGE => match long_message::parse(&payload) {
            Ok(message) => ServerMessage::LongMessage(Box::new(message)),
            Err(error) => malformed(error.to_string()),
        },
        REEL_RESULT => match reel::parse(&payload) {
            Ok(result) => ServerMessage::ReelResult(Box::new(result)),
            Err(error) => malformed(error.to_string()),
        },
        CLOSE_CARGO => ServerMessage::CloseCargo,
        QUICK_BAR => match enter::parse_quick_bar(&payload) {
            Ok(bar) => ServerMessage::QuickBar { bar },
            Err(error) => malformed(error.to_string()),
        },
        LOCK | LOCK_RESET => ServerMessage::Lock {
            opcode: header.opcode,
        },
        QUIT => ServerMessage::Quit,
        REDIRECT => match payload.get(..REDIRECT_PAYLOAD_LEN) {
            Some(bytes) => {
                let exact: [u8; REDIRECT_PAYLOAD_LEN] = bytes
                    .try_into()
                    .expect("the redirect payload slice has the fixed length");

                ServerMessage::Redirect(Redirect {
                    client_id: header.client_id,
                    payload: exact,
                    account: text_at(&exact[..16], 0),
                    token: exact[16..].to_vec(),
                })
            }
            None => malformed(format!(
                "a redirect needs {REDIRECT_PAYLOAD_LEN} payload bytes, got {}",
                payload.len()
            )),
        },
        opcode => ServerMessage::Other { opcode, payload },
    }
}
