use crate::fonts::Chains;
use crate::frame::{AmulSheet, PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::carry;
use crate::scene::chat;
use crate::scene::destroy;
use crate::scene::enter::Entered;
use crate::scene::hover_tip;
use crate::scene::hud::{self, Hud};
use crate::scene::item_move::{self, Coord};
use crate::scene::item_tip;
use crate::scene::item_use;
use crate::scene::login_panels::{GuiTexture, PlacedPanel};
use crate::scene::login_static::Layout;
use crate::scene::mission;
use crate::scene::msgbox;
use crate::scene::notice;
use crate::scene::portal;
use crate::scene::screens;
use crate::scene::skill_master;
use crate::scene::spend;
use crate::scene::tooltip;
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_effect::Effects;
use formats::data::item_icon::{self, IconTable};
use formats::data::item_list::{self, ItemList};
use formats::mesh::appearance::refine_level;
use formats::net::account::Item;
use formats::ui::pane::ElementType;
use render::device::Gpu;
use render::texture::AlphaPolicy;
use std::collections::BTreeMap;
use ui::Strings;
use ui::draw::{Interaction, Owner};
use ui::input::{Event, EventId, Manager, PanelMut, PanelRef, Target};
use ui::text_entry::{Key, Local, Mods};
use ui::widget::cell_group;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registered {
    pub content: PanelId,
    pub class: &'static str,
    pub script: &'static [u8],
    pub shown: bool,
    pub page_of: Option<PanelId>,
}

pub const CONTENT_MSGBOX: PanelId = 0x01;

pub const BOX_CAPTION_ID: PanelId = 0x0001_0001;

pub const CONTENT_NOTICE: PanelId = notice::CONTENT;

pub const CONTENT_PARTY: PanelId = 0x13;

pub const CONTENT_CHARACTER: PanelId = 0x14;

pub const CONTENT_SKILL_MASTER: PanelId = skill_master::CONTENT;

pub const CONTENT_SKILL: PanelId = 0x15;

pub const CONTENT_INVENTORY: PanelId = 0x17;

pub const CONTENT_CARGO: PanelId = 0x18;

pub const CONTENT_ITEM_SHOP: PanelId = 0x1B;

pub const CONTENT_STORE: PanelId = 0x1C;

pub const STORE_CLOSE_BUTTON: &str = "CloseBtn";

pub const STORE_OPEN_BUTTON: &str = "Btn";

pub const TRADE_READY_BUTTON: &str = "CheckBtn";

pub const CONTENT_MISSION_MIX: PanelId = mission::CONTENT;

pub const CONTENT_ARENA: PanelId = 0x20;

pub const CONTENT_QUEST: PanelId = 0x1F;

pub const CONTENT_HELP: PanelId = 0x21;

pub const CONTENT_PORTAL: PanelId = 0x22;

pub const CONTENT_SYSTEM: PanelId = 0x25;

pub const CONTENT_COIN_INPUT: PanelId = 0x27;

pub const CONTENT_MOVE_SERVER: PanelId = 0x26;

pub const CONTENT_TRADE: PanelId = super::trade::CONTENT;

pub const CONTENT_FIELD_SCENE: PanelId = 0x0D;

pub const CONTENT_ITEM_MIX: PanelId = 0x19;

pub const CONTENT_ITEM_MIX4: PanelId = 0x1A;

pub const CONTENT_RPS: PanelId = 0x23;

pub const CONTENT_PGT_MENU: PanelId = 0x24;

pub const CONTENT_GUILD: PanelId = 0x2F;

pub const CONTENT_GUILD_INFO: PanelId = 0x30;

pub const CONTENT_GUILD_MEMBER: PanelId = 0x31;

pub const CONTENT_GUILD_NOTICE: PanelId = 0x33;

pub const CONTENT_GUILD_CREATE: PanelId = 0x34;

pub const CONTENT_REFINE: PanelId = 0x35;

pub const CONTENT_OPTION_STONE: PanelId = 0x36;

pub const CONTENT_COLLECTION: PanelId = super::collection::CONTENT;

pub const SCREENS: [Registered; 35] = [
    Registered {
        content: super::merge_item::CONTENT,
        class: "ProxyMergeItem",
        script: super::merge_item::SCRIPT,
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_MSGBOX,
        class: "PanelMsgBox",
        script: b"ui/Panel/Msgbox/msgbox_editbox.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_NOTICE,
        class: "PanelNotice",
        script: notice::SCRIPT,
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_PARTY,
        class: "PanelPartyInfo",
        script: b"ui/Panel/game/PartyInfo.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_CHARACTER,
        class: "PanelCharacterInfo",
        script: b"ui/Panel/game/Character_Base.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_SKILL_MASTER,
        class: "PanelSkillMaster",
        script: skill_master::SCRIPT,
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_SKILL,
        class: "PanelCharacterSkill",
        script: b"ui/Panel/game/Character_Skill.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_INVENTORY,
        class: "PanelInventory",
        script: b"ui/Panel/game/Inventory.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_CARGO,
        class: "PanelCargo",
        script: b"ui/Panel/game/ItemCargo.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_ITEM_SHOP,
        class: "PanelItemShop",
        script: b"ui/Panel/game/ItemShop.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_STORE,
        class: "PanelAutoTrade",
        script: b"ui/Panel/game/AutoTrade.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_PORTAL,
        class: "PanelPortal",
        script: b"ui/Panel/game/PortalList.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_MISSION_MIX,
        class: "PanelMissionMix",
        script: b"ui/Panel/game/ItemMix.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_QUEST,
        class: "PanelQuest",
        script: b"ui/Panel/game/Quest.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_ARENA,
        class: "PanelArena",
        script: b"ui/Panel/game/Arena.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_HELP,
        class: "PanelHelp",
        script: b"ui/Panel/game/Helpbook.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_SYSTEM,
        class: "PanelSystemMenu",
        script: b"ui/Panel/game/SystemMenu.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_MOVE_SERVER,
        class: "PanelMoveServer",
        script: b"ui/Panel/game/ServerSelectPopup.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_COIN_INPUT,
        class: "PanelMsgboxCoinInput",
        script: b"ui/Panel/Msgbox/msgbox_coininput.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_TRADE,
        class: "PanelTrade",
        script: b"ui/Panel/game/ItemTrade.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_FIELD_SCENE,
        class: "PanelFieldScene",
        script: b"ui/Panel/game/FieldScene.pane",
        shown: true,
        page_of: None,
    },
    Registered {
        content: CONTENT_FIELD_SCENE,
        class: "PanelFieldScene",
        script: b"ui/Panel/game/MiniPopup.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_ITEM_MIX,
        class: "PanelItemMix",
        script: b"ui/Panel/game/ItemMix.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_ITEM_MIX4,
        class: "PanelItemMix4",
        script: b"ui/Panel/game/ItemMix4.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_RPS,
        class: "PanelRPSGame",
        script: b"ui/Panel/game/RpsGame.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_PGT_MENU,
        class: "PanelPGTMenu",
        script: b"ui/Panel/game/PGTMenu.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_GUILD,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_Base.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_GUILD_INFO,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_Info.pane",
        shown: false,
        page_of: Some(CONTENT_GUILD),
    },
    Registered {
        content: CONTENT_GUILD_MEMBER,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_Member.pane",
        shown: false,
        page_of: Some(CONTENT_GUILD),
    },
    Registered {
        content: super::guild_panel::MEMBER_POPUP,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_MemberPopup.pane",
        shown: false,
        page_of: Some(CONTENT_GUILD),
    },
    Registered {
        content: CONTENT_GUILD_NOTICE,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_NoticeEdit.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_GUILD_CREATE,
        class: "PanelGuild",
        script: b"ui/Panel/game/Guild/Guild_Create.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_REFINE,
        class: "PanelItemRefine",
        script: b"ui/Panel/game/ItemRefine.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_OPTION_STONE,
        class: "PanelItemOptionStone",
        script: b"ui/Panel/game/ItemOptionStone.pane",
        shown: false,
        page_of: None,
    },
    Registered {
        content: CONTENT_COLLECTION,
        class: "PanelCollection",
        script: b"ui/Panel/game/Collection/Collection_Base.pane",
        shown: false,
        page_of: None,
    },
];

pub const CHAT_SCREENS: [Registered; 3] = [
    Registered {
        content: chat::CONTENT_CHAT,
        class: "PanelChat",
        script: chat::CHAT_INPUT,
        shown: true,
        page_of: None,
    },
    Registered {
        content: chat::CONTENT_CHAT,
        class: "PanelChat",
        script: chat::CHAT_LOG,
        shown: true,
        page_of: None,
    },
    Registered {
        content: chat::CONTENT_CHAT_MENU,
        class: "PanelChat",
        script: chat::CHAT_MENU,
        shown: false,
        page_of: None,
    },
];

pub const UNBUILT_CONTENT: [(PanelId, &str); 0] = [];

pub const STORE_PROMPT_STRDEF: usize = super::store::PROMPT_STRDEF;

pub fn by_name(name: &str) -> Option<PanelId> {
    let lower = name.trim().to_ascii_lowercase();

    let named = match lower.as_str() {
        "c" | "char" | "character" => Some(CONTENT_CHARACTER),
        "i" | "inven" | "inventory" => Some(CONTENT_INVENTORY),
        "x" | "quest" | "quests" => Some(CONTENT_QUEST),
        "h" | "help" => Some(CONTENT_HELP),
        "s" | "skill" | "skills" => Some(CONTENT_SKILL),
        "p" | "party" => Some(CONTENT_PARTY),
        "esc" | "system" | "config" => Some(CONTENT_SYSTEM),
        "server" | "channel" => Some(CONTENT_MOVE_SERVER),
        "store" | "atrade" | "shop" => Some(CONTENT_STORE),
        "portal" | "teleport" => Some(CONTENT_PORTAL),
        _ => None,
    };

    named.or_else(|| {
        let digits = lower.strip_prefix("0x").unwrap_or(&lower);

        PanelId::from_str_radix(digits, 16).ok()
    })
}

pub fn tab_name(content: PanelId, tab: usize) -> Option<String> {
    match content {
        CONTENT_CHARACTER => screens::SHEET_TABS.get(tab).map(|t| t.to_string()),
        CONTENT_INVENTORY => (tab < screens::BAG_TABS).then(|| format!("TabItem{tab}")),
        CONTENT_HELP => screens::HELP_TABS.get(tab).map(|t| t.to_string()),
        CONTENT_QUEST => screens::QUEST_TABS.get(tab).map(|t| t.to_string()),
        _ => None,
    }
}

pub fn list_name(content: PanelId) -> Option<&'static str> {
    if content == super::guild_panel::PAGE_MEMBER {
        return Some(super::guild_panel::MEMBER_LIST);
    }

    if content == CONTENT_MOVE_SERVER {
        return Some(super::listener::CHANNEL_VIEW);
    }

    (content == CONTENT_QUEST).then_some("TitleList")
}

pub fn select_row(
    panels: &mut Panels,
    entered: &mut Entered,
    content: PanelId,
    row: usize,
) -> Vec<Action> {
    let Some(name) = list_name(content) else {
        return Vec::new();
    };

    let Some(index) = panels.screens.iter().position(|s| s.content == content) else {
        return Vec::new();
    };

    if content != super::guild_panel::PAGE_MEMBER
        && let Some(screen) = panels.screens.get_mut(index)
        && let Some(list) = screen.element_mut(name, formats::ui::pane::ElementType::ListView)
    {
        let rect = list.rect();

        if let Some(state) = list.widget.list_mut() {
            state.selected = row as i32;

            state.reveal_selected(rect);
        }
    }

    let event = Event {
        panel: input_slot(index),
        id: if content == super::guild_panel::PAGE_MEMBER {
            EventId::TreeRightSelect
        } else {
            EventId::ListSelect
        },
        owner: None,
        name: name.to_string(),
        extra: row as i64,
    };

    handle(panels, entered, &[event], spend::Raise::default(), true)
}

pub fn screen_name(content: PanelId) -> &'static str {
    SCREENS
        .iter()
        .find(|s| s.content == content)
        .map(|s| s.class)
        .or_else(|| {
            UNBUILT_CONTENT
                .iter()
                .find(|(id, _)| *id == content)
                .map(|(_, name)| *name)
        })
        .unwrap_or("unknown")
}

pub fn unbuilt(content: PanelId) -> bool {
    UNBUILT_CONTENT.iter().any(|(id, _)| *id == content)
}

pub struct Screen {
    pub content: PanelId,
    pub class: &'static str,
    pub script: &'static [u8],
    pub placed: PlacedPanel,
    pub open: bool,
    pub page: i32,
    pub page_of: Option<PanelId>,
    pub cape_flag: Option<super::cape_flag::WidgetState>,
    pub(crate) combine_widgets: screens::CombineWidgets,
    pub(crate) mix_recipe_owners: [Option<ui::element::Instance>; 24],
    pub(crate) inventory_widgets: screens::InventoryWidgets,
    pub(crate) cargo_widgets: screens::CargoWidgets,
    pub(crate) refine_widgets: std::collections::BTreeMap<String, ui::element::Instance>,
    pub(crate) sheet_widgets: std::collections::BTreeMap<String, ui::element::Instance>,
    pub(crate) guild_member_list: Option<ui::element::Instance>,
    pub(crate) guild_popup_base: Option<ui::Panel>,
}

impl Screen {
    pub(crate) fn guild_member_list_mut(&mut self) -> Option<&mut ui::Element> {
        let owner = self.guild_member_list?;

        self.placed
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == owner && e.element_type == ElementType::ListView)
    }

    pub(crate) fn sheet_element(&self, name: &str, kind: ElementType) -> Option<&ui::Element> {
        let owner = self.sheet_widgets.get(name)?;

        self.placed
            .ui
            .panel
            .elements
            .iter()
            .find(|e| e.instance == *owner && e.element_type == kind)
    }

    pub(crate) fn sheet_element_mut(
        &mut self,
        name: &str,
        kind: ElementType,
    ) -> Option<&mut ui::Element> {
        let owner = *self.sheet_widgets.get(name)?;

        self.placed
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == owner && e.element_type == kind)
    }

    pub(crate) fn refine_element(&self, name: &str, kind: ElementType) -> Option<&ui::Element> {
        let owner = self.refine_widgets.get(name)?;

        self.placed
            .ui
            .panel
            .elements
            .iter()
            .find(|e| e.instance == *owner && e.element_type == kind)
    }

    pub(crate) fn refine_element_mut(
        &mut self,
        name: &str,
        kind: ElementType,
    ) -> Option<&mut ui::Element> {
        let owner = *self.refine_widgets.get(name)?;

        self.placed
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == owner && e.element_type == kind)
    }

    pub fn toggle(&mut self) -> bool {
        self.set_open(!self.open);

        self.open
    }

    pub fn set_open(&mut self, open: bool) {
        self.open = open;

        self.placed.ui.panel.visible = open;
    }

    pub fn z_order(&self) -> i32 {
        self.placed.ui.panel.z_order
    }

    pub fn element_mut(
        &mut self,
        name: &str,
        element_type: formats::ui::pane::ElementType,
    ) -> Option<&mut ui::Element> {
        self.placed.ui.panel.element_mut(name, element_type)
    }

    pub fn element(
        &self,
        name: &str,
        element_type: formats::ui::pane::ElementType,
    ) -> Option<&ui::Element> {
        self.placed.ui.panel.element(name, element_type)
    }
}

pub struct Panels {
    pub(crate) analytics: crate::analytics::Analytics,
    pub(crate) analytics_next_level: i32,
    pub guild_proxy: std::sync::Arc<std::sync::Mutex<super::guild_request::ProxyState>>,
    pub client_config: std::sync::Arc<formats::net::client_config::Config>,
    pub zone_time: super::zone_time::Shared,
    pub sealed_records: super::kept::Records,
    pub now_ms: u32,
    coin_focus_pending: Vec<Option<Target>>,
    guild_popup_position_pending: bool,
    hover_leaves_pending: Vec<PanelId>,
    pub spend_repeat: spend::Repeat,
    pub layout: Layout,
    pub screens: Vec<Screen>,
    pub cargo_open_pending: bool,
    pub item_use_clear_hover: bool,
    pub inventory_open_pending: bool,
    pub sheet_open_pending: bool,
    pub sheet_live_pending: bool,
    pub unconditional_sounds: Vec<i64>,
    pub inventory_partner: PanelId,
    pub inventory_block_equipment: bool,
    pub store_closed_pending: bool,
    pub trade_closed_pending: bool,
    pub field_close_owner_present: bool,
    pub trade_item_clicks: Vec<super::trade::ItemClick>,
    pub guimat_textures: BTreeMap<i32, GuiTexture>,
    pub hover_textures: BTreeMap<i32, GuiTexture>,
    pub hover_missing: Vec<i32>,
    pub missing: Vec<i32>,
    pub notes: Vec<String>,
    pub captions: bool,
    pub strings: Strings,
    pub help: Vec<Vec<screens::ColorLine>>,
    pub notice_requested: bool,
    pub community_requested: bool,
    pub settings_requested: bool,
    pub memo: Vec<screens::ColorLine>,
    pub quests: Vec<Vec<screens::Quest>>,
    quest_game: Option<GameDir>,
    quest_request_pending: bool,
    quest_last_request: u32,
    pub quest_response_pending: bool,
    pub items: ItemList,
    pub icons: IconTable,
    pub missions: Option<mission::Tables>,
    pub mixes: Option<formats::data::mix_list::MixList>,
    pub refine_sets: Vec<formats::data::item_set::SetBonus>,
    pub mix_help: super::mix::HelpTables,
    pub skills: Option<formats::data::skill_data::SkillData>,
    pub icon_sheets: BTreeMap<i32, GuiTexture>,
    pub missing_sheets: Vec<i32>,
    pub tooltip: tooltip::Tip,
    pub hover_tip: hover_tip::Tip,
    hover_source: Option<(Target, ui::element::Instance)>,
    icon_hover_stamp: Option<(ui::element::Instance, u64)>,
    icon_hover_rect: Option<(ui::element::Instance, [i32; 4])>,
    pub carry: carry::Carry,
    pub live: item_use::Live,
    pub cooldowns: item_move::Cooldowns,
    pub reserved: crate::scene::create::Reserved,
    pub chat: chat::Chat,
    pub notice: notice::Notice,
    pub master: skill_master::Master,
    pub skill_master_desc: String,
    pub potal_pos: Vec<formats::text::potal_pos::Row>,
    pub outbox: Vec<item_move::Request>,
    pub staged: Vec<item_move::Request>,
    pub firecracker_slot: Option<i32>,
    pub ticket_accepted: Option<Coord>,
    pub seal_open: Option<i32>,
    pub seal_name_slot: Option<i32>,
    pub wire: Vec<Vec<u8>>,
    pub gate_accepted: bool,
    pub summon_accepted: bool,
    pub guild_fame_accepted_ms: Option<u32>,
    pub chief_accepted: Option<String>,
    pub field_body_guild: Option<u16>,
    pub item_skill_accepted: Option<Coord>,
    pub guild_application_accepted: bool,
    pub death_return_accepted: bool,
    pub resurrect_accepted: bool,
    pub trade_accepted: Option<u16>,
    pub boxes: msgbox::Boxes,
    pub msgbox: msgbox::BoxPanel,
    pub prompts: [Option<destroy::Prompt>; msgbox::SLOTS],
    pub used: Vec<item_use::Action>,
    pub skill_casts: Vec<u8>,
    pub shop_clicks: Vec<super::shop::Click>,
    pub mission_clicks: Vec<mission::Click>,
    pub mission_reset: bool,
    pub mix_clicks: Vec<super::mix::Click>,
    pub mix: super::mix::Browser,
    pub collection_dirty: bool,
    pub collection_signed_up: bool,
    pub collection_cell: usize,
    pub collection_icon: Option<Owner>,
    pub collection_filter: super::collection::Filter,
    pub arena_cost: i32,
    pub pgt_target: Option<super::pgt::Party>,
    pub pgt_trade_requested: bool,
    pub pgt_clear_target: bool,
    pub pgt_toasts: Vec<usize>,
    pub pgt_messages: Vec<usize>,
    pub refine: super::refine::Refine,
    pub refine_command_locked: bool,
    pub guild: super::guild_panel::Guild,
    guild_page_rect: Option<[i32; 4]>,
    pub guild_requests: Vec<super::guild_request::Queued>,
    pub guild_identity: super::guild_request::Identity,
    pub(crate) guild_live_session: Option<super::guild_panel::Session>,
    guild_request_rng: super::rng::Lcg,
    pub guild_host: Option<String>,
    pub guild_ring: Option<super::guild_http::Ring>,
    pub staging: super::combine::Staging,
    pub shop_id: u16,
    pub shop_prices: item_tip::Shop,
    pub store: Option<formats::net::autotrade::Contents>,
    pub store_owner_opened: Option<String>,
    pub store_price: Option<super::store::PricePrompt>,
    pub coin_action: i32,
    pub split_stack: Option<super::stack_split::Prompt>,
    pub portal: portal::Portal,
    pub box_entities: [Option<u16>; msgbox::SLOTS],
    pub box_cells: [Option<Coord>; msgbox::SLOTS],
    pub tooltip_calendar: crate::clock::Calendar,
    pub tooltip_portuguese: bool,
    pub merge_item: super::merge_item::State,
    pub box_groupings: [Option<super::grouping::Pending>; msgbox::SLOTS],
    pub box_combines: [Option<item_move::CombinePrompt>; msgbox::SLOTS],
    pub box_skills: [Option<skill_master::Pending>; msgbox::SLOTS],
    pub notices: super::minipopup::Notices,
    pub summoner: String,
}

mod language;

pub(super) use language::LanguageUpdate;

impl Panels {
    pub(super) fn release_popup_focus(&mut self) {
        self.coin_focus_pending.push(None);
    }

    fn sealed_for(
        &mut self,
        cell: Option<tooltip::Cell>,
        client_id: u16,
        shift: bool,
    ) -> Option<item_tip::Sealed> {
        if shift && self.tooltip.visible() {
            return None;
        }

        let cell = cell?;

        let key = super::kept::seal_key(cell.index, &cell.item)?;

        let sealed = self.sealed_records.sealed(key);

        if sealed.is_none() {
            self.wire.push(super::kept::query_frame(client_id, key));
        }

        sealed
    }

