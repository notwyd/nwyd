use formats::data::item_help::Help;
use formats::text::strdef::StrDef;
use formats::text::string_table::{StringTable, ui};
use std::sync::Arc;

pub const PREFIXES: [&str; 5] = ["uu_", "ss_", "im_", "ii_", "ff_"];

pub trait FileText: std::fmt::Debug + Send + Sync {
    fn text(&self, name: &str) -> Option<String>;
}

#[derive(Debug, Clone, Default)]
pub struct Strings {
    pub ui: Option<StringTable>,
    pub strdef: Option<StrDef>,
    pub item_help: Option<Arc<[Help]>>,
    pub files: Option<Arc<dyn FileText>>,
}

impl Strings {
    pub fn new(ui: StringTable, strdef: StrDef) -> Strings {
        Strings {
            ui: Some(ui),
            strdef: Some(strdef),
            ..Default::default()
        }
    }

    pub fn translate(&self, key: &str) -> String {
        if !Strings::is_key(key) {
            return key.to_string();
        }

        let Some((prefix, suffix)) = key.split_at_checked(3) else {
            return key.to_string();
        };

        match prefix {
            "uu_" => match index(suffix).filter(|n| (1..=499).contains(n)) {
                Some(n) => self
                    .ui
                    .as_ref()
                    .map(|t| t.get(n as usize))
                    .unwrap_or_default()
                    .replace('_', " "),
                None => format!("uu_{}", index(suffix).unwrap_or(0)),
            },
            "ss_" | "im_" => match index(suffix).filter(|n| (1..=1999).contains(n)) {
                Some(n) => self
                    .strdef
                    .as_ref()
                    .map(|t| t.get(n as usize))
                    .unwrap_or_default()
                    .to_string(),
                None => format!("ss_{}", index(suffix).unwrap_or(0)),
            },
            "ii_" => match index(suffix).filter(|n| (1..=14499).contains(n)) {
                Some(n) => self
                    .item_help
                    .as_ref()
                    .and_then(|t| t.get(n as usize))
                    .map(item_text)
                    .unwrap_or_default(),
                None => format!("ii_{}", index(suffix).unwrap_or(0)),
            },
            "ff_" => self
                .files
                .as_ref()
                .and_then(|files| files.text(suffix))
                .unwrap_or_else(|| key.to_string()),
            _ => key.to_string(),
        }
    }

    pub fn is_key(key: &str) -> bool {
        if key.is_empty() || key == " " || key.encode_utf16().count() < 4 {
            return false;
        }

        key.split_at_checked(3)
            .is_some_and(|(prefix, _)| PREFIXES.contains(&prefix))
    }
}

pub fn item_text(help: &Help) -> String {
    if help.color[0] == 0 {
        return String::new();
    }

    let first = help
        .line
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(0);

    let mut out = String::new();

    for (index, line) in help.line.iter().enumerate().skip(first) {
        let rgb = help.argb(index);

        out.push_str(&format!(
            "<color {} {} {}>{line}</color><n>",
            (rgb >> 16) & 255,
            (rgb >> 8) & 255,
            rgb & 255
        ));
    }

    out
}

fn index(suffix: &str) -> Option<i64> {
    let text = suffix.trim_start();

    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };

    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());

    if end == 0 {
        return Some(0);
    }

    digits[..end].parse::<i64>().ok().map(|n| sign * n)
}

pub fn ui_strings(bytes: &[u8], codepage: formats::text::Codepage) -> StringTable {
    ui::parse(bytes, codepage)
}

pub const FF_SEPARATOR: &str = "\r\n";

pub const FF_BREAK: &str = "<n>";

pub fn decode_colored_text_records(bytes: &[u8], codepage: formats::text::Codepage) -> String {
    let file = formats::text::codepage::decode(bytes, codepage);

    let mut out = String::new();

    let mut previous = String::new();

    for record in file.split_terminator(FF_SEPARATOR) {
        let units: Vec<char> = record.chars().collect();

        if units.len() <= 5 {
            continue;
        }

        let color = 0xFF00_0000 | hex(&units[..6]);

        let body: String = units.get(7..).unwrap_or_default().iter().collect();

        if color != 0xFFFF_FFFF {
            let (r, g, b) = ((color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF);

            out.push_str(&format!("<color {r} {g} {b}>"));
        }

        out.push_str(&body);

        let blank = |s: &str| s.is_empty() || s == " ";

        if !(blank(&previous) && blank(&body)) {
            out.push_str(FF_BREAK);
        }

        previous = body;
    }

    out
}

fn hex(units: &[char]) -> u32 {
    let text: String = units.iter().collect();

    let text = text.trim_start();

    let (negate, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };

    let end = digits
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(digits.len());

    let value = u32::from_str_radix(&digits[..end], 16).unwrap_or(0);

    match negate {
        true => value.wrapping_neg(),
        false => value,
    }
}
