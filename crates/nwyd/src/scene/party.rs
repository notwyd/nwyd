use super::actors::{Actor, ActorScene};
use super::field::FieldScene;
use formats::net::party::{self, Leave, Marks, Member};

pub fn leave_frame(client: u16, target: u16) -> Vec<u8> {
    let mut out = vec![0; 16];

    out[4..6].copy_from_slice(&0x37Eu16.to_le_bytes());

    out[6..8].copy_from_slice(&client.to_le_bytes());

    out[12..16].copy_from_slice(&u32::from(target).to_le_bytes());

    out
}

pub fn panel_exit(scene: &mut FieldScene) {
    let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    let Some(screen) = scene
        .panels
        .as_mut()
        .and_then(|p| p.screen_mut(super::panels::CONTENT_PARTY))
    else {
        return;
    };

    scene.outbox.push(leave_frame(player.entity, 0));

    screen.set_open(false);
}

pub fn panel_member(scene: &mut FieldScene, row: usize) {
    let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    let client = player.entity;

    let Some(member) = scene.party.rows.get(row) else {
        return;
    };

    if scene
        .panels
        .as_ref()
        .and_then(|p| p.screen(super::panels::CONTENT_PARTY))
        .is_none()
    {
        return;
    }

    if scene
        .party
        .row_visual(row)
        .is_some_and(|v| v.state == RowState::InvitedPending)
    {
        let accepted = party::Accept::to(member, client);

        if let Some(index) = scene.world.actors.entity(member.id) {
            scene.world.actors.actors[index].tracked = true;
        }

        scene.outbox.push(accepted.to_frame().to_vec());

        return;
    }

    if member.id >= 1000 || !scene.hud.as_ref().is_some_and(|h| h.manager.keys.ctrl) {
        return;
    }

    let panels = scene.panels.as_mut().expect("registered party owner");

    let request = super::msgbox::Request {
        caption: Some(237),
        kind: 0xD,
        context: 0x37,
        body: super::msgbox::Body::Text(
            panels
                .strings
                .translate("ss_289")
                .replace("%s", &member.name),
        ),
        flags: 0x20000001,
    };

    panels.raise_entity_box(&request, member.id);
}

pub const AFFECT_CELLS: usize = formats::net::affect::AFFECTS;

pub const VANISHED_NAME_COLOR: u32 = 0xFF77_7777;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]

pub enum RowState {
    #[default]
    Active = 0,
    InvitedPending = 1,
    JoinedPending = 2,
    ActiveVanished = 3,
    JoinedPendingVanished = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct RowVisual {
    pub state: RowState,
    pub name_color: u32,
}

impl Default for RowVisual {
    fn default() -> Self {
        Self {
            state: RowState::Active,
            name_color: u32::MAX,
        }
    }
}

impl RowVisual {
    fn joined(member: &Member) -> Self {
        match member.pending {
            true => Self {
                state: RowState::JoinedPending,
                name_color: party::PENDING_NAME_COLOR,
            },
            false => Self::default(),
        }
    }

    fn invited(member: &Member) -> Self {
        Self {
            state: if member.pending {
                RowState::InvitedPending
            } else {
                RowState::Active
            },
            name_color: u32::MAX,
        }
    }

