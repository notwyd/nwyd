use super::rng::Lcg;

pub fn form_value(text: &str) -> String {
    let mut encoded = String::new();

    for byte in text.bytes().take_while(|&b| b != 0) {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'@' | b'.' | b'/' | b'\\' | b'-' | b'_' | b':')
        {
            encoded.push(char::from(byte));
        } else if byte == b' ' {
            encoded.push('+');
        } else {
            const HEX: &[u8; 16] = b"0123456789abcdef";

            encoded.push('%');

            encoded.push(char::from(HEX[usize::from(byte >> 4)]));

            encoded.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }

    encoded
}

#[derive(Clone, PartialEq)]
pub struct Queued {
    pub action: super::guild_panel::Action,
    pub request: super::guild_http::Request,
}

impl std::fmt::Debug for Queued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueuedGuildRequest")
            .field("tag", &self.request.tag)
            .field("delay_ms", &self.request.delay_ms)
            .finish_non_exhaustive()
    }
}

impl PartialEq<super::guild_panel::Action> for Queued {
    fn eq(&self, other: &super::guild_panel::Action) -> bool {
        self.action == *other
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub account: String,
    pub character: String,
    pub rank: i32,
    pub entity: u16,
    pub slot: i8,
    pub server: i32,
    pub channel: i32,
    pub present: bool,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuildIdentity")
            .field("present", &self.present)
            .finish_non_exhaustive()
    }
}

impl Identity {
    pub fn suffix(&self, rng: &mut Lcg) -> String {
        if !self.present {
            return String::new();
        }

        let channel = packed(self.channel, rng);

        let server = packed(self.server, rng);

        let slot = packed(i32::from(self.slot), rng);

        let rank = packed(self.rank, rng);

        format!(
            "&p1={}&p2={rank}&p3={}&p4={}&p5={slot}&p6={server}&p7={channel}",
            self.account.to_ascii_uppercase(),
            self.character,
            self.entity
        )
    }
}

pub fn packed(value: i32, rng: &mut Lcg) -> i32 {
    let width = (rng.next_mod(10) as usize).max(6);

    let selector = rng.next_mod(width as u32) as usize + 1;

    let mut digits = [0i32; 11];

    for i in (0..width).rev() {
        digits[i] = if i == width - 1 {
            rng.next_mod(9) as i32 + 1
        } else {
            rng.next_mod(10) as i32
        };
    }

    digits[0] = selector as i32;

    digits[width - selector] = value.wrapping_add(1);

    let text: String = digits.iter().rev().map(i32::to_string).collect();

    let mut number = 0i32;

    for byte in text.bytes().take_while(u8::is_ascii_digit) {
        number = number
            .saturating_mul(10)
            .saturating_add(i32::from(byte - b'0'));
    }

    number
}

#[derive(Default)]
pub struct RetailState {
    suffix: String,
    last_name_fallback: u32,
}

impl std::fmt::Debug for RetailState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetailState").finish_non_exhaustive()
    }
}

impl RetailState {
    pub fn prepare(&mut self, endpoint: formats::net::guild_web::Endpoint, body: &mut String) {
        use formats::net::guild_web::Endpoint;

        if !(1..2048).contains(&body.len()) {
            return;
        }

        if matches!(endpoint, Endpoint::Info | Endpoint::Members) {
            if let Some(at) = body.find("&p1=") {
                let mut end = (at + 511).min(body.len());

                while !body.is_char_boundary(end) {
                    end -= 1;
                }

                self.suffix = body[at..end].to_owned();
            }
        } else if matches!(endpoint, Endpoint::Notice | Endpoint::MemberNotice)
            && !body.contains("&p1=")
            && body.len() + self.suffix.len() < 4096
        {
            body.push_str(&self.suffix);
        }
    }

    pub fn name_result(&mut self, result: i32, now: u32) -> i32 {
        if result > 1
            && (self.last_name_fallback == 0 || now.wrapping_sub(self.last_name_fallback) > 999)
        {
            self.last_name_fallback = now;

            0
        } else {
            result
        }
    }
}
