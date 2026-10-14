use crate::fonts::Chains;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::ui::pane::{RasterOp, ShadowType};
use formats::ui::{guimat, pane};
use render::font::Face;
use render::font::table::{self, FontTable, TextCache};
use render::sprite::{Quad, UvRule};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use ui::draw::{Command, Interaction, LayerDraw, Owner, TextDraw};
use ui::{Panel, Resource, Strings};

pub(crate) fn read_language(
    game: &GameDir,
    name: &str,
) -> std::io::Result<(Vec<u8>, formats::text::Codepage)> {
    use formats::{config::Language, text::Codepage};

    let selected = format!("Lang/{}/{name}", game.language().dir());

    if game.contains(selected.as_bytes()) {
        return game
            .read(selected.as_bytes())
            .map(|bytes| (bytes, Codepage::for_language(game.language())));
    }

    game.read(format!("Lang/EN/{name}").as_bytes())
        .map(|bytes| (bytes, Codepage::for_language(Language::English)))
}

pub fn strings(game: &GameDir) -> Strings {
    use formats::text::strdef;

    Strings {
        ui: read_language(game, "UIString.txt")
            .ok()
            .map(|(bytes, cp)| ui::text::ui_strings(&bytes, cp)),
        strdef: read_language(game, "strdef.bin")
            .ok()
            .map(|(bytes, cp)| strdef::parse(&bytes, cp)),
        item_help: load_item_help(game).ok().map(Into::into),
        files: Some(std::sync::Arc::new(LanguageText {
            game: game.clone(),
            cache: std::sync::Mutex::new(HashMap::new()),
        })),
    }
}

#[derive(Debug)]

struct LanguageText {
    game: GameDir,
    cache: std::sync::Mutex<HashMap<String, Option<String>>>,
}

impl ui::text::FileText for LanguageText {
    fn text(&self, name: &str) -> Option<String> {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        cache
            .entry(name.to_owned())
            .or_insert_with(|| match read_language(&self.game, name) {
                Ok((bytes, cp)) => Some(ui::text::decode_colored_text_records(&bytes, cp)),
                Err(error) => {
                    tracing::warn!(name,% error, "caption language file unavailable");

                    None
                }
            })
            .clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Source {
    Texture(i32),
    Caption(Owner, u32),
    ItemIcon(i32),
    NamedIcon(Owner),
    Amul(crate::frame::AmulSheet),
    Solid,
}

pub const FONT_ZOOM: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct PanelDraw {
    pub owner: Owner,
    pub source: Source,
    pub quad: Quad,
    pub uv: UvRule,
    pub additive: bool,
}

impl PanelDraw {
    pub fn mode(&self) -> render::preset::RenderMode {
        if self.additive {
            render::preset::RenderMode::NuiHighlight2d
        } else {
            render::preset::RenderMode::NuiPanel2d
        }
    }
}

pub struct FontMeasure<'a> {
    fonts: &'a mut FontTable,
}

impl ui::layout::Measure for FontMeasure<'_> {
    fn advance(&mut self, font: i32, unit: u16) -> Option<i32> {
        Some(self.fonts.get(font)?.advance(unit))
    }

    fn height(&mut self, font: i32) -> Option<i32> {
        Some(self.fonts.get(font)?.size() as i32)
    }
}

#[derive(Debug)]

pub struct PanelUi {
    identity: u64,
    lifetime: std::sync::Arc<()>,
    game: Option<GameDir>,
    pub(crate) icon_images: BTreeMap<Owner, IconImage>,
    icon_files: BTreeMap<String, Option<std::sync::Arc<formats::texture::TextureResource>>>,
    icon_revision: usize,
    image_clock: Option<u32>,
    image_ticks: BTreeSet<Owner>,
    balloon_alpha_writes: BTreeMap<Owner, ui::element::Instance>,
    edit_alignment_writes: BTreeMap<Owner, ui::element::Instance>,
    edit_scroll_writes: BTreeMap<Owner, (ui::element::Instance, ui::draw::EditScrollAdmission)>,
    note_page_writes: BTreeMap<Owner, (ui::element::Instance, ui::notepad::PageAdmission)>,
    frame_time: Option<u32>,
    caret_ticks: BTreeMap<Owner, bool>,
    pub panel: Panel,
    pub resource: Resource,
    pub fonts: FontTable,
    pub cache: TextCache<(Owner, u32)>,
    pub notes: Vec<String>,
    pub scale: f32,
    text_layout_scale: f32,
}

#[derive(Debug)]

pub(crate) struct IconImage {
    pub path: String,
    primary: String,
    pub serial: usize,
    pub texture: Option<std::sync::Arc<formats::texture::TextureResource>>,
}

impl PanelUi {
    pub fn build(
        game: &GameDir,
        script: &[u8],
        strings: &Strings,
        chains: &Chains,
        scale: f32,
    ) -> Result<PanelUi> {
        let raw = game
            .read(script)
            .with_context(|| String::from_utf8_lossy(script).into_owned())?;

        let name = String::from_utf8_lossy(script).into_owned();

        let parsed = pane::load(&name, &raw).with_context(|| format!("parsing {name}"))?;

        let mut notes = Vec::new();

        let resource = match ui::panel::resource_path(&parsed) {
            Some(path) => {
                let bytes = game
                    .read(path.as_bytes())
                    .with_context(|| format!("{name} names {path}"))?;

                let guimat = guimat::parse(&bytes).with_context(|| format!("parsing {path}"))?;

                Resource::build(&guimat)
            }
            None => {
                notes.push(format!("{name} names no Resource"));

                Resource::default()
            }
        };

        let panel = Panel::instantiate(&parsed, &resource, strings, true);

        let mut ui = PanelUi::from_resource(panel, resource, chains, scale);

        ui.game = Some(game.clone());

        ui.resolve_templates(strings)?;

        notes.append(&mut ui.notes);

        ui.notes = notes;

        Ok(ui)
    }

