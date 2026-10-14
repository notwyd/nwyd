use super::{minipopup, panels::Panels, screens::ColorLine};
use crate::clock::Calendar;

fn header(label: &str, time: &Calendar) -> String {
    format!(
        "{label} [{:02}:{:02}:{:02}]",
        time.hour, time.minute, time.second
    )
}

fn deliver(panels: &mut Panels, rows: impl IntoIterator<Item = ColorLine>) {
    if panels.screen(super::panels::CONTENT_HELP).is_none() {
        return;
    }

    let mut any = false;

    for row in rows {
        panels.append_colored_memo(row.text, row.color);

        any = true;
    }

    if any {
        minipopup::set(panels, minipopup::Notice::Memo, true);
    }
}

fn sms_rows(body: &str, time: &Calendar) -> [ColorLine; 3] {
    [
        ColorLine {
            text: String::new(),
            color: 0xFFFF_FFFF,
        },
        ColorLine {
            text: header("SMS", time),
            color: 0xFFBB_FFFF,
        },
        ColorLine {
            text: body.into(),
            color: 0xFFBB_FFCC,
        },
    ]
}

pub fn numbered(panels: &mut Panels, body: &str, time: &Calendar) {
    deliver(panels, sms_rows(body, time));
}

pub fn channel(panels: &mut Panels, sender: &str, body: &str, time: &Calendar) {
    let label = super::chat::format_tokens(&panels.strings.translate("ss_226"), &[sender]);

    deliver(
        panels,
        [
            ColorLine {
                text: String::new(),
                color: 0xFFFF_FFFF,
            },
            ColorLine {
                text: header(&label, time),
                color: 0xFFFF_FFFF,
            },
            ColorLine {
                text: body.into(),
                color: 0xFFFF_FFCC,
            },
        ],
    );
}

#[derive(Debug, Clone)]
pub struct Inbox {
    rows: [ColorLine; 100],
}

impl Default for Inbox {
    fn default() -> Self {
        Self {
            rows: std::array::from_fn(|_| ColorLine::default()),
        }
    }
}

impl Inbox {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn receive(&mut self, body: &str, time: &Calendar) -> bool {
        let Some(at) =
            (0..98).find(|&i| self.rows[i].text.is_empty() && self.rows[i + 1].text.is_empty())
        else {
            return false;
        };

        let mut rows = sms_rows(body, time);

        rows[0].text = " ".into();

        self.rows[at..at + 3].clone_from_slice(&rows);

        true
    }

    pub fn enter(&self, panels: &mut Panels) {
        let end = (0..99)
            .find(|&i| self.rows[i].text.is_empty() && self.rows[i + 1].text.is_empty())
            .unwrap_or(99);

        deliver(panels, self.rows[..end].iter().cloned());
    }
}
