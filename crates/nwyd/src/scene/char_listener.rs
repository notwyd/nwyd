use crate::scene::rng::Lcg;
use crate::scene::select_char::{
    CREATE_BACK, CREATE_SUBMIT, MENU_BACK, MENU_CREATE, MENU_DELETE, Mode, NOTHING_SELECTED_STRDEF,
    PANE_CREATE, PANE_MENU, PANE_PASSWORD, PANE_SEL_INFO, SEL_INFO_BACK, SEL_INFO_CONNECT,
    SelectChar,
};
use crate::scene::{connect, create, delete, keypad, lock, msgbox};
use assets::GameDir;
use formats::data::item_list::ItemList;
use ui::input::{EventId, Mouse};

pub struct Context<'a> {
    pub game: &'a GameDir,
    pub items: &'a ItemList,
    pub backbuffer: (u32, u32),
    pub now_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Raised {
    Send {
        what: &'static str,
        opcode: u16,
        body: Vec<u8>,
        slot: Option<usize>,
    },
    Message(usize),
    BackToServerSelect,
    Refused(&'static str),
}

pub fn time_key(scene: &mut SelectChar, key: ui::text_entry::Key, repeat: bool) -> Vec<Raised> {
    if key != ui::text_entry::Key::Insert
        || repeat
        || scene.legacy_boxes.active().is_some()
        || scene.panels.msgbox.placed.ui.panel.visible
        || scene.panels.focused_edit().is_some()
    {
        return Vec::new();
    }

    let mut body = vec![0; 160];

    body[4..6].copy_from_slice(&0x334u16.to_le_bytes());

    body[12..16].copy_from_slice(b"time");

    vec![Raised::Send {
        what: "character selection time",
        opcode: 0x334,
        body,
        slot: None,
    }]
}

pub fn modal_key(scene: &mut SelectChar, key: u16, rng: &mut Lcg) -> Option<Vec<Raised>> {
    if !scene.panels.msgbox.placed.ui.panel.visible {
        return None;
    }

    if key != 0x0D && scene.panels.msgbox.edit_mut().is_some() {
        return None;
    }

    Some(match scene.box_key(key) {
        Some((_, context, answer)) => answered(scene, context, answer, rng),
        None => Vec::new(),
    })
}

pub fn digit_button(name: &str) -> Option<usize> {
    let index: usize = name.strip_prefix("Btn")?.parse().ok()?;

    (index < keypad::DIGITS).then_some(index)
}

pub enum Shortcut {
    Waiting(&'static str),
    Ready(Vec<Raised>),
}

pub struct AccountPin {
    digits: Vec<u8>,
    entered: usize,
    sent: bool,
}

pub enum PinStep {
    Waiting(&'static str),
    Sent(Vec<Raised>),
    Unlocked,
}

impl AccountPin {
    pub fn new(pin: &str) -> Result<Self, &'static str> {
        if !(lock::PIN_MIN..=keypad::PIN_MAX).contains(&pin.len())
            || !pin.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("--pin must contain 4 to 6 ASCII digits");
        }

        Ok(Self {
            digits: pin.bytes().map(|b| b - b'0').collect(),
            entered: 0,
            sent: false,
        })
    }

    pub fn advance(
        &mut self,
        scene: &mut SelectChar,
        rng: &mut Lcg,
        ctx: &Context<'_>,
    ) -> Result<PinStep, &'static str> {
        if scene.lock == lock::UNLOCKED {
            return Ok(PinStep::Unlocked);
        }

        if self.sent {
            return if scene.lock == lock::AWAITING {
                Ok(PinStep::Waiting("waiting for account unlock reply"))
            } else {
                Err("account PIN unlock was rejected or reset; it will not be retried")
            };
        }

        if scene.lock == lock::AWAITING {
            return Ok(PinStep::Waiting("waiting for manual account unlock reply"));
        }

        if scene.lock != lock::LOCKED {
            return Err("account PIN shortcut cannot authorize deletion or a PIN change");
        }

        if scene.account.is_none() {
            return Ok(PinStep::Waiting("waiting for account data"));
        }

        if scene.legacy_boxes.active().is_some()
            || scene.name_dialog.visible
            || scene.name_dialog.pending
            || scene.boxes.topmost().is_some()
            || scene.panels.msgbox.placed.ui.panel.visible
        {
            return Ok(PinStep::Waiting("modal dialog requires attention"));
        }

        if scene.mode != Mode::Select {
            return Ok(PinStep::Waiting("return to character selection"));
        }

        if !scene.panels.password().ui.panel.visible {
            scene.pin_reopen(Mouse::LeftDown, ctx.now_ms, rng);

            if !scene.panels.password().ui.panel.visible {
                return Ok(PinStep::Waiting("waiting for account keypad"));
            }
        }

        if scene.pad.typed != self.digits[..self.entered] {
            return Err("account PIN entry changed during the shortcut; it will not be submitted");
        }

        if let Some(&digit) = self.digits.get(self.entered) {
            let button = scene
                .pad
                .labels
                .iter()
                .position(|&label| label == digit)
                .ok_or("account keypad digit is unavailable")?;

            if scene.pin_press(button, ctx.now_ms) {
                self.entered += 1;
            }

            return Ok(PinStep::Waiting("entering account PIN"));
        }

        if scene.button_enabled(PANE_PASSWORD, keypad::OK) != Some(true) {
            return Ok(PinStep::Waiting("account keypad Ok button is disabled"));
        }

        let raised = clicked(scene, Some(PANE_PASSWORD), keypad::OK, rng, ctx);

        if scene.lock != lock::AWAITING {
            return Err("account PIN unlock could not be submitted");
        }

        self.sent = true;

        self.digits.clear();

        Ok(PinStep::Sent(raised))
    }
}