    pub fn from_resource(
        mut panel: Panel,
        resource: Resource,
        chains: &Chains,
        scale: f32,
    ) -> PanelUi {
        panel.prepare_text_layouts();

        let chain0: Vec<_> = chains.chains[0].faces.clone();

        let chain1: Vec<_> = chains.chains[1].faces.clone();

        let fonts = FontTable::build(&resource.fonts, [&chain0, &chain1], FONT_ZOOM);

        let notes = fonts.notes().to_vec();

        PanelUi {
            text_layout_scale: scale,
            identity: {
                static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            lifetime: std::sync::Arc::new(()),
            panel,
            game: None,
            icon_images: BTreeMap::new(),
            icon_files: BTreeMap::new(),
            icon_revision: 0,
            image_clock: None,
            image_ticks: BTreeSet::new(),
            balloon_alpha_writes: BTreeMap::new(),
            edit_alignment_writes: BTreeMap::new(),
            edit_scroll_writes: BTreeMap::new(),
            note_page_writes: BTreeMap::new(),
            frame_time: None,
            caret_ticks: BTreeMap::new(),
            resource,
            fonts,
            cache: TextCache::new(),
            notes,
            scale,
        }
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub(crate) fn lifetime(&self) -> std::sync::Weak<()> {
        std::sync::Arc::downgrade(&self.lifetime)
    }

    pub fn relocalize(&mut self, game: &GameDir, strings: &Strings) {
        self.panel.relocalize(strings);

        self.game = Some(game.clone());

        let keys: Vec<_> = self.cache.iter().map(|(key, _)| *key).collect();

        for key in keys {
            self.cache.forget(&key);
        }
    }

    pub fn resolve_templates(&mut self, strings: &Strings) -> Result<()> {
        let game = self
            .game
            .as_ref()
            .context("ItemPanel resolution needs an app-owned GameDir")?;

        let mut active = Vec::new();

        let mut remaining = 4096;

        for element in &mut self.panel.elements {
            resolve_item_panels(
                game,
                element,
                &self.resource,
                strings,
                &mut active,
                &mut remaining,
            )?;
        }

        Ok(())
    }

    pub fn apply_element_property(
        &mut self,
        owner: Owner,
        property: &pane::Property,
        strings: &Strings,
    ) -> Result<()> {
        let element = ui::input::retained_element_mut(&mut self.panel, owner)
            .context("property target is no longer retained")?;

        element.apply(property, &self.resource, strings);

        if property.base_key().eq_ignore_ascii_case("ItemPanel") && element.widget.list().is_some()
        {
            let game = self
                .game
                .as_ref()
                .context("ItemPanel resolution needs an app-owned GameDir")?;

            resolve_item_panels(
                game,
                element,
                &self.resource,
                strings,
                &mut Vec::new(),
                &mut 4096,
            )?;
        }

        Ok(())
    }

    pub fn append_note_row(
        &mut self,
        owner: Owner,
        text: &str,
        style: ui::notepad::RowStyle,
    ) -> Result<usize> {
        let element = ui::input::retained_element_mut(&mut self.panel, owner)
            .context("note row target is no longer retained")?;

        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        ui::notepad::append_row(element, text, style, &mut measure).context(
            "note row append needs a note-pad owner, font metrics and a nonempty text area",
        )
    }

    pub fn set_icon_source(&mut self, game: &GameDir) {
        self.game = Some(game.clone());

        self.icon_images.clear();

        self.icon_files.clear();
    }

    fn icon_file(
        &mut self,
        path: &str,
    ) -> Option<std::sync::Arc<formats::texture::TextureResource>> {
        let key = icon_file_key(path);

        if !self.icon_files.contains_key(&key) {
            let _ = self.reload_icon_resource(path, 0);
        }

        self.icon_files.get(&key).cloned().flatten()
    }

    pub fn reload_icon_resource(&mut self, path: &str, color_key: u32) -> Result<()> {
        let key = icon_file_key(path);

        self.icon_images.retain(|_, image| {
            icon_file_key(&image.path) != key && icon_file_key(&image.primary) != key
        });

        let result = self
            .game
            .as_ref()
            .context("named image resolver has no game directory")
            .and_then(|game| game.read(path.as_bytes()).map_err(anyhow::Error::from))
            .and_then(|bytes| {
                formats::texture::decode_resource_with_color_key(&bytes, color_key)
                    .map_err(anyhow::Error::from)
            });

        match result {
            Ok(texture) => {
                self.icon_files
                    .insert(key, Some(std::sync::Arc::new(texture)));

                Ok(())
            }
            Err(error) => {
                self.icon_files.insert(key, None);

                tracing::warn!(path,% error, "named icon texture unavailable");

                self.notes.push(format!("named icon unavailable: {path}"));

                Err(error)
            }
        }
    }

    fn named_icon_draw(
        &mut self,
        icon: &ui::draw::NamedIconDraw,
        origin: (i32, i32),
    ) -> Option<PanelDraw> {
        let primary = self.icon_file(&icon.path);

        let path = icon.secondary_path.unwrap_or(&icon.path);

        if self
            .icon_images
            .get(&icon.owner)
            .is_none_or(|old| old.path != path || old.primary != icon.path)
        {
            self.icon_revision += 1;

            let texture = primary.as_ref().and_then(|_| self.icon_file(path));

            self.icon_images.insert(
                icon.owner,
                IconImage {
                    path: path.to_owned(),
                    primary: icon.path.clone(),
                    serial: self.icon_revision,
                    texture,
                },
            );
        }

        self.icon_images.get(&icon.owner)?.texture.as_ref()?;

        let primary = primary?;

        let uv = icon
            .uv
            .unwrap_or([0, 0, primary.width() as i32, primary.height() as i32]);

        let mut draw = icon_draw(
            &ui::draw::IconDraw {
                owner: icon.owner,
                sheet: 0,
                uv,
                rect: icon.rect,
                color: icon.color,
            },
            origin,
        );

        draw.source = Source::NamedIcon(icon.owner);

        Some(draw)
    }

    fn image_draw(&mut self, image: &ui::draw::ImageDraw, origin: (i32, i32)) -> Vec<PanelDraw> {
        let path = &image.state.external_image_file;

        let resolved = if !path.contains('.') {
            None
        } else if self
            .game
            .as_ref()
            .is_none_or(|game| game.contains(path.as_bytes()))
        {
            Some(path.as_str())
        } else {
            Some("UI/Image/DEFAULT.WYP")
        };

        let texture = resolved.and_then(|path| self.icon_file(path));

        if let Some(texture) = texture {
            let mut uv = [0, 0, texture.width() as i32, texture.height() as i32];

            if image.state.use_ani {
                uv = image.state.cell(uv).unwrap_or(uv);
            } else if image.state.use_external_uv {
                uv = image.material.uv;
            }

            let icon = ui::draw::NamedIconDraw {
                owner: image.owner,
                path: resolved.unwrap().into(),
                secondary_path: None,
                uv: Some(uv),
                rect: image.rect,
                color: image.material.texture_color.normal,
            };

            return self.named_icon_draw(&icon, origin).into_iter().collect();
        }

        self.icon_images.remove(&image.owner);

        if image.state.use_rotation {
            return rotated_image_draw(image, origin).into_iter().collect();
        }

        let uv = if image.state.use_ani {
            image
                .state
                .cell(image.material.uv)
                .unwrap_or(image.material.uv)
        } else {
            image.material.uv
        };

        if image.material.texture_color.live[3] == 0.0 {
            return Vec::new();
        }

        layer_draw(
            &LayerDraw {
                owner: image.owner,
                texture_index: image.material.texture_index,
                uv,
                rect: image.rect,
                color: image.material.texture_color.live_argb(),
                raster_op: image.material.raster_op,
                additive: false,
            },
            origin,
        )
    }

    pub fn draw_list(&mut self, interaction: &Interaction) -> ui::DrawList {
        ui::image_font::resolve_copies(&mut self.panel, &self.resource);

        if self.text_layout_scale != self.scale {
            self.panel.refresh_text_scale();

            self.text_layout_scale = self.scale;
        }

        let interaction = Interaction {
            now_ms: self.frame_time,
            ..*interaction
        };

        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        self.panel.refresh_list_box_metrics(&mut measure);

        ui::notepad::refresh_panel(&mut self.panel, &mut measure);

        self.panel.draw_list_with(&interaction, &mut measure)
    }

    pub fn refresh_input_layout(&mut self) {
        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        self.panel.refresh_list_box_metrics(&mut measure);

        ui::notepad::refresh_panel(&mut self.panel, &mut measure);
    }

    pub fn frame(&mut self, interaction: &Interaction, origin: (i32, i32)) -> Vec<PanelDraw> {
        let list = self.draw_list(interaction);

        self.frame_of(&list, origin)
    }

    pub fn frame_of(&mut self, list: &ui::DrawList, origin: (i32, i32)) -> Vec<PanelDraw> {
        let mut out = Vec::with_capacity(list.commands.len());

        let first = out.len();

        let mut runs: HashMap<Owner, u32> = HashMap::new();

        let mut clips: Vec<[f32; 4]> = Vec::new();

        let mut icon_scopes: Vec<bool> = Vec::new();

        for command in &list.commands {
            let start = out.len();

            match command {
                Command::IconScopePush {
                    path,
                    secondary_path,
                    ..
                } => {
                    let ready = icon_scopes.last().copied().unwrap_or(true)
                        && self.icon_file(path).is_some()
                        && secondary_path.is_none_or(|path| self.icon_file(path).is_some());

                    icon_scopes.push(ready);

                    continue;
                }
                Command::IconScopePop { .. } => {
                    icon_scopes.pop();

                    continue;
                }
                _ => {}
            }

            if icon_scopes.last() == Some(&false) {
                continue;
            }

            match command {
                Command::Layer(layer) => {
                    if let Some(element) = ui::input::retained_element(&self.panel, layer.owner)
                        && element.widget.balloon().is_some_and(|b| b.flat)
                    {
                        self.balloon_alpha_writes
                            .insert(layer.owner, element.instance);
                    }

                    out.extend(layer_draw(layer, origin));
                }
                Command::Text(text) => {
                    let run = runs.entry(text.owner).or_default();

                    let index = *run;

                    *run += 1;

                    self.push_text(text, index, origin, &mut out);
                }
                Command::Fill(fill) => out.push(fill_draw(fill, origin)),
                Command::Sweep(sweep) => {
                    let template = fill_draw(
                        &ui::draw::FillDraw {
                            owner: sweep.owner,
                            rect: sweep.rect,
                            color: ui::draw::SWEEP_COLOR,
                        },
                        origin,
                    );

                    out.extend(sweep.triangles().into_iter().map(|[a, b, c]| {
                        let mut draw = template;

                        draw.quad.normalized_corners = Some([a, b, c, c]);

                        draw
                    }));
                }
                Command::Icon(icon) => out.push(icon_draw(icon, origin)),
                Command::NamedIcon(icon) => {
                    if let Some(draw) = self.named_icon_draw(icon, origin) {
                        out.push(draw);
                    }
                }
                Command::Image(image) => out.extend(self.image_draw(image, origin)),
                Command::ImageTick { owner } => {
                    self.image_ticks.insert(*owner);
                }
                Command::CaretTick { owner, focused } => {
                    self.caret_ticks.insert(*owner, *focused);
                }
                Command::EditAlignment { owner, instance } => {
                    self.edit_alignment_writes.insert(*owner, *instance);
                }
                Command::EditScroll {
                    owner,
                    instance,
                    admission,
                } => {
                    self.edit_scroll_writes
                        .insert(*owner, (*instance, admission.clone()));
                }
                Command::NotePage {
                    owner,
                    instance,
                    admission,
                } => {
                    self.note_page_writes
                        .insert(*owner, (*instance, *admission));
                }
                Command::ClipPush { rect, .. } => {
                    let mut clip = [
                        rect[0] as f32 + origin.0 as f32,
                        rect[1] as f32 + origin.1 as f32,
                        rect[2] as f32 + origin.0 as f32,
                        rect[3] as f32 + origin.1 as f32,
                    ];

                    if let Some(parent) = clips.last() {
                        clip = [
                            clip[0].max(parent[0]),
                            clip[1].max(parent[1]),
                            clip[2].min(parent[2]),
                            clip[3].min(parent[3]),
                        ];
                    }

                    clips.push(clip);
                }
                Command::ClipPop { .. } => {
                    clips.pop();
                }
                Command::IconScopePush { .. } | Command::IconScopePop { .. } => {
                    unreachable!()
                }
            }

            if let Some(&clip) = clips.last() {
                let mut end = start;

                for i in start..out.len() {
                    let mut draw = out[i];

                    if clip_ui_quad(&mut draw.quad, clip) {
                        out[end] = draw;

                        end += 1;
                    }
                }

                out.truncate(end);
            }
        }

        for draw in &mut out[first..] {
            scale_quad(&mut draw.quad, self.scale);
        }

        for (owner, element) in &list.pending {
            self.note_once(format!("{owner:?}: {element:?} has no draw implementation"));
        }

        out
    }

    pub fn begin_frame(&mut self) {
        self.image_ticks.clear();

        self.caret_ticks.clear();

        self.balloon_alpha_writes.clear();

        self.frame_time = None;

        self.edit_alignment_writes.clear();

        self.edit_scroll_writes.clear();

        self.note_page_writes.clear();
    }

    pub fn begin_frame_at(&mut self, now_ms: u32) {
        self.begin_frame();

        self.frame_time = Some(now_ms);
    }

    pub fn finish_frame(&mut self, now_ms: u32) {
        let timed = self.frame_time.take();

        for (owner, (instance, admission)) in std::mem::take(&mut self.edit_scroll_writes) {
            if let Some(element) = ui::input::retained_element_mut(&mut self.panel, owner)
                && element.instance == instance
                && element
                    .layers
                    .get(1)
                    .is_some_and(|m| m.font_index == admission.font)
            {
                let kind = element.element_type;

                let [x, y, w, h] = [element.x, element.y, element.width, element.height];

                if let Some(edit) = element.widget.edit_mut()
                    && !edit.password
                    && !edit.multiline
                    && edit.preedit.is_none()
                    && edit.text == admission.text
                    && edit.caret == admission.caret
                    && edit.scroll == admission.previous
                {
                    let r = edit.rects(kind, x, y, w, h).text;

                    if r[2].saturating_sub(r[0]).max(0) == admission.width {
                        edit.scroll = admission.next;
                    }
                }
            }
        }

        for (owner, (instance, admission)) in std::mem::take(&mut self.note_page_writes) {
            if let Some(element) = ui::input::retained_element_mut(&mut self.panel, owner)
                && element.instance == instance
            {
                ui::notepad::commit_page(element, admission);
            }
        }

        for (owner, instance) in std::mem::take(&mut self.edit_alignment_writes) {
            if let Some(element) = ui::input::retained_element_mut(&mut self.panel, owner)
                && element.instance == instance
                && let Some(material) = element.layers.get_mut(1)
            {
                material.font_align = 0x10;
            }
        }

        for (owner, instance) in std::mem::take(&mut self.balloon_alpha_writes) {
            if let Some(element) = ui::input::retained_element_mut(&mut self.panel, owner)
                && element.instance == instance
                && let Some(body) = element.layers.first_mut()
            {
                body.texture_color.live[3] = 0.7;
            }
        }

        let mut admitted = BTreeSet::new();

        for (owner, focused) in std::mem::take(&mut self.caret_ticks) {
            if let Some(element) = ui::input::retained_element_mut(&mut self.panel, owner)
                && let Some(edit) = element.widget.edit_mut()
                && let Some(now) = timed
            {
                edit.advance_caret(now, focused);

                admitted.insert(element.instance);
            }
        }

        fn lose_hidden(elements: &mut [ui::Element], admitted: &BTreeSet<ui::element::Instance>) {
            for element in elements {
                if !admitted.contains(&element.instance)
                    && let Some(edit) = element.widget.edit_mut()
                {
                    edit.lose_caret_focus();
                }

                if let Some(list) = element.widget.list_mut() {
                    for row in &mut list.items {
                        lose_hidden(&mut row.children, admitted);
                    }
                }
            }
        }

        if timed.is_some() {
            lose_hidden(&mut self.panel.elements, &admitted);
        }

        let seconds = self
            .image_clock
            .replace(now_ms)
            .map_or(0.0, |old| now_ms.wrapping_sub(old) as f32 / 1000.0);

        for owner in std::mem::take(&mut self.image_ticks) {
            let element = ui::input::retained_element_mut(&mut self.panel, owner);

            if let Some(element) = element
                && let ui::Widget::Image(state) = &mut element.widget
                && state.advance(seconds)
            {
                element.visible = false;
            }
        }
    }

    fn push_text(
        &mut self,
        text: &TextDraw,
        run: u32,
        origin: (i32, i32),
        out: &mut Vec<PanelDraw>,
    ) {
        let surface_rect = [text.indent, 0, text.indent + text.width(), text.height()];

        let Some(cached) = self.cache.resolve(
            (text.owner, run),
            &text.text,
            text.font,
            surface_rect,
            text.align,
            &mut self.fonts,
        ) else {
            let note = format!("{:?}: no FontList row {}", text.owner, text.font);

            self.note_once(note);

            return;
        };

        let at = (
            text.rect[0] - text.indent + origin.0,
            text.rect[1] + origin.1,
        );

        for quad in table::quads(
            &cached.bitmap,
            at,
            text.color,
            text.shadow as i32,
            text.shadow_color,
        ) {
            out.push(PanelDraw {
                owner: text.owner,
                source: Source::Caption(text.owner, run),
                quad,
                uv: UvRule::TexelEdge,
                additive: false,
            });
        }
    }

    fn note_once(&mut self, note: String) {
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    pub fn measure(&mut self) -> FontMeasure<'_> {
        FontMeasure {
            fonts: &mut self.fonts,
        }
    }

    pub fn add_tree_item(
        &mut self,
        index: usize,
        parent: Option<usize>,
        text: &str,
        color: u32,
    ) -> Option<usize> {
        self.add_tree_item_with(
            index,
            parent,
            text,
            ui::tree_view::ItemOptions {
                color,
                ..Default::default()
            },
        )
    }

    pub fn add_tree_item_with(
        &mut self,
        index: usize,
        parent: Option<usize>,
        text: &str,
        options: ui::tree_view::ItemOptions,
    ) -> Option<usize> {
        let element = self.panel.elements.get_mut(index)?;

        element.add_measured_tree_item_with(
            parent,
            text,
            options,
            &mut FontMeasure {
                fonts: &mut self.fonts,
            },
        )
    }

    pub fn add_named_tree_child(
        &mut self,
        index: usize,
        parent: &str,
        text: &str,
        color: u32,
    ) -> Option<usize> {
        self.add_named_tree_child_with(
            index,
            parent,
            text,
            ui::tree_view::ItemOptions {
                color,
                ..Default::default()
            },
        )
    }

    pub fn add_named_tree_child_with(
        &mut self,
        index: usize,
        parent: &str,
        text: &str,
        options: ui::tree_view::ItemOptions,
    ) -> Option<usize> {
        let element = self.panel.elements.get_mut(index)?;

        element.add_named_tree_child_with(
            parent,
            text,
            options,
            &mut FontMeasure {
                fonts: &mut self.fonts,
            },
        )
    }

    pub fn add_tagged_tree_child(
        &mut self,
        index: usize,
        parent: i32,
        text: &str,
        options: ui::tree_view::ItemOptions,
    ) -> Option<usize> {
        let element = self.panel.elements.get_mut(index)?;

        element.add_tagged_tree_child(
            parent,
            text,
            options,
            &mut FontMeasure {
                fonts: &mut self.fonts,
            },
        )
    }

    pub fn notepad_click(&mut self, index: usize, x: i32, y: i32, shift: bool) {
        if let Some(element) = self.panel.elements.get_mut(index) {
            let mut measure = FontMeasure {
                fonts: &mut self.fonts,
            };

            ui::notepad::click(element, x, y, shift, &mut measure);
        }
    }

    pub fn notepad_pointer(
        &mut self,
        index: usize,
        event: ui::input::Mouse,
        x: i32,
        y: i32,
        shift: bool,
    ) -> bool {
        let Some(element) = self.panel.elements.get_mut(index) else {
            return false;
        };

        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        ui::notepad::pointer(element, event, x, y, shift, &mut measure)
    }

    pub fn edit_pointer(&mut self, owner: Owner, event: ui::input::Mouse, x: i32) -> bool {
        let Some((element, rect)) = ui::input::owner_element_mut(&mut self.panel, owner) else {
            return false;
        };

        ui::edit_pointer::pointer(
            element,
            rect,
            event,
            x,
            &mut FontMeasure {
                fonts: &mut self.fonts,
            },
        )
    }

    pub fn notepad_key(
        &mut self,
        index: usize,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
    ) {
        self.notepad_key_with_clipboard(index, key, mods, &mut ui::text_entry::Local::default());
    }

    pub fn notepad_key_with_clipboard(
        &mut self,
        index: usize,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
        clipboard: &mut dyn ui::text_entry::Clipboard,
    ) {
        let Some(element) = self.panel.elements.get_mut(index) else {
            return;
        };

        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        ui::notepad::key_with_clipboard(element, key, mods, &mut measure, clipboard);
    }

    pub fn notepad_owner_key(
        &mut self,
        owner: Owner,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
    ) -> bool {
        let Some((element, _)) = ui::input::owner_element_mut(&mut self.panel, owner) else {
            return false;
        };

        if !element.widget.edit().is_some_and(|edit| edit.note_pad) {
            return false;
        }

        let mut measure = FontMeasure {
            fonts: &mut self.fonts,
        };

        ui::notepad::key(element, key, mods, &mut measure);

        true
    }

    pub fn origin(&self, screen: [i32; 4]) -> (i32, i32) {
        self.panel.position(screen)
    }

    pub fn reposition(&mut self, screen: [i32; 4]) -> (i32, i32) {
        let mut origin = self.origin(screen);

        let width = self.panel.width();

        let height = self.panel.height();

        if self.panel.screen_align & formats::ui::pane::screen_align::RIGHT != 0 {
            self.panel.left = screen[2].wrapping_sub(width);

            self.panel.right = screen[2];

            origin.0 = self.panel.left;
        }

        if self.panel.screen_align & formats::ui::pane::screen_align::BOTTOM != 0 {
            self.panel.top = screen[3].wrapping_sub(height);

            self.panel.bottom = screen[3];

            origin.1 = self.panel.top;
        }

        self.panel.prepare_text_layouts();

        origin
    }

    pub fn referenced_textures(&self, interaction: &Interaction) -> Vec<i32> {
        let mut seen = Vec::new();

        let mut push = |index: i32| {
            if !seen.contains(&index) {
                seen.push(index);
            }
        };

        for layer in self.panel.draw_list(interaction).layers() {
            push(layer.texture_index);
        }

        let mut retained: Vec<_> = self.panel.elements.iter().collect();

        while let Some(element) = retained.pop() {
            for material in &element.layers {
                if material.texture_index >= 0 {
                    push(material.texture_index);
                }
            }

            if element.element_type == formats::ui::pane::ElementType::IconButton {
                for alias in &self.resource.aliases {
                    if alias
                        .name
                        .to_ascii_lowercase()
                        .starts_with("ui_icon.iconrect")
                        && alias.texture_index >= 0
                    {
                        push(alias.texture_index);
                    }
                }
            }

            if let Some(combo) = element.widget.combo_box() {
                for material in element.layers.iter().chain(&combo.scrollbar.layers) {
                    if material.texture_index >= 0 {
                        push(material.texture_index);
                    }
                }
            }

            if let Some(pad) = element.widget.edit().and_then(|e| e.pad.as_ref()) {
                for material in element
                    .layers
                    .iter()
                    .chain(&pad.scrollbar.layers)
                    .chain(&pad.child_layers)
                {
                    if material.texture_index >= 0 {
                        push(material.texture_index);
                    }
                }
            }

            let Some(list) = element.widget.list() else {
                continue;
            };

            for material in element.layers.iter().chain(&list.scrollbar.layers) {
                if material.texture_index >= 0 {
                    push(material.texture_index);
                }
            }

            for child in list
                .template
                .iter()
                .chain(list.items.iter().flat_map(|r| &r.children))
            {
                for material in &child.layers {
                    if material.texture_index >= 0 {
                        push(material.texture_index);
                    }
                }

                retained.push(child);
            }
        }

        seen
    }

    pub fn texture_paths(&self) -> BTreeMap<i32, String> {
        self.resource
            .textures
            .iter()
            .map(|t| (t.index, t.path.clone()))
            .collect()
    }
}

fn clip_ui_quad(quad: &mut Quad, clip: [f32; 4]) -> bool {
    if quad.dst_w <= 0.0 || quad.dst_h <= 0.0 {
        return false;
    }

    if quad.rotation != 0.0 {
        if clip[2] <= clip[0] || clip[3] <= clip[1] {
            return false;
        }

        quad.clip = Some(clip);

        return true;
    }

    let left = quad.dst_x.max(clip[0]);

    let top = quad.dst_y.max(clip[1]);

    let right = (quad.dst_x + quad.dst_w).min(clip[2]);

    let bottom = (quad.dst_y + quad.dst_h).min(clip[3]);

    if right <= left || bottom <= top {
        return false;
    }

    quad.src_x += (left - quad.dst_x) * quad.src_w / quad.dst_w;

    quad.src_y += (top - quad.dst_y) * quad.src_h / quad.dst_h;

    quad.src_w *= (right - left) / quad.dst_w;

    quad.src_h *= (bottom - top) / quad.dst_h;

    quad.dst_x = left;

    quad.dst_y = top;

    quad.dst_w = right - left;

    quad.dst_h = bottom - top;

    true
}

pub fn load_template(
    game: &GameDir,
    script: &[u8],
    resource: &Resource,
    strings: &Strings,
) -> Result<Vec<ui::Element>> {
    load_template_inner(game, script, resource, strings, &mut Vec::new(), &mut 4096)
}

fn load_template_inner(
    game: &GameDir,
    script: &[u8],
    resource: &Resource,
    strings: &Strings,
    active: &mut Vec<String>,
    remaining: &mut usize,
) -> Result<Vec<ui::Element>> {
    let key = assets::fold(script);

    anyhow::ensure!(
        !active.contains(&key),
        "cyclic ItemPanel reference: {} -> {key}",
        active.join(" -> ")
    );

    anyhow::ensure!(
        active.len() < 64,
        "ItemPanel nesting exceeds 64 files at {key}"
    );

    anyhow::ensure!(
        *remaining > 0,
        "ItemPanel expansion exceeds 4096 files at {key}"
    );

    *remaining -= 1;

    let raw = game
        .read(script)
        .with_context(|| String::from_utf8_lossy(script).into_owned())?;

    let name = String::from_utf8_lossy(script).into_owned();

    let parsed = pane::load(&name, &raw).with_context(|| format!("parsing {name}"))?;

    let mut elements = Panel::instantiate(&parsed, resource, strings, false).elements;

    active.push(key);

    for element in &mut elements {
        resolve_item_panels(game, element, resource, strings, active, remaining)
            .with_context(|| format!("ItemPanel in {name}"))?;
    }

    active.pop();

    Ok(elements)
}

fn resolve_item_panels(
    game: &GameDir,
    element: &mut ui::Element,
    resource: &Resource,
    strings: &Strings,
    active: &mut Vec<String>,
    remaining: &mut usize,
) -> Result<()> {
    let Some(list) = element.widget.list_mut() else {
        return Ok(());
    };

    while let Some(script) = list.pending_item_panels.first() {
        if !script.is_empty() {
            let children = load_template_inner(
                game,
                script.as_bytes(),
                resource,
                strings,
                active,
                remaining,
            )
            .with_context(|| format!("ItemPanel {script}"))?;

            list.template.extend(children);
        }

        list.pending_item_panels.remove(0);
    }

    for child in list
        .template
        .iter_mut()
        .chain(list.items.iter_mut().flat_map(|row| &mut row.children))
    {
        resolve_item_panels(game, child, resource, strings, active, remaining)?;
    }

    Ok(())
}

const SPLIT_LOW: f32 = 0.49;

const SPLIT_HIGH: f32 = 0.51;

fn layer_draw(layer: &LayerDraw, origin: (i32, i32)) -> Vec<PanelDraw> {
    let [left, top, right, bottom] = layer.rect;

    let (dst_w, dst_h) = ((right - left) as f32, (bottom - top) as f32);

    let [u0, v0, u1, v1] = layer.uv;

    let (src_w, src_h) = ((u1 - u0) as f32, (v1 - v0) as f32);

    let (x0, y0) = ((left + origin.0) as f32, (top + origin.1) as f32);

    let whole = |dst: (f32, f32, f32, f32), src: (f32, f32, f32, f32)| PanelDraw {
        owner: layer.owner,
        source: Source::Texture(layer.texture_index),
        additive: layer.additive,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: dst.0,
            dst_y: dst.1,
            dst_w: dst.2,
            dst_h: dst.3,
            src_x: src.0,
            src_y: src.1,
            src_w: src.2,
            src_h: src.3,
            color: layer.color,
            rotation: 0.0,
        },
        uv: UvRule::TexelEdge,
    };

    if layer.raster_op == RasterOp::Wrap {
        return vec![whole(
            (x0, y0, dst_w, dst_h),
            (u0 as f32, v0 as f32, dst_w, dst_h),
        )];
    }

    let ratio = |dst: f32, src: f32| if src == 0.0 { 1.0 } else { dst / src };

    let (wide, tall) = (ratio(dst_w, src_w) > 1.0, ratio(dst_h, src_h) > 1.0);

    let (cut_x, cut_y) = match layer.raster_op {
        RasterOp::Split => (wide, tall),
        RasterOp::WSplitStretch => (wide, false),
        RasterOp::HStretchSplit => (false, tall),
        _ => (false, false),
    };

    let axis = |cut: bool, near: f32, extent: f32, source: f32| -> (Vec<f32>, Vec<f32>) {
        if !cut {
            return (vec![near, near + extent], vec![0.0, 1.0]);
        }

        let half = source * 0.5;

        (
            vec![near, near + half, near + extent - half, near + extent],
            vec![0.0, SPLIT_LOW, SPLIT_HIGH, 1.0],
        )
    };

    let (xs, fu) = axis(cut_x, x0, dst_w, src_w);

    let (ys, fv) = axis(cut_y, y0, dst_h, src_h);

    let mut out = Vec::with_capacity((xs.len() - 1) * (ys.len() - 1));

    for row in 0..ys.len() - 1 {
        for col in 0..xs.len() - 1 {
            let (sx0, sx1) = (u0 as f32 + src_w * fu[col], u0 as f32 + src_w * fu[col + 1]);

            let (sy0, sy1) = (v0 as f32 + src_h * fv[row], v0 as f32 + src_h * fv[row + 1]);

            out.push(whole(
                (
                    xs[col],
                    ys[row],
                    xs[col + 1] - xs[col],
                    ys[row + 1] - ys[row],
                ),
                (sx0, sy0, sx1 - sx0, sy1 - sy0),
            ));
        }
    }

    out
}

pub fn scale_quad(quad: &mut Quad, scale: f32) {
    quad.dst_x *= scale;

    quad.dst_y *= scale;

    quad.dst_w *= scale;

    quad.dst_h *= scale;

    if let Some(clip) = &mut quad.clip {
        for edge in clip {
            *edge *= scale;
        }
    }
}

fn rotated_image_draw(image: &ui::draw::ImageDraw, origin: (i32, i32)) -> Option<PanelDraw> {
    let material = &image.material;

    if material.texture_color.live[3] == 0.0 {
        return None;
    }

    let [u0, v0, u1, v1] = material.uv;

    let (width, height) = ((u1 - u0) as f32, (v1 - v0) as f32);

    let angle = image.state.rotation;

    if !angle.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }

