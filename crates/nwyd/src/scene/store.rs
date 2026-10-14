use crate::scene::chat;
use crate::scene::enter::Entered;
use crate::scene::field::FieldScene;
use crate::scene::item_use;
use crate::scene::msgbox;
use crate::scene::notice;
use crate::scene::panels::{
    CONTENT_CARGO, CONTENT_CHARACTER, CONTENT_COIN_INPUT, CONTENT_INVENTORY, CONTENT_SKILL,
    CONTENT_STORE, Panels,
};
use crate::scene::screens;
use formats::data::item_effect::{Effects, effect};
use formats::net::account::Item;
use formats::net::autotrade::{Buy, Contents, PANEL_SLOTS, Sold};
use ui::Strings;

pub const CELLS: usize = 10;

pub const PROMPT_KIND: i32 = 0x10;

pub const PROMPT_CONTEXT: u32 = 0x1D;

pub const PROMPT_FLAGS: u32 =
    msgbox::CLOSE_SAME_CONTEXT | msgbox::PANE_EDITBOX | msgbox::BUTTONS_YES_NO;

pub const OWNER_TITLE_BOUND: usize = 0x14;

pub const PROMPT_STRDEF: usize = 141;

pub const PRICE_KIND: i32 = 0x1C;

pub const PRICE_CONTEXT: i32 = 0x1E;

pub const PRICE_ACTION: i32 = 4;

pub const PRICE_STRDEF: usize = 142;

pub const INVALID_PRICE_STRDEF: usize = 34;

pub const PRICE_CAP_STRDEF: usize = 143;

pub const SELECT_ITEMS_STRDEF: usize = 145;

pub const BLOCKED_ITEM_STRDEF: usize = 309;

pub const MAX_PRICE: i32 = 1_999_999_999;

pub const OWNER_NOTICE_MS: u32 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PricePrompt {
    pub captured_item: i32,
    pub source_slot: usize,
    pub display_only: bool,
}

pub fn display_only(price: i32) -> bool {
    price >= 2_000_000_000
}

pub fn exhibition_price(text: &str) -> i32 {
    2_000_000_000
        + text
            .parse::<i32>()
            .ok()
            .filter(|n| {
                (1..1_000_000).contains(n)
                    && text.len() <= 9
                    && text.bytes().all(|c| c.is_ascii_digit())
            })
            .unwrap_or(0)
}

pub fn toggle_exhibition(panels: &mut Panels) {
    if !panels.is_open(CONTENT_COIN_INPUT) || panels.coin_action != 4 {
        return;
    }

    let Some(prompt) = panels.store_price.as_mut() else {
        return;
    };

    prompt.display_only = !prompt.display_only;

    let checked = prompt.display_only;

    let body = panels
        .strings
        .translate(if checked { "ss_1840" } else { "ss_142" });

    if let Some(screen) = panels.screen_mut(CONTENT_COIN_INPUT) {
        screens::coin_input_option(screen, "Expo", true, checked);

        screens::text(screen, "msg", &body);
    }

    if checked && let Some(request) = price_confirmation(panels, 1845, 0x5c) {
        panels.hide_box_contexts(0x5c, 0x5d);

        panels.raise_box(&request, None);
    }
}

fn price_confirmation(panels: &Panels, id: usize, context: u32) -> Option<msgbox::Request> {
    let mut body = panels.strings.translate(&format!("ss_{id}"));

    if body.is_empty() {
        return None;
    }

    let suffix = panels.strings.translate("ss_152");

    if !suffix.is_empty() {
        body.push(' ');

        body.push_str(&suffix);
    }

    Some(msgbox::Request {
        caption: Some(237),
        kind: 0x27,
        context,
        body: msgbox::Body::Text(body),
        flags: msgbox::FLAGS,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Price {
    Empty,
    Invalid,
    TooLarge,
    Accepted(i32),
}

pub fn price(text: &str) -> Price {
    if text.is_empty() {
        return Price::Empty;
    }

    let mut value = 0i32;

    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Price::Invalid;
        }

        let Some(next) = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(i32::from(byte - b'0')))
        else {
            return Price::TooLarge;
        };

        if next > MAX_PRICE {
            return Price::TooLarge;
        }

        value = next;
    }

    Price::Accepted(value)
}

