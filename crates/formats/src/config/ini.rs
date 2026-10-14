use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Ini {
    sections: BTreeMap<String, Section>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

struct Section {
    name: String,
    entries: BTreeMap<String, (String, String)>,
}

pub fn parse(text: &str) -> Ini {
    let mut ini = Ini::default();

    let mut current: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('[') {
            let name = rest.split(']').next().unwrap_or("").trim();

            let key = name.to_ascii_lowercase();

            ini.sections.entry(key.clone()).or_insert_with(|| Section {
                name: name.to_string(),
                entries: BTreeMap::new(),
            });

            current = Some(key);

            continue;
        }

        let Some((k, v)) = line.split_once('=') else {
            continue;
        };

        let Some(section) = current.as_ref().and_then(|c| ini.sections.get_mut(c)) else {
            continue;
        };

        let (k, v) = (k.trim(), v.trim());

        section
            .entries
            .insert(k.to_ascii_lowercase(), (k.to_string(), v.to_string()));
    }

    ini
}

impl Ini {
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(&section.to_ascii_lowercase())?
            .entries
            .get(&key.to_ascii_lowercase())
            .map(|(_, v)| v.as_str())
    }

    pub fn get_int(&self, section: &str, key: &str) -> Option<i32> {
        let raw = self.get(section, key)?;

        let text = raw.trim_start();

        let digits: String = text
            .char_indices()
            .take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+')))
            .map(|(_, c)| c)
            .collect();

        Some(digits.parse().unwrap_or(0))
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let sk = section.to_ascii_lowercase();

        let s = self.sections.entry(sk).or_insert_with(|| Section {
            name: section.to_string(),
            entries: BTreeMap::new(),
        });

        s.entries.insert(
            key.to_ascii_lowercase(),
            (key.to_string(), value.to_string()),
        );
    }

    pub fn to_string_crlf(&self) -> String {
        let mut out = String::new();

        for s in self.sections.values() {
            out.push_str(&format!("[{}]\r\n", s.name));

            for (k, v) in s.entries.values() {
                out.push_str(&format!("{k}={v}\r\n"));
            }
        }

        out
    }

    pub fn sections(&self) -> impl Iterator<Item = &str> {
        self.sections.values().map(|s| s.name.as_str())
    }
}

pub mod guild_mark {

    use super::Ini;

    pub const FILE: &str = "Wgmrk.ini";

    pub const SECTION: &str = "GuildMark";

    pub const KEY: &str = "off";

    pub fn read(ini: &Ini) -> (bool, bool) {
        match ini.get_int(SECTION, KEY) {
            Some(v) => (v != 0, false),
            None => (false, true),
        }
    }

    pub fn toggle(ini: &mut Ini) -> bool {
        let v = ini.get_int(SECTION, KEY).unwrap_or(0);

        let next = match v {
            0 => 1,
            1 => 0,
            other => return other != 0,
        };

        ini.set(SECTION, KEY, &next.to_string());

        next != 0
    }
}

pub mod notice {

    use super::Ini;

    pub const FILE: &str = "WNotice.ini";

    pub const SECTION: &str = "Notice";

    #[derive(Debug, Clone, PartialEq, Eq)]

    pub enum Action {
        Disabled,
        AlreadyShown,
        Show(String),
    }

    pub fn url(ini: &Ini, index: u32) -> String {
        ini.get(SECTION, &format!("url_{index}"))
            .unwrap_or_default()
            .to_string()
    }

    pub fn startup_action(ini: &Ini, index: u32) -> Action {
        match ini.get_int(SECTION, "dp") {
            None => Action::Disabled,
            Some(0) => Action::AlreadyShown,
            Some(_) => match ini.get_int(SECTION, "prev") {
                Some(0) => Action::Show(url(ini, index)),
                _ => Action::AlreadyShown,
            },
        }
    }

    pub fn mark_shown(ini: &mut Ini) {
        ini.set(SECTION, "prev", "1");
    }
}

pub mod graphics {

    pub const FILE: &str = "Graphics.ini";

    pub fn render(high_texture: bool, new_camera: bool) -> String {
        format!(
            "[GRAPHICS]\r\nHigh={}\r\nCamera={}\r\n",
            high_texture as u8, new_camera as u8
        )
    }
}
