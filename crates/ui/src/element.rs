use crate::hash::name_hash;
use crate::material::{ColorSet, Material, State};
use crate::resource::Resource;
use crate::text::Strings;
use crate::widget::Widget;
use formats::ui::pane::{ElementType, Property, RasterOp, ShadowType, element_bool, font_align};

pub const MAX_FONT: i32 = 20;

pub const NO_PAGE: u8 = 0xFF;

pub fn constructor_layers(element: ElementType) -> usize {
    match element {
        ElementType::Image => 1,
        ElementType::GaugeBar | ElementType::ImageFont => 2,
        _ => 0,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub instance: Instance,
    pub element_type: ElementType,
    pub type_name: String,
    pub name: String,
    pub name_hash: i32,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub layers: Vec<Material>,
    pub visible: bool,
    pub enabled: bool,
    pub msg_receive: bool,
    pub is_default: bool,
    pub below_panel: bool,
    pub page_index: u8,
    pub shadow_type: ShadowType,
    pub shadow_color: u32,
    pub screen_align: u32,
    pub accel_key: i32,
    pub tagname: String,
    pub tooltip_key: String,
    pub tooltip: String,
    pub caption_key: String,
    pub caption: String,
    pub text: String,
    pub display: String,
    caption_translation: Option<String>,
    display_translation: Option<String>,
    tooltip_translation: Option<String>,
    pub multiline: bool,
    pub multiline_gap: i32,
    pub richtext: bool,
    pub static_hit_enabled: bool,
    rich_layout_created: bool,
    pub text_insets: [i32; 4],
    pub widget: Widget,
    pub extra: Vec<Property>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Instance(u64);

impl Instance {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(1);

        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("UI instance identity space exhausted"),
        )
    }
}

impl Element {
    pub fn set_combo_font_color(&mut self, argb: u32) -> bool {
        if self.widget.combo_box().is_none() {
            return false;
        }

        for index in [0, 2] {
            if let Some(layer) = self.layers.get_mut(index) {
                layer.font_color.set(State::Normal, argb);
            }
        }

        true
    }

    pub fn set_balloon_font_color(&mut self, argb: u32) -> bool {
        let Some(state) = self.widget.balloon_mut() else {
            return false;
        };

        state.font_color = argb;

        if let Some(body) = self.layers.first_mut() {
            body.font_color.set_normal_live(argb);
        }

        true
    }

    pub fn set_icon_contents(
        &mut self,
        contents: Option<crate::icon::IconContents>,
    ) -> Option<&mut crate::widget::ButtonState> {
        let button = self.widget.button_mut()?;

        self.tooltip.clear();

        self.tooltip_translation = None;

        button.set_icon_contents_for_element(contents, self.visible);

        Some(button)
    }