    let [left, top, right, bottom] = image.rect;

    let [px, py] = image.state.rotation_center;

    let (x, y) = (width * 0.5 - px as f32, height * 0.5 - py as f32);

    let (s, c) = angle.sin_cos();

    Some(PanelDraw {
        owner: image.owner,
        source: Source::Texture(material.texture_index),
        additive: false,
        uv: UvRule::TexelEdge,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: (left as f32 + right as f32) * 0.5 + origin.0 as f32 + x * c
                - y * s
                - width * 0.5,
            dst_y: (top as f32 + bottom as f32) * 0.5 + origin.1 as f32 + x * s + y * c
                - height * 0.5,
            dst_w: width,
            dst_h: height,
            src_x: u0 as f32,
            src_y: v0 as f32,
            src_w: width,
            src_h: height,
            color: material.texture_color.live_argb(),
            rotation: angle,
        },
    })
}

fn fill_draw(fill: &ui::draw::FillDraw, origin: (i32, i32)) -> PanelDraw {
    let [left, top, right, bottom] = fill.rect;

    PanelDraw {
        owner: fill.owner,
        source: Source::Solid,
        additive: false,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: (left + origin.0) as f32,
            dst_y: (top + origin.1) as f32,
            dst_w: (right - left) as f32,
            dst_h: (bottom - top) as f32,
            src_x: 0.0,
            src_y: 0.0,
            src_w: 0.0,
            src_h: 0.0,
            color: fill.color,
            rotation: 0.0,
        },
        uv: UvRule::TexelCentre,
    }
}

