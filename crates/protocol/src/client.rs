use crate::header;

pub const LOGIN_LAUNCHER: u16 = 0x20C;

pub const LOGIN_MANUAL: u16 = 0x20D;

pub const KEEPALIVE: u16 = 0x3A0;

pub const QUIT: u16 = 0x3AE;

pub const LEAVE_WORLD: u16 = 0x215;

pub const WHO_IS: u16 = 0x369;

pub const ENTITY_REREQUEST: u16 = 0x2EC;

pub const MOVE: u16 = 0x36C;

pub const MOVE_STOP: u16 = 0x2CB;

pub const ACT: u16 = 0x368;

pub const OPEN_GATE: u16 = formats::net::gate_command::OPCODE;

pub const QUICK_BAR: u16 = formats::net::enter::QUICK_BAR;

pub const SUMMON_RELOCATION: u16 = 0x334;

pub const OPEN_GATE_LEN: usize = formats::net::gate_command::FRAME_LEN;

pub const LOGIN_LEN: usize = 0x7C;

pub const KEEPALIVE_LEN: usize = 0x0C;

pub const QUIT_LEN: usize = 0x10;

pub const LEAVE_WORLD_LEN: usize = header::LEN;

pub const SUMMON_RELOCATION_LEN: usize = 0xA0;

pub const SUMMON_RELOCATION_TARGET_OFF: usize = 0x0C;

pub const SUMMON_RELOCATION_TARGET_LEN: usize = 0x10;

pub const SUMMON_RELOCATION_NAME_OFF: usize = 0x1C;

pub const SUMMON_RELOCATION_NAME_LEN: usize = 0x80;

pub const SUMMON_RELOCATION_TARGET: &str = "relo";

pub const PASSWORD_OFF: usize = 0x0C;

pub const PASSWORD_LEN: usize = 12;

pub const ACCOUNT_OFF: usize = 0x18;

pub const ACCOUNT_LEN: usize = 16;

pub const EXTRA_OFF: usize = 0x28;

pub const EXTRA_LEN: usize = 52;

pub const VERSION_OFF: usize = 0x5C;

pub const ONE_OFF: usize = 0x60;

pub const GUID_OFF: usize = 0x64;

pub const GUID_DWORDS: usize = 4;

pub const MAC_OFF: usize = 0x74;

pub const MAC_LEN: usize = 6;

pub const SHIPPED_VERSION: i32 = 0xC8C9 * 0x1000 + 0x301;

pub const VERSION: i32 = 0x0C9D_8301;

pub const PROXY_DLL: &str = "version.dll";

pub const PROXY_ANCHOR: [u8; 10] = [0x6A, 0x04, 0x8D, 0x54, 0x24, 0x24, 0xC7, 0x44, 0x24, 0x24];

const PROXY_ANCHOR_735: [u8; 10] = [0x6A, 0x04, 0x8D, 0x54, 0x24, 0x1C, 0xC7, 0x44, 0x24, 0x1C];

pub fn version_from_proxy(dll: &[u8]) -> Option<i32> {
    let mut found = None;

    for candidate in dll.windows(PROXY_ANCHOR.len() + 4) {
        let prefix = &candidate[..PROXY_ANCHOR.len()];

        if prefix != PROXY_ANCHOR && prefix != PROXY_ANCHOR_735 {
            continue;
        }

        if found.is_some() {
            return None;
        }

        let at = PROXY_ANCHOR.len();

        let word = u32::from_le_bytes([
            candidate[at],
            candidate[at + 1],
            candidate[at + 2],
            candidate[at + 3],
        ]);

        found = Some(word);
    }

    found.map(|word| word.wrapping_shl(12).wrapping_add(0x301) as i32)
}

pub const ACCOUNT_MIN: usize = 4;

pub const ACCOUNT_MAX: usize = 14;

pub const PASSWORD_MIN: usize = 4;

pub const PASSWORD_MAX: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fingerprint {
    pub guid: [u32; GUID_DWORDS],
    pub mac: [u8; MAC_LEN],
}

impl Fingerprint {
    pub const ZERO: Fingerprint = Fingerprint {
        guid: [0; GUID_DWORDS],
        mac: [0; MAC_LEN],
    };