    pub fn clone_template(&self) -> Self {
        if matches!(
            self.element_type,
            ElementType::EditBox | ElementType::ImeEditBox | ElementType::ImeNotePadEditBox
        ) {
            let (kind, name) = if self.element_type == ElementType::EditBox {
                (ElementType::EditBox, "EditBox")
            } else {
                (ElementType::ImeEditBox, "ImeEditBox")
            };

            return self.fresh_template_base(kind, name);
        }

        if matches!(
            self.element_type,
            ElementType::Button | ElementType::CheckBox | ElementType::RadioButton
        ) {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_button_properties_from(self);

            return copy;
        }

        if self.element_type == ElementType::IconButton
            && self
                .widget
                .button()
                .is_some_and(|b| b.has_empty_template_inputs())
        {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_empty_icon_properties_from(self);

            return copy;
        }

        if matches!(
            self.element_type,
            ElementType::Static | ElementType::BalloonBox | ElementType::TooltipBox
        ) {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_static_properties_from(self);

            return copy;
        }

        if self.element_type == ElementType::GaugeBar {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_gauge_properties_from(self);

            return copy;
        }

        if self.element_type == ElementType::ImageFont {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_image_font_properties_from(self);

            return copy;
        }

        if self.element_type == ElementType::ComboBox {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_combo_box_properties_from(self);

            return copy;
        }

        if matches!(self.element_type, ElementType::Slider | ElementType::MoMesh) {
            return self.fresh_template_base(self.element_type, &self.type_name);
        }

        if matches!(
            self.element_type,
            ElementType::ListBox
                | ElementType::ListView
                | ElementType::TreeView
                | ElementType::CustomListView
        ) {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.copy_list_properties_from(self);

            return copy;
        }

        if self.element_type == ElementType::ImeNotePad {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            let source = self.widget.edit().unwrap();

            let edit = copy.widget.edit_mut().unwrap();

            edit.border_width = source.border_width;

            edit.spacing = source.spacing;

            edit.left_scrollbar = source.left_scrollbar;

            edit.richtext = source.richtext;

            edit.pad.as_mut().unwrap().scrollbar_width =
                source.pad.as_ref().unwrap().scrollbar_width;

            return copy;
        }

        if self.element_type == ElementType::Image
            && let Widget::Image(source) = &self.widget
        {
            let mut copy = self.fresh_template_base(self.element_type, &self.type_name);

            copy.tooltip_key = self.tooltip_key.clone();

            copy.tooltip = self.tooltip.clone();

            copy.tooltip_translation = self.tooltip_translation.clone();

            let mut state = crate::widget::ImageState::default();

            state.copy_template_from(source);

            copy.widget = Widget::Image(Box::new(state));

            return copy;
        }

        let mut copy = self.clone();

        copy.renew_instances();

        copy
    }

    fn fresh_template_base(&self, kind: ElementType, type_name: &str) -> Self {
        let mut copy = Self::new(kind, type_name, "", &Resource::default());

        copy.copy_base_properties_from(self);

        copy
    }

    pub fn copy_editor_properties_from(&mut self, source: &Self) -> bool {
        if !matches!(
            self.element_type,
            ElementType::EditBox | ElementType::ImeEditBox | ElementType::ImeNotePadEditBox
        ) {
            return false;
        }

        self.copy_base_properties_from(source);

        true
    }

    pub fn copy_static_properties_from(&mut self, source: &Self) -> bool {
        if !accepts_text(self.element_type) || !accepts_text(source.element_type) {
            return false;
        }

        self.copy_base_properties_from(source);

        self.caption_key = source.caption_key.clone();

        self.caption = source.caption.clone();

        self.text = source.text.clone();

        self.display = source.display.clone();

        self.tooltip_key = source.tooltip_key.clone();

        self.tooltip = source.tooltip.clone();

        self.caption_translation = source.caption_translation.clone();

        self.display_translation = source.display_translation.clone();

        self.tooltip_translation = source.tooltip_translation.clone();

        self.shadow_type = source.shadow_type;

        self.shadow_color = source.shadow_color;

        self.text_insets = source.text_insets;

        self.multiline = source.multiline;

        self.richtext = source.richtext;

        self.static_hit_enabled = source.static_hit_enabled;

        true
    }

    pub fn copy_button_properties_from(&mut self, source: &Self) -> bool {
        let basic = |kind| {
            matches!(
                kind,
                ElementType::Button | ElementType::CheckBox | ElementType::RadioButton
            )
        };

        if !basic(self.element_type)
            || !basic(source.element_type)
            || self.widget.button().is_none()
        {
            return false;
        }

        let Some(button) = source.widget.button() else {
            return false;
        };

        self.copy_static_properties_from(source);

        let state = self.widget.button_mut().expect("validated button state");

        state.toggle = button.toggle;

        state.group = button.group.clone();

        state.button_sound = button.button_sound;

        state.down_text_up = button.down_text_up;

        true
    }

    pub fn copy_empty_icon_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != ElementType::IconButton
            || source.element_type != ElementType::IconButton
        {
            return false;
        }

        let Some(button) = source.widget.button() else {
            return false;
        };

        if !button.has_empty_template_inputs() {
            return false;
        }

        self.copy_static_properties_from(source);

