use crate::element::Element;
use crate::layout::{Measure, Unmeasured};
use crate::material::{ColorSet, DEFAULT_DISABLED, Material, State};
use crate::panel::Panel;
use crate::widget::ListState;
use formats::ui::pane::{ElementType, RasterOp, ShadowType, font_align};

pub const TITLEBAR_INSET: i32 = 3;

pub const TITLEBAR_WIDTH_LOSS: i32 = 6;

pub const ICON_MARGIN: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    Background,
    Titlebar,
    Element(usize),
    Close,
    ListRow {
        element: usize,
        row: usize,
        child: Option<usize>,
    },
    ListChildRow {
        element: usize,
        row: usize,
        child: usize,
        item: usize,
    },
    Descendant {
        element: usize,
        node: crate::element::Instance,
        item: Option<usize>,
    },
    Carried,
}

fn text_row_owner(owner: Owner, item: usize) -> Owner {
    match owner {
        Owner::Element(element) => Owner::ListRow {
            element,
            row: item,
            child: None,
        },
        Owner::ListRow {
            element,
            row,
            child: Some(child),
        } => Owner::ListChildRow {
            element,
            row,
            child,
            item,
        },
        Owner::Descendant { element, node, .. } => Owner::Descendant {
            element,
            node,
            item: Some(item),
        },
        _ => owner,
    }
}