fn icon_file_key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn icon_draw(icon: &ui::draw::IconDraw, origin: (i32, i32)) -> PanelDraw {
    let [left, top, right, bottom] = icon.rect;

    let [u0, v0, u1, v1] = icon.uv;

    PanelDraw {
        owner: icon.owner,
        additive: false,
        source: match crate::frame::AmulSheet::of(icon.sheet) {
            Some(amul) => Source::Amul(amul),
            None => Source::ItemIcon(icon.sheet),
        },
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: (left + origin.0) as f32,
            dst_y: (top + origin.1) as f32,
            dst_w: (right - left) as f32,
            dst_h: (bottom - top) as f32,
            src_x: u0 as f32,
            src_y: v0 as f32,
            src_w: (u1 - u0) as f32,
            src_h: (v1 - v0) as f32,
            color: icon.color,
            rotation: 0.0,
        },
        uv: UvRule::TexelEdge,
    }
}

pub fn shadow_offsets(shadow: ShadowType) -> Vec<(i32, i32)> {
    table::shadow_offsets(shadow as i32)
}

pub fn font_table(resource: &Resource, chains: &Chains, zoom: f32) -> FontTable {
    let chain0: Vec<std::sync::Arc<Face>> = chains.chains[0].faces.clone();

    let chain1: Vec<std::sync::Arc<Face>> = chains.chains[1].faces.clone();

    FontTable::build(&resource.fonts, [&chain0, &chain1], zoom)
}

