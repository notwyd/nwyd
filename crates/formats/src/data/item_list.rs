use super::envelope::{self, Integrity};
use super::{ITEM_RECORD_LEN, ITEM_SLOTS, fixed_string};
use crate::error::{Error, Kind, Result};
use crate::text::Codepage;

pub const BODY_LEN: usize = ITEM_SLOTS * ITEM_RECORD_LEN;

pub const LEN: usize = BODY_LEN + 4;

pub const CHECKSUM_PRISTINE: i32 = 0x07EF_F97B;

pub const CHECKSUM_MERGED: i32 = 0x07F5_950C;

pub const EXTRA_RECORD_LEN: usize = 2 + ITEM_RECORD_LEN;

pub const EFFECT_UNUSED: i16 = 255;

pub const EFFECTS: usize = 12;

pub mod field {

    pub const NAME: usize = 0x00;

    pub const NAME_LEN: usize = 64;

    pub const MESH: usize = 0x40;

    pub const MESH_SUB: usize = 0x42;

    pub const REQ_LEVEL: usize = 0x46;

    pub const REQ_STR: usize = 0x48;

    pub const REQ_INT: usize = 0x4A;

    pub const REQ_DEX: usize = 0x4C;

    pub const REQ_CON: usize = 0x4E;

    pub const EFFECT: usize = 0x50;

    pub const PRICE: usize = 0x80;

    pub const GRADE: usize = 0x84;

    pub const EQUIP_MASK: usize = 0x88;

    pub const TIER: usize = 0x8E;

    pub const SCALE_PCT: usize = 0x90;

    pub const FLAGS94: usize = 0x94;

    pub const ITEM_TYPE: usize = 0x98;

    pub const MOUNT_INDEX: usize = 0x9A;

    pub const MAX_STACK: usize = 0x9E;

    pub const SET_INDEX: usize = 0xA4;

    pub const BASE_ID: usize = 0xA8;
}

pub mod equip {

    pub const NONE: u32 = 0x00000;

    pub const FACE: u32 = 0x00001;

    pub const HELMET: u32 = 0x00002;

    pub const ARMOR: u32 = 0x00004;

    pub const PANTS: u32 = 0x00008;

    pub const GLOVES: u32 = 0x00010;

    pub const BOOTS: u32 = 0x00020;

    pub const WEAPON: u32 = 0x00040;

    pub const SHIELD: u32 = 0x00080;

    pub const TWO_HANDED: u32 = WEAPON | SHIELD;

    pub const RING: u32 = 0x00100;

    pub const AMULET: u32 = 0x00200;

    pub const ORB: u32 = 0x00400;

    pub const GEM: u32 = 0x00800;

    pub const ANY_ACCESSORY: u32 = RING | AMULET | ORB | GEM;

    pub const MEDAL: u32 = 0x01000;

    pub const DIVINE_CIRCLE: u32 = 0x02000;

    pub const MOUNT: u32 = 0x04000;

    pub const CAPE: u32 = 0x08000;

    pub const NECKLACE: u32 = 0x10000;

    pub const BELT: u32 = 0x20000;
}

pub mod item_type {

    pub const ORDINARY: i16 = 0;

    pub const COSTUME_SET: i16 = 12;

    pub const PET_EGG: i16 = 0x14;

    pub const PET_STAGE1: i16 = 0x15;

    pub const PET_STAGE2: i16 = 0x16;

    pub const MOUNT: i16 = 0x17;

