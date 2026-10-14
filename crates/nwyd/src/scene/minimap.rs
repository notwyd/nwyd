use anyhow::{Context, Result};
use assets::GameDir;
use formats::text::Codepage;
use formats::texture::{self, PixelFormat, Texture};
use formats::world::ZONE_CELLS;

pub const DYNAMIC_SLOT: u32 = 4;

pub const FALLBACK_SLOT: u32 = 13;

pub const FALLBACK_PATH: &[u8] = br"UI\minimap.wyt";

pub const TILE: u32 = 128;

pub const TILES: i32 = 3;

pub const FIRST_ROW_Y: u32 = 2 * TILE;

pub const ZOOM_DEFAULT: f32 = 0.6;

pub const ZOOM_WIDE: f32 = 1.5;

pub const WINDOW_FRACTION: f32 = 0.25;

pub const WINDOW_HALF_FRACTION: f32 = 0.125;

pub const WINDOW_Y_NUDGE: i32 = -4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Scale {
    Full,
    Half,
}

impl Scale {
    pub fn size(self) -> u32 {
        match self {
            Scale::Full => 512,
            Scale::Half => 256,
        }
    }

    pub fn divisor(self) -> i32 {
        0x200 / self.size() as i32
    }

    pub fn blit(self, value: u32) -> u32 {
        match self {
            Scale::Full => value,
            Scale::Half => value / 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Tile {
    pub zone: (i32, i32),
    pub path: String,
    pub dst: (u32, u32),
    pub present: bool,
}

pub fn tiles(zone: (u8, u8)) -> Vec<Tile> {
    let mut out = Vec::with_capacity((TILES * TILES) as usize);

    for j in 0..TILES {
        for i in 0..TILES {
            let zone = (i32::from(zone.0) - 1 + i, i32::from(zone.1) - 1 + j);

            out.push(Tile {
                zone,
                path: format!(r"UI\m{:02}{:02}.wyt", zone.0, zone.1),
                dst: (i as u32 * TILE, FIRST_ROW_Y - j as u32 * TILE),
                present: false,
            });
        }
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Window {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn window(scale: Scale, zoom: f32, zone: (u8, u8), player: glam::Vec3) -> Window {
    let (w, h) = (scale.size() as f32, scale.size() as f32);

    let (origin_x, origin_z) = (
        f32::from(zone.0) * ZONE_CELLS as f32,
        f32::from(zone.1) * ZONE_CELLS as f32,
    );

    let div = scale.divisor();

    let mx = (player.x - origin_x + TILE as f32) as i32 / div;

    let mz = (player.z - origin_z + FIRST_ROW_Y as f32) as i32 / div;

    Window {
        x: (mx as f32 - w * WINDOW_HALF_FRACTION * zoom) as i32,
        y: ((scale.size() as i32 - mz) as f32 - h * WINDOW_HALF_FRACTION * zoom) as i32
            + WINDOW_Y_NUDGE,
        w: (w * WINDOW_FRACTION * zoom) as i32,
        h: (h * WINDOW_FRACTION * zoom) as i32,
    }
}

#[derive(Debug)]

pub struct Minimap {
    background: Option<loading::Loader>,
    pub scale: Scale,
    pub zoom: f32,
    pub zone: (u8, u8),
    pub tiles: Vec<Tile>,
    pub composite: Texture,
    pub labels: formats::world::minimap::Minimap,
    pub label_codepage: Codepage,
}

impl Minimap {
    pub fn build(game: &GameDir, zone: (u8, u8), scale: Scale) -> Result<Minimap> {
        let (tiles, composite) = compose(game, zone, scale);

        let (labels, label_codepage) =
            read_labels(game).context("reading the minimap landmark table")?;

        Ok(Minimap {
            background: None,
            scale,
            zoom: ZOOM_DEFAULT,
            zone,
            tiles,
            composite,
            labels,
            label_codepage,
        })
    }

    pub fn recentre(&mut self, game: &GameDir, zone: (u8, u8)) -> Result<bool> {
        if self.zone == zone {
            return Ok(false);
        }

        let (tiles, composite) = compose(game, zone, self.scale);

        self.zone = zone;

        self.tiles = tiles;

        self.composite = composite;

        Ok(true)
    }

    pub fn recentre_background(&mut self, game: &GameDir, zone: (u8, u8)) -> bool {
        if self.zone == zone {
            return false;
        }

        let loader = self
            .background
            .get_or_insert_with(|| loading::Loader::new(game.clone()));

        if let Some((tiles, composite)) = loader.poll(zone, self.scale) {
            self.zone = zone;

            self.tiles = tiles;

            self.composite = composite;

            return true;
        }

        false
    }

    pub fn window(&self, position: glam::Vec3) -> Window {
        window(self.scale, self.zoom, self.zone, position)
    }

    pub fn present(&self) -> usize {
        self.tiles.iter().filter(|t| t.present).count()
    }
}

mod loading;

fn compose(game: &GameDir, zone: (u8, u8), scale: Scale) -> (Vec<Tile>, Texture) {
    let size = scale.size();

    let mut composite = vec![0u8; (size * size * 4) as usize];

    let fallback = decode(game, FALLBACK_PATH);

    let mut tiles = tiles(zone);

    for tile in &mut tiles {
        let decoded = decode(game, tile.path.as_bytes());

        tile.present = decoded.is_some();

        let Some(source) = decoded.as_ref().or(fallback.as_ref()) else {
            continue;
        };

        blit(
            &mut composite,
            size,
            source,
            (scale.blit(tile.dst.0), scale.blit(tile.dst.1)),
            scale.blit(TILE),
        );
    }

    let texture = Texture {
        width: size,
        height: size,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![formats::texture::MipLevel {
            width: size,
            height: size,
            offset: 0,
            len: composite.len(),
        }],
        data: composite,
        source_len: 0,
    };

    (tiles, texture)
}

pub(super) fn read_labels(game: &GameDir) -> Result<(formats::world::minimap::Minimap, Codepage)> {
    let (bytes, cp) = crate::ui::read_language(game, "minimap.dat")?;

    Ok((formats::world::minimap::parse(&bytes)?, cp))
}

fn decode(game: &GameDir, path: &[u8]) -> Option<Texture> {
    let mut decoded = texture::decode(&game.read(path).ok()?).ok()?;

    render::texture::apply_color_key(&mut decoded);

    Some(decoded)
}

fn blit(composite: &mut [u8], stride: u32, source: &Texture, dst: (u32, u32), size: u32) {
    if source.format != PixelFormat::Rgba8Unorm || dst.0 >= stride || dst.1 >= stride {
        return;
    }

    let src = source.base_level();

    let step = if stride == 256 { 2 } else { 1 };

    let copy_w = size.min(source.width.div_ceil(step)).min(stride - dst.0);

    let copy_h = size.min(source.height.div_ceil(step)).min(stride - dst.1);

    for row in 0..copy_h {
        let (dy, dx) = (dst.1 + row, dst.0);

        let take = copy_w as usize * 4;

        let from = (row * step * source.width) as usize * 4;

        let to = (dy * stride + dx) as usize * 4;

        if step == 1 {
            composite[to..to + take].copy_from_slice(&src[from..from + take]);
        } else {
            for column in 0..copy_w as usize {
                let sample = from + column * step as usize * 4;

                let dest = to + column * 4;

                composite[dest..dest + 4].copy_from_slice(&src[sample..sample + 4]);
            }
        }
    }
}

#[cfg(test)]

mod language_tests {

    use super::*;
    use formats::config::Language;
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
                    "nwyd-label-language-test-{}-{}",
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

        fn game(&self) -> GameDir {
            let mut game = GameDir::open(&self.0).unwrap();

            game.set_language(Language::Korean);

            game
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]

    fn minimap_retains_the_selected_label_encoding() {
        let root = Directory::new();

        root.write(
            "Lang/KR/minimap.dat",
            b"128 128 1 0 0 \xC7\xD1\xB1\xB9\xBE\xEE\n",
        );

        root.write("Lang/EN/minimap.dat", b"128 128 1 0 0 English\n");

        let minimap = Minimap::build(&root.game(), (1, 1), Scale::Half).unwrap();

        assert_eq!(minimap.label_codepage, Codepage::Windows949);

        assert_eq!(
            minimap.labels.labels()[0].name(minimap.label_codepage),
            "한국어"
        );
    }

    #[test]

    fn minimap_english_fallback_retains_windows1252() {
        let root = Directory::new();

        root.write("Lang/EN/minimap.dat", b"128 128 1 0 0 Price \x80\n");

        let minimap = Minimap::build(&root.game(), (1, 1), Scale::Half).unwrap();

        assert_eq!(minimap.label_codepage, Codepage::Windows1252);

        assert_eq!(
            minimap.labels.labels()[0].name(minimap.label_codepage),
            "Price €"
        );
    }

    #[test]

    fn hud_main_and_sub_effect_labels_decode_their_individual_resources() {
        use crate::scene::hud::EffectStrings;

        for translated_main in [true, false] {
            let root = Directory::new();

            let (main, sub) = if translated_main {
                ("Lang/KR/EffectString.txt", "Lang/EN/EffectSubString.txt")
            } else {
                ("Lang/KR/EffectSubString.txt", "Lang/EN/EffectString.txt")
            };

            root.write(main, b"\xC7\xD1\xB1\xB9\xBE\xEE\n");

            root.write(sub, b"Price\x80\n");

            let effects = EffectStrings::load(&root.game());

            if translated_main {
                assert_eq!(effects.main(1), "한국어");

                assert_eq!(effects.sub(0), "Price€");
            } else {
                assert_eq!(effects.main(1), "Price€");

                assert_eq!(effects.sub(0), "한국어");
            }
        }
    }
}