    pub fn from_adapter(adapter_name: &str, mac: [u8; MAC_LEN]) -> Fingerprint {
        let mut spaced = String::with_capacity(adapter_name.len() + 4);

        let mut kept = 0usize;

        for c in adapter_name.chars() {
            if c == '{' || c == '}' || c == '-' {
                continue;
            }

            spaced.push(c);

            kept += 1;

            if kept.is_multiple_of(8) {
                spaced.push(' ');
            }
        }

        let mut guid = [0u32; GUID_DWORDS];

        for (slot, group) in guid.iter_mut().zip(spaced.split_ascii_whitespace()) {
            match u32::from_str_radix(group, 16) {
                Ok(value) => *slot = value,
                Err(_) => break,
            }
        }

        Fingerprint { guid, mac }
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.guid;

        write!(
            f,
            "{{{a:08X}-{:04X}-{:04X}-{:04X}-{:04X}{d:08X}}}",
            b >> 16,
            b & 0xFFFF,
            c >> 16,
            c & 0xFFFF
        )?;

        for (n, byte) in self.mac.iter().enumerate() {
            write!(f, "{}{byte:02X}", if n == 0 { "/" } else { "-" })?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Login<'a> {
    Manual { account: &'a str, password: &'a str },
    Redirect { account: &'a str, token: &'a [u8] },
    Launcher { account: &'a str, password: &'a str },
}

impl Login<'_> {
    pub fn opcode(&self) -> u16 {
        match self {
            Login::Launcher { .. } => LOGIN_LAUNCHER,
            Login::Manual { .. } | Login::Redirect { .. } => LOGIN_MANUAL,
        }
    }

    pub fn account(&self) -> &str {
        match self {
            Login::Manual { account, .. }
            | Login::Redirect { account, .. }
            | Login::Launcher { account, .. } => account,
        }
    }
}

fn write_field(dst: &mut [u8], text: &str) {
    let bytes = formats::text::codepage::encode(text, formats::text::Codepage::Windows1252);

    let room = dst.len().saturating_sub(1);

    let take = bytes.len().min(room);

    dst[..take].copy_from_slice(&bytes[..take]);
}

fn write_name(dst: &mut [u8], text: &str) {
    let bytes = formats::text::codepage::encode(text, formats::text::Codepage::Windows1252);

    let room = dst.len().saturating_sub(1);

    let take = bytes.len().min(room);

    dst[..take].copy_from_slice(&bytes[..take]);
}

fn write_redirect_field(dst: &mut [u8], bytes: &[u8]) {
    dst.fill(0);

    let take = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());

    let take = take.min(dst.len());

    dst[..take].copy_from_slice(&bytes[..take]);
}

pub fn login(request: &Login<'_>, fingerprint: &Fingerprint, version: i32) -> Vec<u8> {
    let mut out = vec![0u8; LOGIN_LEN];

    header::set_opcode(&mut out, request.opcode());

    match request {
        Login::Manual { password, .. } => {
            write_field(
                &mut out[PASSWORD_OFF..PASSWORD_OFF + PASSWORD_LEN],
                password,
            );
        }
        Login::Redirect { .. } | Login::Launcher { .. } => {}
    }

    let account = &mut out[ACCOUNT_OFF..ACCOUNT_OFF + ACCOUNT_LEN];

    match request {
        Login::Launcher { .. } => write_name(account, request.account()),
        Login::Redirect { .. } => {
            let bytes = formats::text::codepage::encode(
                request.account(),
                formats::text::Codepage::Windows1252,
            );

            write_redirect_field(account, &bytes);
        }
        _ => write_field(account, request.account()),
    }

    match request {
        Login::Launcher { password, .. } => {
            write_name(&mut out[EXTRA_OFF..EXTRA_OFF + EXTRA_LEN], password);
        }
        Login::Redirect { token, .. } => {
            write_redirect_field(&mut out[EXTRA_OFF..EXTRA_OFF + EXTRA_LEN], token);
        }
        Login::Manual { .. } => {}
    }

    out[VERSION_OFF..VERSION_OFF + 4].copy_from_slice(&version.to_le_bytes());

    out[ONE_OFF..ONE_OFF + 4].copy_from_slice(&1i32.to_le_bytes());

    for (n, dword) in fingerprint.guid.iter().enumerate() {
        let at = GUID_OFF + n * 4;

        out[at..at + 4].copy_from_slice(&dword.to_le_bytes());
    }

    out[MAC_OFF..MAC_OFF + MAC_LEN].copy_from_slice(&fingerprint.mac);

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credentials {
    AccountTooShort,
    AccountTooLong,
    PasswordTooShort,
    PasswordTooLong,
}

impl std::fmt::Display for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credentials::AccountTooShort => {
                write!(f, "the account is shorter than {ACCOUNT_MIN} characters")
            }
            Credentials::AccountTooLong => {
                write!(f, "the account is longer than {ACCOUNT_MAX} characters")
            }
            Credentials::PasswordTooLong => {
                write!(f, "the password is longer than {PASSWORD_MAX} characters")
            }
            Credentials::PasswordTooShort => {
                write!(f, "the password is shorter than {PASSWORD_MIN} characters")
            }
        }
    }
}

