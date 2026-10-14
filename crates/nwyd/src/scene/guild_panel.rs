use crate::frame::PanelId;
use crate::scene::msgbox;
use formats::net::guild_web::{self, Info, Member};

pub const CONTENT: PanelId = super::panels::CONTENT_GUILD;

pub const PAGE_INFO: PanelId = super::panels::CONTENT_GUILD_INFO;

pub const PAGE_MEMBER: PanelId = super::panels::CONTENT_GUILD_MEMBER;

pub const NOTICE_EDIT: PanelId = super::panels::CONTENT_GUILD_NOTICE;

pub const CREATE: PanelId = super::panels::CONTENT_GUILD_CREATE;

pub const MEMBER_POPUP: PanelId = 0x32;

pub const MEMBER_POPUP_SCRIPT: &[u8] = b"ui/Panel/game/Guild/Guild_MemberPopup.pane";

pub const HOST_STRDEF: usize = 1606;

pub fn web_host(strings: &ui::Strings) -> Option<String> {
    strings
        .strdef
        .as_ref()
        .map(|table| table.get(HOST_STRDEF))
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
}

pub const MEMBER_ITEM_SCRIPT: &[u8] = b"ui/Panel/game/Guild/Guild_MemberItem.pane";

pub const TAB_RECT_FROM: &str = "Align";

