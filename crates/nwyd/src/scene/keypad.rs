use crate::scene::rng::Lcg;
use crate::ui::PanelUi;
use formats::ui::pane::ElementType;
use ui::Strings;

pub const DIGITS: usize = 10;

pub const POOL: [u8; DIGITS] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];

pub const BUTTON: &str = "Btn%d";

pub const TITLE: &str = "Title";

pub const OK: &str = "Ok";

pub const CANCEL: &str = "Can";

pub const CHANGE: &str = "Change";

pub const TITLE_UISTRING: usize = 236;

pub const NEW_UISTRING: usize = 238;

pub const CONFIRM_UISTRING: usize = 239;

pub const PIN_MAX: usize = 6;

pub const MASK: &str = "**********";

pub const PRESS_MS: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Pad {
    pub labels: [u8; DIGITS],
    pub typed: Vec<u8>,
    pub saved: Vec<u8>,
    pub next_press_ms: u32,
}

impl Default for Pad {
    fn default() -> Pad {
        Pad {
            labels: POOL,
            typed: Vec::new(),
            saved: Vec::new(),
            next_press_ms: 0,
        }
    }
}

impl Pad {
    pub fn shuffle(&mut self, rng: &mut Lcg) {
        let mut pool = POOL.to_vec();

        for slot in 0..DIGITS {
            let remaining = DIGITS - slot;

            let pick = rng.next_mod(remaining as u32) as usize;

            self.labels[slot] = pool[pick];

            pool.remove(pick);
        }

        self.next_press_ms = 0;

        self.typed.clear();
    }

    pub fn debounce(&mut self, now_ms: u32) -> bool {
        if now_ms < self.next_press_ms {
            return false;
        }

        self.next_press_ms = now_ms.wrapping_add(PRESS_MS);

        true
    }

    pub fn press(&mut self, button: usize, ui: &mut PanelUi) -> bool {
        let Some(digit) = self.labels.get(button).copied() else {
            return false;
        };

        if self.typed.len() >= PIN_MAX {
            return false;
        }

        self.typed.push(digit);

        let stars = self.stars();

        self.set_title(ui, &stars);

        true
    }

    pub fn stars(&self) -> String {
        MASK[..self.typed.len().min(MASK.len())].to_string()
    }

    pub fn label(&self, ui: &mut PanelUi) {
        let PanelUi {
            panel, resource, ..
        } = ui;

        for (index, digit) in self.labels.iter().enumerate() {
            let name = BUTTON.replace("%d", &index.to_string());

            panel
                .element_or_create(&name, ElementType::Button, resource)
                .set_display(&digit.to_string());
        }
    }

    pub fn set_title(&self, ui: &mut PanelUi, text: &str) {
        let PanelUi {
            panel, resource, ..
        } = ui;

        panel
            .element_or_create(TITLE, ElementType::Static, resource)
            .set_display(text);
    }

    pub fn set_title_string(&self, ui: &mut PanelUi, strings: &Strings, uistring: usize) {
        let text = strings.translate(&format!("uu_{uistring}"));

        self.set_title(ui, &text);
    }

    pub fn title(&self, ui: &mut PanelUi, strings: &Strings) {
        self.set_title_string(ui, strings, TITLE_UISTRING);
    }

    pub fn pin(&self) -> Vec<u8> {
        self.typed.iter().map(|d| b'0' + d).collect()
    }

    pub fn saved_pin(&self) -> Vec<u8> {
        self.saved.iter().map(|d| b'0' + d).collect()
    }

    pub fn save(&mut self) {
        self.saved = std::mem::take(&mut self.typed);
    }

    pub fn matches(&self) -> bool {
        self.saved == self.typed
    }

    pub fn clear(&mut self) {
        self.typed.clear();

        self.saved.clear();
    }
}