    fn zone_clock_for(
        &mut self,
        cell: Option<tooltip::Cell>,
        client_id: u16,
        shift: bool,
    ) -> Option<i64> {
        if shift && self.tooltip.visible()
            || !cell.is_some_and(|cell| self.tooltip.needs_zone_clock(cell.index, cell.item.index))
        {
            return None;
        }

        let mut clock = self.zone_time.get();

        let (seconds, query) = clock.enter(self.now_ms);

        self.zone_time.set(clock);

        if query {
            self.wire.push(super::zone_time::query_frame(client_id));
        }

        Some(seconds)
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut crate::ui::PanelUi)) {
        for screen in &mut self.screens {
            visit(&mut screen.placed.ui);
        }

        self.msgbox.for_each_ui(visit);

        self.tooltip.for_each_ui(visit);

        self.hover_tip.for_each_ui(visit);

        self.carry.for_each_ui(visit);
    }

    pub fn tick_boxes(&mut self, now_ms: u32) {
        self.now_ms = now_ms;

        for slot in self.boxes.expire_notices(now_ms) {
            self.box_skills[slot] = None;

            self.prompts[slot] = None;

            self.box_entities[slot] = None;

            self.box_cells[slot] = None;

            self.box_combines[slot] = None;

            self.box_groupings[slot] = None;
        }

        self.msgbox.follow(&self.boxes);
    }

    pub fn focused_notepad(
        &mut self,
        manager: &ui::input::Manager,
    ) -> Option<(&mut Screen, usize)> {
        if self.msgbox.placed.ui.panel.visible || self.coin_input_text().is_some() {
            return None;
        }

        let target = manager.focused?;

        let Owner::Element(index) = target.owner else {
            return None;
        };

        let screen = self
            .screens
            .get_mut(target.panel.checked_sub(HUD_PANELS)?)?;

        let element = screen.placed.ui.panel.elements.get(index)?;

        (screen.open
            && screen.placed.ui.panel.visible
            && element.visible
            && element.enabled
            && element.widget.edit().is_some_and(|e| e.note_pad))
        .then_some((screen, index))
    }

    pub fn raise_box(
        &mut self,
        request: &msgbox::Request,
        prompt: Option<destroy::Prompt>,
    ) -> Option<usize> {
        debug_assert!(self.msgbox.covers(request), "a raiser named another .pane");

        let strings = self.strings.clone();

        let slot = self.boxes.raise(request, &strings)?;

        for (index, prompt) in self.prompts.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *prompt = None;
            }
        }

        for (index, entity) in self.box_entities.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *entity = None;
            }
        }

        for (index, cell) in self.box_cells.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *cell = None;
            }
        }

        self.prompts[slot] = prompt;

        for (index, prompt) in self.box_combines.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *prompt = None;
            }
        }

        self.box_combines[slot] = None;

        for (index, pending) in self.box_groupings.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *pending = None;
            }
        }

        self.box_groupings[slot] = None;

        for (index, prompt) in self.box_skills.iter_mut().enumerate() {
            if self.boxes.open().all(|(open, _)| open != index) {
                *prompt = None;
            }
        }

        self.box_skills[slot] = None;

        self.box_entities[slot] = None;

        self.box_cells[slot] = None;

        self.msgbox.raised(&self.boxes, slot);

        Some(slot)
    }

    pub fn raise_entity_box(&mut self, request: &msgbox::Request, entity: u16) -> Option<usize> {
        let slot = self.raise_box(request, None)?;

        self.box_entities[slot] = Some(entity);

        Some(slot)
    }

    pub fn raise_box_with_edit_limit(
        &mut self,
        request: &msgbox::Request,
        limit: i32,
    ) -> Option<usize> {
        let slot = self.raise_box(request, None)?;

        self.boxes.set_edit_limit(slot, limit);

        self.msgbox.follow(&self.boxes);

        Some(slot)
    }

    pub fn raise_cell_box(&mut self, request: &msgbox::Request, at: Coord) -> Option<usize> {
        let slot = self.raise_box(request, None)?;

        self.box_cells[slot] = Some(at);

        Some(slot)
    }

    pub fn box_hit(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let slot = self.msgbox.slot?;

        if !self.msgbox.placed.ui.panel.visible {
            return None;
        }

        self.msgbox.hit(x, y).map(|yes| (slot, yes))
    }

    pub fn box_key(&self, ch: char) -> Option<(usize, bool)> {
        let slot = self.msgbox.slot?;

        if !self.msgbox.placed.ui.panel.visible {
            return None;
        }

        let yes = match ch {
            '\r' | '\n' | 'Y' | 'y' => true,
            'N' | 'n' | '\u{1b}' => false,
            _ => return None,
        };

        let name = if yes {
            msgbox::ELEMENT_OK
        } else {
            msgbox::ELEMENT_CANCEL
        };

        let live = self
            .msgbox
            .placed
            .ui
            .panel
            .element(name, ElementType::Button)
            .is_some_and(|e| e.visible && e.enabled);

        live.then_some((slot, yes))
    }

    pub fn box_edit_text(&self) -> Option<&str> {
        if !self.msgbox.placed.ui.panel.visible {
            return None;
        }

        let element = self
            .msgbox
            .placed
            .ui
            .panel
            .element(msgbox::ELEMENT_EDIT, ElementType::ImeEditBox)?;

        if !element.visible || !element.enabled {
            return None;
        }

        element.widget.edit().map(|edit| edit.text.as_str())
    }

    pub fn box_type_char(&mut self, ch: char) -> bool {
        self.msgbox.character(ch, &mut Local::default()).is_some()
    }

    pub fn box_edit_key(&mut self, key: Key, mods: Mods) -> bool {
        if matches!(key, Key::Return | Key::Escape) {
            return false;
        }

        self.msgbox.key(key, mods, &mut Local::default()).is_some()
    }

    fn coin_input_edit_mut(&mut self) -> Option<&mut ui::EditState> {
        let screen = self.screen_mut(CONTENT_COIN_INPUT)?;

        if !screen.open || !screen.placed.ui.panel.visible {
            return None;
        }

        let element = screen.element_mut("edit", ElementType::ImeEditBox)?;

        if !element.visible || !element.enabled {
            return None;
        }

        element.widget.edit_mut()
    }

    pub fn coin_input_text(&self) -> Option<&str> {
        let screen = self.screen(CONTENT_COIN_INPUT)?;

        if !screen.open || !screen.placed.ui.panel.visible {
            return None;
        }

        let element = screen.element("edit", ElementType::ImeEditBox)?;

        if !element.visible || !element.enabled {
            return None;
        }

        element.widget.edit().map(|edit| edit.text.as_str())
    }

    pub fn coin_input_type_char(&mut self, ch: char) -> bool {
        let Some(edit) = self.coin_input_edit_mut() else {
            return false;
        };

        let events = ui::text_entry::character(edit, ch, &mut Local::default());

        if events.contains(&ui::text_entry::Raised::Changed) {
            self.coin_input_changed();
        }

        true
    }

    pub fn coin_input_changed(&mut self) {
        let action = self.coin_action;

        let Some(edit) = self
            .screen_mut(CONTENT_COIN_INPUT)
            .and_then(|s| s.element_mut("edit", ElementType::ImeEditBox))
            .and_then(|e| e.widget.edit_mut())
        else {
            return;
        };

        let Some(text) = coin_input_replacement(&edit.text, action) else {
            return;
        };

        self.coin_input_set_text(text);
    }

    pub fn coin_input_set_text(&mut self, text: String) {
        let Some(edit) = self
            .screen_mut(CONTENT_COIN_INPUT)
            .and_then(|s| s.element_mut("edit", ElementType::ImeEditBox))
            .and_then(|e| e.widget.edit_mut())
        else {
            return;
        };

        edit.text = text;

        edit.scroll = 0;

        edit.caret = edit.text.chars().count() as i32;

        edit.anchor = edit.caret;

        edit.preedit = None;

        edit.reset_caret();
    }

    pub fn begin_coin_money(&mut self, action: i32, strdef: u32) -> bool {
        if self.screen(CONTENT_COIN_INPUT).is_none() {
            return false;
        }

        self.set_open(CONTENT_COIN_INPUT, true);

        self.coin_action = action;

        self.split_stack = None;

        self.store_price = None;

        self.seal_name_slot = None;

        let body = self.strings.translate(&format!("ss_{strdef}"));

        let strings = self.strings.clone();

        let screen = self.screen_mut(CONTENT_COIN_INPUT).unwrap();

        screens::coin_input_fill(screen, &body);

        let resource = screen.placed.ui.resource.clone();

        if let Some(label) = screen.element_mut("msg", ElementType::Static) {
            label.apply(
                &formats::ui::pane::Property {
                    key: "Caption".into(),
                    value: format!("ss_{strdef}"),
                },
                &resource,
                &strings,
            );
        }

        true
    }

    pub fn coin_input_edit_key(&mut self, key: Key, mods: Mods) -> bool {
        if matches!(key, Key::Return | Key::Escape) {
            return false;
        }

        let Some(edit) = self.coin_input_edit_mut() else {
            return false;
        };

        let events = ui::text_entry::key(edit, key, mods, &mut Local::default());

        if events.contains(&ui::text_entry::Raised::Changed) {
            self.coin_input_changed();
        }

        true
    }

    pub fn coin_input_key(&self, ch: char) -> Option<bool> {
        if !self.is_open(CONTENT_COIN_INPUT) {
            return None;
        }

        let yes = match ch {
            '\r' | '\n' => true,
            '\u{1b}' => false,
            _ => return None,
        };

        let name = if yes { "ok" } else { "cancel" };

        self.screen(CONTENT_COIN_INPUT)?
            .placed
            .ui
            .panel
            .element(name, ElementType::Button)
            .is_some_and(|element| element.visible && element.enabled)
            .then_some(yes)
    }

    pub fn take_store_owner_opened(&mut self) -> Option<String> {
        self.store_owner_opened.take()
    }

    pub fn take_summon_accepted(&mut self) -> bool {
        std::mem::take(&mut self.summon_accepted)
    }

    pub fn take_item_skill_accepted(&mut self) -> Option<Coord> {
        self.item_skill_accepted.take()
    }

    pub fn take_guild_application_accepted(&mut self) -> bool {
        std::mem::take(&mut self.guild_application_accepted)
    }

    pub fn take_death_return_accepted(&mut self) -> bool {
        std::mem::take(&mut self.death_return_accepted)
    }

    pub fn take_resurrect_accepted(&mut self) -> bool {
        std::mem::take(&mut self.resurrect_accepted)
    }

    pub fn answer_box(
        &mut self,
        entered: &mut Entered,
        slot: usize,
        yes: bool,
        now_ms: u32,
    ) -> Option<(i32, u32, i32)> {
        let previous = self.field_body_guild;

        self.field_body_guild = Some(entered.mob_guild);

        let result = self.answer_box_in_field(
            entered,
            slot,
            yes,
            FieldInput {
                now_ms,
                player_present: true,
                player_dead: false,
            },
        );

        self.field_body_guild = previous;

        result
    }

    pub fn answer_box_in_field(
        &mut self,
        entered: &mut Entered,
        slot: usize,
        yes: bool,
        input: FieldInput,
    ) -> Option<(i32, u32, i32)> {
        let now_ms = input.now_ms;

        let edit_text = self.box_edit_text().map(str::to_owned);

        if yes && self.msgbox.slot == Some(slot) && edit_text.as_deref() == Some("") {
            self.boxes.hide(slot);

            self.msgbox.follow(&self.boxes);

            return None;
        }

        let answered = self.boxes.answer(slot, yes)?;

        self.msgbox.follow(&self.boxes);

        let prompt = self.prompts[slot].take();

        let entity = self.box_entities[slot].take();

        let cell = self.box_cells[slot].take();

        let grouping = self.box_groupings[slot].take();

        let combine = self.box_combines[slot].take();

        let skill = self.box_skills[slot].take();

        let (kind, context, answer) = answered;

        if kind == super::merge_item::CONTENT as i32 && context == 0x5a {
            if msgbox::answered(answer) && input.player_present {
                super::merge_item::confirm(self, entered);
            }

            return Some(answered);
        }

        if kind == 0xD && !input.player_present {
            return Some(answered);
        }

        if kind == 0xD && context == 0x24 {
            if msgbox::answered(answer) {
                self.chief_accepted = edit_text;
            }

            return Some(answered);
        }

        if kind == 0xD && context == 0x29 {
            if msgbox::answered(answer)
                && let Some(prompt) = prompt
                && cell
                    == Some(Coord {
                        container: 1,
                        slot: prompt.slot,
                    })
                && prompt.item_id != 0
                && let Some(request) = super::shop::confirm(entered, 0, &prompt)
            {
                self.outbox.push(request);
            }

            return Some(answered);
        }

        if kind == 0xD && context == 0x3E {
            if msgbox::answered(answer)
                && input.player_present
                && let Some(prompt) = combine
            {
                let outcome = item_move::confirm_combine(entered, &self.items, prompt);

                complete_item_drop(self, entered, outcome);
            }

            return Some(answered);
        }

        if kind == 0xD && context == 0x37 {
            if msgbox::answered(answer)
                && input.player_present
                && let Some(entity) = entity
            {
                self.wire
                    .push(super::party::leave_frame(entered.client_id, entity));
            }

            return Some(answered);
        }

        if kind == CONTENT_MISSION_MIX as i32 && context == 0x1A {
            if msgbox::answered(answer) {
                self.mission_clicks.push(mission::Click::Confirm);
            }

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND && context == 0x28 {
            if msgbox::answered(answer) {
                self.item_skill_accepted = cell;
            }

            return Some(answered);
        }

        if kind == super::store::PROMPT_KIND && context == super::store::PROMPT_CONTEXT {
            if msgbox::answered(answer)
                && input.player_present
                && let Some(title) = edit_text
                && self.screen(CONTENT_STORE).is_some()
                && !self.is_open(CONTENT_STORE)
            {
                self.store_owner_opened = Some(title);
            }

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND && context == u32::from(item_use::action::TICKET)
        {
            if msgbox::answered(answer) && input.player_present {
                self.ticket_accepted = cell;
            }

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == u32::from(item_use::action::CONFIRM)
            && msgbox::answered(answer)
            && let Some(at) = cell
        {
            let item = item_move::item_at(entered, at).unwrap_or_default();

            let id = i32::from(item.index as i16);

            let actions = {
                let Panels {
                    items,
                    cooldowns,
                    live,
                    ..
                } = &mut *self;

                let effects = formats::data::item_effect::Effects::new(items, &[]);

                let kind = effects.total_scaled(&item, item_move::EFFECT_KIND);

                item_use::dispatch(
                    entered,
                    items,
                    cooldowns,
                    kind,
                    id,
                    at,
                    now_ms,
                    live.position,
                )
                .actions(kind, id, at)
            };

            carry_out(self, entered, &actions, now_ms);

            self.used.extend(actions);

            return Some(answered);
        }

        if let Some(combine_kind) = super::combine::Staging::kind_of(kind as PanelId)
            && context == combine_kind.confirm_box()
        {
            if msgbox::answered(answer) {
                if combine_kind == super::combine::Kind::Mix {
                    if !input.player_present {
                        return Some(answered);
                    }

                    let Some(list) = self.mixes.as_ref() else {
                        return Some(answered);
                    };

                    if let Err(why) = super::mix::ready(
                        list,
                        &self.mix,
                        self.staging.get(combine_kind),
                        entered.gold,
                    ) {
                        let text = self.strings.translate(&format!("ss_{}", why.strdef()));

                        super::notice::post(self, &text, 2000, true, now_ms);

                        return Some(answered);
                    }

                    let index = self
                        .mix
                        .picked
                        .expect("ready validated the selected recipe");

                    let recipe = list
                        .recipe(index)
                        .expect("ready validated the recipe table");

                    self.staging
                        .get_mut(combine_kind)
                        .set_recipe(index as u16, recipe.opcode);
                } else {
                    match super::combine::ready(self.staging.get(combine_kind)) {
                        Err(record) => {
                            let text = self.strings.translate(&format!("ss_{record}"));

                            super::notice::post(self, &text, 2000, true, now_ms);

                            return Some(answered);
                        }
                        Ok(false) => return Some(answered),
                        Ok(true) => {}
                    }
                }

                let frame = super::combine::wire(self.staging.get(combine_kind));

                self.wire.push(frame);
            }

            return Some(answered);
        }

        if kind == super::guild_panel::BOX_KIND {
            if !input.player_present {
                return Some(answered);
            }

            let text = edit_text;

            let client = entered.client_id;

            let actions = super::guild_panel::answered(
                &self.guild,
                context,
                msgbox::answered(answer),
                text.as_deref(),
                client,
            );

            guild_carry_out(self, &actions);

            if !actions.is_empty() {
                self.guild_refresh_selected();
            }

            return Some(answered);
        }

        if kind == CONTENT_ARENA as i32
            && context == super::arena::BOX_CONTEXT
            && msgbox::answered(answer)
        {
            if self.is_open(CONTENT_ARENA) && input.player_present {
                self.wire.push(super::arena::accept().to_vec());
            }

            return Some(answered);
        }

        if kind == super::minipopup::SUMMON_BOX.0
            && context == super::minipopup::SUMMON_BOX.1
            && msgbox::answered(answer)
        {
            self.summon_accepted = true;

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == super::interact::GUILD_APPLICATION_CONTEXT
        {
            if msgbox::answered(answer) {
                self.guild_application_accepted = true;
            }

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == super::pending::DEATH_BOX_CONTEXT
            && msgbox::answered(answer)
        {
            self.death_return_accepted = true;

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == super::pending::RESURRECT_BOX_CONTEXT
            && msgbox::answered(answer)
        {
            self.resurrect_accepted = true;

            return Some(answered);
        }

        if kind == super::collection::CONTENT as i32
            && context == super::collection::SIGN_UP_BOX
            && msgbox::answered(answer)
        {
            self.collection_signed_up = true;

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == super::gate::CONTEXT
            && msgbox::answered(answer)
        {
            self.gate_accepted = true;

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == super::trade::BOX_CONTEXT
            && msgbox::answered(answer)
            && let Some(entity) = entity
        {
            self.trade_accepted = Some(entity);

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == formats::net::guild_invite::CONTEXT
            && msgbox::answered(answer)
            && let Some(entity) = entity
        {
            self.wire.push(super::guild_invite::reply_frame(entity));

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && context == formats::net::challenge::CONTEXT
            && msgbox::answered(answer)
            && let Some(entity) = entity
        {
            self.wire.push(super::challenge::reply_frame(entity));

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && msgbox::answered(answer)
            && let Some(payload) = entity
            && matches!(context, 0x38 | 0x39 | 0x3A | 0x3D)
        {
            let local_guild = if context == 0x38 {
                0
            } else {
                let Some(guild) = self.field_body_guild else {
                    return Some(answered);
                };

                guild
            };

            if let Some(frame) = super::pgt::guild_answer(
                context,
                entered.client_id,
                local_guild,
                u32::from(payload),
            ) {
                self.wire.push(frame);
            }

            return Some(answered);
        }

        if kind == super::interact::TALK_BOX_KIND
            && msgbox::answered(answer)
            && let Some(flag) = super::interact::talk_confirm(context)
            && let Some(entity) = entity
        {
            self.wire.push(super::interact::confirm_frame(entity, flag));

            if context == super::interact::talk_box::GUILD_FAME {
                self.guild_fame_accepted_ms = Some(now_ms);
            }

            return Some(answered);
        }

        if kind == skill_master::KIND && context == skill_master::CONTEXT {
            self.master.pending = None;

            if msgbox::answered(answer)
                && input.player_present
                && let Some(pending) = skill
            {
                skill_master::confirm(self, pending);
            }

            return Some(answered);
        }

        if kind == destroy::KIND
            && context == destroy::CONTEXT
            && msgbox::answered(answer)
            && let Some(prompt) = prompt
            && let Some(request) = destroy::confirm(entered, &prompt)
        {
            self.outbox.push(request);

            if input.player_present {
                let Self {
                    screens,
                    items,
                    icons,
                    strings,
                    ..
                } = self;

                if let Some(screen) = screens
                    .iter_mut()
                    .find(|s| s.content == CONTENT_INVENTORY && s.open)
                {
                    screens::inventory_bag_refresh(screen, entered, items, icons, strings);
                }
            }
        }

        if kind == 0x27 && context == 0x5e {
            if msgbox::answered(answer)
                && let Some(request) = grouping.and_then(|p| p.confirm(entered))
            {
                self.outbox.push(request);
            }

            return Some(answered);
        }

        if kind == 0x27 && context == 0x5b {
            if !msgbox::answered(answer) && self.split_stack.is_some_and(|p| p.individual) {
                super::stack_split::toggle_individual(self);
            }

            return Some(answered);
        }

        if kind == 0x27 && context == 0x5c {
            if !msgbox::answered(answer) && self.store_price.is_some_and(|p| p.display_only) {
                super::store::toggle_exhibition(self);
            }

            return Some(answered);
        }

        if kind == 0x27 && context == 0x5d {
            if msgbox::answered(answer) && input.player_present && !input.player_dead {
                super::store::answer_price_confirmed(self, entered, true, now_ms, true);
            }

            return Some(answered);
        }

        if kind == super::store::KIND
            && context == super::store::CONTEXT
            && msgbox::answered(answer)
            && let Some(slot) = entity
            && let Some(buy) = super::store::confirm(self, entered, usize::from(slot))
        {
            self.wire.push(super::store::frame(&buy));
        }

        if kind == super::shop::KIND
            && context == super::shop::CONTEXT
            && msgbox::answered(answer)
            && let Some(prompt) = prompt
            && let Some(request) = super::shop::confirm(entered, self.shop_id, &prompt)
        {
            self.outbox.push(request);
        }

        Some(answered)
    }

    pub fn refresh_mission(&mut self, state: &mission::State) {
        if self.is_open(CONTENT_MISSION_MIX) {
            self.pair_inventory(CONTENT_MISSION_MIX, true);
        }

        self.mission_reset = false;

        let Panels {
            screens, missions, ..
        } = self;

        let Some(tables) = missions.as_ref() else {
            return;
        };

        for screen in screens
            .iter_mut()
            .filter(|s| s.content == CONTENT_MISSION_MIX)
        {
            super::screens::mission_refresh(screen, state, tables);
        }
    }

    pub fn refresh_mix(&mut self) {
        let Panels {
            screens,
            mixes,
            mix_help,
            items,
            icons,
            mix,
            strings,
            ..
        } = self;

        let Some(list) = mixes.as_ref() else {
            return;
        };

        for screen in screens
            .iter_mut()
            .filter(|s| s.content == super::mix::CONTENT)
        {
            super::screens::mix_refresh(screen, mix, list, mix_help, items, icons, strings);
        }
    }

    pub fn open_mix(&mut self, filter: (i32, i32, i32), gated: bool) -> bool {
        let Some(list) = self.mixes.clone() else {
            return false;
        };

        if self.screen(super::mix::CONTENT).is_none() {
            return false;
        }

        if !self.is_open(super::mix::CONTENT) {
            self.set_open(super::mix::CONTENT, true);
        }

        self.pair_inventory(super::mix::CONTENT, true);

        let needs = std::mem::take(&mut self.mix.needs);

        self.mix = super::mix::open(&list, filter, gated);

        self.mix.needs = needs;

        if let Some(inventory) = self.screen_mut(CONTENT_INVENTORY) {
            for slot in 0..60 {
                screens::inventory_lock(inventory, slot, false);
            }
        }

        self.refresh_mix();

        true
    }

    pub fn screen(&self, content: PanelId) -> Option<&Screen> {
        self.screens.iter().find(|s| s.content == content)
    }

    pub fn screen_mut(&mut self, content: PanelId) -> Option<&mut Screen> {
        self.screens.iter_mut().find(|s| s.content == content)
    }

    pub fn is_open(&self, content: PanelId) -> bool {
        self.screen(content).is_some_and(|s| s.open)
    }

    pub fn pair_inventory(&mut self, partner: PanelId, block_equipment: bool) {
        if self.screen(CONTENT_INVENTORY).is_none() {
            return;
        }

        if self.inventory_partner != 0 {
            self.clear_inventory_reservations();

            if self.inventory_partner != partner {
                for owner in [
                    0x19,
                    0x1C,
                    0x1D,
                    0x1B,
                    0x1A,
                    0x1E,
                    0x18,
                    0x35,
                    0x36,
                    super::merge_item::CONTENT,
                ] {
                    if self.inventory_partner == owner && self.is_open(owner) {
                        self.toggle(owner);
                    }
                }
            }
        }

        self.inventory_block_equipment = block_equipment;

        self.inventory_partner = partner;

        self.clear_inventory_reservations();
    }

    fn clear_inventory_reservations(&mut self) {
        if let Some(bag) = self.screen_mut(CONTENT_INVENTORY) {
            for slot in 0..60 {
                screens::inventory_lock(bag, slot, false);
            }
        }
    }

    pub fn toggle(&mut self, content: PanelId) -> Option<bool> {
        let open = self.screen_mut(content).map(Screen::toggle)?;

        if Self::shared_toggle(content) {
            self.tooltip.hide();

            self.hover_tip.hide();
        }

        if open {
            self.opened(content);

            self.place_opened(content);
        } else {
            if Self::shared_toggle(content) {
                self.hover_leaves_pending.push(content);
            }

            self.closed(content);
        }

        Some(open)
    }

    pub fn close_by_toggle(&mut self, content: PanelId) -> Option<bool> {
        if self.is_open(content) {
            self.toggle(content)
        } else {
            self.screen(content).map(|_| false)
        }
    }

    fn guild_close_callback(&mut self, content: PanelId) {
        if content == super::guild_panel::NOTICE_EDIT {
            if let Some(screen) = self.screen_mut(content) {
                screen.set_open(false);
            }

            self.coin_focus_pending.push(None);
        } else if self.is_open(CONTENT_GUILD) {
            self.tooltip.hide();

            self.hover_tip.hide();

            self.set_open(CONTENT_GUILD, false);
        }
    }

    pub fn set_open(&mut self, content: PanelId, open: bool) -> Option<bool> {
        let was_open = self.is_open(content);

        if !open && !was_open && Self::shared_close_and_placement(content) {
            return self.screen(content).map(|_| false);
        }

        if matches!(
            content,
            super::mix::CONTENT
                | CONTENT_MISSION_MIX
                | super::refine::CONTENT
                | CONTENT_COLLECTION
                | CONTENT_PGT_MENU
                | CONTENT_SYSTEM
                | CONTENT_ARENA
                | CONTENT_HELP
                | CONTENT_ITEM_SHOP
                | CONTENT_CARGO
                | CONTENT_INVENTORY
                | CONTENT_STORE
                | CONTENT_TRADE
                | CONTENT_ITEM_MIX4
                | CONTENT_OPTION_STONE
                | CONTENT_SKILL
                | CONTENT_RPS
                | CONTENT_CHARACTER
        ) && self.screen(content).is_some_and(|s| s.open == open)
        {
            return Some(open);
        }

        let open = self.screen_mut(content).map(|s| {
            s.set_open(open);

            s.open
        })?;

        if open {
            if !was_open && Self::shared_toggle(content) {
                self.tooltip.hide();

                self.hover_tip.hide();
            }

            self.opened(content);

            if !was_open {
                self.place_opened(content);
            }
        } else {
            self.closed(content);
        }

        Some(open)
    }

    fn place_opened(&mut self, content: PanelId) {
        if !Self::shared_close_and_placement(content) {
            return;
        }

        let rect = self.layout.virtual_screen();

        if let Some(screen) = self.screen_mut(content) {
            screen.placed.origin = screen.placed.ui.origin(rect);
        }
    }

    fn shared_close_and_placement(content: PanelId) -> bool {
        matches!(
            content,
            CONTENT_CHARACTER
                | CONTENT_SKILL
                | CONTENT_SKILL_MASTER
                | CONTENT_INVENTORY
                | CONTENT_CARGO
                | CONTENT_ITEM_MIX
                | CONTENT_ITEM_MIX4
                | CONTENT_ITEM_SHOP
                | CONTENT_STORE
                | CONTENT_TRADE
                | CONTENT_MISSION_MIX
                | CONTENT_QUEST
                | CONTENT_ARENA
                | CONTENT_HELP
                | CONTENT_PORTAL
                | CONTENT_RPS
                | CONTENT_PGT_MENU
                | CONTENT_SYSTEM
                | CONTENT_MOVE_SERVER
                | CONTENT_COIN_INPUT
                | CONTENT_GUILD_NOTICE
                | CONTENT_GUILD_CREATE
                | CONTENT_REFINE
                | CONTENT_OPTION_STONE
        )
    }

    fn shared_toggle(content: PanelId) -> bool {
        Self::shared_close_and_placement(content)
            || matches!(content, CONTENT_GUILD | CONTENT_COLLECTION)
    }

    fn closed(&mut self, content: PanelId) {
        if content == super::merge_item::CONTENT {
            self.hide_box_contexts(0x5a, 0x5a);

            self.merge_item = Default::default();

            self.pair_inventory(0, false);

            self.set_open(CONTENT_INVENTORY, false);

            return;
        }

        if Self::shared_close_and_placement(content) {
            self.tooltip.hide();

            self.hover_tip.hide();
        }

        if content == CONTENT_CHARACTER {
            self.sheet_open_pending = false;

            self.sheet_live_pending = false;
        }

        if content == CONTENT_SKILL {
            if self.is_open(CONTENT_SKILL_MASTER) {
                self.toggle(CONTENT_SKILL_MASTER);
            }

            return;
        }

        if matches!(content, CONTENT_ITEM_MIX4 | CONTENT_OPTION_STONE) {
            if self.is_open(CONTENT_INVENTORY) {
                self.pair_inventory(0, false);

                self.toggle(CONTENT_INVENTORY);
            }

            return;
        }

        if content == CONTENT_TRADE {
            if !self.field_close_owner_present {
                return;
            }

            self.trade_closed_pending = true;

            self.trade_item_clicks.clear();

            self.close_by_toggle(CONTENT_INVENTORY);

            self.pair_inventory(0, false);

            return;
        }

        if content == CONTENT_STORE {
            if !self.field_close_owner_present {
                return;
            }

            self.store_closed_pending = true;

            self.store = None;

            let Panels {
                screens,
                items,
                icons,
                ..
            } = self;

            if let Some(screen) = screens.iter_mut().find(|s| s.content == CONTENT_STORE) {
                let resource = screen.placed.ui.resource.clone();

                for slot in 0..10 {
                    screens::clear_cell(screen, &format!("Inven{slot}"), items, icons, &resource);
                }
            }

            self.close_by_toggle(CONTENT_INVENTORY);

            return;
        }

        if content == CONTENT_INVENTORY {
            self.inventory_open_pending = false;

            self.trade_item_clicks.clear();

            self.pair_inventory(0, false);

            return;
        }

        if content == CONTENT_CARGO {
            self.cargo_open_pending = false;

            if let Some(screen) = self.screen_mut(content) {
                for slot in 0..120 {
                    screens::cargo_lock(screen, slot, false);
                }
            }

            return;
        }

        if matches!(
            content,
            CONTENT_COLLECTION
                | CONTENT_PGT_MENU
                | CONTENT_SYSTEM
                | CONTENT_ARENA
                | CONTENT_HELP
                | CONTENT_ITEM_SHOP
                | CONTENT_RPS
        ) {
            self.tooltip.hide();

            self.hover_tip.hide();
        }

        if content == super::refine::CONTENT {
            refine_reset(self, false);

            self.pair_inventory(0, false);

            return;
        }

        if content == CONTENT_MISSION_MIX {
            self.mission_clicks.clear();

            self.close_by_toggle(CONTENT_INVENTORY);

            self.pair_inventory(0, false);

            return;
        }

        if content == super::mix::CONTENT {
            self.pair_inventory(0, false);

            self.close_by_toggle(CONTENT_INVENTORY);

            return;
        }

        if content == CONTENT_COIN_INPUT {
            self.hide_box_contexts(0x5c, 0x5d);

            self.coin_focus_pending.push(None);

            if let Some(screen) = self.screen_mut(content) {
                if let Some(edit) = screen
                    .element_mut("edit", ElementType::ImeEditBox)
                    .and_then(|e| e.widget.edit_mut())
                {
                    edit.text.clear();

                    edit.preedit = None;

                    edit.caret = 0;

                    edit.anchor = 0;

                    edit.scroll = 0;
                }

                screen.placed.ui.panel.focus = None;
            }
        }

        if content != super::guild_panel::CONTENT {
            return;
        }

        for child in [
            super::guild_panel::PAGE_INFO,
            super::guild_panel::PAGE_MEMBER,
            super::guild_panel::NOTICE_EDIT,
            super::guild_panel::CREATE,
            super::guild_panel::MEMBER_POPUP,
        ] {
            if let Some(screen) = self.screen_mut(child) {
                screen.set_open(false);
            }
        }

        self.hide_box_contexts(0x3F, 0x44);
    }

    pub(super) fn hide_box_contexts(&mut self, first: u32, second: u32) {
        for slot in self.boxes.hide_contexts(first, second) {
            self.coin_focus_pending.push(None);

            self.prompts[slot] = None;

            self.box_entities[slot] = None;

            self.box_cells[slot] = None;

            self.box_combines[slot] = None;

            self.box_groupings[slot] = None;

            self.box_skills[slot] = None;
        }

        self.msgbox.follow(&self.boxes);
    }

    fn request_quest_refresh(&mut self) {
        if let Some(game) = &self.quest_game {
            self.quests = screens::load_quests(game);
        }

        self.quest_request_pending = true;
    }

    pub fn pump_quest(&mut self, entered: &Entered) {
        if std::mem::take(&mut self.quest_request_pending)
            && (self.quest_last_request == 0
                || self.now_ms.wrapping_sub(self.quest_last_request) > 149)
            && let Some(frame) = formats::net::quest::request(entered.client_id)
        {
            self.quest_last_request = self.now_ms.max(1);

            self.wire.push(frame);
        }

        if std::mem::take(&mut self.quest_response_pending) && self.is_open(CONTENT_QUEST) {
            if let Some(game) = &self.quest_game {
                self.quests = screens::load_quests(game);
            }

            let quests = self.quests.clone();

            if let Some(screen) = self.screen_mut(CONTENT_QUEST) {
                screens::quest_page(screen, screen.page, &quests, entered);
            }
        }
    }

    pub fn opened(&mut self, content: PanelId) {
        if content == CONTENT_QUEST {
            self.request_quest_refresh();
        }

        if content == CONTENT_CHARACTER {
            self.spend_repeat.reset();

            self.sheet_open_pending = true;

            self.sheet_live_pending = true;

            self.unconditional_sounds.push(51);

            return;
        }

        if matches!(content, CONTENT_ITEM_MIX4 | CONTENT_OPTION_STONE) {
            let kind = if content == CONTENT_ITEM_MIX4 {
                super::combine::Kind::RefinedEquipment
            } else {
                super::combine::Kind::OptionStone
            };

            let tip_source = self.tooltip.raised_by.filter(|target| {
                self.screens.iter().enumerate().any(|(index, screen)| {
                    screen.content == content
                        && target.panel == input_slot(index)
                        && screen
                            .placed
                            .ui
                            .panel
                            .elements
                            .iter()
                            .enumerate()
                            .any(|(i, e)| {
                                target.owner == Owner::Element(i)
                                    && e.visible
                                    && (matches!(e.name.as_str(), "Mat0" | "Mat1" | "Mat2")
                                        || content == CONTENT_OPTION_STONE
                                            && matches!(e.name.as_str(), "Result0" | "Result1"))
                                    && e.widget.button().is_some_and(|b| b.icon_contents.is_some())
                            })
                })
            });

            self.staging.reset(kind);

            let Panels {
                screens,
                items,
                icons,
                ..
            } = self;

            for screen in screens {
                if screen.content == CONTENT_INVENTORY {
                    for slot in 0..60 {
                        screens::inventory_lock(screen, slot, false);
                    }
                } else if screen.content == content {
                    for element in &mut screen.placed.ui.panel.elements {
                        if element.element_type == ElementType::IconButton
                            && (matches!(element.name.as_str(), "Mat0" | "Mat1" | "Mat2")
                                || content == CONTENT_OPTION_STONE
                                    && matches!(element.name.as_str(), "Result0" | "Result1"))
                        {
                            element.tooltip.clear();
                        }
                    }

                    if content == CONTENT_OPTION_STONE {
                        screens::option_stone_reset(screen, items, icons);
                    } else {
                        for cell in 0..3 {
                            screens::combine_unstaged(screen, cell, items, icons);
                        }
                    }
                }
            }

            if let Some(source) = tip_source {
                self.tooltip.hide();

                self.tooltip.raised_by = Some(source);

                if self.hover_tip.raised_by == Some(source) {
                    self.hover_tip.hide();
                }
            }

            self.pair_inventory(content, true);

            return;
        }

        if content == CONTENT_INVENTORY {
            self.inventory_open_pending = true;

            return;
        }

        if content == CONTENT_CARGO {
            self.cargo_open_pending = true;

            return;
        }

        if content == CONTENT_ITEM_SHOP {
            self.tooltip.hide();

            self.hover_tip.hide();

            self.shop_id = 0;

            if let Some(screen) = self.screen_mut(content) {
                screens::shop_clear_stock(screen);
            }

            return;
        }

        if content == CONTENT_HELP {
            self.tooltip.hide();

            self.hover_tip.hide();
        }

        if matches!(content, CONTENT_SYSTEM | CONTENT_ARENA | CONTENT_RPS) {
            self.tooltip.hide();

            self.hover_tip.hide();

            return;
        }

        if content == super::refine::CONTENT {
            refine_reset(self, true);

            return;
        }

        if content == CONTENT_MISSION_MIX {
            self.mission_reset = true;

            self.mission_clicks.clear();

            if let Some(screen) = self.screen_mut(content) {
                screens::mission_clear(screen);
            }

            if let Some(inventory) = self.screen_mut(CONTENT_INVENTORY) {
                for slot in 0..60 {
                    screens::inventory_lock(inventory, slot, false);
                }
            }

            self.set_open(CONTENT_INVENTORY, true);

            self.pair_inventory(content, true);

            return;
        }

        if content == super::mix::CONTENT {
            self.mix.needs.clear();

            self.staging.reset(super::combine::Kind::Mix);

            if let Some(inventory) = self.screen_mut(CONTENT_INVENTORY) {
                for slot in 0..60 {
                    screens::inventory_lock(inventory, slot, false);
                }
            }

            self.set_open(CONTENT_INVENTORY, true);

            self.pair_inventory(content, true);

            if let Some(screen) = self.screen_mut(content) {
                screens::mix_clear_materials(screen);
            }

            return;
        }

        if content == CONTENT_SKILL_MASTER {
            skill_master::on_open(self);

            return;
        }

        if content == CONTENT_COIN_INPUT {
            if let Some(index) = self.screens.iter().position(|s| s.content == content) {
                let screen = &mut self.screens[index];

                let editor =
                    screen.placed.ui.panel.elements.iter().position(|e| {
                        e.name == "edit" && e.element_type == ElementType::ImeEditBox
                    });

                screen.placed.ui.panel.focus = editor;

                if screen.open
                    && screen.placed.ui.panel.allow_focus
                    && let Some(element) = editor
                    && ui::input::tab_stop(&screen.placed.ui.panel.elements[element])
                {
                    self.coin_focus_pending.push(Some(Target {
                        panel: input_slot(index),
                        owner: Owner::Element(element),
                    }));
                }
            }

            return;
        }

        if content == super::collection::CONTENT {
            self.collection_filter = super::collection::Filter::All;

            self.collection_dirty = true;

            self.collection_cell = 0;

            self.collection_icon = None;

            self.collection_signed_up = false;

            super::collection::check_filter(self);

            return;
        }

        if content == super::pgt::CONTENT {
            super::pgt::opened(self);

            return;
        }

        if content == super::guild_panel::CONTENT {
            if !self.is_open(content) {
                return;
            }

            self.hide_box_contexts(0x3F, 0x44);

            let actions = super::guild_panel::tab_click(&self.guild, "Info");

            guild_carry_out(self, &actions);

            return;
        }

        if content != CONTENT_HELP {
            return;
        }

        let page = self.screen(CONTENT_HELP).map_or(0, |s| s.page);

        let lines = match page {
            super::minipopup::MEMO_HELP_TAB => self.memo.clone(),
            _ => self.help.get(page as usize).cloned().unwrap_or_default(),
        };

        if let Some(screen) = self.screen_mut(CONTENT_HELP) {
            screens::help_page(screen, page, &lines);
        }
    }

    pub fn append_memo(&mut self, text: String) {
        self.append_colored_memo(text, 0xFFFF_FFFF);
    }

    pub fn append_colored_memo(&mut self, text: String, color: u32) {
        screens::help_memo(&mut self.memo, text);

        if let Some(row) = self.memo.last_mut() {
            row.color = color;
        }

        let page = self.screen(CONTENT_HELP).map_or(0, |screen| screen.page);

        if page == super::minipopup::MEMO_HELP_TAB {
            let lines = self.memo.clone();

            if let Some(screen) = self.screen_mut(CONTENT_HELP) {
                screens::help_page(screen, page, &lines);
            }
        }

        if let Some(screen) = self.screen_mut(CONTENT_HELP)
            && let Some(element) = screen.element_mut("List", ElementType::ListView)
        {
            let rect = element.rect();

            if let Some(list) = element.widget.list_mut() {
                list.scroll = 0xFF_FFFF;

                list.scroll = list.clamped_scroll(list.content_rect(rect));
            }
        }
    }

    pub fn guild_info_arrived(&mut self, body: &str) -> bool {
        let Some(info) = formats::net::guild_web::info(body) else {
            return false;
        };

        self.guild.session.guild = info.uid.max(0) as u32;

        self.guild.info = Some(info);

        self.guild_refresh();

        true
    }

    pub fn guild_members_arrived(&mut self, body: &str) -> bool {
        let Some(rows) = formats::net::guild_web::members(body) else {
            return false;
        };

        self.guild.members = rows;

        self.guild_refresh();

        true
    }

    pub fn guild_session(&mut self, session: super::guild_panel::Session) {
        self.guild_live_session = Some(session);

        self.guild.session = session;

        self.guild_refresh();
    }

    pub fn guild_row_clicked(&mut self, row: usize) -> bool {
        if !self.guild.session.in_a_guild() {
            return false;
        }

        let Some(member) = self
            .guild
            .members
            .iter()
            .filter(|m| m.char_uid != 0)
            .nth(row)
            .cloned()
        else {
            return false;
        };

        self.guild.picked = Some(super::guild_panel::Picked {
            char_uid: member.char_uid.max(0) as u32,
            name: member.name.clone(),
            rank: member.rank,
        });

        let opens = super::guild_panel::popup_opens(&self.guild);

        if opens {
            self.set_open(super::guild_panel::MEMBER_POPUP, true);

            let guild = self.guild.clone();

            let strings = self.strings.clone();

            let bounds = self.layout.virtual_screen();

            if let Some(screen) = self.screen_mut(super::guild_panel::MEMBER_POPUP) {
                let completed = screens::guild_popup_refresh(screen, &guild, &strings);

                screen.placed.origin = screen.placed.ui.origin(bounds);

                if completed {
                    self.guild_popup_position_pending = true;

                    self.release_popup_focus();

                    self.hover_tip.hide();
                }
            }
        }

        opens
    }

    pub fn guild_refresh(&mut self) {
        let guild = self.guild.clone();

        let strings = self.strings.clone();

        if let Some(screen) = self.screen_mut(super::guild_panel::PAGE_INFO) {
            screens::guild_info_refresh(screen, &guild);
        }

        if let Some(screen) = self.screen_mut(super::guild_panel::PAGE_MEMBER) {
            screens::guild_member_refresh(screen, &guild, &strings);
        }
    }

    fn guild_refresh_selected(&mut self) {
        if self.screen(super::guild_panel::PAGE_INFO).is_none() {
            return;
        }

        self.guild_refresh_session();

        let guild = self.guild.clone();

        match guild.page {
            super::guild_panel::Page::Info => {
                if let Some(screen) = self.screen_mut(super::guild_panel::PAGE_INFO) {
                    screens::guild_info_refresh(screen, &guild);
                }
            }
            super::guild_panel::Page::Member => {
                let strings = self.strings.clone();

                if let Some(screen) = self.screen_mut(super::guild_panel::PAGE_MEMBER) {
                    screens::guild_member_refresh(screen, &guild, &strings);
                }
            }
        }
    }

    pub(crate) fn guild_membership_refresh(&mut self) {
        use super::guild_panel::{Action, Fetch};

        guild_carry_out(
            self,
            &[
                Action::Refetch(Fetch::Info),
                Action::Refetch(Fetch::Members),
            ],
        );

        self.guild_refresh_selected();
    }

    pub fn open_ids(&self) -> Vec<PanelId> {
        let mut open: Vec<&Screen> = self.screens.iter().filter(|s| s.open).collect();

        open.sort_by_key(|s| s.z_order());

        open.iter().map(|s| s.content).collect()
    }

    fn place_guild_pages(&mut self) {
        let Some(base) = self.screen(CONTENT_GUILD) else {
            return;
        };

        let Some([left, top, right, bottom]) = self.guild_page_rect else {
            return;
        };

        let origin = (base.placed.origin.0 + left, base.placed.origin.1 + top);

        let size = (right - left, bottom - top);

        for id in [CONTENT_GUILD_INFO, CONTENT_GUILD_MEMBER] {
            if let Some(page) = self.screen_mut(id) {
                page.placed.origin = origin;

                let panel = &mut page.placed.ui.panel;

                panel.right = panel.left + size.0;

                panel.bottom = panel.top + size.1;
            }
        }
    }

    fn select_guild_tab(&mut self) {
        let info = self.guild.page == super::guild_panel::Page::Info;

        if let Some(base) = self.screen_mut(CONTENT_GUILD) {
            for (name, checked) in [("Info", info), ("Member", !info)] {
                if let Some(state) = base
                    .element_mut(name, ElementType::Button)
                    .and_then(|e| e.widget.button_mut())
                {
                    state.checked = checked;
                }
            }
        }
    }

    pub fn reposition(&mut self, backbuffer: (u32, u32)) {
        self.layout = Layout::new(backbuffer.0, backbuffer.1);

        let screen = self.layout.virtual_screen();

        let scale = self.layout.ui_scale();

        for panel in &mut self.screens {
            panel.placed.ui.scale = scale;

            panel.placed.origin = panel.placed.ui.reposition(screen);
        }

        if let Some(input) = chat::index_of(self, chat::CHAT_INPUT) {
            chat::resized(self, input);
        }

        self.place_guild_pages();

        self.for_each_ui(&mut |ui| ui.scale = scale);

        self.msgbox.reposition(&self.layout);

        self.hover_tip.set_scale(scale);
    }
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    strings: &Strings,
    chains: &Chains,
    captions: bool,
) -> Result<Panels> {
    let layout = crate::scene::login_static::Layout::new(backbuffer.0, backbuffer.1);

    let screen_rect = layout.virtual_screen();

    let zoom = layout.ui_scale();

    let mut screens = Vec::with_capacity(SCREENS.len() + CHAT_SCREENS.len());

    for registered in SCREENS.into_iter().chain(CHAT_SCREENS) {
        if registered.content == super::merge_item::CONTENT
            && game.resolve(registered.script).is_none()
        {
            continue;
        }

        let mut ui = PanelUi::build(game, registered.script, strings, chains, zoom)
            .with_context(|| String::from_utf8_lossy(registered.script).into_owned())?;

        ui.panel.visible = registered.shown;

        if matches!(
            registered.content,
            CONTENT_GUILD_NOTICE | CONTENT_GUILD_CREATE
        ) {
            ui.panel.z_order = ui.panel.z_order.saturating_sub(1);
        }

        let origin = ui.origin(screen_rect);

        let placed = PlacedPanel { ui, origin };

        let guild_popup_base = (registered.content == super::guild_panel::MEMBER_POPUP)
            .then(|| placed.ui.panel.clone());

        let mut screen = Screen {
            content: registered.content,
            class: registered.class,
            script: registered.script,
            placed,
            open: registered.shown,
            page: 0,
            page_of: registered.page_of,
            cape_flag: None,
            combine_widgets: Default::default(),
            mix_recipe_owners: [None; 24],
            inventory_widgets: screens::InventoryWidgets::default(),
            cargo_widgets: screens::CargoWidgets::default(),
            refine_widgets: Default::default(),
            sheet_widgets: Default::default(),
            guild_member_list: None,
            guild_popup_base,
        };

        screens::guild_bind(&mut screen);

        screens.push(screen);
    }

    let guild_page_rect = screens
        .iter_mut()
        .find(|s| s.content == CONTENT_GUILD)
        .map(|base| {
            let resource = base.placed.ui.resource.clone();

            base.placed
                .ui
                .panel
                .element_or_create(
                    super::guild_panel::TAB_RECT_FROM,
                    ElementType::Static,
                    &resource,
                )
                .rect()
        });

    let mut panels = Panels {
        now_ms: 0,
        zone_time: Default::default(),
        sealed_records: Default::default(),
        coin_focus_pending: Vec::new(),
        guild_popup_position_pending: false,
        hover_leaves_pending: Vec::new(),
        spend_repeat: spend::Repeat::default(),
        layout: Layout::new(backbuffer.0, backbuffer.1),
        screens,
        guimat_textures: BTreeMap::new(),
        hover_textures: BTreeMap::new(),
        hover_missing: Vec::new(),
        missing: Vec::new(),
        notes: Vec::new(),
        captions,
        strings: strings.clone(),
        help: screens::load_help(game),
        notice_requested: false,
        community_requested: false,
        settings_requested: false,
        memo: Vec::new(),
        quests: screens::load_quests(game),
        quest_game: Some(game.clone()),
        quest_request_pending: false,
        quest_last_request: 0,
        quest_response_pending: false,
        items: localized_item_table(game)?,
        missions: mission_tables(game),
        mixes: game
            .read(MIX_LIST)
            .ok()
            .and_then(|raw| formats::data::mix_list::parse(&raw).ok()),
        mix_help: {
            let (bytes, cp) =
                crate::ui::read_language(game, super::mix::HELP_SCRIPT).unwrap_or_default();

            let (help, names) = formats::data::item_help::parse_mix(&bytes, cp);

            super::mix::HelpTables { help, names }
        },
        refine_sets: game
            .read(b"ItemSet.bin")
            .ok()
            .and_then(|raw| {
                formats::data::item_set::parse_strict(&raw)
                    .map_err(|error| {
                        tracing::warn!(
                            % error, "Refine ItemSet recipe table was rejected"
                        )
                    })
                    .ok()
            })
            .unwrap_or_default(),
        icons: item_icon::parse(&game.read(ITEM_ICON).unwrap_or_default()),
        skills: game
            .read(SKILL_DATA)
            .ok()
            .and_then(|raw| formats::data::skill_data::parse(&raw).ok()),
        icon_sheets: BTreeMap::new(),
        missing_sheets: Vec::new(),
        tooltip: tooltip::build(game, strings, chains, zoom)
            .context("building the item tooltip")?,
        hover_tip: hover_tip::build(game, chains, zoom)
            .context("building the HUD hover balloon")?,
        hover_source: None,
        icon_hover_stamp: None,
        icon_hover_rect: None,
        carry: carry::build(game, strings, chains, zoom)?,
        live: item_use::Live::default(),
        cooldowns: item_move::Cooldowns::default(),
        master: skill_master::Master::default(),
        skill_master_desc: crate::ui::read_language(game, skill_master::DESC_FILE)
            .map(|(bytes, cp)| ui::text::decode_colored_text_records(&bytes, cp))
            .unwrap_or_default(),
        potal_pos: crate::ui::read_language(game, "PotalPos.txt")
            .map(|(bytes, cp)| formats::text::potal_pos::parse(&bytes, cp))
            .unwrap_or_default(),
        outbox: Vec::new(),
        staged: Vec::new(),
        firecracker_slot: None,
        ticket_accepted: None,
        seal_open: None,
        seal_name_slot: None,
        wire: Vec::new(),
        gate_accepted: false,
        summon_accepted: false,
        guild_fame_accepted_ms: None,
        chief_accepted: None,
        field_body_guild: None,
        item_skill_accepted: None,
        guild_application_accepted: false,
        death_return_accepted: false,
        resurrect_accepted: false,
        trade_accepted: None,
        boxes: msgbox::Boxes::new(),
        msgbox: msgbox::BoxPanel::build(game, strings, chains, zoom, layout.virtual_extent())
            .context("building the field message box")?,
        prompts: [None; msgbox::SLOTS],
        used: Vec::new(),
        skill_casts: Vec::new(),
        shop_clicks: Vec::new(),
        mission_clicks: Vec::new(),
        mission_reset: false,
        mix_clicks: Vec::new(),
        mix: super::mix::Browser::default(),
        staging: super::combine::Staging::default(),
        refine: super::refine::Refine::default(),
        refine_command_locked: false,
        guild: super::guild_panel::Guild::default(),
        guild_page_rect,
        guild_requests: Vec::new(),
        guild_identity: super::guild_request::Identity::default(),
        guild_live_session: None,
        guild_request_rng: super::rng::Lcg::new(1),
        guild_host: super::guild_panel::web_host(strings),
        guild_ring: None,
        pgt_target: None,
        pgt_trade_requested: false,
        pgt_clear_target: false,
        pgt_toasts: Vec::new(),
        pgt_messages: Vec::new(),
        collection_filter: super::collection::Filter::default(),
        arena_cost: 0,
        collection_dirty: false,
        cargo_open_pending: false,
        item_use_clear_hover: false,
        inventory_open_pending: false,
        sheet_open_pending: false,
        sheet_live_pending: false,
        unconditional_sounds: Vec::new(),
        inventory_partner: 0,
        inventory_block_equipment: false,
        store_closed_pending: false,
        trade_closed_pending: false,
        field_close_owner_present: true,
        trade_item_clicks: Vec::new(),
        collection_signed_up: false,
        collection_cell: 0,
        collection_icon: None,
        shop_id: 0,
        shop_prices: item_tip::Shop::default(),
        store: None,
        store_owner_opened: None,
        store_price: None,
        analytics: Default::default(),
        analytics_next_level: 0,
        client_config: Default::default(),
        guild_proxy: Default::default(),
        coin_action: 0,
        split_stack: None,
        portal: portal::Portal::default(),
        box_entities: [None; msgbox::SLOTS],
        box_cells: [None; msgbox::SLOTS],
        tooltip_calendar: crate::clock::Calendar::GOLDEN,
        tooltip_portuguese: game.language() == formats::config::Language::Portuguese,
        merge_item: Default::default(),
        box_groupings: [None; msgbox::SLOTS],
        box_combines: [None; msgbox::SLOTS],
        box_skills: [None; msgbox::SLOTS],
        chat: chat::Chat::new(),
        notice: notice::Notice::default(),
        notices: super::minipopup::Notices::default(),
        summoner: String::new(),
        reserved: crate::scene::create::Reserved::load(game),
    };

    panels.place_guild_pages();

    if let Some(missions) = panels.missions.as_mut() {
        missions.fill_missing_help(&panels.items, &panels.icons, &panels.strings);
    }

    panels.select_guild_tab();

    load_textures(game, &mut panels)?;

    chat::bind(&mut panels, strings);

    super::minipopup::bind(&mut panels);

    panels.notes = panels
        .screens
        .iter()
        .flat_map(|s| s.placed.ui.notes.iter().cloned())
        .collect();

    Ok(panels)
}

fn load_textures(game: &GameDir, panels: &mut Panels) -> Result<()> {
    let interaction = Interaction::default();

    let mut referenced: Vec<(usize, i32)> = Vec::new();

    for (index, screen) in panels.screens.iter_mut().enumerate() {
        let hidden = screen.placed.ui.panel.visible;

        screen.placed.ui.panel.visible = true;

        for slot in screen.placed.ui.referenced_textures(&interaction) {
            referenced.push((index, slot));
        }

        screen.placed.ui.panel.visible = hidden;
    }

    for (index, slot) in referenced {
        if panels.guimat_textures.contains_key(&slot) || panels.missing.contains(&slot) {
            continue;
        }

        let ui = &panels.screens[index].placed.ui;

        match crate::scene::login_panels::load_guimat_texture(game, ui, slot)? {
            Some(loaded) => {
                panels.guimat_textures.insert(slot, loaded);
            }
            None => panels.missing.push(slot),
        }
    }

    for loaded in panels.msgbox.textures(game, &mut panels.missing)? {
        panels.guimat_textures.entry(loaded.slot).or_insert(loaded);
    }

    let strings = panels.strings.clone();

    for slot in panels.tooltip.referenced_textures(&strings) {
        if panels.guimat_textures.contains_key(&slot) || panels.missing.contains(&slot) {
            continue;
        }

        match crate::scene::login_panels::load_guimat_texture(game, panels.tooltip.ui(), slot)? {
            Some(loaded) => {
                panels.guimat_textures.insert(slot, loaded);
            }
            None => panels.missing.push(slot),
        }
    }

    for slot in panels.hover_tip.referenced_textures() {
        if panels.hover_textures.contains_key(&slot) || panels.hover_missing.contains(&slot) {
            continue;
        }

        match crate::scene::login_panels::load_guimat_texture(game, panels.hover_tip.ui(), slot)? {
            Some(loaded) => {
                panels.hover_textures.insert(slot, loaded);
            }
            None => panels.hover_missing.push(slot),
        }
    }

    for slot in panels.carry.referenced_textures() {
        if panels.guimat_textures.contains_key(&slot) || panels.missing.contains(&slot) {
            continue;
        }

        match crate::scene::login_panels::load_guimat_texture(game, panels.carry.ui(), slot)? {
            Some(loaded) => {
                panels.guimat_textures.insert(slot, loaded);
            }
            None => panels.missing.push(slot),
        }
    }

    Ok(())
}

fn mission_tables(game: &GameDir) -> Option<mission::Tables> {
    let items = formats::data::mission::parse(&game.read(MISSION_ITEMS).ok()?).ok()?;

    let (help, names) = match crate::ui::read_language(game, MISSION_HELP) {
        Ok((raw, cp)) => formats::data::item_help::parse_mission(&raw, cp),
        Err(_) => (
            vec![Default::default(); formats::data::item_help::MISSION_RECORDS],
            vec![String::new(); formats::data::item_help::MISSION_RECORDS],
        ),
    };

    let mut overrides = Default::default();

    if let Ok(bytes) = game.read(b"MItem.txt") {
        formats::data::mitem::overlay(&mut overrides, &bytes);
    }

    if let Ok((bytes, codepage)) = crate::ui::read_language(game, "MItem.txt") {
        formats::data::mitem::overlay_with_codepage(&mut overrides, &bytes, codepage);
    }

    Some(mission::Tables {
        items,
        help,
        names,
        overrides,
    })
}

fn localized_item_table(game: &GameDir) -> Result<item_list::ItemList> {
    let mut items = super::item_table::load(game)?;

    if let Ok((raw, codepage)) = crate::ui::read_language(game, ITEM_NAME) {
        items.apply_names_with_codepage(&formats::data::item_name::parse(&raw), codepage);
    }

    Ok(items)
}

pub const ITEM_NAME: &str = "itemname.bin";

pub const ITEM_ICON: &[u8] = b"itemicon.bin";

pub const MISSION_ITEMS: &[u8] = b"Missionitems.bin";

pub const MIX_LIST: &[u8] = b"Mixlist.bin";

pub const MISSION_HELP: &str = "missionhelp.dat";

pub const SKILL_DATA: &[u8] = tooltip::SKILL_DATA;

fn sheet_path(sheet: i32) -> String {
    match AmulSheet::of(sheet) {
        Some(AmulSheet::Plain) => String::from(formats::net::affect::ICON_SHEET),
        Some(AmulSheet::On) => String::from(formats::net::affect::ICON_SHEET_ON),
        None => format!("UI/ItemIcon{sheet:02}.wyt"),
    }
}

fn icon_key(sheet: i32) -> TextureKey {
    AmulSheet::of(sheet).map_or(TextureKey::ItemIcon(sheet), TextureKey::Amul)
}

pub fn load_icon_sheets(game: &GameDir, panels: &mut Panels) -> Result<()> {
    fn collect(elements: &[ui::Element], wanted: &mut Vec<i32>) {
        for element in elements {
            if let Some(contents) = element.widget.button().and_then(|b| b.icon_contents)
                && !wanted.contains(&contents.sheet)
            {
                wanted.push(contents.sheet);
            }

            if let Some(list) = element.widget.list() {
                for row in &list.items {
                    collect(&row.children, wanted);
                }
            }
        }
    }

    let mut wanted: Vec<i32> = Vec::new();

    for screen in &panels.screens {
        collect(&screen.placed.ui.panel.elements, &mut wanted);
    }

    if wanted.iter().any(|&sheet| AmulSheet::of(sheet).is_some()) {
        for sheet in [ui::icon::SKILL_SHEET, ui::icon::SKILL_SHEET_ON] {
            if !wanted.contains(&sheet) {
                wanted.push(sheet);
            }
        }
    }

    for sheet in wanted {
        if panels.icon_sheets.contains_key(&sheet) || panels.missing_sheets.contains(&sheet) {
            continue;
        }

        let path = sheet_path(sheet);

        let Ok(bytes) = game.read(path.as_bytes()) else {
            panels.missing_sheets.push(sheet);

            continue;
        };

        let texture =
            formats::texture::decode(&bytes).with_context(|| format!("decoding {path}"))?;

        panels.icon_sheets.insert(
            sheet,
            GuiTexture {
                slot: sheet,
                path,
                color_key: 0,
                keyed_texels: 0,
                texture,
            },
        );
    }

    Ok(())
}

pub fn upload(panels: &Panels, gpu: &Gpu, renderer: &mut SceneRenderer) {
    for (slot, loaded) in &panels.guimat_textures {
        renderer.upload_once(
            gpu,
            TextureKey::GuiMat(*slot),
            &loaded.texture,
            AlphaPolicy::Stored,
            &format!("{} (guimat slot {slot})", loaded.path),
        );
    }

    for (slot, loaded) in &panels.hover_textures {
        renderer.upload_once(
            gpu,
            TextureKey::HoverGuiMat(*slot),
            &loaded.texture,
            AlphaPolicy::Stored,
            &format!("{} (balloon guimat slot {slot})", loaded.path),
        );
    }

    for (sheet, loaded) in &panels.icon_sheets {
        renderer.upload_once(
            gpu,
            icon_key(*sheet),
            &loaded.texture,
            AlphaPolicy::Stored,
            &loaded.path,
        );
    }
}

pub fn sync(panels: &mut Panels, gpu: &Gpu, renderer: &mut SceneRenderer) {
    let captions = panels.captions;

    panels.tooltip.sync(gpu, renderer, captions);

    panels.hover_tip.sync(gpu, renderer, captions);

    panels.carry.sync(gpu, renderer, captions);

    for screen in &mut panels.screens {
        if screen.placed.ui.panel.visible {
            renderer.sync_named_icons(gpu, screen.content, &screen.placed.ui);

            if captions {
                renderer.sync_captions(gpu, screen.content, &screen.placed.ui);
            }
        }
    }

    for (slot, placed) in panels.msgbox.visible() {
        let id = msgbox::caption_id(BOX_CAPTION_ID, slot);

        renderer.sync_named_icons(gpu, id, &placed.ui);

        if captions {
            renderer.sync_captions(gpu, id, &placed.ui);
        }
    }
}

pub(crate) fn append_panel(
    out: &mut Vec<PlannedQuad>,
    panel: &mut PlacedPanel,
    interaction: &Interaction,
    id: PanelId,
    captions: bool,
    external: Option<super::cape_flag::ExternalImage>,
) {
    if !panel.ui.panel.visible {
        return;
    }

    for draw in panel.ui.frame(interaction, panel.origin) {
        if !captions && matches!(draw.source, Source::Caption(..)) {
            continue;
        }

        let mut quad = draw.quad;

        let texture = match (
            external.filter(|image| image.owner == draw.owner),
            draw.source,
        ) {
            (Some(image), Source::Texture(_)) => {
                quad.src_x = 0.0;

                quad.src_y = 0.0;

                quad.src_w = image.size[0] as f32;

                quad.src_h = image.size[1] as f32;

                TextureKey::ExternalUi(image.texture_slot)
            }
            (_, Source::Texture(slot)) => TextureKey::GuiMat(slot),
            (_, Source::ItemIcon(sheet)) => TextureKey::ItemIcon(sheet),
            (_, Source::Amul(sheet)) => TextureKey::Amul(sheet),
            (_, Source::Caption(owner, run)) => TextureKey::Caption(id, owner, run),
            (_, Source::NamedIcon(owner)) => TextureKey::NamedIcon(id, owner),
            (_, Source::Solid) => TextureKey::White,
        };

        out.push(PlannedQuad {
            mode: draw.mode(),
            texture,
            quad,
            uv: draw.uv,
        });
    }
}

fn append_overlays(
    panels: &mut Panels,
    manager: &Manager,
    out: &mut Vec<PlannedQuad>,
    hud_visible: Option<[bool; 3]>,
) {
    refresh_tooltip_owner(panels, hud_visible);

    let captions = panels.captions;

    let carried = manager.carried;

    out.extend(panels.tooltip.frame_draws(captions));

    for (slot, placed) in panels.msgbox.visible_mut() {
        append_panel(
            out,
            placed,
            &Interaction::default(),
            msgbox::caption_id(BOX_CAPTION_ID, slot),
            captions,
            None,
        );
    }

    out.extend(panels.hover_tip.frame_draws(captions));

    out.extend(
        panels
            .carry
            .frame_draws(carried.as_ref(), manager.cursor, captions),
    );
}

fn refresh_tooltip_owner(panels: &mut Panels, hud_visible: Option<[bool; 3]>) {
    let Some(shown) = &panels.tooltip.shown else {
        return;
    };

    if panels.hover_tip.visible() && shown.panel.visible {
        let [left, top, right, bottom] = panels.hover_tip.rect();

        let (x, y) = shown.origin;

        if left < x + shown.panel.width()
            && x < right
            && top < y + shown.panel.height()
            && y < bottom
        {
            panels.hover_tip.hide();
        }
    }

    if let Some(content) = shown.source_content.filter(|&id| id != 0) {
        let live = if (0x10..=0x12).contains(&content) {
            hud_visible.is_none_or(|visible| visible[(content - 0x10) as usize])
        } else {
            panels
                .screens
                .iter()
                .any(|s| s.content as i32 == content && s.placed.ui.panel.visible)
        };

        if !live {
            panels.tooltip.hide();
        }
    }
}

fn retire_tooltip_element(panels: &mut Panels, hud: Option<&Hud>) {
    let Some((target, instance)) = panels.hover_source else {
        return;
    };

    let panel = if target.panel < HUD_PANELS {
        let Some(hud) = hud else {
            return;
        };

        match target.panel {
            0 => &hud.bar.ui.panel,
            1 => &hud.plate.ui.panel,
            _ => &hud.target.ui.panel,
        }
    } else {
        let Some(screen) = panels.screens.get(target.panel - HUD_PANELS) else {
            retire_tooltip_source(panels, target);

            return;
        };

        &screen.placed.ui.panel
    };

    let live = panel.elements.iter().enumerate().any(|(element, _)| {
        ui::input::retained_element(
            panel,
            Owner::Descendant {
                element,
                node: instance,
                item: None,
            },
        )
        .is_some()
    });

    if !live {
        retire_tooltip_source(panels, target);
    }
}

fn retire_tooltip_source(panels: &mut Panels, target: Target) {
    if panels.tooltip.raised_by == Some(target) {
        panels.tooltip.hide();
    }

    if panels.hover_tip.raised_by == Some(target) {
        panels.hover_tip.hide();
    }

    panels.hover_source = None;
}

pub fn frame_draws(panels: &mut Panels, manager: &Manager) -> Vec<PlannedQuad> {
    retire_tooltip_element(panels, None);

    for screen in &mut panels.screens {
        screen
            .placed
            .ui
            .panel
            .set_combo_screen(panels.layout.virtual_screen(), screen.placed.origin);
    }

    let captions = panels.captions;

    let mut out = Vec::new();

    for index in draw_order(&panels.screens) {
        let is_minipopup = {
            let screen = &mut panels.screens[index];

            let interaction = manager.interaction_for(
                input_slot(index),
                &screen.placed.ui.panel,
                screen.placed.origin,
            );

            let external = match screen.cape_flag {
                Some(super::cape_flag::WidgetState::External(image)) => Some(image),
                _ => None,
            };

            append_panel(
                &mut out,
                &mut screen.placed,
                &interaction,
                screen.content,
                captions,
                external,
            );

            screen.script == super::minipopup::SCRIPT_POPUP
        };

        if is_minipopup {
            out.extend(super::minipopup::frame_draws(panels));
        }
    }

    append_overlays(panels, manager, &mut out, None);

    out
}

pub fn field_frame_draws(hud: &mut Hud, panels: &mut Panels) -> Vec<PlannedQuad> {
    retire_tooltip_element(panels, Some(hud));

    sync_coin_focus(hud, panels);

    for panel in [&mut hud.bar, &mut hud.plate, &mut hud.target] {
        panel
            .ui
            .panel
            .set_combo_screen(panels.layout.virtual_screen(), panel.origin);
    }

    for screen in &mut panels.screens {
        screen
            .placed
            .ui
            .panel
            .set_combo_screen(panels.layout.virtual_screen(), screen.placed.origin);
    }

    let order = field_draw_order(hud, panels);

    let interactions: Vec<Interaction> = (0..HUD_PANELS + panels.screens.len())
        .map(|slot| {
            panel_at(hud, panels, slot).map_or_else(
                || hud.manager.interaction(slot),
                |(panel, origin, _)| hud.manager.interaction_for(slot, panel, origin),
            )
        })
        .collect();

    let mut out = Vec::new();

    for slot in order {
        let interaction = &interactions[slot];

        if slot < HUD_PANELS {
            let (id, panel) = match slot {
                0 => (hud::CONTENT_HUD, &mut hud.bar),
                1 => (hud::CONTENT_MY_INFO, &mut hud.plate),
                2 => (hud::CONTENT_YOU_INFO, &mut hud.target),
                _ => continue,
            };

            append_panel(&mut out, panel, interaction, id, hud.captions, None);

            continue;
        }

        let index = slot - HUD_PANELS;

        let is_minipopup = {
            let screen = &mut panels.screens[index];

            let content = screen.content;

            let external = match screen.cape_flag {
                Some(super::cape_flag::WidgetState::External(image)) => Some(image),
                _ => None,
            };

            append_panel(
                &mut out,
                &mut screen.placed,
                interaction,
                content,
                panels.captions,
                external,
            );

            screen.script == super::minipopup::SCRIPT_POPUP
        };

        if is_minipopup {
            out.extend(super::minipopup::frame_draws(panels));
        }
    }

    append_overlays(
        panels,
        &hud.manager,
        &mut out,
        Some([
            hud.bar.ui.panel.visible,
            hud.plate.ui.panel.visible,
            hud.target.ui.panel.visible,
        ]),
    );

    out
}

pub fn draw_order(screens: &[Screen]) -> Vec<usize> {
    let mut order = input_order(screens);

    order.reverse();

    order
}

pub fn input_order(screens: &[Screen]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..screens.len()).collect();

    order.sort_by_key(|&i| (screens[i].z_order(), !screens[i].page_of.is_some()));

    order
}

pub fn input_slot(index: usize) -> usize {
    HUD_PANELS + index
}

pub const HUD_PANELS: usize = 3;

#[derive(Clone, Copy, Debug)]
pub struct FieldInput {
    pub now_ms: u32,
    pub player_present: bool,
    pub player_dead: bool,
}

pub fn route(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    quick_bar: &mut super::quick_bar::QuickBar,
    event: ui::input::Mouse,
    x: i32,
    y: i32,
    now_ms: u32,
) -> (Vec<Event>, bool) {
    route_in_field(
        hud,
        panels,
        entered,
        quick_bar,
        event,
        (x, y),
        FieldInput {
            now_ms,
            player_present: true,
            player_dead: false,
        },
    )
}

pub fn route_in_field(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    quick_bar: &mut super::quick_bar::QuickBar,
    event: ui::input::Mouse,
    at: (i32, i32),
    input: FieldInput,
) -> (Vec<Event>, bool) {
    let (x, y) = at;

    let now_ms = input.now_ms;

    sync_coin_focus(hud, panels);

    if event == ui::input::Mouse::LeftDown
        && let Some((slot, yes)) = panels.box_hit(x, y)
    {
        panels.answer_box_in_field(entered, slot, yes, input);

        return (Vec::new(), true);
    }

    if hud.manager.carried.is_some() && event == ui::input::Mouse::LeftDown {
        hud.manager.cursor = (x, y);

        if drop_carry(hud, panels, entered, quick_bar, now_ms) {
            return (Vec::new(), true);
        }
    }

    let consumed = {
        let hud_hit = hud
            .panel_refs()
            .iter()
            .any(|p| p.panel.visible && p.contains(x, y));

        let screen_hit = panels.screens.iter().any(|s| {
            let reference = PanelRef {
                panel: &s.placed.ui.panel,
                origin: s.placed.origin,
            };

            reference.panel.visible && reference.contains(x, y)
        });

        hud_hit || screen_hit
    };

    let order = walk_order(hud, panels);

    let mut manager = std::mem::take(&mut hud.manager);

    manager.screen = panels.layout.virtual_screen();

    let mut refs: Vec<PanelMut<'_>> = Vec::with_capacity(HUD_PANELS + panels.screens.len());

    for panel in [&mut hud.bar, &mut hud.plate, &mut hud.target] {
        panel.ui.refresh_input_layout();

        refs.push(PanelMut {
            panel: &mut panel.ui.panel,
            origin: &mut panel.origin,
        });
    }

    for screen in &mut panels.screens {
        screen.placed.ui.refresh_input_layout();

        refs.push(PanelMut {
            panel: &mut screen.placed.ui.panel,
            origin: &mut screen.placed.origin,
        });
    }

    let pressed_before = manager.pressed;

    let capture_before = manager.capture;

    let events = manager.mouse_in_order(&mut refs, &order, event, x, y);

    let keys = manager.keys;

    if matches!(
        event,
        ui::input::Mouse::RightDown | ui::input::Mouse::RightUp
    ) {
        tracing::debug!(
            target : "nwyd::equip_input", ? event, x, y, ctrl = keys.ctrl, shift = keys
            .shift, alt = keys.alt, ? pressed_before, ? capture_before, pressed_after = ?
            manager.pressed, capture_after = ? manager.capture, hovered = ? manager
            .hovered, carrying = manager.carried.is_some(), ? events,
            "right-button panel routing"
        );
    }

    hud.manager = manager;

    panels.place_guild_pages();

    if let Some(target) = hud.manager.capture {
        let placed = match target.panel {
            0 => Some(&mut hud.bar),
            1 => Some(&mut hud.plate),
            2 => Some(&mut hud.target),
            n => panels
                .screens
                .get_mut(n - HUD_PANELS)
                .map(|s| &mut s.placed),
        };

        if let Some(placed) = placed {
            placed
                .ui
                .edit_pointer(target.owner, event, x - placed.origin.0);
        }
    }

    if matches!(
        event,
        ui::input::Mouse::LeftDown | ui::input::Mouse::LeftDouble | ui::input::Mouse::Move
    ) && let Some(target) = hud.manager.capture
        && let Some(index) = target.panel.checked_sub(HUD_PANELS)
        && let Some(screen) = panels.screens.get_mut(index)
        && let Owner::Element(element) = target.owner
        && screen
            .placed
            .ui
            .panel
            .elements
            .get(element)
            .is_some_and(ui::notepad::pointer_active)
    {
        let origin = screen.placed.origin;

        if screen
            .placed
            .ui
            .notepad_pointer(element, event, x - origin.0, y - origin.1, keys.shift)
        {
            if event != ui::input::Mouse::Move {
                set_focus(hud, panels, Some(target));
            }
        } else if hud.manager.capture == Some(target) {
            hud.manager.capture = None;

            hud.manager.pressed = None;
        }
    }

    use_cell(panels, entered, &events, keys, now_ms, input.player_present);

    (events, consumed)
}

pub fn set_focus(hud: &mut Hud, panels: &mut Panels, target: Option<Target>) {
    sync_coin_focus(hud, panels);

    apply_focus(hud, panels, target);
}

pub fn sync_coin_focus(hud: &mut Hud, panels: &mut Panels) {
    if std::mem::take(&mut panels.guild_popup_position_pending) {
        let bounds = panels.layout.virtual_screen();

        let cursor = hud.manager.cursor;

        if let Some(screen) = panels.screen_mut(super::guild_panel::MEMBER_POPUP) {
            let p = &screen.placed.ui.panel;

            screen.placed.origin = (
                cursor
                    .0
                    .clamp(bounds[0], (bounds[2] - p.width()).max(bounds[0])),
                cursor
                    .1
                    .clamp(bounds[1], (bounds[3] - p.height()).max(bounds[1])),
            );
        }
    }

    for content in std::mem::take(&mut panels.hover_leaves_pending) {
        if let Some((index, screen)) = panels
            .screens
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.content == content)
        {
            hud.manager
                .leave_hovered_panel(input_slot(index), &mut screen.placed.ui.panel);
        }
    }

    for target in std::mem::take(&mut panels.coin_focus_pending) {
        apply_focus(hud, panels, target);
    }
}

fn apply_focus(hud: &mut Hud, panels: &mut Panels, target: Option<Target>) {
    let chrome = [
        (0, &mut hud.bar.ui.panel),
        (1, &mut hud.plate.ui.panel),
        (2, &mut hud.target.ui.panel),
    ];

    let screens = panels
        .screens
        .iter_mut()
        .enumerate()
        .map(|(i, s)| (HUD_PANELS + i, &mut s.placed.ui.panel));

    hud.manager
        .request_focus(target, chrome.into_iter().chain(screens));
}

fn use_cell(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    keys: Mods,
    now_ms: u32,
    player_present: bool,
) {
    for event in events.iter().filter(|e| e.id == EventId::RightUp) {
        let Some(index) = event.panel.checked_sub(HUD_PANELS) else {
            continue;
        };

        let Some(screen) = panels.screens.get(index) else {
            continue;
        };

        if screen.content == CONTENT_SKILL {
            skill_cell(panels, index, event);

            continue;
        }

        if screen.content == CONTENT_MISSION_MIX {
            if screen.open {
                panels.mission_clicks.push(mission::Click::Back);
            }

            continue;
        }

        if screen.content == CONTENT_CARGO {
            if !player_present || !panels.is_open(CONTENT_INVENTORY) {
                continue;
            }

            let Some(element) = event
                .owner
                .and_then(|o| ui::input::retained_element(&screen.placed.ui.panel, o))
            else {
                continue;
            };

            if element.element_type != ElementType::IconButton {
                continue;
            }

            let Some(state) = element.widget.button() else {
                continue;
            };

            let slot = i32::from(state.icon_slot_index);

            if !state.tooltip_binding().is_some_and(|b| b.kind == 1)
                || state.cell_group != 3
                || !(0..120).contains(&slot)
            {
                continue;
            }

            let actions = item_use::right_click(
                entered,
                &panels.items,
                &panels.live,
                &mut panels.cooldowns,
                keys,
                now_ms,
                Coord { container: 2, slot },
                0,
            );

            panels.item_use_clear_hover |= actions
                .iter()
                .any(|a| matches!(a, item_use::Action::Send(item_move::Request::Move { .. })));

            carry_out(panels, entered, &actions, now_ms);

            panels.used.extend(actions);

            continue;
        }

        if screen.content != CONTENT_INVENTORY {
            continue;
        }

        if keys.ctrl {
            tracing::debug!(
                player_present, paired = ? paired(panels), owner = ? event.owner, body =
                entered.equipment.items[0].index, account_body = entered.account_body,
                main_hand = entered.equipment.items[6].index, off_hand = entered
                .equipment.items[7].index, "CTRL-right-click inventory callback"
            );
        }

        if !player_present {
            continue;
        }

        if !matches!(paired(panels), None | Some(item_use::CARGO_CONTENT)) {
            continue;
        }

        let Some(Owner::Element(element)) = event.owner else {
            continue;
        };

        let Some(element) = screen.placed.ui.panel.elements.get(element) else {
            continue;
        };

        if element.element_type != ElementType::IconButton {
            continue;
        }

        let Some(state) = element.widget.button() else {
            continue;
        };

        if !state.tooltip_binding().is_some_and(|b| b.kind == 1) {
            continue;
        }

        let slot = i32::from(state.icon_slot_index);

        if !(0..item_use::MAX_SLOT).contains(&slot) {
            continue;
        }

        if !matches!(
            state.cell_group,
            cell_group::EQUIPMENT | cell_group::INVENTORY
        ) {
            continue;
        }

        let at = Coord::of_cell(state.cell_group, slot);

        let open_tab = screen.page;

        let actions = {
            let Panels {
                items,
                live,
                cooldowns,
                ..
            } = &mut *panels;

            item_use::right_click(entered, items, live, cooldowns, keys, now_ms, at, open_tab)
        };

        if keys.ctrl {
            tracing::debug!(?at, ?actions, "CTRL-right-click inventory result");
        }

        carry_out(panels, entered, &actions, now_ms);

        panels.used.extend(actions);
    }
}

pub(crate) fn carry_out(
    panels: &mut Panels,
    entered: &mut Entered,
    actions: &[item_use::Action],
    now_ms: u32,
) {
    let mut touched = false;

    for action in actions {
        match action {
            item_use::Action::Toast { strdef } => {
                let text = panels.strings.translate(&format!("ss_{strdef}"));

                super::notice::post(panels, &text, 3000, true, now_ms);
            }
            item_use::Action::Firecracker { slot } => {
                panels.firecracker_slot = Some(*slot);
            }
            item_use::Action::Seal { slot } => panels.seal_open = Some(*slot),
            item_use::Action::Prompt {
                action: item_use::action::SELL,
                slot,
                ..
            } => {
                let at = Coord {
                    container: 1,
                    slot: *slot,
                };

                if let Some(item) = item_move::item_at(entered, at).filter(|item| !item.is_empty())
                {
                    let prompt = destroy::Prompt {
                        item_id: item.index,
                        slot: *slot,
                        snapshot: item,
                    };

                    let request = msgbox::Request {
                        caption: Some(237),
                        kind: 0xD,
                        context: 0x29,
                        body: msgbox::Body::Text(super::shop::message(
                            &panels.items,
                            &panels.strings,
                            &prompt,
                        )),
                        flags: 0x20000001,
                    };

                    if let Some(index) = panels.raise_box(&request, Some(prompt)) {
                        panels.box_cells[index] = Some(at);
                    }
                }
            }
            item_use::Action::Prompt {
                action: item_use::action::EXCHANGE,
                slot,
                ..
            } => {
                let request = msgbox::Request {
                    caption: None,
                    kind: super::interact::TALK_BOX_KIND,
                    context: 0x28,
                    body: msgbox::Body::TwoLines(149, 150),
                    flags: 0x20000001,
                };

                panels.raise_cell_box(
                    &request,
                    Coord {
                        container: 1,
                        slot: *slot,
                    },
                );
            }
            item_use::Action::Send(request) => {
                panels.outbox.push(*request);

                touched = true;
            }
            item_use::Action::Feed(send) => {
                panels.wire.push(send.frame(0));

                touched = true;
            }
            item_use::Action::Stage(request) => {
                panels.staged.push(*request);

                touched = true;
            }
            item_use::Action::HuntingScroll { page, request } => {
                panels.portal.page = Some(*page);

                panels.portal.staged = Some(*request);

                panels.portal.destination = 0;

                if panels.set_open(CONTENT_PORTAL, true).is_none() {
                    report_used(panels, action.clone());

                    continue;
                }

                let rows =
                    formats::text::potal_pos::page(&panels.potal_pos, *page as usize).to_vec();

                let strings = panels.strings.clone();

                if let Some(screen) = panels.screen_mut(CONTENT_PORTAL) {
                    screens::portal_scroll(screen, &rows, &strings, *page);
                }

                tracing::info!(
                    "NWYD: the hunting scroll opened the teleport master on page {page}"
                );

                touched = true;
            }
            item_use::Action::TrackUse { id } => {
                if let Some(name) = usize::try_from(*id)
                    .ok()
                    .and_then(|id| panels.items.name(id))
                {
                    panels.analytics.item_actioned(entered, *id, &name);
                }
            }
            item_use::Action::Sound { slot } => panels.notice.sounds.push(*slot),
            item_use::Action::ItemChanged { at } => {
                if at.container == item_move::container::INVENTORY {
                    refresh_inventory_hover(panels, entered, at.slot);
                }

                refine_item_changed(panels, entered);
            }
            item_use::Action::Prompt {
                action,
                strdef,
                slot,
            } if item_use::RAISED_PROMPTS.contains(action) => {
                let request = item_use::prompt_box(*action, *strdef);

                let at = Coord {
                    container: item_move::container::INVENTORY,
                    slot: *slot,
                };

                panels.raise_cell_box(&request, at);
            }
            item_use::Action::Open { content, slot } => {
                let content = *content as PanelId;

                if content == CONTENT_OPTION_STONE && panels.is_open(content) {
                    continue;
                }

                if content == CONTENT_OPTION_STONE && panels.screen(content).is_some() {
                    panels.pair_inventory(content, true);
                }

                if panels.set_open(content, true).is_none() {
                    report_used(panels, action.clone());

                    continue;
                }

                if content != CONTENT_OPTION_STONE {
                    panels.set_open(CONTENT_INVENTORY, true);
                }

                if content == CONTENT_OPTION_STONE
                    && (0..item_move::INVENTORY_SLOTS as i32).contains(slot)
                {
                    let at = Coord {
                        container: item_move::container::INVENTORY,
                        slot: *slot,
                    };

                    if let Some(item) =
                        item_move::item_at(entered, at).filter(|item| !item.is_empty())
                    {
                        option_stone_stage(panels, item, at, now_ms);
                    }
                }

                tracing::info!(
                    "NWYD: content {content:#04X} opened from backpack slot {slot}, \
                     bag paired"
                );

                touched = true;
            }
            item_use::Action::Refine { at } => {
                if panels.is_open(super::refine::CONTENT) {
                    continue;
                }

                let before = at.container == item_move::container::EQUIPMENT
                    || item_move::item_at(entered, *at).is_some_and(|item| {
                        item_use::POWDERS.contains(&i32::from(item.index as i16))
                    });

                if before && panels.screen(super::refine::CONTENT).is_some() {
                    panels.pair_inventory(super::refine::CONTENT, true);
                }

                if panels.set_open(super::refine::CONTENT, true).is_none() {
                    report_used(panels, action.clone());

                    continue;
                }

                if !before {
                    panels.pair_inventory(super::refine::CONTENT, true);
                }

                refine_stage_source(panels, entered, *at);

                touched = true;
            }
            other => {
                if let Some(line) = item_use::describe(other) {
                    tracing::info!("{line}");
                }
            }
        }
    }

    if touched {
        screens::refresh_all(panels, entered);
    }
}

fn refine_source_lock(panels: &mut Panels, at: Coord, locked: bool) {
    let Some(screen) = panels.screen_mut(CONTENT_INVENTORY) else {
        return;
    };

    let Some(owner) = screens::inventory_source(screen, at) else {
        return;
    };

    if let Some(state) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.instance == owner)
        .and_then(|element| element.widget.button_mut())
    {
        state.icon_locked = locked;
    }
}

fn refine_equipment_locked(panels: &Panels, slot: i32) -> bool {
    panels
        .screen(CONTENT_INVENTORY)
        .and_then(|screen| {
            let owner = screens::inventory_source(screen, Coord { container: 0, slot })?;

            screen
                .placed
                .ui
                .panel
                .elements
                .iter()
                .find(|e| e.instance == owner)
        })
        .and_then(|element| element.widget.button())
        .is_some_and(|button| button.icon_locked)
}

fn refine_paint_cell(panels: &mut Panels, cell: usize, item: Item) {
    let Panels {
        screens,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == super::refine::CONTENT)
    {
        let resource = screen.placed.ui.resource.clone();

        screens::set_cell_icon(
            screen,
            &format!("Mat{cell}"),
            &item,
            items,
            icons,
            &resource,
        );
    }
}

fn refine_clear_cell(panels: &mut Panels, cell: usize) {
    let Panels {
        screens,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == super::refine::CONTENT)
    {
        let resource = screen.placed.ui.resource.clone();

        screens::clear_cell(screen, &format!("Mat{cell}"), items, icons, &resource);
    }
}

fn refine_halt_ui(panels: &mut Panels) {
    refine_stop_controls(panels);

    if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
        screens::refine_pulse_halt(screen);
    }
}

fn refine_stop_controls(panels: &mut Panels) {
    panels.refine.running = false;

    panels.refine.auto = false;

    let strings = panels.strings.clone();

    if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
        screens::refine_auto_mirror(screen, false);

        screens::refine_controls(screen, false, true, &strings);
    }
}

fn refine_toast(panels: &mut Panels, strdef: usize) {
    let text = panels.strings.translate(&format!("ss_{strdef}"));

    notice::post(panels, &text, 2000, true, panels.now_ms);
}

fn refine_reset(panels: &mut Panels, opening: bool) {
    for slot in 0..60 {
        refine_source_lock(
            panels,
            Coord {
                container: item_move::container::INVENTORY,
                slot,
            },
            false,
        );
    }

    for slot in 1..18 {
        refine_source_lock(
            panels,
            Coord {
                container: item_move::container::EQUIPMENT,
                slot,
            },
            false,
        );
    }

    panels.refine.reset();

    for cell in 0..super::refine::CELLS {
        refine_clear_cell(panels, cell);
    }

    let Panels {
        screens,
        refine,
        strings,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|s| s.content == super::refine::CONTENT)
    {
        screens::refine_result(screen, refine, items, icons);

        if opening {
            screens::refine_opened(screen, strings);
        } else {
            screens::refine_closed(screen, strings);
        }
    }
}

fn refine_select_recipe(panels: &mut Panels, entered: &Entered, mode: i32) -> bool {
    if mode == 0 && panels.refine.mode == 0 {
        return true;
    }

    let Some(list) = panels.mixes.as_ref() else {
        return mode == 0;
    };

    if !panels.refine.retain_recipe(list, mode) {
        return false;
    }

    let locked = super::refine::gather_recipe(
        panels.refine.recipe.as_mut().expect("retained recipe"),
        list,
        &panels.items,
        &entered.carry,
    );

    panels.refine.powder = Default::default();

    for (slot, locked) in locked.into_iter().enumerate() {
        refine_source_lock(
            panels,
            Coord {
                container: item_move::container::INVENTORY,
                slot: slot as i32,
            },
            locked,
        );
    }

    let Panels {
        screens,
        refine,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == super::refine::CONTENT)
    {
        screens::refine_recipe_icons(
            screen,
            refine.recipe.as_ref().expect("retained recipe"),
            items,
            icons,
        );
    }

    true
}

fn refine_refresh(panels: &mut Panels) {
    let Panels {
        screens,
        refine,
        mixes,
        items,
        icons,
        strings,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == super::refine::CONTENT)
    {
        screens::refine_refresh(screen, refine, strings);

        screens::refine_result(screen, refine, items, icons);

        if let Some(list) = mixes
            && refine.mode != 0
        {
            screens::refine_recipe_labels(screen, refine, list, items, strings);
        }
    }
}

fn refine_displayed_item(panels: &Panels, name: &str) -> Item {
    panels
        .screen(super::refine::CONTENT)
        .and_then(|screen| screen.refine_element(name, ElementType::IconButton))
        .and_then(|element| element.widget.button())
        .and_then(|button| button.icon_contents)
        .map(|icon| icon.item)
        .unwrap_or_default()
}

fn refine_stage_powder(panels: &mut Panels, entered: &Entered, at: Coord, item: Item) -> bool {
    if !super::refine::displayed_piece_accepts_powder(
        &panels.items,
        refine_displayed_item(panels, "Mat0"),
        item,
    ) {
        refine_toast(panels, super::refine::WRONG_POWDER);

        refine_halt_ui(panels);

        return false;
    }

    for slot in 0..item_move::INVENTORY_SLOTS {
        refine_source_lock(
            panels,
            Coord {
                container: item_move::container::INVENTORY,
                slot: slot as i32,
            },
            false,
        );
    }

    let total = entered
        .carry
        .iter()
        .take(64)
        .filter(|candidate| candidate.index == item.index)
        .map(super::consume::stored_count)
        .sum::<i32>();

    for (slot, candidate) in entered.carry.iter().take(60).enumerate() {
        if candidate.index == item.index {
            refine_source_lock(
                panels,
                Coord {
                    container: item_move::container::INVENTORY,
                    slot: slot as i32,
                },
                true,
            );
        }
    }

    let preview = Item {
        index: item.index,
        appearance: total as u16,
        ..Default::default()
    };

    panels.refine.retain_powder(at, preview);

    refine_paint_cell(panels, 1, preview);

    let Panels { screens, items, .. } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|s| s.content == super::refine::CONTENT)
    {
        if let Some(icon) = screen
            .refine_element_mut("Mat1", ElementType::IconButton)
            .and_then(|element| element.widget.button_mut())
            .and_then(|button| button.icon_contents.as_mut())
        {
            icon.count = total;
        }

        screens::refine_powder_name(screen, &item, items);
    }

    true
}

fn refine_stage_source(panels: &mut Panels, entered: &Entered, at: Coord) -> bool {
    let Some(item) = item_move::item_at(entered, at).filter(|item| !item.is_empty()) else {
        return false;
    };

    let id = i32::from(item.index as i16);

    let equipment = at.container == item_move::container::EQUIPMENT;

    if equipment && !(1..14500).contains(&id) {
        return false;
    }

    if let Some(mode) = super::refine::selected_recipe(&panels.items, &panels.refine_sets, &item) {
        if !refine_select_recipe(panels, entered, mode) {
            return false;
        }

        if equipment && refine_equipment_locked(panels, panels.refine.piece.at.slot) {
            refine_source_lock(
                panels,
                Coord {
                    container: 0,
                    slot: panels.refine.piece.at.slot,
                },
                false,
            );

            refine_clear_cell(panels, 0);

            let Panels {
                screens,
                items,
                icons,
                ..
            } = panels;

            if let Some(screen) = screens
                .iter_mut()
                .find(|s| s.content == super::refine::CONTENT)
            {
                let resource = screen.placed.ui.resource.clone();

                screens::clear_cell(screen, "ResultItem", items, icons, &resource);
            }

            panels.refine.current = 0;
        }

        let shown_powder = refine_displayed_item(panels, "Mat1");

        if equipment && panels.refine.mode == 0 && !shown_powder.is_empty() {
            let effects = Effects::new(&panels.items, &[]);

            let powder = i32::from(shown_powder.index as i16);

            if !super::refine::powder_matches(
                powder,
                id,
                refine_level(&item, &panels.items) as i32,
                effects.total_scaled(&item, 0x70),
                effects.total_scaled(&item, 0x71),
            ) {
                refine_toast(panels, super::refine::WRONG_POWDER);

                refine_stop_controls(panels);

                return false;
            }
        }

        if equipment && refine_equipment_locked(panels, at.slot) {
            return false;
        }

        if !equipment {
            for slot in 1..18 {
                refine_source_lock(panels, Coord { container: 0, slot }, false);
            }
        }

        let old = panels.refine.piece;

        if old.selected {
            refine_source_lock(panels, old.at, false);
        }

        if !equipment && old.at.slot != at.slot {
            panels.refine.target = 0;
        }

        let level = refine_level(&item, &panels.items) as i32;

        panels.refine.current = level;

        let from = if panels.refine.target < level + 1 {
            level
        } else {
            panels.refine.target - 1
        };

        if let Some(target) =
            super::refine::sanc_up(from, item_use::refine_ceiling(&panels.items, &item))
        {
            panels.refine.preview_target(item, target, &panels.items);
        }

        panels.refine.retain_piece(at, item);

        refine_source_lock(panels, at, true);

        refine_paint_cell(panels, 0, item);

        refine_refresh(panels);

        return true;
    }

    if equipment {
        refine_toast(panels, super::refine::WRONG_POWDER);

        refine_halt_ui(panels);

        return false;
    }

    if panels.refine.mode == 0 && item_use::POWDERS.contains(&id) {
        return refine_stage_powder(panels, entered, at, item);
    }

    if panels.refine.mode > 0 && at.container == item_move::container::INVENTORY {
        let Some(list) = panels.mixes.as_ref() else {
            return false;
        };

        let effects = Effects::new(&panels.items, &[]);

        let admitted = panels.refine.recipe.as_mut().and_then(|fill| {
            super::refine::admit_recipe_item(
                fill,
                list,
                &panels.items,
                &effects,
                at.slot as usize,
                item,
            )
        });

        if let Some(admitted) = admitted {
            refine_source_lock(panels, at, true);

            refine_paint_cell(panels, admitted.need, item);

            refine_refresh(panels);

            return true;
        }

        return false;
    }

    refine_halt_ui(panels);

    false
}

fn report_used(panels: &mut Panels, action: item_use::Action) {
    if let Some(line) = item_use::describe(&action) {
        tracing::info!("{line}");
    }

    panels.used.push(action);
}

fn skill_cell_id(panels: &Panels, index: usize, owner: Option<Owner>) -> Option<i32> {
    let panel = &panels.screens.get(index)?.placed.ui.panel;

    let element = ui::input::retained_element(panel, owner?)?;

    if element.element_type != ElementType::IconButton {
        return None;
    }

    let state = element.widget.button()?;

    let contents = state.tooltip_binding()?;

    if contents.kind != ui::icon::SKILL_KIND || state.cell_group != cell_group::SKILL {
        return None;
    }

    (contents.index != -1 && contents.index != 0).then_some(contents.index)
}

fn hovered_skill_icon_id(panels: &Panels, index: usize, owner: Option<Owner>) -> Option<i32> {
    let Some(Owner::Element(owner)) = owner else {
        return None;
    };

    let element = panels
        .screens
        .get(index)?
        .placed
        .ui
        .panel
        .elements
        .get(owner)?;

    if element.element_type != ElementType::IconButton {
        return None;
    }

    let contents = element.widget.button()?.icon_contents?;

    (contents.kind == ui::icon::SKILL_KIND
        && (1..=super::shop::MAX_TEMPLATE_ID).contains(&contents.index))
    .then_some(contents.index)
}

pub const MASTERY_BOOK_BIAS: std::ops::RangeInclusive<i32> = 144..=191;

fn mission_click(panels: &mut Panels, index: usize, event: &Event) {
    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    let Some(Owner::Element(owner)) = event.owner else {
        return;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return;
    };

    if !screen.open || !element.visible {
        return;
    }

    if element.name.eq_ignore_ascii_case("Btn") {
        panels.raise_box(&mission::confirmation(), None);

        return;
    }

    if element.name.eq_ignore_ascii_case("close") {
        return;
    }

    let is_recipe = (0..24).any(|cell| element.name.eq_ignore_ascii_case(&format!("Inven{cell}")));

    let recipe = is_recipe
        .then(|| {
            element
                .widget
                .button()
                .map(|b| i32::from(b.icon_slot_index))
        })
        .flatten();

    let screen = &mut panels.screens[index];

    for (at, element) in screen.placed.ui.panel.elements.iter_mut().enumerate() {
        if (0..24).any(|cell| element.name.eq_ignore_ascii_case(&format!("Inven{cell}")))
            && let Some(button) = element.widget.button_mut()
        {
            button.checked = at == owner;
        }
    }

    if let Some(id) = recipe {
        panels.mission_clicks.push(mission::Click::Slot(id));
    }
}

fn mix_click(panels: &mut Panels, index: usize, event: &Event) {
    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    if !screen.open || event.name == "close" {
        return;
    }

    if event.name == super::mix::SEND_BUTTON {
        panels.mix_clicks.push(super::mix::Click::Send);

        return;
    }

    let Some(Owner::Element(owner)) = event.owner else {
        return;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return;
    };

    if !screen.mix_recipe_owners.contains(&Some(element.instance)) {
        return;
    }

    let Some(state) = element.widget.button() else {
        return;
    };

    let Ok(recipe) = usize::try_from(state.tooltip_binding().map_or(0, |b| b.help_id)) else {
        return;
    };

    panels.mix_clicks.push(super::mix::Click::Recipe(recipe));
}

fn combine_command(
    panels: &mut Panels,
    kind: super::combine::Kind,
    name: &str,
) -> Option<Vec<Action>> {
    if kind == super::combine::Kind::RefinedEquipment {
        if panels.is_open(kind.content()) && name == super::combine::COMBINE_BUTTON {
            panels.raise_box(&super::combine::confirm(kind), None);
        }

        return Some(Vec::new());
    }

    if !panels.is_open(kind.content()) || name == "close" {
        return Some(Vec::new());
    }

    if name == super::combine::COMBINE_BUTTON {
        if panels.is_open(kind.content()) {
            panels.raise_box(&super::combine::confirm(kind), None);
        }

        return Some(Vec::new());
    }

    let cell = panels.screen(kind.content()).and_then(|screen| {
        screen
            .element(name, ElementType::IconButton)
            .and_then(|element| {
                screen
                    .combine_widgets
                    .materials
                    .iter()
                    .position(|bound| *bound == Some(element.instance))
            })
    });

    if let Some(cell) = cell {
        option_stone_remove(panels, cell);
    }

    Some(Vec::new())
}

fn option_stone_remove(panels: &mut Panels, cell: usize) {
    let freed = panels.staging.unstage_option_stone(cell);

    let preview = panels.staging.option_stone_preview(&panels.items);

    let Panels {
        screens,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_OPTION_STONE)
    {
        screens::option_stone_unstaged(screen, cell, items, icons);

        screens::option_stone_preview(screen, preview, items, icons);
    }

    if let Some(slot) = freed
        && let Some(inventory) = screens
            .iter_mut()
            .find(|screen| screen.content == CONTENT_INVENTORY)
    {
        screens::inventory_lock(inventory, usize::from(slot), false);
    }
}

fn option_stone_stage(panels: &mut Panels, item: Item, at: Coord, now_ms: u32) {
    let staged = {
        let Panels { staging, items, .. } = panels;

        staging.stage_option_stone(item, at, items)
    };

    let replacement = match staged {
        Ok(replacement) => replacement,
        Err(super::combine::OptionStoneRefused::Incompatible) => {
            let text = panels.strings.translate("ss_1271");

            super::notice::post(panels, &text, 2000, true, now_ms);

            return;
        }
    };

    let preview = panels.staging.option_stone_preview(&panels.items);

    let Panels {
        screens,
        items,
        icons,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_OPTION_STONE)
    {
        screens::option_stone_staged(screen, replacement.cell, &item, items, icons);
    }

    if let Some(inventory) = screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_INVENTORY)
    {
        screens::inventory_lock(inventory, at.slot as usize, true);

        if let Some(old) = replacement.replaced {
            screens::inventory_lock(inventory, usize::from(old), false);
        }
    }

    if let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_OPTION_STONE)
    {
        screens::option_stone_preview(screen, preview, items, icons);
    }
}

fn pgt_parties(
    panels: &Panels,
    entered: &Entered,
    player_model: u8,
    player_dead: bool,
) -> (super::pgt::Party, Option<super::pgt::Party>) {
    let player = super::pgt::Party {
        entity: entered.client_id,
        guild: entered.mob_guild,
        rank: i16::from(entered.mob_guild_rank),
        dead: player_dead,
        tracked: false,
        name: formats::text::codepage::decode(
            &entered.name[..entered
                .name
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(entered.name.len())],
            formats::text::Codepage::Windows1252,
        ),
        model_id: player_model,
        score: (
            entered.status.level,
            entered.status.max_hp as u16,
            entered.status.hp as u16,
        ),
    };

    (player, panels.pgt_target.clone())
}

fn skill_cell(panels: &mut Panels, index: usize, event: &Event) {
    let Some(id) = skill_cell_id(panels, index, event.owner) else {
        return;
    };

    let Some(code) = super::cast::panel_code(id) else {
        return;
    };

    panels.skill_casts.push(code);
}

pub fn pick_up(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    now_ms: u32,
) {
    let _ = pick_up_in_field(
        hud,
        panels,
        entered,
        events,
        FieldInput {
            now_ms,
            player_present: true,
            player_dead: false,
        },
    );
}

pub fn pick_up_in_field(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    input: FieldInput,
) -> bool {
    let now_ms = input.now_ms;

    let mut clear_hover = false;

    for event in events {
        if event.id != EventId::Click {
            continue;
        }

        let Some(index) = event.panel.checked_sub(HUD_PANELS) else {
            continue;
        };

        let Some(screen) = panels.screens.get(index) else {
            continue;
        };

        if screen.content == super::merge_item::CONTENT {
            if let Some(Owner::Element(owner)) = event.owner
                && screen
                    .placed
                    .ui
                    .panel
                    .elements
                    .get(owner)
                    .is_some_and(|e| e.element_type == ElementType::IconButton)
            {
                super::merge_item::command(panels, entered, &event.name);
            }

            continue;
        }

        if screen.content == CONTENT_CARGO {
            cargo_click(hud, panels, entered, index, event, input);

            continue;
        }

        if screen.content == CONTENT_ITEM_SHOP {
            shop_click(panels, entered, index, event);

            continue;
        }

        if screen.content == CONTENT_MISSION_MIX {
            mission_click(panels, index, event);

            continue;
        }

        if screen.content == super::mix::CONTENT {
            mix_click(panels, index, event);

            continue;
        }

        if screen.content == CONTENT_SKILL {
            skill_drag(hud, panels, index, event);

            continue;
        }

        if screen.content == CONTENT_STORE {
            store_click(panels, entered, index, event);

            continue;
        }

        if screen.content != CONTENT_INVENTORY {
            continue;
        }

        if !input.player_present || !screen.open || event.name == "close" {
            continue;
        }

        let Some(Owner::Element(owner)) = event.owner else {
            continue;
        };

        let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
            continue;
        };

        if element.element_type != ElementType::IconButton {
            continue;
        }

        let Some(state) = element.widget.button() else {
            continue;
        };

        if !state.tooltip_binding().is_some_and(|b| b.kind == 1) {
            continue;
        }

        if state.icon_locked {
            continue;
        }

        let slot = i32::from(state.icon_slot_index);

        match state.cell_group {
            cell_group::EQUIPMENT if slot == INERT_EQUIP_SLOT => continue,
            cell_group::EQUIPMENT | cell_group::INVENTORY => {}
            _ => continue,
        }

        let paired = paired(panels);

        if paired == Some(super::merge_item::CONTENT) {
            if state.cell_group == cell_group::INVENTORY && (0..60).contains(&slot) {
                super::merge_item::stage(panels, entered, slot as usize);
            }

            continue;
        }

        if paired == Some(super::refine::CONTENT) {
            if panels.is_open(super::refine::CONTENT) && !panels.refine.running {
                let at = Coord::of_cell(state.cell_group, slot);

                refine_stage_source(panels, entered, at);
            }

            continue;
        }

        if panels.inventory_block_equipment && state.cell_group == cell_group::EQUIPMENT {
            continue;
        }

        if paired == Some(CONTENT_TRADE) && state.cell_group == cell_group::INVENTORY {
            if (0..60).contains(&slot) {
                panels.trade_item_clicks.push(super::trade::ItemClick {
                    source: slot as usize,
                    owner: element.instance,
                });
            }

            continue;
        }

        if paired == Some(super::mix::CONTENT) && state.cell_group == cell_group::INVENTORY {
            if panels.is_open(super::mix::CONTENT)
                && (0..60).contains(&slot)
                && let Some(item) = entered.carry.get(slot as usize).copied()
            {
                let Panels {
                    mixes,
                    items,
                    mix,
                    staging,
                    ..
                } = panels;

                if let Some(list) = mixes.as_ref()
                    && super::mix::admit_item(
                        list,
                        items,
                        mix,
                        staging.get_mut(super::combine::Kind::Mix),
                        slot as usize,
                        item,
                    )
                    .is_some()
                {
                    if let Some(bag) = panels.screen_mut(CONTENT_INVENTORY) {
                        screens::inventory_lock(bag, slot as usize, true);
                    }

                    panels.refresh_mix();
                }
            }

            continue;
        }

        if paired == Some(CONTENT_MISSION_MIX) && state.cell_group == cell_group::INVENTORY {
            if (0..60).contains(&slot) {
                panels
                    .mission_clicks
                    .push(mission::Click::Material(slot as usize));
            }

            continue;
        }

        if paired == Some(CONTENT_ITEM_MIX4) && state.cell_group == cell_group::INVENTORY {
            if !panels.is_open(CONTENT_ITEM_MIX4) {
                continue;
            }

            if !(0..item_move::INVENTORY_SLOTS as i32).contains(&slot) {
                continue;
            }

            let at = Coord::of_cell(state.cell_group, slot);

            let Some(item) = item_move::item_at(entered, at).filter(|item| !item.is_empty()) else {
                continue;
            };

            let staged = {
                let Panels { staging, items, .. } = panels;

                super::combine::stage_refined_equipment(
                    staging.get_mut(super::combine::Kind::RefinedEquipment),
                    item,
                    at,
                    items,
                )
            };

            match staged {
                Ok(cell) => {
                    let Panels {
                        screens,
                        items,
                        icons,
                        ..
                    } = panels;

                    if let Some(screen) = screens
                        .iter_mut()
                        .find(|screen| screen.content == CONTENT_ITEM_MIX4)
                    {
                        screens::combine_staged(screen, cell, &item, items, icons);
                    }

                    if let Some(inventory) = screens
                        .iter_mut()
                        .find(|screen| screen.content == CONTENT_INVENTORY)
                    {
                        screens::inventory_lock(inventory, slot as usize, true);
                    }
                }
                Err(refused) => {
                    if let Some(record) = refused.strdef() {
                        let text = panels.strings.translate(&format!("ss_{record}"));

                        super::notice::post(panels, &text, 2000, true, now_ms);
                    }
                }
            }

            continue;
        }

        if paired == Some(CONTENT_OPTION_STONE) && state.cell_group == cell_group::INVENTORY {
            if !panels.is_open(CONTENT_OPTION_STONE) {
                continue;
            }

            if !(0..item_move::INVENTORY_SLOTS as i32).contains(&slot) {
                continue;
            }

            let at = Coord::of_cell(state.cell_group, slot);

            let Some(item) = item_move::item_at(entered, at).filter(|item| !item.is_empty()) else {
                continue;
            };

            option_stone_stage(panels, item, at, now_ms);

            continue;
        }

        if paired == Some(CONTENT_STORE) {
            continue;
        }

        let at = Coord::of_cell(state.cell_group, slot);

        let cargo = warehouse(panels, entered);

        match item_use::left_click(entered, &panels.items, hud.manager.keys, cargo, at) {
            item_use::Click::Nothing => continue,
            item_use::Click::Split { slot } => {
                let owner = element.instance;

                super::stack_split::begin_with_owner(panels, entered, slot as usize, Some(owner));

                continue;
            }
            item_use::Click::Send(request) => {
                panels.outbox.push(request);

                panels.used.push(item_use::Action::Send(request));

                clear_hover = true;

                continue;
            }
            item_use::Click::PickUp => {}
        }

        let Some(carried) = ui::Carried::pick_up(ui::carry::origin_screen::INVENTORY, element)
        else {
            continue;
        };

        hud.manager.pick_up(carried);
    }

    if std::mem::take(&mut panels.refine.manual_requested) {
        refine_manual_send(panels, entered, now_ms);
    }

    clear_hover
}

fn skill_drag(hud: &mut Hud, panels: &mut Panels, index: usize, event: &Event) {
    let Some(id) = skill_cell_id(panels, index, event.owner) else {
        return;
    };

    let row = formats::data::skill_data::skill_index(id);

    let passive = usize::try_from(row)
        .ok()
        .and_then(|row| panels.skills.as_ref()?.get(row))
        .is_some_and(|skill| skill.is_passive());

    if passive {
        return;
    }

    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    let Some(element) = event
        .owner
        .and_then(|owner| ui::input::retained_element(&screen.placed.ui.panel, owner))
    else {
        return;
    };

    let Some(carried) = ui::Carried::pick_up(ui::carry::origin_screen::INVENTORY, element) else {
        return;
    };

    hud.manager.pick_up(ui::Carried {
        kind: ui::icon::SKILL_KIND,
        slot: 0,
        ..carried
    });
}

fn cargo_click(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &Entered,
    index: usize,
    event: &Event,
    input: FieldInput,
) {
    let now_ms = input.now_ms;

    if matches!(
        event.name.as_str(),
        "close" | "TabItem0" | "TabItem1" | "TabItem2"
    ) {
        return;
    }

    if event.name == "MoneyBtn"
        && (!input.player_present
            || input.player_dead
            || panels.screen(CONTENT_COIN_INPUT).is_none()
            || panels.is_open(CONTENT_INVENTORY))
    {
        return;
    }

    let Some(Owner::Element(owner)) = event.owner else {
        return;
    };

    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return;
    };

    if element.element_type != ElementType::IconButton {
        return;
    }

    if screens::cargo_destination(screen, element.instance).is_none() {
        return;
    }

    let Some(state) = element.widget.button() else {
        return;
    };

    if !state.tooltip_binding().is_some_and(|b| b.kind == 1) || state.icon_locked {
        return;
    }

    let slot = i32::from(state.icon_slot_index);

    if panels.is_open(CONTENT_STORE) {
        let at = Coord { container: 2, slot };

        if item_move::item_at(entered, at).is_none_or(|item| item.index == 0) {
            return;
        }

        if item_use::deposits(entered, &panels.items, at) {
            super::store::begin_price(panels, entered, slot);
        } else {
            super::store::blocked_item(panels, now_ms);
        }

        return;
    }

    if !input.player_present {
        return;
    }

    if hud.manager.keys.ctrl && panels.is_open(CONTENT_INVENTORY) {
        let free = item_use::free_carry_slot(
            entered,
            panels.screen(CONTENT_INVENTORY).unwrap().page,
            true,
        );

        if free < 0 {
            return;
        }

        let request = item_move::Request::Move {
            dst: Coord {
                container: item_move::container::INVENTORY,
                slot: free,
            },
            src: Coord {
                container: item_move::container::CARGO,
                slot,
            },
        };

        panels.outbox.push(request);

        panels.used.push(item_use::Action::Send(request));

        panels.item_use_clear_hover = true;

        return;
    }

    if state.cell_group != cell_group::CARGO {
        return;
    }

    if let Some(carried) = ui::Carried::pick_up(ui::carry::origin_screen::INVENTORY, element) {
        hud.manager.pick_up(carried);
    }
}