pub fn select_and_connect(
    scene: &mut SelectChar,
    slot: usize,
    rng: &mut Lcg,
    ctx: &Context<'_>,
) -> Result<Shortcut, &'static str> {
    let Some(row) = scene.account.as_ref().and_then(|b| b.slots.get(slot)) else {
        return Err("character slot is unavailable");
    };

    if row.name.first().is_none_or(|b| *b == 0) {
        return Err("character slot is empty");
    }

    if !scene.world.actors.actors.iter().any(|a| a.record == slot) {
        return Err("character slot has no loaded doll");
    }

    if scene.lock != lock::UNLOCKED || scene.panels.password().ui.panel.visible {
        return Ok(Shortcut::Waiting("account PIN must be unlocked"));
    }

    if scene.legacy_boxes.active().is_some()
        || scene.name_dialog.visible
        || scene.name_dialog.pending
    {
        return Ok(Shortcut::Waiting(
            "character-name dialog requires attention",
        ));
    }

    if scene.panels.msgbox.placed.ui.panel.visible || scene.boxes.topmost().is_some() {
        return Ok(Shortcut::Waiting("modal dialog requires attention"));
    }

    if scene.mode != super::select_char::Mode::Select {
        return Ok(Shortcut::Waiting("return to character selection"));
    }

    if scene.selected != Some(slot) {
        if scene.selected.is_some() {
            return Err("another character was selected manually");
        }

        if scene.name_dialog.prompt_required && !scene.name_dialog.prompt_due(ctx.now_ms) {
            return Ok(Shortcut::Waiting("character-name prompt is not ready"));
        }

        let strings = scene.panels.strings.clone();

        if !scene.select_slot(slot, ctx.items, &strings, ctx.now_ms) {
            return Ok(Shortcut::Waiting("character selection is not ready"));
        }
    }

    if scene.legacy_boxes.active().is_some()
        || scene.boxes.topmost().is_some()
        || scene.name_dialog.visible
    {
        return Ok(Shortcut::Waiting(
            "character-name dialog requires attention",
        ));
    }

    if scene.button_enabled(PANE_SEL_INFO, SEL_INFO_CONNECT) != Some(true) {
        return Ok(Shortcut::Waiting("Connect button is disabled"));
    }

    Ok(Shortcut::Ready(clicked(
        scene,
        Some(PANE_SEL_INFO),
        SEL_INFO_CONNECT,
        rng,
        ctx,
    )))
}