        self.tooltip.clear();

        self.tooltip_translation = None;

        self.widget
            .button_mut()
            .expect("IconButton state")
            .copy_empty_icon_from(button, self.visible);

        true
    }

    pub fn copy_combo_box_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != ElementType::ComboBox
            || source.element_type != ElementType::ComboBox
            || self.widget.combo_box().is_none()
        {
            return false;
        }

        let Some(combo) = source.widget.combo_box() else {
            return false;
        };

        self.copy_static_properties_from(source);

        let button = &mut self
            .widget
            .combo_box_mut()
            .expect("validated ComboBox state")
            .button;

        button.toggle = combo.button.toggle;

        button.group = combo.button.group.clone();

        button.button_sound = combo.button.button_sound;

        button.down_text_up = combo.button.down_text_up;

        true
    }

    pub fn copy_slider_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != ElementType::Slider || source.element_type != ElementType::Slider {
            return false;
        }

        self.copy_base_properties_from(source);

        true
    }

    pub fn copy_image_font_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != ElementType::ImageFont
            || source.element_type != ElementType::ImageFont
            || self.widget.image_font().is_none()
        {
            return false;
        }

        let Some(font) = source.widget.image_font() else {
            return false;
        };

        self.copy_base_properties_from(source);

        self.widget
            .image_font_mut()
            .expect("validated ImageFont state")
            .copy_template_from(font);

        true
    }

    pub fn copy_gauge_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != ElementType::GaugeBar
            || source.element_type != ElementType::GaugeBar
            || self.widget.gauge().is_none()
        {
            return false;
        }

        let Some(gauge) = source.widget.gauge() else {
            return false;
        };

        self.copy_static_properties_from(source);

        let state = self.widget.gauge_mut().expect("validated gauge state");

        state.range = gauge.range;

        state.position = gauge.position;

        state.out_space = gauge.out_space;

        state.text_type = gauge.text_type;

        state.text_postfix = gauge.text_postfix.clone();

        true
    }

    pub fn copy_list_properties_from(&mut self, source: &Self) -> bool {
        if self.element_type != source.element_type || self.widget.list().is_none() {
            return false;
        }

        let Some(list) = source.widget.list() else {
            return false;
        };

        self.copy_base_properties_from(source);

        self.refresh_list_view_layout();

        self.refresh_tree_layout();

        let state = self.widget.list_mut().expect("validated list state");

        state.scrollbar_width = list.scrollbar_width;

        state.border_width = list.border_width;

        state.spacing = list.spacing;

        state.left_scrollbar = list.left_scrollbar;

        if self.element_type == ElementType::CustomListView {
            state.item_spacing = list.item_spacing;

            state.item_size = list.item_size;

            state.item_panel = list.item_panel.clone();
        } else {
            state.text_row_height = list.text_row_height;

            state.ignore_selection = list.ignore_selection;

            state.richtext = list.richtext;

            state.selection_flags = list.selection_flags;
        }

        self.refresh_list_view_layout();

        self.refresh_tree_layout();

        if matches!(
            self.element_type,
            ElementType::ListView | ElementType::TreeView
        ) {
            self.widget.list_mut().unwrap().item_size = list.item_size;
        }

        true
    }

    fn copy_base_properties_from(&mut self, source: &Self) {
        self.x = source.x;

        self.y = source.y;

        self.width = source.width;

        self.height = source.height;

        self.visible = source.visible;

        self.msg_receive = source.msg_receive;

        self.name = source.name.clone();

        self.name_hash = name_hash(&self.name);

        self.tagname = source.tagname.clone();

        self.layers = source.layers.clone();
    }

    fn renew_instances(&mut self) {
        self.instance = Instance::new();

        if let Some(pad) = self.widget.edit_mut().and_then(|e| e.pad.as_mut()) {
            pad.scrollbar.drag_offset = None;

            pad.scroll_metrics = None;

            pad.scrollbar.press = 0;

            pad.scrollbar.hover = [false, false];
        }

        if let Some(list) = self.widget.list_mut() {
            for child in list
                .template
                .iter_mut()
                .chain(list.items.iter_mut().flat_map(|r| &mut r.children))
            {
                child.renew_instances();
            }
        }
    }

    pub fn set_image_uv(&mut self, uv: Option<[i32; 4]>) {
        if let Widget::Image(state) = &mut self.widget {
            if let Some(uv) = uv
                && let Some(material) = self.layers.first_mut()
            {
                material.uv = uv;
            }

            state.use_external_uv = uv.is_some();
        }
    }

    pub fn new(
        element_type: ElementType,
        type_name: &str,
        name: &str,
        resource: &Resource,
    ) -> Element {
        let mut element = Element {
            instance: Instance::new(),
            element_type,
            type_name: type_name.to_string(),
            name: name.to_string(),
            name_hash: name_hash(name),
            id: -1,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            layers: vec![Material::widget_seed(); constructor_layers(element_type)],
            visible: true,
            enabled: true,
            msg_receive: true,
            is_default: false,
            below_panel: false,
            page_index: NO_PAGE,
            shadow_type: ShadowType::None,
            shadow_color: 0xFF00_0000,
            screen_align: 0,
            accel_key: 0,
            tagname: String::new(),
            tooltip_key: String::new(),
            tooltip: String::new(),
            caption_key: String::new(),
            caption: String::new(),
            display: String::new(),
            text: String::new(),
            caption_translation: None,
            display_translation: None,
            tooltip_translation: None,
            multiline: false,
            multiline_gap: 5,
            richtext: false,
            static_hit_enabled: false,
            rich_layout_created: false,
            text_insets: [0; 4],
            widget: Widget::new(element_type),
            extra: Vec::new(),
        };

        if matches!(
            element_type,
            ElementType::BalloonBox | ElementType::TooltipBox
        ) {
            element.layers = vec![Material::widget_seed(); 2];

            for layer in resource
                .layers(ElementType::BalloonBox)
                .filter(|l| l.layer < 2)
            {
                element.layers[layer.layer as usize] = layer.material;
            }

            element.layers[0].font_color.select(State::Normal);

            element.set_rich_text(true);

            element.multiline_gap = 3;
        }

        for layer in resource.layers(element_type) {
            element.set_layer(layer.layer as usize, layer.material);
        }

        element.bind_embedded_materials(resource);

        element
    }

    pub fn bind_embedded_materials(&mut self, resource: &Resource) {
        if matches!(
            self.element_type,
            ElementType::Button | ElementType::CheckBox | ElementType::RadioButton
        ) && let Some(button) = self.widget.button_mut()
        {
            button.default_key |= self.name.eq_ignore_ascii_case("ok");

            button.cancel_key |= self.name.eq_ignore_ascii_case("cancel");
        }

        if self.widget.balloon().is_some() {
            self.width = 200;

            self.height = 150;

            self.text_insets = [5; 4];
        }

        if let Some(combo) = self.widget.combo_box_mut() {
            combo.font_heights = resource.fonts.iter().map(|f| (f.index, f.height)).collect();

            for layer in resource.layers(ElementType::ScrollBar) {
                let index = layer.layer as usize;

                combo.scrollbar.layers.resize(
                    combo.scrollbar.layers.len().max(index + 1),
                    Material::default(),
                );

                combo.scrollbar.layers[index] = layer.material;
            }
        }

        if let Some(list) = self.widget.list_mut() {
            for layer in resource.layers(ElementType::ScrollBar) {
                let index = layer.layer as usize;

                if list.scrollbar.layers.len() <= index {
                    list.scrollbar.layers.resize(index + 1, Material::default());
                }

                list.scrollbar.layers[index] = layer.material;
            }
        }

        if let Some(pad) = self.widget.edit_mut().and_then(|e| e.pad.as_mut()) {
            pad.font_heights = resource.fonts.iter().map(|f| (f.index, f.height)).collect();

            for layer in resource.layers(ElementType::ScrollBar) {
                let index = layer.layer as usize;

                pad.scrollbar.layers.resize(
                    pad.scrollbar.layers.len().max(index + 1),
                    Material::default(),
                );

                pad.scrollbar.layers[index] = layer.material;
            }

            for layer in resource.layers(ElementType::ImeEditBox) {
                let index = layer.layer as usize;

                pad.child_layers
                    .resize(pad.child_layers.len().max(index + 1), Material::default());

                pad.child_layers[index] = layer.material;
            }
        }
    }

    pub fn set_layer(&mut self, index: usize, material: Material) {
        if self.layers.len() <= index {
            self.layers.resize(index + 1, Material::default());
        }

        self.layers[index] = material;
    }

    pub fn apply(&mut self, property: &Property, resource: &Resource, strings: &Strings) {
        self.apply_property(property, resource, strings);

        if [
            "area",
            "rect",
            "left",
            "top",
            "width",
            "height",
            "BorderWidth",
            "Spacing",
            "ItemSpacing",
            "ScrollBarWidth",
            "LeftScrollbar",
            "ItemSize",
        ]
        .iter()
        .any(|key| property.base_key().eq_ignore_ascii_case(key))
        {
            self.refresh_list_view_layout();

            self.refresh_tree_layout();

            if !["ScrollBarWidth", "ItemSize"]
                .iter()
                .any(|k| property.base_key().eq_ignore_ascii_case(k))
                && let Some(pad) = self.widget.edit_mut().and_then(|e| e.pad.as_mut())
            {
                pad.layout_changed();
            }
        }
    }

    pub fn refresh_list_view_layout(&mut self) {
        if self.element_type != ElementType::ListView {
            return;
        }

        let rect = self.rect();

        let Some(list) = self.widget.list_mut() else {
            return;
        };

        let content = list.content_rect(rect);

        list.scroll_page = list.rows_per_page(content);

        list.scroll = list.clamped_scroll(content);

        if list.selected >= 0 {
            let row = (list.selected / list.columns(content)).min(list.scroll_range(content).1 - 1);

            let page = list.rows_per_page(content);

            if row < list.scroll {
                list.scroll = row;
            } else if row >= list.scroll.saturating_add(page) {
                list.scroll = row.saturating_sub(page).saturating_add(1);
            }
        }
    }

    pub fn refresh_tree_layout(&mut self) {
        if self.element_type != ElementType::TreeView {
            return;
        }

        let rect = self.rect();

        let Some(list) = self.widget.list_mut() else {
            return;
        };

        let Some(tree) = &list.tree else {
            return;
        };

        let layout = tree.layout(list, rect);

        list.scroll_page = layout.page as i32;

        list.scroll = layout.first as i32;
    }

    fn apply_property(&mut self, property: &Property, resource: &Resource, strings: &Strings) {
        if self.apply_common(property, resource) {
            return;
        }

        if self.apply_type_specific(property, strings) {
            return;
        }

        if let Some(font) = self.widget.image_font_mut()
            && font.apply(property, resource)
        {
            return;
        }

        if property.base_key().eq_ignore_ascii_case("Icon") {
            let button = match &mut self.widget {
                Widget::Button(state) => Some(state.as_mut()),
                Widget::ComboBox(state) => Some(&mut state.button),
                _ => None,
            };

            if let Some(button) = button {
                button.icon =
                    !property.value.is_empty() && resource.alias(&property.value).is_some();

                return;
            }
        }

        if self.widget.apply(property, self.element_type) {
            return;
        }

        self.extra.push(property.clone());
    }

    fn apply_common(&mut self, property: &Property, resource: &Resource) -> bool {
        let key = property.base_key();

        let layer = property.layer().map(|(_, digit)| digit);

        let value = property.value.as_str();

        match key {
            k if k.eq_ignore_ascii_case("id") => self.id = property.int(),
            k if k.eq_ignore_ascii_case("name") => {
                self.name = value.to_string();

                self.name_hash = name_hash(value);
            }
            k if k.eq_ignore_ascii_case("PageIndex") => {
                self.page_index = property.int() as u8;
            }
            k if k.eq_ignore_ascii_case("enabled") => {
                self.enabled = element_bool(k, value);
            }
            k if k.eq_ignore_ascii_case("visible") => {
                self.visible = element_bool(k, value);
            }
            k if k.eq_ignore_ascii_case("MsgReceive") => {
                self.msg_receive = element_bool(k, value);
            }
            k if k.eq_ignore_ascii_case("area") => {
                let [x, y, w, h] = property.ints4();

                (self.x, self.y, self.width, self.height) = (x, y, w, h);
            }
            k if k.eq_ignore_ascii_case("rect") => {
                let [l, t, r, b] = property.ints4();

                (self.x, self.y, self.width, self.height) = (l, t, r - l, b - t);
            }
            k if k.eq_ignore_ascii_case("left") => self.x = property.int(),
            k if k.eq_ignore_ascii_case("top") => self.y = property.int(),
            k if k.eq_ignore_ascii_case("width") => self.width = property.int(),
            k if k.eq_ignore_ascii_case("height") => self.height = property.int(),
            k if k.eq_ignore_ascii_case("default") => {
                self.is_default = !value.eq_ignore_ascii_case("false");
            }
            k if k.eq_ignore_ascii_case("BelowPanel") => {
                self.below_panel = property.flag();
            }
            k if k.eq_ignore_ascii_case("shadowType") => {
                self.shadow_type = ShadowType::parse(value);
            }
            k if k.eq_ignore_ascii_case("shadowColor") => {
                self.shadow_color = property.argb();
            }
            k if k.eq_ignore_ascii_case("Tagname") => self.tagname = value.to_string(),
            k if k.eq_ignore_ascii_case("Font") => {
                let index = property.int().clamp(0, MAX_FONT);

                self.for_layers(layer, |m| m.font_index = index);
            }
            k if k.eq_ignore_ascii_case("FontAlign") => {
                let flags = self.fold_align(value);

                self.for_layers(layer, |m| m.font_align = flags);
            }
            k if k.eq_ignore_ascii_case("FontOffset") => {
                let [x, y, _, _] = property.ints4();

                self.for_layers(layer, |m| m.font_offset = [x, y]);
            }
            k if k.eq_ignore_ascii_case("FontColor") => {
                let argb = property.argb();

                self.for_layers(layer, |m| m.font_color.set_all(argb));
            }
            k if k.eq_ignore_ascii_case("TextureColor") => {
                let argb = property.argb();

                self.for_layers(layer, |m| m.texture_color.set_all(argb));
            }
            k if k.eq_ignore_ascii_case("RasterOp") => {
                let op = RasterOp::parse(value).unwrap_or(RasterOp::Stretch);

                self.for_layers(layer, |m| m.raster_op = op);
            }
            k if k.eq_ignore_ascii_case("TextureAlias") => {
                let (index, uv) = resource.alias_rect(value);

                match layer {
                    Some(n) => self.with_layer(n, |m| {
                        m.texture_index = index;

                        m.uv = uv;
                    }),
                    None => {
                        self.with_layer(0, |m| {
                            m.texture_index = index;

                            m.uv = uv;
                        });

                        if self.element_type == ElementType::Button {
                            for m in self.layers.iter_mut().skip(1) {
                                m.texture_index = index;

                                m.uv = uv;
                            }
                        }
                    }
                }
            }
            _ => return self.apply_state_color(key, property, layer),
        }

        true
    }

    fn apply_state_color(&mut self, key: &str, property: &Property, layer: Option<i32>) -> bool {
        let Some((channel, state)) = key.split_once('.') else {
            return false;
        };

        let state = match state {
            s if s.eq_ignore_ascii_case("Normal") => State::Normal,
            s if s.eq_ignore_ascii_case("Pressed") => State::Pressed,
            s if s.eq_ignore_ascii_case("MouseOver") => State::MouseOver,
            s if s.eq_ignore_ascii_case("Disabled") => State::Disabled,
            _ => return false,
        };

        let argb = property.argb();

        if channel.eq_ignore_ascii_case("FontColor") {
            self.for_layers(layer, |m| m.font_color.set(state, argb));
        } else if channel.eq_ignore_ascii_case("TextureColor") {
            self.for_layers(layer, |m| m.texture_color.set(state, argb));
        } else {
            return false;
        }

        true
    }

    fn apply_type_specific(&mut self, property: &Property, strings: &Strings) -> bool {
        let key = property.base_key();

        let value = property.value.as_str();

        let text_widget = accepts_text(self.element_type);

        match key {
            k if k.eq_ignore_ascii_case("ToolTip")
                && (text_widget || self.element_type == ElementType::Image) =>
            {
                self.set_tooltip(value, strings);
            }
            k if k.eq_ignore_ascii_case("Caption") && text_widget => {
                self.caption_key = value.to_string();

                self.caption = strings.translate(value);

                self.caption_translation = Strings::is_key(value).then(|| self.caption.clone());

                if !value.is_empty() {
                    self.set_display(&self.caption.clone());

                    self.display_translation = Strings::is_key(value).then(|| self.display.clone());
                }
            }
            k if k.eq_ignore_ascii_case("Text") && text_widget => {
                self.text = value.to_string();

                self.set_display(value);
            }
            k if k.eq_ignore_ascii_case("MultiLine") && text_widget => {
                self.multiline = property.flag();
            }
            k if k.eq_ignore_ascii_case("MultiLineGap") && text_widget => {
                let mut gap = [5];

                property.scan(&mut gap);

                self.multiline_gap = gap[0];
            }
            k if k.eq_ignore_ascii_case("RichText") && text_widget => {
                self.set_rich_text(property.flag());
            }
            _ => return false,
        }

        true
    }

    fn fold_align(&self, value: &str) -> u32 {
        let mut flags = self.layers.first().map_or(0, |m| m.font_align);

        for token in value.split([' ', '\t']).filter(|t| !t.is_empty()) {
            match token {
                t if t.eq_ignore_ascii_case("left") => {
                    flags &= !(font_align::CENTER | font_align::RIGHT);
                }
                t if t.eq_ignore_ascii_case("right") => {
                    flags = flags & !font_align::CENTER | font_align::RIGHT;
                }
                t if t.eq_ignore_ascii_case("center") => {
                    flags = flags & !font_align::RIGHT | font_align::CENTER;
                }
                _ => {}
            }

            match token {
                t if t.eq_ignore_ascii_case("top") => {
                    flags &= !(font_align::VCENTER | font_align::BOTTOM);
                }
                t if t.eq_ignore_ascii_case("vcenter") => {
                    flags = flags & !font_align::BOTTOM | font_align::VCENTER;
                }
                t if t.eq_ignore_ascii_case("bottom") => {
                    flags = flags & !font_align::VCENTER | font_align::BOTTOM;
                }
                t if t.eq_ignore_ascii_case("wordbreak") => {
                    flags |= font_align::WORDBREAK;
                }
                _ => {}
            }
        }

        flags
    }

    fn for_layers(&mut self, layer: Option<i32>, f: impl Fn(&mut Material)) {
        match layer {
            Some(n) => self.with_layer(n, f),
            None => self.layers.iter_mut().for_each(f),
        }
    }

    fn with_layer(&mut self, index: i32, f: impl FnOnce(&mut Material)) {
        if let Ok(index) = usize::try_from(index)
            && let Some(material) = self.layers.get_mut(index)
        {
            f(material);
        }
    }

    pub fn rect(&self) -> [i32; 4] {
        [self.x, self.y, self.x + self.width, self.y + self.height]
    }

    pub fn matches(&self, name: &str, element_type: ElementType) -> bool {
        self.element_type == element_type
            && self.name_hash == name_hash(name)
            && self.name.eq_ignore_ascii_case(name)
    }
}