fn store_click(panels: &mut Panels, entered: &Entered, index: usize, event: &Event) {
    if !panels.field_close_owner_present || matches!(event.name.as_str(), "CloseBtn" | "Btn") {
        return;
    }

    let Some(Owner::Element(owner)) = event.owner else {
        return;
    };

    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return;
    };

    if element.element_type != ElementType::IconButton {
        return;
    }

    let Some(slot) = (0..10).find(|slot| {
        screen
            .element(&format!("Inven{slot}"), ElementType::IconButton)
            .is_some_and(|bound| std::ptr::eq(bound, element))
    }) else {
        return;
    };

    match super::store::clicked(panels, entered, slot) {
        super::store::Bought::Asked { slot } => {
            let request = super::store::request(&panels.strings.clone());

            if let Some(box_slot) = panels.raise_box(&request, None) {
                panels.box_entities[box_slot] = u16::try_from(slot).ok();
            }
        }
        super::store::Bought::NoSpace => super::store::no_space(panels),
        _ => {}
    }
}

fn shop_click(panels: &mut Panels, _entered: &Entered, index: usize, event: &Event) {
    let Some(Owner::Element(owner)) = event.owner else {
        return;
    };

    let Some(screen) = panels.screens.get(index) else {
        return;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return;
    };

    if element.name.eq_ignore_ascii_case(SHOP_PAGE_CELL) {
        panels.shop_clicks.push(super::shop::Click::Page);

        return;
    }

    if element.element_type != ElementType::IconButton {
        return;
    }

    let Some(state) = element.widget.button() else {
        return;
    };

    let Some(contents) = state.icon_contents else {
        return;
    };

    if contents.kind != 1 || state.cell_group != cell_group::SHOP {
        return;
    }

    let (cell, item_id) = (i32::from(state.icon_slot_index), contents.index);

    if (0..27).any(|i| {
        screen
            .element(&format!("Inven{i}"), ElementType::IconButton)
            .is_some_and(|e| std::ptr::eq(e, element))
    }) {
        panels
            .shop_clicks
            .push(super::shop::Click::Buy { cell, item_id });
    }
}