pub fn load_item_help(game: &GameDir) -> std::io::Result<Vec<formats::data::item_help::Help>> {
    use formats::data::item_help;

    let (bytes, cp) = read_language(game, "itemhelp.dat")?;

    let mut help = item_help::parse(&bytes, cp);

    if let Some(bytes) = [
        b"Lang/EN/itemhelp.dat".as_slice(),
        b"Lang/PT/itemhelp.dat",
        b"itemhelp.dat",
    ]
    .into_iter()
    .find_map(|path| game.read(path).ok())
    {
        item_help::overlay_colors(&mut help, &bytes);
    }

    Ok(help)
}

#[cfg(test)]

mod language_tests {

    use super::*;
    use formats::{
        config::Language,
        text::{Codepage, codepage},
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);

            loop {
                let path = std::env::temp_dir().join(format!(
                    "nwyd-language-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));

                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("creating test directory: {error}"),
                }
            }
        }

        fn write(&self, path: &str, bytes: &[u8]) {
            let path = self.0.join(path);

            fs::create_dir_all(path.parent().unwrap()).unwrap();

            fs::write(path, bytes).unwrap();
        }

        fn game(&self, language: Language) -> GameDir {
            let mut game = GameDir::open(&self.0).unwrap();

            game.set_language(language);

            game
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]

