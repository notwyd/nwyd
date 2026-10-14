use super::field::FieldScene;
use formats::net::kickout::{self, Kickout};

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct MessagePanel {
    pub visible: bool,
    pub shown_at_ms: u32,
    pub life_ms: u32,
    pub record: Option<usize>,
    pub text: Option<String>,
}

impl MessagePanel {
    pub fn new() -> MessagePanel {
        MessagePanel::default()
    }

    pub fn post(&mut self, record: usize, life_ms: u32, now_ms: u32) -> bool {
        self.record = Some(record);

        self.text = None;

        self.shown_at_ms = now_ms;

        self.life_ms = life_ms;

        self.visible = true;

        true
    }

    pub fn post_formatted(
        &mut self,
        record: usize,
        text: String,
        life_ms: u32,
        now_ms: u32,
    ) -> bool {
        self.post(record, life_ms, now_ms);

        self.text = Some(text);

        true
    }

    pub fn replace_formatted(&mut self, record: usize, text: String, life_ms: u32, now_ms: u32) {
        let visible = self.visible;

        self.post_formatted(record, text, life_ms, now_ms);

        self.visible = visible;
    }

    pub fn post_text(&mut self, text: String, life_ms: u32, now_ms: u32) -> bool {
        self.post_formatted(0, text, life_ms, now_ms);

        self.record = None;

        true
    }

    pub fn tick(&mut self, now_ms: u32) -> bool {
        if !self.visible || self.life_ms == 0 {
            return false;
        }

        if now_ms.wrapping_sub(self.shown_at_ms) > self.life_ms {
            self.visible = false;

            self.record = None;

            self.text = None;

            return true;
        }

        false
    }
}

pub fn level_of(scene: &FieldScene) -> Option<i16> {
    let player = scene.player?;

    Some(scene.world.actors.actors.get(player)?.status.level as i16)
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Applied {
    Characters {
        code: i16,
    },
    Death {
        code: i16,
        level: Option<i16>,
        died: bool,
    },
    Disconnected {
        code: i16,
        banner: Option<Banner>,
        leaves: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Banner {
    pub record: usize,
    pub life_ms: u32,
    pub sound: Option<i64>,
}

impl Applied {
    pub fn scene(&self) -> Option<u32> {
        match self {
            Applied::Characters { .. } => Some(kickout::SCENE_SELECT_CHAR),
            Applied::Death { .. } => None,
            Applied::Disconnected { leaves: true, .. } => Some(kickout::SCENE_SELECT_SERVER),
            Applied::Disconnected { leaves: false, .. } => None,
        }
    }
}

pub fn apply(
    scene: &mut FieldScene,
    kickout: &Kickout,
    launcher_login: bool,
    now_ms: u32,
) -> Applied {
    match *kickout {
        Kickout::Characters { code } => Applied::Characters { code },
        Kickout::Death { code } => {
            let level = level_of(scene);

            let died = match (level, scene.player) {
                (Some(level), Some(player)) if level < kickout::DEATH_LEVEL_CAP => {
                    super::kill::die(scene, player, now_ms)
                }
                _ => false,
            };

            Applied::Death { code, level, died }
        }
        Kickout::Disconnected { code } => {
            let banner = match scene.message_panel.visible {
                true => None,
                false => {
                    let sound = scene.message_panel.post(
                        kickout::MESSAGE_STRDEF,
                        kickout::MESSAGE_LIFE_MS,
                        now_ms,
                    );

                    if sound {
                        scene.world.play_sound(kickout::MESSAGE_SOUND);
                    }

                    Some(Banner {
                        record: kickout::MESSAGE_STRDEF,
                        life_ms: kickout::MESSAGE_LIFE_MS,
                        sound: sound.then_some(kickout::MESSAGE_SOUND),
                    })
                }
            };

            Applied::Disconnected {
                code,
                banner,
                leaves: !launcher_login,
            }
        }
    }
}

pub fn describe(applied: &Applied) -> String {
    match applied {
        Applied::Characters { code } => {
            format!("NWYD: the server sent code {code} — leaving the world for the character list")
        }
        Applied::Death {
            code,
            level,
            died: true,
        } => {
            format!(
                "NWYD: the server sent code {code} — the character dies where it stands (level {})",
                level.map_or("?".to_string(), |l| l.to_string()),
            )
        }
        Applied::Death {
            code,
            level,
            died: false,
        } => {
            format!(
                "NWYD: the server sent code {code} — nothing died ({})",
                match level {
                    Some(level) =>
                        format!("level {level} is not under {}", kickout::DEATH_LEVEL_CAP),
                    None => "this scene has no local player".to_string(),
                },
            )
        }
        Applied::Disconnected {
            code,
            banner,
            leaves,
        } => {
            format!(
                "NWYD: the server sent code {code} — {}{}",
                match banner {
                    Some(banner) => format!("strdef {} for {} ms", banner.record, banner.life_ms),
                    None => "the message panel was already up, so nothing was posted".to_string(),
                },
                match leaves {
                    true => ", then back to the server list",
                    false => ", and the launcher login keeps the client where it is",
                },
            )
        }
    }
}
