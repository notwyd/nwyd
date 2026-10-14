use crate::error::{Error, Kind, Result};
use crate::text::{Codepage, codepage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum ControlType {
    Panel = 1,
    Button = 2,
    CheckBox = 3,
    ListBox = 6,
    Progress = 10,
    Text = 12,
    Edit = 13,
    Object3d = 15,
    Grid = 16,
}

impl ControlType {
    pub fn from_tag(tag: u32) -> Option<ControlType> {
        Some(match tag {
            1 => ControlType::Panel,
            2 => ControlType::Button,
            3 => ControlType::CheckBox,
            6 => ControlType::ListBox,
            10 => ControlType::Progress,
            12 => ControlType::Text,
            13 => ControlType::Edit,
            15 => ControlType::Object3d,
            16 => ControlType::Grid,
            _ => return None,
        })
    }

    pub fn tag(self) -> u32 {
        self as u32
    }

    pub fn body_len(self) -> usize {
        match self {
            ControlType::Panel | ControlType::Button | ControlType::Grid => 0x28,
            ControlType::CheckBox => 0x20,
            ControlType::ListBox | ControlType::Text => 0x34,
            ControlType::Progress => 0x30,
            ControlType::Edit => 0xB8,
            ControlType::Object3d => 0x1C,
        }
    }

    pub fn source_tag(self) -> &'static str {
        match self {
            ControlType::Panel => "[PANEL]",
            ControlType::Button => "[BUTTON]",
            ControlType::CheckBox => "[CHECKBOX]",
            ControlType::ListBox => "[LISTBOX]",
            ControlType::Progress => "[PROGRESS]",
            ControlType::Text => "[TEXT]",
            ControlType::Edit => "[EDIT]",
            ControlType::Object3d => "[3DOBJ]",
            ControlType::Grid => "[GRID]",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub name_id: u32,
    pub parent_id: u32,
}

impl Header {
    pub fn is_root_child(&self) -> bool {
        self.parent_id == 0
    }
}

pub const TEXT_INDEX_LIMIT: i32 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub control: ControlType,
    pub header: Header,
    pub fields: Vec<i32>,
    pub text_buffer: Option<Vec<u8>>,
}

pub const EDIT_FIELDS: usize = 12;

pub const EDIT_TEXT_LEN: usize = 0x80;

impl Record {
    pub fn control_id(&self) -> Option<i32> {
        self.fields.first().copied()
    }

    pub fn rect(&self) -> Option<[i32; 4]> {
        match self.control {
            ControlType::Panel
            | ControlType::Button
            | ControlType::Grid
            | ControlType::CheckBox
            | ControlType::Text
            | ControlType::Object3d => self.fields.get(1..5)?.try_into().ok(),
            ControlType::Progress | ControlType::ListBox | ControlType::Edit => None,
        }
    }

    pub fn default_text(&self) -> Option<String> {
        self.text_buffer
            .as_deref()
            .map(|b| codepage::decode(b, Codepage::Windows1252))
    }

    pub fn text_index_resolves(index: i32) -> bool {
        index <= TEXT_INDEX_LIMIT
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneUi {
    pub records: Vec<Record>,
}

pub fn parse(bytes: &[u8]) -> Result<SceneUi> {
    let mut records = Vec::new();

    let mut off = 0usize;

    while off < bytes.len() {
        let tag = u32::from_le_bytes(
            bytes
                .get(off..off + 4)
                .ok_or(Error::Truncated {
                    kind: Kind::SceneUi,
                    offset: off,
                    needed: 4,
                    len: bytes.len(),
                })?
                .try_into()
                .expect("4 bytes"),
        );

        let control = ControlType::from_tag(tag).ok_or_else(|| {
            Error::structure(
                Kind::SceneUi,
                format!("unsupported control type {tag} at offset {off}"),
            )
        })?;

        let body_len = control.body_len();

        let body = bytes
            .get(off + 4..off + 4 + body_len)
            .ok_or(Error::Truncated {
                kind: Kind::SceneUi,
                offset: off + 4,
                needed: body_len,
                len: bytes.len(),
            })?;

        let header = Header {
            name_id: u32::from_le_bytes(body[0..4].try_into().expect("4 bytes")),
            parent_id: u32::from_le_bytes(body[4..8].try_into().expect("4 bytes")),
        };

        let (int_bytes, text_buffer) = if control == ControlType::Edit {
            let split = 8 + EDIT_FIELDS * 4;

            let raw = &body[split..];

            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());

            (&body[8..split], Some(raw[..end].to_vec()))
        } else {
            (&body[8..], None)
        };

        let fields = int_bytes
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().expect("4 bytes")))
            .collect();

        records.push(Record {
            control,
            header,
            fields,
            text_buffer,
        });

        off += 4 + body_len;
    }

    Ok(SceneUi { records })
}

impl SceneUi {
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn count_of(&self, control: ControlType) -> usize {
        self.records.iter().filter(|r| r.control == control).count()
    }

    pub fn orphans(&self) -> Vec<usize> {
        let names: std::collections::HashSet<u32> =
            self.records.iter().map(|r| r.header.name_id).collect();

        self.records
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.header.is_root_child() && !names.contains(&r.header.parent_id))
            .map(|(i, _)| i)
            .collect()
    }
}

pub fn is_legacy_incompatible(file_name: &str) -> bool {
    matches!(file_name, "LoginScene.bin" | "LoginScene2.bin")
}