pub fn check(account: &str, password: &str) -> Result<(), Credentials> {
    if account.chars().count() < ACCOUNT_MIN {
        return Err(Credentials::AccountTooShort);
    }

    if account.chars().count() > ACCOUNT_MAX {
        return Err(Credentials::AccountTooLong);
    }

    if password.chars().count() < PASSWORD_MIN {
        return Err(Credentials::PasswordTooShort);
    }

    if password.chars().count() > PASSWORD_MAX {
        return Err(Credentials::PasswordTooLong);
    }

    Ok(())
}

pub fn keepalive() -> Vec<u8> {
    let mut out = vec![0u8; KEEPALIVE_LEN];

    header::set_opcode(&mut out, KEEPALIVE);

    out
}

pub fn quit() -> Vec<u8> {
    let mut out = vec![0u8; QUIT_LEN];

    header::set_opcode(&mut out, QUIT);

    out
}

pub fn leave_world(client_id: u16) -> Vec<u8> {
    let mut out = vec![0u8; LEAVE_WORLD_LEN];

    header::set_opcode(&mut out, LEAVE_WORLD);

    out[header::CLIENT_ID_OFF..header::CLIENT_ID_OFF + 2].copy_from_slice(&client_id.to_le_bytes());

    out
}

pub const WHO_IS_LEN: usize = 0x10;

pub const ENTITY_REREQUEST_LEN: usize = 0x18;

pub const MOVE_LEN: usize = header::LEN + formats::net::action::PAYLOAD_LEN;

pub const MOVE_STOP_LEN: usize = 0x24;

pub const MOVE_STOP_NEXT_X_OFF: usize = 0x0C;

pub const MOVE_STOP_NEXT_Y_OFF: usize = 0x10;

pub const MOVE_STOP_CURRENT_X_OFF: usize = 0x14;

pub const MOVE_STOP_CURRENT_Y_OFF: usize = 0x18;

pub const MOVE_STOP_LAST_X_OFF: usize = 0x1C;

pub const MOVE_STOP_LAST_Y_OFF: usize = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveStop {
    pub next: (i32, i32),
    pub current: (i32, i32),
    pub last: (i32, i32),
}

pub fn who_is(entity: u16) -> Vec<u8> {
    let mut out = vec![0u8; WHO_IS_LEN];

    header::set_opcode(&mut out, WHO_IS);

    out[header::LEN..header::LEN + 4].copy_from_slice(&u32::from(entity).to_le_bytes());

    out
}

pub fn rerequest_entity(entity: u16, arg: u32) -> Vec<u8> {
    let mut out = vec![0u8; ENTITY_REREQUEST_LEN];

    header::set_opcode(&mut out, ENTITY_REREQUEST);

    out[header::LEN..header::LEN + 4].copy_from_slice(&2u32.to_le_bytes());

    out[header::LEN + 4..header::LEN + 8].copy_from_slice(&arg.to_le_bytes());

    out[header::LEN + 8..header::LEN + 12].copy_from_slice(&u32::from(entity).to_le_bytes());

    out
}

pub fn move_to(action: &formats::net::action::Action) -> Vec<u8> {
    action_frame(MOVE, action)
}

