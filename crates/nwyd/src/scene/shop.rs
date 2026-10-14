use crate::frame::PanelId;
use crate::scene::chat;
use crate::scene::destroy;
use crate::scene::enter::Entered;
use crate::scene::field::FieldScene;
use crate::scene::hud;
use crate::scene::item_move;
use crate::scene::item_tip;
use crate::scene::item_use;
use crate::scene::msgbox;
use crate::scene::notice;
use crate::scene::panels::{
    CONTENT_CARGO, CONTENT_CHARACTER, CONTENT_INVENTORY, CONTENT_ITEM_SHOP, CONTENT_SKILL,
    CONTENT_STORE, Panels,
};
use crate::scene::screens;
use crate::scene::skill_master;
use anyhow::Context;
use formats::data::ItemList;
use formats::net::shop::{Kind, Sale, SellList, Stock};
use ui::Strings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Click {
    Page,
    Buy { cell: i32, item_id: i32 },
}

pub fn sync_prices(scene: &mut FieldScene) {
    if let Some(panels) = scene.panels.as_mut() {
        panels.shop_prices.repurchase = scene.shop_price_mode != 0;

        panels.shop_prices.tax_percent = scene.shop_tax_percent;

        panels.shop_prices.coupon = scene.shop_coupon;

        panels.shop_prices.buy_back = scene.shop_sell.map(|row| i32::from(row.item.index as i16));
    }
}

pub fn panel_clicks(scene: &mut FieldScene) {
    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    let pending = std::mem::take(&mut panels.shop_clicks);

    let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    let player_id = player.entity;

    if !panels.is_open(CONTENT_ITEM_SHOP) {
        return;
    }

    for click in pending {
        match click {
            Click::Page => {
                let Some(screen) = panels.screen(CONTENT_ITEM_SHOP) else {
                    continue;
                };

                let mut frame = if screen.page == 0 {
                    scene.shop_price_mode = 1;

                    super::interact::frame(0x3e8, 212, 0)
                } else {
                    scene.shop_price_mode = 0;

                    let npc = scene.shop_npc.unwrap_or(0);

                    panels.shop_id = npc;

                    scene.last_interact_ms = scene.time.now_ms;

                    super::interact::frame(0x27b, 16, u32::from(npc))
                };

                frame[6..8].copy_from_slice(&player_id.to_le_bytes());

                scene.outbox.push(frame);

                panels.shop_prices.repurchase = scene.shop_price_mode != 0;
            }
            Click::Buy { cell, item_id } => {
                let shop = if scene.shop_price_mode == 0 {
                    panels.shop_id
                } else {
                    player_id
                };

                match buy_at(panels, &scene.entered, cell, item_id, shop) {
                    Bought::Sent(request) => panels.used.push(item_use::Action::Send(request)),
                    Bought::BagFull => bag_full(panels),
                    _ => {}
                }
            }
        }
    }
}