pub const SHOP_PAGE_CELL: &str = "Inven39";

pub const SHOP_SELL_AREA: &str = "Sell";

fn sell_drop(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &Entered,
    owner: Owner,
    index: usize,
) -> bool {
    let Owner::Element(owner) = owner else {
        return false;
    };

    let Some(screen) = panels.screens.get(index) else {
        return false;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
        return false;
    };

    if element.element_type != ElementType::IconButton
        && !element.name.eq_ignore_ascii_case(SHOP_SELL_AREA)
    {
        return false;
    }

    let Some(held) = hud.manager.carried else {
        return false;
    };

    if held.kind != 1 {
        return false;
    }

    if usize::try_from(held.slot)
        .ok()
        .and_then(|slot| entered.carry.get(slot))
        != Some(&held.contents.item)
    {
        hud.manager.cancel_carry();

        return true;
    }

    let carry = super::shop::Carry {
        kind: held.kind,
        group: held.group,
        slot: held.slot,
        item_id: held.index,
    };

    match super::shop::sell(panels, entered, hud.manager.keys.ctrl, carry) {
        super::shop::Sold::Sent(request) => panels.used.push(item_use::Action::Send(request)),
        super::shop::Sold::Asks { prompt, .. } => {
            let request = super::shop::request(&panels.items, &panels.strings, &prompt);

            panels.raise_box(&request, Some(prompt));
        }
        _ => {}
    }

    hud.manager.cancel_carry();

    true
}