pub fn clock_desync(position: (i32, i32), speed: i32, code: u8) -> [u8; 0x34] {
    let mut out = [0; 0x34];

    header::set_opcode(&mut out, 0x36C);

    out[0x0C..0x0E].copy_from_slice(&(position.0 as u16).to_le_bytes());

    out[0x0E..0x10].copy_from_slice(&(position.1 as u16).to_le_bytes());

    out[0x14..0x18].copy_from_slice(&speed.to_le_bytes());

    out[0x18..0x1A].copy_from_slice(&0x3050u16.to_le_bytes());

    out[0x1A] = code;

    out
}

pub fn clock_drift(seconds: u32) -> [u8; 24] {
    let mut out = [0; 24];

    header::set_opcode(&mut out, 0x2EC);

    out[12..16].copy_from_slice(&1u32.to_le_bytes());

    out[16..20].copy_from_slice(&seconds.to_le_bytes());

    out
}

pub fn move_stop(stop: MoveStop) -> Vec<u8> {
    let mut out = vec![0u8; MOVE_STOP_LEN];

    header::set_opcode(&mut out, MOVE_STOP);

    for (offset, value) in [
        (MOVE_STOP_NEXT_X_OFF, stop.next.0),
        (MOVE_STOP_NEXT_Y_OFF, stop.next.1),
        (MOVE_STOP_CURRENT_X_OFF, stop.current.0),
        (MOVE_STOP_CURRENT_Y_OFF, stop.current.1),
        (MOVE_STOP_LAST_X_OFF, stop.last.0),
        (MOVE_STOP_LAST_Y_OFF, stop.last.1),
    ] {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    out
}

pub fn act(action: &formats::net::action::Action) -> Vec<u8> {
    action_frame(ACT, action)
}

fn action_frame(opcode: u16, action: &formats::net::action::Action) -> Vec<u8> {
    let mut out = vec![0u8; MOVE_LEN];

    header::set_opcode(&mut out, opcode);

    out[header::LEN..].copy_from_slice(&action.to_bytes());

    out
}

pub const QUICK_BAR_LEN: usize = formats::net::enter::QUICK_BAR_FRAME_LEN;

pub fn quick_bar(bar: &[u8; formats::net::enter::QUICK_SLOTS], class: u8) -> Vec<u8> {
    let mut out = vec![0u8; QUICK_BAR_LEN];

    header::set_opcode(&mut out, QUICK_BAR);

    out[header::LEN..].copy_from_slice(&formats::net::enter::unrebase_for_send(bar, class));

    out
}

pub fn open_gate(entity: u16) -> Vec<u8> {
    let ask = formats::net::gate_command::Command {
        entity: u32::from(entity),
        state: formats::net::gate_command::ASK_OPEN,
        fill: 0,
    };

    let mut out = vec![0u8; OPEN_GATE_LEN];

    header::set_opcode(&mut out, OPEN_GATE);

    out[header::LEN..].copy_from_slice(&ask.to_bytes());

    out
}

pub fn summon_relocation(requester: &str) -> Vec<u8> {
    let mut out = vec![0u8; SUMMON_RELOCATION_LEN];

    header::set_opcode(&mut out, SUMMON_RELOCATION);

    write_field(
        &mut out[SUMMON_RELOCATION_TARGET_OFF
            ..SUMMON_RELOCATION_TARGET_OFF + SUMMON_RELOCATION_TARGET_LEN],
        SUMMON_RELOCATION_TARGET,
    );

    write_name(
        &mut out
            [SUMMON_RELOCATION_NAME_OFF..SUMMON_RELOCATION_NAME_OFF + SUMMON_RELOCATION_NAME_LEN],
        requester,
    );

    out
}

pub const PICK_UP_LEN: usize = formats::net::ground_item::PICK_UP_FRAME_LEN;

pub fn pick_up_item(ask: formats::net::ground_item::PickUp) -> Vec<u8> {
    let mut out = vec![0u8; PICK_UP_LEN];

    header::set_opcode(&mut out, formats::net::ground_item::PICK_UP);

    out[header::LEN..].copy_from_slice(&ask.to_bytes());

    out
}