    pub fn is_mount_backed(t: i16) -> bool {
        matches!(t, PET_STAGE1 | PET_STAGE2 | MOUNT)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemplateEffect {
    pub id: i16,
    pub value: i16,
}

impl TemplateEffect {
    pub fn is_used(self) -> bool {
        self.id != EFFECT_UNUSED
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub mesh: i16,
    pub mesh_sub: i16,
    pub req_level: i16,
    pub req_str: i16,
    pub req_int: i16,
    pub req_dex: i16,
    pub req_con: i16,
    pub effect: [TemplateEffect; EFFECTS],
    pub price: i32,
    pub grade: i16,
    pub equip_mask: u32,
    pub tier: i16,
    pub scale_pct: i16,
    pub flags94: i32,
    pub item_type: i16,
    pub mount_index: i16,
    pub max_stack: i16,
    pub set_index: u32,
    pub base_id: u32,
}

impl Entry {
    pub fn effects(&self) -> impl Iterator<Item = TemplateEffect> + '_ {
        self.effect.iter().copied().filter(|e| e.is_used())
    }

    pub fn effect_sum(&self, id: i16) -> i32 {
        self.effects()
            .filter(|e| e.id == id)
            .map(|e| i32::from(e.value))
            .sum()
    }

    pub fn display_name(&self) -> String {
        self.name.replace('_', " ")
    }

    pub fn is_equippable(&self) -> bool {
        self.equip_mask != equip::NONE
    }

    pub fn is_mount_backed(&self) -> bool {
        item_type::is_mount_backed(self.item_type)
    }
}

#[derive(Debug, Clone)]
pub struct ItemList {
    raw: Vec<u8>,
    codepage: Codepage,
    name_codepages: Vec<Option<Codepage>>,
}

impl ItemList {
    pub fn from_plain(body: Vec<u8>, codepage: Codepage) -> ItemList {
        assert_eq!(
            body.len(),
            BODY_LEN,
            "a hand-built table is still the full array"
        );

        ItemList {
            raw: body,
            codepage,
            name_codepages: vec![None; ITEM_SLOTS],
        }
    }
}

fn i16_at(rec: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([rec[off], rec[off + 1]])
}

fn i32_at(rec: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([rec[off], rec[off + 1], rec[off + 2], rec[off + 3]])
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> Result<ItemList> {
    let raw = envelope::open(
        bytes,
        BODY_LEN,
        Integrity::ConstantA(CHECKSUM_PRISTINE),
        Kind::ItemList,
    )?;

    Ok(ItemList::from_plain(raw, codepage))
}

pub fn parse_with_patch(bytes: &[u8], patch: &[u8], codepage: Codepage) -> Result<ItemList> {
    let mut table = parse(bytes, codepage)?;

    let applied = table.apply_patch(patch);

    let mut reencrypted = table.raw.clone();

    envelope::xor_5a(&mut reencrypted);

    let got = envelope::nine_step_checksum(&reencrypted);

    if got != CHECKSUM_MERGED {
        return Err(Error::structure(
            Kind::ItemList,
            format!(
                "post-merge checksum mismatch after {applied} patches: \
                 expected {CHECKSUM_MERGED:#010x}, computed {got:#010x}"
            ),
        ));
    }

    Ok(table)
}

impl ItemList {
    pub fn len(&self) -> usize {
        ITEM_SLOTS
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn codepage(&self) -> Codepage {
        self.codepage
    }

    fn name_codepage(&self, id: usize) -> Codepage {
        self.name_codepages[id].unwrap_or(self.codepage)
    }

    fn record(&self, id: usize) -> Option<&[u8]> {
        let start = id.checked_mul(ITEM_RECORD_LEN)?;

        self.raw.get(start..start + ITEM_RECORD_LEN)
    }

    pub fn get(&self, id: usize) -> Option<Entry> {
        let rec = self.record(id)?;

        let effect = std::array::from_fn(|i| {
            let off = field::EFFECT + i * 4;

            TemplateEffect {
                id: i16_at(rec, off),
                value: i16_at(rec, off + 2),
            }
        });

        Some(Entry {
            name: fixed_string(
                &rec[field::NAME..field::NAME + field::NAME_LEN],
                self.name_codepage(id),
            ),
            mesh: i16_at(rec, field::MESH),
            mesh_sub: i16_at(rec, field::MESH_SUB),
            req_level: i16_at(rec, field::REQ_LEVEL),
            req_str: i16_at(rec, field::REQ_STR),
            req_int: i16_at(rec, field::REQ_INT),
            req_dex: i16_at(rec, field::REQ_DEX),
            req_con: i16_at(rec, field::REQ_CON),
            effect,
            price: i32_at(rec, field::PRICE),
            grade: i16_at(rec, field::GRADE),
            equip_mask: i32_at(rec, field::EQUIP_MASK) as u32,
            tier: i16_at(rec, field::TIER),
            scale_pct: i16_at(rec, field::SCALE_PCT),
            flags94: i32_at(rec, field::FLAGS94),
            item_type: i16_at(rec, field::ITEM_TYPE),
            mount_index: i16_at(rec, field::MOUNT_INDEX),
            max_stack: i16_at(rec, field::MAX_STACK),
            set_index: i32_at(rec, field::SET_INDEX) as u32,
            base_id: i32_at(rec, field::BASE_ID) as u32,
        })
    }

    pub fn name(&self, id: usize) -> Option<String> {
        let rec = self.record(id)?;

        Some(fixed_string(
            &rec[field::NAME..field::NAME + field::NAME_LEN],
            self.name_codepage(id),
        ))
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, Entry)> + '_ {
        (0..ITEM_SLOTS).filter_map(|id| self.get(id).map(|e| (id, e)))
    }

    pub fn populated(&self) -> usize {
        (0..ITEM_SLOTS)
            .filter(|&id| self.record(id).is_some_and(|r| r[field::NAME] != 0))
            .count()
    }

    pub fn apply_patch(&mut self, patch: &[u8]) -> usize {
        let mut applied = 0;

        for rec in patch.chunks_exact(EXTRA_RECORD_LEN) {
            let id = u16::from_le_bytes([rec[0], rec[1]]) as usize;

            if !(1..ITEM_SLOTS).contains(&id) {
                continue;
            }

            let start = id * ITEM_RECORD_LEN;

            self.raw[start..start + ITEM_RECORD_LEN].copy_from_slice(&rec[2..]);

            self.name_codepages[id] = None;

            applied += 1;
        }

        applied
    }

    pub fn set_price(&mut self, id: usize, price: i32) -> bool {
        let Some(start) = id.checked_mul(ITEM_RECORD_LEN) else {
            return false;
        };

        let Some(rec) = self.raw.get_mut(start..start + ITEM_RECORD_LEN) else {
            return false;
        };

        rec[field::PRICE..field::PRICE + 4].copy_from_slice(&price.to_le_bytes());

        true
    }

    pub fn apply_names(&mut self, records: &[super::item_name::Record]) -> usize {
        self.apply_names_with_codepage(records, self.codepage)
    }

    pub fn reset_names_from(&mut self, source: &Self) {
        for id in 0..ITEM_SLOTS {
            let start = id * ITEM_RECORD_LEN + field::NAME;

            let end = start + field::NAME_LEN;

            self.raw[start..end].copy_from_slice(&source.raw[start..end]);

            let codepage = source.name_codepage(id);

            self.name_codepages[id] = (codepage != self.codepage).then_some(codepage);
        }
    }

    pub fn apply_names_with_codepage(
        &mut self,
        records: &[super::item_name::Record],
        codepage: Codepage,
    ) -> usize {
        records
            .iter()
            .filter(|record| {
                let Ok(id) = usize::try_from(record.id) else {
                    return false;
                };

                if !self.set_name(id, &record.name) {
                    return false;
                }

                self.name_codepages[id] = (codepage != self.codepage).then_some(codepage);

                true
            })
            .count()
    }

    pub fn apply_prices(&mut self, overrides: &[super::item_price::PriceOverride]) -> usize {
        overrides
            .iter()
            .filter(|p| self.set_price(p.item_id as usize, p.price))
            .count()
    }

    pub fn set_name(&mut self, id: usize, name: &[u8]) -> bool {
        if id >= ITEM_SLOTS {
            return false;
        }

        let Some(start) = id.checked_mul(ITEM_RECORD_LEN) else {
            return false;
        };

        let Some(rec) = self.raw.get_mut(start..start + ITEM_RECORD_LEN) else {
            return false;
        };

        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());

        let n = end.min(field::NAME_LEN - 1);

        rec[field::NAME..field::NAME + n].copy_from_slice(&name[..n]);

        rec[field::NAME + n] = 0;

        self.name_codepages[id] = None;

        true
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }

    pub fn to_bytes(&self, trailer: i32) -> Vec<u8> {
        let mut out = self.raw.clone();

        envelope::xor_5a(&mut out);

        out.extend_from_slice(&trailer.to_le_bytes());

        out
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::data::item_name::Record;
    use crate::text::codepage;

    fn table() -> ItemList {
        ItemList::from_plain(vec![0; BODY_LEN], Codepage::Windows1252)
    }

    fn name(id: i32, text: &str, codepage: Codepage) -> Record {
        Record {
            id,
            name: codepage::encode(text, codepage),
        }
    }

    #[test]
    fn localized_names_keep_independent_encodings_and_leave_base_names_intact() {
        let mut table = table();

        assert!(table.set_name(1, b"Espad\xE3o"));

        assert!(table.set_name(2, b"Polish"));

        assert!(table.set_name(3, b"Korean"));

        assert_eq!(
            table.apply_names_with_codepage(
                &[name(2, "Łódź", Codepage::Windows1250)],
                Codepage::Windows1250,
            ),
            1
        );

        assert_eq!(
            table.apply_names_with_codepage(
                &[name(3, "한국어", Codepage::Windows949)],
                Codepage::Windows949,
            ),
            1
        );

        assert!(table.set_price(2, 777));

        for (id, expected) in [(1, "Espadão"), (2, "Łódź"), (3, "한국어")] {
            assert_eq!(table.name(id).as_deref(), Some(expected));

            assert_eq!(table.get(id).unwrap().name, expected);

            assert_eq!(table.clone().name(id).as_deref(), Some(expected));
        }

        assert_eq!(table.get(2).unwrap().price, 777);

        assert_eq!(table.codepage(), Codepage::Windows1252);

        let bytes = table.as_bytes();

        let expected = codepage::encode("한국어", Codepage::Windows949);

        assert_eq!(
            &bytes[3 * ITEM_RECORD_LEN..3 * ITEM_RECORD_LEN + expected.len()],
            expected
        );

        let mut serialized = table.to_bytes(42);

        assert_eq!(serialized.len(), LEN);

        assert_eq!(&serialized[BODY_LEN..], &42_i32.to_le_bytes());

        envelope::xor_5a(&mut serialized[..BODY_LEN]);

        assert_eq!(&serialized[..BODY_LEN], table.as_bytes());
    }

    #[test]
    fn replacing_localized_names_restores_the_base_encoding_only_for_replaced_records() {
        let mut table = table();

        let records: Vec<_> = (1..=4)
            .map(|id| name(id, "Łódź", Codepage::Windows1250))
            .collect();

        assert_eq!(
            table.apply_names_with_codepage(&records, Codepage::Windows1250),
            4
        );

        assert!(table.set_name(1, b"\xA3"));

        assert_eq!(
            table.apply_names(&[Record {
                id: 2,
                name: vec![0xA3]
            }]),
            1
        );

        let mut patch = vec![0; EXTRA_RECORD_LEN];

        patch[..2].copy_from_slice(&3_u16.to_le_bytes());

        patch[2 + field::NAME] = 0xA3;

        assert_eq!(table.apply_patch(&patch), 1);

        for id in 1..=3 {
            assert_eq!(table.name(id).as_deref(), Some("£"));

            assert_eq!(table.get(id).unwrap().name, "£");
        }

        assert_eq!(table.name(4).as_deref(), Some("Łódź"));

        assert_eq!(
            table.apply_names_with_codepage(
                &[name(4, "한국어", Codepage::Windows949)],
                Codepage::Windows949,
            ),
            1
        );

        assert_eq!(table.name(4).as_deref(), Some("한국어"));
    }

    #[test]
    fn invalid_localized_record_ids_do_not_change_bytes_or_name_encodings() {
        let mut table = table();

        table.apply_names_with_codepage(
            &[name(1, "Łódź", Codepage::Windows1250)],
            Codepage::Windows1250,
        );

        let original = table.as_bytes().to_vec();

        assert_eq!(
            table.apply_names_with_codepage(
                &[-1, ITEM_SLOTS as i32, i32::MAX].map(|id| name(
                    id,
                    "한국어",
                    Codepage::Windows949
                )),
                Codepage::Windows949,
            ),
            0
        );

        assert!(!table.set_name(usize::MAX, b"invalid"));

        assert!(!table.set_name(usize::MAX / ITEM_RECORD_LEN, b"invalid"));

        let mut patch = vec![0; EXTRA_RECORD_LEN];

        patch[..2].copy_from_slice(&(ITEM_SLOTS as u16).to_le_bytes());

        assert_eq!(table.apply_patch(&patch), 0);

        assert_eq!(table.as_bytes(), original);

        assert_eq!(table.name(1).as_deref(), Some("Łódź"));
    }

    #[test]
    fn resetting_names_keeps_server_prices_and_every_other_record_field() {
        let mut source = table();

        source.set_name(1, b"Root name");

        source.apply_names_with_codepage(
            &[name(2, "Łódź", Codepage::Windows1250)],
            Codepage::Windows1250,
        );

        let mut active = ItemList::from_plain(vec![0x53; BODY_LEN], Codepage::Windows949);

        active.set_price(1, 123456);

        active.apply_names_with_codepage(
            &[name(1, "한국어", Codepage::Windows949)],
            Codepage::Windows949,
        );

        let before = active.as_bytes().to_vec();

        active.reset_names_from(&source);

        assert_eq!(active.name(1).as_deref(), Some("Root name"));

        assert_eq!(active.name(2).as_deref(), Some("Łódź"));

        assert_eq!(active.get(1).unwrap().price, 123456);

        for id in 0..ITEM_SLOTS {
            let start = id * ITEM_RECORD_LEN;

            let end = start + ITEM_RECORD_LEN;

            assert_eq!(
                &active.as_bytes()[start + field::NAME_LEN..end],
                &before[start + field::NAME_LEN..end]
            );
        }
    }
}
