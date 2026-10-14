use crate::material::{ColorSet, DEFAULT_DISABLED, Material, State};
use formats::ui::guimat::{
    Alias, Binding, FontDef, FontLayer, GuiMat, HAlign, Section, TextureDef, TextureLayer, VAlign,
};
use formats::ui::pane::{ElementType, font_align};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelMaterials {
    pub caption: Material,
    pub close_normal: Material,
    pub close_pressed: Material,
    pub close_mouseover: Material,
}

impl Default for PanelMaterials {
    fn default() -> PanelMaterials {
        PanelMaterials {
            caption: Material::seed(),
            close_normal: Material::seed(),
            close_pressed: Material::seed(),
            close_mouseover: Material::seed(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeMaterial {
    pub element: ElementType,
    pub layer: u32,
    pub material: Material,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resource {
    pub fonts: Vec<FontDef>,
    pub textures: Vec<TextureDef>,
    pub aliases: Vec<Alias>,
    pub type_materials: Vec<TypeMaterial>,
    pub panel: PanelMaterials,
    pub titlebar_height: i32,
    pub close_button_location: [i32; 4],
    pub warnings: Vec<String>,
}

impl Resource {
    pub fn build(guimat: &GuiMat) -> Resource {
        let mut resource = Resource {
            fonts: guimat.fonts.clone(),
            textures: guimat.textures.clone(),
            aliases: guimat.aliases.clone(),
            warnings: guimat.warnings.clone(),
            ..Resource::default()
        };

        for section in &guimat.sections {
            let material = resource.material_for(section);

            match section.binding {
                Some(Binding::Panel(slot)) => {
                    match slot {
                        0 => resource.panel.caption = material,
                        1 => resource.panel.close_normal = material,
                        2 => resource.panel.close_pressed = material,
                        _ => resource.panel.close_mouseover = material,
                    }

                    if let Some(height) = section.titlebar_height {
                        resource.titlebar_height = height;
                    }

                    if let Some(location) = section.close_button_location {
                        resource.close_button_location = location;
                    }
                }
                Some(Binding::Element { element, layer }) => {
                    resource.set_type_material(element, layer, material);

                    if element == ElementType::EditBox {
                        resource.set_type_material(
                            formats::ui::guimat::EDITBOX_ALSO_BINDS_IME,
                            layer,
                            material,
                        );
                    }
                }
                None => {}
            }
        }

        resource
    }

    fn set_type_material(&mut self, element: ElementType, layer: u32, material: Material) {
        match self
            .type_materials
            .iter_mut()
            .find(|t| t.element == element && t.layer == layer)
        {
            Some(existing) => existing.material = material,
            None => self.type_materials.push(TypeMaterial {
                element,
                layer,
                material,
            }),
        }
    }

    fn material_for(&self, section: &Section) -> Material {
        let mut material = Material::seed();

        if let Some(texture) = &section.texture {
            self.apply_texture(&mut material, texture);
        }

        if let Some(font) = &section.font {
            apply_font(&mut material, font);
        }

        material
    }

    fn apply_texture(&self, material: &mut Material, texture: &TextureLayer) {
        if let Some(index) = texture.index {
            material.set_texture_index(index);
        }

        if let Some(uv) = texture.uv {
            material.uv = uv;
        }

        if let Some(name) = &texture.alias {
            let (index, uv) = self.alias_rect(name);

            material.texture_index = index;

            material.uv = uv;
        }

        if let Some(raster_op) = texture.raster_op {
            material.raster_op = raster_op;
        }

        apply_colors(&mut material.texture_color, &texture.colors);
    }

    pub fn alias_rect(&self, name: &str) -> (i32, [i32; 4]) {
        match self.alias(name) {
            Some(alias) => (alias.texture_index, alias.rect),
            None => (0, [0; 4]),
        }
    }

    pub fn alias(&self, name: &str) -> Option<&Alias> {
        self.aliases
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    pub fn texture(&self, index: i32) -> Option<&TextureDef> {
        self.textures.iter().find(|t| t.index == index)
    }

    pub fn font(&self, index: i32) -> Option<&FontDef> {
        self.fonts.iter().find(|f| f.index == index)
    }

    pub fn layers(&self, element: ElementType) -> impl Iterator<Item = &TypeMaterial> {
        let source = material_source(element);

        self.type_materials
            .iter()
            .filter(move |t| t.element == source)
    }
}

pub fn material_source(element: ElementType) -> ElementType {
    match element {
        ElementType::ListView
        | ElementType::TreeView
        | ElementType::ImeNotePad
        | ElementType::CustomListView => ElementType::ListBox,
        ElementType::ImeNotePadEditBox => ElementType::ImeEditBox,
        other => other,
    }
}

fn apply_font(material: &mut Material, font: &FontLayer) {
    if let Some(index) = font.index {
        material.set_font_index(index);
    }

    if let Some(halign) = font.halign {
        material.font_align &= !(font_align::CENTER | font_align::RIGHT);

        material.font_align |= match halign {
            HAlign::Left => 0,
            HAlign::Center => font_align::CENTER,
            HAlign::Right => font_align::RIGHT,
        };
    }

    if let Some(valign) = font.valign {
        material.font_align &= !(font_align::VCENTER | font_align::BOTTOM);

        material.font_align |= match valign {
            VAlign::Top => 0,
            VAlign::Center => font_align::VCENTER,
            VAlign::Bottom => font_align::BOTTOM,
        };
    }

    if font.wordbreak {
        material.font_align |= font_align::WORDBREAK;
    }

    if let Some(offset) = font.offset {
        material.font_offset = offset;
    }

    apply_colors(&mut material.font_color, &font.colors);
}

fn apply_colors(colors: &mut ColorSet, source: &formats::ui::guimat::Colors) {
    if let Some(argb) = source.default {
        *colors = ColorSet::init(argb, DEFAULT_DISABLED);
    }

    for (state, value) in [
        (State::Normal, source.normal),
        (State::Pressed, source.pressed),
        (State::MouseOver, source.mouseover),
        (State::Disabled, source.disabled),
    ] {
        if let Some(argb) = value {
            colors.set(state, argb);
        }
    }
}
