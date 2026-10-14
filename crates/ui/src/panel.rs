use crate::element::Element;
use crate::material::{Material, State};
use crate::resource::{PanelMaterials, Resource};
use crate::text::Strings;
use formats::ui::pane::{
    AlignPanel, Block, ElementType, Pane, Property, RasterOp, ShadowType, resize_tip, screen_align,
};

pub const CLOSE_BUTTON: &str = "close";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    pub texture_index: i32,
    pub uv: [i32; 4],
    pub raster_op: RasterOp,
    pub color: u32,
    pub offset: [i32; 4],
}

impl Default for Background {
    fn default() -> Background {
        Background {
            texture_index: -1,
            uv: [0, 0, 100, 100],
            raster_op: RasterOp::Wrap,
            color: 0xFFFF_FFFF,
            offset: [0; 4],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    pub resource_path: String,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub z_order: i32,
    pub background: Background,
    pub screen_align: u32,
    pub align_x: AlignPanel,
    pub align_y: AlignPanel,
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale_type: i32,
    pub caption_key: String,
    pub caption: String,
    pub caption_translation: Option<String>,
    pub caption_color: u32,
    pub caption_shadow_type: ShadowType,
    pub caption_shadow_color: u32,
    pub caption_font: i32,
    pub titlebar: bool,
    pub titlebar_height: i32,
    pub close_button: Option<Element>,
    pub close_button_location: [i32; 4],
    pub movable: bool,
    pub visible: bool,
    pub allow_focus: bool,
    pub input_focus_latched: bool,
    pub enter_defaults: bool,
    pub tab_navigation: bool,
    pub modal: bool,
    pub focus_protected: bool,
    pub force_notifications: bool,
    pub popup: bool,
    pub keep_popup_on_click: bool,
    pub page_index: i32,
    pub open_sound: i32,
    pub close_sound: i32,
    pub resize_tip: u32,
    pub animation: String,
    pub materials: PanelMaterials,
    pub elements: Vec<Element>,
    pub next_element_id: i32,
    pub focus: Option<usize>,
    pub warnings: Vec<String>,
}

impl Default for Panel {
    fn default() -> Panel {
        Panel {
            resource_path: String::new(),
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
            z_order: 0,
            background: Background::default(),
            screen_align: 0,
            align_x: AlignPanel::None,
            align_y: AlignPanel::None,
            offset_x: 0.0,
            offset_y: 0.0,
            scale_type: 0,
            caption_key: String::new(),
            caption: String::new(),
            caption_translation: None,
            caption_color: 0xFFFF_FFFF,
            caption_shadow_type: ShadowType::None,
            caption_shadow_color: 0xFF00_0000,
            caption_font: 0,
            titlebar: false,
            titlebar_height: 0x1C,
            close_button: None,
            close_button_location: [0; 4],
            movable: true,
            visible: true,
            allow_focus: true,
            input_focus_latched: false,
            enter_defaults: true,
            tab_navigation: false,
            modal: false,
            focus_protected: false,
            force_notifications: false,
            popup: false,
            keep_popup_on_click: false,
            page_index: -1,
            open_sound: 0,
            close_sound: 0,
            resize_tip: 0,
            animation: String::new(),
            materials: PanelMaterials::default(),
            elements: Vec::new(),
            next_element_id: 0,
            focus: None,
            warnings: Vec::new(),
        }
    }
}

pub fn resource_path(pane: &Pane) -> Option<&str> {
    pane.panel.get("Resource")
}

impl Panel {
    pub fn relocalize(&mut self, strings: &Strings) {
        if Strings::is_key(&self.caption_key)
            && self.caption_translation.as_ref() == Some(&self.caption)
        {
            self.caption = strings.translate(&self.caption_key);

            self.caption_translation = Some(self.caption.clone());
        } else {
            self.caption_translation = None;
        }

        for element in &mut self.elements {
            element.relocalize(strings);
        }

        if let Some(close) = &mut self.close_button {
            close.relocalize(strings);
        }
    }

    pub fn prepare_text_layouts(&mut self) {
        self.refresh_text_layouts(false);
    }

    pub fn refresh_text_scale(&mut self) {
        self.refresh_text_layouts(true);
    }

    fn refresh_text_layouts(&mut self, scale_only: bool) {
        fn visit(element: &mut Element, scale_only: bool) {
            if !scale_only || element.richtext {
                element.prepare_text_layout();
            }

            if let Some(list) = element.widget.list_mut() {
                for row in &mut list.items {
                    for child in &mut row.children {
                        visit(child, scale_only);
                    }
                }
            }
        }

        for element in &mut self.elements {
            visit(element, scale_only);
        }

        if let Some(close) = &mut self.close_button {
            visit(close, scale_only);
        }
    }

    pub fn instantiate(pane: &Pane, resource: &Resource, strings: &Strings, show: bool) -> Panel {
        let mut panel = Panel {
            materials: resource.panel,
            titlebar_height: resource.titlebar_height,
            close_button_location: resource.close_button_location,
            warnings: pane.warnings.clone(),
            ..Panel::default()
        };

        for property in &pane.panel.properties {
            panel.apply(property, resource, strings);
        }

        for declaration in &pane.elements {
            let Some(element_type) = declaration.element_type() else {
                panel.warnings.push(format!(
                    "unknown element type {:?} for {:?}",
                    declaration.type_name, declaration.name,
                ));

                continue;
            };

            let mut element = Element::new(
                element_type,
                &declaration.type_name,
                &declaration.name,
                resource,
            );

            panel.next_element_id += 1;

            element.id = panel.next_element_id;

            for property in &declaration.block.properties {
                element.apply(property, resource, strings);
            }

            element.prepare_text_layout();

            if let Some(edit) = element.widget.edit_mut()
                && edit.range <= 0
            {
                if edit.password {
                    edit.range = 11;
                } else if element.name == "id" {
                    edit.range = 14;
                }
            }

            panel.elements.push(element);
        }

        if !show {
            panel.allow_focus = false;
        }

        panel.focus = panel.initial_focus();

        panel
    }

    fn apply(&mut self, property: &Property, resource: &Resource, strings: &Strings) {
        let key = property.base_key();

        let value = property.value.as_str();

        match key {
            k if k.eq_ignore_ascii_case("Resource") => {
                self.resource_path = value.to_string();
            }
            k if k.eq_ignore_ascii_case("TextureAlias") => {
                let (index, uv) = resource.alias_rect(value);

                self.background.texture_index = index;

                self.background.uv = uv;
            }
            k if k.eq_ignore_ascii_case("RasterOp") => {
                self.background.raster_op = RasterOp::parse(value).unwrap_or(RasterOp::Stretch);
            }
            k if k.eq_ignore_ascii_case("textureOffset") => {
                self.background.offset = property.ints4();
            }
            k if k.eq_ignore_ascii_case("Color") || k.eq_ignore_ascii_case("texturecolor") => {
                self.background.color = property.argb();
            }
            k if k.eq_ignore_ascii_case("area") => {
                let [l, t, w, h] = property.ints4();

                (self.left, self.top, self.right, self.bottom) =
                    (l, t, l.wrapping_add(w), t.wrapping_add(h));
            }
            k if k.eq_ignore_ascii_case("rect") => {
                let [l, t, r, b] = property.ints4();

                (self.left, self.top, self.right, self.bottom) = (l, t, r, b);
            }
            k if k.eq_ignore_ascii_case("left") => {
                let width = self.right.wrapping_sub(self.left);

                self.left = property.int();

                self.right = self.left.wrapping_add(width);
            }
            k if k.eq_ignore_ascii_case("top") => {
                let height = self.bottom.wrapping_sub(self.top);

                self.top = property.int();

                self.bottom = self.top.wrapping_add(height);
            }
            k if k.eq_ignore_ascii_case("Width") => {
                self.right = self.left.wrapping_add(property.int());
            }
            k if k.eq_ignore_ascii_case("Height") => {
                self.bottom = self.top.wrapping_add(property.int());
            }
            k if k.eq_ignore_ascii_case("ZOrder") => self.z_order = property.int(),
            k if k.eq_ignore_ascii_case("Caption") => {
                self.caption_key = value.to_string();

                self.caption = strings.translate(value);

                self.caption_translation = Strings::is_key(value).then(|| self.caption.clone());
            }
            k if k.eq_ignore_ascii_case("CaptionColor") => {
                self.caption_color = property.argb();

                self.materials.caption.font_color = crate::material::ColorSet::init(
                    self.caption_color,
                    crate::material::DEFAULT_DISABLED,
                );
            }
            k if k.eq_ignore_ascii_case("CaptionShadowType") => {
                let shadow = ShadowType::parse(value);

                if shadow != ShadowType::None {
                    self.caption_shadow_type = shadow;
                }
            }
            k if k.eq_ignore_ascii_case("CaptionShadowColor") => {
                self.caption_shadow_color = property.argb();
            }
            k if k.eq_ignore_ascii_case("CaptionFont") => {
                self.caption_font = property.int();

                self.materials.caption.font_index = self.caption_font;
            }
            k if k.eq_ignore_ascii_case("Titlebar") => self.titlebar = property.flag(),
            k if k.eq_ignore_ascii_case("TitlebarHeight") => {
                self.titlebar_height = property.int();
            }
            k if k.eq_ignore_ascii_case("CloseButton") => {
                if property.flag() {
                    self.create_close_button(resource);
                }
            }
            k if k.eq_ignore_ascii_case("CloseButtonLocation") => {
                self.close_button_location = property.ints4();
            }
            k if k.eq_ignore_ascii_case("AllowFocus") => {
                if value.eq_ignore_ascii_case("false") {
                    self.allow_focus = false;
                }
            }
            k if k.eq_ignore_ascii_case("PageIndex") => self.page_index = property.int(),
            k if k.eq_ignore_ascii_case("OpenSound") => self.open_sound = property.int(),
            k if k.eq_ignore_ascii_case("CloseSound") => {
                self.close_sound = property.int();
            }
            k if k.eq_ignore_ascii_case("resizeTip") => {
                self.resize_tip = resize_tip::parse(
                    value
                        .split([',', ' ', '\r', '\n'])
                        .next()
                        .unwrap_or_default(),
                );
            }
            k if k.eq_ignore_ascii_case("ScreenAlign") => {
                self.screen_align |= screen_align::parse(value);
            }
            k if k.eq_ignore_ascii_case("AlignPanelX") => {
                let align = AlignPanel::parse(value);

                if align != AlignPanel::None || value.eq_ignore_ascii_case("none") {
                    self.align_x = align;
                }
            }
            k if k.eq_ignore_ascii_case("AlignPanelY") => {
                let align = AlignPanel::parse(value);

                if align != AlignPanel::None || value.eq_ignore_ascii_case("none") {
                    self.align_y = align;
                }
            }
            k if k.eq_ignore_ascii_case("OffsetX") => self.offset_x = float(value),
            k if k.eq_ignore_ascii_case("OffsetY") => self.offset_y = float(value),
            k if k.eq_ignore_ascii_case("PanelScaleType") => {
                if property.int() != 0 {
                    self.scale_type = 1;
                }
            }
            k if k.eq_ignore_ascii_case("Animation") => {
                self.animation = value.to_string();
            }
            _ => self
                .warnings
                .push(format!("unknown panel property {:?}", property.key)),
        }
    }

    fn create_close_button(&mut self, resource: &Resource) {
        if self.close_button.is_some() {
            return;
        }

        let mut button = Element::new(ElementType::Button, "Button", CLOSE_BUTTON, resource);

        button.set_layer(0, self.materials.close_normal);

        button.set_layer(1, self.materials.close_pressed);

        button.set_layer(2, self.materials.close_mouseover);

        if let Some(hover) = button.layers.get_mut(2) {
            hover.texture_color.set(State::Pressed, 0);
        }

        self.close_button = Some(button);
    }

    pub fn initial_focus(&self) -> Option<usize> {
        if !self.allow_focus {
            return None;
        }

        if let Some(index) = self.elements.iter().position(|e| e.is_default) {
            return Some(index);
        }

        self.elements.iter().position(|e| {
            matches!(
                e.element_type,
                ElementType::Button | ElementType::EditBox | ElementType::ImeEditBox
            )
        })
    }

    pub fn element(&self, name: &str, element_type: ElementType) -> Option<&Element> {
        self.elements.iter().find(|e| e.matches(name, element_type))
    }

    pub fn element_mut(&mut self, name: &str, element_type: ElementType) -> Option<&mut Element> {
        self.elements
            .iter_mut()
            .find(|e| e.matches(name, element_type))
    }

    pub fn set_visible_by_tagname(&mut self, tag: &str, visible: bool) -> usize {
        let mut matched = 0;

        for element in &mut self.elements {
            if element.tagname.eq_ignore_ascii_case(tag) {
                element.visible = visible;

                matched += 1;
            }
        }

        matched
    }

    pub fn element_or_create(
        &mut self,
        name: &str,
        element_type: ElementType,
        resource: &Resource,
    ) -> &mut Element {
        if let Some(index) = self
            .elements
            .iter()
            .position(|e| e.matches(name, element_type))
        {
            return &mut self.elements[index];
        }

        let mut element = Element::new(element_type, "", name, resource);

        self.next_element_id += 1;

        element.id = self.next_element_id;

        self.elements.push(element);

        self.elements.last_mut().expect("just pushed")
    }

    pub fn create_element(
        &mut self,
        name: &str,
        element_type: ElementType,
        resource: &Resource,
    ) -> &mut Element {
        let mut element = Element::new(element_type, "", name, resource);

        self.next_element_id += 1;

        element.id = self.next_element_id;

        self.elements.push(element);

        self.elements.last_mut().expect("just pushed")
    }

    pub fn width(&self) -> i32 {
        self.right.wrapping_sub(self.left)
    }

    pub fn close_button_rect(&self) -> [i32; 4] {
        let [inset, top, width, height] = self.close_button_location;

        let right = self.right.saturating_sub(self.left).saturating_sub(inset);

        [
            right.saturating_sub(width),
            top,
            right,
            top.saturating_add(height),
        ]
    }

    pub fn height(&self) -> i32 {
        self.bottom.wrapping_sub(self.top)
    }

    pub fn position(&self, screen: [i32; 4]) -> (i32, i32) {
        let [left, top, right, bottom] = screen;

        let x = match self.align_x {
            AlignPanel::None => self.offset_x as i32 + left,
            AlignPanel::Center => (left + right) / 2 - self.width() / 2 + self.offset_x as i32,
            AlignPanel::Side => (self.left - self.right) + right + self.offset_x as i32,
        };

        let y = match self.align_y {
            AlignPanel::None => self.offset_y as i32 + top,
            AlignPanel::Center => (top + bottom) / 2 - self.height() / 2 + self.offset_y as i32,
            AlignPanel::Side => (self.top - self.bottom) + bottom + self.offset_y as i32,
        };

        (x, y)
    }

    pub fn materials(&self) -> impl Iterator<Item = &Material> {
        self.elements
            .iter()
            .chain(self.close_button.as_ref())
            .flat_map(|e| e.layers.iter())
    }
}

fn float(value: &str) -> f32 {
    let text = value.trim();

    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e'))
        .unwrap_or(text.len());

    text[..end].parse().unwrap_or(0.0)
}

pub fn empty_block() -> Block {
    Block::default()
}
