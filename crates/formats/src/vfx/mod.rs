pub mod dispatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub enum Chain {
    Impact,
    Caster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub enum Body {
    Caster,
    Target,
    Bystander,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]

pub enum PositionRef {
    Caster,
    Target,
    TargetCell,
    Midpoint,
    Offset(Anchor, [f32; 3]),
    Along {
        from: Anchor,
        to: Anchor,
        from_offset: [f32; 3],
        to_offset: [f32; 3],
        divisor: f32,
    },
    Composite([AxisRef; 3]),
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]

pub enum AxisRef {
    From(Anchor, f32),
    Computed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub enum Anchor {
    Caster,
    Target,
    TargetCell,
    Midpoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub enum SpawnArgument {
    Int(i32),
    Bits(u32),
    Float(f32),
    Pos(Axis),
    Toward(Axis),
    Allocation(u8),
    Body(Body),
    Scene(u32),
    Temporary,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PropertyOverride {
    pub offset: u16,
    pub bytes: u32,
    pub axis: Option<Axis>,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct EffectCommand {
    pub slot: u16,
    pub args: &'static [SpawnArgument],
    pub at: Option<PositionRef>,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Spawn {
    pub class: dispatch::Class,
    pub at: PositionRef,
    pub toward: PositionRef,
    pub size: u16,
    pub list: u32,
    pub arguments: &'static [SpawnArgument],
    pub property_overrides: &'static [PropertyOverride],
    pub commands: &'static [EffectCommand],
}

impl Spawn {
    pub fn equivalent_ignoring_allocation(&self, other: &Spawn) -> bool {
        let without_allocation_id = |a: &SpawnArgument| match a {
            SpawnArgument::Allocation(_) => SpawnArgument::Allocation(0),
            other => *other,
        };

        self.class == other.class
            && self.at == other.at
            && self.toward == other.toward
            && self.size == other.size
            && self.list == other.list
            && self.property_overrides == other.property_overrides
            && self.commands == other.commands
            && self.arguments.len() == other.arguments.len()
            && self
                .arguments
                .iter()
                .map(without_allocation_id)
                .eq(other.arguments.iter().map(without_allocation_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct RowFlags(pub u8);

impl RowFlags {
    pub const FAULTED: u8 = 1;

    pub const STUBBED: u8 = 2;

    pub const UNMODELLED_CALL: u8 = 4;

    pub const SUB_OBJECT: u8 = 8;

    pub fn has(self, mask: u8) -> bool {
        self.0 & mask == mask
    }

    pub fn is_clean(self) -> bool {
        self.0 == 0
    }
}

pub mod state {

    pub const VOLLEY_ARMED: u32 = 0x01;

    pub const HIDDEN: u32 = 0x02;

    pub const AFFECT_37: u32 = 0x04;

    pub const TARGET: u32 = 0x08;

    pub const MIDPOINT: u32 = 0x10;

    pub const SELF: u32 = 0x20;

    pub const COUNT: u32 = 0x40;
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct DispatchRow {
    pub chain: Chain,
    pub skill: u8,
    pub variants: u32,
    pub states: u64,
    pub spawn_ids: &'static [u16],
    pub sounds: &'static [u16],
    pub flags: RowFlags,
}

impl DispatchRow {
    pub fn spawns(&self) -> impl Iterator<Item = &'static Spawn> + '_ {
        self.spawn_ids
            .iter()
            .filter_map(|&i| dispatch::SPAWNS.get(usize::from(i)))
    }
}

pub fn row(chain: Chain, skill: u8, variant: u8, state: u32) -> Option<&'static DispatchRow> {
    if variant as u32 >= dispatch::VARIANTS || state >= state::COUNT {
        return None;
    }

    dispatch::ROWS.iter().find(|r| {
        r.chain == chain
            && r.skill == skill
            && r.variants & (1 << variant) != 0
            && r.states & (1u64 << state) != 0
    })
}

pub fn rows_for(chain: Chain, skill: u8) -> impl Iterator<Item = &'static DispatchRow> {
    dispatch::ROWS
        .iter()
        .filter(move |r| r.chain == chain && r.skill == skill)
}

pub fn supports_caster_effects(skill: i16) -> bool {
    (0..0x60).contains(&skill) || (0x97..=0x99).contains(&skill) || skill == 0x68 || skill == 0x69
}

pub fn supports_impact_effects(skill: i16) -> bool {
    (0..0x68).contains(&skill)
        || (0x97..=0x9B).contains(&skill)
        || skill == 0x68
        || skill == 0x69
        || skill == 0x6F
}
