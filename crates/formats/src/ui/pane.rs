use super::script::{self, Reader};
use crate::error::{Kind, Result};
pub use crate::texture::wyp::decrypt;
pub use crate::texture::wyp::encrypt;

pub fn is_ciphered(path: &str) -> bool {
    let n = path.chars().count();

    n >= 5 && path.chars().skip(n - 5).eq(".pane".chars())
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Property {
    pub key: String,
    pub value: String,
}

impl Property {
    pub fn layer(&self) -> Option<(&str, i32)> {
        let open = self.key.find('[')?;

        let digit = self.key[open + 1..].chars().next()?;

        Some((&self.key[..open], digit as i32 - 0x30))
    }

    pub fn base_key(&self) -> &str {
        match self.key.find('[') {
            Some(i) => &self.key[..i],
            None => &self.key,
        }
    }

    pub fn int(&self) -> i32 {
        script::parse_integer_prefix_or_zero(&self.value)
    }

    pub fn ints4(&self) -> [i32; 4] {
        script::ints4(&self.value)
    }

    pub fn argb(&self) -> u32 {
        script::argb(&self.value)
    }

    pub fn float(&self) -> f32 {
        script::parse_float_prefix_or_zero(&self.value)
    }

    pub fn scan(&self, out: &mut [i32]) -> usize {
        script::scan_ints(&self.value, out)
    }

    pub fn flag(&self) -> bool {
        self.value.eq_ignore_ascii_case("true")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Block {
    pub properties: Vec<Property>,
}

impl Block {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .rev()
            .find(|p| p.key.eq_ignore_ascii_case(key))
            .map(|p| p.value.as_str())
    }

    pub fn all(&self, base_key: &str) -> impl Iterator<Item = &Property> {
        self.properties
            .iter()
            .filter(move |p| p.base_key().eq_ignore_ascii_case(base_key))
    }

    pub fn int(&self, key: &str) -> Option<i32> {
        self.get(key).map(script::parse_integer_prefix_or_zero)
    }

    pub fn argb(&self, key: &str) -> Option<u32> {
        self.get(key).map(script::argb)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Element {
    pub type_name: String,
    pub name: String,
    pub block: Block,
}

impl Element {
    pub fn element_type(&self) -> Option<ElementType> {
        ElementType::from_name(&self.type_name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Pane {
    pub panel: Block,
    pub elements: Vec<Element>,
    pub warnings: Vec<String>,
}

impl Pane {
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }
}

pub fn load(path: &str, bytes: &[u8]) -> Result<Pane> {
    if is_ciphered(path) {
        parse(&decrypt(bytes.to_vec()))
    } else {
        parse(bytes)
    }
}

pub fn parse(bytes: &[u8]) -> Result<Pane> {
    let chars = script::decode(Kind::Pane, bytes)?;

    let mut reader = Reader::new(&chars);

    let mut pane = Pane::default();

    while let Some(line) = reader.read_line() {
        if line.is_empty() || script::is_pane_comment(&line) {
            continue;
        }

        let tokens = script::split_all(&line, script::TOKEN_DELIMS);

        let Some(head) = tokens.first() else {
            pane.warnings.push(format!("invalid line: {line}"));

            continue;
        };

        if head.eq_ignore_ascii_case("panel") {
            pane.panel = read_block(&mut reader, &mut pane.warnings);
        } else if head.eq_ignore_ascii_case("element") {
            if tokens.len() < 2 {
                pane.warnings.push(format!("invalid line: {line}"));
            }

            let parts = script::split_all(
                tokens.get(1).map_or("", String::as_str),
                &['\t', ' ', '(', ')'],
            );

            if parts.len() != 1 && parts.len() != 2 {
                pane.warnings.push(format!("invalid line: {line}"));
            }

            let type_name = parts.first().cloned().unwrap_or_default();

            let name = if parts.len() == 2 {
                parts[1].clone()
            } else {
                String::new()
            };

            let block = read_block(&mut reader, &mut pane.warnings);

            pane.elements.push(Element {
                type_name,
                name,
                block,
            });
        } else {
            pane.warnings.push(format!("invalid line: {line}"));
        }
    }

    Ok(pane)
}

fn read_block(reader: &mut Reader<'_>, warnings: &mut Vec<String>) -> Block {
    if !reader.skip_to(&['{']) {
        warnings.push("no found: '{'".to_string());

        return Block::default();
    }

    let mut block = Block::default();

    while let Some(line) = reader.read_line() {
        if line.is_empty() || script::is_pane_comment(&line) {
            continue;
        }

        if line.eq_ignore_ascii_case("}") {
            return block;
        }

        let Some((key, value)) = script::split_token_and_remainder(&line) else {
            warnings.push(format!("invalid param: {line}"));

            continue;
        };

        block.properties.push(Property {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    warnings.push("there is no symbol '}'".to_string());

    block
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]

pub enum ElementType {
    Button = 0,
    Static = 1,
    CheckBox = 2,
    RadioButton = 3,
    ComboBox = 4,
    Slider = 5,
    EditBox = 6,
    ImeEditBox = 7,
    ImeNotePadEditBox = 8,
    ListBox = 9,
    ListView = 10,
    TreeView = 11,
    ScrollBar = 12,
    Image = 13,
    ImeNotePad = 14,
    GaugeBar = 15,
    BalloonBox = 16,
    TooltipBox = 17,
    ImageFont = 18,
    IconButton = 19,
    MoMesh = 20,
    CustomListView = 21,
}

impl ElementType {
    pub fn from_name(name: &str) -> Option<ElementType> {
        use ElementType::*;

        let t = [
            ("Button", Button),
            ("Static", Static),
            ("CheckBox", CheckBox),
            ("RadioButton", RadioButton),
            ("ComboBox", ComboBox),
            ("Slider", Slider),
            ("EditBox", EditBox),
            ("ImeEditBox", ImeEditBox),
            ("ImeNotePadEditBox", ImeNotePadEditBox),
            ("ListBox", ListBox),
            ("ListView", ListView),
            ("TreeView", TreeView),
            ("image", Image),
            ("ImeNotePad", ImeNotePad),
            ("GaugeBar", GaugeBar),
            ("BalloonBox", BalloonBox),
            ("TooltipBox", TooltipBox),
            ("ImageFont", ImageFont),
            ("IconButton", IconButton),
            ("MoMesh", MoMesh),
            ("CustomListView", CustomListView),
        ];

        t.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, t)| *t)
    }

    pub fn id(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum RasterOp {
    Wrap = 0,
    #[default]
    Stretch = 1,
    Split = 2,
    WSplitStretch = 3,
    HStretchSplit = 4,
}

impl RasterOp {
    pub fn parse(value: &str) -> Option<RasterOp> {
        Some(match value {
            v if v.eq_ignore_ascii_case("wrap") => RasterOp::Wrap,
            v if v.eq_ignore_ascii_case("stretch") => RasterOp::Stretch,
            v if v.eq_ignore_ascii_case("split") => RasterOp::Split,
            v if v.eq_ignore_ascii_case("wSplitStretch") => RasterOp::WSplitStretch,
            v if v.eq_ignore_ascii_case("hStretchSplit") => RasterOp::HStretchSplit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum ShadowType {
    #[default]
    None = 0,
    BottomRight = 1,
    Outline = 2,
    Top = 3,
}

impl ShadowType {
    pub fn parse(value: &str) -> ShadowType {
        match value {
            v if v.eq_ignore_ascii_case("outline") => ShadowType::Outline,
            v if v.eq_ignore_ascii_case("bottomRight") => ShadowType::BottomRight,
            v if v.eq_ignore_ascii_case("top") => ShadowType::Top,
            _ => ShadowType::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum AlignPanel {
    #[default]
    None = 0,
    Center = 1,
    Side = 2,
}

impl AlignPanel {
    pub fn parse(value: &str) -> AlignPanel {
        match value {
            v if v.eq_ignore_ascii_case("Center") => AlignPanel::Center,
            v if v.eq_ignore_ascii_case("SIDE") => AlignPanel::Side,
            _ => AlignPanel::None,
        }
    }
}

pub mod font_align {

    pub const CENTER: u32 = 0x1;

    pub const RIGHT: u32 = 0x2;

    pub const VCENTER: u32 = 0x4;

    pub const BOTTOM: u32 = 0x8;

    pub const WORDBREAK: u32 = 0x10;

    pub fn parse(value: &str) -> u32 {
        let mut flags = 0u32;

        for token in value.split([' ', '\t']).filter(|t| !t.is_empty()) {
            match token {
                t if t.eq_ignore_ascii_case("left") => flags &= !(CENTER | RIGHT),
                t if t.eq_ignore_ascii_case("center") => flags = flags & !RIGHT | CENTER,
                t if t.eq_ignore_ascii_case("right") => flags = flags & !CENTER | RIGHT,
                t if t.eq_ignore_ascii_case("top") => flags &= !(VCENTER | BOTTOM),
                t if t.eq_ignore_ascii_case("vcenter") => {
                    flags = flags & !BOTTOM | VCENTER;
                }
                t if t.eq_ignore_ascii_case("bottom") => {
                    flags = flags & !VCENTER | BOTTOM;
                }
                t if t.eq_ignore_ascii_case("wordbreak") => flags |= WORDBREAK,
                _ => {}
            }
        }

        flags
    }
}

pub mod resize_tip {

    pub const TOP: u32 = 0x01;

    pub const BOTTOM: u32 = 0x02;

    pub const LEFT: u32 = 0x04;

    pub const RIGHT: u32 = 0x08;

    pub const TOP_RIGHT: u32 = 0x10;

    pub const TOP_LEFT: u32 = 0x20;

    pub const BOTTOM_RIGHT: u32 = 0x40;

    pub const BOTTOM_LEFT: u32 = 0x80;

    pub const ALL: u32 = 0xFFFF_FFFF;

    pub fn parse(value: &str) -> u32 {
        let mut flags = 0u32;

        for token in value
            .split([',', ' ', '\r', '\n'])
            .filter(|t| !t.is_empty())
        {
            if token.eq_ignore_ascii_case("all") {
                flags = ALL;

                continue;
            }

            flags |= match token {
                t if t.eq_ignore_ascii_case("top") => TOP,
                t if t.eq_ignore_ascii_case("bottom") => BOTTOM,
                t if t.eq_ignore_ascii_case("left") => LEFT,
                t if t.eq_ignore_ascii_case("right") => RIGHT,
                t if t.eq_ignore_ascii_case("topLeft") => TOP_LEFT,
                t if t.eq_ignore_ascii_case("topRight") => TOP_RIGHT,
                t if t.eq_ignore_ascii_case("bottomLeft") => BOTTOM_LEFT,
                t if t.eq_ignore_ascii_case("bottomRight") => BOTTOM_RIGHT,
                _ => 0,
            };
        }

        flags
    }
}

pub mod screen_align {

    pub const LEFT: u32 = 0x1;

    pub const TOP: u32 = 0x2;

    pub const RIGHT: u32 = 0x4;

    pub const BOTTOM: u32 = 0x8;

    pub fn parse(value: &str) -> u32 {
        let mut flags = 0u32;

        for token in value.split([' ', '\t']).filter(|t| !t.is_empty()) {
            flags |= match token {
                t if t.eq_ignore_ascii_case("left") => LEFT,
                t if t.eq_ignore_ascii_case("top") => TOP,
                t if t.eq_ignore_ascii_case("right") => RIGHT,
                t if t.eq_ignore_ascii_case("bottom") => BOTTOM,
                _ => 0,
            };
        }

        flags
    }
}

pub fn forces_false_boolean_property(key: &str) -> bool {
    ["enabled", "visible", "MsgReceive"]
        .iter()
        .any(|k| k.eq_ignore_ascii_case(key))
}

pub fn element_bool(key: &str, value: &str) -> bool {
    !forces_false_boolean_property(key) && value.eq_ignore_ascii_case("true")
}

pub const NAME_PROPERTY_IS_BROKEN: bool = true;
