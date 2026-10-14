pub mod billboard;
pub mod blit;
pub mod camera;
pub mod cull;
pub mod device;
pub mod error;
pub mod font;
pub mod geometry;
pub mod mesh;
pub mod pack;
pub mod preset;
pub mod quirk;
pub mod renderer;
pub mod sea;
pub mod shaders;
pub mod skin;
pub mod sprite;
pub mod target;
pub mod temporal;
pub mod texture;
pub mod world;

pub use billboard::{
    BillboardArray, BillboardDraw, BillboardPass, BillboardRenderer, BillboardUniforms,
    BillboardVertex,
};
pub use blit::{BlitSource, Blitter, Fit};
pub use camera::{Matrix4, Projection, View};
pub use device::Gpu;
pub use error::{Error, Result};
pub use font::{CAPTION_ALIGN, Face, Font, FontTable, Glyph, Page, Placed, TextBitmap, TextCache};
pub use preset::{Preset, RenderMode};
pub use renderer::{BoundTexture, SpriteRenderer, TextureSet};
pub use sea::{SeaDraw, SeaRenderer, SeaTexture, SeaUniforms, SeaVertex};
pub use skin::{
    BonePalette, SkinArena, SkinDraw, SkinFog, SkinFrame, SkinObject, SkinParams, SkinRenderer,
    SkinVertexGpu,
};
pub use sprite::{Quad, SpriteBatcher, TlVertex, UvRule};
pub use target::{COLOR_FORMAT, DEPTH_FORMAT, DepthBuffer, Offscreen};
pub use texture::{AlphaPolicy, GpuTexture};
pub use world::{TerrainBatch, TerrainMesh, TerrainRenderer, WorldVertex};

#[inline(always)]

pub const fn gpu_label(label: &str) -> Option<&str> {
    Some(label)
}