pub fn sell_list(scene: &mut FieldScene, list: &SellList) -> Stocked {
    scene.shop_price_mode = formats::net::shop::SELL_MODE;

    scene.shop_sell = list.rows;

    sync_prices(scene);

    let npc = scene.shop_npc;

    let rows = list.filled();

    let Some(panels) = scene.panels.as_mut() else {
        return Stocked {
            rows,
            drawn: 0,
            panel: false,
            from_buy_page: false,
        };
    };

    if !panels.is_open(CONTENT_ITEM_SHOP) {
        return Stocked {
            rows,
            drawn: 0,
            panel: false,
            from_buy_page: false,
        };
    }

    panels.shop_id = npc.unwrap_or(0);

    let (items, icons) = (panels.items.clone(), panels.icons.clone());

    let Some(screen) = panels.screen_mut(CONTENT_ITEM_SHOP) else {
        return Stocked {
            rows,
            drawn: 0,
            panel: false,
            from_buy_page: false,
        };
    };

    let from_buy_page = screen.page == 0;

    let drawn = screens::shop_sell_page(screen, &list.rows, &items, &icons);

    Stocked {
        rows,
        drawn,
        panel: true,
        from_buy_page,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Stocked {
    pub rows: usize,
    pub drawn: usize,
    pub panel: bool,
    pub from_buy_page: bool,
}

pub fn describe_sell_list(applied: &Stocked) -> String {
    format!(
        "NWYD: 0x3E8 — {} of {} sell rows{}",
        applied.rows,
        formats::net::shop::SELL_SLOTS,
        match (applied.panel, applied.from_buy_page) {
            (false, _) =>
                ", with no shop on screen — the table and the price mode moved anyway".to_string(),
            (true, true) => format!(
                ", {} drawn on a page turned from the buy view",
                applied.drawn
            ),
            (true, false) => format!(", {} drawn", applied.drawn),
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Opened {
    Shop { npc: Option<u16>, filled: usize },
    SkillMaster { npc: u16, filled: usize },
    Ignored(u32),
    NoScreens,
}

pub fn stocked(scene: &mut FieldScene, stock: &Stock) -> Opened {
    match stock.kind {
        Kind::Shop => {}
        Kind::SkillMaster => {
            let Some(panels) = scene.panels.as_mut() else {
                return Opened::NoScreens;
            };

            let filled = skill_master::fill(panels, stock);

            let npc = panels.master.npc;

            open_skill_master(panels);

            return Opened::SkillMaster { npc, filled };
        }
        Kind::Other(value) => return Opened::Ignored(value),
    }

    scene.shop_tax_percent = i32::from_le_bytes(stock.tail);

    scene.shop_coupon = std::mem::take(&mut scene.shop_coupon_pending);

    sync_prices(scene);

    let npc = scene.shop_npc;

    if let Some(panels) = scene.panels.as_mut() {
        panels.set_open(CONTENT_CHARACTER, false);

        panels.set_open(skill_master::CONTENT, false);

        panels.set_open(CONTENT_SKILL, false);
    }

    if super::store::close(scene).wire() {
        scene.outbox.push(super::store::close_frame());
    }

    if let Some(frame) = super::trade::close(scene).frame {
        scene.outbox.push(frame);
    }

    let Some(panels) = scene.panels.as_mut() else {
        return Opened::NoScreens;
    };

    let filled = open(panels, stock);

    panels.shop_id = npc.unwrap_or(0);

    Opened::Shop { npc, filled }
}

pub fn open(panels: &mut Panels, stock: &Stock) -> usize {
    for content in [
        CONTENT_CHARACTER,
        skill_master::CONTENT,
        CONTENT_SKILL,
        CONTENT_STORE,
        super::panels::CONTENT_TRADE,
        CONTENT_INVENTORY,
        CONTENT_CARGO,
    ] {
        panels.set_open(content, false);
    }

    panels.pair_inventory(0, false);

    panels.set_open(CONTENT_ITEM_SHOP, true);

    let (items, icons) = (panels.items.clone(), panels.icons.clone());

    if let Some(screen) = panels.screen_mut(CONTENT_ITEM_SHOP) {
        screens::shop_refresh(screen, &stock.items, &items, &icons);
    }

    panels.set_open(CONTENT_INVENTORY, true);

    panels.pair_inventory(CONTENT_ITEM_SHOP, true);

    stock.items.iter().filter(|item| !item.is_empty()).count()
}

pub fn close(panels: &mut Panels) -> bool {
    if !panels.is_open(CONTENT_ITEM_SHOP) {
        return false;
    }

    panels.close_by_toggle(CONTENT_ITEM_SHOP);

    panels.close_by_toggle(CONTENT_INVENTORY);

    true
}

pub fn describe(opened: &Opened) -> String {
    match opened {
        Opened::Shop { npc, filled } => match npc {
            Some(npc) => {
                format!("NWYD: 0x17C — PanelItemShop open for entity {npc}, {filled} item(s)")
            }
            None => {
                format!("NWYD: 0x17C — PanelItemShop open, {filled} item(s), no 0x27B behind it")
            }
        },
        Opened::SkillMaster { npc, filled } => {
            format!("NWYD: 0x17C — PanelSkillMaster open for entity {npc}, {filled} book(s)")
        }
        Opened::Ignored(value) => format!("NWYD: 0x17C kind {value} reaches no arm"),
        Opened::NoScreens => "NWYD: 0x17C arrived with no kernel UI up".to_string(),
    }
}

pub fn open_skill_master(panels: &mut Panels) {
    panels.set_open(CONTENT_CHARACTER, false);

    panels.set_open(CONTENT_ITEM_SHOP, false);

    panels.set_open(skill_master::CONTENT, true);

    panels.set_open(CONTENT_SKILL, true);
}

pub const CLOSED_BY_OPEN: [PanelId; 2] = [CONTENT_STORE, CONTENT_CARGO];

pub const SHOP_ID_FIELD: u32 = 0xC6;

pub fn buy(panels: &mut Panels, entered: &Entered, cell: i32, item_id: i32) -> Bought {
    buy_at(panels, entered, cell, item_id, panels.shop_id)
}

fn buy_at(panels: &mut Panels, entered: &Entered, cell: i32, item_id: i32, shop: u16) -> Bought {
    let Some(screen) = panels.screen(CONTENT_ITEM_SHOP) else {
        return Bought::NoScreen;
    };

    let page = screen.page;

    if !(1..=MAX_TEMPLATE_ID).contains(&item_id) {
        return Bought::Refused;
    }

    if cell < 0 || (page == 0 && cell >= 0x1B) || (page == 1 && cell >= 10) {
        return Bought::Refused;
    }

    let tab = panels
        .screen(CONTENT_INVENTORY)
        .map_or(0, |screen| screen.page);

    let slot = item_use::free_carry_slot(entered, tab, true);

    if slot < 0 {
        return Bought::BagFull;
    }

    if item_id == UNBUYABLE {
        return Bought::Unbuyable;
    }

    let request = item_move::Request::Buy {
        shop,
        stock: item_move::stock_index(cell),
        slot,
    };

    panels.outbox.push(request);

    Bought::Sent(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Bought {
    Sent(item_move::Request),
    Refused,
    BagFull,
    Unbuyable,
    NoScreen,
}

pub fn bag_full(panels: &mut Panels) {
    let text = panels.strings.translate(&format!("ss_{BAG_FULL_STRDEF}"));

    chat::system(panels, &text, BAG_FULL_COLOR);

    panels.notice.sounds.push(BAG_FULL_SOUND);
}

pub const BAG_FULL_STRDEF: usize = 1;

pub const BAG_FULL_COLOR: u32 = hud::TOAST_COLOR;

pub const BAG_FULL_SOUND: i64 = notice::SOUND_SLOT;

pub const MAX_TEMPLATE_ID: i32 = 0x38A3;

pub const UNBUYABLE: i32 = 0x1033;

pub fn sell(panels: &mut Panels, entered: &Entered, ctrl: bool, carried: Carry) -> Sold {
    let Some(screen) = panels.screen(CONTENT_ITEM_SHOP) else {
        return Sold::NoScreen;
    };

    if screen.page != 0 {
        return Sold::Refused;
    }

    if carried.kind != CARRIED_KIND {
        return Sold::Refused;
    }

    if carried.group != ui::widget::cell_group::INVENTORY {
        return Sold::Refused;
    }

    if !(1..=MAX_TEMPLATE_ID).contains(&carried.item_id) {
        return Sold::Refused;
    }

    let slot = carried.slot;

    if !(0..CARRY_CELLS).contains(&slot) {
        return Sold::Refused;
    }

    let live = entered.carry.get(slot as usize).copied();

    if live.map(|item| i32::from(item.index as i16)) != Some(carried.item_id) {
        return Sold::Moved;
    }

    let request = item_move::Request::Sell {
        shop: panels.shop_id,
        at: item_move::Coord {
            container: item_move::container::INVENTORY,
            slot,
        },
    };

    if !ctrl {
        let (Ok(item_id), Some(snapshot)) = (u16::try_from(carried.item_id), live) else {
            return Sold::Refused;
        };

        return Sold::Asks {
            request,
            prompt: destroy::Prompt {
                item_id,
                slot,
                snapshot,
            },
        };
    }

    panels.outbox.push(request);

    Sold::Sent(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Sold {
    Sent(item_move::Request),
    Asks {
        request: item_move::Request,
        prompt: destroy::Prompt,
    },
    Moved,
    Refused,
    NoScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Carry {
    pub kind: i32,
    pub group: i32,
    pub slot: i32,
    pub item_id: i32,
}

pub const CARRIED_KIND: i32 = ui::carry::ITEM_KIND;

pub const CARRY_CELLS: i32 = 0x3C;

pub const KIND: i32 = 0x1B;

pub const CONTEXT: u32 = 0x19;

pub const CONFIRM_NAME_STRDEF: usize = item_use::strdef::SELL_NAME;

pub const CONFIRM_STRDEF: usize = item_use::strdef::SELL_CONFIRM;

pub const LINE_BREAK: &str = ui::text::FF_BREAK;

pub fn message(items: &ItemList, strings: &Strings, prompt: &destroy::Prompt) -> String {
    let name = items.name(usize::from(prompt.item_id)).unwrap_or_default();

    let titled = strings
        .translate(&format!("ss_{CONFIRM_NAME_STRDEF}"))
        .replacen("%s", &name, 1);

    format!(
        "{titled}{LINE_BREAK}{}",
        strings.translate(&format!("ss_{CONFIRM_STRDEF}"))
    )
}

pub fn request(items: &ItemList, strings: &Strings, prompt: &destroy::Prompt) -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: KIND,
        context: CONTEXT,
        body: msgbox::Body::Text(message(items, strings, prompt)),
        flags: msgbox::FLAGS,
    }
}

pub fn confirm(
    entered: &Entered,
    shop_id: u16,
    prompt: &destroy::Prompt,
) -> Option<item_move::Request> {
    if !(0..CARRY_CELLS).contains(&prompt.slot) {
        return None;
    }

    let index = usize::try_from(prompt.slot).ok()?;

    let live = *entered.carry.get(index)?;

    if live.index != prompt.item_id {
        return None;
    }

    if live != prompt.snapshot {
        return None;
    }

    Some(item_move::Request::Sell {
        shop: shop_id,
        at: item_move::Coord {
            container: item_move::container::INVENTORY,
            slot: prompt.slot,
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Cleared {
    pub shop: u16,
    pub container: i16,
    pub slot: i16,
    pub accepted: bool,
    pub out_of_range: bool,
    pub written: bool,
    pub credited: i32,
    pub redressed: bool,
}

pub fn sold(
    scene: &mut FieldScene,
    game: &assets::GameDir,
    sale: &Sale,
) -> anyhow::Result<Cleared> {
    let shop_id = scene.panels.as_ref().map_or(0, |panels| panels.shop_id);

    let mut cleared = Cleared {
        shop: sale.shop,
        container: sale.container,
        slot: sale.slot,
        accepted: accepts(shop_id, &scene.entered, sale.shop),
        out_of_range: false,
        written: false,
        credited: 0,
        redressed: false,
    };

    if cleared.accepted {
        if sale.container == 0 || sale.container == 1 {
            let bound = CARRY_CELLS as usize;

            let Some(index) = usize::try_from(sale.slot).ok().filter(|at| *at < bound) else {
                cleared.out_of_range = true;

                return Ok(cleared);
            };

            let item = scene.entered.carry[index];

            cleared.credited = sale_price(&scene.statics.items, i32::from(item.index as i16));

            scene.entered.carry[index] = Default::default();

            cleared.written = true;

            scene.entered.gold = scene.entered.gold.wrapping_add(cleared.credited as u32);

            if let Some(panels) = scene.panels.as_mut() {
                super::panels::refine_item_changed(panels, &scene.entered);
            }
        }

        if let FieldScene {
            panels: Some(panels),
            entered,
            ..
        } = &mut *scene
        {
            screens::refresh_all(panels, entered);

            panels.notice.sounds.push(SALE_SOUND);
        }
    }

    if let Some(index) = scene.player {
        cleared.redressed = super::live::redress_player(scene, game, index)
            .context("re-dressing the player after a 0x37A")?;
    }

    Ok(cleared)
}

fn accepts(shop_id: u16, entered: &Entered, shop: u16) -> bool {
    if shop_id == shop {
        return true;
    }

    shop == 0
        && entered.class == ANGEL_SELL_CLASS
        && entered.skill_mask[0] & ANGEL_SELL_SKILL_BIT != 0
}

pub const ANGEL_SELL_CLASS: u8 = 3;

pub const ANGEL_SELL_SKILL_BIT: u32 = 0x1000;

pub const SALE_SOUND: i64 = 0x1F;

fn sale_price(items: &ItemList, id: i32) -> i32 {
    if !(1..=MAX_TEMPLATE_ID).contains(&id) {
        return 0;
    }

    let Some(entry) = usize::try_from(id).ok().and_then(|id| items.get(id)) else {
        return 0;
    };

    item_tip::quartered_price(entry.price)
}

pub fn describe_sale(cleared: &Cleared) -> String {
    let Cleared {
        shop,
        container,
        slot,
        ..
    } = *cleared;

    if !cleared.accepted {
        return format!(
            "NWYD: 0x37A named shop {shop}, which is not the open shop and not the Angel's zero — nothing sold. The gate's third arm is not built here; a server that rewrites the shop id would land exactly here (D-329)",
        );
    }

    if cleared.out_of_range {
        return format!("NWYD: 0x37A named slot {slot}, outside the backpack's 60 — nothing sold");
    }

    if !cleared.written {
        return format!(
            "NWYD: 0x37A named container {container}, which is neither 0 nor 1 — nothing sold",
        );
    }

    format!(
        "NWYD: 0x37A sold — backpack slot {slot} emptied for shop {shop}, {} gold{}",
        cleared.credited,
        match cleared.redressed {
            true => ", body re-dressed",
            false => "",
        },
    )
}