pub const INERT_EQUIP_SLOT: i32 = 15;

fn warehouse<'a>(panels: &'a Panels, entered: &'a Entered) -> Option<item_use::Cargo<'a>> {
    let screen = panels.screen(CONTENT_CARGO)?;

    if !screen.open {
        return None;
    }

    Some(item_use::Cargo {
        slots: &entered.warehouse.cargo,
        tab: screen.page,
    })
}

fn paired(panels: &Panels) -> Option<PanelId> {
    (panels.inventory_partner != 0).then_some(panels.inventory_partner)
}

pub const PAIRABLE: [PanelId; 7] = [0x35, 0x1D, 0x1C, 0x19, 0x1A, 0x1E, 0x36];

pub const DELETE_CELL: &str = "Delete";

fn drop_carry(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    quick_bar: &mut super::quick_bar::QuickBar,
    now_ms: u32,
) -> bool {
    let target = {
        let mut refs: Vec<PanelRef<'_>> = Vec::with_capacity(HUD_PANELS + panels.screens.len());

        for panel in [&hud.bar, &hud.plate, &hud.target] {
            refs.push(PanelRef {
                panel: &panel.ui.panel,
                origin: panel.origin,
            });
        }

        for screen in &panels.screens {
            refs.push(PanelRef {
                panel: &screen.placed.ui.panel,
                origin: screen.placed.origin,
            });
        }

        hud.manager.drop_at(&refs)
    };

    let Some(target) = target else {
        return false;
    };

    if target.panel < HUD_PANELS {
        return quick_drop(hud, panels, entered, quick_bar, target);
    }

    let Some(index) = target.panel.checked_sub(HUD_PANELS) else {
        return false;
    };

    let Some(screen) = panels.screens.get(index) else {
        return false;
    };

    if screen.content == CONTENT_ITEM_SHOP {
        return sell_drop(hud, panels, entered, target.owner, index);
    }

    if !matches!(screen.content, CONTENT_INVENTORY | CONTENT_CARGO) {
        return false;
    }

    let Owner::Element(element) = target.owner else {
        return false;
    };

    let Some(element) = screen.placed.ui.panel.elements.get(element) else {
        return false;
    };

    if element.element_type != ElementType::IconButton {
        return false;
    }

    let Some(state) = element.widget.button() else {
        return false;
    };

    let Some(carried) = hud.manager.carried else {
        return false;
    };

    if carried.kind != 1 {
        return false;
    }

    if screen.content == CONTENT_INVENTORY && state.icon_locked {
        hud.manager.cancel_carry();

        return true;
    }

    if screen.content == CONTENT_INVENTORY && element.name == DELETE_CELL {
        if let Some(carried) = hud.manager.carried
            && let Ok(item_id) = u16::try_from(carried.index)
            && let Some(prompt) = destroy::prompt(entered, carried.group, item_id, carried.slot)
        {
            let request = destroy::request(&panels.items, &panels.strings, &prompt);

            panels.raise_box(&request, Some(prompt));
        }

        hud.manager.cancel_carry();

        return true;
    }

    let dst = if screen.content == CONTENT_INVENTORY {
        let Some(dst) = screens::inventory_destination(screen, element.instance) else {
            return false;
        };

        dst
    } else {
        let Some(slot) = screens::cargo_destination(screen, element.instance) else {
            return false;
        };

        if !matches!(carried.group, 1..=3) {
            hud.manager.cancel_carry();

            return true;
        }

        Coord {
            container: 2,
            slot: slot as i32,
        }
    };

    let src = Coord::of_cell(carried.group, carried.slot);

    let closes_combine = item_move::closes_combine_prompt(
        item_move::item_at(entered, src).unwrap_or_default(),
        item_move::item_at(entered, dst).unwrap_or_default(),
    );

    let outcome = match dst.container == item_move::container::EQUIPMENT
        && src.container == item_move::container::EQUIPMENT
    {
        true => item_move::Outcome::Nothing,
        false => item_move::drop_item(
            entered,
            &panels.items,
            &mut panels.cooldowns,
            now_ms,
            src,
            dst,
        ),
    };

    if closes_combine
        && matches!(
            outcome,
            item_move::Outcome::Sent(item_move::Request::Combine { .. })
        )
    {
        panels.hide_box_contexts(0x3E, 0);
    }

    complete_item_drop(panels, entered, outcome);

    hud.manager.cancel_carry();

    true
}

pub fn complete_item_drop(panels: &mut Panels, entered: &Entered, outcome: item_move::Outcome) {
    if let item_move::Outcome::ConfirmCombine(prompt) = outcome {
        panels.hide_box_contexts(0x3E, 0);

        let request = msgbox::Request {
            caption: Some(237),
            kind: 0xD,
            context: 0x3E,
            body: msgbox::Body::StrDef(prompt.message),
            flags: 0x20000001,
        };

        if let Some(slot) = panels.raise_box(&request, None) {
            panels.box_combines[slot] = Some(prompt);
        }
    }

    if let item_move::Outcome::Sent(request) = outcome {
        if !super::grouping::intercept(panels, entered, request) {
            panels.outbox.push(request);
        }
    }

    if let item_move::Outcome::Sent(item_move::Request::Combine { .. }) = outcome {
        let Panels {
            screens,
            items,
            icons,
            strings,
            ..
        } = panels;

        if let Some(screen) = screens.iter_mut().find(|s| s.content == CONTENT_INVENTORY) {
            screens::inventory_refresh(screen, entered, items, icons, strings);
        }

        refine_item_changed(panels, entered);
    }
}

pub const BAR_PANEL: usize = 0;

fn quick_drop(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    quick_bar: &mut super::quick_bar::QuickBar,
    target: Target,
) -> bool {
    let Some(element) = hud
        .input_pane(target.panel)
        .and_then(|p| ui::input::retained_element(&p.ui.panel, target.owner))
    else {
        return false;
    };

    if element.element_type != ElementType::IconButton {
        return false;
    }

    let Some(carried) = hud.manager.carried else {
        return false;
    };

    if carried.kind == ui::icon::ITEM_KIND {
        return quick_item_drop(hud, panels, entered, quick_bar, target, carried);
    }

    if carried.kind != ui::icon::SKILL_KIND || carried.group != cell_group::SKILL {
        return false;
    }

    let bind = quick_bind(hud, panels, entered, target, carried.index);

    if bind {
        panels
            .wire
            .push(protocol::client::quick_bar(&entered.quick, entered.class));

        panels.notice.sounds.push(super::shop::SALE_SOUND);
    }

    hud.manager.cancel_carry();

    true
}

