use super::field::FieldScene;
use super::panels::{CONTENT_CARGO, CONTENT_INVENTORY};
use formats::net::warehouse::{Balances, Change, Move};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub change: Change,
    pub amount: u32,
    pub before: Balances,
    pub after: Balances,
    pub cargo_open: bool,
    pub inventory_open: bool,
}

pub fn moved(scene: &mut FieldScene, moved: &Move) -> Moved {
    let before = Balances {
        carried: scene.entered.gold,
        warehouse: scene.entered.warehouse.gold,
    };

    let after = moved.apply(before);

    scene.entered.gold = after.carried;

    scene.entered.warehouse.gold = after.warehouse;

    let open = |content| {
        scene
            .panels
            .as_ref()
            .and_then(|panels| panels.screen(content))
            .is_some_and(|s| s.open)
    };

    Moved {
        change: moved.change,
        amount: moved.amount,
        before,
        after,
        cargo_open: open(CONTENT_CARGO),
        inventory_open: open(CONTENT_INVENTORY),
    }
}

pub fn describe(applied: &Moved) -> String {
    format!(
        "{} — {} coins {}: pocket {} -> {}, warehouse {} -> {}{}",
        match applied.change {
            Change::Set => "0x339 warehouse total",
            Change::Deposit => "0x388 warehouse deposit",
            Change::Withdraw => "0x387 warehouse withdrawal",
        },
        applied.amount,
        match applied.change {
            Change::Set => "stated",
            Change::Deposit => "in",
            Change::Withdraw => "out",
        },
        applied.before.carried,
        applied.after.carried,
        applied.before.warehouse,
        applied.after.warehouse,
        match (applied.cargo_open, applied.inventory_open) {
            (false, false) => ", with neither money label on screen",
            (true, false) => ", warehouse label on screen",
            (false, true) => ", pocket label on screen",
            (true, true) => ", both money labels on screen",
        },
    )
}