pub(crate) fn child_owner(owner: Owner, row: usize, child: usize, element: &Element) -> Owner {
    match owner {
        Owner::Element(index) => Owner::ListRow {
            element: index,
            row,
            child: Some(child),
        },
        Owner::ListRow { element: index, .. } | Owner::Descendant { element: index, .. } => {
            Owner::Descendant {
                element: index,
                node: element.instance,
                item: None,
            }
        }
        _ => owner,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Interaction {
    pub hovered: Option<Owner>,
    pub pressed: Option<Owner>,
    pub press_lifts: bool,
    pub focused: Option<Owner>,
    pub now_ms: Option<u32>,
}

impl Interaction {
    pub fn state_of(&self, owner: Owner, element: &Element) -> (State, i32) {
        if !element.enabled {
            return (State::Disabled, 0);
        }

        let latched = matches!(
            element.element_type,
            ElementType::Button | ElementType::IconButton
        ) && element
            .widget
            .button()
            .is_some_and(|button| (button.toggle || !button.group.is_empty()) && button.checked);

        let scrollbar_drag = element
            .widget
            .combo_box()
            .is_some_and(|c| c.drag_offset.is_some());

        if (self.pressed == Some(owner) && !scrollbar_drag) || latched {
            let lifts = self.press_lifts || element.widget.button().is_some_and(|b| b.down_text_up);

            return (State::Pressed, if lifts { -1 } else { 1 });
        }

        if self.hovered == Some(owner) {
            return (State::MouseOver, 0);
        }

        (State::Normal, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerDraw {
    pub owner: Owner,
    pub texture_index: i32,
    pub uv: [i32; 4],
    pub rect: [i32; 4],
    pub color: u32,
    pub raster_op: RasterOp,
    pub additive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextDraw {
    pub owner: Owner,
    pub text: String,
    pub rect: [i32; 4],
    pub indent: i32,
    pub font: i32,
    pub align: u32,
    pub color: u32,
    pub shadow: ShadowType,
    pub shadow_color: u32,
    pub bold: bool,
}

impl TextDraw {
    pub fn width(&self) -> i32 {
        self.rect[2] - self.rect[0]
    }

    pub fn height(&self) -> i32 {
        self.rect[3] - self.rect[1]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillDraw {
    pub owner: Owner,
    pub rect: [i32; 4],
    pub color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconDraw {
    pub owner: Owner,
    pub sheet: i32,
    pub uv: [i32; 4],
    pub rect: [i32; 4],
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedIconDraw {
    pub owner: Owner,
    pub path: String,
    pub secondary_path: Option<&'static str>,
    pub uv: Option<[i32; 4]>,
    pub rect: [i32; 4],
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageDraw {
    pub owner: Owner,
    pub material: Material,
    pub rect: [i32; 4],
    pub state: crate::widget::ImageState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditScrollAdmission {
    pub text: String,
    pub caret: i32,
    pub previous: i32,
    pub next: i32,
    pub width: i32,
    pub font: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Layer(LayerDraw),
    Text(TextDraw),
    Fill(FillDraw),
    Sweep(SweepDraw),
    Icon(IconDraw),
    NamedIcon(NamedIconDraw),
    Image(ImageDraw),
    ImageTick {
        owner: Owner,
    },
    CaretTick {
        owner: Owner,
        focused: bool,
    },
    EditAlignment {
        owner: Owner,
        instance: crate::element::Instance,
    },
    EditScroll {
        owner: Owner,
        instance: crate::element::Instance,
        admission: EditScrollAdmission,
    },
    NotePage {
        owner: Owner,
        instance: crate::element::Instance,
        admission: crate::notepad::PageAdmission,
    },
    IconScopePush {
        owner: Owner,
        path: String,
        secondary_path: Option<&'static str>,
    },
    IconScopePop {
        owner: Owner,
    },
    ClipPush {
        owner: Owner,
        rect: [i32; 4],
    },
    ClipPop {
        owner: Owner,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SweepDraw {
    pub owner: Owner,
    pub rect: [i32; 4],
    pub angle: f32,
}

impl SweepDraw {
    pub fn triangles(&self) -> Vec<[[f32; 2]; 3]> {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

        if !self.angle.is_finite() || self.angle <= 0.0 {
            return Vec::new();
        }

        let angle = self.angle.min(TAU);

        let mut points = vec![[0.5, 0.0]];

        for (i, corner) in [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]
            .into_iter()
            .enumerate()
        {
            if angle > FRAC_PI_4 + i as f32 * FRAC_PI_2 {
                points.push(corner);
            }
        }

        let (s, c) = angle.sin_cos();

        let extent = s.abs().max(c.abs());

        points.push([0.5 - 0.5 * s / extent, 0.5 - 0.5 * c / extent]);

        points
            .windows(2)
            .map(|p| [[0.5, 0.5], p[1], p[0]])
            .collect()
    }
}

impl Command {
    pub fn owner(&self) -> Owner {
        match self {
            Command::Layer(layer) => layer.owner,
            Command::Text(text) => text.owner,
            Command::Fill(fill) => fill.owner,
            Command::Sweep(sweep) => sweep.owner,
            Command::Icon(icon) => icon.owner,
            Command::NamedIcon(icon) => icon.owner,
            Command::Image(image) => image.owner,
            Command::ImageTick { owner } => *owner,
            Command::CaretTick { owner, .. } => *owner,
            Command::EditAlignment { owner, .. } => *owner,
            Command::EditScroll { owner, .. } => *owner,
            Command::NotePage { owner, .. } => *owner,
            Command::ClipPush { owner, .. }
            | Command::ClipPop { owner }
            | Command::IconScopePush { owner, .. }
            | Command::IconScopePop { owner } => *owner,
        }
    }

    pub fn as_layer(&self) -> Option<&LayerDraw> {
        match self {
            Command::Layer(layer) => Some(layer),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&TextDraw> {
        match self {
            Command::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_fill(&self) -> Option<&FillDraw> {
        match self {
            Command::Fill(fill) => Some(fill),
            _ => None,
        }
    }

    pub fn as_icon(&self) -> Option<&IconDraw> {
        match self {
            Command::Icon(icon) => Some(icon),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList {
    pub commands: Vec<Command>,
    pub pending: Vec<(Owner, ElementType)>,
}

impl DrawList {
    fn finish_highlights(&mut self) {
        let mut clips = Vec::new();

        let mut highlights = Vec::new();

        self.commands.retain(|command| {
            match command {
                Command::ClipPush { .. } => clips.push(command.clone()),
                Command::ClipPop { .. } => {
                    clips.pop();
                }
                Command::Layer(layer) if layer.additive => {
                    highlights.extend(clips.iter().cloned());

                    highlights.push(command.clone());

                    highlights.extend(clips.iter().rev().map(|clip| Command::ClipPop {
                        owner: clip.owner(),
                    }));

                    return false;
                }
                _ => {}
            }

            true
        });

        self.commands.extend(highlights);
    }

    pub fn layers(&self) -> impl Iterator<Item = &LayerDraw> {
        self.commands.iter().filter_map(Command::as_layer)
    }

    pub fn texts(&self) -> impl Iterator<Item = &TextDraw> {
        self.commands.iter().filter_map(Command::as_text)
    }

    pub fn fills(&self) -> impl Iterator<Item = &FillDraw> {
        self.commands.iter().filter_map(Command::as_fill)
    }
}

impl Panel {
    pub fn set_combo_screen(&mut self, screen: [i32; 4], origin: (i32, i32)) {
        let mut elements: Vec<_> = self.elements.iter_mut().collect();

        while let Some(e) = elements.pop() {
            if let Some(combo) = e.widget.combo_box_mut() {
                combo.screen = (screen[3] > screen[1]).then_some([
                    screen[1].saturating_sub(origin.1),
                    screen[3].saturating_sub(origin.1),
                ]);
            }

            if let Some(list) = e.widget.list_mut() {
                elements.extend(list.template.iter_mut());

                for row in &mut list.items {
                    elements.extend(row.children.iter_mut());
                }
            }
        }
    }

    pub fn refresh_list_box_metrics(&mut self, measure: &mut dyn Measure) {
        let mut elements: Vec<_> = self.elements.iter_mut().collect();

        while let Some(element) = elements.pop() {
            if let Some(combo) = element.widget.combo_box_mut() {
                for material in &element.layers {
                    if let Some(height) = measure.height(material.font_index) {
                        if let Some((_, old)) = combo
                            .font_heights
                            .iter_mut()
                            .find(|(i, _)| *i == material.font_index)
                        {
                            *old = height;
                        } else {
                            combo.font_heights.push((material.font_index, height));
                        }
                    }
                }
            }

            let rect = element.rect();

            if let Some(state) = element.widget.list_mut() {
                if element.element_type == ElementType::ListBox {
                    let height = element
                        .layers
                        .first()
                        .and_then(|m| measure.height(m.font_index))
                        .filter(|h| *h > 0);

                    state.text_row_height = height;

                    if height.is_some() {
                        let content = state.content_rect(rect);

                        state.scroll_page = state.rows_per_page(content);

                        state.scroll = state.clamped_scroll(content);
                    }
                } else if element.element_type == ElementType::ListView {
                    state.scroll_page = state.rows_per_page(state.content_rect(rect));
                } else if element.element_type == ElementType::TreeView {
                    state.scroll_page = crate::tree_view::Tree::page(state, rect) as i32;
                }

                elements.extend(state.template.iter_mut());

                for row in &mut state.items {
                    elements.extend(row.children.iter_mut());
                }
            }
        }
    }

    pub fn draw_list(&self, interaction: &Interaction) -> DrawList {
        self.draw_list_with(interaction, &mut Unmeasured)
    }

    pub fn draw_list_with(&self, interaction: &Interaction, measure: &mut dyn Measure) -> DrawList {
        let mut list = DrawList::default();

        if !self.visible {
            return list;
        }

        let mut background_drawn = false;

        for (index, element) in self.elements.iter().enumerate() {
            let owner = Owner::Element(index);

            if !background_drawn && !element.below_panel {
                if !self.page_shows(element) {
                    continue;
                }

                background_drawn = true;

                self.push_background(&mut list);
            }

            if !element.visible || !self.page_shows(element) {
                continue;
            }

            let focused = interaction
                .focused
                .map_or(self.focus == Some(index), |who| who == owner);

            push_element(owner, element, interaction, focused, measure, &mut list);
        }

        if !background_drawn {
            self.push_background(&mut list);
        }

        if let Some(close) = &self.close_button {
            let mut placed = close.clone();

            let [left, top, right, bottom] = self.close_button_rect();

            placed.x = left;

            placed.y = top;

            placed.width = right.saturating_sub(left);

            placed.height = bottom.saturating_sub(top);

            let focused = interaction.focused == Some(Owner::Close);

            push_element(
                Owner::Close,
                &placed,
                interaction,
                focused,
                measure,
                &mut list,
            );
        }

        self.push_titlebar(&mut list);

        list.finish_highlights();

        for (index, element) in self.elements.iter().enumerate() {
            let owner = Owner::Element(index);

            let focused = interaction
                .focused
                .map_or(self.focus == Some(index), |who| who == owner);

            if focused
                && element.visible
                && self.page_shows(element)
                && element.widget.combo_box().is_some()
            {
                push_combo_popup(owner, element, measure, &mut list);
            }
        }

        list
    }

    fn page_shows(&self, element: &Element) -> bool {
        let element_page = i32::from(element.page_index as i8);

        self.page_index == -1 || element_page == -1 || self.page_index == element_page
    }

    fn push_background(&self, list: &mut DrawList) {
        if self.background.texture_index == -1 {
            return;
        }

        let [ox, oy, ow, oh] = self.background.offset;

        let material = Material {
            texture_index: self.background.texture_index,
            uv: self.background.uv,
            texture_color: ColorSet::init(self.background.color, DEFAULT_DISABLED),
            ..Material::seed()
        };

        push_layer(
            Owner::Background,
            &material,
            self.background.raster_op,
            [ox, oy, ow + self.width(), oh + self.height()],
            list,
        );
    }

    fn push_titlebar(&self, list: &mut DrawList) {
        if !self.titlebar {
            return;
        }

        let rect = [
            TITLEBAR_INSET,
            TITLEBAR_INSET,
            self.width() - TITLEBAR_WIDTH_LOSS,
            self.titlebar_height,
        ];

        let material = &self.materials.caption;

        push_layer(Owner::Titlebar, material, RasterOp::Stretch, rect, list);

        if self.caption.is_empty() || material.font_color.live[3] == 0.0 {
            return;
        }

        list.commands.push(Command::Text(TextDraw {
            owner: Owner::Titlebar,
            text: self.caption.clone(),
            rect,
            indent: 0,
            font: material.font_index,
            align: material.font_align,
            color: material.font_color.live_argb(),
            shadow: self.caption_shadow_type,
            shadow_color: self.caption_shadow_color,
            bold: false,
        }));
    }
}

fn push_element(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    focused: bool,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let mut blinking;

    let element = if let Some(now) = interaction.now_ms
        && let Some(edit) = element.widget.edit()
        && edit.caret_phase_at(now, focused) != edit.caret_visible
    {
        blinking = element.clone();

        blinking.widget.edit_mut().unwrap().caret_visible = edit.caret_phase_at(now, focused);

        &blinking
    } else {
        element
    };

    let mut composed;

    let element = if focused
        && let Some(edit) = element.widget.edit()
        && !edit.pad.as_ref().is_some_and(|pad| pad.rows_cleared)
        && let std::borrow::Cow::Owned(shown) = crate::text_entry::presentation(edit)
    {
        composed = element.clone();

        *composed.widget.edit_mut().unwrap() = shown;

        &composed
    } else {
        element
    };

    match element.element_type {
        ElementType::Static if element.multiline => {
            push_multiline_static(owner, element, measure, list);
        }
        ElementType::Static => {
            if let Some(material) = element.layers.first() {
                push_text(
                    owner,
                    element,
                    material,
                    element.rect(),
                    &element.display,
                    list,
                );
            }
        }
        ElementType::Button => push_button(owner, element, interaction, measure, list),
        ElementType::ComboBox => {
            push_combo_face(owner, element, interaction, measure, list);

            if !focused || !matches!(owner, Owner::Element(_)) {
                push_combo_popup(owner, element, measure, list);
            }
        }
        ElementType::BalloonBox | ElementType::TooltipBox => {
            push_balloon_widget(owner, element, measure, list)
        }
        ElementType::IconButton => {
            push_icon_button(owner, element, list);

            push_icon_highlight(owner, element, interaction, list);
        }
        ElementType::CheckBox | ElementType::RadioButton => {
            push_check_box(owner, element, interaction, list);
        }
        ElementType::Image => push_image(owner, element, list),
        ElementType::MoMesh => {}
        ElementType::ImageFont => {
            if let Some(state) = element.widget.image_font()
                && let Some(body) = element.layers.first()
                && let Some(layout) = state.layout(element.rect(), body.font_align)
            {
                push_layer(owner, body, body.raster_op, layout.body, list);

                if let Some(material) = element.layers.get(1) {
                    for (glyph, rect) in layout.glyphs {
                        let mut material = *material;

                        material.texture_index = glyph.texture;

                        material.uv = glyph.uv;

                        push_layer(owner, &material, RasterOp::Stretch, rect, list);
                    }
                }
            }
        }
        ElementType::GaugeBar => push_gauge(owner, element, list),
        ElementType::Slider => {
            if let Some(slider) = element.widget.slider() {
                let state = if !element.enabled {
                    State::Disabled
                } else if slider.drag_offset.is_some() {
                    State::Pressed
                } else if interaction.hovered == Some(owner) {
                    State::MouseOver
                } else {
                    State::Normal
                };

                for (material, rect) in element
                    .layers
                    .iter()
                    .take(2)
                    .zip([element.rect(), slider.thumb(element.rect())])
                {
                    let mut material = *material;

                    material.texture_color.select(state);

                    push_layer(owner, &material, material.raster_op, rect, list);
                }
            }
        }
        ElementType::EditBox | ElementType::ImeEditBox | ElementType::ImeNotePadEditBox => {
            push_edit_box(
                owner,
                element,
                focused,
                interaction.now_ms.map(|_| owner),
                measure,
                list,
            );
        }
        ElementType::ImeNotePad => push_notepad(
            owner,
            element,
            focused,
            interaction.now_ms.is_some(),
            measure,
            list,
        ),
        ElementType::ListBox => push_list_box(owner, element, measure, list),
        ElementType::TreeView => push_tree_view(owner, element, interaction, measure, list),
        ElementType::CustomListView => push_custom_list(owner, element, interaction, measure, list),
        ElementType::ListView if element.widget.list().is_some() => {
            push_list_view(owner, element, interaction, measure, list);
        }
        other => list.pending.push((owner, other)),
    }
}

fn combo_layout(element: &Element, measure: &mut dyn Measure) -> Option<crate::combo_box::Layout> {
    let state = element.widget.combo_box()?;

    let face = element.layers.first()?.font_index;

    let popup = element.layers.get(2)?.font_index;

    state.layout(
        element.rect(),
        measure.height(face).or_else(|| state.font_height(face))?,
        measure.height(popup).or_else(|| state.font_height(popup))?,
    )
}

fn push_combo_face(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.combo_box() else {
        return;
    };

    if let Some(body) = element.layers.first() {
        let mut body = *body;

        body.texture_color.select(State::Normal);

        push_layer(owner, &body, RasterOp::Split, element.rect(), list);
    }

    let layout = combo_layout(element, measure);

    if let Some(layout) = &layout {
        if let Some(row) = usize::try_from(state.selected)
            .ok()
            .and_then(|i| state.items.get(i))
            && let Some(material) = element.layers.get(3)
        {
            let mut material = *material;

            material.font_color.set_all(row.color);

            let [l, t, r, b] = layout.face;

            let width = layout.content[2].saturating_sub(layout.content[0]);

            let x = l.saturating_add((r.saturating_sub(l).saturating_sub(width) / 2).max(0));

            let y = t.saturating_add((b.saturating_sub(t).saturating_sub(layout.pitch) / 2).max(0));

            push_text(
                owner,
                element,
                &material,
                [
                    x,
                    y,
                    x.saturating_add(width),
                    y.saturating_add(layout.pitch),
                ],
                &row.text,
                list,
            );
        }
    } else if !state.items.is_empty() {
        list.pending.push((owner, element.element_type));
    }

    if let Some(arrow) = element.layers.get(1) {
        let [l, t, r, b] = element.rect();

        let side = ((i64::from(b) - i64::from(t)) as f64 * 0.8999999761581421) as i32;

        let rect = layout.map_or([r.saturating_sub(side).max(l), t, r, b], |l| l.arrow);

        let mut arrow = *arrow;

        let (color, _) = interaction.state_of(owner, element);

        arrow.texture_color.select(color);

        let shift = i32::from(color == State::Pressed);

        push_layer(
            owner,
            &arrow,
            arrow.raster_op,
            rect.map(|x| x.saturating_add(shift)),
            list,
        );
    }
}

fn push_combo_popup(
    owner: Owner,
    element: &Element,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element
        .widget
        .combo_box()
        .filter(|s| s.open && !s.items.is_empty())
    else {
        return;
    };

    let Some(layout) = combo_layout(element, measure) else {
        return;
    };

    let Some(body) = element.layers.get(2) else {
        return;
    };

    if layout.page < state.items.len() {
        push_bar(owner, element.enabled, &state.scrollbar, layout.bar, list);
    }

    let mut body = *body;

    body.texture_color.select(State::Normal);

    push_layer(owner, &body, body.raster_op, layout.popup, list);

    for &(index, rect) in &layout.rows {
        if (state.selected == index as i32 || state.hovered == index as i32)
            && let Some(highlight) = element.layers.get(3)
        {
            let mut highlight = *highlight;

            highlight
                .texture_color
                .select(if state.selected == index as i32 {
                    State::Normal
                } else {
                    State::MouseOver
                });

            push_layer(owner, &highlight, body.raster_op, rect, list);
        }
    }

    for &(index, rect) in &layout.rows {
        let row = &state.items[index];

        let material = if state.selected == index as i32 || state.hovered == index as i32 {
            element.layers.get(3)
        } else {
            element.layers.first()
        };

        if let Some(material) = material {
            let mut material = *material;

            material.font_color.set_all(row.color);

            let row_owner = text_row_owner(owner, index);

            push_text(row_owner, element, &material, rect, &row.text, list);
        }
    }
}

fn push_balloon_widget(
    owner: Owner,
    element: &Element,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.balloon() else {
        return;
    };

    let [dx, dy] = state.translation(element.width, element.height);

    let shift = |r: [i32; 4]| {
        [
            r[0].saturating_add(dx),
            r[1].saturating_add(dy),
            r[2].saturating_add(dx),
            r[3].saturating_add(dy),
        ]
    };

    let [l, t, r, b] = element.rect();

    let frame = if state.flat {
        [l, t, r, b]
    } else {
        [
            l,
            t.saturating_sub(4),
            r.saturating_add(4),
            b.saturating_add(4),
        ]
    };

    if let Some(body) = element.layers.first() {
        let mut body = *body;

        if state.flat {
            body.texture_color.live[3] = 0.7;
        }

        push_layer(owner, &body, RasterOp::Split, shift(frame), list);
    }

    if !state.flat
        && let Some(tail) = element.layers.get(1)
    {
        let width = tail.uv[2].saturating_sub(tail.uv[0]);

        let height = tail.uv[3].saturating_sub(tail.uv[1]);

        let left = ((i64::from(frame[2]) - i64::from(frame[0]) - i64::from(width)) / 2
            + i64::from(frame[0]))
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;

        let top = frame[3].saturating_sub(4);

        push_layer(
            owner,
            tail,
            RasterOp::Wrap,
            shift([
                left,
                top,
                left.saturating_add(width),
                top.saturating_add(height),
            ]),
            list,
        );
    }

    let Some(material) = element.layers.first() else {
        return;
    };

    if element.display.is_empty() {
        return;
    }

    if element.multiline {
        let [il, it, ir, ib] = element.text_insets;

        push_rich_caption(
            owner,
            element,
            material,
            shift([l + il, t + it, r - ir, b - ib]),
            element.width - il - ir,
            measure,
            list,
        );
    } else {
        push_text(
            owner,
            element,
            material,
            shift(element.rect()),
            &element.display,
            list,
        );
    }
}

fn push_list_box(owner: Owner, element: &Element, measure: &mut dyn Measure, list: &mut DrawList) {
    let Some(state) = element.widget.list() else {
        return;
    };

    if let Some(material) = element.layers.first() {
        push_layer(owner, material, material.raster_op, element.rect(), list);
    }

    let Some(material) = element.layers.get(1) else {
        list.pending.push((owner, ElementType::ListBox));

        return;
    };

    let height = element
        .layers
        .first()
        .and_then(|m| measure.height(m.font_index))
        .filter(|h| *h > 0)
        .or(state.text_row_height);

    let Some(height) = height else {
        list.pending.push((owner, ElementType::ListBox));

        return;
    };

    let mut layout = std::borrow::Cow::Borrowed(state);

    if state.text_row_height != Some(height) {
        layout.to_mut().text_row_height = Some(height);
    }

    let content = layout.content_rect(element.rect());

    let scroll = layout.clamped_scroll(content);

    if layout.scroll != scroll {
        layout.to_mut().scroll = scroll;
    }

    for (index, row) in layout
        .items
        .iter()
        .enumerate()
        .skip(layout.scroll.max(0) as usize)
    {
        let (rect, visible) = layout.item_rect(index, content);

        if !visible {
            continue;
        }

        let row_owner = text_row_owner(owner, index);

        let selected = !layout.ignore_selection && layout.row_selected(index);

        let mut material = *material;

        let pick = if selected {
            State::Pressed
        } else {
            State::Normal
        };

        material.texture_color.select(pick);

        material.font_align &= !3;

        if layout.draw_highlight && (selected || row.background != 0) {
            if !selected {
                material.texture_color = crate::material::ColorSet::init(
                    row.background,
                    material.texture_color.disabled,
                );
            }

            push_layer(row_owner, &material, material.raster_op, rect, list);
        }

        let color = if row.color == 0 {
            material.font_color.live_argb()
        } else {
            row.color
        };

        if layout.richtext
            && let Some(line) = row.rich.as_ref().filter(|line| line.text() == row.text)
        {
            let mut offset = 0;

            for run in &line.runs {
                list.commands.push(Command::Text(TextDraw {
                    owner: row_owner,
                    text: run.text.clone(),
                    rect: [rect[0] + offset, rect[1], rect[2], rect[3]],
                    indent: offset,
                    font: material.font_index,
                    align: material.font_align,
                    color: run.style.color.map_or(color, |rgb| 0xFF000000 | rgb),
                    shadow: row.shadow,
                    shadow_color: run
                        .style
                        .shadow
                        .map_or(row.shadow_color, |rgb| 0xFF000000 | rgb),
                    bold: false,
                }));

                for unit in run.text.encode_utf16() {
                    offset += measure.advance(material.font_index, unit).unwrap_or(0);
                }
            }
        } else if !row.text.is_empty() {
            list.commands.push(Command::Text(TextDraw {
                owner: row_owner,
                text: row.text.clone(),
                rect,
                indent: 0,
                font: material.font_index,
                align: material.font_align,
                color,
                shadow: row.shadow,
                shadow_color: row.shadow_color,
                bold: false,
            }));
        }
    }

    if layout.show_scrollbar
        && (!layout.auto_hide_scrollbar
            || layout.rows_per_page(content) < layout.items.len() as i32)
    {
        push_scroll_bar(owner, element, &layout, list);
    }
}

fn push_tree_view(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.list() else {
        return;
    };

    let Some(tree) = &state.tree else {
        return;
    };

    let Some(body) = element.layers.first() else {
        list.pending.push((owner, ElementType::TreeView));

        return;
    };

    push_layer(owner, body, body.raster_op, element.rect(), list);

    let Some(text_material) = element.layers.get(1) else {
        list.pending.push((owner, ElementType::TreeView));

        return;
    };

    let layout = tree.layout(state, element.rect());

    for &(index, rect) in &layout.rows {
        let (Some(node), Some(row)) = (tree.node(index), state.items.get(index)) else {
            continue;
        };

        let row_owner = text_row_owner(owner, index);

        if tree.show_markers && node.marker {
            let layer = if !tree.has_children(index) {
                4
            } else if node.expanded {
                2
            } else {
                3
            };

            if let Some(marker) = element.layers.get(layer) {
                let q = (rect[3] - rect[1]) / 4;

                push_layer(
                    row_owner,
                    marker,
                    RasterOp::Stretch,
                    [rect[0] + q, rect[1] + q, rect[0] + 3 * q, rect[1] + 3 * q],
                    list,
                );
            } else {
                list.pending.push((row_owner, ElementType::TreeView));
            }
        }

        let selected = if state.ignore_selection {
            row.selected
        } else {
            state.row_selected(index)
        };

        let hovered = (interaction.hovered == Some(row_owner) || tree.hovered == Some(index))
            && !(tree.suppress_marker_hover && node.marker);

        let mut material = *text_material;

        material.font_align &= !3;

        material.font_color.select(State::Normal);

        material.texture_color.select(if selected {
            State::Pressed
        } else if hovered {
            State::MouseOver
        } else {
            State::Normal
        });

        if state.draw_highlight {
            push_layer(row_owner, &material, material.raster_op, rect, list);
        }

        if !row.text.is_empty() {
            let own_font = node.font != 0;

            let color = if own_font {
                body.font_color.live_argb()
            } else if row.color != 0 {
                row.color
            } else {
                material.font_color.live_argb()
            };

            let default_selected_shadow = selected && row.color == 0 && !own_font;

            list.commands.push(Command::Text(TextDraw {
                owner: row_owner,
                text: row.text.clone(),
                rect,
                indent: 0,
                font: if own_font {
                    i32::from(node.font)
                } else {
                    material.font_index
                },
                align: if own_font {
                    body.font_align & !3
                } else {
                    material.font_align
                },
                color,
                shadow: if default_selected_shadow {
                    formats::ui::pane::ShadowType::BottomRight
                } else {
                    row.shadow
                },
                shadow_color: if default_selected_shadow {
                    0xFF000000
                } else {
                    row.shadow_color
                },
                bold: false,
            }));
        }

        for (child_index, child) in row.children.iter().enumerate() {
            if !child.visible {
                continue;
            }

            let mut placed = child.clone();

            placed.x = placed.x.saturating_add(rect[0]);

            placed.y = placed.y.saturating_add(rect[1]);

            let child_owner = child_owner(owner, index, child_index, child);

            push_element(
                child_owner,
                &placed,
                interaction,
                interaction.focused == Some(child_owner),
                measure,
                list,
            );
        }
    }

    if state.show_scrollbar && (!state.auto_hide_scrollbar || layout.page < layout.visible.len()) {
        push_bar(owner, element.enabled, &state.scrollbar, layout.bar, list);
    }
}

fn push_custom_list(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.list() else {
        return;
    };

    let Some(custom) = &state.custom else {
        return;
    };

    let Some(body) = element.layers.first() else {
        list.pending.push((owner, ElementType::CustomListView));

        return;
    };

    let layout = crate::custom_list::layout(state, element.rect());

    list.commands.push(Command::ClipPush {
        owner,
        rect: element.rect(),
    });

    let mut body = *body;

    body.texture_color.select(State::Normal);

    push_layer(owner, &body, body.raster_op, element.rect(), list);

    let mut popup = None;

    for &(index, rect) in &layout.rows {
        for (child_index, child) in state.items[index].children.iter().enumerate() {
            if !child.visible {
                continue;
            }

            let mut placed = child.clone();

            placed.x = placed.x.saturating_add(rect[0]);

            placed.y = placed.y.saturating_add(rect[1]);

            let child_owner = child_owner(owner, index, child_index, child);

            let focused = interaction
                .focused
                .map_or(custom.focused == Some((index, child_index)), |who| {
                    who == child_owner
                });

            if focused && placed.widget.combo_box().is_some() {
                push_combo_face(child_owner, &placed, interaction, measure, list);

                popup = Some((child_owner, placed));
            } else {
                push_element(child_owner, &placed, interaction, focused, measure, list);
            }
        }
    }

    list.commands.push(Command::ClipPop { owner });

    if let Some((owner, element)) = popup {
        let mut clips = Vec::new();

        for command in &list.commands {
            match command {
                Command::ClipPush { .. } => clips.push(command.clone()),
                Command::ClipPop { .. } => {
                    clips.pop();
                }
                _ => {}
            }
        }

        list.commands.extend(
            clips
                .iter()
                .rev()
                .map(|c| Command::ClipPop { owner: c.owner() }),
        );

        push_combo_popup(owner, &element, measure, list);

        list.commands.extend(clips);
    }

    if layout.show_bar {
        push_bar(owner, element.enabled, &state.scrollbar, layout.bar, list);
    }
}

fn push_list_view(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.list() else {
        return;
    };

    let content = state.content_rect(element.rect());

    let row_owner = |row: usize, child: Option<usize>| match child {
        Some(child) => child_owner(owner, row, child, &state.items[row].children[child]),
        None => text_row_owner(owner, row),
    };

    let first = match state.render_method {
        0 => (state.scroll * state.columns(content)).max(0) as usize,
        _ => 0,
    };

    if let Some(material) = element.layers.first() {
        push_layer(owner, material, material.raster_op, element.rect(), list);
    }

    if state.render_method != 0 {
        list.commands.push(Command::ClipPush {
            owner,
            rect: element.rect(),
        });
    }

    for (index, item) in state.items.iter().enumerate().skip(first) {
        if item.text.is_empty() {
            continue;
        }

        let (rect, visible) = state.item_rect(index, content);

        if !visible && state.render_method == 0 {
            continue;
        }

        let Some(material) = element.layers.first() else {
            break;
        };

        let color = match item.color {
            0 => material.font_color.live_argb(),
            own => own,
        };

        if state.richtext
            && let Some(line) = item.rich.as_ref().filter(|line| line.text() == item.text)
        {
            let mut offset = 0;

            for run in &line.runs {
                list.commands.push(Command::Text(TextDraw {
                    owner: row_owner(index, None),
                    text: run.text.clone(),
                    rect: [
                        rect[0] + offset,
                        rect[1],
                        rect[0] + offset + run.width,
                        rect[3],
                    ],
                    indent: offset,
                    font: material.font_index,
                    align: material.font_align,
                    color: run.style.color.map_or(color, |rgb| 0xFF00_0000 | rgb),
                    shadow: element.shadow_type,
                    shadow_color: run
                        .style
                        .shadow
                        .map_or(element.shadow_color, |rgb| 0xFF00_0000 | rgb),
                    bold: false,
                }));

                for unit in run.text.encode_utf16() {
                    offset += measure.advance(material.font_index, unit).unwrap_or(0);
                }
            }

            continue;
        }

        list.commands.push(Command::Text(TextDraw {
            owner: row_owner(index, None),
            text: item.text.clone(),
            rect,
            indent: 0,
            font: material.font_index,
            align: material.font_align,
            color,
            shadow: element.shadow_type,
            shadow_color: element.shadow_color,
            bold: false,
        }));
    }

    let mut highlights = DrawList::default();

    for (index, _item) in state.items.iter().enumerate().skip(first) {
        let (rect, visible) = state.item_rect(index, content);

        if !visible && state.render_method == 0 {
            continue;
        }

        let selected = !state.ignore_selection && state.row_selected(index);

        let state_pick = if selected {
            State::Pressed
        } else if interaction.hovered == Some(row_owner(index, None)) {
            State::MouseOver
        } else {
            State::Normal
        };

        if !state.draw_highlight {
            continue;
        }

        let Some(highlight) = element.layers.get(1) else {
            break;
        };

        let mut highlight = *highlight;

        highlight.texture_color.select(state_pick);

        push_layer(
            row_owner(index, None),
            &highlight,
            highlight.raster_op,
            rect,
            &mut highlights,
        );
    }

    if !state.draw_select_box_last {
        list.commands.append(&mut highlights.commands);
    }

    for (index, item) in state.items.iter().enumerate().skip(first) {
        let (rect, visible) = state.item_rect(index, content);

        if !visible && state.render_method == 0 {
            continue;
        }

        for (child_index, child) in item.children.iter().enumerate() {
            if !child.visible {
                continue;
            }

            let mut placed = child.clone();

            placed.x += rect[0];

            placed.y += rect[1];

            push_element(
                row_owner(index, Some(child_index)),
                &placed,
                interaction,
                interaction.focused == Some(row_owner(index, Some(child_index))),
                measure,
                list,
            );
        }
    }

    if state.draw_select_box_last {
        list.commands.append(&mut highlights.commands);
    }

    if state.show_scrollbar
        && (!state.auto_hide_scrollbar || state.rows_per_page(content) < state.items.len() as i32)
    {
        push_scroll_bar(owner, element, state, list);
    }

    if state.render_method != 0 {
        list.commands.push(Command::ClipPop { owner });
    }
}

fn push_scroll_bar(owner: Owner, element: &Element, state: &ListState, list: &mut DrawList) {
    push_bar(
        owner,
        element.enabled,
        &state.scrollbar,
        state.scrollbar_rects(element.rect()),
        list,
    );
}

fn push_bar(
    owner: Owner,
    enabled: bool,
    bar: &crate::widget::ScrollBar,
    rects: crate::widget::ScrollBarRects,
    list: &mut DrawList,
) {
    let select = if !enabled || rects.thumb.is_none() {
        State::Disabled
    } else {
        State::Normal
    };

    let quad = |index: usize, op: RasterOp, rect: [i32; 4], list: &mut DrawList| {
        let Some(material) = bar.layers.get(index) else {
            return;
        };

        let mut material = *material;

        material.texture_color.select(select);

        push_layer(owner, &material, op, rect, list);
    };

    quad(0, RasterOp::Split, rects.track, list);

    if bar.arrows {
        let up = match (bar.press, bar.hover[0]) {
            (1 | 3, _) => 4,
            (0, true) => 6,
            _ => 1,
        };

        let down = match (bar.press, bar.hover[1]) {
            (2 | 4, _) => 5,
            (0, true) => 7,
            _ => 2,
        };

        quad(up, RasterOp::Stretch, rects.up, list);

        quad(down, RasterOp::Stretch, rects.down, list);
    }

    if let Some(thumb) = rects.thumb {
        quad(3, RasterOp::Split, thumb, list);
    }
}

fn push_notepad(
    owner: Owner,
    element: &Element,
    focused: bool,
    timed: bool,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    if let Some(frame) = element.layers.first() {
        push_layer(owner, frame, frame.raster_op, element.rect(), list);
    }

    let Some(view) = crate::notepad::layout_for_draw(element, measure) else {
        list.pending.push((owner, ElementType::ImeNotePad));

        return;
    };

    let edit = element.widget.edit().unwrap();

    let pad = edit.pad.as_ref().unwrap();

    if let Some(admission) = view.page_admission {
        list.commands.push(Command::NotePage {
            owner,
            instance: element.instance,
            admission,
        });
    }

    let frame = &element.layers[0];

    let Some(material) = element.layers.get(1) else {
        return;
    };

    let active = focused.then(|| {
        view.lines
            .iter()
            .rposition(|r| r.start <= edit.caret.max(0) as usize)
            .unwrap_or(0)
    });

    let mut child = None;

    for (row, range) in view
        .lines
        .iter()
        .enumerate()
        .skip(view.scroll)
        .take(view.visible_rows)
    {
        let y = view.client[1] + (row - view.scroll) as i32 * view.pitch;

        let rect = [view.client[0], y, view.client[2], y + view.pitch];

        let text: String = view.chars[range.clone()].iter().collect();

        let row_owner = text_row_owner(owner, row);

        if active == Some(row) {
            let mut editor = element.clone();

            editor.element_type = ElementType::ImeEditBox;

            editor.layers = pad.child_layers.clone();

            editor.x = rect[0] - 5;

            editor.y = rect[1] - 5;

            editor.width = rect[2] - rect[0] + 25;

            editor.height = view.pitch + 10;

            for layer in &mut editor.layers {
                layer.font_index = frame.font_index;
            }

            let state = editor.widget.edit_mut().unwrap();

            state.pad = None;

            state.note_pad = false;

            state.border_width = 1;

            state.spacing = 4;

            state.draw_quads = false;

            state.scroll = 0;

            state.text = text;

            state.caret = (edit.caret - range.start as i32).clamp(0, range.len() as i32);

            state.anchor = (edit.anchor - range.start as i32).clamp(0, range.len() as i32);

            state.caret_suppressed = pad.read_only;

            state.text_color = frame.font_color.live_argb();

            child = Some((row_owner, editor));
        } else {
            let retained = view.retained_rows.as_ref().and_then(|rows| rows.get(row));

            if pad.draw_row_backgrounds
                && let Some(row) = retained.filter(|row| row.style.background != 0)
            {
                let mut background = *material;

                background.texture_color.set_all(row.style.background);

                push_layer(row_owner, &background, background.raster_op, rect, list);
            }

            let selected = focused
                && edit.caret != edit.anchor
                && range.start < edit.caret.max(edit.anchor).max(0) as usize
                && range.end > edit.caret.min(edit.anchor).max(0) as usize;

            if selected {
                list.commands.push(Command::Fill(FillDraw {
                    owner: row_owner,
                    rect,
                    color: edit.sel_bk_color,
                }));
            }

            let color = retained.map_or(0, |r| r.style.color);

            let color = if color == 0 {
                material.font_color.live_argb()
            } else {
                color
            };

            let (shadow, shadow_color) = retained
                .map_or((element.shadow_type, element.shadow_color), |r| {
                    (r.style.shadow, r.style.shadow_color)
                });

            if edit.richtext && retained.is_some_and(|r| r.style.color != 0) {
                if let Some(line) = retained.and_then(|r| r.rich.as_ref()) {
                    let mut offset = 0;

                    for run in &line.runs {
                        list.commands.push(Command::Text(TextDraw {
                            owner: row_owner,
                            text: run.text.clone(),
                            rect: [rect[0] + offset, rect[1], rect[2], rect[3]],
                            indent: offset,
                            font: material.font_index,
                            align: material.font_align | 0x120,
                            color: run.style.color.map_or(color, |rgb| 0xff000000 | rgb),
                            shadow,
                            shadow_color,
                            bold: false,
                        }));

                        for unit in run.text.encode_utf16() {
                            offset += measure.advance(material.font_index, unit).unwrap_or(0);
                        }
                    }
                }

                continue;
            }

            if text.is_empty() {
                continue;
            }

            list.commands.push(Command::Text(TextDraw {
                owner: row_owner,
                text,
                rect,
                indent: 0,
                font: material.font_index,
                align: material.font_align | 0x120,
                color,
                shadow,
                shadow_color,
                bold: false,
            }));
        }
    }

    push_bar(owner, element.enabled, &pad.scrollbar, view.bar, list);

    if let Some((row_owner, child)) = child {
        push_edit_box(
            row_owner,
            &child,
            true,
            timed.then_some(owner),
            measure,
            list,
        );
    }
}

fn push_multiline_static(
    owner: Owner,
    element: &Element,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    if element.display.is_empty() {
        return;
    }

    let Some(material) = element.layers.first() else {
        return;
    };

    let [inset_left, inset_top, inset_right, inset_bottom] = element.text_insets;

    let wrap_width = element.width - inset_left - inset_right;

    let [left, top, right, bottom] = element.rect();

    let rect = [
        left + inset_left,
        top + inset_top,
        right - inset_right,
        bottom - inset_bottom,
    ];

    push_rich_caption(owner, element, material, rect, wrap_width, measure, list);
}

fn push_rich_caption(
    owner: Owner,
    element: &Element,
    material: &Material,
    rect: [i32; 4],
    wrap_width: i32,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let align = material.font_align;

    let Some(layout) = crate::rich::wrap(
        &element.display,
        material.font_index,
        wrap_width,
        element.multiline_gap,
        measure,
    ) else {
        list.pending.push((owner, element.element_type));

        return;
    };

    let [left, top, _right, bottom] = rect;

    let mut y = top + crate::layout::block_offset(align, bottom - top, layout.total_height());

    for line in &layout.lines {
        let mut dx = crate::layout::line_offset(align, wrap_width, line.width);

        for run in &line.runs {
            let base_color = material.font_color.live_argb();

            list.commands.push(Command::Text(TextDraw {
                owner,
                text: run.text.clone(),
                rect: [left + dx, y, left + dx + run.width, y + line.height],
                indent: dx,
                font: run.style.font.unwrap_or(material.font_index),
                align,
                color: (base_color & 0xFF00_0000)
                    | run.style.color.unwrap_or(base_color) & 0x00FF_FFFF,
                shadow: element.shadow_type,
                shadow_color: run
                    .style
                    .shadow
                    .map_or(element.shadow_color, |rgb| 0xFF00_0000 | rgb),
                bold: false,
            }));

            dx += run.width;
        }

        y += line.height;
    }
}

fn push_edit_box(
    owner: Owner,
    element: &Element,
    focused: bool,
    clock_owner: Option<Owner>,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let mut scrolled;

    let element = if let Some(edit) = element
        .widget
        .edit()
        .filter(|e| !e.password && !e.multiline)
        && let Some(font) = element.layers.get(1).map(|m| m.font_index)
    {
        let rect = edit
            .rects(
                element.element_type,
                element.x,
                element.y,
                element.width,
                element.height,
            )
            .text;

        let width = rect[2].saturating_sub(rect[0]).max(0);

        let mut edges = vec![0i32];

        let mut complete = true;

        for ch in edit.text.chars() {
            let mut advance = 0i32;

            for unit in ch.encode_utf16(&mut [0; 2]) {
                if let Some(value) = measure.advance(font, *unit) {
                    advance = advance.saturating_add(value.max(0));
                } else {
                    complete = false;

                    break;
                }
            }

            if !complete {
                break;
            }

            edges.push(edges.last().unwrap().saturating_add(advance));
        }

        let caret = (edit.caret.max(0) as usize).min(edges.len() - 1);

        let mut first = (edit.scroll.max(0) as usize).min(caret);

        while first < caret && edges[caret].saturating_sub(edges[first]) > width {
            first += 1;
        }

        if complete && first as i32 != edit.scroll {
            list.commands.push(Command::EditScroll {
                owner,
                instance: element.instance,
                admission: EditScrollAdmission {
                    text: edit.text.clone(),
                    caret: edit.caret,
                    previous: edit.scroll,
                    next: first as i32,
                    width,
                    font,
                },
            });

            scrolled = element.clone();

            scrolled.widget.edit_mut().unwrap().scroll = first as i32;

            &scrolled
        } else {
            element
        }
    } else {
        element
    };

    push_edit_body(owner, element, focused, clock_owner, measure, list);

    let Some(state) = element.widget.edit().filter(|s| focused && s.indicator) else {
        return;
    };

    let Some(rect) = state
        .rects(
            element.element_type,
            element.x,
            element.y,
            element.width,
            element.height,
        )
        .ime
    else {
        return;
    };

    let Some(material) = element.layers.get(2) else {
        list.pending.push((owner, element.element_type));

        return;
    };

    let mut material = *material;

    material.texture_color.select(State::Normal);

    push_layer(owner, &material, material.raster_op, rect, list);

    let color = match state.ime_status {
        crate::widget::ImeStatus::Disabled => 0xFF80_8080,
        crate::widget::ImeStatus::Ready => 0xFFFF_FFFF,
        crate::widget::ImeStatus::Composing => 0xFFFF_D060,
    };

    list.commands.push(Command::Text(TextDraw {
        owner,
        text: "IM".into(),
        rect,
        indent: 0,
        font: material.font_index,
        align: 0x125,
        color,
        shadow: element.shadow_type,
        shadow_color: element.shadow_color,
        bold: false,
    }));
}

fn push_edit_body(
    owner: Owner,
    element: &Element,
    focused: bool,
    clock_owner: Option<Owner>,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let Some(state) = element.widget.edit() else {
        return;
    };

    let rects = state.rects(
        element.element_type,
        element.x,
        element.y,
        element.width,
        element.height,
    );

    let select = if element.enabled {
        State::Normal
    } else {
        State::Disabled
    };

    if state.draw_quads {
        if let Some(frame) = element.layers.first().filter(|_| state.draw_box) {
            let mut frame = *frame;

            frame.texture_color.select(select);

            push_layer(owner, &frame, frame.raster_op, rects.frame, list);
        }

        if let Some(area) = element.layers.get(1) {
            push_layer(owner, area, area.raster_op, rects.text, list);
        }
    }

    let Some(material) = element.layers.get(1) else {
        return;
    };

    let (font, mut align) = (material.font_index, material.font_align);

    let color = crate::material::pack(crate::material::unpack(state.text_color));

    let active = state.masked();

    let units: Vec<u16> = active.encode_utf16().collect();

    let scroll_width = prefix_width(&units, font, state.scroll, measure);

    let mut caret_x = prefix_width(&units, font, state.caret, measure);

    let mut anchor_x = if state.caret == state.anchor {
        caret_x
    } else {
        prefix_width(&units, font, state.anchor, measure)
    };

    let centred = align & font_align::CENTER != 0;

    let whole_width = if centred {
        prefix_width(&units, font, WHOLE_STRING, measure)
    } else {
        0
    };

    if centred {
        let offset =
            ((rects.text[2] - rects.text[0]) / 2 - (whole_width - scroll_width) / 2).max(0);

        caret_x += offset;

        anchor_x += offset;
    }

    if state.multiline {
        caret_x = prefix_width(&units, font, state.caret, measure);
    }

    let band = (focused && state.caret != state.anchor).then(|| {
        let dx = rects.text[0] - scroll_width;

        let mut rect = intersect(
            rects.text,
            [
                caret_x.min(anchor_x) + dx,
                rects.text[1],
                caret_x.max(anchor_x) + dx,
                rects.text[3],
            ],
        );

        rect[3] += 1;

        rect
    });

    if let Some(rect) = band {
        list.commands.push(Command::Fill(FillDraw {
            owner,
            rect,
            color: state.sel_bk_color,
        }));
    }

    let start = edit_unit_index(&units, state.scroll);

    let body = String::from_utf16_lossy(&units[start..]);

    if state.multiline {
        align = 0x10;

        list.commands.push(Command::EditAlignment {
            owner,
            instance: element.instance,
        });

        if let Some(height) = measure.height(font) {
            let width = prefix_width(&units[start..], font, WHOLE_STRING, measure);

            list.commands.push(Command::Fill(FillDraw {
                owner,
                rect: [0, 0, width, height],
                color: 0x6400ff00,
            }));
        } else {
            list.pending.push((owner, element.element_type));
        }
    }

    if !body.is_empty() && color >> 24 != 0 {
        list.commands.push(Command::Text(TextDraw {
            owner,
            text: body,
            rect: rects.text,
            indent: 0,
            font,
            align,
            color,
            shadow: element.shadow_type,
            shadow_color: element.shadow_color,
            bold: false,
        }));
    }

    if let Some(rect) = band.filter(|_| !state.multiline) {
        let first = state.scroll.max(state.caret.min(state.anchor));

        let last = state.caret.max(state.anchor);

        let first = edit_unit_index(&units, first);

        let last = edit_unit_index(&units, last);

        let run = String::from_utf16_lossy(&units[first.min(last)..last]);

        let mut live = crate::material::unpack(color);

        for channel in &mut live[..3] {
            *channel = 1.0 - *channel;
        }

        if !run.is_empty() && color >> 24 != 0 {
            list.commands.push(Command::Text(TextDraw {
                owner,
                text: run,
                rect,
                indent: 0,
                font,
                align,
                color: crate::material::pack(live),
                shadow: ShadowType::None,
                shadow_color: element.shadow_color,
                bold: false,
            }));
        }
    }

    if let Some(owner) = clock_owner {
        list.commands.push(Command::CaretTick { owner, focused });
    }

    if focused && state.caret_visible && !state.caret_suppressed {
        let centred = align & font_align::CENTER != 0;

        let x = caret_x - scroll_width + rects.text[0];

        let mut rect = [x, rects.text[1], x + CARET_WIDTH, rects.text[3]];

        if centred {
            rect[0] -= 2;

            rect[2] -= 2;
        }

        if align & font_align::RIGHT != 0 {
            let left = rects.text[2] - caret_x;

            rect = [left, rects.text[1], left + CARET_WIDTH, rects.text[3]];
        }

        if !state.insert {
            let width = prefix_width(&units, font, state.caret, measure);

            if centred {
                let centre =
                    rects.text[0] + ((rects.text[2] - rects.text[0]) / 2 - whole_width / 2) + width;

                rect = [centre - 2, rects.text[1], centre + 2, rects.text[3]];
            } else {
                rect[2] = width - scroll_width + rects.text[0];
            }
        }

        list.commands
            .push(Command::Fill(FillDraw { owner, rect, color }));
    }
}

pub const CARET_WIDTH: i32 = 2;

const WHOLE_STRING: i32 = -1;

fn prefix_width(units: &[u16], font: i32, index: i32, measure: &mut dyn Measure) -> i32 {
    let n = if index == WHOLE_STRING {
        units.len()
    } else if index < 0 {
        return 0;
    } else {
        edit_unit_index(units, index)
    };

    units[..n]
        .iter()
        .map(|&unit| measure.advance(font, unit).unwrap_or(0))
        .sum()
}

fn edit_unit_index(units: &[u16], index: i32) -> usize {
    let mut at = 0;

    for _ in 0..index.max(0) as usize {
        let Some(&first) = units.get(at) else {
            break;
        };

        at += if (0xd800..=0xdbff).contains(&first)
            && units
                .get(at + 1)
                .is_some_and(|v| (0xdc00..=0xdfff).contains(v))
        {
            2
        } else {
            1
        };
    }

    at
}

fn intersect(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    let out = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];

    if out[0] < out[2] && out[1] < out[3] {
        out
    } else {
        [0; 4]
    }
}

fn push_image(owner: Owner, element: &Element, list: &mut DrawList) {
    let Some(material) = element.layers.first() else {
        return;
    };

    let state = element.widget.image();

    if state.is_some_and(|s| s.use_ani) {
        list.commands.push(Command::ImageTick { owner });
    }

    if let Some(state) = state.filter(|s| !s.external_image_file.is_empty() || s.use_rotation) {
        list.commands.push(Command::Image(ImageDraw {
            owner,
            material: *material,
            rect: element.rect(),
            state: state.clone(),
        }));

        return;
    }

    let mut material = *material;

    if let Some(cell) = state
        .filter(|s| s.use_ani)
        .and_then(|s| s.cell(material.uv))
    {
        material.uv = cell;
    }

    push_layer(owner, &material, material.raster_op, element.rect(), list);
}

fn push_gauge(owner: Owner, element: &Element, list: &mut DrawList) {
    let Some(state) = element.widget.gauge() else {
        return;
    };

    let rect = element.rect();

    let inner = state.inner(rect);

    let fill = state.fill(rect);

    if let Some(background) = element.layers.first() {
        push_layer(owner, background, background.raster_op, inner, list);
    }

    if let Some(bar) = element.layers.get(1) {
        let (uv, raster_op) = gauge_fill_source(state, bar, inner, fill);

        let mut bar = *bar;

        bar.uv = uv;

        push_layer(owner, &bar, raster_op, fill, list);
    }

    if let Some(frame) = element.layers.get(2).filter(|_| !state.no_frame) {
        push_layer(owner, frame, frame.raster_op, rect, list);
    }

    if element.display.is_empty() {
        return;
    }

    let Some(layer) = element.layers.first() else {
        return;
    };

    let mut colors = layer.texture_color;

    colors.select(State::Normal);

    let [left, top, right, bottom] = rect;

    let [dx, dy] = layer.font_offset;

    list.commands.push(Command::Text(TextDraw {
        owner,
        text: element.display.clone(),
        rect: [left + dx, top + dy, right + dx, bottom + dy],
        indent: 0,
        font: layer.font_index,
        align: font_align::CENTER | font_align::VCENTER,
        color: colors.live_argb(),
        shadow: element.shadow_type,
        shadow_color: element.shadow_color,
        bold: false,
    }));
}

fn gauge_fill_source(
    state: &crate::widget::GaugeState,
    bar: &Material,
    inner: [i32; 4],
    fill: [i32; 4],
) -> ([i32; 4], RasterOp) {
    let uv_width = bar.uv[2] - bar.uv[0];

    let [min, max] = state.range;

    if uv_width <= 0 || state.position <= min || max <= min {
        return (bar.uv, bar.raster_op);
    }

    let fill_width = fill[2] - fill[0];

    let inner_width = inner[2] - inner[0];

    match state.fill_draw_method {
        1 => {
            let scaled =
                (f64::from(fill_width) / f64::from(inner_width) * f64::from(uv_width)) as f32;

            let cropped = (f64::from(scaled) + 0.5) as i32;

            let cropped = if cropped <= 0 { 1 } else { cropped };

            (
                [bar.uv[0], bar.uv[1], bar.uv[0] + cropped, bar.uv[3]],
                bar.raster_op,
            )
        }
        2 if uv_width <= fill_width => (bar.uv, RasterOp::Split),
        2 => (
            [bar.uv[0], bar.uv[1], bar.uv[0] + fill_width, bar.uv[3]],
            RasterOp::HStretchSplit,
        ),
        _ => (bar.uv, bar.raster_op),
    }
}

fn push_check_box(owner: Owner, element: &Element, interaction: &Interaction, list: &mut DrawList) {
    let (state, _) = interaction.state_of(owner, element);

    let [left, top, right, bottom] = element.rect();

    let box_rect = [left, top, left + (bottom - top), bottom];

    let text_rect = [
        left + (f64::from(bottom - top) * 1.25) as i32,
        top,
        right,
        bottom,
    ];

    let Some(face) = element.layers.first() else {
        return;
    };

    let mut face = *face;

    face.texture_color.select(state);

    face.font_color.select(state);

    push_layer(owner, &face, RasterOp::Stretch, box_rect, list);

    push_text(owner, element, &face, text_rect, &element.display, list);

    let checked = element.widget.button().is_some_and(|b| b.checked);

    if let Some(tick) = element.layers.get(1).filter(|_| checked) {
        let mut tick = *tick;

        tick.texture_color.select(state);

        push_layer(owner, &tick, RasterOp::Stretch, box_rect, list);
    }
}

fn push_button(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    measure: &mut dyn Measure,
    list: &mut DrawList,
) {
    let (state, nudge) = interaction.state_of(owner, element);

    let Some(material) = button_layer(element, state) else {
        return;
    };

    let mut material = *material;

    material.texture_color.select(state);

    material.font_color.select(state);

    let face = if state == State::Normal
        && element
            .widget
            .button()
            .is_some_and(|b| b.pulse_enabled && b.pulse_frequency > 0.0)
    {
        element.layers.get(2).unwrap_or(&material)
    } else {
        &material
    };

    push_layer(owner, face, face.raster_op, element.rect(), list);

    let [left, top, right, bottom] = element.rect();

    let [inset_left, inset_top, inset_right, _] = element.text_insets;

    let mut rect = [
        left + inset_left,
        top + inset_top + nudge,
        right + inset_left,
        bottom + inset_top + nudge,
    ];

    if element.widget.button().is_some_and(|b| b.icon) {
        rect[0] += rect[3] - rect[1];
    }

    rect[0] += material.font_offset[0];

    rect[2] += material.font_offset[0];

    rect[1] += material.font_offset[1];

    rect[3] += material.font_offset[1];

    if element.has_text_layout() {
        push_rich_caption(
            owner,
            element,
            &material,
            rect,
            element.width - inset_left - inset_right,
            measure,
            list,
        );
    } else {
        push_text(owner, element, &material, rect, &element.display, list);
    }
}

fn push_icon_button(owner: Owner, element: &Element, list: &mut DrawList) {
    let start = list.commands.len();

    push_icon_button_contents(owner, element, list);

    let Some(button) = element.widget.button() else {
        return;
    };

    let Some(path) = &button.icon_texture else {
        return;
    };

    let kind = button.icon_contents.map_or(0, |c| c.kind);

    if button.icon_contents.is_none() {
        list.commands.push(Command::NamedIcon(NamedIconDraw {
            owner,
            path: path.clone(),
            secondary_path: None,
            uv: button.icon_texture_uv,
            rect: element.rect(),
            color: if element.enabled {
                0xffff_ffff
            } else {
                0xff7c_7c7c
            },
        }));
    } else {
        for command in &mut list.commands[start..] {
            let Command::Icon(icon) = command else {
                continue;
            };

            let secondary_path = (kind == crate::icon::AFFECT_KIND
                || icon.sheet == crate::icon::SKILL_SHEET_ON)
                .then_some("UI/NewAmulOn.wyt");

            *command = Command::NamedIcon(NamedIconDraw {
                owner,
                path: path.clone(),
                secondary_path,
                uv: button.icon_texture_uv,
                rect: icon.rect,
                color: icon.color,
            });
        }
    }

    list.commands.insert(
        start,
        Command::IconScopePush {
            owner,
            path: path.clone(),
            secondary_path: matches!(kind, crate::icon::SKILL_KIND | crate::icon::AFFECT_KIND)
                .then_some("UI/NewAmulOn.wyt"),
        },
    );

    list.commands.push(Command::IconScopePop { owner });
}

fn push_icon_button_contents(owner: Owner, element: &Element, list: &mut DrawList) {
    let Some(button) = element.widget.button() else {
        return;
    };

    let Some(contents) = button.icon_contents else {
        return;
    };

    if !matches!(contents.kind, 1 | 3 | 4 | 7) {
        list.commands.push(Command::Icon(IconDraw {
            owner,
            sheet: contents.sheet,
            uv: contents.sheet_uv,
            rect: element.rect(),
            color: if element.enabled {
                0xffff_ffff
            } else {
                0xff7c_7c7c
            },
        }));

        return;
    }

    if contents.kind == crate::icon::AFFECT_KIND {
        push_affect_icon(owner, &contents, element, list);

        return;
    }

    if contents.kind == crate::icon::SKILL_KIND {
        push_skill_icon(owner, &contents, element, list);

        return;
    }

    push_icon(
        owner,
        &contents,
        element.rect(),
        0xFFFF_FFFF,
        LabelBox::Cell {
            staged: button.icon_locked && matches!(contents.kind, 1 | 7),
            collection: button.collection.filter(|_| matches!(contents.kind, 1 | 7)),
        },
        (element.shadow_type, element.shadow_color),
        list,
    );

    if let Some(fade) = button.pulse_fade {
        push_sweep(
            owner,
            element.rect(),
            (1.0 - fade).fract() * crate::icon::SWEEP_TURN,
            list,
        );
    }
}

fn push_icon_highlight(
    owner: Owner,
    element: &Element,
    interaction: &Interaction,
    list: &mut DrawList,
) {
    let Some(button) = element.widget.button() else {
        return;
    };

    let pressed = interaction.pressed == Some(owner)
        || (button.toggle || !button.group.is_empty()) && button.checked;

    if interaction.hovered != Some(owner) && !button.pulse_highlight && !pressed {
        return;
    }

    push_button_highlight(owner, element, pressed, list);
}

fn push_button_highlight(owner: Owner, element: &Element, pressed: bool, list: &mut DrawList) {
    let Some(button) = element.widget.button() else {
        return;
    };

    let Some(mut material) = element.layers.get(2).copied() else {
        return;
    };

    if material.texture_index < 0 {
        return;
    }

    let state = if !element.enabled {
        State::Disabled
    } else if pressed {
        State::Pressed
    } else {
        State::Normal
    };

    material.texture_color.select(state);

    if !(button.pulse_enabled && button.pulse_frequency != 0.0 && state == State::Normal) {
        material.texture_color.live[3] = 1.0;
    }

    if material.texture_color.live[3] == 0.0 {
        return;
    }

    list.commands.push(Command::Layer(LayerDraw {
        owner,
        texture_index: material.texture_index,
        uv: material.uv,
        rect: element.rect(),
        color: material.texture_color.live_argb(),
        raster_op: material.raster_op,
        additive: true,
    }));
}

fn push_sweep(owner: Owner, rect: [i32; 4], angle: f32, list: &mut DrawList) {
    if angle > 0.0 {
        list.commands
            .push(Command::Sweep(SweepDraw { owner, rect, angle }));
    }
}

fn push_affect_icon(
    owner: Owner,
    contents: &crate::icon::IconContents,
    element: &Element,
    list: &mut DrawList,
) {
    if contents.index <= 0 {
        return;
    }

    let rect = element.rect();

    let color = match (contents.blink_off, element.enabled) {
        (true, _) => formats::net::affect::BLINK_COLOUR,
        (false, true) => formats::net::affect::ENABLED_COLOUR,
        (false, false) => formats::net::affect::DISABLED_COLOUR,
    };

    list.commands.push(Command::Icon(IconDraw {
        owner,
        sheet: contents.sheet,
        uv: contents.sheet_uv,
        rect,
        color,
    }));

    if element
        .widget
        .button()
        .is_some_and(|state| state.icon_slot_index == -1)
    {
        return;
    }

    let Some(text) = element
        .widget
        .button()
        .and_then(|state| state.affect_countdown.clone())
        .or_else(|| contents.countdown())
        .filter(|text| !text.is_empty())
    else {
        return;
    };

    let half = (rect[3] - rect[1]).div_euclid(2);

    list.commands.push(Command::Text(TextDraw {
        owner,
        text,
        rect: [
            rect[0] + 1,
            rect[3] - half + 1,
            rect[2] + 0x12,
            rect[3] + 6 + half,
        ],
        indent: 0,
        font: 1,
        align: font_align::VCENTER,
        color: formats::net::affect::countdown_colour(
            usize::try_from(contents.count).unwrap_or(usize::MAX),
            contents.refine,
        ),
        shadow: element.shadow_type,
        shadow_color: element.shadow_color,
        bold: false,
    }));
}

fn push_skill_icon(
    owner: Owner,
    contents: &crate::icon::IconContents,
    element: &Element,
    list: &mut DrawList,
) {
    let armed = element.widget.button().is_some_and(|b| b.checked);

    let sheet = match armed {
        true => crate::icon::SKILL_SHEET_ON,
        false => contents.sheet,
    };

    list.commands.push(Command::Icon(IconDraw {
        owner,
        sheet,
        uv: contents.sheet_uv,
        rect: element.rect(),
        color: match element.enabled {
            true => 0xFFFF_FFFF,
            false => 0x44FF_FFFF,
        },
    }));

    if let Some(angle) = contents.sweep_angle()
        && angle > 0.0
    {
        push_sweep(owner, element.rect(), angle, list);
    }
}

pub const SWEEP_COLOR: u32 = 0x8800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelBox {
    Cell {
        staged: bool,
        collection: Option<crate::icon::CollectionIcon>,
    },
    Carried,
}

#[allow(clippy::too_many_arguments)]
fn push_icon(
    owner: Owner,
    contents: &crate::icon::IconContents,
    rect: [i32; 4],
    color: u32,
    label_box: LabelBox,
    shadow: (ShadowType, u32),
    list: &mut DrawList,
) {
    list.commands.push(Command::Icon(IconDraw {
        owner,
        sheet: contents.sheet,
        uv: contents.sheet_uv,
        rect,
        color,
    }));

    for alias in [Some(contents.grade), contents.overlay]
        .into_iter()
        .flatten()
    {
        list.commands.push(Command::Layer(LayerDraw {
            owner,
            texture_index: alias.texture_index,
            uv: alias.uv,
            rect,
            color,
            raster_op: RasterOp::Stretch,
            additive: false,
        }));
    }

    if matches!(label_box, LabelBox::Cell { staged: true, .. }) {
        list.commands.push(Command::Fill(FillDraw {
            owner,
            rect,
            color: 0x32FF_0000,
        }));
    }

    let mut label = contents.label();

    if let LabelBox::Cell {
        collection: Some(collection),
        ..
    } = label_box
    {
        if collection.state == 1 {
            list.commands.push(Command::Fill(FillDraw {
                owner,
                rect,
                color: 0x8010_1010,
            }));
        } else if let Some(alias) = collection.mark {
            list.commands.push(Command::Layer(LayerDraw {
                owner,
                texture_index: alias.texture_index,
                uv: alias.uv,
                rect,
                color: 0xFFFF_FFFF,
                raster_op: RasterOp::Stretch,
                additive: false,
            }));
        }

        if collection.state != -1 && contents.item.effects[1].id != 0 {
            label = Some((format!("Lv {}", contents.item.effects[1].id), 0xFFFF_FF55));
        }
    }

    let Some((text, label_color)) = label else {
        return;
    };

    let (width, height) = (rect[2] - rect[0], rect[3] - rect[1]);

    let quarter = height.div_euclid(4);

    let overhang = match label_box {
        LabelBox::Cell { .. } => width.div_euclid(16),
        LabelBox::Carried => width.div_euclid(8),
    };

    let label_rect = [
        rect[0],
        rect[3] - quarter - 1,
        rect[2] + overhang,
        rect[3] + quarter,
    ];

    list.commands.push(Command::Text(TextDraw {
        owner,
        text: text.clone(),
        rect: label_rect,
        indent: 0,
        font: if text.chars().count() > LABEL_LONG {
            1
        } else {
            2
        },
        align: font_align::RIGHT,
        color: label_color,
        shadow: shadow.0,
        shadow_color: shadow.1,
        bold: false,
    }));
}

pub fn carried_draws(carried: &crate::carry::Carried, cursor: (i32, i32)) -> DrawList {
    let mut list = DrawList::default();

    if carried.contents.sheet == 0 {
        return list;
    }

    if carried.kind != crate::carry::ITEM_KIND {
        list.commands.push(Command::Icon(IconDraw {
            owner: Owner::Carried,
            sheet: carried.contents.sheet,
            uv: carried.contents.sheet_uv,
            rect: carried.rect(cursor),
            color: crate::carry::CARRY_COLOR,
        }));

        return list;
    }

    push_icon(
        Owner::Carried,
        &carried.contents,
        carried.rect(cursor),
        crate::carry::CARRY_COLOR,
        LabelBox::Carried,
        (ShadowType::None, 0),
        &mut list,
    );

    if let Some(Command::Text(label)) = list.commands.pop() {
        let shifted = [
            label.rect[0] + 1,
            label.rect[1] + 1,
            label.rect[2] + 1,
            label.rect[3] + 1,
        ];

        list.commands.push(Command::Text(TextDraw {
            text: label.text.clone(),
            rect: shifted,
            color: 0xFF00_0000,
            ..label.clone()
        }));

        list.commands.push(Command::Text(label));
    }

    list
}

pub const LABEL_LONG: usize = 4;

fn button_layer(element: &Element, state: State) -> Option<&Material> {
    let index = match state {
        State::Disabled if element.layers.len() < 4 => 0,
        other => other as usize,
    };

    element.layers.get(index)
}

fn push_layer(
    owner: Owner,
    material: &Material,
    raster_op: RasterOp,
    rect: [i32; 4],
    list: &mut DrawList,
) {
    if material.texture_color.live[3] == 0.0 {
        return;
    }

    list.commands.push(Command::Layer(LayerDraw {
        owner,
        texture_index: material.texture_index,
        uv: material.uv,
        rect,
        color: material.texture_color.live_argb(),
        raster_op,
        additive: false,
    }));
}

fn push_text(
    owner: Owner,
    element: &Element,
    material: &Material,
    rect: [i32; 4],
    text: &str,
    list: &mut DrawList,
) {
    if text.is_empty() || material.font_color.live[3] == 0.0 {
        return;
    }

    list.commands.push(Command::Text(TextDraw {
        owner,
        text: text.to_string(),
        rect,
        indent: 0,
        font: material.font_index,
        align: material.font_align,
        color: material.font_color.live_argb(),
        shadow: element.shadow_type,
        shadow_color: element.shadow_color,
        bold: false,
    }));
}