fn quick_item_drop(
    hud: &mut Hud,
    panels: &Panels,
    entered: &Entered,
    quick_bar: &mut super::quick_bar::QuickBar,
    target: Target,
    carried: ui::carry::Carried,
) -> bool {
    if carried.group == cell_group::NONE {
        return false;
    }

    if carried.group != cell_group::INVENTORY {
        hud.manager.cancel_carry();

        return true;
    }

    let Some(state) = hud
        .input_pane(target.panel)
        .and_then(|p| ui::input::retained_element(&p.ui.panel, target.owner))
        .and_then(|e| e.widget.button())
    else {
        return false;
    };

    if state.cell_group != cell_group::QUICK {
        return false;
    }

    let source = usize::try_from(carried.slot)
        .ok()
        .filter(|&slot| slot < 0x40);

    if let (Some(source), Ok(id)) = (source, u32::try_from(carried.index))
        && (1..=super::shop::MAX_TEMPLATE_ID as u32).contains(&id)
        && quick_item_allowed(id)
        && entered
            .carry
            .get(source)
            .is_some_and(|item| u32::from(item.index) == id)
    {
        let pane = hud
            .input_pane_mut(target.panel)
            .expect("resolved HUD target");

        let item = Item {
            index: id as u16,
            ..Default::default()
        };

        let contents = ui::icon::set_icon(&item, &panels.items, &panels.icons, &pane.ui.resource);

        let element = ui::input::retained_element_mut(&mut pane.ui.panel, target.owner)
            .expect("resolved receiver");

        if let Some(button) = element.set_icon_contents(contents)
            && contents.is_none()
        {
            button.set_tooltip_binding(Some(ui::icon::TooltipBinding {
                kind: 1,
                index: id as i32,
                item,
                help_id: 0,
            }));
        }

        let (ids, cache_slot) = hud.recount_quick(id, &entered.carry, &panels.items);

        quick_bar.ids = ids;

        if let Some(slot) = cache_slot {
            quick_bar.write(slot, id);
        }
    }

    hud.manager.cancel_carry();

    true
}

pub fn quick_item_allowed(id: u32) -> bool {
    if id == 0xC87 {
        return false;
    }

    const RANGES: [std::ops::RangeInclusive<u32>; 10] = [
        0x190..=0x199,
        0x19F..=0x1A0,
        0x1AC..=0x1B3,
        0x286..=0x287,
        0x2A8..=0x2B3,
        0xCEE..=0xCF0,
        0xCF7..=0xCFB,
        0xD28..=0xD31,
        0xD37..=0xD38,
        0xC80..=0xC8A,
    ];

    const EXACT: [u32; 9] = [
        0xD67, 0x1031, 0x6CB, 0xD95, 0xD90, 0x1001, 0xD89, 0x986, 0x1740,
    ];

    RANGES.iter().any(|range| range.contains(&id)) || EXACT.contains(&id)
}

fn quick_bind(
    hud: &mut Hud,
    panels: &Panels,
    entered: &mut Entered,
    target: Target,
    id: i32,
) -> bool {
    let Some(state) = hud
        .input_pane(target.panel)
        .and_then(|p| ui::input::retained_element(&p.ui.panel, target.owner))
        .and_then(|e| e.widget.button())
    else {
        return false;
    };

    if state.cell_group != cell_group::HUD_SKILL {
        return false;
    }

    let Ok(slot) = usize::try_from(state.icon_slot_index) else {
        return false;
    };

    if slot >= hud::SKILL_CELLS {
        return false;
    }

    if !formats::data::skill_data::is_skill_book(id) {
        return false;
    }

    let pane = hud
        .input_pane_mut(target.panel)
        .expect("resolved HUD target");

    let element = ui::input::retained_element_mut(&mut pane.ui.panel, target.owner)
        .expect("resolved receiver");

    bind_skill_receiver(element, panels, entered, slot, id)
}

fn bind_skill(hud: &mut Hud, panels: &Panels, entered: &mut Entered, slot: usize, id: i32) -> bool {
    let Some(element) = hud.skill_receiver_mut(slot) else {
        return false;
    };

    bind_skill_receiver(element, panels, entered, slot, id)
}

fn bind_skill_receiver(
    element: &mut ui::Element,
    panels: &Panels,
    entered: &mut Entered,
    slot: usize,
    id: i32,
) -> bool {
    if !(1..=super::shop::MAX_TEMPLATE_ID).contains(&id) {
        return false;
    }

    let row = formats::data::skill_data::skill_index(id);

    let passive = usize::try_from(row)
        .ok()
        .and_then(|row| panels.skills.as_ref()?.get(row))
        .is_some_and(|skill| skill.is_passive());

    if passive {
        return false;
    }

    let contents = ui::icon::set_skill_icon(id, &panels.items);

    element.set_icon_contents(contents);

    if let Some(icon) = panels
        .items
        .get(id as usize)
        .map(|item| item.mesh_sub as u8)
    {
        entered.quick[slot] = icon;
    }

    true
}

pub fn bind_hovered_skill(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    ch: char,
) -> bool {
    let Some(local) = super::skill_bar::symbol_slot(ch) else {
        return false;
    };

    if !panels.is_open(CONTENT_SKILL) {
        return false;
    }

    let Some(index) = panels
        .screens
        .iter()
        .position(|screen| screen.content == CONTENT_SKILL)
    else {
        return false;
    };

    let Some(target) = hud.manager.hovered else {
        return false;
    };

    if target.panel != input_slot(index) {
        return false;
    }

    let Some(id) = hovered_skill_icon_id(panels, index, Some(target.owner)) else {
        return false;
    };

    let Ok(page) = usize::try_from(hud.page) else {
        return false;
    };

    let Some(slot) = page
        .checked_mul(super::skill_bar::PAGE_SLOTS)
        .and_then(|n| n.checked_add(local))
    else {
        return false;
    };

    if slot >= hud::SKILL_CELLS || !bind_skill(hud, panels, entered, slot, id) {
        return false;
    }

    panels
        .wire
        .push(protocol::client::quick_bar(&entered.quick, entered.class));

    panels.notice.sounds.push(super::shop::SALE_SOUND);

    hud.refresh_skills(&entered.quick, &panels.items);

    true
}

pub fn hover(hud: &Hud, panels: &mut Panels, entered: &Entered, shift: bool) {
    retire_tooltip_element(panels, Some(hud));

    refresh_icon_hover(hud, panels, entered);

    let moved_to =
        hovered_element(hud, panels).map(|(target, element, ..)| (target, element.instance));

    let transitioned = panels.hover_source != moved_to;

    panels.hover_source = moved_to;

    if !transitioned {
        if !shift && panels.tooltip.visible() && panels.tooltip.raised_by.is_none() {
            panels.tooltip.hide();
        }

        return;
    }

    enter_hover(hud, panels, entered, shift, None);
}

fn tooltip_source<'a>(
    hud: &'a Hud,
    panels: &'a Panels,
    forced: Option<Target>,
) -> Option<(Target, &'a ui::Element, [i32; 4], (i32, i32), i32)> {
    let Some(target) = forced else {
        return hovered_element(hud, panels);
    };

    let (panel, origin, z) = panel_at(hud, panels, target.panel)?;

    let element = ui::input::retained_element(panel, target.owner)?;

    let rect = ui::input::control_geometry(panel, target.owner)
        .map(|(_, r)| r)
        .or_else(|| {
            panels
                .icon_hover_rect
                .filter(|(node, _)| *node == element.instance)
                .map(|(_, r)| r)
        })?;

    Some((target, element, rect, origin, z))
}

fn enter_hover(
    hud: &Hud,
    panels: &mut Panels,
    entered: &Entered,
    shift: bool,
    forced: Option<Target>,
) {
    panels.icon_hover_rect =
        tooltip_source(hud, panels, forced).map(|(_, e, rect, ..)| (e.instance, rect));

    panels.icon_hover_stamp = tooltip_source(hud, panels, forced)
        .and_then(|(_, e, ..)| e.widget.button().map(|b| (e.instance, b.icon_update())));

    if let Some(link) = hovered_item_link(hud, panels, forced) {
        let [_, _, sw, sh] = panels.layout.virtual_screen();

        let shop = shop_state(panels);

        panels.hover_tip.hide();

        let zone_clock = panels.zone_clock_for(Some(link.1), entered.client_id, shift);

        let sealed = panels.sealed_for(Some(link.1), entered.client_id, shift);

        let hosts = tooltip::Hosts {
            expiry: Some((
                &panels.client_config,
                panels.tooltip_calendar,
                panels.tooltip_portuguese,
            )),
            items: &panels.items,
            strings: &panels.strings,
            entered,
            shop,
            zone_clock,
            sealed,
        };

        panels
            .tooltip
            .update_item_link(link, &hosts, &panels.mix_help, (sw, sh), shift);

        return;
    }

    let balloon = hovered_balloon(hud, panels, forced);

    let found = if balloon.is_some() {
        None
    } else {
        hovered_cell(hud, panels, forced)
    };

    let pet_card = found.is_some()
        && tooltip_source(hud, panels, forced)
            .and_then(|(_, e, ..)| e.widget.button()?.tooltip_binding())
            .filter(|b| b.kind == 7)
            .and_then(|b| {
                usize::try_from(b.index)
                    .ok()
                    .and_then(|id| panels.items.get(id))
            })
            .is_some_and(|entry| entry.item_type == 22);

    let [_, _, sw, sh] = panels.layout.virtual_screen();

    let screen = (sw, sh);

    let bound = balloon
        .is_none()
        .then(|| tooltip_source(hud, panels, forced))
        .flatten()
        .filter(|(_, e, ..)| {
            e.element_type == ElementType::IconButton
                && e.tooltip.is_empty()
                && e.widget.button().is_some_and(|b| {
                    b.icon_tooltip_enabled
                        && b.tooltip_binding()
                            .is_none_or(|c| c.kind != 4 || (0..=54).contains(&c.index))
                })
        })
        .map(|(target, ..)| target);

    if balloon.is_none()
        && let Some((target, cell, id)) = hovered_mix_help(hud, panels, forced)
    {
        panels.hover_tip.hide();

        panels.tooltip.update_mix_help(
            (target, cell, id),
            &panels.mix_help,
            &panels.strings,
            screen,
            shift,
        );

        return;
    }

    if balloon.is_none()
        && let Some((target, cell, id)) = hovered_mission(hud, panels, forced)
        && let Some(missions) = &panels.missions
    {
        panels.hover_tip.hide();

        panels
            .tooltip
            .update_mission((target, cell, id), missions, &panels.strings, screen, shift);

        return;
    }

    match balloon {
        Some(shown) => {
            panels.hover_tip.show(
                shown.target,
                &shown.text,
                shown.source,
                panels.layout.virtual_screen(),
                shown.offset,
            );
        }
        None => panels.hover_tip.hide(),
    }

    let shop = shop_state(panels);

    let zone_clock = panels.zone_clock_for(
        found.filter(|_| !pet_card).map(|(_, c)| c),
        entered.client_id,
        shift,
    );

    let sealed = panels.sealed_for(
        found.filter(|_| !pet_card).map(|(_, c)| c),
        entered.client_id,
        shift,
    );

    let Panels {
        tooltip,
        items,
        strings,
        ..
    } = panels;

    let hosts = tooltip::Hosts {
        expiry: Some((
            &panels.client_config,
            panels.tooltip_calendar,
            panels.tooltip_portuguese,
        )),
        items,
        strings,
        entered,
        shop,
        zone_clock,
        sealed,
    };

    if pet_card && let Some(hovered) = found {
        tooltip.update_pet_card(hovered, &hosts, screen, shift);
    } else {
        tooltip.update(found, &hosts, screen, shift);
    }

    if !tooltip.visible() {
        tooltip.raised_by = bound;
    }
}

pub fn refresh_icon_hover(hud: &Hud, panels: &mut Panels, entered: &Entered) {
    let Some(mut target) = panels.tooltip.raised_by else {
        return;
    };

    let Some((panel, ..)) = panel_at(hud, panels, target.panel) else {
        return;
    };

    if let Some((node, _)) = panels.icon_hover_stamp {
        if ui::input::retained_element(panel, target.owner).is_none_or(|e| e.instance != node) {
            let Some(owner) = panel.elements.iter().enumerate().find_map(|(element, _)| {
                let owner = Owner::Descendant {
                    element,
                    node,
                    item: None,
                };

                ui::input::retained_element(panel, owner).map(|_| owner)
            }) else {
                return;
            };

            target.owner = owner;
        }
    }

    let Some(e) = ui::input::retained_element(panel, target.owner) else {
        return;
    };

    let Some(button) = e
        .widget
        .button()
        .filter(|_| e.element_type == ElementType::IconButton)
    else {
        return;
    };

    let stamp = (e.instance, button.icon_update());

    let changed = panels
        .icon_hover_stamp
        .is_some_and(|old| old.0 == stamp.0 && old.1 != stamp.1);

    if let Some((_, rect)) = ui::input::control_geometry(panel, target.owner) {
        panels.icon_hover_rect = Some((stamp.0, rect));
    }

    panels.tooltip.raised_by = Some(target);

    if panels.hover_source.is_some_and(|(_, node)| node == stamp.0) {
        panels.hover_source = Some((target, stamp.0));
    }

    panels.icon_hover_stamp = Some(stamp);

    if changed && tooltip_source(hud, panels, Some(target)).is_some() {
        enter_hover(hud, panels, entered, hud.manager.keys.shift, Some(target));
    }
}

pub fn refresh_combat_hover(hud: &Hud, panels: &mut Panels) {
    let Some(target) = panels.hover_tip.raised_by else {
        return;
    };

    if target.panel != BAR_PANEL {
        return;
    }

    let Owner::Element(owner) = target.owner else {
        return;
    };

    let Some(element) = hud.bar.ui.panel.elements.get(owner) else {
        return;
    };

    if !hud.owns_combat_tooltip(element.instance) {
        return;
    }

    if element.tooltip.is_empty() {
        panels.hover_tip.hide();

        return;
    }

    let source = [
        hud.bar.origin.0 + element.x,
        hud.bar.origin.1 + element.y,
        hud.bar.origin.0 + element.x + element.width,
        hud.bar.origin.1 + element.y + element.height,
    ];

    panels.hover_tip.show(
        target,
        &element.tooltip,
        source,
        panels.layout.virtual_screen(),
        Some(hover_tip::HUD_OFFSET),
    );
}

struct BalloonHover {
    target: Target,
    text: String,
    source: [i32; 4],
    offset: Option<(i16, i16)>,
}

fn hovered_item_link(
    hud: &Hud,
    panels: &Panels,
    forced: Option<Target>,
) -> Option<(Target, tooltip::Cell, i32)> {
    let (target, element, rect, origin, z) = tooltip_source(hud, panels, forced)?;

    hovered_balloon_text(hud, element)?;

    let suffix = element.tooltip_key.strip_prefix("im_")?;

    if suffix.is_empty() {
        return None;
    }

    let id = formats::ui::script::parse_integer_prefix_or_zero(suffix);

    let item = formats::net::account::Item {
        index: if (1..14500).contains(&id) {
            id as u16
        } else {
            0
        },
        ..Default::default()
    };

    Some((
        target,
        tooltip::Cell {
            index: id,
            item,
            rect,
            owner_origin: origin,
            owner_z: z,
            owner_content: owner_content(panels, target.panel),
            asking_price: -1,
        },
        id,
    ))
}

fn has_balloon_hover(kind: ElementType) -> bool {
    matches!(
        kind,
        ElementType::Static
            | ElementType::Button
            | ElementType::CheckBox
            | ElementType::RadioButton
            | ElementType::ComboBox
            | ElementType::GaugeBar
            | ElementType::BalloonBox
            | ElementType::TooltipBox
            | ElementType::IconButton
            | ElementType::Image
    )
}

fn hovered_balloon_text(hud: &Hud, element: &ui::Element) -> Option<String> {
    if !has_balloon_hover(element.element_type) {
        return None;
    }

    if element.element_type == ElementType::IconButton
        && let Some(contents) = element
            .widget
            .button()?
            .icon_contents
            .filter(|c| c.kind == ui::icon::AFFECT_KIND)
    {
        if !(0..=54).contains(&contents.index) {
            return None;
        }

        hud.affect_tooltip_for(element)
    } else {
        (!element.tooltip.is_empty()).then(|| element.tooltip.clone())
    }
}

fn hovered_balloon(hud: &Hud, panels: &Panels, forced: Option<Target>) -> Option<BalloonHover> {
    let (target, element, rect, origin, _) = tooltip_source(hud, panels, forced)?;

    let text = hovered_balloon_text(hud, element)?;

    let source = [
        origin.0 + rect[0],
        origin.1 + rect[1],
        origin.0 + rect[2],
        origin.1 + rect[3],
    ];

    let explicit = target.panel == 0 && hud.owns_offset_tooltip(element.instance);

    Some(BalloonHover {
        target,
        text,
        source,
        offset: explicit.then_some(hover_tip::HUD_OFFSET),
    })
}

fn shop_state(panels: &Panels) -> item_tip::Shop {
    item_tip::Shop {
        open: panels.is_open(item_tip::OWNER_ITEM_SHOP as PanelId),
        in_game: true,
        ..panels.shop_prices
    }
}

fn hovered_cell(
    hud: &Hud,
    panels: &Panels,
    forced: Option<Target>,
) -> Option<(Target, tooltip::Cell)> {
    let (target, element, rect, origin, z_order) = tooltip_source(hud, panels, forced)?;

    if element.element_type != ElementType::IconButton {
        return None;
    }

    if !element.tooltip.is_empty() {
        return None;
    }

    let button = element.widget.button()?;

    if !button.icon_tooltip_enabled {
        return None;
    }

    let binding = button.tooltip_binding()?;

    if binding.kind == ui::icon::AFFECT_KIND {
        return None;
    }

    if !matches!(binding.kind, 1 | 2 | 3 | 7) {
        return None;
    }

    let invalid = match binding.kind {
        2 => binding.index >= 14500,
        _ => matches!(binding.index, 0 | -1),
    };

    if invalid {
        return None;
    }

    Some((
        target,
        tooltip::Cell {
            index: binding.index,
            item: binding.item,
            rect,
            owner_origin: origin,
            owner_z: z_order,
            owner_content: owner_content(panels, target.panel),
            asking_price: if binding.kind == 2 {
                -1
            } else {
                button.icon_contents.map_or(-1, |c| c.asking_price)
            },
        },
    ))
}

fn hovered_mission(
    hud: &Hud,
    panels: &Panels,
    forced: Option<Target>,
) -> Option<(Target, tooltip::Cell, i32)> {
    let (target, element, rect, origin, z) = tooltip_source(hud, panels, forced)?;

    if !element.tooltip.is_empty() {
        return None;
    }

    let id = mission_tooltip_id(element)?;

    let button = element.widget.button()?;

    Some((
        target,
        tooltip::Cell {
            index: id,
            item: button.tooltip_binding()?.item,
            rect,
            owner_origin: origin,
            owner_z: z,
            owner_content: owner_content(panels, target.panel),
            asking_price: -1,
        },
        id,
    ))
}

fn mission_tooltip_id(element: &ui::Element) -> Option<i32> {
    if element.element_type != ElementType::IconButton {
        return None;
    }

    let button = element.widget.button()?;

    if !button.icon_tooltip_enabled {
        return None;
    }

    let binding = button.tooltip_binding()?;

    if binding.kind != 5 {
        return None;
    }

    let id = binding.index;

    (1..5000).contains(&id).then_some(id)
}

fn mix_help_id(element: &ui::Element) -> Option<usize> {
    if element.element_type != ElementType::IconButton
        || !element.visible
        || !element.tooltip.is_empty()
    {
        return None;
    }

    let button = element.widget.button()?;

    if !button.icon_tooltip_enabled {
        return None;
    }

    let binding = button.tooltip_binding()?;

    (binding.kind == 2 && binding.index == 0 && binding.help_id != 0)
        .then_some(binding.help_id as usize)
}

fn hovered_mix_help(
    hud: &Hud,
    panels: &Panels,
    forced: Option<Target>,
) -> Option<(Target, tooltip::Cell, usize)> {
    let (target, element, rect, origin, z) = tooltip_source(hud, panels, forced)?;

    let id = mix_help_id(element)?;

    Some((
        target,
        tooltip::Cell {
            index: id as i32,
            item: Default::default(),
            rect,
            owner_origin: origin,
            owner_z: z,
            owner_content: owner_content(panels, target.panel),
            asking_price: -1,
        },
        id,
    ))
}

fn hovered_element<'a>(
    hud: &'a Hud,
    panels: &'a Panels,
) -> Option<(Target, &'a ui::Element, [i32; 4], (i32, i32), i32)> {
    let retained = hud.manager.hovered?;

    let (panel, origin, z_order) = panel_at(hud, panels, retained.panel)?;

    let refined = hud
        .manager
        .interaction_for(retained.panel, panel, origin)
        .hovered?;

    if matches!(refined, Owner::Descendant { item: None, .. }) {
        let (element, rect) = ui::input::owner_element(panel, refined)?;

        if !element.msg_receive
            || (!ui::input::in_rect(
                rect,
                hud.manager.cursor.0.saturating_sub(origin.0),
                hud.manager.cursor.1.saturating_sub(origin.1),
            ) && hud.manager.carried.is_none())
        {
            return None;
        }

        return Some((
            Target {
                owner: refined,
                ..retained
            },
            element,
            rect,
            origin,
            z_order,
        ));
    }

    let index = match retained.owner {
        Owner::Element(index)
        | Owner::ListRow {
            element: index,
            child: Some(_),
            ..
        } => index,
        _ => return None,
    };

    let (panel, origin, z_order) = panel_at(hud, panels, retained.panel)?;

    if !panel.visible {
        return None;
    }

    let parent = panel.elements.get(index)?;

    if !parent.visible
        || (panel.page_index != -1
            && parent.page_index != 0xff
            && panel.page_index != i32::from(parent.page_index))
    {
        return None;
    }

    let local = (
        hud.manager.cursor.0 - origin.0,
        hud.manager.cursor.1 - origin.1,
    );

    let retained_child = if let Owner::ListRow {
        row,
        child: Some(child),
        ..
    } = retained.owner
    {
        let element = parent.widget.list()?.items.get(row)?.children.get(child)?;

        if !parent.enabled || !element.visible || !element.enabled || !element.msg_receive {
            return None;
        }

        let rect = if hud.manager.carried.is_none() {
            let (hit_row, hit_child, rect) = ui::input::list_child_at(parent, local.0, local.1)?;

            if (hit_row, hit_child) != (row, child) {
                return None;
            }

            rect
        } else {
            let (_, r) = ui::input::container_rows(parent)
                .into_iter()
                .find(|(i, _)| *i == row)?;

            [
                r[0] + element.x,
                r[1] + element.y,
                r[0] + element.x + element.width,
                r[1] + element.y + element.height,
            ]
        };

        Some((retained, element, rect))
    } else {
        None
    };

    let refined = (retained_child.is_none() && hud.manager.carried.is_none())
        .then(|| ui::input::list_child_at(parent, local.0, local.1))
        .flatten();

    let (target, element, rect) = if let Some(child) = retained_child {
        child
    } else if let Some((row, child, rect)) = refined {
        let element = parent.widget.list()?.items.get(row)?.children.get(child)?;

        (
            Target {
                panel: retained.panel,
                owner: Owner::ListRow {
                    element: index,
                    row,
                    child: Some(child),
                },
            },
            element,
            rect,
        )
    } else {
        (retained, parent, parent.rect())
    };

    if hud.manager.carried.is_none() {
        if !ui::input::in_rect(rect, local.0, local.1) {
            return None;
        }
    }

    Some((target, element, rect, origin, z_order))
}

fn owner_content(panels: &Panels, slot: usize) -> Option<i32> {
    if slot < 3 {
        return Some(0x10 + slot as i32);
    }

    slot.checked_sub(HUD_PANELS)
        .and_then(|i| panels.screens.get(i))
        .map(|s| s.content as i32)
}

pub fn cell_at(panels: &Panels, content: PanelId, cell: &str) -> Result<(usize, usize)> {
    let index = panels
        .screens
        .iter()
        .position(|s| s.content == content)
        .with_context(|| format!("content {content:#04x} is not built"))?;

    let elements = &panels.screens[index].placed.ui.panel.elements;

    let icon = |e: &ui::Element| e.element_type == ElementType::IconButton;

    let element = match cell.parse::<usize>() {
        Ok(nth) => elements
            .iter()
            .enumerate()
            .filter(|(_, e)| icon(e))
            .nth(nth)
            .map(|(i, _)| i)
            .with_context(|| {
                let cells = elements.iter().filter(|e| icon(e)).count();

                format!(
                    "{} has {cells} icon cells, not {}",
                    screen_name(content),
                    nth + 1
                )
            })?,
        Err(_) => elements
            .iter()
            .position(|e| e.name == cell && icon(e))
            .with_context(|| format!("{} has no icon cell named {cell}", screen_name(content)))?,
    };

    Ok((index, element))
}

pub fn cell_center(panels: &Panels, content: PanelId, cell: &str) -> Result<(i32, i32)> {
    let (index, element) = cell_at(panels, content, cell)?;

    let screen = &panels.screens[index];

    let e = &screen.placed.ui.panel.elements[element];

    Ok((
        screen.placed.origin.0 + e.x + e.width / 2,
        screen.placed.origin.1 + e.y + e.height / 2,
    ))
}

pub fn hover_at(
    panels: &mut Panels,
    entered: &Entered,
    content: PanelId,
    cell: &str,
) -> Result<i32> {
    let (index, element) = cell_at(panels, content, cell)?;

    let target = Target {
        panel: input_slot(index),
        owner: Owner::Element(element),
    };

    let screen = &panels.screens[index];

    let e = &screen.placed.ui.panel.elements[element];

    panels.icon_hover_stamp = e.widget.button().map(|b| (e.instance, b.icon_update()));

    panels.icon_hover_rect = Some((e.instance, e.rect()));

    let pet_card = e
        .widget
        .button()
        .and_then(|b| b.tooltip_binding())
        .filter(|b| b.kind == 7)
        .and_then(|b| {
            usize::try_from(b.index)
                .ok()
                .and_then(|id| panels.items.get(id))
        })
        .is_some_and(|entry| entry.item_type == 22);

    if let Some(id) = mix_help_id(e) {
        let cell = tooltip::Cell {
            index: id as i32,
            item: Default::default(),
            rect: e.rect(),
            owner_origin: screen.placed.origin,
            owner_z: screen.z_order(),
            owner_content: Some(content as i32),
            asking_price: -1,
        };

        let [_, _, w, h] = panels.layout.virtual_screen();

        panels.tooltip.update_mix_help(
            (target, cell, id),
            &panels.mix_help,
            &panels.strings,
            (w, h),
            false,
        );

        return Ok(id as i32);
    }

    if e.visible
        && let Some(id) = mission_tooltip_id(e)
    {
        let cell = tooltip::Cell {
            index: id,
            item: e
                .widget
                .button()
                .and_then(|b| b.icon_contents)
                .map_or(Default::default(), |c| c.item),
            rect: e.rect(),
            owner_origin: screen.placed.origin,
            owner_z: screen.z_order(),
            owner_content: Some(content as i32),
            asking_price: -1,
        };

        let missions = panels
            .missions
            .as_ref()
            .context("mission tables are unavailable")?;

        let [_, _, w, h] = panels.layout.virtual_screen();

        panels
            .tooltip
            .update_mission((target, cell, id), missions, &panels.strings, (w, h), false);

        return Ok(id);
    }

    let found = {
        let e = &screen.placed.ui.panel.elements[element];

        let button = e.widget.button();

        let contents = button
            .and_then(|b| b.tooltip_binding())
            .with_context(|| format!("{}'s {} cell is empty", screen_name(content), e.name))?;

        tooltip::Cell {
            index: contents.index,
            item: contents.item,
            rect: [e.x, e.y, e.x + e.width, e.y + e.height],
            owner_origin: screen.placed.origin,
            owner_z: screen.z_order(),
            owner_content: Some(content as i32),
            asking_price: if contents.kind == 2 {
                -1
            } else {
                button
                    .and_then(|b| b.icon_contents)
                    .map_or(-1, |c| c.asking_price)
            },
        }
    };

    let size = (panels.layout.width as i32, panels.layout.height as i32);

    let shop = shop_state(panels);

    let zone_clock = panels.zone_clock_for((!pet_card).then_some(found), entered.client_id, false);

    let sealed = panels.sealed_for((!pet_card).then_some(found), entered.client_id, false);

    let Panels {
        tooltip,
        items,
        strings,
        ..
    } = panels;

    let hosts = tooltip::Hosts {
        expiry: Some((
            &panels.client_config,
            panels.tooltip_calendar,
            panels.tooltip_portuguese,
        )),
        items,
        strings,
        entered,
        shop,
        zone_clock,
        sealed,
    };

    if pet_card {
        tooltip.update_pet_card((target, found), &hosts, size, false);
    } else {
        tooltip.update(Some((target, found)), &hosts, size, false);
    }

    Ok(found.index)
}

pub fn refresh_inventory_hover(panels: &mut Panels, entered: &Entered, slot: i32) -> bool {
    if slot <= 0 {
        return false;
    }

    let Ok((index, element)) = cell_at(panels, CONTENT_INVENTORY, &format!("Inven{slot}")) else {
        return false;
    };

    let target = Target {
        panel: input_slot(index),
        owner: Owner::Element(element),
    };

    if panels.tooltip.raised_by != Some(target) {
        return false;
    }

    let screen = &panels.screens[index];

    if !screen.open {
        return false;
    }

    let found = {
        let e = &screen.placed.ui.panel.elements[element];

        e.widget
            .button()
            .and_then(|button| button.icon_contents)
            .map(|contents| tooltip::Cell {
                index: contents.index,
                item: contents.item,
                rect: [e.x, e.y, e.x + e.width, e.y + e.height],
                owner_origin: screen.placed.origin,
                owner_z: screen.z_order(),
                owner_content: Some(CONTENT_INVENTORY as i32),
                asking_price: contents.asking_price,
            })
    };

    let Some(found) = found else {
        return false;
    };

    let size = (panels.layout.width as i32, panels.layout.height as i32);

    let shop = shop_state(panels);

    let zone_clock = panels.zone_clock_for(Some(found), entered.client_id, false);

    let sealed = panels.sealed_for(Some(found), entered.client_id, false);

    let Panels {
        tooltip,
        items,
        strings,
        ..
    } = panels;

    let hosts = tooltip::Hosts {
        expiry: Some((
            &panels.client_config,
            panels.tooltip_calendar,
            panels.tooltip_portuguese,
        )),
        items,
        strings,
        entered,
        shop,
        zone_clock,
        sealed,
    };

    tooltip.update(Some((target, found)), &hosts, size, false);

    true
}