pub fn price_prompt(entered: &Entered, source_slot: i32) -> Option<PricePrompt> {
    let source_slot = usize::try_from(source_slot).ok()?;

    let item = *entered.warehouse.cargo.get(source_slot)?;

    if item.is_empty() {
        return None;
    }

    Some(PricePrompt {
        captured_item: i32::from(item.index as i16),
        source_slot,
        display_only: false,
    })
}

pub fn begin_price(panels: &mut Panels, entered: &Entered, source_slot: i32) -> bool {
    let Some(prompt) = price_prompt(entered, source_slot) else {
        return false;
    };

    if panels.screen(CONTENT_COIN_INPUT).is_none() {
        return false;
    }

    panels.hide_box_contexts(0x5c, 0x5d);

    panels.set_open(CONTENT_COIN_INPUT, true);

    panels.split_stack = None;

    panels.seal_name_slot = None;

    panels.store_price = Some(prompt);

    panels.coin_action = 4;

    let body = panels.strings.translate(&format!("ss_{PRICE_STRDEF}"));

    if let Some(screen) = panels.screen_mut(CONTENT_COIN_INPUT) {
        screens::coin_input_fill(screen, &body);

        screens::coin_input_option(screen, "Expo", true, false);
    }

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Stocked {
    Placed {
        slot: usize,
        source_slot: usize,
        item: Item,
        price: i32,
    },
    NoSource,
    NonPositive,
    Full,
}

pub fn stock(
    contents: &mut Contents,
    cargo: &[Item],
    prompt: PricePrompt,
    asking_price: i32,
) -> Stocked {
    let Some(item) = cargo
        .get(prompt.source_slot)
        .copied()
        .filter(|item| !item.is_empty())
    else {
        return Stocked::NoSource;
    };

    if asking_price <= 0 {
        return Stocked::NonPositive;
    }

    let Some(slot) = contents.items[..PANEL_SLOTS]
        .iter()
        .position(Item::is_empty)
    else {
        return Stocked::Full;
    };

    contents.items[slot] = item;

    contents.source_slots[slot] = prompt.source_slot as u8;

    contents.prices[slot] = asking_price;

    Stocked::Placed {
        slot,
        source_slot: prompt.source_slot,
        item,
        price: asking_price,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Priced {
    NotOpen,
    NotOurs,
    Cancelled,
    Empty,
    Invalid,
    TooLarge,
    Confirming,
    Stocked(Stocked),
}

pub fn answer_price(panels: &mut Panels, entered: &Entered, yes: bool, now_ms: u32) -> Priced {
    answer_price_confirmed(panels, entered, yes, now_ms, false)
}

pub fn answer_price_confirmed(
    panels: &mut Panels,
    entered: &Entered,
    yes: bool,
    now_ms: u32,
    confirmed: bool,
) -> Priced {
    if !panels.is_open(CONTENT_COIN_INPUT) {
        return Priced::NotOpen;
    }

    if !yes {
        panels.store_price = None;

        panels.set_open(CONTENT_COIN_INPUT, false);

        return Priced::Cancelled;
    }

    let Some(prompt) = panels.store_price else {
        return Priced::NotOurs;
    };

    let parsed = if prompt.display_only {
        Price::Accepted(exhibition_price(
            panels.coin_input_text().unwrap_or_default(),
        ))
    } else {
        panels.coin_input_text().map_or(Price::Empty, price)
    };

    let asking_price = match parsed {
        Price::Empty => return Priced::Empty,
        Price::Invalid => {
            owner_notice(panels, INVALID_PRICE_STRDEF, now_ms);

            return Priced::Invalid;
        }
        Price::TooLarge => {
            owner_notice(panels, PRICE_CAP_STRDEF, now_ms);

            return Priced::TooLarge;
        }
        Price::Accepted(asking_price) => asking_price,
    };

    if asking_price > 0
        && !prompt.display_only
        && !confirmed
        && let Some(request) = price_confirmation(panels, 1846, 0x5d)
    {
        panels.hide_box_contexts(0x5c, 0x5d);

        panels.raise_box(&request, None);

        return Priced::Confirming;
    }

    let outcome = if !panels.is_open(CONTENT_STORE) {
        Stocked::NoSource
    } else {
        stock_from_cargo(panels, entered, prompt, asking_price, now_ms)
    };

    panels.store_price = None;

    panels.set_open(CONTENT_COIN_INPUT, false);

    Priced::Stocked(outcome)
}

fn stock_from_cargo(
    panels: &mut Panels,
    entered: &Entered,
    prompt: PricePrompt,
    asking_price: i32,
    now_ms: u32,
) -> Stocked {
    let Some(_) = entered
        .warehouse
        .cargo
        .get(prompt.source_slot)
        .copied()
        .filter(|item| !item.is_empty())
    else {
        return Stocked::NoSource;
    };

    if !panels.is_open(CONTENT_CARGO) {
        return Stocked::NoSource;
    }

    let unlocked = panels
        .screen_mut(CONTENT_CARGO)
        .is_some_and(|screen| screens::cargo_lock(screen, prompt.source_slot, false));

    if !unlocked {
        return Stocked::NoSource;
    }

    let outcome = match panels.store.as_mut() {
        Some(contents) => stock(contents, &entered.warehouse.cargo, prompt, asking_price),
        None => Stocked::NoSource,
    };

    match outcome {
        Stocked::Placed {
            slot,
            source_slot,
            item,
            price,
        } => {
            if let Some(screen) = panels.screen_mut(CONTENT_CARGO) {
                screens::cargo_lock(screen, source_slot, true);
            }

            let (items, icons) = (panels.items.clone(), panels.icons.clone());

            if let Some(screen) = panels.screen_mut(CONTENT_STORE) {
                screens::store_stocked(screen, slot, &item, price, &items, &icons);
            }
        }
        Stocked::Full => owner_notice(panels, NO_SPACE_STRDEF, now_ms),
        Stocked::NoSource | Stocked::NonPositive => {}
    }

    outcome
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Announced {
    Empty,
    Sent(Vec<u8>),
}

pub fn ready(contents: &Contents) -> bool {
    contents.items[..PANEL_SLOTS]
        .iter()
        .any(|item| (item.index as i16) > 0)
}

pub fn open_frame(contents: &Contents) -> Vec<u8> {
    let mut out = vec![0u8; formats::net::autotrade::CONTENTS_FRAME_LEN];

    protocol::header::set_opcode(&mut out, formats::net::autotrade::CONTENTS);

    out[protocol::header::LEN..].copy_from_slice(&contents.to_bytes());

    out
}

pub fn announce(panels: &mut Panels, entered: &Entered, now_ms: u32) -> Announced {
    let Some(contents) = panels.store.as_mut() else {
        owner_notice(panels, SELECT_ITEMS_STRDEF, now_ms);

        return Announced::Empty;
    };

    if !ready(contents) {
        owner_notice(panels, SELECT_ITEMS_STRDEF, now_ms);

        return Announced::Empty;
    }

    contents.owner = entered.client_id;

    let frame = open_frame(contents);

    if let Some(screen) = panels.screen_mut(CONTENT_STORE) {
        screens::store_announced(screen);
    }

    panels.close_by_toggle(CONTENT_CARGO);

    let right = panels.layout.virtual_screen()[2];

    if let Some(screen) = panels.screen_mut(CONTENT_STORE) {
        screen.placed.origin = (right - screen.placed.ui.panel.width(), 10);
    }

    Announced::Sent(frame)
}

pub fn blocked_item(panels: &mut Panels, now_ms: u32) {
    owner_notice(panels, BLOCKED_ITEM_STRDEF, now_ms);
}

fn owner_notice(panels: &mut Panels, strdef: usize, now_ms: u32) {
    let mut text = panels.strings.translate(&format!("ss_{strdef}"));

    if strdef == PRICE_CAP_STRDEF {
        text = text.replacen("%d", "2000000000", 1);
    }

    let life = if matches!(strdef, INVALID_PRICE_STRDEF | PRICE_CAP_STRDEF) {
        1000
    } else {
        OWNER_NOTICE_MS
    };

    notice::post(panels, &text, life, true, now_ms);
}

pub const CONFIRM_STRDEF: usize = 144;

pub const NO_SPACE_STRDEF: usize = 1;

pub const NO_SPACE_COLOR: u32 = 0xFFFF_AAAA;

pub const MONEY_USE_KIND: i32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Opened {
    Browsing { owner: u16, filled: usize },
    Own { title: String },
    NoScreens,
}

pub fn prompt() -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: PROMPT_KIND,
        context: PROMPT_CONTEXT,
        body: msgbox::Body::StrDef(PROMPT_STRDEF),
        flags: PROMPT_FLAGS,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Started {
    Opened,
    AlreadyOpen,
    NoScreen,
}

pub fn open_owner(panels: &mut Panels, entered: &Entered, title: &str) -> Started {
    if panels.screen(CONTENT_STORE).is_none() {
        return Started::NoScreen;
    }

    if panels.is_open(CONTENT_STORE) {
        return Started::AlreadyOpen;
    }

    open_selling(panels);

    let mut contents = Contents::default();

    let encoded = formats::text::codepage::encode(title, formats::text::Codepage::Windows1252);

    for (to, from) in contents
        .title
        .iter_mut()
        .zip(encoded.into_iter().take(OWNER_TITLE_BOUND))
    {
        *to = from;
    }

    contents.owner = entered.client_id;

    panels.store = Some(contents);

    let (items, icons) = (panels.items.clone(), panels.icons.clone());

    if let Some(screen) = panels.screen_mut(CONTENT_STORE) {
        screens::store_owner_fill(screen, entered, title, &items, &icons);
    }

    Started::Opened
}

pub fn contents(scene: &mut FieldScene, contents: &Contents) -> Opened {
    finish_panel_close(scene);

    let mine = contents.owner == scene.entered.client_id;

    let seller = scene
        .world
        .actors
        .entity(contents.owner)
        .map(|index| scene.world.actors.actors[index].name.clone())
        .unwrap_or_default();

    let title = contents.title();

    let Some(panels) = scene.panels.as_mut() else {
        return match mine {
            true => {
                raise_signboard(scene, &title);

                Opened::Own { title }
            }
            false => Opened::NoScreens,
        };
    };

    panels.store = Some(contents.clone());

    let (items, icons) = (panels.items.clone(), panels.icons.clone());

    let filled = match panels.screen_mut(CONTENT_STORE) {
        Some(screen) => screens::store_fill(screen, contents, &seller, mine, &items, &icons),
        None => 0,
    };

    if mine {
        raise_signboard(scene, &title);

        return Opened::Own { title };
    }

    let Some(panels) = scene.panels.as_mut() else {
        return Opened::NoScreens;
    };

    open(panels);

    Opened::Browsing {
        owner: contents.owner,
        filled,
    }
}

fn raise_signboard(scene: &mut FieldScene, title: &str) {
    finish_panel_close(scene);

    let me = scene.entered.client_id;

    if let Some(index) = scene.world.actors.entity(me) {
        scene.world.actors.actors[index].shop_title = title.to_string();
    }
}

pub fn finish_panel_close(scene: &mut FieldScene) {
    let owner_present = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_some();

    if let Some(p) = scene.panels.as_mut() {
        p.field_close_owner_present = owner_present;
    }

    let pending = scene
        .panels
        .as_mut()
        .is_some_and(|p| std::mem::take(&mut p.store_closed_pending));

    if pending && let Some(index) = scene.world.actors.entity(scene.entered.client_id) {
        scene.world.actors.actors[index].shop_title.clear();
    }
}

pub fn open(panels: &mut Panels) {
    for content in [CONTENT_CHARACTER, CONTENT_SKILL] {
        panels.close_by_toggle(content);
    }

    panels.set_open(CONTENT_STORE, true);

    panels.set_open(CONTENT_INVENTORY, true);

    panels.pair_inventory(CONTENT_STORE, true);
}

pub fn open_selling(panels: &mut Panels) {
    for content in [CONTENT_CHARACTER, CONTENT_SKILL] {
        panels.close_by_toggle(content);
    }

    panels.close_by_toggle(CONTENT_INVENTORY);

    panels.set_open(CONTENT_CARGO, true);

    panels.set_open(CONTENT_STORE, true);

    let anchor = panels.screen(CONTENT_STORE).map(|screen| {
        (
            screen.placed.origin.0 + screen.placed.ui.panel.width(),
            screen.placed.origin.1,
        )
    });

    if let Some(anchor) = anchor
        && let Some(cargo) = panels.screen_mut(CONTENT_CARGO)
    {
        cargo.placed.origin = anchor;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Closed {
    WasShut,
    Browsed { owner: u16 },
    Own,
}

impl Closed {
    pub fn wire(&self) -> bool {
        matches!(self, Closed::Own)
    }
}

pub fn close(scene: &mut FieldScene) -> Closed {
    finish_panel_close(scene);

    let me = scene.entered.client_id;

    let Some(panels) = scene.panels.as_ref() else {
        return Closed::WasShut;
    };

    if !panels.is_open(CONTENT_STORE) || panels.screen(CONTENT_CARGO).is_none() {
        return Closed::WasShut;
    }

    let owner = panels.store.as_ref().map(|contents| contents.owner);

    if let Some(index) = scene.world.actors.entity(me) {
        scene.world.actors.actors[index].shop_title.clear();
    }

    let panels = scene.panels.as_mut().unwrap();

    if !scene.reel.visible {
        panels.close_by_toggle(CONTENT_INVENTORY);
    }

    if let Some(contents) = panels.store.as_mut() {
        contents.owner = 0;
    }

    panels.close_by_toggle(CONTENT_CARGO);

    finish_panel_close(scene);

    match owner {
        Some(owner) if owner == me => Closed::Own,
        Some(owner) => Closed::Browsed { owner },
        None => Closed::Browsed { owner: 0 },
    }
}

pub fn close_frame() -> Vec<u8> {
    let mut out = vec![0u8; CLOSE_FRAME_LEN];

    protocol::header::set_opcode(&mut out, formats::net::autotrade::CLOSE_OWN);

    out
}

pub const CLOSE_FRAME_LEN: usize = 0x0C;

pub fn describe_close(closed: &Closed) -> String {
    match closed {
        Closed::WasShut => {
            "NWYD: CloseBtn — no open shop with a built Cargo controller".to_string()
        }
        Closed::Browsed { owner } => {
            format!("NWYD: CloseBtn — cleared shop owner {owner}, nothing sent")
        }
        Closed::Own => "NWYD: CloseBtn — my own shop cancelled, 0x384 required".to_string(),
    }
}

pub fn slot_sold(scene: &mut FieldScene, sold: &Sold) -> Sale {
    let Some(panels) = scene.panels.as_mut() else {
        return Sale::NoScreens;
    };

    if !panels.is_open(CONTENT_STORE) {
        return Sale::NotOpen;
    }

    let Some(contents) = panels.store.as_mut() else {
        return Sale::NotOpen;
    };

    if u32::from(contents.owner) != sold.owner {
        return Sale::OtherShop {
            latched: contents.owner,
        };
    }

    if !sold.slot_is_drawable() {
        return Sale::OutOfRange { slot: sold.slot };
    }

    let slot = sold.slot as usize;

    contents.items[slot] = Default::default();

    let (items, icons) = (panels.items.clone(), panels.icons.clone());

    if let Some(screen) = panels.screen_mut(CONTENT_STORE) {
        let resource = screen.placed.ui.resource.clone();

        screens::clear_cell(screen, &format!("Inven{slot}"), &items, &icons, &resource);

        screens::store_exhibition(screen, slot, false);
    }

    Sale::Cleared { slot }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Sale {
    Cleared { slot: usize },
    NotOpen,
    OtherShop { latched: u16 },
    OutOfRange { slot: i32 },
    NoScreens,
}

pub const KIND: i32 = 0x1C;

pub const CONTEXT: u32 = 0x1F;

pub const BOX_FLAGS: u32 = 0x2000_0001;

pub fn request(strings: &Strings) -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: KIND,
        context: CONTEXT,
        body: msgbox::Body::Text(strings.translate(&format!("ss_{CONFIRM_STRDEF}"))),
        flags: msgbox::FLAGS,
    }
}

pub fn frame(buy: &Buy) -> Vec<u8> {
    let mut out = vec![0u8; formats::net::autotrade::BUY_FRAME_LEN];

    protocol::header::set_opcode(&mut out, formats::net::autotrade::BUY);

    out[protocol::header::LEN..].copy_from_slice(&buy.to_bytes());

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Bought {
    Asked { slot: usize },
    MyOwnShop,
    NoSpace,
    NoScreen,
}

pub fn clicked(panels: &Panels, entered: &Entered, slot: usize) -> Bought {
    let Some(contents) = panels.store.as_ref() else {
        return Bought::NoScreen;
    };

    if slot >= PANEL_SLOTS {
        return Bought::NoScreen;
    }

    if contents.owner == entered.client_id {
        return Bought::MyOwnShop;
    }

    let item = contents.items[slot];

    if item.is_empty() || display_only(contents.prices[slot]) {
        return Bought::NoScreen;
    }

    if item_use::free_carry_slot(entered, 0, true) < 0 && !is_money(panels, &item) {
        return Bought::NoSpace;
    }

    Bought::Asked { slot }
}

fn is_money(panels: &Panels, item: &formats::net::account::Item) -> bool {
    Effects::new(&panels.items, &[]).total_scaled(item, effect::USE_KIND) == MONEY_USE_KIND
}

pub fn confirm(panels: &Panels, entered: &Entered, slot: usize) -> Option<Buy> {
    if !panels.is_open(CONTENT_STORE) {
        return None;
    }

    let contents = panels.store.as_ref()?;

    if contents.owner == entered.client_id {
        return None;
    }

    if slot >= PANEL_SLOTS || contents.items[slot].is_empty() || display_only(contents.prices[slot])
    {
        return None;
    }

    Buy::of(contents, slot)
}

pub fn no_space(panels: &mut Panels) {
    let text = panels.strings.translate(&format!("ss_{NO_SPACE_STRDEF}"));

    chat::system(panels, &text, NO_SPACE_COLOR);

    panels.notice.sounds.push(super::shop::BAG_FULL_SOUND);
}

pub fn describe(opened: &Opened) -> String {
    match opened {
        Opened::Browsing { owner, filled } => {
            format!(
                "NWYD: 0x397 — entity {owner}'s shop is up, {filled} of {CELLS} slot(s) stocked"
            )
        }
        Opened::Own { title } => {
            format!("NWYD: 0x397 — my own shop is up, titled {title:?}")
        }
        Opened::NoScreens => "NWYD: 0x397 arrived with no kernel UI (--no-hud)".to_string(),
    }
}

pub fn describe_sale(sale: &Sale) -> String {
    match sale {
        Sale::Cleared { slot } => {
            format!("NWYD: 0x39B — shop slot {slot} sold, cell cleared")
        }
        Sale::NotOpen => "NWYD: 0x39B — no shop on screen, ignored".to_string(),
        Sale::OtherShop { latched } => {
            format!("NWYD: 0x39B — names another shop; {latched} is the one on screen")
        }
        Sale::OutOfRange { slot } => {
            format!("NWYD: 0x39B — slot {slot} is not one of the ten")
        }
        Sale::NoScreens => "NWYD: 0x39B arrived with no kernel UI (--no-hud)".to_string(),
    }
}
