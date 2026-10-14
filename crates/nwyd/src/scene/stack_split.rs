use super::{
    enter::Entered,
    panels::{CONTENT_COIN_INPUT, CONTENT_INVENTORY, Panels},
    screens,
};
use formats::net::account::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Prompt {
    pub slot: usize,
    pub item: Item,
    pub individual: bool,
}

pub fn begin(panels: &mut Panels, entered: &Entered, slot: usize) -> bool {
    begin_with_owner(panels, entered, slot, None)
}

pub(crate) fn begin_with_owner(
    panels: &mut Panels,
    entered: &Entered,
    slot: usize,
    owner: Option<ui::element::Instance>,
) -> bool {
    let Some(item) = entered
        .carry
        .get(slot)
        .copied()
        .filter(|i| slot < 60 && !i.is_empty())
    else {
        return false;
    };

    if panels.screen(CONTENT_COIN_INPUT).is_none() {
        return false;
    }

    panels.set_open(CONTENT_COIN_INPUT, true);

    panels.store_price = None;

    panels.seal_name_slot = None;

    panels.split_stack = Some(Prompt {
        slot,
        item,
        individual: false,
    });

    panels.coin_action = 12;

    let body = panels.strings.translate("ss_408");

    screens::coin_input_fill(panels.screen_mut(CONTENT_COIN_INPUT).unwrap(), &body);

    screens::coin_input_option(
        panels.screen_mut(CONTENT_COIN_INPUT).unwrap(),
        "Div",
        true,
        false,
    );

    if let Some(screen) = panels.screen_mut(CONTENT_INVENTORY) {
        if let Some(owner) = owner {
            if let Some(button) = screen
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
        } else {
            screens::inventory_lock(screen, slot, true);
        }
    }

    true
}

pub fn toggle_individual(panels: &mut Panels) {
    if !panels.is_open(CONTENT_COIN_INPUT) || panels.coin_action != 12 {
        return;
    }

    let Some(prompt) = panels.split_stack.as_mut() else {
        return;
    };

    prompt.individual = !prompt.individual;

    let checked = prompt.individual;

    if let Some(screen) = panels.screen_mut(CONTENT_COIN_INPUT) {
        screens::coin_input_option(screen, "Div", true, checked);
    }

    if checked {
        panels.raise_box(
            &super::msgbox::Request {
                caption: Some(237),
                kind: 0x27,
                context: 0x5b,
                body: super::msgbox::Body::StrDef(1662),
                flags: super::msgbox::FLAGS,
            },
            None,
        );
    }
}

pub fn answer(
    panels: &mut Panels,
    entered: &Entered,
    yes: bool,
    dead: bool,
    now_ms: u32,
) -> Option<Vec<u8>> {
    if !panels.is_open(CONTENT_COIN_INPUT) {
        return None;
    }

    let prompt = panels.split_stack?;

    let mut frame = None;

    if yes {
        if dead {
            return None;
        }

        let text = panels.coin_input_text().unwrap_or_default();

        if text.is_empty() || text == "all" {
            return None;
        }

        if !text.bytes().all(|b| b.is_ascii_digit()) {
            let message = panels.strings.translate("ss_409");

            super::notice::post(panels, &message, 1000, true, now_ms);

            return None;
        }

        let quantity = text.parse::<u32>().unwrap_or(u32::MAX);

        if entered.carry.get(prompt.slot) == Some(&prompt.item) {
            if quantity > 0
                && quantity < (formats::data::item_effect::stack_size(&prompt.item) as u32)
            {
                frame = if prompt.individual {
                    formats::net::inventory::split_individual_frame(
                        entered.client_id,
                        prompt.slot,
                        prompt.item.index,
                        quantity,
                    )
                    .map(|frame| frame.to_vec())
                } else {
                    formats::net::inventory::split_frame(
                        entered.client_id,
                        prompt.slot,
                        prompt.item.index,
                        quantity,
                    )
                    .map(|frame| frame.to_vec())
                };
            } else {
                panels.coin_input_set_text("0".into());
            }
        }
    }

    if let Some(screen) = panels.screen_mut(CONTENT_INVENTORY) {
        screens::inventory_lock(screen, prompt.slot, false);
    }

    panels.split_stack = None;

    panels.set_open(CONTENT_COIN_INPUT, false);

    frame
}