    fn relocalization_keeps_panel_identity_and_advances_caption_texture_revisions() {
        let root = Directory::new();

        root.write("Lang/EN/UIString.txt", b"1 English\n");

        root.write("Lang/PT/UIString.txt", b"1 Portugu\xEAs\n");

        let english = root.game(Language::English);

        let portuguese = root.game(Language::Portuguese);

        let old_strings = strings(&english);

        let new_strings = strings(&portuguese);

        let mut resource = Resource::default();

        resource.fonts.push(guimat::FontDef {
            index: 0,
            face: 0,
            height: 12,
            width_adjust: 0,
            field4: 0,
            bold: false,
        });

        let mut chains = Chains::default();

        chains.chains[0].faces.push(std::sync::Arc::new(
            Face::parse(crate::fonts::FALLBACK, 0).unwrap(),
        ));

        let mut element =
            ui::Element::new(pane::ElementType::Static, "Static", "translated", &resource);

        element.apply(
            &pane::Property {
                key: "Caption".into(),
                value: "uu_1".into(),
            },
            &resource,
            &old_strings,
        );

        let panel = Panel {
            elements: vec![element],
            focus: Some(0),
            ..Default::default()
        };

        let mut ui = PanelUi::from_resource(panel, resource, &chains, 1.25);

        let identity = ui.identity();

        let lifetime = ui.lifetime();

        let instance = ui.panel.elements[0].instance;

        let key = (Owner::Element(0), 0);

        let old_serial = ui
            .cache
            .resolve(key, "English", 0, [0, 0, 128, 24], 0, &mut ui.fonts)
            .unwrap()
            .serial;

        ui.relocalize(&portuguese, &new_strings);

        assert_eq!(ui.identity(), identity);

        assert!(ui.lifetime().ptr_eq(&lifetime));

        assert_eq!(ui.panel.elements[0].instance, instance);

        assert_eq!(ui.panel.elements[0].display, "Português");

        assert_eq!(ui.panel.focus, Some(0));

        assert_eq!(ui.scale, 1.25);

        assert_eq!(ui.game.as_ref().unwrap().language(), Language::Portuguese);

        assert!(ui.cache.is_empty());

        assert_eq!(ui.cache.built(), old_serial);

        let new_serial = ui
            .cache
            .resolve(
                key,
                &ui.panel.elements[0].display,
                0,
                [0, 0, 128, 24],
                0,
                &mut ui.fonts,
            )
            .unwrap()
            .serial;

        assert!(new_serial > old_serial);
    }