fn panel_at<'a>(
    hud: &'a Hud,
    panels: &'a Panels,
    slot: usize,
) -> Option<(&'a ui::Panel, (i32, i32), i32)> {
    if slot < HUD_PANELS {
        let placed = [&hud.bar, &hud.plate, &hud.target][slot];

        return Some((&placed.ui.panel, placed.origin, placed.ui.panel.z_order));
    }

    let screen = panels.screens.get(slot - HUD_PANELS)?;

    Some((
        &screen.placed.ui.panel,
        screen.placed.origin,
        screen.z_order(),
    ))
}

pub fn walk_order(hud: &Hud, panels: &Panels) -> Vec<usize> {
    let mut z: Vec<(usize, i32, bool, bool)> =
        Vec::with_capacity(HUD_PANELS + panels.screens.len());

    for (slot, panel) in [&hud.bar, &hud.plate, &hud.target].into_iter().enumerate() {
        z.push((slot, panel.ui.panel.z_order, false, false));
    }

    for (index, screen) in panels.screens.iter().enumerate() {
        z.push((
            input_slot(index),
            screen.z_order(),
            true,
            screen.page_of.is_none(),
        ));
    }

    z.sort_by_key(|&(_, order, is_screen, is_ordinary_screen)| {
        (order, is_screen, is_ordinary_screen)
    });

    z.into_iter().map(|(slot, _, _, _)| slot).collect()
}

pub fn field_draw_order(hud: &Hud, panels: &Panels) -> Vec<usize> {
    let mut order = walk_order(hud, panels);

    order.reverse();

    order
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Toggle(PanelId),
    SetOpen(PanelId, bool),
    HideFirecracker,
    AirMove(u32),
    SkillWindow,
    SkillPage,
    MapCycle,
    PkToggle,
    AutoAttack,
    Potion(super::consume::Which),
    FeedMount,
    Escape,
    Logout,
    Quit,
    ServerSelect,
    MoveChannel(i32),
    MoveServerVisibility(bool),
    StorePrompt,
    OpenStore,
    CoinInput(bool),
    SplitIndividual,
    StoreExhibition,
    CoinMoney(PanelId),
    CloseStore,
    TradeReady,
    CloseTrade,
    PartyAutoJoin(bool),
    PartyExit,
    PartyMember(usize),
    PartyCast(usize),
    RefineAuto(bool),
    GuildChat,
    OpenNotice,
    OpenCommunity,
    OpenSettings,
    HudInventory,
    Unbuilt(PanelId),
    Unmodelled(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct Hotkey {
    pub arm: &'static str,
    pub chars: &'static [char],
    pub action: Action,
}

pub const HOTKEYS: [Hotkey; 16] = [
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0001", "toggle PvP"),
        chars: &['k', 'K', '`', '~'],
        action: Action::PkToggle,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0002", "toggle help"),
        chars: &['h', 'H'],
        action: Action::Toggle(CONTENT_HELP),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0003", "toggle guild"),
        chars: &['o', 'O'],
        action: Action::Toggle(CONTENT_GUILD),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0004", "toggle collection"),
        chars: &['l', 'L'],
        action: Action::Toggle(CONTENT_COLLECTION),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0005", "toggle mount"),
        chars: &['r', 'R'],
        action: Action::Unmodelled(crate::internal_reason!(
            "NWYD-H0006",
            "mount up or dismount"
        )),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0007", "feed mount"),
        chars: &super::consume::FEED_KEYS,
        action: Action::FeedMount,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0008", "use health potion"),
        chars: &super::consume::HP_POTION_KEYS,
        action: Action::Potion(super::consume::Which::Hp),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0009", "use mana potion"),
        chars: &super::consume::MP_POTION_KEYS,
        action: Action::Potion(super::consume::Which::Mp),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0010", "auto attack"),
        chars: &['y', 'Y'],
        action: Action::AutoAttack,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0011", "toggle quests"),
        chars: &['x', 'X'],
        action: Action::Toggle(CONTENT_QUEST),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0012", "guild chat"),
        chars: &['\'', '"'],
        action: Action::GuildChat,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0013", "skill window"),
        chars: &['s', 'S'],
        action: Action::SkillWindow,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0014", "toggle inventory"),
        chars: &['i', 'I', 'g', 'G'],
        action: Action::Toggle(CONTENT_INVENTORY),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0015", "toggle character"),
        chars: &['c', 'C'],
        action: Action::Toggle(CONTENT_CHARACTER),
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0016", "cycle map"),
        chars: &['m', 'M'],
        action: Action::MapCycle,
    },
    Hotkey {
        arm: crate::internal_reason!("NWYD-H0017", "skill page"),
        chars: &['z', 'Z'],
        action: Action::SkillPage,
    },
];

pub const PARTY_HOTKEY: Hotkey = Hotkey {
    arm: crate::internal_reason!("NWYD-H0018", "toggle party"),
    chars: &['p', 'P'],
    action: Action::Toggle(CONTENT_PARTY),
};

pub const BUSY_CHARACTER: [PanelId; 3] = [CONTENT_STORE, 0x1D, CONTENT_ITEM_SHOP];

pub const BUSY_SKILL: [PanelId; 2] = [CONTENT_STORE, CONTENT_ITEM_SHOP];

pub const BUSY_INVENTORY: [PanelId; 1] = [CONTENT_STORE];

pub fn hotkey(panels: &Panels, ch: char) -> Option<Action> {
    let busy = |ids: &[PanelId]| ids.iter().any(|&id| panels.is_open(id));

    for key in HOTKEYS.iter().chain(std::iter::once(&PARTY_HOTKEY)) {
        if !key.chars.contains(&ch) {
            continue;
        }

        return match key.action {
            Action::Toggle(CONTENT_CHARACTER)
                if busy(&BUSY_CHARACTER) && !panels.is_open(CONTENT_CHARACTER) =>
            {
                None
            }
            Action::SkillWindow if busy(&BUSY_SKILL) => None,
            Action::Toggle(CONTENT_INVENTORY) if busy(&BUSY_INVENTORY) => None,
            action => Some(action),
        };
    }

    None
}

pub const ESCAPE_ORDER: [PanelId; 18] = [
    CONTENT_MSGBOX,
    CONTENT_STORE,
    CONTENT_PGT_MENU,
    CONTENT_COIN_INPUT,
    super::merge_item::CONTENT,
    CONTENT_CHARACTER,
    CONTENT_QUEST,
    CONTENT_ITEM_SHOP,
    CONTENT_CARGO,
    CONTENT_PORTAL,
    CONTENT_TRADE,
    CONTENT_SKILL,
    CONTENT_HELP,
    CONTENT_INVENTORY,
    CONTENT_ARENA,
    CONTENT_PARTY,
    CONTENT_MOVE_SERVER,
    CONTENT_SYSTEM,
];

pub const CLOSES_THE_INVENTORY: [PanelId; 2] = [CONTENT_ITEM_SHOP, CONTENT_CARGO];

pub const SKILL_CLEARS: [PanelId; 2] = [CONTENT_ITEM_SHOP, CONTENT_CARGO];

pub fn escape(panels: &Panels) -> Action {
    for &content in &ESCAPE_ORDER {
        if content == CONTENT_SYSTEM {
            break;
        }

        if panels.is_open(content) {
            if content == CONTENT_STORE {
                return Action::CloseStore;
            }

            if content == CONTENT_TRADE {
                return Action::CloseTrade;
            }

            if matches!(
                content,
                CONTENT_PGT_MENU
                    | CONTENT_COIN_INPUT
                    | CONTENT_CHARACTER
                    | CONTENT_QUEST
                    | CONTENT_SKILL
                    | CONTENT_HELP
                    | CONTENT_ARENA
                    | CONTENT_MOVE_SERVER
                    | CONTENT_INVENTORY
            ) {
                return Action::Toggle(content);
            }

            return Action::SetOpen(content, false);
        }
    }

    Action::Toggle(CONTENT_SYSTEM)
}

pub fn handle(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    at: spend::Raise,
    move_server_input: bool,
) -> Vec<Action> {
    handle_with_pgt_player(panels, entered, events, at, move_server_input, 0, false)
}

pub fn handle_with_pgt_model(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    at: spend::Raise,
    move_server_input: bool,
    pgt_player_model: u8,
) -> Vec<Action> {
    handle_with_pgt_player(
        panels,
        entered,
        events,
        at,
        move_server_input,
        pgt_player_model,
        false,
    )
}

pub fn handle_with_pgt_player(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    at: spend::Raise,
    move_server_input: bool,
    pgt_player_model: u8,
    pgt_player_dead: bool,
) -> Vec<Action> {
    let target_present = panels.pgt_target.is_some();

    let context = super::pgt::Context::snapshot(
        panels.pgt_target.as_ref(),
        pgt_player_model,
        pgt_player_dead,
        target_present,
        None,
        0xFFFF,
    );

    handle_with_pgt_scene(panels, entered, events, at, move_server_input, &context)
}

pub fn handle_with_pgt_scene(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    at: spend::Raise,
    move_server_input: bool,
    pgt_context: &super::pgt::Context,
) -> Vec<Action> {
    let mut out = Vec::new();

    for event in events {
        out.extend(handle_primary(
            panels,
            entered,
            std::slice::from_ref(event),
            at,
            move_server_input,
            pgt_context,
        ));

        finish_popup_event(panels, event);
    }

    out
}

pub fn handle_with_manager(
    hud: &mut Hud,
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    move_server_input: bool,
    context: &super::pgt::Context,
    input: FieldInput,
) -> (Vec<Action>, bool) {
    let at = spend::Raise {
        dead: input.player_dead,
        absent: !input.player_present,
    };

    let mut out = Vec::new();

    let mut picked_up = false;

    for event in events {
        let one = std::slice::from_ref(event);

        out.extend(handle_primary(
            panels,
            entered,
            one,
            at,
            move_server_input,
            context,
        ));

        picked_up |= pick_up_in_field(hud, panels, entered, one, input);
    }

    (out, picked_up)
}

fn handle_primary(
    panels: &mut Panels,
    entered: &mut Entered,
    events: &[Event],
    at: spend::Raise,
    move_server_input: bool,
    pgt_context: &super::pgt::Context,
) -> Vec<Action> {
    let mut out = Vec::new();

    for event in events {
        let Some(index) = event.panel.checked_sub(HUD_PANELS) else {
            continue;
        };

        let Some(screen) = panels.screens.get(index) else {
            continue;
        };

        let content = screen.content;

        if matches!(content, CONTENT_MOVE_SERVER | CONTENT_SYSTEM) && !move_server_input {
            continue;
        }

        match event.id.command_id() {
            EventId::LeftDown if content == chat::CONTENT_CHAT_MENU => {
                let strings = panels.strings.clone();

                chat::button_down(panels, content, &event.name, &strings);
            }
            EventId::LeftDown if content == CONTENT_CHARACTER => {
                let command = (0..4)
                    .flat_map(|i| [spend::STAT_BUTTONS[i], spend::MASTERY_BUTTONS[i]])
                    .find(|name| {
                        screen
                            .sheet_element(name, ElementType::Button)
                            .is_some_and(|e| e.name == event.name)
                    });

                if let Some(command) = command
                    && let Some(result) = spend::press(panels, entered, command, at)
                    && let Some(message) = spend::describe(&result, command)
                {
                    tracing::info!("{message}");
                }
            }
            EventId::Click if content == CONTENT_CHARACTER => {
                if event.owner == Some(Owner::Close) {
                    out.push(Action::SetOpen(content, false));
                } else if let Some(Owner::Element(owner)) = event.owner
                    && let Some(element) = screen.placed.ui.panel.elements.get(owner)
                    && let Some(page) = screens::SHEET_TABS
                        .iter()
                        .position(|name| screen.sheet_widgets.get(*name) == Some(&element.instance))
                    && screen.page != page as i32
                {
                    let screen = &mut panels.screens[index];

                    screens::sheet_page(screen, page as i32);

                    if pgt_context.player_present {
                        screens::sheet_refresh_current(screen, &panels.strings, entered);

                        panels.sheet_live_pending = true;
                    }
                }
            }
            EventId::Click if content == CONTENT_SKILL_MASTER => {
                if let Some(owner) = event.owner {
                    skill_master::clicked_owner(panels, owner);
                }
            }
            EventId::Click
                if content == CONTENT_MOVE_SERVER && event.owner == Some(Owner::Close) =>
            {
                if pgt_context.player_present {
                    out.push(Action::SetOpen(content, false));
                }
            }
            EventId::Click if content == CONTENT_TRADE && event.owner == Some(Owner::Close) => {
                out.push(Action::CloseTrade);
            }
            EventId::Click if content == super::mix::CONTENT => {
                if event.owner == Some(Owner::Close) {
                    out.push(Action::SetOpen(content, false));
                }
            }
            EventId::Click
                if matches!(content, CONTENT_ITEM_MIX4 | CONTENT_OPTION_STONE)
                    && event.owner == Some(Owner::Close) =>
            {
                out.push(Action::SetOpen(content, false));
            }
            EventId::Click if content == CONTENT_OPTION_STONE => {
                if !screen.open || event.name == "close" {
                    continue;
                }

                if event.name == super::combine::COMBINE_BUTTON {
                    panels.raise_box(
                        &super::combine::confirm(super::combine::Kind::OptionStone),
                        None,
                    );
                } else if let Some(Owner::Element(owner)) = event.owner
                    && let Some(element) = screen.placed.ui.panel.elements.get(owner)
                    && let Some(cell) = screen
                        .combine_widgets
                        .materials
                        .iter()
                        .position(|bound| *bound == Some(element.instance))
                {
                    option_stone_remove(panels, cell);
                }
            }
            EventId::Click
                if event.owner == Some(Owner::Close)
                    && matches!(
                        content,
                        CONTENT_GUILD
                            | super::guild_panel::NOTICE_EDIT
                            | super::guild_panel::CREATE
                            | super::guild_panel::MEMBER_POPUP
                    ) =>
            {
                let actions = guild_command(
                    panels,
                    entered,
                    content,
                    &event.name,
                    pgt_context.player_present,
                );

                guild_carry_out(panels, &actions);

                panels.guild_close_callback(content);
            }
            EventId::Click if content == CONTENT_QUEST => {
                if !pgt_context.player_present {
                    continue;
                }

                if event.name == "Close" {
                    out.push(Action::Toggle(content));
                } else if event.owner == Some(Owner::Close) {
                    out.push(Action::SetOpen(content, false));
                } else if let Some(Owner::Element(owner)) = event.owner
                    && let Some(element) = screen.placed.ui.panel.elements.get(owner)
                    && let Some(tab) = screens::QUEST_TABS.iter().position(|name| {
                        screen
                            .element(name, ElementType::Button)
                            .is_some_and(|bound| std::ptr::eq(bound, element))
                    })
                {
                    panels.request_quest_refresh();

                    let quests = panels.quests.clone();

                    screens::quest_page(&mut panels.screens[index], tab as i32, &quests, entered);
                }
            }
            EventId::Resized if content == chat::CONTENT_CHAT => chat::resized(panels, index),
            EventId::Click
                if content == CONTENT_COLLECTION
                    && matches!(event.owner, Some(Owner::ListRow { child: Some(_), .. })) =>
            {
                super::collection::item_clicked(panels, event);
            }
            EventId::Click | EventId::RightUp if content == CONTENT_PARTY => {
                if event.id.command_id() == EventId::Click
                    && event.name.eq_ignore_ascii_case("Exit")
                {
                    out.push(Action::PartyExit);
                } else if event.name.eq_ignore_ascii_case("SlotBtn")
                    && let Some(Owner::ListRow {
                        element,
                        row,
                        child: Some(child),
                    }) = event.owner
                    && let Some(list) = screen.placed.ui.panel.elements.get(element)
                    && screen
                        .element("List", ElementType::ListView)
                        .is_some_and(|bound| std::ptr::eq(bound, list))
                    && let Some(item) = list.widget.list().and_then(|s| s.items.get(row))
                    && item.children.get(child).is_some_and(|e| {
                        e.name.eq_ignore_ascii_case("SlotBtn")
                            && e.element_type == ElementType::Button
                    })
                {
                    out.push(if event.id.command_id() == EventId::Click {
                        Action::PartyMember(row)
                    } else {
                        Action::PartyCast(row)
                    });
                }
            }
            EventId::Click => out.extend(command_at_with_pgt_scene(
                panels,
                entered,
                content,
                &event.name,
                at,
                pgt_context,
            )),
            EventId::CheckChanged if content == CONTENT_PARTY => {
                let Some(Owner::Element(owner)) = event.owner else {
                    continue;
                };

                let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
                    continue;
                };

                if screen
                    .element("AutoJoin", ElementType::CheckBox)
                    .is_some_and(|bound| std::ptr::eq(bound, element))
                    && event.name.eq_ignore_ascii_case("AutoJoin")
                {
                    out.push(Action::PartyAutoJoin(event.extra != 0));
                }
            }
            EventId::CheckChanged if content == CONTENT_REFINE => {
                let Some(Owner::Element(owner)) = event.owner else {
                    continue;
                };

                let Some(element) = screen.placed.ui.panel.elements.get(owner) else {
                    continue;
                };

                if element.element_type == ElementType::CheckBox
                    && element.name.eq_ignore_ascii_case("Auto")
                    && event.name.eq_ignore_ascii_case("Auto")
                {
                    out.push(Action::RefineAuto(event.extra != 0));
                }
            }
            EventId::ListSelect
                if matches!(content, CONTENT_MOVE_SERVER | CONTENT_PORTAL)
                    && !pgt_context.player_present => {}
            EventId::ListSelect => out.extend(selected(panels, content, event)),
            EventId::TreeRightSelect if content == super::guild_panel::PAGE_MEMBER => {
                guild_member_right_select(panels, event)
            }
            _ => {}
        }
    }

    out
}

fn finish_popup_event(panels: &mut Panels, event: &Event) {
    if let Some(index) = event.panel.checked_sub(HUD_PANELS) {
        finish_popup_notification(panels, index, event.id);
    }
}

pub fn finish_popup_events(panels: &mut Panels, events: &[Event]) {
    for event in events {
        finish_popup_event(panels, event);
    }
}

fn finish_popup_notification(panels: &mut Panels, index: usize, id: EventId) {
    let Some(screen) = panels.screens.get_mut(index) else {
        return;
    };

    if screen.placed.ui.panel.popup
        && !screen.placed.ui.panel.keep_popup_on_click
        && matches!(id, EventId::Click | EventId::RightUp)
    {
        screen.set_open(false);
    }
}

fn guild_member_right_select(panels: &mut Panels, event: &Event) {
    use super::guild_panel::{MEMBER_LIST, PAGE_MEMBER};

    if event.name != MEMBER_LIST {
        return;
    }

    let Ok(row) = usize::try_from(event.extra) else {
        return;
    };

    let Some(list) = panels
        .screen_mut(PAGE_MEMBER)
        .and_then(Screen::guild_member_list_mut)
    else {
        return;
    };

    let Some(payload) = list
        .widget
        .list()
        .and_then(|s| s.items.get(row))
        .map(|r| r.payload)
    else {
        return;
    };

    let Ok(member) = usize::try_from(payload) else {
        return;
    };

    if member
        >= panels
            .guild
            .members
            .iter()
            .filter(|m| m.char_uid != 0)
            .count()
    {
        return;
    }

    panels.guild_row_clicked(member);

    if let Some(list) = panels
        .screen_mut(PAGE_MEMBER)
        .and_then(Screen::guild_member_list_mut)
    {
        let rect = list.rect();

        if let Some(state) = list.widget.list_mut() {
            state.select(row as i32, rect);
        }
    }
}

fn selected(panels: &mut Panels, content: PanelId, event: &Event) -> Vec<Action> {
    if content == CONTENT_PORTAL {
        if !event.name.is_empty() {
            return Vec::new();
        }

        if !matches!(panels.portal.staged, Some(item_move::Request::Use { .. })) {
            return vec![Action::SetOpen(content, false)];
        }

        panels.portal.destination = (event.extra as u16).wrapping_add(1);

        return Vec::new();
    }

    if content == CONTENT_COLLECTION
        && event
            .name
            .eq_ignore_ascii_case(super::collection::LIST_VIEW)
    {
        super::collection::row_selected(panels);

        return Vec::new();
    }

    if content == CONTENT_MOVE_SERVER && event.name == super::listener::CHANNEL_VIEW {
        let Ok(row) = usize::try_from(event.extra) else {
            return Vec::new();
        };

        let Some(screen) = panels.screen(content) else {
            return Vec::new();
        };

        return super::screens::server_popup_channel(screen, row)
            .map(Action::MoveChannel)
            .into_iter()
            .collect();
    }

    if content != CONTENT_QUEST || event.name != "TitleList" {
        return Vec::new();
    }

    panels.request_quest_refresh();

    let Ok(row) = usize::try_from(event.extra) else {
        return Vec::new();
    };

    let quests = panels.quests.clone();

    if let Some(screen) = panels.screen_mut(content) {
        screens::quest_select(screen, row, &quests);
    }

    Vec::new()
}

pub fn command(
    panels: &mut Panels,
    entered: &mut Entered,
    content: PanelId,
    name: &str,
) -> Vec<Action> {
    command_at(panels, entered, content, name, spend::Raise::default())
}

pub fn command_at(
    panels: &mut Panels,
    entered: &mut Entered,
    content: PanelId,
    name: &str,
    at: spend::Raise,
) -> Vec<Action> {
    command_at_with_pgt_player(panels, entered, content, name, at, 0, false)
}

fn command_at_with_pgt_player(
    panels: &mut Panels,
    entered: &mut Entered,
    content: PanelId,
    name: &str,
    at: spend::Raise,
    pgt_player_model: u8,
    pgt_player_dead: bool,
) -> Vec<Action> {
    let target_present = panels.pgt_target.is_some();

    let context = super::pgt::Context::snapshot(
        panels.pgt_target.as_ref(),
        pgt_player_model,
        pgt_player_dead,
        target_present,
        None,
        0xFFFF,
    );

    command_at_with_pgt_scene(panels, entered, content, name, at, &context)
}

fn command_at_with_pgt_scene(
    panels: &mut Panels,
    entered: &mut Entered,
    content: PanelId,
    name: &str,
    at: spend::Raise,
    pgt_context: &super::pgt::Context,
) -> Vec<Action> {
    if content == CONTENT_QUEST && !name.eq_ignore_ascii_case("close") {
        panels.request_quest_refresh();
    }

    if content == super::merge_item::CONTENT && !matches!(name, "close" | "Close") {
        if name == "ConfirmBtn" {
            super::merge_item::command(panels, entered, name);
        }

        return Vec::new();
    }

    if content == CONTENT_INVENTORY
        && name != "close"
        && (!pgt_context.player_present || !panels.is_open(content))
    {
        return Vec::new();
    }

    if matches!(
        content,
        CONTENT_SYSTEM
            | CONTENT_ARENA
            | CONTENT_HELP
            | CONTENT_RPS
            | CONTENT_QUEST
            | CONTENT_MOVE_SERVER
            | CONTENT_PORTAL
            | CONTENT_STORE
    ) && !pgt_context.player_present
    {
        return Vec::new();
    }

    if content == CONTENT_SYSTEM {
        return system_menu(name);
    }

    if matches!(content, CONTENT_ARENA | CONTENT_HELP) && name == "Close" {
        return vec![Action::Toggle(content)];
    }

    if content == CONTENT_QUEST && name == "Close" {
        return vec![Action::Toggle(content)];
    }

    if content == CONTENT_MOVE_SERVER {
        return match name {
            "ServerSel" => vec![Action::ServerSelect],
            "Close" => vec![Action::Toggle(content)],
            _ => Vec::new(),
        };
    }

    if content == chat::CONTENT_CHAT || content == chat::CONTENT_CHAT_MENU {
        let (client_id, strings) = (entered.client_id, panels.strings.clone());

        return chat::command(panels, content, name, client_id, &strings);
    }

    if content == CONTENT_PORTAL {
        return portal_command(panels, entered, name);
    }

    if content == super::rps::CONTENT {
        let client_id = entered.client_id;

        return super::rps::command(panels, client_id, name);
    }

    if let Some(combine_kind) = super::combine::Staging::kind_of(content)
        && combine_kind != super::combine::Kind::Mix
    {
        if let Some(actions) = combine_command(panels, combine_kind, name) {
            return actions;
        }

        if name.eq_ignore_ascii_case("close") {
            return vec![Action::SetOpen(content, false)];
        }
    }

    if content == super::collection::CONTENT
        && let Some(command) = super::collection::command(panels.collection_filter, name)
    {
        match command {
            super::collection::Command::Filter(filter) => {
                panels.collection_filter = filter;

                panels.collection_cell = 0;

                panels.collection_icon = None;

                panels.collection_signed_up = false;

                super::collection::check_filter(panels);

                panels.collection_dirty = true;
            }
            super::collection::Command::Nothing => {}
            super::collection::Command::Ask => {
                panels.raise_box(&super::collection::confirm(), None);
            }
        }

        return Vec::new();
    }

    if content == super::refine::CONTENT && !name.eq_ignore_ascii_case("close") {
        if !panels.is_open(content) {
            return Vec::new();
        }

        if matches!(name, "AutoDelayDown" | "AutoDelayUp") {
            panels.refine.auto_delay.step(name == "AutoDelayUp");

            let delay = panels.refine.auto_delay.milliseconds();

            if let Some(screen) = panels.screen_mut(content) {
                screens::refine_delay(screen, delay);
            }

            return Vec::new();
        }

        let max = panels.refine_max();

        let actions =
            super::refine::command(&panels.refine, name, max, panels.refine_command_locked);

        refine_carry_out(panels, &actions);

        return Vec::new();
    }

    if content == super::guild_panel::CONTENT
        || content == super::guild_panel::PAGE_INFO
        || content == super::guild_panel::NOTICE_EDIT
        || content == super::guild_panel::CREATE
        || content == super::guild_panel::MEMBER_POPUP
    {
        let actions = guild_command(panels, entered, content, name, pgt_context.player_present);

        guild_carry_out(panels, &actions);

        return Vec::new();
    }

    if content == super::pgt::CONTENT {
        let (player, target) = pgt_parties(
            panels,
            entered,
            pgt_context.player_model,
            pgt_context.player_dead,
        );

        let actions = super::pgt::command_with_context(
            &player,
            target.as_ref(),
            pgt_context,
            entered.gold,
            name,
        );

        super::pgt::carry_out(panels, &actions);

        return Vec::new();
    }

    if content == CONTENT_ARENA {
        let strings = panels.strings.clone();

        let cost = panels.arena_cost;

        if let super::arena::Clicked::Asked(request) =
            super::arena::command(Some(&strings), cost, name)
        {
            panels.raise_box(&request, None);

            return Vec::new();
        }
    }

    if content == super::minipopup::CONTENT {
        return super::minipopup::command(panels, name);
    }

    if content == CONTENT_STORE {
        return match name {
            STORE_CLOSE_BUTTON => vec![Action::CloseStore],
            STORE_OPEN_BUTTON => vec![Action::OpenStore],
            _ => Vec::new(),
        };
    }

    if content == CONTENT_COIN_INPUT {
        if !panels.is_open(content) {
            return Vec::new();
        }

        return match name {
            "ok" => vec![Action::CoinInput(true)],
            "cancel" => vec![Action::CoinInput(false)],
            "DivChk" if panels.coin_action == 12 => vec![Action::SplitIndividual],
            "ExpoChk" if panels.coin_action == 4 => vec![Action::StoreExhibition],
            _ => Vec::new(),
        };
    }

    if matches!(content, CONTENT_INVENTORY | CONTENT_CARGO) && name == "MoneyBtn" {
        return vec![Action::CoinMoney(content)];
    }

    if content == super::trade::CONTENT {
        if pgt_context.player_present && name == TRADE_READY_BUTTON {
            return vec![Action::TradeReady];
        }

        return Vec::new();
    }

    if name.eq_ignore_ascii_case("close") {
        return match panels.screen(content) {
            Some(_) => vec![Action::SetOpen(content, false)],
            None => Vec::new(),
        };
    }

    if content == CONTENT_MSGBOX && (name == "ok" || name == "cancel") {
        return vec![Action::SetOpen(CONTENT_MSGBOX, false)];
    }

    if content == CONTENT_SKILL_MASTER
        && name
            .get(..8)
            .is_some_and(|head| head.eq_ignore_ascii_case("SkillBtn"))
    {
        if let Some(pending) = skill_master::clicked(panels, name) {
            tracing::info!(
                "NWYD: learn item {} from entity {}?",
                pending.item,
                pending.npc
            );
        }

        return Vec::new();
    }

    if content == CONTENT_CHARACTER
        && let Some(spent) = spend::command(panels, entered, name, at)
    {
        if let Some(line) = spend::describe(&spent, name) {
            tracing::info!("{line}");
        }

        return Vec::new();
    }

    if content == CONTENT_HELP {
        let help = panels.help.clone();

        let memo = panels.memo.clone();

        let Some(screen) = panels.screen_mut(content) else {
            return Vec::new();
        };

        return match screens::help_command(screen, name, &help, &memo) {
            Some(screens::HelpCommand::Notice) => {
                vec![Action::Toggle(CONTENT_HELP), Action::OpenNotice]
            }
            Some(screens::HelpCommand::Refilled) | None => Vec::new(),
        };
    }

    if content == CONTENT_INVENTORY {
        let Panels {
            screens,
            items,
            icons,
            strings,
            ..
        } = panels;

        if let Some(screen) = screens.iter_mut().find(|s| s.content == content) {
            let page = screen.page;

            if screens::inventory_command(screen, name) && page != screen.page {
                screens::inventory_bag_refresh(screen, entered, items, icons, strings);
            }
        }

        return Vec::new();
    }

    if content == CONTENT_CARGO {
        let Panels {
            screens,
            items,
            icons,
            ..
        } = panels;

        if let Some(screen) = screens.iter_mut().find(|s| s.content == content) {
            let page = screen.page;

            if screens::cargo_command(screen, name)
                && page != screen.page
                && screen.open
                && pgt_context.player_present
            {
                screens::cargo_items_refresh(screen, entered, items, icons);
            }
        }

        return Vec::new();
    }

    let quests = panels.quests.clone();

    let Some(screen) = panels.screen_mut(content) else {
        return Vec::new();
    };

    let claimed = match content {
        CONTENT_CHARACTER => screens::sheet_command(screen, name),
        CONTENT_QUEST => screens::quest_command(screen, name, &quests, entered),
        _ => false,
    };

    let _ = claimed;

    Vec::new()
}

