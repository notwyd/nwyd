use super::*;
use crate::scene::consume::{self, Which};
use formats::data::item_list::{self, field};
use formats::net::account::Item;

const MP_POTION: u16 = 405;
const RECOVERY_JEWEL: u16 = 3203;
const JEWEL: u16 = 3200;
const POSITION: (u16, u16) = (2100, 2100);

fn items() -> ItemList {
    let mut bytes = vec![0; item_list::BODY_LEN];

    for (id, kind) in [(MP_POTION, 1i16), (RECOVERY_JEWEL, 0xF3), (JEWEL, 0xF2)] {
        let base = usize::from(id) * formats::data::ITEM_RECORD_LEN;

        bytes[base + field::EFFECT..base + field::EFFECT + 2]
            .copy_from_slice(&i16::from(item_move::EFFECT_KIND).to_le_bytes());

        bytes[base + field::EFFECT + 2..base + field::EFFECT + 4]
            .copy_from_slice(&kind.to_le_bytes());

        bytes[base + field::MAX_STACK..base + field::MAX_STACK + 2]
            .copy_from_slice(&120i16.to_le_bytes());
    }

    ItemList::from_plain(bytes, Codepage::Windows1252)
}

fn player() -> Entered {
    let packet = formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();

    Entered::from_packet(&packet, 0, None)
}

fn stack(index: u16, count: u16) -> Item {
    Item {
        index,
        appearance: count,
        ..Default::default()
    }
}

fn bar(id: u16) -> QuickBar {
    let mut bar = QuickBar::none();

    bar.ids[3] = u32::from(id);

    bar
}

#[test]
fn automatic_mp_consumption_does_not_block_jewel_quick_slots() {
    let items = items();
    let effects = Effects::new(&items, &[]);
    let live = item_use::Live {
        position: POSITION,
        ..Default::default()
    };

    for id in [RECOVERY_JEWEL, JEWEL] {
        let mut player = player();

        player.carry[0] = stack(id, 120);
        player.carry[1] = stack(MP_POTION, 120);

        let bar = bar(id);
        let mut cooldowns = Cooldowns::default();

        for step in 0..6u32 {
            let now = 3000 + step * 300;
            let drunk = consume::potion(&mut player, &items, &live, &mut cooldowns, now, Which::Mp);

            assert_eq!(
                drunk.sent,
                vec![(
                    1,
                    Request::Use {
                        slot: 1,
                        at: POSITION,
                    }
                )]
            );

            assert!(!cooldown_handled(
                &bar,
                &player,
                &effects,
                &cooldowns,
                3,
                now + 10,
            ));

            let actions = use_slot(
                &bar,
                &mut player,
                &items,
                &effects,
                &mut cooldowns,
                3,
                now + 10,
                POSITION,
            );

            assert_eq!(
                actions.first(),
                Some(&item_use::Action::Send(Request::Use {
                    slot: 0,
                    at: POSITION,
                }))
            );

            assert_eq!(player.carry[0], stack(id, 119 - step as u16));
            assert_eq!(player.carry[1], stack(MP_POTION, 119 - step as u16));
            assert_eq!(cooldowns.last[consume::MP_GROUP], now);
            assert_eq!(cooldowns.last[item_move::cooldown_bucket(0xF3)], now + 10);

            assert!(cooldown_handled(
                &bar,
                &player,
                &effects,
                &cooldowns,
                3,
                now + 20,
            ));

            assert!(
                use_slot(
                    &bar,
                    &mut player,
                    &items,
                    &effects,
                    &mut cooldowns,
                    3,
                    now + 20,
                    POSITION,
                )
                .is_empty()
            );

            assert_eq!(player.carry[0], stack(id, 119 - step as u16));
        }
    }
}

#[test]
fn jewel_cooldown_does_not_block_mp_consumption() {
    let items = items();
    let effects = Effects::new(&items, &[]);
    let mut player = player();

    player.carry[0] = stack(RECOVERY_JEWEL, 120);
    player.carry[1] = stack(MP_POTION, 120);

    let mut cooldowns = Cooldowns::default();

    assert!(
        !use_slot(
            &bar(RECOVERY_JEWEL),
            &mut player,
            &items,
            &effects,
            &mut cooldowns,
            3,
            3000,
            POSITION,
        )
        .is_empty()
    );

    let drunk = consume::potion(
        &mut player,
        &items,
        &item_use::Live {
            position: POSITION,
            ..Default::default()
        },
        &mut cooldowns,
        3010,
        Which::Mp,
    );

    assert_eq!(drunk.sent.len(), 1);
    assert_eq!(player.carry[0], stack(RECOVERY_JEWEL, 119));
    assert_eq!(player.carry[1], stack(MP_POTION, 119));
}

#[test]
fn five_hundred_jewels_across_inventory_stacks_spend_one_from_first_stack() {
    let items = items();
    let effects = Effects::new(&items, &[]);
    let mut player = player();

    for (slot, count) in [120, 120, 120, 120, 20].into_iter().enumerate() {
        player.carry[slot] = stack(RECOVERY_JEWEL, count);
    }

    let mut cooldowns = Cooldowns::default();
    let actions = use_slot(
        &bar(RECOVERY_JEWEL),
        &mut player,
        &items,
        &effects,
        &mut cooldowns,
        3,
        3000,
        POSITION,
    );

    assert_eq!(
        actions.first(),
        Some(&item_use::Action::Send(Request::Use {
            slot: 0,
            at: POSITION,
        }))
    );

    assert_eq!(player.carry[0], stack(RECOVERY_JEWEL, 119));

    for (slot, count) in [120, 120, 120, 20].into_iter().enumerate() {
        assert_eq!(player.carry[slot + 1], stack(RECOVERY_JEWEL, count));
    }

    assert_eq!(
        player.carry[..5]
            .iter()
            .map(consume::stored_count)
            .sum::<i32>(),
        499,
    );
}
