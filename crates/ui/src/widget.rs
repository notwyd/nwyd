use crate::element::Element;
use crate::material::Material;
use formats::ui::pane::{ElementType, Property};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Widget {
    #[default]
    None,
    Button(Box<ButtonState>),
    ComboBox(Box<crate::combo_box::ComboBox>),
    Image(Box<ImageState>),
    Gauge(Box<GaugeState>),
    Slider(Box<crate::slider::Slider>),
    Balloon(Box<crate::balloon::WidgetState>),
    ImageFont(Box<crate::image_font::ImageFont>),
    Edit(Box<EditState>),
    List(Box<ListState>),
}

impl Widget {
    pub fn new(element: ElementType) -> Widget {
        use ElementType::*;

        match element {
            Button | CheckBox | RadioButton | IconButton => Widget::Button(Box::default()),
            ComboBox => Widget::ComboBox(Box::default()),
            Image => Widget::Image(Box::default()),
            GaugeBar => Widget::Gauge(Box::default()),
            Slider => Widget::Slider(Box::default()),
            BalloonBox | TooltipBox => Widget::Balloon(Box::default()),
            ImageFont => Widget::ImageFont(Box::default()),
            EditBox | ImeEditBox | ImeNotePadEditBox | ImeNotePad => {
                Widget::Edit(Box::new(EditState::new(element == ImeNotePad)))
            }
            ListBox | ListView | TreeView | CustomListView => Widget::List(Box::new(ListState {
                plain: element == ListBox,
                tree: (element == TreeView).then(Box::default),
                custom: (element == CustomListView).then(Box::default),
                ..ListState::default()
            })),
            _ => Widget::None,
        }
    }

    pub fn apply(&mut self, property: &Property, element: ElementType) -> bool {
        match self {
            Widget::Button(state) => state.apply(property, element),
            Widget::ComboBox(state) => state.apply(property),
            Widget::Image(state) => state.apply(property),
            Widget::Gauge(state) => state.apply(property),
            Widget::Slider(state) => state.apply(property),
            Widget::Edit(state) => state.apply(property, element),
            Widget::List(state) => state.apply(property, element),
            Widget::None | Widget::Balloon(_) | Widget::ImageFont(_) => false,
        }
    }

    pub fn button(&self) -> Option<&ButtonState> {
        match self {
            Widget::Button(state) => Some(state),
            _ => None,
        }
    }

    pub fn button_mut(&mut self) -> Option<&mut ButtonState> {
        match self {
            Widget::Button(state) => Some(state),
            _ => None,
        }
    }

    pub fn image(&self) -> Option<&ImageState> {
        match self {
            Widget::Image(state) => Some(state),
            _ => None,
        }
    }

    pub fn combo_box(&self) -> Option<&crate::combo_box::ComboBox> {
        match self {
            Widget::ComboBox(state) => Some(state),
            _ => None,
        }
    }

    pub fn combo_box_mut(&mut self) -> Option<&mut crate::combo_box::ComboBox> {
        match self {
            Widget::ComboBox(state) => Some(state),
            _ => None,
        }
    }

    pub fn gauge(&self) -> Option<&GaugeState> {
        match self {
            Widget::Gauge(state) => Some(state),
            _ => None,
        }
    }

    pub fn gauge_mut(&mut self) -> Option<&mut GaugeState> {
        match self {
            Widget::Gauge(state) => Some(state),
            _ => None,
        }
    }

    pub fn edit(&self) -> Option<&EditState> {
        match self {
            Widget::Edit(state) => Some(state),
            _ => None,
        }
    }

