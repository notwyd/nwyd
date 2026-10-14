use super::pane::{ElementType, RasterOp};
use super::script::{self, Reader};
use crate::error::{Kind, Result};

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct FontDef {
    pub index: i32,
    pub face: i32,
    pub height: i32,
    pub width_adjust: i32,
    pub field4: i32,
    pub bold: bool,
}

impl FontDef {
    pub fn face(written: i32) -> i32 {
        written.min(1)
    }

    pub const FACE_CHAINS: [[(&'static str, &'static str, bool); 3]; 2] = [
        [
            ("Tahoma", "Tahoma.ttf", true),
            ("NanumBarunGothic", "ui/NanumBarunGothic.ttf", false),
            ("Gulim", "Gulim.ttc", true),
        ],
        [
            ("Test", "Test.ttf", false),
            ("Gulim", "Gulim.ttc", true),
            ("Gulim", "Gulim.ttc", true),
        ],
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct TextureDef {
    pub index: i32,
    pub color_key: i32,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Alias {
    pub name: String,
    pub texture_index: i32,
    pub rect: [i32; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Colors {
    pub default: Option<u32>,
    pub normal: Option<u32>,
    pub pressed: Option<u32>,
    pub mouseover: Option<u32>,
    pub disabled: Option<u32>,
}

impl Colors {
    fn set(&mut self, state: &str, value: u32) -> bool {
        let slot = match state {
            s if s.eq_ignore_ascii_case("default") => &mut self.default,
            s if s.eq_ignore_ascii_case("normal") => &mut self.normal,
            s if s.eq_ignore_ascii_case("pressed") => &mut self.pressed,
            s if s.eq_ignore_ascii_case("mouseover") => &mut self.mouseover,
            s if s.eq_ignore_ascii_case("disabled") => &mut self.disabled,
            _ => return false,
        };

        *slot = Some(value);

        true
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct TextureLayer {
    pub index: Option<i32>,
    pub uv: Option<[i32; 4]>,
    pub alias: Option<String>,
    pub raster_op: Option<RasterOp>,
    pub colors: Colors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum VAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct FontLayer {
    pub index: Option<i32>,
    pub halign: Option<HAlign>,
    pub valign: Option<VAlign>,
    pub offset: Option<[i32; 2]>,
    pub wordbreak: bool,
    pub colors: Colors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Binding {
    Panel(u32),
    Element { element: ElementType, layer: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Section {
    pub name: String,
    pub binding: Option<Binding>,
    pub texture: Option<TextureLayer>,
    pub font: Option<FontLayer>,
    pub titlebar_height: Option<i32>,
    pub close_button_location: Option<[i32; 4]>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct GuiMat {
    pub fonts: Vec<FontDef>,
    pub textures: Vec<TextureDef>,
    pub aliases: Vec<Alias>,
    pub sections: Vec<Section>,
    pub warnings: Vec<String>,
}

impl GuiMat {
    pub fn alias(&self, name: &str) -> Option<&Alias> {
        self.aliases
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    pub fn texture(&self, index: i32) -> Option<&TextureDef> {
        self.textures.iter().find(|t| t.index == index)
    }

    pub fn font(&self, index: i32) -> Option<&FontDef> {
        self.fonts.iter().find(|f| f.index == index)
    }

    pub fn material(&self, element: ElementType, layer: u32) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.binding == Some(Binding::Element { element, layer }))
    }
}

pub fn parse(bytes: &[u8]) -> Result<GuiMat> {
    let chars = script::decode(Kind::GuiMat, bytes)?;

    let mut reader = Reader::new(&chars);

    let mut out = GuiMat::default();

    while let Some(line) = reader.read_line() {
        let name = script::strip_guimat_comment(&line);

        if name.is_empty() {
            continue;
        }

        if name.eq_ignore_ascii_case("FontList") {
            read_font_list(&mut reader, &mut out);
        } else if name.eq_ignore_ascii_case("TextureList") {
            read_texture_list(&mut reader, &mut out);
        } else if name.eq_ignore_ascii_case("TextureAlias") {
            read_alias_list(&mut reader, &mut out);
        } else {
            let name = name.to_string();

            let section = read_section(&mut reader, &name, &mut out.warnings);

            if section.binding.is_none() {
                out.warnings.push(format!("invalid keyword: {name}"));
            }

            out.sections.push(section);
        }
    }

    Ok(out)
}

fn body(reader: &mut Reader<'_>, warnings: &mut Vec<String>) -> Vec<String> {
    if !reader.skip_to(&['{']) {
        warnings.push("no found: '{'".to_string());

        return Vec::new();
    }

    let mut rows = Vec::new();

    while let Some(line) = reader.read_line() {
        let line = script::strip_guimat_comment(&line);

        if line == "}" {
            return rows;
        }

        if !line.is_empty() {
            rows.push(line.to_string());
        }
    }

    warnings.push("there is no symbol '}'".to_string());

    rows
}

fn read_font_list(reader: &mut Reader<'_>, out: &mut GuiMat) {
    for row in body(reader, &mut out.warnings) {
        let f: Vec<&str> = row
            .split(',')
            .map(|s| s.trim_matches(script::TRIM))
            .collect();

        if f.len() != 6 {
            out.warnings.push(format!(
                "usage: [index] [fontname] [fontsize] [fontwidth] [isbold(true/false)]: {row}"
            ));
        }

        let at = |i: usize| {
            f.get(i)
                .map(|s| script::parse_integer_prefix_or_zero(s))
                .unwrap_or(0)
        };

        out.fonts.push(FontDef {
            index: at(0),
            face: FontDef::face(at(1)),
            height: at(2),
            width_adjust: at(3),
            field4: at(4),
            bold: f.get(5).is_some_and(|s| s.eq_ignore_ascii_case("true")),
        });
    }
}

fn read_texture_list(reader: &mut Reader<'_>, out: &mut GuiMat) {
    for row in body(reader, &mut out.warnings) {
        let Some((index, rest)) = script::split_token_and_remainder(&row) else {
            continue;
        };

        let Some((color_key, path)) = script::split_token_and_remainder(rest) else {
            out.warnings
                .push(format!("usage: [index] [texture_file_path]: {row}"));

            continue;
        };

        if path.is_empty() {
            out.warnings
                .push(format!("usage: [index] [texture_file_path]: {row}"));
        }

        out.textures.push(TextureDef {
            index: script::parse_integer_prefix_or_zero(index),
            color_key: script::parse_integer_prefix_or_zero(color_key),
            path: path.to_string(),
        });
    }
}

fn read_alias_list(reader: &mut Reader<'_>, out: &mut GuiMat) {
    for row in body(reader, &mut out.warnings) {
        match parse_alias(&row) {
            Some(a) => out.aliases.push(a),
            None => out.warnings.push(format!(
                "usage: [TextureAliasName] [textureIndex left top right bottom]: {row}"
            )),
        }
    }
}

fn parse_alias(row: &str) -> Option<Alias> {
    const MAX_NAME: usize = 127;

    let mut rest = row.trim_start_matches(script::TRIM);

    let end = rest.find(script::TRIM).unwrap_or(rest.len());

    let name = &rest[..end];

    if name.is_empty() || name.chars().count() > MAX_NAME {
        return None;
    }

    rest = rest[end..].trim_start_matches(script::TRIM);

    let rest = rest.strip_prefix('<')?;

    let (index, rest) = rest.split_once('>')?;

    let texture_index = scan_int(index)?;

    let mut fields = rest.split_whitespace();

    let mut next = || fields.next().and_then(scan_int);

    let rect = [next()?, next()?, next()?, next()?];

    Some(Alias {
        name: name.to_string(),
        texture_index,
        rect,
    })
}

fn scan_int(s: &str) -> Option<i32> {
    let s = s.trim();

    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);

    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    Some(script::parse_integer_prefix_or_zero(s))
}

fn read_section(reader: &mut Reader<'_>, name: &str, warnings: &mut Vec<String>) -> Section {
    let mut section = Section {
        name: name.to_string(),
        binding: binding_for(name),
        texture: None,
        font: None,
        titlebar_height: None,
        close_button_location: None,
    };

    if !reader.skip_to(&['{']) {
        warnings.push("no found: '{'".to_string());

        return section;
    }

    while let Some(line) = reader.read_line() {
        let line = script::strip_guimat_comment(&line);

        if line == "}" {
            break;
        }

        if line.is_empty() {
            continue;
        }

        if line.eq_ignore_ascii_case("texture") {
            section.texture = Some(read_texture_layer(reader, warnings));

            continue;
        }

        if line.eq_ignore_ascii_case("font") {
            section.font = Some(read_font_layer(reader, warnings));

            continue;
        }

        let tokens = script::split_all(line, script::TRIM);

        let key = tokens.first().map(String::as_str).unwrap_or("");

        if name.len() >= 5 && name[..5].eq_ignore_ascii_case("panel") {
            if key.eq_ignore_ascii_case("titlebar_height") {
                section.titlebar_height = Some(
                    tokens
                        .get(1)
                        .map(|s| script::parse_integer_prefix_or_zero(s))
                        .unwrap_or(0),
                );
            } else if key.eq_ignore_ascii_case("closebuttonLocation") {
                section.close_button_location = Some(std::array::from_fn(|i| {
                    tokens
                        .get(i + 1)
                        .map(|s| script::parse_integer_prefix_or_zero(s))
                        .unwrap_or(0)
                }));
            }
        }
    }

    section
}

fn read_texture_layer(reader: &mut Reader<'_>, warnings: &mut Vec<String>) -> TextureLayer {
    let mut layer = TextureLayer::default();

    for row in body(reader, warnings) {
        let tokens = script::split_all(&row, script::TRIM);

        let Some(key) = tokens.first() else {
            continue;
        };

        let arg = |i: usize| {
            tokens
                .get(i)
                .map(|s| script::parse_integer_prefix_or_zero(s))
                .unwrap_or(0)
        };

        match key.as_str() {
            k if k.eq_ignore_ascii_case("index") => layer.index = Some(arg(1)),
            k if k.eq_ignore_ascii_case("uv") => {
                layer.uv = Some(std::array::from_fn(|i| arg(i + 1)));
            }
            k if k.eq_ignore_ascii_case("TextureAlias") => {
                layer.alias = Some(tokens.get(1).cloned().unwrap_or_default());
            }
            k if k.eq_ignore_ascii_case("RasterOp") => {
                layer.raster_op = tokens.get(1).and_then(|v| RasterOp::parse(v));
            }
            k if k.len() > 6 && k[..6].eq_ignore_ascii_case("color_") => {
                let value = script::argb(row[key.len()..].trim_matches(script::TRIM));

                if !layer.colors.set(&key[6..], value) {
                    warnings.push(format!("unknown colour state: {row}"));
                }
            }
            _ => {}
        }
    }

    layer
}

fn read_font_layer(reader: &mut Reader<'_>, warnings: &mut Vec<String>) -> FontLayer {
    let mut layer = FontLayer::default();

    for row in body(reader, warnings) {
        let tokens = script::split_all(&row, script::TRIM);

        let Some(key) = tokens.first() else {
            continue;
        };

        let arg = |i: usize| {
            tokens
                .get(i)
                .map(|s| script::parse_integer_prefix_or_zero(s))
                .unwrap_or(0)
        };

        let word = |i: usize| tokens.get(i).cloned().unwrap_or_default();

        match key.as_str() {
            k if k.eq_ignore_ascii_case("index") => layer.index = Some(arg(1)),
            k if k.eq_ignore_ascii_case("halign") => {
                layer.halign = Some(match word(1) {
                    v if v.eq_ignore_ascii_case("center") => HAlign::Center,
                    v if v.eq_ignore_ascii_case("right") => HAlign::Right,
                    _ => HAlign::Left,
                });
            }
            k if k.eq_ignore_ascii_case("valign") => {
                layer.valign = Some(match word(1) {
                    v if v.eq_ignore_ascii_case("center") => VAlign::Center,
                    v if v.eq_ignore_ascii_case("bottom") => VAlign::Bottom,
                    _ => VAlign::Top,
                });
            }
            k if k.eq_ignore_ascii_case("fontoffset") => {
                layer.offset = Some([arg(1), arg(2)]);
            }
            k if k.eq_ignore_ascii_case("wordbreak") => {
                layer.wordbreak = word(1) == "true";
            }
            k if k.len() > 6 && k[..6].eq_ignore_ascii_case("color_") => {
                let value = script::argb(row[key.len()..].trim_matches(script::TRIM));

                if !layer.colors.set(&key[6..], value) {
                    warnings.push(format!("unknown colour state: {row}"));
                }
            }
            _ => {}
        }
    }

    layer
}

fn binding_for(name: &str) -> Option<Binding> {
    use ElementType::*;

    if let Some(rest) = strip_prefix_ci(name, "panel_") {
        let n = script::parse_integer_prefix_or_zero(rest);

        return (0..=3).contains(&n).then_some(Binding::Panel(n as u32));
    }

    let table: &[(&str, ElementType, u32)] = &[
        ("static_", Static, 1),
        ("slider_", Slider, 2),
        ("checkbox_", CheckBox, 2),
        ("button_", Button, 3),
        ("iconbutton_", IconButton, 4),
        ("radiobutton_", RadioButton, 2),
        ("combobox_", ComboBox, 4),
        ("scrollbar_", ScrollBar, 8),
        ("listbox_", ListBox, 6),
        ("editbox_", EditBox, 3),
        ("gaugebar_", GaugeBar, 3),
        ("balloonbox_", BalloonBox, 2),
        ("imagefont_", ImageFont, 2),
        ("tooltipbox_", TooltipBox, 2),
    ];

    for (prefix, element, layers) in table {
        if let Some(rest) = strip_prefix_ci(name, prefix) {
            let n = script::parse_integer_prefix_or_zero(rest);

            if (0..*layers as i32).contains(&n) {
                return Some(Binding::Element {
                    element: *element,
                    layer: n as u32,
                });
            }
        }
    }

    None
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix))
        .then(|| &s[prefix.len()..])
}

pub const EDITBOX_ALSO_BINDS_IME: ElementType = ElementType::ImeEditBox;