pub fn mouse(
    scene: &mut SelectChar,
    event: Mouse,
    x: i32,
    y: i32,
    rng: &mut Lcg,
    ctx: &Context<'_>,
) -> Vec<Raised> {
    scene.sync_name_focus();

    scene.pointer(Some((x, y)));

    if scene.legacy_boxes.active().is_some() {
        scene.legacy_boxes.mouse_with_capture(
            event,
            (x, y),
            &scene.layout,
            ctx.now_ms,
            scene.name_dialog.owns_drag_capture(),
        );

        return Vec::new();
    }

    if scene.name_dialog.visible {
        if let Some(id) = scene
            .name_dialog
            .mouse(event, x, y, &scene.layout, ctx.now_ms)
        {
            let raised = match id {
                0x274 => scene
                    .submit_name(ctx.now_ms)
                    .into_iter()
                    .map(|body| Raised::Send {
                        what: "character name",
                        opcode: 0xFAA,
                        body,
                        slot: None,
                    })
                    .collect(),
                0x275 => {
                    scene.name_dialog.cancel();

                    Vec::new()
                }
                _ => Vec::new(),
            };

            scene.name_dialog.finish_mouse(id, ctx.now_ms);

            return raised;
        }

        return Vec::new();
    }

    let message_cap = super::message_panel::consumes_mouse(
        &scene.message_panel.state,
        &scene.layout,
        event,
        x,
        y,
    );

    if !message_cap && scene.pin_reopen(event, ctx.now_ms, rng) {
        return Vec::new();
    }

    let scale = scene.layout.ui_scale();

    let (px, py) = ((x as f32 / scale) as i32, (y as f32 / scale) as i32);

    if scene.panels.msgbox.placed.ui.panel.visible {
        if matches!(event, Mouse::LeftDown)
            && let Some((slot, yes)) = scene.box_hit(px, py)
            && let Some((_, context, answer)) = scene.answer_box(slot, yes)
        {
            return answered(scene, context, answer, rng);
        }

        return Vec::new();
    }

    let events = scene.panels.mouse(event, px, py);

    let raised = widget_events(scene, &events, rng, ctx);

    if !events.is_empty() || message_cap {
        return raised;
    }

    if scene.lock == lock::LOCKED {
        return raised;
    }

    if matches!(event, Mouse::LeftUp)
        && x > 0
        && y > 0
        && (x as u32) < ctx.backbuffer.0
        && (y as u32) < ctx.backbuffer.1
    {
        let strings = scene.panels.strings.clone();

        scene.click(x, y, ctx.backbuffer, ctx.items, &strings, ctx.now_ms);
    }

    raised
}

pub fn widget_events(
    scene: &mut SelectChar,
    events: &[ui::input::Event],
    rng: &mut Lcg,
    ctx: &Context<'_>,
) -> Vec<Raised> {
    let order = scene.panels.layers();

    let mut raised = Vec::new();

    for ui in events {
        if ui.id.command_id() != EventId::Click {
            continue;
        }

        let pane = order.get(ui.panel).copied().flatten();

        let actions = clicked(scene, pane, &ui.name, rng, ctx);

        for action in &actions {
            if let Raised::Message(record) = action {
                let text = scene.panels.strings.translate(&format!("ss_{record}"));

                scene.notice.post(&text, 2000, true, ctx.now_ms);
            }
        }

        raised.extend(actions);
    }

    raised
}

