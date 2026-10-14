use super::actors::{ANI_SOUND, BONE_ANI, VALID_INDEX};
use super::objects::{EFFECT_TEXTURE_LIST, MESH_LIST, MESH_TEXTURE_LIST};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::audio::ani_sound::{self, AniSound};
use formats::data::item_list::ItemList;
use formats::mesh::mesh_list::{self, MeshList};
use formats::mesh::registry::{self, ModelEntry, ValidIndex};
use formats::scene::rc::{self, ResourceTable};
use formats::texture::texture_list::{self, TextureList};

#[derive(Debug)]
pub struct Registries {
    pub resource_names: ResourceTable,
    pub entries: Vec<ModelEntry>,
    pub valid: ValidIndex,
    pub sounds: AniSound,
    pub textures: TextureList,
    pub effect_textures: TextureList,
    pub items: ItemList,
    pub list: MeshList,
    pub arena_visual: Vec<formats::data::arena_visual::Record>,
}

impl Registries {
    pub fn load(game: &GameDir) -> Result<Registries> {
        let resource_names =
            parse_resource_names(&game.read(br"UI\RC.bin").context("reading UI/RC.bin")?)?;

        let entries =
            registry::parse_bone_ani(&game.read(BONE_ANI).context(r"reading BoneAni4.txt")?);

        let valid = registry::parse_valid_index(
            &game.read(VALID_INDEX).context(r"reading ValidIndex.bin")?,
        )
        .context("parsing ValidIndex.bin")?;

        let sounds = ani_sound::parse(&game.read(ANI_SOUND).context("reading AniSound4.txt")?)
            .context("parsing AniSound4.txt")?;

        let textures = texture_list::parse(
            &game
                .read(MESH_TEXTURE_LIST)
                .context(r"reading MeshTextureList.bin")?,
        )
        .context("parsing MeshTextureList.bin")?;

        let effect_textures = texture_list::parse(
            &game
                .read(EFFECT_TEXTURE_LIST)
                .context(r"reading EffectTextureList.bin")?,
        )
        .context("parsing EffectTextureList.bin")?;

        let items = super::item_table::load(game)?;

        let list = mesh_list::parse(&game.read(MESH_LIST).context(r"reading mesh\MeshList.txt")?);

        let arena_visual = formats::data::arena_visual::parse(
            &game
                .read(b"ArenaVisualData.bin")
                .context("reading ArenaVisualData.bin")?,
        )
        .context("parsing ArenaVisualData.bin")?;

        Ok(Registries {
            resource_names,
            entries,
            valid,
            sounds,
            textures,
            effect_textures,
            items,
            list,
            arena_visual,
        })
    }
}

fn parse_resource_names(bytes: &[u8]) -> Result<ResourceTable> {
    rc::parse(&bytes[..bytes.len().min(rc::SLOTS * rc::RECORD_LEN)]).context("parsing UI/RC.bin")
}
