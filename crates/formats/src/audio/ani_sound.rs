use crate::error::{Error, Kind, Result};

pub const ANIMATIONS: usize = 56;

pub const CLASSES: usize = 60;

pub const PLAYER_CLASSES: usize = 2;

pub const VARIANTS: usize = 4;

pub const VARIANT_BLOCKS: usize = PLAYER_CLASSES * VARIANTS;

pub const MOUNTED_OFFSET: usize = 28;

pub const ANIMATION_NAMES: [&str; ANIMATIONS] = [
    "STAND01", "STAND02", "WALK", "RUN", "ATTACK1", "ATTACK2", "ATTACK3", "SKILL01", "SKILL02",
    "SKILL03", "STRIKE", "DIE", "DEAD", "SEAT", "LEVELUP", "PUNISH", "CACKLE", "YAWN", "MERCHL",
    "RELAX", "NEVER", "COMEON", "SALUTE", "HOLY", "SEATING", "STAND03", "PUNING", "PUNEND",
    "MSTND01", "MSTND02", "MWALK", "MRUN", "MATT1", "MATT2", "MATT3", "MSKIL01", "MSKIL02",
    "MSKIL03", "MSTRIKE", "MDIE", "MDEAD", "MSEAT", "MLVLUP", "MPUNISH", "MCACKLE", "MYAWN",
    "MMERCHL", "MRELAX", "MNEVER", "MCOMEON", "MSALUTE", "MHOLY", "MSEATNG", "MSTND03", "MPUNING",
    "MPUNEND",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Binding {
    pub frame: i32,
    pub speed: i32,
    pub sound: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassTable {
    pub speed: [i32; ANIMATIONS],
    pub frame: [i32; ANIMATIONS],
    pub sound: [i32; ANIMATIONS],
}

impl Default for ClassTable {
    fn default() -> Self {
        ClassTable {
            speed: [0; ANIMATIONS],
            frame: [0; ANIMATIONS],
            sound: [0; ANIMATIONS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantBlock {
    pub speed: [i32; ANIMATIONS],
    pub frame: [i32; ANIMATIONS],
}

impl Default for VariantBlock {
    fn default() -> Self {
        VariantBlock {
            speed: [0; ANIMATIONS],
            frame: [0; ANIMATIONS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AniSound {
    classes: Vec<ClassTable>,
    variants: Vec<VariantBlock>,
    present: Vec<bool>,
    section_names: Vec<(String, usize)>,
}

impl Default for AniSound {
    fn default() -> Self {
        AniSound {
            classes: vec![ClassTable::default(); CLASSES],
            variants: vec![VariantBlock::default(); VARIANT_BLOCKS],
            present: vec![false; CLASSES],
            section_names: Vec::new(),
        }
    }
}

pub fn variant_block(class: usize, variant: usize) -> Option<usize> {
    (class < PLAYER_CLASSES && variant < VARIANTS).then(|| class + variant * PLAYER_CLASSES)
}

pub fn variant_for_job(job: i16) -> Option<usize> {
    if job >= 40 {
        return None;
    }

    let (q, r) = (job / 10, job % 10);

    let v = if r == 1 {
        q
    } else if r < 6 {
        return None;
    } else {
        r - 6
    };

    let v = if v > 3 { 0 } else { v };

    usize::try_from(v).ok()
}

impl AniSound {
    pub fn class(&self, class: usize) -> Option<&ClassTable> {
        self.classes.get(class)
    }

    pub fn variant(&self, class: usize, variant: usize) -> Option<&VariantBlock> {
        self.variants.get(variant_block(class, variant)?)
    }

    pub fn has_class(&self, class: usize) -> bool {
        self.present.get(class).copied().unwrap_or(false)
    }

    pub fn missing_classes(&self) -> Vec<usize> {
        (0..CLASSES).filter(|&c| !self.has_class(c)).collect()
    }

    pub fn section_names(&self) -> &[(String, usize)] {
        &self.section_names
    }

    pub fn binding(&self, class: usize, job: i16, anim: usize) -> Option<Binding> {
        let generic = self.class(class)?;

        let sound = *generic.sound.get(anim)?;

        match variant_for_job(job).and_then(|v| self.variant(class, v)) {
            Some(b) => Some(Binding {
                frame: b.frame[anim],
                speed: b.speed[anim],
                sound,
            }),
            None => Some(Binding {
                frame: generic.frame[anim],
                speed: generic.speed[anim],
                sound,
            }),
        }
    }

    pub fn sound_indices(&self) -> Vec<i32> {
        let mut out: Vec<i32> = self
            .classes
            .iter()
            .flat_map(|c| c.sound.iter().copied())
            .filter(|&s| s != 0)
            .collect();

        out.sort_unstable();

        out.dedup();

        out
    }
}

pub fn parse(bytes: &[u8]) -> Result<AniSound> {
    let text: String = bytes.iter().map(|&b| b as char).collect();

    let mut tokens = text.split_ascii_whitespace();

    let mut out = AniSound::default();

    while let Some(name) = tokens.next() {
        let class = tokens.next().ok_or_else(|| {
            Error::structure(
                Kind::AniSound,
                format!("section {name:?} has no class index"),
            )
        })?;

        let class: i64 = class.parse().map_err(|e| {
            Error::structure(Kind::AniSound, format!("section {name:?} class index: {e}"))
        })?;

        let class = usize::try_from(class)
            .ok()
            .filter(|&c| c < CLASSES)
            .ok_or_else(|| {
                Error::structure(
                    Kind::AniSound,
                    format!(
                        "section {name:?} declares class {class}, past the {CLASSES}-slot table"
                    ),
                )
            })?;

        let wide = class < PLAYER_CLASSES;

        for anim in 0..ANIMATIONS {
            let row = tokens.next().ok_or_else(|| {
                Error::structure(
                    Kind::AniSound,
                    format!(
                        "section {name:?} (class {class}) ends after {anim} of {ANIMATIONS} rows"
                    ),
                )
            })?;

            let want = if wide { VARIANTS * 2 + 1 } else { 3 };

            let mut ints = Vec::with_capacity(want);

            for field in 0..want {
                let tok = tokens.next().ok_or_else(|| {
                    Error::structure(
                        Kind::AniSound,
                        format!("class {class} row {row:?} ends after {field} of {want} fields"),
                    )
                })?;

                ints.push(tok.parse::<i32>().map_err(|e| {
                    Error::structure(
                        Kind::AniSound,
                        format!("class {class} row {row:?} field {field}: {tok:?}: {e}"),
                    )
                })?);
            }

            if wide {
                for v in 0..VARIANTS {
                    let block = &mut out.variants[class + v * PLAYER_CLASSES];

                    block.frame[anim] = ints[v * 2];

                    block.speed[anim] = ints[v * 2 + 1];
                }

                out.classes[class].frame[anim] = ints[0];

                out.classes[class].speed[anim] = ints[1];

                out.classes[class].sound[anim] = ints[VARIANTS * 2];
            } else {
                out.classes[class].frame[anim] = ints[0];

                out.classes[class].speed[anim] = ints[1];

                out.classes[class].sound[anim] = ints[2];
            }
        }

        out.present[class] = true;

        out.section_names.push((name.to_string(), class));
    }

    Ok(out)
}