pub fn portal_command(panels: &mut Panels, entered: &mut Entered, name: &str) -> Vec<Action> {
    if name == portal::CLOSE {
        panels.portal.staged = None;

        panels.portal.page = None;

        panels.portal.destination = 0;

        return vec![Action::SetOpen(CONTENT_PORTAL, false)];
    }

    if name != portal::OK {
        return Vec::new();
    }

    let Some(row) = panels
        .screen(CONTENT_PORTAL)
        .and_then(screens::portal_selected)
    else {
        return Vec::new();
    };

    if panels.portal.mode != Some(portal::Mode::AirMove) {
        if panels.portal.destination == 0 {
            let payload = panels
                .screen(CONTENT_PORTAL)
                .and_then(|s| s.element(portal::LIST, ElementType::ListView))
                .and_then(|e| e.widget.list())
                .and_then(|s| s.items.get(row as usize))
                .map(|item| item.payload);

            panels.portal.destination = if panels.portal.page.is_some() {
                payload.unwrap_or(i64::from(row) + 1) as u16
            } else {
                (row as u16).wrapping_add(1)
            };
        }

        if let Some(item_move::Request::Use { slot, .. }) = panels.portal.staged {
            let cell = Coord {
                container: item_move::container::INVENTORY,
                slot,
            };

            if item_move::item_at(entered, cell).is_none_or(|item| item.is_empty()) {
                return Vec::new();
            }
        }

        let staged = panels.portal.staged.take();

        panels.portal.page = None;

        let destination = std::mem::take(&mut panels.portal.destination);

        if let Some(item_move::Request::Use { slot, .. }) = staged {
            let items = panels.items.clone();

            let cell = Coord {
                container: item_move::container::INVENTORY,
                slot,
            };

            item_use::spend(entered, &items, cell);

            refresh_inventory_hover(panels, entered, cell.slot);

            refine_item_changed(panels, entered);

            panels.staged.push(item_move::Request::Warp {
                slot,
                at: panels.live.position,
                row: destination,
            });

            panels.outbox.push(item_move::Request::Drink);

            tracing::info!("NWYD: the hunting scroll picked row {row} — five seconds to the warp");
        }

        return vec![Action::SetOpen(CONTENT_PORTAL, false)];
    }

    vec![Action::AirMove(row), Action::SetOpen(CONTENT_PORTAL, false)]
}

pub fn system_menu(name: &str) -> Vec<Action> {
    match name {
        "Settings" => vec![Action::OpenSettings, Action::SetOpen(CONTENT_SYSTEM, false)],
        "ServerSel" => {
            vec![
                Action::Toggle(CONTENT_MOVE_SERVER),
                Action::Toggle(CONTENT_SYSTEM),
            ]
        }
        "CharLogout" => vec![Action::Logout],
        "Exit" => vec![Action::Quit],
        "Close" => vec![Action::Toggle(CONTENT_SYSTEM)],
        _ => Vec::new(),
    }
}

pub fn apply(panels: &mut Panels, actions: &[Action]) -> Vec<Action> {
    let mut deferred = Vec::new();

    for action in actions {
        match *action {
            Action::OpenNotice => panels.notice_requested = true,
            Action::OpenCommunity => panels.community_requested = true,
            Action::OpenSettings => panels.settings_requested = true,
            Action::Toggle(content) => {
                let before = panels.is_open(content);

                if content == CONTENT_INVENTORY && !before {
                    panels.pair_inventory(0, false);
                }

                match panels.toggle(content) {
                    Some(open) => {
                        if content == CONTENT_MOVE_SERVER && open != before {
                            deferred.push(Action::MoveServerVisibility(open));
                        }
                    }
                    None => deferred.push(Action::Unbuilt(content)),
                }
            }
            Action::SetOpen(content, open) => {
                let before = panels.is_open(content);

                if content == CONTENT_PORTAL
                    && !open
                    && panels.portal.mode == Some(portal::Mode::AirMove)
                    && panels.screen(CONTENT_PORTAL).is_some()
                    && !panels.is_open(CONTENT_STORE)
                {
                    deferred.push(Action::HideFirecracker);
                }

                let visibility = if !open && CLOSES_THE_INVENTORY.contains(&content) {
                    panels.close_by_toggle(content)
                } else {
                    panels.set_open(content, open)
                };

                match visibility {
                    Some(open) => {
                        if content == CONTENT_MOVE_SERVER && open != before {
                            deferred.push(Action::MoveServerVisibility(open));
                        }
                    }
                    None => deferred.push(Action::Unbuilt(content)),
                }

                if !open && before && CLOSES_THE_INVENTORY.contains(&content) {
                    panels.close_by_toggle(CONTENT_INVENTORY);
                }

                if !open && content == CONTENT_PORTAL {
                    panels.portal.mode = None;

                    panels.portal.staged = None;

                    panels.portal.page = None;

                    panels.portal.destination = 0;
                }
            }
            Action::SkillWindow => {
                if !panels.is_open(CONTENT_SKILL) {
                    for &content in &SKILL_CLEARS {
                        if panels.is_open(content) {
                            deferred.extend(apply(panels, &[Action::SetOpen(content, false)]));
                        }
                    }
                } else if panels.is_open(CONTENT_SKILL_MASTER) {
                    deferred.extend(apply(panels, &[Action::Toggle(CONTENT_SKILL_MASTER)]));
                }

                if panels.toggle(CONTENT_SKILL).is_none() {
                    deferred.push(Action::Unbuilt(CONTENT_SKILL));
                }
            }
            Action::Escape => {
                let next = escape(panels);

                deferred.extend(apply(panels, &[next]));
            }
            Action::StorePrompt => {
                panels.raise_box(&super::store::prompt(), None);
            }
            Action::RefineAuto(checked) => refine_auto_carry_out(panels, checked),
            other => deferred.push(other),
        }
    }

    deferred
}

pub fn refresh_guild_notice_limit(screen: &mut Screen) {
    use super::guild_panel as guild;

    if screen.content != guild::NOTICE_EDIT {
        return;
    }

    let Some(edit) = screen
        .element(guild::NOTICE_EDIT_FIELD, ElementType::ImeNotePad)
        .and_then(|element| element.widget.edit())
    else {
        return;
    };

    let enabled = edit.text.len() < 1022;

    if let Some(button) = screen.element_mut(guild::NOTICE_OK, ElementType::Button) {
        button.enabled = enabled;
    }
}

fn guild_command(
    panels: &mut Panels,
    entered: &Entered,
    content: PanelId,
    name: &str,
    player_present: bool,
) -> Vec<super::guild_panel::Action> {
    use super::guild_panel as guild;

    match content {
        guild::CONTENT => {
            if guild::TAB_CAPTIONS
                .iter()
                .any(|tab| name.eq_ignore_ascii_case(tab))
            {
                panels.hide_box_contexts(0x3F, 0x44);
            }

            guild::tab_click(&panels.guild, name)
        }
        guild::PAGE_INFO => guild::info_click(name),
        guild::NOTICE_EDIT => {
            if !name.eq_ignore_ascii_case(guild::NOTICE_OK) {
                return guild::notice_click(name, "");
            }

            if let Some(screen) = panels.screen_mut(guild::PAGE_INFO) {
                let resource = screen.placed.ui.resource.clone();

                screen.placed.ui.panel.element_or_create(
                    guild::NOTICE,
                    ElementType::ImeNotePad,
                    &resource,
                );
            }

            let text = panels
                .screen_mut(guild::NOTICE_EDIT)
                .and_then(|screen| {
                    let resource = screen.placed.ui.resource.clone();

                    screen
                        .placed
                        .ui
                        .panel
                        .element_or_create(
                            guild::NOTICE_EDIT_FIELD,
                            ElementType::ImeNotePad,
                            &resource,
                        )
                        .widget
                        .edit()
                        .map(|edit| edit.text.clone())
                })
                .unwrap_or_default();

            guild::notice_click(name, &text)
        }
        guild::CREATE => {
            if name.eq_ignore_ascii_case(guild::CREATE_OK) {
                panels.guild_refresh_session();
            }

            let typed = if name.eq_ignore_ascii_case(guild::CREATE_OK)
                && !panels.guild.session.in_a_guild()
            {
                panels
                    .screen_mut(guild::CREATE)
                    .and_then(|screen| {
                        let resource = screen.placed.ui.resource.clone();

                        screen
                            .placed
                            .ui
                            .panel
                            .element_or_create(
                                guild::CREATE_FIELD,
                                ElementType::ImeEditBox,
                                &resource,
                            )
                            .widget
                            .edit()
                            .map(|edit| edit.text.clone())
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let bytes = formats::text::codepage::encode(&typed, CODEPAGE);

            let reserved = panels.reserved.clone();

            guild::create_click(&panels.guild, name, &bytes, CODEPAGE, &reserved)
        }
        guild::MEMBER_POPUP => {
            panels.hide_box_contexts(0x40, 0x44);

            if !player_present {
                return Vec::new();
            }

            if name == guild::POPUP_BUTTON || name == format!("{}0", guild::POPUP_BUTTON) {
                return vec![
                    guild::Action::RefetchAfter(guild::Fetch::Members, 500),
                    guild::Action::Repaint,
                ];
            }

            let Some(entry) = guild_entry_of(name) else {
                return Vec::new();
            };

            guild::entry_click(&panels.guild, entry, entered.client_id)
        }
        _ => Vec::new(),
    }
}

const CODEPAGE: formats::text::Codepage = formats::text::Codepage::Windows1252;

pub fn guild_entry_of(name: &str) -> Option<super::guild_panel::Entry> {
    let suffix = name.strip_prefix(super::guild_panel::POPUP_BUTTON)?;

    super::guild_panel::Entry::from_tag(suffix.parse::<u32>().ok()?)
}

fn guild_carry_out(panels: &mut Panels, actions: &[super::guild_panel::Action]) {
    use super::guild_panel::Action;

    for action in actions {
        match action {
            Action::Repaint => panels.guild_refresh_selected(),
            Action::Send(frame) => panels.wire.push(frame.clone()),
            Action::Ask(request) => {
                panels.raise_box(request, None);
            }
            Action::AskForMember {
                context,
                record,
                name,
            } => {
                let name = name.split('\0').next().unwrap_or("");

                let bytes = formats::text::codepage::encode(name, CODEPAGE);

                let name = formats::text::codepage::decode(&bytes, CODEPAGE);

                let template = panels.strings.translate(&format!("ss_{record}"));

                let text = super::chat::format_tokens(&template, &[&name]);

                let request = super::guild_panel::box_request(*context, msgbox::Body::Text(text));

                panels.raise_box(&request, None);
            }
            Action::AskWithEditLimit(request, limit) => {
                panels.raise_box_with_edit_limit(request, *limit);
            }
            Action::Toast(strdef) => {
                refine_toast(panels, *strdef);
            }
            Action::TimedNotice(strdef, lifetime_ms) => {
                let request = msgbox::Request {
                    caption: Some(237),
                    kind: 0,
                    context: 2,
                    body: msgbox::Body::StrDef(*strdef),
                    flags: 0x20000001,
                };

                if let Some(slot) = panels.raise_box(&request, None) {
                    let armed = panels
                        .boxes
                        .arm_notice_timeout(slot, panels.now_ms, *lifetime_ms);

                    debug_assert!(armed, "global notices have no answer listener");
                }
            }
            Action::Go(page) => {
                panels.guild.page = *page;

                panels.select_guild_tab();

                let (info, member) = (
                    *page == super::guild_panel::Page::Info,
                    *page == super::guild_panel::Page::Member,
                );

                panels.set_open(super::guild_panel::PAGE_INFO, info);

                panels.set_open(super::guild_panel::PAGE_MEMBER, member);
            }
            Action::Show(content) => {
                let fee = panels.items.get(509).map_or(0, |item| item.price);

                if *content == super::guild_panel::CREATE
                    && let Some(screen) = panels.screen_mut(*content)
                {
                    let resource = screen.placed.ui.resource.clone();

                    let magnitude = screens::thousands(u64::from(fee.unsigned_abs()));

                    let cost = if fee < 0 {
                        format!("-{magnitude}")
                    } else {
                        magnitude
                    };

                    screen
                        .placed
                        .ui
                        .panel
                        .element_or_create("msg", ElementType::Static, &resource)
                        .set_display(&cost);

                    let element = screen.placed.ui.panel.element_or_create(
                        super::guild_panel::CREATE_FIELD,
                        ElementType::ImeEditBox,
                        &resource,
                    );

                    element.set_display("");

                    let edit = element.widget.edit_mut().unwrap();

                    edit.text.clear();

                    edit.preedit = None;

                    edit.caret = 0;

                    edit.anchor = 0;

                    edit.scroll = 0;
                }

                if *content == super::guild_panel::NOTICE_EDIT
                    && let Some(screen) = panels.screen_mut(*content)
                {
                    let resource = screen.placed.ui.resource.clone();

                    let element = screen.placed.ui.panel.element_or_create(
                        super::guild_panel::NOTICE_EDIT_FIELD,
                        ElementType::ImeNotePad,
                        &resource,
                    );

                    element.set_display("");
                }

                if let Some(screen) = panels.screen_mut(*content) {
                    refresh_guild_notice_limit(screen);
                }

                if let Some(screen) = panels.screen_mut(*content) {
                    screen.set_open(true);
                }
            }
            Action::Hide(content) => {
                if let Some(screen) = panels.screen_mut(*content) {
                    screen.set_open(false);
                }
            }
            Action::Park(name) => panels.guild.pending_name = Some(name.clone()),
            Action::Post(_)
            | Action::Refetch(_)
            | Action::RefetchAfter(_, _)
            | Action::UpdateConn(_) => {
                panels.queue_guild_request(action.clone());
            }
        }
    }
}

impl Panels {
    pub(crate) fn guild_refresh_session(&mut self) {
        if let Some(session) = self.guild_live_session {
            self.guild.session = session;
        }
    }

    pub fn guild_update_conn(&mut self, report: super::guild_panel::ConnectionReport) {
        self.queue_guild_request(super::guild_panel::Action::UpdateConn(report));
    }

    pub fn queue_guild_request(&mut self, action: super::guild_panel::Action) {
        use super::guild_panel::{Action, Fetch, Post};

        if matches!(
            action,
            Action::Refetch(Fetch::Info)
                | Action::RefetchAfter(Fetch::Info, _)
                | Action::UpdateConn(_)
        ) {
            self.guild_refresh_session();
        }

        let joined = self.guild.session.in_a_guild();

        if matches!(
            action,
            Action::Refetch(_)
                | Action::RefetchAfter(_, _)
                | Action::UpdateConn(_)
                | Action::Post(Post::Notice(_))
        ) && !joined
        {
            return;
        }

        if matches!(action, Action::Post(Post::Create(_))) && joined {
            return;
        }

        let guild = self.guild.session.guild;

        let ally = self.guild.session.ally;

        let charidx = self.guild.session.charidx.unwrap_or(0);

        let delay_ms = match action {
            Action::RefetchAfter(_, ms) => ms,
            _ => 0,
        };

        let (endpoint, mut body) = match action {
            Action::Refetch(Fetch::Info) | Action::RefetchAfter(Fetch::Info, _) => (
                Fetch::Info.endpoint(),
                format!("guildidx={guild}&ally={ally}&war1=0&war2=0&war3=0&war4=0"),
            ),
            Action::Refetch(Fetch::Members) | Action::RefetchAfter(Fetch::Members, _) => {
                (Fetch::Members.endpoint(), format!("guildidx={guild}"))
            }
            Action::Post(ref post) => {
                let text = post.text();

                let body = match post {
                    Post::Notice(_) => {
                        format!(
                            "guildidx={guild}&charidx=0&guildnotice={}",
                            super::guild_request::form_value(text)
                        )
                    }
                    Post::MemberNotice(_) => {
                        format!(
                            "charidx={charidx}&MemberNotice={}",
                            super::guild_request::form_value(text)
                        )
                    }
                    Post::Create(_) => format!("guildname={text}"),
                };

                (post.endpoint(), body)
            }
            Action::UpdateConn(report) => {
                (formats::net::guild_web::Endpoint::UpdateConn, report.body())
            }
            _ => return,
        };

        if matches!(action, Action::Refetch(_) | Action::RefetchAfter(_, _)) {
            body.push_str(&self.guild_identity.suffix(&mut self.guild_request_rng));
        }

        self.guild_proxy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .prepare(endpoint, &mut body);

        let mut request = super::guild_http::Request::new(
            self.guild_host.as_deref().unwrap_or(""),
            endpoint,
            body,
        );

        request.delay_ms = delay_ms;

        self.guild_requests
            .push(super::guild_request::Queued { action, request });
    }

    pub fn guild_pump(&mut self, entered: &Entered) {
        if self.guild_host.is_none()
            && self.guild_ring.is_none()
            && !self
                .guild_requests
                .iter()
                .any(|q| q.request.url.starts_with("http://"))
        {
            self.guild_requests.clear();

            return;
        }

        if self.guild_ring.is_none() {
            self.guild_ring = Some(super::guild_http::Ring::spawn(
                super::guild_http::CONNECT_TIMEOUT,
            ));
        }

        for queued in std::mem::take(&mut self.guild_requests) {
            if let Some(ring) = self.guild_ring.as_mut() {
                let _ = ring.push(queued.request);
            }
        }

        while let Some(answer) = self.guild_ring.as_mut().and_then(|r| r.poll()) {
            if answer.state != super::guild_http::State::Done {
                continue;
            }

            use formats::net::guild_web as web;

            match answer.tag {
                web::TAG_INFO => {
                    self.guild_info_arrived(&answer.body);
                }
                web::TAG_MEMBERS => {
                    self.guild_members_arrived(&answer.body);
                }
                web::TAG_NAME_CHECK => {
                    if let Some(check) = web::name_check(&answer.body) {
                        self.guild_name_checked(entered, &check);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn guild_name_checked(
        &mut self,
        entered: &Entered,
        answer: &formats::net::guild_web::NameCheck,
    ) {
        let mut answer = answer.clone();

        answer.result = self
            .guild_proxy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .name_result(answer.result, self.now_ms);

        let actions = super::guild_panel::name_checked(&self.guild, &answer, entered.client_id);

        self.guild.pending_name = None;

        guild_carry_out(self, &actions);
    }
}

impl Panels {
    pub fn refine_max(&self) -> i32 {
        item_use::refine_ceiling(&self.items, &self.refine.result)
    }
}

fn refine_carry_out(panels: &mut Panels, actions: &[super::refine::Action]) {
    use super::refine::Action;

    let mut refresh = false;

    let mut halted = false;

    for action in actions {
        match action {
            Action::Send(frame) => panels.wire.push(frame.clone()),
            Action::Toast(strdef) => refine_toast(panels, *strdef),
            Action::Target(level) => {
                panels
                    .refine
                    .preview_target(panels.refine.result, *level, &panels.items);

                refresh = true;
            }
            Action::Halt => {
                panels.refine.running = false;

                panels.refine.auto = false;

                halted = true;
            }
            Action::Manual => panels.refine.manual_requested = true,
        }
    }

    let strings = panels.strings.clone();

    if let Some(screen) = panels.screen_mut(super::refine::CONTENT)
        && halted
    {
        screens::refine_auto_mirror(screen, false);

        screens::refine_controls(screen, false, true, &strings);

        screens::refine_pulse_halt(screen);
    }

    if refresh {
        refine_refresh(panels);
    }
}

fn refine_auto_carry_out(panels: &mut Panels, checked: bool) {
    let open = panels.is_open(super::refine::CONTENT);

    let Some(changed) = super::refine::auto_changed(&mut panels.refine, open, u8::from(checked))
    else {
        return;
    };

    let mirror = panels.refine.auto;

    let strings = panels.strings.clone();

    let Some(screen) = panels.screen_mut(super::refine::CONTENT) else {
        return;
    };

    screens::refine_auto_mirror(screen, mirror);

    if changed.halted {
        screens::refine_controls(screen, mirror, true, &strings);

        screens::refine_pulse_halt(screen);
    }

    screens::refine_stop_caption(screen, checked, &strings);
}

fn refine_write_item(entered: &mut Entered, at: Coord, item: Item) -> bool {
    let Ok(slot) = usize::try_from(at.slot) else {
        return false;
    };

    let target = match at.container {
        item_move::container::EQUIPMENT => entered.equipment.items.get_mut(slot),
        item_move::container::INVENTORY if slot < item_move::INVENTORY_SLOTS => {
            entered.carry.get_mut(slot)
        }
        item_move::container::CARGO => entered.warehouse.cargo.get_mut(slot),
        _ => None,
    };

    let Some(target) = target else {
        return false;
    };

    *target = item;

    true
}

pub fn refine_item_changed(panels: &mut Panels, entered: &Entered) {
    if panels.is_open(super::refine::CONTENT) {
        refine_settle(panels, entered);
    }
}

fn refine_settle(panels: &mut Panels, entered: &Entered) {
    let powder_at = panels.refine.powder.at;

    let piece_at = panels.refine.piece.at;

    let powder = item_move::item_at(entered, powder_at);

    let piece = item_move::item_at(entered, piece_at);

    let displayed_piece = refine_displayed_item(panels, "Mat0");

    let settlement = {
        let Panels { refine, items, .. } = panels;

        super::refine::settle(
            refine,
            super::refine::SettleInput {
                powder,
                piece,
                displayed_piece,
                carry: &entered.carry,
                items,
            },
        )
    };

    let Some(settlement) = settlement else {
        return;
    };

    if settlement.rejected_powder {
        refine_toast(panels, super::refine::WRONG_POWDER);

        refine_halt_ui(panels);
    }

    if let Some(at) = settlement.recovered_powder
        && let Some(item) = usize::try_from(at.slot)
            .ok()
            .and_then(|slot| entered.carry.get(slot))
            .copied()
    {
        refine_stage_powder(panels, entered, at, item);
    }

    if settlement.cleared_powder {
        refine_clear_cell(panels, 1);

        if let Some(screen) = panels.screen_mut(super::refine::CONTENT)
            && let Some(label) = screen.refine_element_mut("MatText1", ElementType::Static)
        {
            label.set_display("");
        }
    }

    if settlement.halted {
        let strings = panels.strings.clone();

        if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
            screens::refine_auto_mirror(screen, false);

            screens::refine_controls(screen, false, true, &strings);
        }
    }

    {
        let Panels {
            screens,
            items,
            icons,
            ..
        } = panels;

        if let Some(screen) = screens
            .iter_mut()
            .find(|s| s.content == super::refine::CONTENT)
        {
            if let Some(name) = screen.refine_element_mut("ItemName", ElementType::Static) {
                name.set_display("");
            }

            let resource = screen.placed.ui.resource.clone();

            for name in ["Mat0", "ResultItem"] {
                screens::clear_cell(screen, name, items, icons, &resource);
            }
        }
    }

    if let Some(at) = settlement.refresh_powder {
        refine_source_lock(panels, at, false);

        refine_stage_source(panels, entered, at);
    }

    refine_source_lock(panels, settlement.refresh_piece, false);

    refine_stage_source(panels, entered, settlement.refresh_piece);

    let checked = panels.refine.auto;

    let strings = panels.strings.clone();

    if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
        screens::refine_stop_caption(screen, checked, &strings);
    }
}

pub fn refine_manual_send(panels: &mut Panels, entered: &mut Entered, now_ms: u32) {
    panels.refine.manual_requested = false;

    let powder_at = panels.refine.powder.at;

    let powder = item_move::item_at(entered, powder_at);

    let piece = item_move::item_at(entered, panels.refine.piece.at);

    let result = {
        let Panels {
            refine,
            cooldowns,
            items,
            mixes,
            ..
        } = panels;

        super::refine::manual_send(
            refine,
            super::refine::ManualInput {
                client: entered.client_id,
                now_ms,
                cooldowns: *cooldowns,
                gold: entered.gold as i32,
                powder,
                piece,
                items,
                mixes: mixes.as_ref(),
            },
        )
    };

    match result {
        super::refine::Manual::Idle { settle: false } => return,
        super::refine::Manual::Idle { settle: true } => {}
        super::refine::Manual::Refused { strdef } => {
            refine_toast(panels, strdef);

            refine_halt_ui(panels);

            return;
        }
        super::refine::Manual::Sent(sent) => {
            let real_send = sent.frame.is_some();

            let powder_send = sent.spend.is_some();

            if let Some(spend) = sent.spend {
                refine_write_item(entered, spend.at, spend.after);

                if spend.after.is_empty() {
                    refine_clear_cell(panels, 1);

                    if let Some(screen) = panels.screen_mut(super::refine::CONTENT)
                        && let Some(label) =
                            screen.refine_element_mut("MatText1", ElementType::Static)
                    {
                        label.set_display("");
                    }
                }
            }

            panels.refine.last_send_ms = sent.now_ms;

            if real_send && !powder_send && sent.animate {
                if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
                    screens::refine_pulse_begin(screen);
                }
            }

            if let Some(frame) = sent.frame {
                panels.wire.push(frame);
            }

            if real_send && powder_send && sent.animate {
                if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
                    screens::refine_pulse_begin(screen);
                }
            }

            if sent.broadcast_slot.is_some() {
                screens::refresh_all(panels, entered);

                refine_item_changed(panels, entered);
            }

            super::refine::manual_after_notification(
                &mut panels.refine,
                item_move::item_at(entered, powder_at).unwrap_or_default(),
            );

            if panels.refine.auto {
                let checked = panels.refine.auto;

                let strings = panels.strings.clone();

                if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
                    screens::refine_controls(screen, checked, false, &strings);
                }
            }

            panels.cooldowns.last[sent.cooldown_bucket] = sent.now_ms;

            panels.refine.last_send_ms = sent.now_ms;

            if sent.animate {
                let delay = panels.refine.auto_delay;

                if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
                    screens::refine_pulse_tick(screen, sent.now_ms, sent.now_ms, delay);
                }
            }
        }
    }

    refine_settle(panels, entered);
}

pub fn refine_tick(panels: &mut Panels, entered: &mut Entered, now_ms: u32) {
    let stamp = panels.refine.last_send_ms;

    if stamp == 0 || panels.screen(super::refine::CONTENT).is_none() {
        return;
    }

    match super::refine::repeat(&panels.refine, stamp, now_ms) {
        None => {}
        Some(super::refine::Repeat::Refresh) => refine_settle(panels, entered),
        Some(super::refine::Repeat::Send) => refine_manual_send(panels, entered, now_ms),
    }

    let stamp = panels.refine.last_send_ms;

    let delay = panels.refine.auto_delay;

    if let Some(screen) = panels.screen_mut(super::refine::CONTENT) {
        screens::refine_pulse_tick(screen, stamp, now_ms, delay);
    }
}

fn coin_input_replacement(text: &str, action: i32) -> Option<String> {
    if action == 6 || text.encode_utf16().count() < 10 {
        return None;
    }

    let input = text.trim_start_matches(char::is_whitespace);

    let negative = input.starts_with('-');

    let input = input.strip_prefix(['+', '-']).unwrap_or(input);

    let mut value = 0i64;

    for ch in input.chars() {
        let Some(digit) = [
            0x30, 0x660, 0x6f0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xc66, 0xce6, 0xd66, 0xe50,
            0xed0, 0xf20, 0x1040, 0x17e0, 0x1810, 0xff10,
        ]
        .into_iter()
        .find_map(|base| u32::from(ch).checked_sub(base).filter(|&n| n < 10)) else {
            break;
        };

        value = (value * 10 + i64::from(digit)).min(2147483648);
    }

    let value = if negative {
        -value
    } else {
        value.min(2147483647)
    };

    if value == 0 {
        return None;
    }

    let cap = if matches!(action, 4 | 5) {
        1999999999
    } else {
        2000000000
    };

    if value >= cap {
        return Some(cap.to_string());
    }

    let mut replacement = text.to_owned();

    if text.encode_utf16().count() > 10 {
        replacement.pop();
    }

    Some(replacement)
}

#[cfg(test)]
mod language_tests;
