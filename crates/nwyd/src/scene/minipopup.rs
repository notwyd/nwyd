use crate::frame::{MiniPopupPhase, PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::msgbox;
use crate::scene::panels::{self, Action, Panels};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::ui::pane::ElementType;
use render::device::Gpu;
use render::sprite::{Quad, UvRule};
use render::texture::AlphaPolicy;

pub const CONTENT: PanelId = 0x0D;

pub const SCRIPT_FIELD: &[u8] = b"ui/Panel/game/FieldScene.pane";

pub const SCRIPT_POPUP: &[u8] = b"ui/Panel/game/MiniPopup.pane";

pub const TAGS: [&str; 3] = ["Memo", "Quest", "Summon"];

pub const MEMO_ICON: &str = "IconMemo";

pub const SUMMON_ICON: &str = "IconSummon";

pub const QUEST_BUTTON: &str = "PopupQuest";

pub const BLINK_MS: u32 = 1000;

pub const BLINK_TEXTURES: [&str; 2] = ["NUI/iconon.wyt", "NUI/iconoff.wyt"];

pub const SUMMON_RECT: [i32; 4] = [0x140, 0x40, 0x15F, 0x5F];

pub const MEMO_RECT: [i32; 4] = [0x1C0, 0xE0, 0x1DF, 0xFF];

#[derive(Debug)]

struct IconTexture {
    phase: MiniPopupPhase,
    path: &'static str,
    texture: formats::texture::Texture,
}

#[derive(Debug, Default)]

pub struct IconTextures {
    loaded: Vec<IconTexture>,
}

impl IconTextures {
    pub fn len(&self) -> usize {
        self.loaded.len()
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    pub fn paths(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.loaded.iter().map(|loaded| loaded.path)
    }
}

pub fn load_textures(game: &GameDir) -> Result<IconTextures> {
    let mut loaded = Vec::new();

    for (phase, path) in [
        (MiniPopupPhase::Lit, BLINK_TEXTURES[0]),
        (MiniPopupPhase::Dark, BLINK_TEXTURES[1]),
    ] {
        let Ok(raw) = game.read(path.as_bytes()) else {
            continue;
        };

        let texture = formats::texture::decode(&raw)
            .with_context(|| format!("decoding MiniPopup icon sheet {path}"))?;

        loaded.push(IconTexture {
            phase,
            path,
            texture,
        });
    }

    Ok(IconTextures { loaded })
}

pub fn upload(textures: &IconTextures, gpu: &Gpu, renderer: &mut SceneRenderer) {
    for loaded in &textures.loaded {
        renderer.upload_once(
            gpu,
            TextureKey::MiniPopupIcon(loaded.phase),
            &loaded.texture,
            AlphaPolicy::Stored,
            loaded.path,
        );
    }
}

pub const MEMO_HELP_TAB: i32 = 5;

pub const SUMMON_PROMPT: [usize; 2] = [228, 229];

pub const SUMMON_CAPTION: usize = 237;

pub const SUMMON_BOX: (i32, u32) = (0x0D, 0x25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Notice {
    Memo,
    Quest,
    Summon,
}

impl Notice {
    pub fn tag(self) -> &'static str {
        match self {
            Notice::Memo => TAGS[0],
            Notice::Quest => TAGS[1],
            Notice::Summon => TAGS[2],
        }
    }

    pub fn describe(self, on: bool) -> String {
        let verb = match on {
            true => "raised",
            false => "cleared",
        };

        match self {
            Notice::Memo => {
                format!("NWYD: minipopup memo {verb} (help book tab {MEMO_HELP_TAB})")
            }
            Notice::Quest => format!("NWYD: minipopup quest {verb}"),
            Notice::Summon => format!("NWYD: minipopup summon {verb}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Blink {
    pub deadline_ms: u32,
    pub lit: bool,
}

impl Blink {
    pub fn texture(&self) -> &'static str {
        match self.lit {
            true => BLINK_TEXTURES[0],
            false => BLINK_TEXTURES[1],
        }
    }

    pub fn phase(&self) -> MiniPopupPhase {
        match self.lit {
            true => MiniPopupPhase::Lit,
            false => MiniPopupPhase::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Notices {
    pub memo: bool,
    pub quest: bool,
    pub summon: bool,
    pub blink: Blink,
    memo_phase: Option<MiniPopupPhase>,
    summon_phase: Option<MiniPopupPhase>,
}

impl Notices {
    pub fn any(&self) -> bool {
        self.memo || self.quest || self.summon
    }
}

pub fn set(panels: &mut Panels, which: Notice, on: bool) -> bool {
    match which {
        Notice::Memo => panels.notices.memo = on,
        Notice::Quest => panels.notices.quest = on,
        Notice::Summon => panels.notices.summon = on,
    }

    let any = panels.notices.any();

    let tag = which.tag();

    if let Some(screen) = popup_mut(panels) {
        screen.placed.ui.panel.set_visible_by_tagname(tag, on);

        screen.set_open(any);
    }

    any
}

pub fn frame(panels: &mut Panels, now_ms: u32) -> bool {
    if !panels
        .screens
        .iter()
        .any(|s| s.script == SCRIPT_FIELD && s.placed.ui.panel.visible)
        || panels.notices.blink.deadline_ms >= now_ms
    {
        return false;
    }

    let parity = (panels.notices.blink.deadline_ms / BLINK_MS) & 1;

    panels.notices.blink.lit = parity == 0;

    let visible = |name| {
        popup(panels)
            .and_then(|s| s.element(name, ElementType::IconButton))
            .is_some_and(|e| e.visible)
    };

    let (memo, summon) = (visible(MEMO_ICON), visible(SUMMON_ICON));

    let phase = Some(panels.notices.blink.phase());

    if memo {
        panels.notices.memo_phase = phase;
    }

    if summon {
        panels.notices.summon_phase = phase;
    }

    panels.notices.blink.deadline_ms = now_ms.wrapping_add(BLINK_MS);

    true
}

pub fn bind(panels: &mut Panels) {
    panels.notices = Notices::default();

    for which in [Notice::Memo, Notice::Quest, Notice::Summon] {
        set(panels, which, false);
    }
}

pub fn command(panels: &mut Panels, name: &str) -> Vec<Action> {
    if name.eq_ignore_ascii_case(MEMO_ICON) {
        set(panels, Notice::Memo, false);

        tracing::info!("{}", Notice::Memo.describe(false));

        if let Some(screen) = panels.screen_mut(panels::CONTENT_HELP) {
            screen.page = MEMO_HELP_TAB;
        }

        if panels.is_open(panels::CONTENT_HELP) {
            panels.opened(panels::CONTENT_HELP);

            return Vec::new();
        }

        return vec![Action::SetOpen(panels::CONTENT_HELP, true)];
    }

    if name.eq_ignore_ascii_case(QUEST_BUTTON) {
        set(panels, Notice::Quest, false);

        tracing::info!("{}", Notice::Quest.describe(false));

        return match panels.is_open(panels::CONTENT_QUEST) {
            true => Vec::new(),
            false => vec![Action::SetOpen(panels::CONTENT_QUEST, true)],
        };
    }

    if name.eq_ignore_ascii_case(SUMMON_ICON) {
        set(panels, Notice::Summon, false);

        tracing::info!("{}", Notice::Summon.describe(false));

        let request = summon_request(&panels.strings, &panels.summoner);

        panels.raise_box(&request, None);

        return Vec::new();
    }

    Vec::new()
}

pub fn summon_request(strings: &ui::Strings, summoner: &str) -> msgbox::Request {
    let record = |n: usize| strings.translate(&format!("ss_{n}"));

    let head = record(SUMMON_PROMPT[0]).replacen("%s", summoner, 1);

    let body = format!("{head}{}{}", ui::text::FF_BREAK, record(SUMMON_PROMPT[1]));

    msgbox::Request {
        caption: Some(SUMMON_CAPTION),
        kind: SUMMON_BOX.0,
        context: SUMMON_BOX.1,
        body: msgbox::Body::Text(body),
        flags: msgbox::FLAGS,
    }
}

pub fn popup_mut(panels: &mut Panels) -> Option<&mut panels::Screen> {
    let index = panels
        .screens
        .iter()
        .position(|s| s.script == SCRIPT_POPUP)?;

    panels.screens.get_mut(index)
}

pub fn popup(panels: &Panels) -> Option<&panels::Screen> {
    panels.screens.iter().find(|s| s.script == SCRIPT_POPUP)
}

pub fn source_uv(rect: [i32; 4]) -> [i32; 4] {
    [
        rect[0],
        rect[1],
        rect[2].saturating_add(1),
        rect[3].saturating_add(1),
    ]
}

fn icon_quad(
    local: [i32; 4],
    origin: (i32, i32),
    scale: f32,
    source: [i32; 4],
    phase: MiniPopupPhase,
) -> PlannedQuad {
    let uv = source_uv(source);

    let mut quad = Quad {
        normalized_corners: None,
        clip: None,
        dst_x: (origin.0 + local[0]) as f32,
        dst_y: (origin.1 + local[1]) as f32,
        dst_w: (local[2] - local[0]) as f32,
        dst_h: (local[3] - local[1]) as f32,
        src_x: uv[0] as f32,
        src_y: uv[1] as f32,
        src_w: (uv[2] - uv[0]) as f32,
        src_h: (uv[3] - uv[1]) as f32,
        color: 0xFFFF_FFFF,
        rotation: 0.0,
    };

    crate::ui::scale_quad(&mut quad, scale);

    PlannedQuad {
        mode: super::hud::NUI_MODE,
        texture: TextureKey::MiniPopupIcon(phase),
        quad,
        uv: UvRule::TexelEdge,
    }
}

pub fn frame_draws(panels: &Panels) -> Vec<PlannedQuad> {
    let Some(screen) = popup(panels) else {
        return Vec::new();
    };

    if !screen.open || !screen.placed.ui.panel.visible {
        return Vec::new();
    }

    let origin = screen.placed.origin;

    let scale = screen.placed.ui.scale;

    screen
        .placed
        .ui
        .panel
        .elements
        .iter()
        .filter_map(|element| {
            if !element.visible || element.element_type != ElementType::IconButton {
                return None;
            }

            let (source, phase) = if element.name.eq_ignore_ascii_case(SUMMON_ICON) {
                (SUMMON_RECT, panels.notices.summon_phase?)
            } else if element.name.eq_ignore_ascii_case(MEMO_ICON) {
                (MEMO_RECT, panels.notices.memo_phase?)
            } else {
                return None;
            };

            Some(icon_quad(element.rect(), origin, scale, source, phase))
        })
        .collect()
}

pub fn elements_present(panels: &Panels) -> bool {
    let Some(screen) = popup(panels) else {
        return false;
    };

    screen.element(MEMO_ICON, ElementType::IconButton).is_some()
        && screen
            .element(SUMMON_ICON, ElementType::IconButton)
            .is_some()
        && screen.element(QUEST_BUTTON, ElementType::Button).is_some()
}