pub const TAB_CAPTIONS: [&str; 2] = ["Info", "Member"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum Page {
    #[default]
    Info,
    Member,
}

impl Page {
    pub fn index(self) -> usize {
        match self {
            Page::Info => 0,
            Page::Member => 1,
        }
    }
}

pub const INFO_VALUES: [&str; 9] = [
    "GuildName",
    "MasterName",
    "MemberCount",
    "GuildFame",
    "GuildInfluence",
    "CreateDate",
    "NoticeDate",
    "ally",
    "war",
];

pub const INFO_LABELS: std::ops::Range<usize> = 1..10;

pub const NOTICE: &str = "Notice";

pub const NOTICE_EDIT_BUTTON: &str = "NoticeEdit";

pub const CREATE_BUTTON: &str = "CreateGuild";

pub const MEMBER_LIST: &str = "ListView";

pub const NOTICE_EDIT_FIELD: &str = "noticeedit";

pub const NOTICE_OK: &str = "noticeok";

pub const NOTICE_CANCEL: &str = "cancle";

pub const CREATE_FIELD: &str = "Text";

pub const CREATE_OK: &str = "ok";

pub const CREATE_DISMISS: [&str; 2] = ["close", "cancle"];

pub const POPUP_TITLE: &str = "Title";

pub const POPUP_TEMPLATE: &str = "Menu";

pub const POPUP_BUTTON: &str = "BtnMemberMenu";

pub const POPUP_BACKGROUND: &str = "BG";

pub const POPUP_BACKGROUND_MARGIN: i32 = 0x16;

pub const RANK_MASTER: i32 = guild_web::RANK_MASTER;

pub const RANK_CHIEF: [i32; 2] = guild_web::RANK_CHIEF;

pub const MAX_CHIEFS: i32 = 2;

pub mod strings {

    pub const RANK_MASTER: &str = "uu_303";

    pub const RANK_CHIEF: &str = "uu_304";

    pub const TODAY: &str = "uu_309";

    pub const DAYS_AGO: &str = "uu_305";

    pub const MONTHS_AGO: &str = "uu_310";

    pub const YEARS_AGO: &str = "uu_311";

    pub const CHANNEL: &str = "uu_125";

    pub const MENU: [&str; 7] = [
        "uu_294", "uu_296", "uu_295", "uu_306", "uu_307", "uu_297", "uu_308",
    ];
}

pub const WAR_SEPARATOR: &str = "   ";

pub const MEMBER_COUNT_FORMAT: [&str; 2] = ["%d/52 ", "%d/13 "];

pub const NAME_TOO_SHORT: usize = 15;

pub const NAME_TOO_LONG: usize = 16;

pub const NAME_INVALID: usize = 17;

pub const GUILD_NAME_MIN: usize = 4;

pub const GUILD_NAME_MAX: usize = 11;

pub const MEMBER_NOTICE_MAX: usize = 0x3C;

pub const BOX_KIND: i32 = CONTENT as i32;

pub mod context {

    pub const MEMBER_NOTICE: u32 = 0x3F;

    pub const TRANSFER: u32 = 0x41;

    pub const DISBAND: u32 = 0x42;

    pub const LEAVE: u32 = 0x43;

    pub const EXPEL: u32 = 0x44;
}

pub mod ask {

    pub const EXPEL: usize = 1605;

    pub const TRANSFER: usize = 1601;

    pub const MEMBER_NOTICE: usize = 1598;

    pub const LEAVE: usize = 1600;

    pub const DISBAND: usize = 1599;

    pub const TOO_MANY_CHIEFS: usize = 1609;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Session {
    pub guild: u32,
    pub rank: i32,
    pub charidx: Option<u32>,
    pub ally: u32,
}

impl Session {
    pub fn in_a_guild(&self) -> bool {
        self.guild != 0
    }

    pub fn is_master(&self) -> bool {
        self.rank == RANK_MASTER
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Guild {
    pub session: Session,
    pub page: Page,
    pub info: Option<Info>,
    pub members: Vec<Member>,
    pub picked: Option<Picked>,
    pub pending_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Picked {
    pub char_uid: u32,
    pub name: String,
    pub rank: i32,
}

impl Guild {
    pub fn chiefs(&self) -> i32 {
        i32::try_from(self.members.iter().filter(|m| m.is_chief()).count()).unwrap_or(i32::MAX)
    }

    pub fn picked_is_me(&self) -> bool {
        match (&self.picked, self.session.charidx) {
            (Some(picked), Some(mine)) => picked.char_uid == mine,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Label {
    pub name: &'static str,
    pub text: String,
}

pub fn info_labels(guild: &Guild) -> Vec<Label> {
    let blank = |name| Label {
        name,
        text: String::new(),
    };

    let Some(info) = guild.info.as_ref().filter(|_| guild.session.in_a_guild()) else {
        return INFO_VALUES
            .iter()
            .chain([&NOTICE])
            .map(|name| blank(*name))
            .collect();
    };

    let text = |name: &'static str, text: String| Label { name, text };

    let blanked = info.member_count == guild_web::MEMBER_COUNT_BLANK;

    vec![
        text("GuildName", info.name.clone()),
        text("MasterName", info.master.clone()),
        if blanked {
            blank("MemberCount")
        } else {
            text("MemberCount", member_count(info.member_count))
        },
        if blanked {
            blank("GuildFame")
        } else {
            text("GuildFame", info.fame.to_string())
        },
        if blanked {
            blank("GuildInfluence")
        } else {
            text("GuildInfluence", info.influence.to_string())
        },
        text("CreateDate", info.create_date.clone()),
        text("NoticeDate", info.notice_date.clone()),
        text("ally", info.ally.clone()),
        text("war", info.wars.join(WAR_SEPARATOR)),
        text(NOTICE, info.notice.clone()),
    ]
}

pub fn member_count(count: i32) -> String {
    MEMBER_COUNT_FORMAT[0].replace("%d", &count.to_string())
}

pub fn info_visibility(guild: &Guild) -> Vec<(String, bool)> {
    let joined = guild.session.in_a_guild();

    let rank = if (0..=RANK_MASTER).contains(&guild.session.rank) {
        guild.session.rank
    } else {
        0
    };

    let mut out: Vec<(String, bool)> = INFO_LABELS.map(|n| (format!("Info{n}"), joined)).collect();

    out.push((NOTICE.to_string(), joined));

    out.push((
        NOTICE_EDIT_BUTTON.to_string(),
        joined && rank == RANK_MASTER,
    ));

    out.push((CREATE_BUTTON.to_string(), !joined));

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Row {
    pub user_name: String,
    pub guild_level: Option<&'static str>,
    pub member_notice: String,
    pub conn_date: ConnDate,
    pub join_date: String,
    pub guild_influence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum ConnDate {
    Today,
    Ago { days: i32, word: &'static str },
    Online { channel: i32 },
}

impl ConnDate {
    pub fn text(&self, translate: &dyn Fn(&str) -> String) -> String {
        match self {
            ConnDate::Today => translate(strings::TODAY),
            ConnDate::Ago { days, word } => format!("{days}{}", translate(word)),
            ConnDate::Online { channel } => {
                format!("{}{channel}", translate(strings::CHANNEL))
            }
        }
    }
}

pub const D_517_RAW_DAY_COUNT: bool = true;

pub const CONN_YEAR: i32 = 0x16d;

pub const CONN_MONTH: i32 = 0x1e;

pub fn conn_date(member: &Member) -> ConnDate {
    if member.online() {
        return ConnDate::Online {
            channel: member.conn,
        };
    }

    let days = member.last_closer;

    if days == 0 {
        return ConnDate::Today;
    }

    let word = if days / CONN_YEAR != 0 {
        strings::YEARS_AGO
    } else if days / CONN_MONTH != 0 {
        strings::MONTHS_AGO
    } else {
        strings::DAYS_AGO
    };

    ConnDate::Ago { days, word }
}

pub fn member_rows(guild: &Guild) -> Vec<Row> {
    guild
        .members
        .iter()
        .filter(|member| member.char_uid != 0)
        .map(|member| Row {
            user_name: member.name.clone(),
            guild_level: match member.rank {
                RANK_MASTER => Some(strings::RANK_MASTER),
                rank if RANK_CHIEF.contains(&rank) => Some(strings::RANK_CHIEF),
                _ => None,
            },
            member_notice: member.notice.clone(),
            conn_date: conn_date(member),
            join_date: member.join_date.clone(),
            guild_influence: member.influence.to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]

pub enum Entry {
    PromoteChief = 1,
    Expel = 2,
    ReleaseChief = 3,
    Transfer = 4,
    ChangeMessage = 5,
    Leave = 6,
    Disband = 7,
}

pub const ENTRIES: [Entry; 7] = [
    Entry::PromoteChief,
    Entry::Expel,
    Entry::ReleaseChief,
    Entry::Transfer,
    Entry::ChangeMessage,
    Entry::Leave,
    Entry::Disband,
];

impl Entry {
    pub fn caption(self) -> &'static str {
        strings::MENU[self as usize - 1]
    }

    pub fn tag(self) -> u32 {
        self as u32
    }

    pub fn from_tag(tag: u32) -> Option<Entry> {
        ENTRIES.iter().copied().find(|entry| entry.tag() == tag)
    }
}

pub fn popup_opens(guild: &Guild) -> bool {
    let Some(picked) = guild.picked.as_ref() else {
        return false;
    };

    if !guild.session.in_a_guild() {
        return false;
    }

    if guild.session.rank == 0 && !guild.picked_is_me() {
        return false;
    }

    if guild.session.is_master() {
        return true;
    }

    picked.rank != RANK_MASTER
}

pub fn entries(guild: &Guild) -> Vec<Entry> {
    if !popup_opens(guild) {
        return Vec::new();
    }

    let Some(picked) = guild.picked.as_ref() else {
        return Vec::new();
    };

    let mine = guild.session.rank;

    let theirs = picked.rank;

    if guild.picked_is_me() {
        let mut out = vec![Entry::ChangeMessage];

        if mine != RANK_MASTER {
            out.push(Entry::Leave);
        } else {
            out.push(Entry::Disband);
        }

        return out;
    }

    if theirs == mine {
        return Vec::new();
    }

    let mut out = Vec::new();

    if mine == RANK_MASTER && theirs <= 1 {
        out.push(Entry::PromoteChief);
    }

    if theirs > mine {
        return Vec::new();
    }

    out.push(Entry::Expel);

    if RANK_CHIEF.contains(&theirs) && mine == RANK_MASTER {
        out.push(Entry::ReleaseChief);

        out.push(Entry::Transfer);
    }

    out
}

#[derive(Debug, Clone, PartialEq)]

pub enum Action {
    Send(Vec<u8>),
    Ask(msgbox::Request),
    AskForMember {
        context: u32,
        record: usize,
        name: String,
    },
    AskWithEditLimit(msgbox::Request, i32),
    Toast(usize),
    TimedNotice(usize, u32),
    Refetch(Fetch),
    RefetchAfter(Fetch, u32),
    Repaint,
    Post(Post),
    UpdateConn(ConnectionReport),
    Park(String),
    Hide(PanelId),
    Show(PanelId),
    Go(Page),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct ConnectionReport {
    pub charidx: u32,
    pub report_channel: bool,
    pub channel: i32,
}

impl ConnectionReport {
    pub fn body(self) -> String {
        let conn = if self.report_channel { self.channel } else { 0 };

        format!("charidx={}&conn={conn}", self.charidx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Fetch {
    Info,
    Members,
}

impl Fetch {
    pub fn endpoint(self) -> guild_web::Endpoint {
        match self {
            Fetch::Info => guild_web::Endpoint::Info,
            Fetch::Members => guild_web::Endpoint::Members,
        }
    }
}

impl Post {
    pub fn endpoint(&self) -> guild_web::Endpoint {
        match self {
            Post::Notice(_) => guild_web::Endpoint::Notice,
            Post::MemberNotice(_) => guild_web::Endpoint::MemberNotice,
            Post::Create(_) => guild_web::Endpoint::NameCheck,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Post::Notice(t) | Post::MemberNotice(t) | Post::Create(t) => t,
        }
    }
}

pub const SUB_LEN: usize = 0x38;

pub mod field {

    pub const LEN: usize = 0x00;

    pub const OPCODE: usize = 0x04;

    pub const CLIENT: usize = 0x06;

    pub const COMMAND: usize = 0x0C;

    pub const GUILD: usize = 0x10;

    pub const SECOND_ID: usize = 0x14;

    pub const SENTINEL: usize = 0x1A;

    pub const TARGET: usize = 0x1C;

    pub const TARGET_RANK: usize = 0x20;

    pub const FLAG: usize = 0x22;
}

pub const DISBAND_OPCODE: u16 = 0x3E1A;

pub const DISBAND_LEN: usize = 0x10;

pub const DISBAND_GUILD_AT: usize = 0x0C;

pub mod command {

    pub const REMOVE: u32 = 2;

    pub const RANK: u32 = 3;
}

pub mod rank_flag {

    pub const PROMOTE_CHIEF: u16 = 2;

    pub const RELEASE_CHIEF: u16 = 0;

    pub const TRANSFER: u16 = 4;
}

#[allow(clippy::too_many_arguments)]

pub fn guild_member_command_frame(
    client: u16,
    command: u32,
    guild: u32,
    target: u32,
    target_rank: i16,
    flag: Option<u16>,
    second_id: Option<u32>,
    sentinel: Option<u16>,
) -> Vec<u8> {
    let mut frame = vec![0u8; SUB_LEN];

    let put16 = |frame: &mut Vec<u8>, at: usize, value: u16| {
        frame[at..at + 2].copy_from_slice(&value.to_le_bytes());
    };

    let put32 = |frame: &mut Vec<u8>, at: usize, value: u32| {
        frame[at..at + 4].copy_from_slice(&value.to_le_bytes());
    };

    put16(&mut frame, field::LEN, SUB_LEN as u16);

    put16(&mut frame, field::OPCODE, formats::net::guild::OPCODE);

    put16(&mut frame, field::CLIENT, client);

    put32(&mut frame, field::COMMAND, command);

    put32(&mut frame, field::GUILD, guild);

    put32(&mut frame, field::TARGET, target);

    put16(&mut frame, field::TARGET_RANK, target_rank as u16);

    if let Some(flag) = flag {
        put16(&mut frame, field::FLAG, flag);
    }

    if let Some(second) = second_id {
        put32(&mut frame, field::SECOND_ID, second);
    }

    if let Some(sentinel) = sentinel {
        put16(&mut frame, field::SENTINEL, sentinel);
    }

    frame
}

pub fn disband_frame(client: u16, guild: u32) -> Vec<u8> {
    let mut frame = vec![0u8; DISBAND_LEN];

    frame[field::LEN..field::LEN + 2].copy_from_slice(&(DISBAND_LEN as u16).to_le_bytes());

    frame[field::OPCODE..field::OPCODE + 2].copy_from_slice(&DISBAND_OPCODE.to_le_bytes());

    frame[field::CLIENT..field::CLIENT + 2].copy_from_slice(&client.to_le_bytes());

    frame[DISBAND_GUILD_AT..DISBAND_GUILD_AT + 4].copy_from_slice(&guild.to_le_bytes());

    frame
}

pub const CREATE_OPCODE: u16 = 0x3E16;

pub const CREATE_LEN: usize = 0x38;

pub const CREATE_NAME_AT: usize = 0x18;

pub const CREATE_TRAILER_AT: usize = 0x22;

pub const CREATE_NAME_MAX: usize = CREATE_TRAILER_AT - CREATE_NAME_AT;

pub const NAME_IN_USE: usize = 1603;

pub fn create_frame(client: u16, name: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; CREATE_LEN];

    frame[field::LEN..field::LEN + 2].copy_from_slice(&(CREATE_LEN as u16).to_le_bytes());

    frame[field::OPCODE..field::OPCODE + 2].copy_from_slice(&CREATE_OPCODE.to_le_bytes());

    frame[field::CLIENT..field::CLIENT + 2].copy_from_slice(&client.to_le_bytes());

    let take = name.len().min(CREATE_NAME_MAX);

    frame[CREATE_NAME_AT..CREATE_NAME_AT + take].copy_from_slice(&name[..take]);

    frame
}

pub fn name_checked(
    guild: &Guild,
    answer: &formats::net::guild_web::NameCheck,
    client: u16,
) -> Vec<Action> {
    use formats::net::guild_web::{NAME_FREE, NAME_TAKEN};

    if answer.result == NAME_TAKEN {
        return vec![Action::TimedNotice(NAME_IN_USE, 5000)];
    }

    if answer.result != NAME_FREE {
        return Vec::new();
    }

    let Some(parked) = guild.pending_name.as_deref() else {
        return vec![Action::TimedNotice(NAME_IN_USE, 5000)];
    };

    let cp = formats::text::Codepage::Windows1252;

    let answered =
        formats::text::codepage::encode(answer.name.split('\0').next().unwrap_or(""), cp);

    let expected = formats::text::codepage::encode(parked.split('\0').next().unwrap_or(""), cp);

    if answered != expected {
        return vec![Action::TimedNotice(NAME_IN_USE, 5000)];
    }

    vec![
        Action::Send(create_frame(client, &answered)),
        Action::Hide(CREATE),
    ]
}

pub fn box_request(context: u32, body: msgbox::Body) -> msgbox::Request {
    msgbox::Request {
        caption: Some(msgbox::DEFAULT_CAPTION_STRDEF),
        kind: BOX_KIND,
        context,
        body,
        flags: msgbox::FLAGS,
    }
}

pub fn member_notice_box() -> msgbox::Request {
    msgbox::Request {
        caption: Some(msgbox::DEFAULT_CAPTION_STRDEF),
        kind: BOX_KIND,
        context: context::MEMBER_NOTICE,
        body: msgbox::Body::StrDef(ask::MEMBER_NOTICE),
        flags: msgbox::FLAGS | msgbox::PANE_EDITBOX,
    }
}

pub fn entry_click(guild: &Guild, entry: Entry, client: u16) -> Vec<Action> {
    let Some(picked) = guild.picked.as_ref() else {
        return Vec::new();
    };

    let mine = guild.session.rank;

    let theirs = picked.rank;

    let id = guild.session.guild;

    let rank16 = theirs as i16;

    let refresh = vec![Action::RefetchAfter(Fetch::Members, 500), Action::Repaint];

    match entry {
        Entry::PromoteChief => {
            if mine != RANK_MASTER || theirs > 2 {
                return Vec::new();
            }

            if guild.chiefs() > MAX_CHIEFS {
                return vec![Action::Toast(ask::TOO_MANY_CHIEFS)];
            }

            let mut out = vec![Action::Send(guild_member_command_frame(
                client,
                command::RANK,
                id,
                picked.char_uid,
                rank16,
                Some(rank_flag::PROMOTE_CHIEF),
                None,
                None,
            ))];

            out.extend(refresh);

            out
        }
        Entry::Expel => {
            if mine < 2 || mine <= theirs {
                return Vec::new();
            }

            vec![
                Action::AskForMember {
                    context: context::EXPEL,
                    record: ask::EXPEL,
                    name: picked.name.clone(),
                },
                Action::RefetchAfter(Fetch::Members, 500),
                Action::Repaint,
            ]
        }
        Entry::ReleaseChief => {
            if mine != RANK_MASTER || !RANK_CHIEF.contains(&theirs) {
                return Vec::new();
            }

            let mut out = vec![Action::Send(guild_member_command_frame(
                client,
                command::RANK,
                id,
                picked.char_uid,
                rank16,
                Some(rank_flag::RELEASE_CHIEF),
                None,
                None,
            ))];

            out.extend(refresh);

            out
        }
        Entry::Transfer => {
            if mine != RANK_MASTER || !RANK_CHIEF.contains(&theirs) {
                return Vec::new();
            }

            vec![
                Action::AskForMember {
                    context: context::TRANSFER,
                    record: ask::TRANSFER,
                    name: picked.name.clone(),
                },
                Action::RefetchAfter(Fetch::Members, 500),
                Action::Repaint,
            ]
        }
        Entry::ChangeMessage => {
            vec![
                Action::AskWithEditLimit(member_notice_box(), MEMBER_NOTICE_MAX as i32),
                Action::RefetchAfter(Fetch::Members, 500),
                Action::Repaint,
            ]
        }
        Entry::Leave => {
            if !guild.picked_is_me() && mine < 3 {
                return Vec::new();
            }

            let mut out = vec![Action::Ask(box_request(
                context::LEAVE,
                msgbox::Body::StrDef(ask::LEAVE),
            ))];

            if mine == RANK_MASTER {
                out.push(Action::Ask(box_request(
                    context::DISBAND,
                    msgbox::Body::StrDef(ask::DISBAND),
                )));

                out.extend(refresh);
            }

            out
        }
        Entry::Disband => {
            if mine != RANK_MASTER {
                return Vec::new();
            }

            vec![
                Action::Ask(box_request(
                    context::DISBAND,
                    msgbox::Body::StrDef(ask::DISBAND),
                )),
                Action::RefetchAfter(Fetch::Members, 500),
                Action::Repaint,
            ]
        }
    }
}

pub const PROMOTE_GUARD_IS_WIDER_THAN_THE_MENU: bool = true;

pub const LEAVE_FALLS_THROUGH_INTO_DISBAND: bool = true;

pub fn answered(
    guild: &Guild,
    context: u32,
    yes: bool,
    text: Option<&str>,
    client: u16,
) -> Vec<Action> {
    if !yes {
        return Vec::new();
    }

    let id = guild.session.guild;

    let picked = guild.picked.as_ref();

    let target = picked.map_or(0, |p| p.char_uid);

    let rank16 = picked.map_or(0, |p| p.rank as i16);

    let refresh = vec![Action::RefetchAfter(Fetch::Members, 500)];

    match context {
        context::MEMBER_NOTICE => {
            let Some(text) = text else {
                return Vec::new();
            };

            vec![
                Action::Post(Post::MemberNotice(escape_for_query(text))),
                Action::Refetch(Fetch::Members),
            ]
        }
        context::TRANSFER => {
            if !guild.session.is_master() {
                return Vec::new();
            }

            let mut out = vec![Action::Send(guild_member_command_frame(
                client,
                command::RANK,
                id,
                target,
                rank16,
                Some(rank_flag::TRANSFER),
                None,
                Some(0xFFFF),
            ))];

            out.extend(refresh);

            out
        }
        context::DISBAND => {
            if !guild.session.is_master() {
                return Vec::new();
            }

            let mut out = vec![Action::Send(disband_frame(client, id))];

            out.extend(refresh);

            out
        }
        context::LEAVE => {
            if guild.session.is_master() {
                return Vec::new();
            }

            let mut out = vec![Action::Send(guild_member_command_frame(
                client,
                command::REMOVE,
                id,
                target,
                0,
                None,
                Some(guild.session.charidx.unwrap_or(0)),
                None,
            ))];

            out.extend(refresh);

            out
        }
        context::EXPEL => {
            let mut out = vec![Action::Send(guild_member_command_frame(
                client,
                command::REMOVE,
                id,
                target,
                0,
                None,
                None,
                None,
            ))];

            out.extend(refresh);

            out
        }
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Post {
    Notice(String),
    MemberNotice(String),
    Create(String),
}

pub const ESCAPES: [(&str, &str); 9] = [
    ("\\", "\\\\"),
    ("\"", "\\\""),
    ("/", "\\/"),
    ("\u{8}", "\\b"),
    ("\u{c}", "\\f"),
    ("\n", "\\n"),
    ("\r", "\\r"),
    ("\t", "\\t"),
    ("'", "''"),
];

pub fn escape_for_query(text: &str) -> String {
    let mut out = text.to_string();

    for (needle, replacement) in ESCAPES {
        out = out.replace(needle, replacement);
    }

    out
}

pub fn tab_click(guild: &Guild, name: &str) -> Vec<Action> {
    if name.eq_ignore_ascii_case(TAB_CAPTIONS[0]) {
        return vec![
            Action::Go(Page::Info),
            Action::Refetch(Fetch::Info),
            Action::Repaint,
        ];
    }

    if name.eq_ignore_ascii_case(TAB_CAPTIONS[1]) && guild.session.in_a_guild() {
        return vec![
            Action::Go(Page::Member),
            Action::Refetch(Fetch::Members),
            Action::Repaint,
        ];
    }

    Vec::new()
}

pub fn info_click(name: &str) -> Vec<Action> {
    if name.eq_ignore_ascii_case(NOTICE_EDIT_BUTTON) {
        return vec![Action::Show(NOTICE_EDIT)];
    }

    if name.eq_ignore_ascii_case(CREATE_BUTTON) {
        return vec![Action::Show(CREATE)];
    }

    Vec::new()
}

pub fn notice_click(name: &str, text: &str) -> Vec<Action> {
    if name.eq_ignore_ascii_case(NOTICE_OK) {
        return vec![
            Action::Post(Post::Notice(escape_for_query(text))),
            Action::Hide(NOTICE_EDIT),
            Action::Refetch(Fetch::Info),
            Action::Repaint,
        ];
    }

    if name.eq_ignore_ascii_case(NOTICE_CANCEL) {
        return vec![Action::Hide(NOTICE_EDIT)];
    }

    Vec::new()
}

pub fn create_click(
    guild: &Guild,
    name: &str,
    typed: &[u8],
    codepage: formats::text::Codepage,
    reserved: &super::create::Reserved,
) -> Vec<Action> {
    if CREATE_DISMISS
        .iter()
        .any(|button| name.eq_ignore_ascii_case(button))
    {
        return vec![Action::Hide(CREATE)];
    }

    if !name.eq_ignore_ascii_case(CREATE_OK) {
        return Vec::new();
    }

    if guild.session.in_a_guild() {
        return vec![
            Action::Hide(CREATE),
            Action::Refetch(Fetch::Info),
            Action::RefetchAfter(Fetch::Members, 500),
            Action::Repaint,
        ];
    }

    match check_guild_name(typed, codepage, reserved) {
        Err(strdef) => vec![Action::Toast(strdef)],
        Ok(accepted) => {
            let name = formats::text::codepage::decode(&accepted, codepage);

            vec![
                Action::Park(name.clone()),
                Action::Post(Post::Create(escape_for_query(&name))),
            ]
        }
    }
}

pub fn syllable_ok(name: &[u8]) -> bool {
    if !super::create::VALIDATOR_LEN.contains(&name.len()) {
        return false;
    }

    name.iter()
        .enumerate()
        .all(|(index, &byte)| byte.is_ascii_alphanumeric() || index % 2 != 0 || syllable_lead(byte))
}

pub fn syllable_lead(byte: u8) -> bool {
    (0xB0..=0xC9).contains(&byte)
}

pub fn check_guild_name(
    typed: &[u8],
    codepage: formats::text::Codepage,
    reserved: &super::create::Reserved,
) -> Result<Vec<u8>, usize> {
    let typed = typed.split(|b| *b == 0).next().unwrap_or_default();

    if typed.len() < GUILD_NAME_MIN {
        return Err(NAME_TOO_SHORT);
    }

    if typed.len() > GUILD_NAME_MAX {
        return Err(NAME_TOO_LONG);
    }

    if !super::create::charset_ok(typed)
        || !super::create::printable_ok(typed, codepage)
        || !syllable_ok(typed)
    {
        return Err(NAME_INVALID);
    }

    let mut filtered = typed.to_vec();

    if !super::create::filter(&mut filtered) {
        return Err(NAME_INVALID);
    }

    reserved.substitute(&mut filtered, codepage);

    if filtered.windows(2).any(|pair| pair == [0xa1, 0xa1]) {
        return Err(NAME_INVALID);
    }

    Ok(typed.to_vec())
}