    pub fn edit_mut(&mut self) -> Option<&mut EditState> {
        match self {
            Widget::Edit(state) => Some(state),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&ListState> {
        match self {
            Widget::List(state) => Some(state),
            _ => None,
        }
    }

    pub fn slider(&self) -> Option<&crate::slider::Slider> {
        match self {
            Widget::Slider(state) => Some(state),
            _ => None,
        }
    }

    pub fn slider_mut(&mut self) -> Option<&mut crate::slider::Slider> {
        match self {
            Widget::Slider(state) => Some(state),
            _ => None,
        }
    }

    pub fn balloon(&self) -> Option<&crate::balloon::WidgetState> {
        match self {
            Widget::Balloon(state) => Some(state),
            _ => None,
        }
    }

    pub fn balloon_mut(&mut self) -> Option<&mut crate::balloon::WidgetState> {
        match self {
            Widget::Balloon(state) => Some(state),
            _ => None,
        }
    }

    pub fn image_font(&self) -> Option<&crate::image_font::ImageFont> {
        match self {
            Widget::ImageFont(state) => Some(state),
            _ => None,
        }
    }

    pub fn image_font_mut(&mut self) -> Option<&mut crate::image_font::ImageFont> {
        match self {
            Widget::ImageFont(state) => Some(state),
            _ => None,
        }
    }

    pub fn list_mut(&mut self) -> Option<&mut ListState> {
        match self {
            Widget::List(state) => Some(state),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ButtonState {
    pub focus_enabled: bool,
    pub default_key: bool,
    pub cancel_key: bool,
    pub group: String,
    pub toggle: bool,
    pub icon: bool,
    pub down_text_up: bool,
    pub button_sound: i32,
    pub checked: bool,
    pub pulse_enabled: bool,
    pub pulse_frequency: f32,
    pub icon_texture: Option<String>,
    pub icon_texture_uv: Option<[i32; 4]>,
    pub icon_slot_index: i16,
    pub icon_slot_type: i32,
    pub cell_group: i32,
    pub icon_contents: Option<crate::icon::IconContents>,
    pub icon_tooltip: Option<crate::icon::TooltipBinding>,
    pub affect_detail: Option<formats::net::affect::Detail>,
    pub affect_countdown: Option<String>,
    pub icon_tooltip_enabled: bool,
    icon_update: u64,
    icon_callback_visible: bool,
    icon_template_empty: bool,
    pub collection: Option<crate::icon::CollectionIcon>,
    pub pulse_fade: Option<f32>,
    pub pulse_highlight: bool,
    pub icon_locked: bool,
}

impl Default for ButtonState {
    fn default() -> ButtonState {
        ButtonState {
            focus_enabled: true,
            default_key: false,
            cancel_key: false,
            group: String::new(),
            toggle: false,
            icon: false,
            down_text_up: false,
            button_sound: 0,
            checked: false,
            icon_texture: None,
            icon_texture_uv: Some([0, 0, 64, 64]),
            icon_slot_index: -1,
            icon_slot_type: 0,
            cell_group: cell_group::NONE,
            icon_contents: None,
            icon_tooltip: None,
            affect_detail: None,
            affect_countdown: None,
            icon_tooltip_enabled: true,
            icon_update: 0,
            icon_callback_visible: true,
            icon_template_empty: true,
            collection: None,
            pulse_fade: None,
            pulse_highlight: false,
            pulse_enabled: false,
            pulse_frequency: 0.0,
            icon_locked: false,
        }
    }
}

pub mod cell_group {

    pub const NONE: i32 = 0;

    pub const EQUIPMENT: i32 = 1;

    pub const INVENTORY: i32 = 2;

    pub const CARGO: i32 = 3;

    pub const QUICK: i32 = 5;

    pub const SHOP: i32 = 6;

    pub const MIX: i32 = 7;

    pub const SKILL: i32 = 8;

    pub const HUD_SKILL: i32 = 9;
}

impl ButtonState {
    pub fn set_pulse(&mut self, enabled: bool, period: f32) {
        self.pulse_enabled = enabled;

        self.pulse_frequency = if period == 0.0 { 0.0 } else { 1.0 / period };
    }

    pub fn set_icon_contents(&mut self, contents: Option<crate::icon::IconContents>) {
        self.set_icon_contents_for_element(contents, true);
    }

    pub(crate) fn set_icon_contents_for_element(
        &mut self,
        contents: Option<crate::icon::IconContents>,
        visible: bool,
    ) {
        self.icon_template_empty = false;

        self.icon_callback_visible = visible;

        if visible && (contents.is_some() || self.tooltip_binding().is_some_and(|b| b.kind != 0)) {
            self.icon_update = self.icon_update.wrapping_add(1);
        }

        self.icon_texture = None;

        self.icon_contents = contents;

        self.icon_tooltip = None;

        self.affect_countdown = None;

        self.affect_detail = contents
            .filter(|c| c.kind == crate::icon::AFFECT_KIND)
            .map(|c| formats::net::affect::Detail {
                kind: c.index as u8,
                level: 0,
                value: 0,
                time_8s: 0,
            });
    }

    pub fn icon_update(&self) -> u64 {
        self.icon_update
    }

    pub(crate) fn has_empty_template_inputs(&self) -> bool {
        self.icon_template_empty && self.icon_contents.is_none() && self.icon_tooltip.is_none()
    }

    pub(crate) fn copy_empty_icon_from(&mut self, source: &Self, visible: bool) {
        self.toggle = source.toggle;

        self.group = source.group.clone();

        self.button_sound = source.button_sound;

        self.down_text_up = source.down_text_up;

        self.icon_slot_type = source.icon_slot_type;

        self.set_icon_contents_for_element(None, visible);

        self.icon_template_empty = true;

        self.collection = None;

        self.pulse_fade = None;

        if visible {
            self.icon_update = self.icon_update.wrapping_add(1);
        }
    }

    pub fn set_tooltip_binding(&mut self, binding: Option<crate::icon::TooltipBinding>) {
        self.icon_template_empty = false;

        if binding.is_some() && self.icon_callback_visible {
            self.icon_update = self.icon_update.wrapping_add(1);
        }

        self.icon_tooltip = binding;
    }

    pub fn tooltip_binding(&self) -> Option<crate::icon::TooltipBinding> {
        self.icon_tooltip.or_else(|| {
            self.icon_contents.map(|c| crate::icon::TooltipBinding {
                kind: c.kind,
                index: c.index,
                item: c.item,
                help_id: 0,
            })
        })
    }

    pub fn set_icon_texture(&mut self, path: String, slot: u32) {
        self.icon_texture = Some(path);

        self.icon_texture_uv = if slot == u32::MAX {
            None
        } else {
            let x = (slot & 7).wrapping_mul(64) as i32;

            let y = (slot >> 3).wrapping_mul(64) as i32;

            Some([x, y, x.wrapping_add(64), y.wrapping_add(64)])
        };
    }

    pub(crate) fn apply(&mut self, property: &Property, element: ElementType) -> bool {
        let key = property.base_key();

        let icon_button = element == ElementType::IconButton;

        match key {
            k if k.eq_ignore_ascii_case("Group") => self.group = property.value.clone(),
            k if k.eq_ignore_ascii_case("ToggleButton") => self.toggle = property.flag(),
            k if k.eq_ignore_ascii_case("DownTextUp") => {
                self.down_text_up = property.flag();
            }
            k if k.eq_ignore_ascii_case("ButtonSound") => {
                self.button_sound = property.int();
            }
            k if icon_button && k.eq_ignore_ascii_case("IconTexture") => {
                self.set_icon_texture(property.value.clone(), 0);
            }
            k if icon_button && k.eq_ignore_ascii_case("IconTextureSlot") => {
                let slot = property.int();

                self.icon_texture = Some(format!("UI/ItemIcon{:02}.wyt", slot / 100 + 1));

                let x = (slot % 100 % 10) * 100;

                let y = (slot % 100 / 10) * 100;

                self.icon_texture_uv = Some([x, y, x + 100, y + 100]);
            }
            k if icon_button && k.eq_ignore_ascii_case("IconSlotIndex") => {
                self.icon_slot_index = property.int() as i16;
            }
            k if icon_button && k.eq_ignore_ascii_case("IconSlotType") => {
                self.icon_slot_type = property.int();
            }
            _ => return false,
        }

        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageState {
    pub external_image_file: String,
    pub use_external_uv: bool,
    pub use_rotation: bool,
    pub rotation: f32,
    pub rotation_center: [i32; 2],
    pub use_ani: bool,
    pub ani_cut: [u8; 2],
    pub animation_pages: [u8; 2],
    pub animation_delay: f32,
    pub animation_loop_count: i16,
    pub current_page: u8,
    pub loop_counter: u8,
    pub elapsed: f32,
}

impl Default for ImageState {
    fn default() -> ImageState {
        ImageState {
            external_image_file: String::new(),
            use_external_uv: false,
            use_rotation: false,
            rotation: 0.0,
            rotation_center: [0, 0],
            use_ani: false,
            ani_cut: [0, 0],
            animation_pages: [1, 1],
            animation_delay: 0.0,
            animation_loop_count: -1,
            current_page: 1,
            loop_counter: 0,
            elapsed: 0.0,
        }
    }
}

impl ImageState {
    pub fn copy_template_from(&mut self, source: &Self) {
        if !source.external_image_file.is_empty() {
            self.external_image_file = source.external_image_file.clone();
        }

        if source.use_ani {
            self.use_ani = true;

            self.ani_cut = source.ani_cut;

            self.animation_pages = source.animation_pages;

            self.animation_delay = source.animation_delay;

            self.animation_loop_count = source.animation_loop_count;

            self.elapsed = 0.0;

            self.current_page = 1;

            self.loop_counter = 0;
        }
    }

    pub fn advance(&mut self, seconds: f32) -> bool {
        if !self.use_ani {
            return false;
        }

        self.elapsed += seconds;

        if self.animation_delay <= self.elapsed {
            self.current_page = self.current_page.wrapping_add(1);

            self.elapsed = 0.0;

            if self.current_page > self.animation_pages[1] {
                self.loop_counter = self.loop_counter.wrapping_add(1);

                self.current_page = self.animation_pages[0];
            }
        }

        self.animation_loop_count != -1 && i16::from(self.loop_counter) >= self.animation_loop_count
    }

    fn apply(&mut self, property: &Property) -> bool {
        let key = property.base_key();

        match key {
            k if k.eq_ignore_ascii_case("ExternalImageFile") => {
                self.external_image_file = property.value.clone();
            }
            k if k.eq_ignore_ascii_case("UseRotation") => {
                self.use_rotation = property.flag();
            }
            k if k.eq_ignore_ascii_case("Rotation") => self.rotation = property.float(),
            k if k.eq_ignore_ascii_case("RotationCenter") => {
                property.scan(&mut self.rotation_center);
            }
            k if k.eq_ignore_ascii_case("UseAni") => self.use_ani = property.flag(),
            k if k.eq_ignore_ascii_case("AniCutXPage") => {
                self.ani_cut[0] = property.int() as u8;
            }
            k if k.eq_ignore_ascii_case("AniCutYPage") => {
                self.ani_cut[1] = property.int() as u8;
            }
            k if k.eq_ignore_ascii_case("AnimationMinPage") => {
                self.animation_pages[0] = property.int() as u8;
            }
            k if k.eq_ignore_ascii_case("AnimationMaxPage") => {
                self.animation_pages[1] = property.int() as u8;
            }
            k if k.eq_ignore_ascii_case("AnimationDelay") => {
                if let Some(value) = formats::ui::script::scan_float(&property.value) {
                    self.animation_delay = value;
                }
            }
            k if k.eq_ignore_ascii_case("AnimationLoopCnt") => {
                self.animation_loop_count = property.int() as i16;
            }
            _ => return false,
        }

        true
    }

    pub fn cell(&self, uv: [i32; 4]) -> Option<[i32; 4]> {
        let (cut_x, cut_y) = (i32::from(self.ani_cut[0]), i32::from(self.ani_cut[1]));

        if cut_x == 0 || cut_y == 0 {
            return None;
        }

        let frame = i32::from(self.current_page);

        let (col, row) = if cut_x >= frame {
            (frame, 1)
        } else {
            let (mut col, mut row) = (frame % cut_x, frame / cut_x + 1);

            if col == 0 {
                row -= 1;

                col = cut_x;
            }

            (col, row)
        };

        let cell_w = (uv[2] - uv[0]) / cut_x;

        let cell_h = (uv[3] - uv[1]) / cut_y;

        Some([
            (col - 1) * cell_w + uv[0],
            (row - 1) * cell_h + uv[1],
            col * cell_w + uv[0] - 1,
            row * cell_h + uv[1] - 1,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarDirection {
    #[default]
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
}

impl BarDirection {
    pub fn parse(value: &str) -> Option<BarDirection> {
        [
            ("right", BarDirection::Right),
            ("left", BarDirection::Left),
            ("up", BarDirection::Up),
            ("down", BarDirection::Down),
        ]
        .into_iter()
        .find(|(name, _)| value.eq_ignore_ascii_case(name))
        .map(|(_, d)| d)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GaugeState {
    pub range: [i32; 2],
    pub position: i32,
    pub direction: BarDirection,
    pub out_space: [i32; 4],
    pub fill_draw_method: i32,
    pub no_frame: bool,
    pub flash_fadeout_rate: f32,
    pub text_type: i32,
    pub text_postfix: String,
}

impl Default for GaugeState {
    fn default() -> GaugeState {
        GaugeState {
            range: [0, 100],
            position: 50,
            direction: BarDirection::Right,
            out_space: [0; 4],
            fill_draw_method: 0,
            no_frame: false,
            flash_fadeout_rate: 1.0,
            text_type: 0,
            text_postfix: String::new(),
        }
    }
}

impl GaugeState {
    fn apply(&mut self, property: &Property) -> bool {
        let key = property.base_key();

        match key {
            k if k.eq_ignore_ascii_case("BarRange") => {
                property.scan(&mut self.range);
            }
            k if k.eq_ignore_ascii_case("BarPos") => self.position = property.int(),
            k if k.eq_ignore_ascii_case("BarDirection") => {
                if let Some(d) = BarDirection::parse(&property.value) {
                    self.direction = d;
                }
            }
            k if k.eq_ignore_ascii_case("BarOutSpace") => {
                property.scan(&mut self.out_space);
            }
            k if k.eq_ignore_ascii_case("FillDrawMethod") => {
                self.fill_draw_method = property.int();
            }
            k if k.eq_ignore_ascii_case("noFrame") => self.no_frame = property.flag(),
            k if k.eq_ignore_ascii_case("BarFlashFadeoutRate") => {
                self.flash_fadeout_rate = property.float();
            }
            k if k.eq_ignore_ascii_case("text_type") => {
                self.text_type = match &property.value {
                    v if v.eq_ignore_ascii_case("rate_fraction") => 1,
                    v if v.eq_ignore_ascii_case("rate_percent") => 2,
                    _ => self.text_type,
                };
            }
            k if k.eq_ignore_ascii_case("text_postfix") => {
                self.text_postfix = property.value.clone();
            }
            k if k.eq_ignore_ascii_case("BarFlash") || k.eq_ignore_ascii_case("BarSpark") => {}
            _ => return false,
        }

        true
    }

    pub fn inner(&self, rect: [i32; 4]) -> [i32; 4] {
        [
            rect[0] + 2 * self.out_space[0],
            rect[1] + 2 * self.out_space[1],
            rect[2] - 2 * self.out_space[2],
            rect[3] - 2 * self.out_space[3],
        ]
    }

    pub fn fill(&self, rect: [i32; 4]) -> [i32; 4] {
        let inner = self.inner(rect);

        let [min, max] = self.range;

        let range = if max - min == 0 { 1 } else { max - min };

        let p = self.position.min(max);

        let extent = |span: i32| (f64::from(span) * f64::from(p) / f64::from(range)) as i32;

        let mut fill = inner;

        match self.direction {
            BarDirection::Right => fill[2] = inner[0] + extent(inner[2] - inner[0]),
            BarDirection::Left => fill[0] = inner[2] - extent(inner[2] - inner[0]),
            BarDirection::Up => fill[1] = inner[3] - extent(inner[3] - inner[1]),
            BarDirection::Down => fill[3] = inner[1] + extent(inner[3] - inner[1]),
        }

        fill
    }

    pub fn set_pos(&mut self, position: i32) {
        self.position = position;
    }

    pub fn set_range(&mut self, min: i32, max: i32) {
        self.range = [min, max];
    }

    pub fn set_range_and_pos(&mut self, min: i32, max: i32, position: i32) {
        self.range = [min, max.max(min)];

        self.position = position.max(min).min(self.range[1]);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditState {
    pub note_pad: bool,
    pub pad: Option<Box<crate::notepad::State>>,
    pub border_width: i32,
    pub spacing: i32,
    pub item_spacing: i32,
    pub draw_box: bool,
    pub password: bool,
    pub num_attrib: bool,
    pub range: i32,
    pub multiline: bool,
    pub indicator: bool,
    pub richtext: bool,
    pub left_scrollbar: bool,
    pub sel_bk_color: u32,
    pub sel_color: u32,
    pub caret_color: u32,
    pub text: String,
    pub preedit: Option<crate::text_entry::Preedit>,
    pub ime_status: ImeStatus,
    pub caret: i32,
    pub anchor: i32,
    pub selection_held: bool,
    pub scroll: i32,
    pub insert: bool,
    pub caret_visible: bool,
    pub caret_period_ms: u32,
    pub caret_clock: Option<(u32, bool)>,
    pub caret_suppressed: bool,
    pub draw_quads: bool,
    pub text_color: u32,
}

pub const MASK: char = '\u{25cf}';

pub const MASK_LIMIT: i32 = 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImeStatus {
    #[default]
    Disabled,
    Ready,
    Composing,
}

impl EditState {
    fn new(note_pad: bool) -> EditState {
        EditState {
            note_pad,
            pad: note_pad.then(|| Box::new(crate::notepad::State::default())),
            border_width: if note_pad { 2 } else { 1 },
            spacing: 4,
            item_spacing: 4,
            draw_box: !note_pad,
            password: false,
            num_attrib: false,
            range: 0,
            multiline: false,
            indicator: false,
            richtext: false,
            left_scrollbar: false,
            sel_bk_color: 0xFFFF_FF00,
            sel_color: 0xFF00_0000,
            caret_color: 0xFF64_FFFF,
            text: String::new(),
            preedit: None,
            ime_status: ImeStatus::Disabled,
            caret: 0,
            anchor: 0,
            selection_held: false,
            scroll: 0,
            insert: true,
            caret_visible: true,
            caret_period_ms: 500,
            caret_clock: None,
            caret_suppressed: false,
            draw_quads: true,
            text_color: 0xFFFF_FFFF,
        }
    }

    pub fn caret_phase_at(&self, now: u32, focused: bool) -> bool {
        let Some((last, was_focused)) = self.caret_clock else {
            return self.caret_visible;
        };

        if focused && !was_focused {
            return true;
        }

        if self.caret_period_ms != u32::MAX && now.wrapping_sub(last) >= self.caret_period_ms {
            !self.caret_visible
        } else {
            self.caret_visible
        }
    }

    pub fn advance_caret(&mut self, now: u32, focused: bool) {
        let phase = self.caret_phase_at(now, focused);

        let reset = self.caret_clock.is_none_or(|(last, was_focused)| {
            focused && !was_focused
                || (self.caret_period_ms != u32::MAX
                    && now.wrapping_sub(last) >= self.caret_period_ms)
        });

        let last = if reset {
            now
        } else {
            self.caret_clock.unwrap().0
        };

        self.caret_visible = phase;

        self.caret_clock = Some((last, focused));
    }

    pub fn lose_caret_focus(&mut self) {
        if let Some((_, focused)) = &mut self.caret_clock {
            *focused = false;
        }
    }

    pub fn reset_caret(&mut self) {
        self.caret_visible = true;

        self.caret_clock = None;
    }

    pub fn masked(&self) -> std::borrow::Cow<'_, str> {
        if !self.password {
            return std::borrow::Cow::Borrowed(&self.text);
        }

        let count = if self.range != 0 {
            self.range
        } else {
            MASK_LIMIT
        };

        let typed = self.text.chars().count() as i32;

        let n = count.min(typed).clamp(0, MASK_LIMIT) as usize;

        std::borrow::Cow::Owned(std::iter::repeat_n(MASK, n).collect())
    }

    pub fn rects(&self, element_type: ElementType, x: i32, y: i32, w: i32, h: i32) -> EditRects {
        let (bw, sp) = (self.border_width, self.spacing);

        let ime = matches!(
            element_type,
            ElementType::ImeEditBox | ElementType::ImeNotePadEditBox
        );

        let reserved = if ime {
            2 * bw
                + if self.indicator {
                    IME_INDICATOR_WIDTH
                } else {
                    0
                }
        } else {
            0
        };

        let inner_w = w - reserved;

        EditRects {
            frame: [x, y, x + inner_w, y + h],
            text: [
                x + bw + sp,
                y + bw + sp,
                x + inner_w - bw - sp,
                y + h - bw - sp,
            ],
            ime: ime.then_some([x + inner_w, y, x + w, y + h]),
        }
    }

    fn apply(&mut self, property: &Property, element: ElementType) -> bool {
        let key = property.base_key();

        let note_pad = element == ElementType::ImeNotePad;

        let color = || {
            let mut channels = [0; 4];

            property.scan(&mut channels);

            u32::from_be_bytes(channels.map(|n| n as u8))
        };

        match key {
            k if k.eq_ignore_ascii_case("BorderWidth") => {
                self.border_width = property.int();
            }
            k if k.eq_ignore_ascii_case("Spacing") => self.spacing = property.int(),
            k if note_pad && k.eq_ignore_ascii_case("ItemSpacing") => {
                self.item_spacing = property.int();
            }
            k if !note_pad && k.eq_ignore_ascii_case("DrawBox") => {
                self.draw_box = property.flag();
            }
            k if !note_pad && k.eq_ignore_ascii_case("Password") => {
                self.password = property.flag();
            }
            k if !note_pad && k.eq_ignore_ascii_case("NumAttrib") => {
                self.num_attrib = property.flag();
            }
            k if !note_pad && k.eq_ignore_ascii_case("Range") => {
                self.range = property.int();
            }
            k if !note_pad && k.eq_ignore_ascii_case("MultiLine") => {
                self.multiline = property.flag();
            }
            k if !note_pad && k.eq_ignore_ascii_case("Indicator") => {
                self.indicator = property.flag();
            }
            k if !note_pad && k.eq_ignore_ascii_case("SelBkColor") => {
                self.sel_bk_color = color();
            }
            k if !note_pad && k.eq_ignore_ascii_case("SelColor") => {
                self.sel_color = color();
            }
            k if !note_pad && k.eq_ignore_ascii_case("CaretColor") => {
                self.caret_color = color();
            }
            k if note_pad && k.eq_ignore_ascii_case("RichText") => {
                self.richtext = property.flag();
            }
            k if note_pad && k.eq_ignore_ascii_case("LeftScrollbar") => {
                self.left_scrollbar = property.flag();
            }
            k if note_pad && k.eq_ignore_ascii_case("TestString") => {
                if let Some(pad) = &mut self.pad {
                    pad.test_string = property.value.clone();
                }
            }
            k if note_pad && k.eq_ignore_ascii_case("Wrap") => {}
            _ => return false,
        }

        true
    }
}

pub const IME_INDICATOR_WIDTH: i32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditRects {
    pub frame: [i32; 4],
    pub text: [i32; 4],
    pub ime: Option<[i32; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    pub selected: bool,
    pub height: Option<i32>,
    pub payload: i64,
    pub text: String,
    pub color: u32,
    pub background: u32,
    pub shadow: formats::ui::pane::ShadowType,
    pub shadow_color: u32,
    pub rich: Option<crate::rich::Line>,
    pub children: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListState {
    pub tree: Option<Box<crate::tree_view::Tree>>,
    pub custom: Option<Box<crate::custom_list::CustomList>>,
    pub plain: bool,
    pub text_row_height: Option<i32>,
    pub border_width: i32,
    pub spacing: i32,
    pub item_spacing: i32,
    pub scrollbar_width: i32,
    pub left_scrollbar: bool,
    pub richtext: bool,
    pub draw_select_box_last: bool,
    pub draw_highlight: bool,
    pub ignore_selection: bool,
    pub render_method: i32,
    pub item_size: [i32; 2],
    pub item_panel: String,
    pub pending_item_panels: Vec<String>,
    pub template: Vec<Element>,
    pub items: Vec<ListRow>,
    pub empty_range_reset: bool,
    pub selection_flags: u32,
    pub selection_anchor: i32,
    pub selection_held: bool,
    pub selected: i32,
    pub scroll: i32,
    pub scroll_page: i32,
    pub show_scrollbar: bool,
    pub auto_hide_scrollbar: bool,
    pub scrollbar: ScrollBar,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScrollBar {
    pub layers: Vec<Material>,
    pub arrows: bool,
    pub press: i32,
    pub hover: [bool; 2],
    pub(crate) pointer: [i32; 2],
    pub(crate) drag_offset: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollBarRects {
    pub up: [i32; 4],
    pub down: [i32; 4],
    pub track: [i32; 4],
    pub thumb: Option<[i32; 4]>,
}

impl ScrollBarRects {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        [self.up, self.track, self.down]
            .into_iter()
            .any(|rect| crate::input::in_rect(rect, x, y))
    }

    pub fn drag_position(&self, range: [i32; 2], page: i32, grab: i32, y: i32) -> Option<i32> {
        let thumb = self.thumb?;

        let travel = i128::from(self.track[3])
            - i128::from(self.track[1])
            - (i128::from(thumb[3]) - i128::from(thumb[1]));

        let extent = i128::from(range[1]) - i128::from(range[0]) - i128::from(page);

        if travel <= 0 || extent <= 0 || page < 0 {
            return None;
        }

        let top = (i128::from(y) - i128::from(grab) - i128::from(self.track[1])).clamp(0, travel);

        let position = (top + travel / (extent * 2)) * extent / travel + i128::from(range[0]);

        Some(position.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32)
    }

    pub fn page_delta(&self, page: i32, y: i32) -> Option<i32> {
        let thumb = self.thumb?;

        if y < self.track[1] || y >= self.track[3] {
            return None;
        }

        if y < thumb[1] {
            Some(1i32.saturating_sub(page))
        } else if y >= thumb[3] {
            Some(page.saturating_sub(1))
        } else {
            None
        }
    }
}

impl ScrollBar {
    pub fn rects(
        &self,
        rect: [i32; 4],
        range: [i32; 2],
        page: i32,
        position: i32,
    ) -> ScrollBarRects {
        let [l, t, r, b] = rect;

        let bounded = |n: i128| n.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32;

        let side = i128::from(r) - i128::from(l);

        let up = [
            l,
            t,
            r,
            if self.arrows {
                bounded(i128::from(t) + side)
            } else {
                t
            },
        ];

        let down = [
            l,
            if self.arrows {
                bounded(i128::from(b) - side)
            } else {
                b
            },
            r,
            b,
        ];

        let track = [up[0], up[3], down[2], down[1]];

        let span = i128::from(range[1]) - i128::from(range[0]);

        let page = i128::from(page);

        let thumb = if page >= 0 && page < span {
            let height = i128::from(track[3]) - i128::from(track[1]);

            let size = (height * page / span).max(8);

            let y = i128::from(track[1])
                + (i128::from(position) - i128::from(range[0])) * (height - size) / (span - page);

            Some([l, bounded(y), r, bounded(y + size)])
        } else {
            None
        };

        ScrollBarRects {
            up,
            down,
            track,
            thumb,
        }
    }
}

impl Default for ListState {
    fn default() -> ListState {
        ListState {
            tree: None,
            custom: None,
            border_width: 2,
            spacing: 4,
            item_spacing: 4,
            scrollbar_width: 16,
            left_scrollbar: false,
            richtext: false,
            draw_select_box_last: false,
            draw_highlight: true,
            ignore_selection: false,
            plain: false,
            text_row_height: None,
            render_method: 0,
            item_size: [50, 50],
            item_panel: String::new(),
            pending_item_panels: Vec::new(),
            template: Vec::new(),
            items: Vec::new(),
            empty_range_reset: true,
            selection_flags: 0,
            selection_anchor: 0,
            selection_held: false,
            selected: -1,
            scroll: 0,
            scroll_page: 1,
            show_scrollbar: true,
            auto_hide_scrollbar: false,
            scrollbar: ScrollBar::default(),
        }
    }
}

impl ListState {
    pub fn clear_items(&mut self) {
        self.empty_range_reset = true;

        if self.tree.is_some() {
            self.clear_tree();

            return;
        }

        self.items.clear();

        self.selected = -1;

        self.selection_anchor = 0;

        if let Some(custom) = &mut self.custom {
            **custom = Default::default();

            self.scroll = 0;
        } else {
            self.scroll = self
                .scroll
                .clamp(0, 1i32.saturating_sub(self.scroll_page).max(0));
        }
    }

    pub fn content_rect(&self, element_rect: [i32; 4]) -> [i32; 4] {
        let b = self.border_width;

        let [mut left, mut top, mut right, mut bottom] = [
            element_rect[0] + b,
            element_rect[1] + b,
            element_rect[2] - b,
            element_rect[3] - b,
        ];

        if self.left_scrollbar {
            left += self.scrollbar_width;
        } else {
            right -= self.scrollbar_width;
        }

        let inset = if self.plain {
            self.spacing
        } else {
            b + self.spacing
        };

        left += inset;

        top += inset;

        right -= inset;

        bottom -= inset;

        [left, top, right, bottom]
    }

    pub fn pitch(&self) -> i32 {
        if self.plain {
            return self.text_row_height.unwrap_or(0);
        }

        self.item_size[1] + self.item_spacing
    }

    pub fn rows_per_page(&self, content: [i32; 4]) -> i32 {
        let height = content[3] - content[1];

        let pitch = self.pitch();

        if self.plain {
            return if pitch > 0 { height / pitch } else { 0 };
        }

        if pitch == 0 {
            height
        } else {
            (height + self.item_spacing) / pitch
        }
    }

    pub fn columns(&self, content: [i32; 4]) -> i32 {
        if self.plain {
            return 1;
        }

        if self.item_size[0] <= 0 {
            return 1;
        }

        ((content[2] - content[0]) / self.item_size[0]).max(1)
    }

    pub fn item_rect(&self, index: usize, content: [i32; 4]) -> ([i32; 4], bool) {
        if self.plain {
            let top = (index as i32 - self.scroll) * self.pitch() + content[1];

            let bottom = top + self.pitch();

            return (
                [content[0], top, content[2], bottom],
                self.pitch() > 0 && top >= content[1] && bottom <= content[3],
            );
        }

        let columns = self.columns(content);

        let index = index as i32;

        let top = (index / columns - self.scroll) * self.pitch() + content[1];

        let mut left = self.item_size[0] * (index % columns) + content[0];

        let mut right = left + self.item_size[0];

        let bottom = top + self.item_size[1];

        if columns == 1 {
            left = left.min(content[2]);

            right = right.min(content[2]);
        }

        ([left, top, right, bottom], bottom <= content[3])
    }

    pub fn scrollbar_rect(&self, element_rect: [i32; 4]) -> [i32; 4] {
        let b = self.border_width;

        let left = if self.left_scrollbar {
            element_rect[0] + b
        } else {
            element_rect[2] - b - self.scrollbar_width
        };

        [
            left,
            element_rect[1] + b,
            left + self.scrollbar_width,
            element_rect[3] - b,
        ]
    }

    pub fn scroll_range(&self, content: [i32; 4]) -> (i32, i32) {
        if self.items.is_empty() && self.empty_range_reset {
            return (0, 1);
        }

        let columns = self.columns(content);

        let items = self.items.len() as i32;

        (0, items / columns + i32::from(items % columns != 0))
    }

    pub fn wheel_scroll(&mut self, rect: [i32; 4], delta: i32) {
        let notches = delta / 120;

        let (max, step) = if let Some(tree) = &self.tree {
            let view = tree.layout(self, rect);

            (view.range[1].saturating_sub(view.page as i32).max(0), 1)
        } else if self.custom.is_some() {
            let view = crate::custom_list::layout(self, rect);

            let bar = self.scrollbar_rect(rect);

            (view.max_scroll, ((bar[3] - bar[1]) / 20).max(1))
        } else {
            let content = self.content_rect(rect);

            let (_, end) = self.scroll_range(content);

            (end.saturating_sub(self.rows_per_page(content)).max(0), 1)
        };

        self.scroll = self
            .scroll
            .saturating_sub(step.saturating_mul(notches))
            .clamp(0, max);
    }

    pub fn clamped_scroll(&self, content: [i32; 4]) -> i32 {
        let (min, max) = self.scroll_range(content);

        let page = self.rows_per_page(content);

        if self.scroll < min || page >= max - min {
            return min;
        }

        self.scroll.min(max - page)
    }

    pub fn reveal_selected(&mut self, rect: [i32; 4]) {
        if self.custom.is_some() {
            return;
        }

        if self.selected < 0 && !self.plain {
            return;
        }

        let content = self.content_rect(rect);

        if let Some(tree) = &self.tree {
            let layout = tree.layout(self, rect);

            let end = layout.visible.len().min(i32::MAX as usize) as i32;

            let page = layout.page.min(i32::MAX as usize) as i32;

            let row = (self.selected.max(0) / self.columns(content)).min(end - 1);

            if row < self.scroll {
                self.scroll = row;
            } else if row >= self.scroll.saturating_add(page) {
                self.scroll = row.saturating_sub(page).saturating_add(1);
            }

            return;
        }

        let page = self.rows_per_page(content).max(1);

        let (_, end) = self.scroll_range(content);

        if end <= 0 {
            self.scroll = 0;

            return;
        }

        let row = (self.selected.max(0) / self.columns(content)).min(end - 1);

        let first = self.clamped_scroll(content).max(0);

        self.scroll = if row < first {
            row
        } else if row >= first.saturating_add(page) {
            row.saturating_sub(page - 1)
        } else {
            first
        };
    }

    pub fn row_selected(&self, index: usize) -> bool {
        if self.custom.is_some() {
            return false;
        }

        if self.selection_flags & 1 != 0 {
            if self.selection_held
                && let (Ok(active), Ok(anchor)) = (
                    usize::try_from(self.selected),
                    usize::try_from(self.selection_anchor),
                )
                && active < self.items.len()
                && anchor < self.items.len()
                && active.min(anchor) <= index
                && index <= active.max(anchor)
            {
                return self.items[anchor].selected;
            }

            self.items.get(index).is_some_and(|row| row.selected)
        } else {
            usize::try_from(self.selected).ok() == Some(index)
                || self.tree.is_some() && self.items.get(index).is_some_and(|row| row.selected)
        }
    }

    pub fn selection_notification_index(&self) -> i32 {
        if self.selection_flags & 1 != 0 {
            self.items
                .iter()
                .position(|r| r.selected)
                .map_or(-1, |i| i as i32)
        } else {
            self.selected
        }
    }

    pub fn select(&mut self, index: i32, rect: [i32; 4]) -> bool {
        if self.items.is_empty() || self.custom.is_some() {
            return false;
        }

        let last = i32::try_from(self.items.len() - 1).unwrap_or(i32::MAX);

        let index = index.clamp(0, last);

        if self.selected == index {
            return false;
        }

        self.selected = index;

        self.selection_anchor = index;

        if self.selection_flags & 1 != 0 {
            self.items[index as usize].selected = true;
        }

        self.reveal_selected(rect);

        true
    }

    pub(crate) fn mouse_row_at(&self, rect: [i32; 4], x: i32, y: i32) -> Option<usize> {
        if self.custom.is_some() || self.tree.is_some() || self.items.is_empty() {
            return None;
        }

        let content = self.content_rect(rect);

        let hit = [
            content[0].saturating_sub(self.spacing),
            content[1].saturating_sub(self.spacing),
            content[2].saturating_add(self.spacing),
            content[3].saturating_add(self.spacing),
        ];

        if x < hit[0] || x >= hit[2] || y < hit[1] || y >= hit[3] {
            return None;
        }

        let pitch = i64::from(self.pitch());

        if pitch <= 0 {
            return None;
        }

        let columns = i64::from(self.columns(content));

        let first = i64::from(self.clamped_scroll(content));

        let row = (i64::from(y) - i64::from(content[1])) / pitch + first;

        if row < first || row >= first + i64::from(self.rows_per_page(content).max(0)) {
            return None;
        }

        let column = if self.plain {
            0
        } else {
            let width = i64::from(self.item_size[0]);

            if width <= 0 {
                return None;
            }

            ((i64::from(x) - i64::from(content[0])) / width).min(columns - 1)
        };

        if column < 0 {
            return None;
        }

        let index = usize::try_from(row.checked_mul(columns)?.checked_add(column)?).ok()?;

        (index < self.items.len()).then_some(index)
    }

    pub(crate) fn select_mouse(&mut self, index: usize, shift: bool, ctrl: bool) -> bool {
        if self.custom.is_some() || self.tree.is_some() || index >= self.items.len() {
            return false;
        }

        self.selection_held = true;

        self.selected = index as i32;

        if !shift {
            self.selection_anchor = self.selected;
        }

        if self.selection_flags & 1 == 0 {
            return true;
        }

        let anchor = self
            .selection_anchor
            .clamp(0, (self.items.len() - 1).min(i32::MAX as usize) as i32)
            as usize;

        let lo = anchor.min(index);

        let hi = anchor.max(index);

        match (shift, ctrl) {
            (false, true) => self.items[index].selected = !self.items[index].selected,
            (true, false) => {
                for (i, row) in self.items.iter_mut().enumerate() {
                    row.selected = lo <= i && i <= hi;
                }
            }
            (true, true) => {
                let selected = self.items[anchor].selected;

                for row in self.items.iter_mut().take(hi).skip(lo + 1) {
                    row.selected = selected;
                }

                self.items[index].selected = true;

                self.selected = anchor as i32;
            }
            (false, false) => {
                for row in &mut self.items {
                    row.selected = false;
                }

                self.items[index].selected = true;
            }
        }

        true
    }

    pub(crate) fn finish_mouse_selection(&mut self) -> bool {
        if self.custom.is_some() || self.tree.is_some() {
            return false;
        }

        self.selection_held = false;

        let Ok(active) = usize::try_from(self.selected) else {
            return false;
        };

        let Ok(anchor) = usize::try_from(self.selection_anchor) else {
            return false;
        };

        if active >= self.items.len() || anchor >= self.items.len() {
            return false;
        }

        let selected = self.items[anchor].selected;

        for row in self
            .items
            .iter_mut()
            .take(active.max(anchor))
            .skip(active.min(anchor) + 1)
        {
            row.selected = selected;
        }

        self.items[active].selected = selected;

        active != anchor
    }

    pub fn navigate(&mut self, vk: u32, rect: [i32; 4]) -> Option<bool> {
        self.navigate_with_shift(vk, rect, false)
    }

    pub fn navigate_with_shift(&mut self, vk: u32, rect: [i32; 4], shift: bool) -> Option<bool> {
        if self.custom.is_some() || !matches!(vk, 0x21..=0x24 | 0x26 | 0x28) {
            return None;
        }

        if self.items.is_empty() {
            return Some(false);
        }

        let last = i32::try_from(self.items.len() - 1).unwrap_or(i32::MAX);

        let page = self.tree.as_ref().map_or_else(
            || self.rows_per_page(self.content_rect(rect)),
            |tree| tree.layout(self, rect).page.min(i32::MAX as usize) as i32,
        );

        let step = page.saturating_sub(1);

        let next = match vk {
            0x21 => self.selected.saturating_sub(step),
            0x22 => self.selected.saturating_add(step),
            0x23 => last,
            0x24 => 0,
            0x26 => self.selected.saturating_sub(1),
            _ => self.selected.saturating_add(1),
        }
        .clamp(0, last);

        if next == self.selected {
            return Some(false);
        }

        self.selected = next;

        if self.selection_flags & 1 != 0 {
            let anchor = if shift {
                self.selection_anchor.clamp(0, last)
            } else {
                next
            };

            let range = anchor.min(next)..=anchor.max(next);

            for (index, row) in self.items.iter_mut().enumerate() {
                row.selected = range.contains(&(index as i32));
            }

            if !shift {
                self.selection_anchor = next;
            }
        } else {
            self.selection_anchor = next;
        }

        self.reveal_selected(rect);

        Some(true)
    }

    pub fn scrollbar_rects(&self, element_rect: [i32; 4]) -> ScrollBarRects {
        let content = self.content_rect(element_rect);

        let (min, max) = self.scroll_range(content);

        self.scrollbar.rects(
            self.scrollbar_rect(element_rect),
            [min, max],
            self.rows_per_page(content),
            self.clamped_scroll(content),
        )
    }

    pub fn add_item(&mut self, text: &str) -> usize {
        self.add_line(text, 0)
    }

    pub fn add_line(&mut self, text: &str, color: u32) -> usize {
        self.empty_range_reset = false;

        if let Some(tree) = &mut self.tree {
            tree.push_root(self.items.len());
        }

        self.items.push(ListRow {
            selected: false,
            height: self.custom.as_ref().map(|_| self.item_size[1].max(0)),
            payload: 0,
            text: text.to_string(),
            color,
            background: 0,
            shadow: formats::ui::pane::ShadowType::BottomRight,
            shadow_color: 0xFF000000,
            rich: None,
            children: self
                .template
                .iter()
                .map(crate::Element::clone_template)
                .collect(),
        });

        self.items.len() - 1
    }

    pub fn add_rich_line(&mut self, line: crate::rich::Line, color: u32) -> usize {
        let index = self.add_line(&line.text(), color);

        self.items[index].rich = Some(line);

        index
    }

    pub fn remove_front(&mut self, count: usize) {
        let ordinary = self.tree.is_none() && self.custom.is_none();

        if ordinary && self.items.is_empty() {
            return;
        }

        let count = count.min(self.items.len());

        if let Some(tree) = &mut self.tree {
            tree.remove_prefix(count);
        }

        if let Some(custom) = &mut self.custom {
            custom.focused = custom
                .focused
                .and_then(|(row, child)| row.checked_sub(count).map(|row| (row, child)));

            custom.hovered = custom
                .hovered
                .and_then(|(row, child)| row.checked_sub(count).map(|row| (row, child)));

            custom.drag_offset = None;
        }

        self.items.drain(..count);

        let remaining = self.items.len() as i32;

        if ordinary {
            self.empty_range_reset = false;

            self.scroll = self
                .scroll
                .clamp(0, remaining.saturating_sub(self.scroll_page).max(0));
        }

        if remaining <= self.selected {
            self.selected = remaining - 1;
        }

        if remaining <= self.selection_anchor {
            self.selection_anchor = (remaining - 1).max(0);
        }
    }
}

impl ListState {
    fn apply(&mut self, property: &Property, element: ElementType) -> bool {
        let key = property.base_key();

        let ordinary = element != ElementType::CustomListView;

        let view = matches!(
            element,
            ElementType::ListView | ElementType::TreeView | ElementType::CustomListView
        );

        match key {
            k if k.eq_ignore_ascii_case("BorderWidth") => {
                self.border_width = property.int();
            }
            k if k.eq_ignore_ascii_case("Spacing") => self.spacing = property.int(),
            k if k.eq_ignore_ascii_case("ItemSpacing") => {
                self.item_spacing = property.int();
            }
            k if k.eq_ignore_ascii_case("ScrollBarWidth") => {
                self.scrollbar_width = property.int();
            }
            k if k.eq_ignore_ascii_case("LeftScrollbar") => {
                self.left_scrollbar = property.flag();
            }
            k if ordinary && k.eq_ignore_ascii_case("RichText") => {
                self.richtext = property.flag();
            }
            k if ordinary && k.eq_ignore_ascii_case("DrawSelectBoxLast") => {
                self.draw_select_box_last = property.flag();
            }
            k if ordinary && k.eq_ignore_ascii_case("RenderMethod") => {
                self.render_method = property.int();
            }
            k if view && k.eq_ignore_ascii_case("ItemSize") => {
                property.scan(&mut self.item_size);
            }
            k if view && k.eq_ignore_ascii_case("ItemPanel") => {
                self.item_panel = property.value.clone();

                self.pending_item_panels.push(property.value.clone());
            }
            _ => return false,
        }

        true
    }
}