fn clicked(
    scene: &mut SelectChar,
    pane: Option<usize>,
    name: &str,
    rng: &mut Lcg,
    ctx: &Context<'_>,
) -> Vec<Raised> {
    match (pane, name) {
        (Some(PANE_SEL_INFO), SEL_INFO_CONNECT) => {
            let slot = scene.selected;

            match scene.connect_pressed(rng, ctx.now_ms) {
                connect::Pressed::Send(body) => {
                    vec![Raised::Send {
                        what: "select character",
                        opcode: connect::OPCODE,
                        body: body.to_vec(),
                        slot,
                    }]
                }
                connect::Pressed::NoSlot => {
                    vec![Raised::Message(NOTHING_SELECTED_STRDEF)]
                }
                connect::Pressed::Empty => {
                    vec![Raised::Refused("that slot holds no character")]
                }
            }
        }
        (Some(PANE_SEL_INFO), SEL_INFO_BACK) => {
            scene.back(ctx.now_ms);

            Vec::new()
        }
        (Some(PANE_CREATE), CREATE_SUBMIT) => {
            let typed = scene.panels.create_name();

            match scene.create_clicked(&typed, ctx.now_ms) {
                create::Pressed::Send(body) => {
                    vec![Raised::Send {
                        what: "create character",
                        opcode: create::CREATE_OPCODE,
                        body: body.to_vec(),
                        slot: None,
                    }]
                }
                create::Pressed::BadName(reject) => {
                    vec![Raised::Message(reject.strdef())]
                }
                create::Pressed::NoClass => {
                    vec![Raised::Message(create::NO_CLASS_STRDEF)]
                }
                create::Pressed::NoSlot => Vec::new(),
            }
        }
        (Some(PANE_CREATE), CREATE_BACK) => {
            if !scene.create_back(ctx.now_ms) {
                scene.set_mode(Mode::Select, ctx.now_ms);
            }

            Vec::new()
        }
        (Some(PANE_MENU), MENU_CREATE) => {
            if scene.lock != lock::LOCKED {
                scene.set_mode(Mode::Create, ctx.now_ms);
            }

            Vec::new()
        }
        (Some(PANE_MENU), MENU_DELETE) => match scene.delete_box() {
            Some(request) => {
                scene.raise(&request);

                Vec::new()
            }
            None => vec![Raised::Refused("the account is locked")],
        },
        (Some(PANE_MENU), MENU_BACK) => {
            if let Some(request) = scene.menu_back(ctx.now_ms) {
                scene.raise(&request);
            }

            Vec::new()
        }
        (Some(PANE_PASSWORD), name) => keypad_pane(scene, name, ctx),
        _ => Vec::new(),
    }
}

fn keypad_pane(scene: &mut SelectChar, name: &str, ctx: &Context<'_>) -> Vec<Raised> {
    if let Some(button) = digit_button(name) {
        scene.pin_press(button, ctx.now_ms);

        return Vec::new();
    }

    match name {
        keypad::OK => match scene.pin_ok(ctx.now_ms) {
            lock::Ok::Unlock(body) => {
                vec![Raised::Send {
                    what: "unlock the account",
                    opcode: lock::OPCODE,
                    body: body.to_vec(),
                    slot: None,
                }]
            }
            lock::Ok::Delete(body) => {
                vec![Raised::Send {
                    what: "delete character",
                    opcode: delete::DELETE_OPCODE,
                    body: body.to_vec(),
                    slot: None,
                }]
            }
            lock::Ok::TooShort => vec![Raised::Refused("the PIN is too short")],
            lock::Ok::Ignored => Vec::new(),
        },
        keypad::CHANGE => match scene.pin_change(ctx.now_ms) {
            lock::Change::Submit(body) | lock::Change::Confirm(body) => {
                vec![Raised::Send {
                    what: "change the account PIN",
                    opcode: lock::OPCODE,
                    body: body.to_vec(),
                    slot: None,
                }]
            }
            lock::Change::TooShort => vec![Raised::Refused("the PIN is too short")],
            lock::Change::Saved | lock::Change::Mismatch | lock::Change::Ignored => Vec::new(),
        },
        keypad::CANCEL => {
            scene.pin_cancel();

            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn answered(scene: &mut SelectChar, context: u32, answer: i32, rng: &mut Lcg) -> Vec<Raised> {
    if scene.message_box_answer(context, answer, rng) {
        return Vec::new();
    }

    if msgbox::answered(answer) && context == msgbox::CONTEXT_BACK {
        return vec![Raised::BackToServerSelect];
    }

    if msgbox::answered(answer) && context == msgbox::CONTEXT_21F {
        let mut body = vec![0; 16];

        body[4..6].copy_from_slice(&0x21Fu16.to_le_bytes());

        let slot = scene
            .selected
            .filter(|slot| *slot < 4)
            .map_or(-1, |slot| slot as i32);

        body[12..16].copy_from_slice(&slot.to_le_bytes());

        return vec![Raised::Send {
            what: "cancel pending character deletion",
            opcode: 0x21F,
            body,
            slot: None,
        }];
    }

    Vec::new()
}

pub fn received(scene: &mut SelectChar, opcode: u16, rng: &mut Lcg) -> lock::Reply {
    scene.lock_reply(opcode, rng)
}