    #[test]

    fn selected_resources_decode_in_their_language_with_one_read() {
        let root = Directory::new();

        root.write("Lang/KR/Caption.txt", b"\xC7\xD1\xB1\xB9\xBE\xEE");

        root.write("Lang/EN/Caption.txt", b"English");

        let game = root.game(Language::Korean);

        let (bytes, cp) = read_language(&game, "caption.TXT").unwrap();

        assert_eq!(cp, Codepage::Windows949);

        assert_eq!(codepage::decode(&bytes, cp), "한국어");

        assert_eq!(game.reads(), 1);
    }

    #[test]

    fn english_fallback_uses_english_encoding_and_existing_translation_errors_propagate() {
        let root = Directory::new();

        root.write("Lang/EN/Caption.txt", b"Price \x80");

        let game = root.game(Language::Korean);

        let (bytes, cp) = read_language(&game, "Caption.txt").unwrap();

        assert_eq!(cp, Codepage::Windows1252);

        assert_eq!(codepage::decode(&bytes, cp), "Price €");

        assert_eq!(game.reads(), 1);

        root.write("Lang/KR/Caption.txt", b"translated");

        let game = root.game(Language::Korean);

        fs::remove_file(root.0.join("Lang/KR/Caption.txt")).unwrap();

        assert_eq!(
            read_language(&game, "Caption.txt").unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );

        assert_eq!(game.reads(), 1);
    }

