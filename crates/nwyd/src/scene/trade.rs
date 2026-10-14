use super::field::FieldScene;
use super::msgbox;
use formats::net::trade::{self, Offer};
use ui::Strings;

pub const READY_CONTROL: u32 = 0x269;

pub const CONTENT: u32 = 0x1D;

pub const BOX_CONTEXT: u32 = 0x1C;

pub const BOX_KIND: i32 = 0xD;

pub const BOX_FLAGS: u32 = 0x2000_0001;

pub const READY_COOLDOWN_MS: u32 = 2000;

pub const OPENING_CLOSES: [u32; 9] = [0x14, 0x17, 0x18, 0x15, 0x16, 0x1B, 0x19, 0x1A, 0x36];

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct Session {
    pub mine: Offer,
    pub theirs: Offer,
    pub ready_control: bool,
    pub stamp_ms: u32,
    pub partner_name: String,
    pub my_name: String,
}

impl Session {
    pub fn clear(&mut self) {
        self.mine = Offer::cleared();

        self.theirs = Offer::cleared();

        self.ready_control = false;

        self.partner_name.clear();

        self.my_name.clear();
    }

    pub fn engaged(&self) -> bool {
        self.mine.partner != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Arm {
    Request,
    Contents,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Raise {
    pub kind: i32,
    pub context: u32,
    pub flags: u32,
    pub subject: u16,
    pub body: String,
    pub slot: Option<usize>,
}

pub fn question(strings: &Strings, partner: &str) -> String {
    let record = |n: usize| strings.translate(&format!("ss_{n}"));

    compose(
        &record(trade::WITH_STRDEF),
        &record(trade::QUESTION_STRDEF),
        partner,
    )
}

pub fn compose(with: &str, ask: &str, partner: &str) -> String {
    let with = match with.find("%s") {
        Some(at) => format!("{}{partner}{}", &with[..at], &with[at + 2..]),
        None => with.to_string(),
    };

    format!("{with}{}{ask}", ui::text::FF_BREAK)
}

pub fn request(strings: &Strings, partner: &str) -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: BOX_KIND,
        context: BOX_CONTEXT,
        body: msgbox::Body::Text(question(strings, partner)),
        flags: BOX_FLAGS,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Offered {
    pub entity: u16,
    pub local: bool,
    pub arm: Option<Arm>,
    pub partner: u16,
    pub known: bool,
    pub changed: bool,
    pub opened: bool,
    pub raised: Option<Raise>,
    pub items: usize,
    pub gold: u32,
}

pub fn offer(scene: &mut FieldScene, entity: u16, offer: &Offer) -> Offered {
    finish_panel_close(scene);

    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let partner = offer.partner;

    let known = partner != 0 && scene.world.actors.entity(partner).is_some();

    let refused = Offered {
        entity,
        local,
        arm: None,
        partner,
        known,
        changed: false,
        opened: false,
        raised: None,
        items: 0,
        gold: 0,
    };

    if !local
        || scene
            .panels
            .as_ref()
            .is_none_or(|p| p.screen(CONTENT).is_none())
    {
        return refused;
    }

    if !scene.trade.engaged() && partner != 0 {
        if !known {
            return Offered {
                arm: Some(Arm::Request),
                ..refused
            };
        }

        scene.trade.mine = offer.clone();

        scene.trade.mine.partner = 0;

        scene.trade.mine.gold = 0;

        let name = name_of(scene, partner);

        let panels = scene
            .panels
            .as_mut()
            .expect("the CONTENT guard above found a panel");

        let request = request(&panels.strings, &name);

        let body = match &request.body {
            msgbox::Body::Text(text) => text.clone(),
            other => other.text(&panels.strings),
        };

        let slot = panels.raise_entity_box(&request, partner);

        return Offered {
            arm: Some(Arm::Request),
            raised: Some(Raise {
                kind: BOX_KIND,
                context: BOX_CONTEXT,
                flags: BOX_FLAGS,
                subject: partner,
                body,
                slot,
            }),
            ..refused
        };
    }

    let changed = offer.differs_from(&scene.trade.theirs);

    if changed {
        scene.trade.stamp_ms = scene.time.now_ms;

        scene.trade.mine.ready = 0;
    }

    let opened = open_window(scene);

    scene.trade.theirs = offer.clone();

    if known {
        scene.trade.partner_name = bracketed(name_of(scene, partner));

        scene.trade.my_name = bracketed(local_name(scene));

        if let Some(panels) = scene.panels.as_mut() {
            panels.set_open(super::panels::CONTENT_INVENTORY, true);

            panels.pair_inventory(CONTENT, true);
        }
    }

    refresh_display(scene);

    Offered {
        arm: Some(Arm::Contents),
        changed,
        opened,
        items: offer.filled(),
        gold: offer.gold,
        ..refused
    }
}

fn open_window(scene: &mut FieldScene) -> bool {
    let Some(panels) = scene.panels.as_mut() else {
        return false;
    };

    if panels.is_open(CONTENT) {
        return false;
    }

    for content in OPENING_CLOSES {
        panels.set_open(content, false);
    }

    panels.set_open(CONTENT, true);

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Closed {
    pub entity: u16,
    pub local: bool,
    pub closed_trade: bool,
    pub closed_store: bool,
}

pub fn closed(scene: &mut FieldScene, entity: u16) -> Closed {
    finish_panel_close(scene);

    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    if !local {
        return Closed {
            entity,
            local,
            closed_trade: false,
            closed_store: false,
        };
    }

    scene.trade.mine.partner = 0;

    scene.trade.mine.ready = 0;

    let closed_trade = scene.panels.as_ref().is_some_and(|p| p.is_open(CONTENT));

    if closed_trade {
        scene.trade.clear();

        if let Some(panels) = scene.panels.as_mut() {
            panels.close_by_toggle(CONTENT);
        }
    }

    let closed_store = scene
        .panels
        .as_ref()
        .is_some_and(|p| p.is_open(super::panels::CONTENT_STORE));

    super::store::close(scene);

    if let Some(panels) = scene.panels.as_mut() {
        panels.close_by_toggle(super::panels::CONTENT_STORE);
    }

    super::store::finish_panel_close(scene);

    finish_panel_close(scene);

    Closed {
        entity,
        local,
        closed_trade,
        closed_store,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Toggled {
    pub had_control: bool,
    pub mirrored: bool,
}

pub fn toggle(scene: &mut FieldScene) -> Toggled {
    finish_panel_close(scene);

    let had_control = scene
        .panels
        .as_ref()
        .is_some_and(|p| p.screen(CONTENT).is_some());

    let before = scene.trade.mine.ready;

    scene.trade.ready_control = true;

    refresh_checks(scene);

    Toggled {
        had_control,
        mirrored: scene.trade.mine.ready != before,
    }
}

pub fn accept(scene: &mut FieldScene, partner: u16) -> Option<Vec<u8>> {
    finish_panel_close(scene);

    let panels = scene.panels.as_mut()?;

    if panels.screen(CONTENT).is_none() || panels.is_open(CONTENT) {
        return None;
    }

    if partner == 0 || scene.world.actors.entity(partner).is_none() {
        let text = panels.strings.translate("ss_37");

        super::notice::post(panels, &text, 2000, true, scene.time.now_ms);

        return None;
    }

    panels.set_open(CONTENT, true);

    panels.set_open(super::panels::CONTENT_INVENTORY, true);

    panels.pair_inventory(CONTENT, true);

    scene.trade.mine.partner = partner;

    scene.trade.theirs = Offer::cleared();

    scene.trade.partner_name = bracketed(name_of(scene, partner));

    scene.trade.my_name = bracketed(local_name(scene));

    refresh_display(scene);

    Some(scene.trade.mine.to_frame().to_vec())
}

pub fn panel_accepts(scene: &mut FieldScene) -> Option<Vec<u8>> {
    let partner = scene
        .panels
        .as_mut()
        .and_then(|panels| panels.trade_accepted.take())?;

    accept(scene, partner)
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Ready {
    Cooling,
    Sent { ready: bool, frame: Vec<u8> },
}

pub fn press_ready(scene: &mut FieldScene, now_ms: u32) -> Ready {
    finish_panel_close(scene);

    if now_ms < scene.trade.stamp_ms.wrapping_add(READY_COOLDOWN_MS) {
        scene.trade.stamp_ms = now_ms;

        if let Some(panels) = scene.panels.as_mut() {
            let text = panels.strings.translate("ss_41");

            super::notice::post(panels, &text, 2000, true, now_ms);
        }

        return Ready::Cooling;
    }

    let checked = scene
        .panels
        .as_ref()
        .and_then(|p| p.screen(CONTENT))
        .and_then(|p| p.element("CheckMy", formats::ui::pane::ElementType::CheckBox))
        .and_then(|e| e.widget.button())
        .map_or(scene.trade.ready_control, |b| b.checked);

    let ready = !checked;

    scene.trade.ready_control = ready;

    scene.trade.mine.ready = u8::from(ready);

    if let Some(state) = scene
        .panels
        .as_mut()
        .and_then(|p| p.screen_mut(CONTENT))
        .and_then(|p| p.element_mut("CheckMy", formats::ui::pane::ElementType::CheckBox))
        .and_then(|e| e.widget.button_mut())
    {
        state.checked = ready;
    }

    let frame = scene.trade.mine.to_frame().to_vec();

    scene.trade.stamp_ms = now_ms;

    Ready::Sent { ready, frame }
}

pub fn offer_gold(scene: &mut FieldScene, gold: u32) -> Vec<u8> {
    finish_panel_close(scene);

    scene.trade.stamp_ms = scene.time.now_ms;

    scene.trade.ready_control = false;

    scene.trade.theirs.ready = 0;

    scene.trade.mine.ready = 0;

    scene.trade.mine.gold = gold;

    refresh_checks(scene);

    if let Some(screen) = scene.panels.as_mut().and_then(|p| p.screen_mut(CONTENT)) {
        super::screens::text(
            screen,
            "MoneyMy",
            &super::screens::thousands(u64::from(gold)),
        );
    }

    scene.trade.mine.to_frame().to_vec()
}

fn refresh_display(scene: &mut FieldScene) {
    scene.trade.ready_control = scene.trade.mine.ready != 0;

    if let Some(super::panels::Panels {
        screens,
        items,
        icons,
        ..
    }) = scene.panels.as_mut()
        && let Some(screen) = screens.iter_mut().find(|s| s.content == CONTENT)
    {
        super::screens::trade_fill(screen, &scene.trade, items, icons);
    }
}

fn refresh_checks(scene: &mut FieldScene) {
    if let Some(screen) = scene.panels.as_mut().and_then(|p| p.screen_mut(CONTENT)) {
        super::screens::trade_checks(screen, &scene.trade);
    }
}

#[derive(Debug, Clone, Copy)]

pub struct ItemClick {
    pub source: usize,
    pub owner: ui::element::Instance,
}

pub fn panel_items(scene: &mut FieldScene) {
    finish_panel_close(scene);

    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    let sources = std::mem::take(&mut panels.trade_item_clicks);

    for ItemClick { source, owner } in sources {
        let available = scene
            .panels
            .as_ref()
            .and_then(|p| p.screen(super::panels::CONTENT_INVENTORY))
            .filter(|p| p.open)
            .and_then(|p| {
                p.placed
                    .ui
                    .panel
                    .elements
                    .iter()
                    .find(|e| e.instance == owner)
            })
            .and_then(|e| e.widget.button())
            .is_some_and(|b| !b.icon_locked);

        if !available || source >= 60 {
            continue;
        }

        let Some(slot) = scene
            .trade
            .mine
            .items
            .iter()
            .position(|item| item.is_empty())
        else {
            continue;
        };

        let item = scene.entered.carry[source];

        if item.is_empty() {
            continue;
        }

        scene.trade.mine.items[slot] = item;

        scene.trade.mine.source_slots[slot] = source as u8;

        if let Some(bag) = scene
            .panels
            .as_mut()
            .and_then(|p| p.screen_mut(super::panels::CONTENT_INVENTORY))
            && let Some(button) = bag
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == owner)
                .and_then(|e| e.widget.button_mut())
        {
            button.icon_locked = true;
        }

        scene.trade.mine.ready = 0;

        scene.trade.stamp_ms = scene.time.now_ms;

        scene.outbox.push(scene.trade.mine.to_frame().to_vec());

        if scene.panels.as_ref().is_some_and(|p| p.is_open(CONTENT)) {
            scene.trade.theirs.ready = 0;

            refresh_display(scene);
        }

        tracing::debug!(
            source,
            offer_slot = slot,
            "staged inventory item in trade offer"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Closing {
    pub frame: Option<Vec<u8>>,
    pub was_open: bool,
    pub bag_closed: bool,
}

pub fn close(scene: &mut FieldScene) -> Closing {
    finish_panel_close(scene);

    let was_open = scene.panels.as_ref().is_some_and(|p| p.is_open(CONTENT));

    if !was_open {
        return Closing {
            frame: None,
            was_open: false,
            bag_closed: false,
        };
    }

    let bag_closed = scene
        .panels
        .as_ref()
        .is_some_and(|p| p.is_open(super::panels::CONTENT_INVENTORY));

    let frame = scene.trade.engaged().then(|| trade::close_frame().to_vec());

    scene.trade.clear();

    if let Some(panels) = scene.panels.as_mut() {
        panels.close_by_toggle(CONTENT);

        if bag_closed {
            panels.close_by_toggle(super::panels::CONTENT_INVENTORY);
        }
    }

    finish_panel_close(scene);

    Closing {
        frame,
        was_open,
        bag_closed,
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
        .is_some_and(|p| std::mem::take(&mut p.trade_closed_pending));

    if !pending {
        return;
    }

    if scene.trade.engaged() {
        scene.outbox.push(trade::close_frame().to_vec());
    }

    scene.trade.clear();

    scene.trade.stamp_ms = scene.time.now_ms;
}

fn bracketed(name: String) -> String {
    match name.is_empty() {
        true => String::new(),
        false => format!("[{name}]"),
    }
}

fn local_name(scene: &FieldScene) -> String {
    scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map(|actor| actor.name.clone())
        .unwrap_or_default()
}

fn name_of(scene: &FieldScene, entity: u16) -> String {
    scene
        .world
        .actors
        .entity(entity)
        .and_then(|index| scene.world.actors.actors.get(index))
        .map(|actor| actor.name.clone())
        .unwrap_or_default()
}

pub fn describe(applied: &Offered) -> String {
    match applied.arm {
        None if !applied.local => {
            format!(
                "NWYD: 0x383 for entity {} is not the local player",
                applied.entity
            )
        }
        None => "NWYD: 0x383 arrived with no PanelTrade to carry it".to_string(),
        Some(Arm::Request) => match applied.raised.is_some() {
            true => {
                format!("NWYD: 0x383 — entity {} wants to trade", applied.partner)
            }
            false => {
                format!(
                    "NWYD: 0x383 — a trade request naming unknown {}",
                    applied.partner
                )
            }
        },
        Some(Arm::Contents) => {
            format!(
                "NWYD: 0x383 — partner {} offers {} item(s) and {} gold{}{}",
                applied.partner,
                applied.items,
                applied.gold,
                match applied.changed {
                    true => ", changed",
                    false => "",
                },
                match applied.opened {
                    true => ", window raised",
                    false => "",
                },
            )
        }
    }
}

pub fn describe_close(applied: &Closed) -> String {
    if !applied.local {
        return format!(
            "NWYD: 0x384 for entity {} is not the local player",
            applied.entity
        );
    }

    match (applied.closed_trade, applied.closed_store) {
        (true, true) => "NWYD: 0x384 — the trade window and the shop are down".to_string(),
        (true, false) => "NWYD: 0x384 — the trade window is down".to_string(),
        (false, true) => "NWYD: 0x384 — the shop is down".to_string(),
        (false, false) => "NWYD: 0x384 — both windows were already down".to_string(),
    }
}