    fn vanish(&mut self) -> bool {
        self.state = match self.state {
            RowState::Active => RowState::ActiveVanished,
            RowState::JoinedPending => RowState::JoinedPendingVanished,
            RowState::InvitedPending
            | RowState::ActiveVanished
            | RowState::JoinedPendingVanished => return false,
        };

        self.name_color = VANISHED_NAME_COLOR;

        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct LiveRow {
    pub level: i16,
    pub hp: i32,
    pub max_hp: i32,
    pub affects: [Option<formats::net::affect::Affect>; AFFECT_CELLS],
}

pub fn live_row(actors: &ActorScene, id: u16) -> Option<LiveRow> {
    let actor = actors
        .entity(id)
        .and_then(|index| actors.actors.get(index))?;

    Some(LiveRow {
        level: actor.status.level as i16,
        hp: actor.status.hp as i32,
        max_hp: actor.status.max_hp as i32,
        affects: compact_affects(&actor.affects),
    })
}

pub fn compact_affects(
    affects: &[formats::net::affect::Affect; formats::net::affect::AFFECTS],
) -> [Option<formats::net::affect::Affect>; AFFECT_CELLS] {
    let mut compact = [None; AFFECT_CELLS];

    let mut filled = 0;

    for affect in affects {
        if !affect.is_set() {
            continue;
        }

        compact[filled] = Some(*affect);

        filled += 1;
    }

    compact
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct RowVitals {
    pub level: i16,
    pub hp: i32,
    pub max_hp: i32,
}

impl From<&Member> for RowVitals {
    fn from(member: &Member) -> Self {
        Self {
            level: member.level,
            hp: i32::from(member.hp),
            max_hp: i32::from(member.max_hp),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct State {
    pub rows: Vec<Member>,
    pub row_visuals: Vec<RowVisual>,
    pub row_vitals: Vec<RowVitals>,
    pub marks: [[u16; party::MARKS]; party::ROWS],
}

impl State {
    pub fn position(&self, id: u16) -> Option<usize> {
        self.rows.iter().position(|row| row.id == id)
    }

    pub fn row_visual(&self, at: usize) -> Option<RowVisual> {
        let member = self.rows.get(at)?;

        Some(
            self.row_visuals
                .get(at)
                .copied()
                .unwrap_or_else(|| RowVisual::joined(member)),
        )
    }

    fn repair_visuals(&mut self) {
        self.row_vitals.truncate(self.rows.len());

        while self.row_vitals.len() < self.rows.len() {
            self.row_vitals
                .push(RowVitals::from(&self.rows[self.row_vitals.len()]));
        }

        self.row_visuals.truncate(self.rows.len());

        while self.row_visuals.len() < self.rows.len() {
            let at = self.row_visuals.len();

            self.row_visuals.push(RowVisual::joined(&self.rows[at]));
        }
    }

    fn remove(&mut self, at: usize) -> Member {
        self.repair_visuals();

        self.row_vitals.remove(at);

        self.row_visuals.remove(at);

        self.rows.remove(at)
    }

    fn push(&mut self, member: Member, visual: RowVisual) {
        self.repair_visuals();

        self.row_vitals.push(RowVitals::from(&member));

        self.rows.push(member);

        self.row_visuals.push(visual);
    }

    pub fn vanish_row(&mut self, at: usize) -> bool {
        self.repair_visuals();

        self.row_visuals.get_mut(at).is_some_and(RowVisual::vanish)
    }

    pub fn clear_roster(&mut self) {
        self.rows.clear();

        self.row_visuals.clear();

        self.row_vitals.clear();
    }

    pub fn row_vitals(&self, at: usize) -> Option<RowVitals> {
        let row = self.rows.get(at)?;

        Some(
            self.row_vitals
                .get(at)
                .copied()
                .unwrap_or_else(|| RowVitals::from(row)),
        )
    }

    pub(super) fn row_vitals_mut(&mut self, at: usize) -> Option<&mut RowVitals> {
        self.repair_visuals();

        self.row_vitals.get_mut(at)
    }
}

pub fn vanished(scene: &mut FieldScene, entity: u16) -> bool {
    let Some(index) = scene.world.actors.entity(entity) else {
        return false;
    };

    let tracked = scene.world.actors.actors[index].tracked;

    if scene.party.rows.len() < 2 && tracked {
        scene.party.clear_roster();

        let actor = &mut scene.world.actors.actors[index];

        actor.tracked = false;

        actor.party_tint = None;

        if let Some(player) = scene.player
            && let Some(actor) = scene.world.actors.actors.get_mut(player)
        {
            actor.tracked = false;
        }

        return true;
    }

    let Some(row) = scene.party.position(entity) else {
        return false;
    };

    let actor = &mut scene.world.actors.actors[index];

    actor.tracked = false;

    actor.party_tint = None;

    let _ = scene.party.vanish_row(row);

    true
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Joined {
    pub member: Member,
    pub replaced: bool,
    pub rows: usize,
    pub marked: bool,
    pub raises: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Invited {
    pub member: Member,
    pub cleared: usize,
    pub tinted: bool,
    pub lines: Vec<usize>,
    pub accepted: Option<party::Accept>,
    pub hides_auto: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Leaving {
    pub leave: Leave,
    pub removed: Vec<u16>,
    pub rows: usize,
    pub dissolved_the_remainder: bool,
    pub unmarked: Vec<u16>,
    pub kingdom_zone: Vec<u16>,
    pub retinted: Vec<(u16, u32)>,
    pub markers_destroyed: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Marked {
    pub owner: u16,
    pub row: Option<usize>,
    pub on_roster: bool,
    pub marked: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Change {
    Joined(Joined),
    Invited(Invited),
}

impl Change {
    pub fn opcode(&self) -> u16 {
        match self {
            Change::Joined(_) => party::JOINED,
            Change::Invited(_) => party::INVITED,
        }
    }

    pub fn wire(&self) -> Option<&party::Accept> {
        match self {
            Change::Invited(invited) => invited.accepted.as_ref(),
            Change::Joined(_) => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Change::Joined(joined) => describe_joined(joined),
            Change::Invited(invited) => describe_invited(invited),
        }
    }
}

pub fn joined(scene: &mut FieldScene, member: &Member) -> Joined {
    let replaced = match scene.party.position(member.id) {
        Some(at) => {
            scene.party.remove(at);

            true
        }
        None => false,
    };

    scene.party.push(member.clone(), RowVisual::joined(member));

    let marked = mark(scene, member.id, true, Some(party::PARTY_TINT));

    if member.raises_the_window() {
        show_roster(scene);
    }

    Joined {
        member: member.clone(),
        replaced,
        rows: scene.party.rows.len(),
        marked,
        raises: member.raises_the_window(),
    }
}

pub(super) fn spawn_markers(scene: &mut FieldScene, index: usize, packet: (i16, i16), fresh: bool) {
    let actor = &mut scene.world.actors.actors[index];

    let entity = actor.entity;

    if fresh
        && entity >= 1000
        && (28..=30).contains(&((actor.position.x as i32) >> 7))
        && (27..=28).contains(&((actor.position.z as i32) >> 7))
        && Some(index) != scene.player
    {
        actor.party_tint = Some(0xaaff0000);
    }

    scene.party.repair_visuals();

    let mut revived = false;

    for row in 0..scene.party.rows.len() {
        if scene.party.rows[row].id == entity {
            if scene.party.row_visuals[row].state != RowState::InvitedPending {
                mark(scene, entity, true, Some(party::PARTY_TINT));
            }

            let visual = &mut scene.party.row_visuals[row];

            match visual.state {
                RowState::ActiveVanished => {
                    visual.state = RowState::Active;

                    visual.name_color = 0xffffffff;

                    revived = true;
                }
                RowState::JoinedPendingVanished => {
                    visual.state = RowState::JoinedPending;

                    visual.name_color = 0xffaaaaff;

                    revived = true;
                }
                _ => {}
            }

            break;
        }

        let hit = scene
            .party
            .marks
            .get(row)
            .and_then(|ids| ids.iter().position(|id| *id == entity));

        if let Some(slot) = hit
            && let Some(found) = scene.world.actors.entity(entity)
        {
            if scene.world.actors.actors[found].body as i16
                == scene.world.actors.actors[index].body as i16
            {
                mark(scene, entity, true, Some(party::PARTY_TINT));
            } else {
                scene.world.actors.actors[found].tracked = false;

                scene.party.marks[row][slot] = 0;
            }
        }
    }

    if kingdom_zone_at(f32::from(packet.0), f32::from(packet.1))
        && let Some(tint) = kingdom_tint(scene.world.actors.actors[index].kingdom)
    {
        scene.world.actors.actors[index].party_tint = Some(tint);
    }

    if revived
        && let Some(screen) = scene
            .panels
            .as_mut()
            .and_then(|panels| panels.screen_mut(0x13))
    {
        super::screens::party_refresh_in_arena(screen, &scene.party, scene.arena.visible);
    }
}

pub fn invited(scene: &mut FieldScene, member: &Member) -> Invited {
    let cleared = scene.party.rows.len();

    scene.party.clear_roster();

    scene.party.push(member.clone(), RowVisual::invited(member));

    show_roster(scene);

    let auto_join = scene.party_auto_join;

    let tinted = mark(scene, member.id, auto_join, Some(party::PARTY_TINT));

    let mut lines = vec![party::INVITE_STRDEF];

    let accepted = if auto_join {
        let accept = party::Accept::to(member, scene.entered.client_id);

        scene.outbox.push(accept.to_frame().to_vec());

        Some(accept)
    } else {
        lines.push(party::INVITE_HINT_STRDEF);

        None
    };

    if let Some(panels) = scene.panels.as_mut() {
        for strdef in &lines {
            let text = strdef_line(panels, *strdef, &member.name);

            super::chat::system(panels, &text, party::INVITE_CHAT_COLOR);
        }
    }

    Invited {
        member: member.clone(),
        cleared,
        tinted,
        lines,
        accepted,
        hides_auto: !auto_join,
    }
}

fn show_roster(scene: &mut FieldScene) {
    if let Some(screen) = scene
        .panels
        .as_mut()
        .and_then(|p| p.screen_mut(super::panels::CONTENT_PARTY))
    {
        screen.set_open(true);
    }
}

pub fn leaving(scene: &mut FieldScene, leave: &Leave) -> Leaving {
    let mut report = Leaving {
        leave: *leave,
        removed: vec![],
        rows: scene.party.rows.len(),
        dissolved_the_remainder: false,
        unmarked: vec![],
        kingdom_zone: vec![],
        retinted: vec![],
        markers_destroyed: vec![],
    };

    if scene.party.rows.is_empty() {
        return report;
    }

    let local = scene.entered.client_id;

    let saved = scene
        .party
        .position(local)
        .and_then(|row| scene.party.marks.get(row))
        .copied()
        .unwrap_or([0; party::MARKS]);

    match leave {
        Leave::Dissolved => {
            if let Some(player) = scene.player {
                scene.world.actors.actors[player].tracked = false;
            }

            report.removed = scene.party.rows.iter().map(|row| row.id).collect();

            for row in 0..scene.party.rows.len() {
                let id = scene.party.rows[row].id;

                leave_roster_body(scene, id, &mut report);

                if id != local
                    && let Some(ids) = scene.party.marks.get(row).copied()
                {
                    for marked in ids {
                        leave_marked_body(scene, marked, &mut report);
                    }
                }
            }

            scene.party.clear_roster();

            scene.party.marks.fill([0; party::MARKS]);

            scene.party.marks[0] = saved;
        }
        Leave::Member(id) => {
            if let Some(row) = scene
                .party
                .rows
                .iter()
                .position(|row| u32::from(row.id) == *id)
            {
                let entity = scene.party.rows[row].id;

                if leave_roster_body(scene, entity, &mut report)
                    && let Some(ids) = scene.party.marks.get(row).copied()
                {
                    for marked in ids {
                        leave_marked_body(scene, marked, &mut report);
                    }

                    scene.party.marks[row].fill(0);
                }

                scene.party.remove(row);

                report.removed.push(entity);
            } else {
                for row in 0..party::ROWS {
                    for slot in 0..party::MARKS {
                        let marked = scene.party.marks[row][slot];

                        if marked != 0 && u32::from(marked) == *id {
                            leave_marked_body(scene, marked, &mut report);

                            scene.party.marks[row][slot] = 0;
                        }
                    }
                }
            }
        }
    }

    report.dissolved_the_remainder = scene.party.rows.len() == 1;

    if report.dissolved_the_remainder {
        scene.party.clear_roster();

        if let Some(player) = scene.player {
            scene.world.actors.actors[player].tracked = false;
        }

        scene.party.marks.fill([0; party::MARKS]);

        scene.party.marks[0] = saved;
    }

    report.rows = scene.party.rows.len();

    report
}

fn leave_roster_body(scene: &mut FieldScene, id: u16, report: &mut Leaving) -> bool {
    let Some(index) = scene.world.actors.entity(id) else {
        return false;
    };

    let actor = &mut scene.world.actors.actors[index];

    actor.tracked = false;

    report.unmarked.push(id);

    if in_kingdom_zone(actor) {
        report.kingdom_zone.push(id);

        if let Some(tint) = kingdom_tint(actor.kingdom) {
            actor.party_tint = Some(tint);

            report.retinted.push((id, tint));
        }
    } else {
        actor.party_tint = None;

        report.markers_destroyed.push(id);
    }

    true
}

fn leave_marked_body(scene: &mut FieldScene, id: u16, report: &mut Leaving) {
    if id != 0 && mark(scene, id, false, Some(party::PARTY_TINT)) {
        report.unmarked.push(id);

        report.retinted.push((id, party::PARTY_TINT));
    }
}

pub fn marked(scene: &mut FieldScene, marks: &Marks) -> Marked {
    let position = scene.party.position(marks.owner);

    let row = Marks::row_for(position, scene.party.rows.len());

    let Some(row) = row else {
        return Marked {
            owner: marks.owner,
            row: None,
            on_roster: position.is_some(),
            marked: vec![],
        };
    };

    scene.party.marks[row] = marks.ids;

    let mut hit = Vec::new();

    for id in marks.ids {
        if id == 0 {
            continue;
        }

        if mark(scene, id, true, Some(party::PARTY_TINT)) {
            hit.push(id);
        }
    }

    Marked {
        owner: marks.owner,
        row: Some(row),
        on_roster: position.is_some(),
        marked: hit,
    }
}

fn strdef_line(panels: &super::panels::Panels, record: usize, name: &str) -> String {
    let template = panels
        .strings
        .strdef
        .as_ref()
        .map(|table| table.get(record).to_string())
        .unwrap_or_default();

    template.replacen("%s", name, 1)
}

fn mark(scene: &mut FieldScene, id: u16, tracked: bool, tint: Option<u32>) -> bool {
    let Some(index) = scene.world.actors.entity(id) else {
        return false;
    };

    let actor = &mut scene.world.actors.actors[index];

    actor.tracked = tracked;

    if let Some(tint) = tint {
        actor.party_tint = Some(tint);
    }

    true
}

pub fn in_kingdom_zone(actor: &Actor) -> bool {
    kingdom_zone_at(actor.position.x, actor.position.z)
}

pub fn kingdom_zone_at(x: f32, z: f32) -> bool {
    let cell = |v: f32| (f64::from(v) * party::ZONE_SCALE) as i32;

    let (x, z) = (cell(x), cell(z));

    party::KINGDOM_ZONES
        .iter()
        .any(|(cx, cz)| x == *cx as i32 && z == *cz as i32)
}

pub fn kingdom_tint(kingdom: u8) -> Option<u32> {
    match kingdom {
        1 => Some(party::KINGDOM_ONE_TINT),
        2 => Some(party::KINGDOM_TWO_TINT),
        _ => None,
    }
}

pub fn describe_joined(applied: &Joined) -> String {
    format!(
        "NWYD: 0x37D — {} (id {}) on the roster, {} row(s){}{}{}",
        applied.member.name,
        applied.member.id,
        applied.rows,
        if applied.replaced {
            ", replacing a row it already had"
        } else {
            ""
        },
        if applied.marked {
            ", body marked"
        } else {
            ", no body in this world"
        },
        if applied.raises {
            ""
        } else {
            ", and the id is a mob so the window stays down"
        },
    )
}

pub fn describe_invited(applied: &Invited) -> String {
    format!(
        "NWYD: 0x37F — {} (id {}) invites you; the roster's {} row(s) were replaced by one{}",
        applied.member.name,
        applied.member.id,
        applied.cleared,
        match &applied.accepted {
            Some(_) => ", and auto-join answered with a 0x3AB",
            None => ", awaiting a click in the party window",
        },
    )
}

pub fn describe_leaving(applied: &Leaving) -> String {
    let head = match applied.leave {
        Leave::Dissolved => "0x37E — the party dissolved".to_string(),
        Leave::Member(id) => format!("0x37E — member {id} left"),
    };

    format!(
        "NWYD: {head}: {} row(s) removed, {} left{}, {} body(s) un-marked{}",
        applied.removed.len(),
        applied.rows,
        if applied.dissolved_the_remainder {
            " (the last one went with it)"
        } else {
            ""
        },
        applied.unmarked.len(),
        match (applied.kingdom_zone.len(), applied.retinted.len()) {
            (0, _) => String::new(),
            (zones, tinted) =>
                format!(", {zones} of them in a kingdom zone and {tinted} re-tinted"),
        }
    )
}

pub fn describe_marked(applied: &Marked) -> String {
    match applied.row {
        None => {
            format!(
                "NWYD: 0x3EA for member {} would write row {} of {} — refused",
                applied.owner,
                party::ROWS,
                party::ROWS,
            )
        }
        Some(row) => {
            format!(
                "NWYD: 0x3EA — row {row} for member {}{}, {} of {} ids marked a body",
                applied.owner,
                if applied.on_roster {
                    ""
                } else {
                    " (not on the roster)"
                },
                applied.marked.len(),
                party::MARKS,
            )
        }
    }
}
