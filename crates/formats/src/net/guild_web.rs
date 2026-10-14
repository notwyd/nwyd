use std::collections::BTreeMap;

pub const TAG_INFO: i32 = 0;

pub const TAG_MEMBERS: i32 = 1;

pub const TAG_NAME_CHECK: i32 = 3;

pub const ENDPOINTS: [&str; 6] = [
    "/guild/GuildInfo.asp",
    "/guild/GuildMemberList.asp",
    "/guild/GuildUpdateNotice.asp",
    "/guild/GuildUpdateMemberNotice.asp",
    "/guild/GuildName.asp",
    "/guild/GuildUpdateConn.asp",
];

pub const QUERIES: [&str; 6] = [
    "guildidx=%d&ally=%d&war1=%d&war2=%d&war3=%d&war4=%d",
    "guildidx=%d",
    "guildidx=%d&charidx=%d&guildnotice=",
    "charidx=%u&MemberNotice=",
    "guildname=",
    "charidx=%u&conn=%d",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Endpoint {
    Info,
    Members,
    Notice,
    MemberNotice,
    NameCheck,
    UpdateConn,
}

impl Endpoint {
    pub fn parts(self) -> (&'static str, &'static str) {
        let i = self as usize;

        (ENDPOINTS[i], QUERIES[i])
    }

    pub fn tag(self) -> i32 {
        match self {
            Endpoint::Info | Endpoint::UpdateConn => TAG_INFO,
            Endpoint::Members | Endpoint::Notice | Endpoint::MemberNotice => TAG_MEMBERS,
            Endpoint::NameCheck => TAG_NAME_CHECK,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Info {
    pub uid: i32,
    pub clan: i32,
    pub fame: i32,
    pub member_count: i32,
    pub influence: i32,
    pub name: String,
    pub master: String,
    pub notice: String,
    pub create_date: String,
    pub notice_date: String,
    pub ally: String,
    pub wars: [String; 4],
}

pub const MEMBER_COUNT_BLANK: i32 = -1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Member {
    pub char_uid: i32,
    pub rank: i32,
    pub influence: i32,
    pub last_closer: i32,
    pub conn: i32,
    pub name: String,
    pub notice: String,
    pub join_date: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct NameCheck {
    pub result: i32,
    pub name: String,
}

pub const NAME_FREE: i32 = 0;

pub const NAME_TAKEN: i32 = 1;

impl NameCheck {
    fn from_json(value: &Json) -> Self {
        NameCheck {
            result: value.get("RESULT").int(),
            name: value.get("RESULT_GUILD_NAME").text().to_string(),
        }
    }
}

pub fn name_check(body: &str) -> Option<NameCheck> {
    Some(NameCheck::from_json(parse(body)?.at(0)))
}

pub const RANK_MASTER: i32 = 4;

pub const RANK_CHIEF: [i32; 2] = [3, 2];

impl Member {
    pub fn is_chief(&self) -> bool {
        RANK_CHIEF.contains(&self.rank)
    }

    pub fn online(&self) -> bool {
        self.conn != 0
    }
}

#[derive(Debug, Clone, PartialEq)]

pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

const NULL: Json = Json::Null;

impl Json {
    pub fn get(&self, key: &str) -> &Json {
        match self {
            Json::Object(map) => map.get(key).unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    pub fn at(&self, index: usize) -> &Json {
        match self {
            Json::Array(items) => items.get(index).unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Json::Array(items) => items.len(),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn int(&self) -> i32 {
        match self {
            Json::Number(value) if value.is_finite() => {
                value.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
            }
            Json::Bool(true) => 1,
            _ => 0,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Json::Text(value) => value,
            _ => "",
        }
    }
}

impl Info {
    pub fn from_json(node: &Json) -> Info {
        Info {
            uid: node.get("GUILD_UID").int(),
            clan: node.get("GUILD_CLAN").int(),
            fame: node.get("GUILD_FAME").int(),
            member_count: node.get("GUILD_MEMBER_COUNT").int(),
            influence: node.get("GUILD_INFLUENCE").int(),
            name: node.get("GUILD_NAME").text().to_string(),
            master: node.get("CHAR_NAME").text().to_string(),
            notice: node.get("GUILD_NOTICE").text().to_string(),
            create_date: node.get("GUILD_CREATE_DATE").text().to_string(),
            notice_date: node.get("GUILD_NOTICE_DATE").text().to_string(),
            ally: node.get("ALLY").text().to_string(),
            wars: [
                node.get("WAR1").text().to_string(),
                node.get("WAR2").text().to_string(),
                node.get("WAR3").text().to_string(),
                node.get("WAR4").text().to_string(),
            ],
        }
    }
}

impl Member {
    pub fn from_json(node: &Json) -> Member {
        Member {
            char_uid: node.get("GM_CHAR_UID").int(),
            rank: node.get("GM_GUILD_LEVEL").int(),
            influence: node.get("GM_GUILD_INFLUENCE").int(),
            last_closer: node.get("GM_LAST_CLOSER").int(),
            conn: node.get("GM_CONN").int(),
            name: node.get("CHAR_NAME").text().to_string(),
            notice: node.get("GM_NOTICE").text().to_string(),
            join_date: node.get("GM_JOIN_DATE").text().to_string(),
        }
    }
}

pub fn info(body: &str) -> Option<Info> {
    Some(Info::from_json(parse(body)?.at(0)))
}

pub fn members(body: &str) -> Option<Vec<Member>> {
    let root = parse(body)?;

    Some(
        (0..root.len())
            .map(|i| Member::from_json(root.at(i)))
            .collect(),
    )
}

pub fn parse(body: &str) -> Option<Json> {
    let bytes = body.as_bytes();

    let mut at = 0usize;

    let value = read_value(bytes, &mut at)?;

    Some(value)
}

fn skip_space(bytes: &[u8], at: &mut usize) {
    loop {
        while *at < bytes.len() && bytes[*at].is_ascii_whitespace() {
            *at += 1;
        }

        if bytes[*at..].starts_with(b"//") {
            while *at < bytes.len() && bytes[*at] != b'\n' {
                *at += 1;
            }

            continue;
        }

        if bytes[*at..].starts_with(b"/*") {
            *at += 2;

            while *at < bytes.len() && !bytes[*at..].starts_with(b"*/") {
                *at += 1;
            }

            *at = (*at + 2).min(bytes.len());

            continue;
        }

        return;
    }
}

fn read_value(bytes: &[u8], at: &mut usize) -> Option<Json> {
    skip_space(bytes, at);

    match *bytes.get(*at)? {
        b'{' => read_object(bytes, at),
        b'[' => read_array(bytes, at),
        b'"' => read_string(bytes, at).map(Json::Text),
        b't' => read_literal(bytes, at, b"true", Json::Bool(true)),
        b'f' => read_literal(bytes, at, b"false", Json::Bool(false)),
        b'n' => read_literal(bytes, at, b"null", Json::Null),
        _ => read_number(bytes, at),
    }
}

fn read_literal(bytes: &[u8], at: &mut usize, word: &[u8], value: Json) -> Option<Json> {
    if bytes[*at..].starts_with(word) {
        *at += word.len();

        Some(value)
    } else {
        None
    }
}

fn read_number(bytes: &[u8], at: &mut usize) -> Option<Json> {
    let start = *at;

    if matches!(bytes.get(*at), Some(b'-' | b'+')) {
        *at += 1;
    }

    while matches!(
        bytes.get(* at), Some(c) if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E'
        | b'-' | b'+')
    ) {
        *at += 1;
    }

    if *at == start {
        return None;
    }

    std::str::from_utf8(&bytes[start..*at])
        .ok()?
        .parse::<f64>()
        .ok()
        .map(Json::Number)
}

fn read_string(bytes: &[u8], at: &mut usize) -> Option<String> {
    if bytes.get(*at) != Some(&b'"') {
        return None;
    }

    *at += 1;

    let mut out = String::new();

    loop {
        let byte = *bytes.get(*at)?;

        *at += 1;

        match byte {
            b'"' => return Some(out),
            b'\\' => {
                let escape = *bytes.get(*at)?;

                *at += 1;

                match escape {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = bytes.get(*at..*at + 4)?;

                        *at += 4;

                        let code = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;

                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => return None,
                }
            }
            _ => {
                let start = *at - 1;

                let mut end = start + 1;

                while end < bytes.len() && bytes[end] != b'"' && bytes[end] != b'\\' {
                    end += 1;
                }

                out.push_str(&String::from_utf8_lossy(&bytes[start..end]));

                *at = end;
            }
        }
    }
}

fn read_array(bytes: &[u8], at: &mut usize) -> Option<Json> {
    *at += 1;

    let mut items = Vec::new();

    skip_space(bytes, at);

    if bytes.get(*at) == Some(&b']') {
        *at += 1;

        return Some(Json::Array(items));
    }

    loop {
        items.push(read_value(bytes, at)?);

        skip_space(bytes, at);

        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b']') => {
                *at += 1;

                return Some(Json::Array(items));
            }
            _ => return None,
        }
    }
}

fn read_object(bytes: &[u8], at: &mut usize) -> Option<Json> {
    *at += 1;

    let mut map = BTreeMap::new();

    skip_space(bytes, at);

    if bytes.get(*at) == Some(&b'}') {
        *at += 1;

        return Some(Json::Object(map));
    }

    loop {
        skip_space(bytes, at);

        let key = read_string(bytes, at)?;

        skip_space(bytes, at);

        if bytes.get(*at) != Some(&b':') {
            return None;
        }

        *at += 1;

        map.insert(key, read_value(bytes, at)?);

        skip_space(bytes, at);

        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;

                return Some(Json::Object(map));
            }
            _ => return None,
        }
    }
}