impl Element {
    pub fn set_display(&mut self, text: &str) {
        self.display_translation = None;

        self.display = set_text(text);

        if accepts_text(self.element_type)
            && [
                "<color", "<n>", "<font", "<dic", "<dicf", "<dict", "<icon", "<refs",
            ]
            .iter()
            .any(|prefix| self.display.contains(prefix))
        {
            self.multiline = true;

            self.richtext = true;
        }

        self.prepare_text_layout();

        if let Some(edit) = self.widget.edit_mut().filter(|e| e.note_pad) {
            crate::notepad::clear_rows(edit);

            edit.text = text.to_owned();

            edit.caret = 0;

            edit.anchor = 0;

            if let Some(pad) = edit.pad.as_mut() {
                pad.scroll = 0;

                pad.rows_cleared = false;
            }
        }
    }

    pub fn set_rich_text(&mut self, on: bool) {
        self.richtext = on;

        self.multiline |= on;

        self.static_hit_enabled |= on;
    }

    pub fn prepare_text_layout(&mut self) {
        if accepts_text(self.element_type) && self.multiline {
            self.rich_layout_created = true;
        }
    }

    pub fn has_text_layout(&self) -> bool {
        self.rich_layout_created
    }

    pub fn set_layer_texture_color(&mut self, layer: usize, argb: u32) {
        if let Some(material) = self.layers.get_mut(layer) {
            material.texture_color = ColorSet::init(argb, crate::material::DEFAULT_DISABLED);
        }
    }