    #[test]

    fn ui_captions_and_character_descriptions_respect_individual_resource_languages() {
        let root = Directory::new();

        root.write("Lang/KR/UIString.txt", b"1 \xC7\xD1\xB1\xB9\xBE\xEE\r\n");

        root.write(
            "Lang/KR/Caption.txt",
            b"FFFFFF \xC7\xD1\xB1\xB9\xBE\xEE\r\n",
        );

        root.write("Lang/EN/Fallback.txt", b"FFFFFF Price \x80\r\n");

        root.write(
            "Lang/KR/chardesctrans.txt",
            b"FFFFFF \xC7\xD1\xB1\xB9\xBE\xEE\r\n",
        );

        root.write("Lang/EN/chardescfoema.txt", b"FFFFFF Price \x80\r\n");

        let game = root.game(Language::Korean);

        let strings = strings(&game);

        assert_eq!(strings.translate("uu_1"), "한국어");

        assert_eq!(strings.translate("ff_Caption.txt"), "한국어<n>");

        assert_eq!(strings.translate("ff_Fallback.txt"), "Price €<n>");

        let reads = game.reads();

        assert_eq!(strings.translate("ff_Caption.txt"), "한국어<n>");

        assert_eq!(game.reads(), reads);

        let descriptions =
            crate::scene::create::descriptions(&game, Codepage::Windows1252).unwrap();

        assert_eq!(descriptions[0], "한국어<n>");

        assert_eq!(descriptions[1], "Price €<n>");
    }
}
