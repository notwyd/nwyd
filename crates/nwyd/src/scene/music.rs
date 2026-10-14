use super::field::FieldScene;

pub const CASTLE_WAR: i32 = 10;

pub const FIELD_1_RECT: (i32, i32, i32, i32) = (2048, 1792, 2688, 2304);

pub const FIELD_2_RECT: (i32, i32, i32, i32) = (1920, 1408, 2688, 2048);

pub const KINGDOM_RECT: (i32, i32, i32, i32) = (1665, 1537, 1792, 1920);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Switched {
    Nothing,
    Field { slot: usize },
    Arena { slot: usize },
}

fn contains(rect: (i32, i32, i32, i32), cell: (i32, i32)) -> bool {
    let (left, top, right, bottom) = rect;

    (left..right).contains(&cell.0) && (top..bottom).contains(&cell.1)
}

fn field_rect_row(cell: (i32, i32)) -> usize {
    if contains(FIELD_1_RECT, cell) {
        0
    } else if contains(FIELD_2_RECT, cell) {
        1
    } else {
        2
    }
}

pub fn field_slot(cell: (i32, i32), env_global: i32, counter_visible: bool) -> i32 {
    let x_block = cell.0 >> 7;

    let z_block = cell.1 >> 7;

    if z_block > 25 {
        if (9..=15).contains(&x_block) {
            return 7;
        }

        if (x_block, z_block) == (18, 30) {
            return 12;
        }

        if (17..=19).contains(&x_block) && z_block >= 30 {
            return 11;
        }

        return 5;
    }

    if contains(KINGDOM_RECT, cell) {
        return 6;
    }

    if env_global == 3 || ((27..=30).contains(&x_block) && (21..=24).contains(&z_block)) {
        return 9;
    }

    let rect_row = field_rect_row(cell);

    let folded = rect_row.min(1);

    let mut slot = (folded as i32 + 1) * 2;

    if env_global == 4 && counter_visible {
        slot = 5;
    }

    slot
}

pub fn tick(
    scene: &mut FieldScene,
    player: Option<((i32, i32), bool)>,
    field_slot_before_city: i32,
) -> Switched {
    if scene.siege.at_war() {
        if scene.siege.arena_music_slot == CASTLE_WAR {
            return Switched::Nothing;
        }

        scene.siege.arena_music_slot = CASTLE_WAR;

        scene.pending_music = Some(CASTLE_WAR as usize);

        return Switched::Arena {
            slot: CASTLE_WAR as usize,
        };
    }

    let Some((cell, safe)) = player else {
        return Switched::Nothing;
    };

    let slot = match safe {
        true => super::city::music_slot(cell),
        false => field_slot(
            cell,
            scene.world.env_global,
            scene.event_hud.counter.visible,
        ),
    };

    scene.city.music_slot = slot;

    if field_slot_before_city == slot {
        return Switched::Nothing;
    }

    scene.pending_music = Some(slot as usize);

    Switched::Field {
        slot: slot as usize,
    }
}

pub fn describe(switched: &Switched) -> Option<String> {
    match switched {
        Switched::Nothing => None,
        Switched::Field { slot } => Some(format!(
            "NWYD: field background music switched to slot {slot}"
        )),
        Switched::Arena { slot } => Some(format!(
            "NWYD: castle-war background music switched to slot {slot}"
        )),
    }
}