    pub fn set_layer_texture_alias(&mut self, layer: usize, alias: &str, resource: &Resource) {
        let (texture_index, uv) = resource.alias_rect(alias);

        if let Some(material) = self.layers.get_mut(layer) {
            material.texture_index = texture_index;

            material.uv = uv;
        }
    }

    pub fn set_tooltip(&mut self, key: &str, strings: &Strings) {
        self.tooltip_key = key.to_string();

        self.tooltip_translation = None;

        if !key.is_empty() {
            self.set_tooltip_text(&strings.translate(key));

            self.tooltip_translation = Strings::is_key(key).then(|| self.tooltip.clone());
        }
    }

    pub fn set_tooltip_text(&mut self, text: &str) {
        self.tooltip_translation = None;

        self.tooltip = text.to_string();
    }

    pub fn relocalize(&mut self, strings: &Strings) {
        let caption_bound = Strings::is_key(&self.caption_key)
            && self.caption_translation.as_ref() == Some(&self.caption);

        let display_bound = Strings::is_key(&self.caption_key)
            && self.display_translation.as_ref() == Some(&self.display);

        let caption =
            (caption_bound || display_bound).then(|| strings.translate(&self.caption_key));

        if caption_bound {
            self.caption = caption.as_ref().unwrap().clone();

            self.caption_translation = Some(self.caption.clone());
        } else {
            self.caption_translation = None;
        }

        if display_bound {
            self.set_display(caption.as_ref().unwrap());

            self.display_translation = Some(self.display.clone());
        } else {
            self.display_translation = None;
        }

        if Strings::is_key(&self.tooltip_key)
            && self.tooltip_translation.as_ref() == Some(&self.tooltip)
        {
            self.tooltip = strings.translate(&self.tooltip_key);

            self.tooltip_translation = Some(self.tooltip.clone());
        } else {
            self.tooltip_translation = None;
        }

        if let Some(list) = self.widget.list_mut() {
            for child in list
                .template
                .iter_mut()
                .chain(list.items.iter_mut().flat_map(|row| &mut row.children))
            {
                child.relocalize(strings);
            }
        }
    }
}

fn set_text(text: &str) -> String {
    text.replace('_', " ")
}

pub fn accepts_text(element: ElementType) -> bool {
    matches!(
        element,
        ElementType::Static
            | ElementType::Button
            | ElementType::CheckBox
            | ElementType::RadioButton
            | ElementType::IconButton
            | ElementType::ComboBox
            | ElementType::BalloonBox
            | ElementType::TooltipBox
            | ElementType::GaugeBar
    )
}
