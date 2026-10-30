//! Stable preview material layers with original pixel art and optional local resource textures.
//! No game assets are embedded or downloaded by this module.

use std::{
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const SIZE: u32 = 32;
const MAX_PNG_BYTES: u64 = 16 * 1024 * 1024;
const MAX_EDGE: u32 = 2048;
const MAX_PIXELS: u64 = 4 * 1024 * 1024;

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Material {
    GrassTop = 0,
    GrassSide = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Water = 5,
    OakLogSide = 6,
    OakLogTop = 7,
    OakLeaves = 8,
    TallGrass = 9,
    Poppy = 10,
    Dandelion = 11,
    Snow = 12,
    CoalOre = 13,
    IronOre = 14,
    Gravel = 15,
    Clay = 16,
    BirchLogSide = 17,
    BirchLogTop = 18,
    BirchLeaves = 19,
    SpruceLogSide = 20,
    SpruceLogTop = 21,
    SpruceLeaves = 22,
    AcaciaLogSide = 23,
    AcaciaLogTop = 24,
    AcaciaLeaves = 25,
    RedSand = 26,
    Terracotta = 27,
    Podzol = 28,
    Cactus = 29,
    DeadBush = 30,
    JungleLogSide = 31,
    JungleLogTop = 32,
    JungleLeaves = 33,
    Vine = 34,
    Fern = 35,
    Bamboo = 36,
    Melon = 37,
    Pumpkin = 38,
    Cocoa = 39,
    RedMushroom = 40,
    BrownMushroom = 41,
    Sunflower = 42,
    SugarCane = 43,
    OakPlanks = 44,
    Cobblestone = 45,
    RoofTiles = 46,
    Glass = 47,
    Farmland = 48,
    Wheat = 49,
    HayBale = 50,
    Path = 51,
    Sandstone = 52,
    SprucePlanks = 53,
}

impl Material {
    pub const ALL: [Self; 54] = [
        Self::GrassTop,
        Self::GrassSide,
        Self::Dirt,
        Self::Stone,
        Self::Sand,
        Self::Water,
        Self::OakLogSide,
        Self::OakLogTop,
        Self::OakLeaves,
        Self::TallGrass,
        Self::Poppy,
        Self::Dandelion,
        Self::Snow,
        Self::CoalOre,
        Self::IronOre,
        Self::Gravel,
        Self::Clay,
        Self::BirchLogSide,
        Self::BirchLogTop,
        Self::BirchLeaves,
        Self::SpruceLogSide,
        Self::SpruceLogTop,
        Self::SpruceLeaves,
        Self::AcaciaLogSide,
        Self::AcaciaLogTop,
        Self::AcaciaLeaves,
        Self::RedSand,
        Self::Terracotta,
        Self::Podzol,
        Self::Cactus,
        Self::DeadBush,
        Self::JungleLogSide,
        Self::JungleLogTop,
        Self::JungleLeaves,
        Self::Vine,
        Self::Fern,
        Self::Bamboo,
        Self::Melon,
        Self::Pumpkin,
        Self::Cocoa,
        Self::RedMushroom,
        Self::BrownMushroom,
        Self::Sunflower,
        Self::SugarCane,
        Self::OakPlanks,
        Self::Cobblestone,
        Self::RoofTiles,
        Self::Glass,
        Self::Farmland,
        Self::Wheat,
        Self::HayBale,
        Self::Path,
        Self::Sandstone,
        Self::SprucePlanks,
    ];

    pub const fn layer(self) -> u16 {
        self as u16
    }

    /// Vertex tint for native grayscale vegetation and water. Side grass is composited
    /// into the texture so the same tint never turns the dirt beneath it green.
    pub const fn tint(self) -> [u8; 3] {
        match self {
            Self::GrassTop | Self::TallGrass => [112, 169, 76],
            Self::OakLeaves => [94, 151, 66],
            Self::BirchLeaves => [130, 175, 68],
            Self::SpruceLeaves => [62, 105, 83],
            Self::AcaciaLeaves => [160, 168, 65],
            Self::JungleLeaves | Self::Vine => [62, 145, 49],
            Self::Fern => [74, 160, 55],
            Self::Bamboo => [108, 162, 55],
            Self::SugarCane => [131, 184, 82],
            Self::Water => [78, 145, 225],
            _ => [255; 3],
        }
    }

    pub const fn translucent(self) -> bool {
        matches!(self, Self::Water)
    }

    pub const fn leaves(self) -> bool {
        matches!(
            self,
            Self::OakLeaves
                | Self::BirchLeaves
                | Self::SpruceLeaves
                | Self::AcaciaLeaves
                | Self::JungleLeaves
        )
    }
    fn filenames(self) -> &'static [&'static str] {
        match self {
            Self::GrassTop => &["grass_block_top.png"],
            Self::GrassSide => &["grass_block_side.png"],
            Self::Dirt => &["dirt.png"],
            Self::Stone => &["stone.png"],
            Self::Sand => &["sand.png"],
            Self::Water => &["water_still.png"],
            Self::OakLogSide => &["oak_log.png"],
            Self::OakLogTop => &["oak_log_top.png"],
            Self::OakLeaves => &["oak_leaves.png"],
            Self::TallGrass => &["short_grass.png", "tall_grass_bottom.png", "grass.png"],
            Self::Poppy => &["poppy.png"],
            Self::Dandelion => &["dandelion.png"],
            Self::Snow => &["snow.png"],
            Self::CoalOre => &["coal_ore.png"],
            Self::IronOre => &["iron_ore.png"],
            Self::Gravel => &["gravel.png"],
            Self::Clay => &["clay.png"],
            Self::BirchLogSide => &["birch_log.png"],
            Self::BirchLogTop => &["birch_log_top.png"],
            Self::BirchLeaves => &["birch_leaves.png"],
            Self::SpruceLogSide => &["spruce_log.png"],
            Self::SpruceLogTop => &["spruce_log_top.png"],
            Self::SpruceLeaves => &["spruce_leaves.png"],
            Self::AcaciaLogSide => &["acacia_log.png"],
            Self::AcaciaLogTop => &["acacia_log_top.png"],
            Self::AcaciaLeaves => &["acacia_leaves.png"],
            Self::RedSand => &["red_sand.png"],
            Self::Terracotta => &["terracotta.png"],
            Self::Podzol => &["podzol_top.png"],
            Self::Cactus => &["cactus_side.png"],
            Self::DeadBush => &["dead_bush.png"],
            Self::JungleLogSide => &["jungle_log.png"],
            Self::JungleLogTop => &["jungle_log_top.png"],
            Self::JungleLeaves => &["jungle_leaves.png"],
            Self::Vine => &["vine.png"],
            Self::Fern => &["fern.png"],
            Self::Bamboo => &["bamboo_stalk.png"],
            Self::Melon => &["melon_side.png"],
            Self::Pumpkin => &["pumpkin_side.png"],
            Self::Cocoa => &["cocoa_stage2.png"],
            Self::RedMushroom => &["red_mushroom.png"],
            Self::BrownMushroom => &["brown_mushroom.png"],
            Self::Sunflower => &["sunflower_front.png"],
            Self::SugarCane => &["sugar_cane.png"],
            Self::OakPlanks => &["oak_planks.png"],
            Self::Cobblestone => &["cobblestone.png"],
            Self::RoofTiles => &["bricks.png"],
            Self::Glass => &["glass.png"],
            Self::Farmland => &["farmland.png"],
            Self::Wheat => &["wheat_stage7.png"],
            Self::HayBale => &["hay_block_side.png"],
            Self::Path => &["dirt_path_top.png"],
            Self::Sandstone => &["sandstone.png"],
            Self::SprucePlanks => &["spruce_planks.png"],
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureAtlas {
    /// Consecutive square RGBA8 layers, ordered by `Material::ALL`.
    pub pixels: Vec<u8>,
    pub size: u32,
    pub layers: u32,
    /// Materials loaded from local PNGs; the optional overlay is not another layer.
    pub loaded: usize,
    pub fallback: usize,
    pub missing: Vec<Material>,
    pub grass_overlay_loaded: bool,
}

impl TextureAtlas {
    /// Accept a pack/prepared-resource root, a `textures` root, or a block-texture directory.
    /// Explicit unrecognised roots and corrupt present PNGs are errors; absent layers keep
    /// their original procedural artwork. Errors identify texture names, never full paths.
    pub fn load(root: Option<&Path>) -> Result<Self, Box<dyn Error>> {
        let mut atlas = Self {
            pixels: Material::ALL.into_iter().flat_map(procedural).collect(),
            size: SIZE,
            layers: Material::ALL.len() as u32,
            loaded: 0,
            fallback: Material::ALL.len(),
            missing: Vec::new(),
            grass_overlay_loaded: false,
        };
        let Some(root) = root else {
            atlas.missing.extend(Material::ALL);
            return Ok(atlas);
        };
        let directory = texture_directory(root)?;
        for material in Material::ALL {
            let filename = material
                .filenames()
                .iter()
                .copied()
                .find(|name| directory.join(name).exists());
            let Some(filename) = filename else {
                atlas.missing.push(material);
                continue;
            };
            let mut pixels = load_texture(&directory.join(filename), filename)?;
            if material == Material::GrassSide {
                const OVERLAY: &str = "grass_block_side_overlay.png";
                let path = directory.join(OVERLAY);
                if path.exists() {
                    let overlay = load_texture(&path, OVERLAY)?;
                    compose_grass(&mut pixels, &overlay);
                    atlas.grass_overlay_loaded = true;
                }
            }
            if material == Material::Water {
                for pixel in pixels.as_chunks_mut::<4>().0 {
                    pixel[3] = pixel[3].min(160);
                }
            }
            let start = material.layer() as usize * (SIZE * SIZE * 4) as usize;
            atlas.pixels[start..start + pixels.len()].copy_from_slice(&pixels);
            atlas.loaded += 1;
        }
        atlas.fallback = Material::ALL.len() - atlas.loaded;
        Ok(atlas)
    }
}

fn texture_directory(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if !root.is_dir() {
        return Err("Texture root must be an existing directory".into());
    }
    for candidate in [
        root.join("assets/minecraft/textures/block"),
        root.join("minecraft/textures/block"),
        root.join("textures/block"),
        root.join("block"),
        root.to_path_buf(),
    ] {
        if candidate.is_dir()
            && Material::ALL.iter().any(|material| {
                material
                    .filenames()
                    .iter()
                    .any(|name| candidate.join(name).exists())
            })
        {
            return Ok(candidate);
        }
    }
    Err("Texture root contains none of the supported block PNGs; choose a resource pack, prepared resources, textures, or block directory".into())
}

fn texture_error(name: &str, reason: impl std::fmt::Display) -> Box<dyn Error> {
    format!("Texture {name}: {reason}").into()
}

fn load_texture(path: &Path, name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !std::fs::metadata(path)
        .map_err(|error| texture_error(name, error))?
        .is_file()
    {
        return Err(texture_error(name, "PNG must be a regular file"));
    }
    let file = File::open(path).map_err(|error| texture_error(name, error))?;
    let size = file
        .metadata()
        .map_err(|error| texture_error(name, error))?
        .len();
    if !(33..=MAX_PNG_BYTES).contains(&size) {
        return Err(texture_error(name, "PNG must be 33 bytes to 16 MiB"));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_PNG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| texture_error(name, error))?;
    if bytes.len() as u64 > MAX_PNG_BYTES
        || bytes.len() < 33
        || bytes[..8] != *b"\x89PNG\r\n\x1a\n"
        || bytes[12..16] != *b"IHDR"
        || bytes[8..12] != 13_u32.to_be_bytes()
    {
        return Err(texture_error(name, "invalid PNG signature or IHDR"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into()?);
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || width as u64 * height as u64 > MAX_PIXELS
    {
        return Err(texture_error(
            name,
            "dimensions exceed the 2048-edge / 4-megapixel limit",
        ));
    }
    // Same decoder as hud::decode, but read once so the validated header and decoded
    // bytes cannot come from different files if a resource pack is replaced mid-load.
    let (decoded_width, decoded_height, pixels) = horizon_client_render::hud::decode_bytes(&bytes)
        .map_err(|error| texture_error(name, error))?;
    if (decoded_width, decoded_height) != (width, height)
        || pixels.len() != (width * height * 4) as usize
    {
        return Err(texture_error(
            name,
            "decoded RGBA dimensions do not match the PNG header",
        ));
    }
    // Square textures and vertical animation strips use their first square frame.
    // Horizontal strips are treated symmetrically. Nonintegral aspect ratios are not
    // silently distorted into squares.
    let side = width.min(height);
    if !width.is_multiple_of(side) || !height.is_multiple_of(side) {
        return Err(texture_error(
            name,
            "image must be square or an integral strip of square frames",
        ));
    }
    let mut resized = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let at = ((y * side / SIZE * width + x * side / SIZE) * 4) as usize;
            resized.extend_from_slice(&pixels[at..at + 4]);
        }
    }
    Ok(resized)
}

fn compose_grass(base: &mut [u8], overlay: &[u8]) {
    let tint = Material::GrassTop.tint();
    for (base, overlay) in base
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(overlay.as_chunks::<4>().0)
    {
        let alpha = overlay[3] as u32;
        let base_alpha = base[3] as u32;
        let out_alpha = alpha + (base_alpha * (255 - alpha) + 127) / 255;
        for channel in 0..3 {
            let grass = (overlay[channel] as u32 * tint[channel] as u32 + 127) / 255;
            let premultiplied =
                grass * alpha + (base[channel] as u32 * base_alpha * (255 - alpha) + 127) / 255;
            base[channel] = (premultiplied + out_alpha / 2)
                .checked_div(out_alpha)
                .unwrap_or(0) as u8;
        }
        base[3] = out_alpha as u8;
    }
}

fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut value =
        x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b) ^ salt.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^ (value >> 15)
}

fn color(rgb: [u8; 3], shade: i32, alpha: u8) -> [u8; 4] {
    [
        (rgb[0] as i32 + shade).clamp(0, 255) as u8,
        (rgb[1] as i32 + shade).clamp(0, 255) as u8,
        (rgb[2] as i32 + shade).clamp(0, 255) as u8,
        alpha,
    ]
}

fn procedural(material: Material) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for row in 0..SIZE {
        for column in 0..SIZE {
            let (x, y) = (column / 2, row / 2);
            let grain = hash(x, y, material.layer() as u32);
            let shade = (grain % 29) as i32 - 14;
            let rgba = match material {
                Material::GrassTop => {
                    color([209; 3], shade + if grain % 11 < 2 { -25 } else { 0 }, 255)
                }
                Material::GrassSide => {
                    if y < 3 + hash(x / 2, 0, 31) % 4 {
                        color([86, 139, 55], shade, 255)
                    } else {
                        color([126, 88, 57], shade, 255)
                    }
                }
                Material::Dirt => color(
                    [123, 84, 53],
                    shade + if grain % 13 < 2 { -19 } else { 0 },
                    255,
                ),
                Material::Stone => color(
                    [127, 130, 128],
                    shade / 2
                        + if (x + y * 2).is_multiple_of(11) {
                            -15
                        } else {
                            0
                        },
                    255,
                ),
                Material::Sand => color([218, 204, 153], shade / 2, 255),
                Material::Water => color(
                    [197; 3],
                    shade / 3 + if (x + y * 3) % 13 < 2 { 24 } else { 0 },
                    155,
                ),
                Material::OakLogSide => color(
                    [126, 92, 50],
                    (hash(x / 2, y / 5, 5) % 31) as i32 - 15
                        + if x.is_multiple_of(5) { -25 } else { 0 },
                    255,
                ),
                Material::OakLogTop => {
                    let (dx, dy) = (x as i32 - 7, y as i32 - 8);
                    let ring = ((dx * dx + dy * dy) as u32).isqrt();
                    if x == 0 || y == 0 || x == 15 || y == 15 {
                        color([106, 74, 40], shade / 2, 255)
                    } else {
                        color(
                            [177, 139, 83],
                            shade / 3 - (ring.is_multiple_of(3)) as i32 * 29,
                            255,
                        )
                    }
                }
                Material::OakLeaves
                | Material::BirchLeaves
                | Material::SpruceLeaves
                | Material::AcaciaLeaves
                | Material::JungleLeaves => color(
                    [190; 3],
                    shade + (hash(x / 3, y / 3, 9) % 43) as i32 - 21,
                    if grain.is_multiple_of(if material == Material::JungleLeaves {
                        17
                    } else {
                        7
                    }) {
                        0
                    } else {
                        255
                    },
                ),
                Material::TallGrass => {
                    let blade = y > 2 + hash(x / 3, 0, 11) % 8
                        && (x + (15 - y) / 3) % 4 == 1
                        && x > 1
                        && x < 15;
                    color([207; 3], shade, if blade { 255 } else { 0 })
                }
                Material::Poppy | Material::Dandelion => {
                    let (dx, dy) = ((x as i32 - 8).abs(), (y as i32 - 5).abs());
                    if dx + dy <= 4 && dx <= 3 && dy <= 3 {
                        let flower = if material == Material::Poppy {
                            [209, 46, 43]
                        } else {
                            [249, 207, 49]
                        };
                        color(flower, shade / 2 - if dx + dy <= 1 { 28 } else { 0 }, 255)
                    } else if (x == 8 && y >= 8)
                        || ((6..8).contains(&x) && y == 11)
                        || (x == 9 && y == 13)
                    {
                        color([67, 126, 43], shade / 2, 255)
                    } else {
                        [0; 4]
                    }
                }
                Material::JungleLogSide => color(
                    [118, 100, 56],
                    shade / 2 - if (x + y / 5).is_multiple_of(5) { 25 } else { 0 },
                    255,
                ),
                Material::Vine => {
                    let stem = x % 7 == 2 || ((y / 3 + x / 3) % 3 == 0 && x % 7 < 5 && y % 4 < 2);
                    color([197; 3], shade, if stem { 255 } else { 0 })
                }
                Material::Fern => {
                    let distance = (x as i32 - 8).abs();
                    let blade = (y >= 2 && x == 8)
                        || ((4..15).contains(&y)
                            && distance <= (y as i32 - 1) / 2
                            && (distance + y as i32) % 3 != 0);
                    color([205; 3], shade, if blade { 255 } else { 0 })
                }
                Material::BirchLogSide => color(
                    [224, 222, 207],
                    if (x + hash(0, y / 2, 41) % 13) % 16 < 5 && y % 5 < 2 {
                        -160
                    } else {
                        shade / 2
                    },
                    255,
                ),
                Material::SpruceLogSide => color(
                    [79, 57, 37],
                    shade / 2 - if x.is_multiple_of(4) { 18 } else { 0 },
                    255,
                ),
                Material::AcaciaLogSide => color(
                    [103, 100, 89],
                    shade / 2 - if x.is_multiple_of(5) { 22 } else { 0 },
                    255,
                ),
                Material::BirchLogTop
                | Material::SpruceLogTop
                | Material::AcaciaLogTop
                | Material::JungleLogTop => {
                    let rgb = match material {
                        Material::BirchLogTop => [210, 192, 143],
                        Material::AcaciaLogTop => [177, 96, 58],
                        Material::JungleLogTop => [170, 133, 83],
                        _ => [132, 101, 66],
                    };
                    let ring = (((x as i32 - 8).pow(2) + (y as i32 - 8).pow(2)) as u32).isqrt();
                    color(
                        rgb,
                        shade / 3 - if ring.is_multiple_of(3) { 26 } else { 0 },
                        255,
                    )
                }
                Material::RedSand => color([190, 106, 50], shade / 2, 255),
                Material::Terracotta => color(
                    [161, 83, 56],
                    shade / 3 - if y % 6 < 2 { 23 } else { 0 },
                    255,
                ),
                Material::Podzol => color(
                    [104, 75, 42],
                    shade + if (x + y) % 5 == 0 { 16 } else { 0 },
                    255,
                ),
                Material::Cactus => color(
                    [69, 129, 48],
                    shade / 2 - if x.is_multiple_of(4) { 24 } else { 0 },
                    255,
                ),
                Material::DeadBush => {
                    let stem = x == 8 && y > 5
                        || (3..14).contains(&x) && (y == 7 + x / 3 || y == 15 - x / 2);
                    color([135, 98, 52], shade, if stem { 255 } else { 0 })
                }
                Material::Bamboo => {
                    let stalk = (6..=9).contains(&x);
                    let branch =
                        (y == 4 && (3..=12).contains(&x)) || (y == 11 && (5..=14).contains(&x));
                    color(
                        [213; 3],
                        shade / 2 - if y.is_multiple_of(7) { 35 } else { 0 },
                        if stalk || branch { 255 } else { 0 },
                    )
                }
                Material::Melon => color(
                    [102, 149, 45],
                    shade / 2 - if x % 4 < 2 { 28 } else { 0 },
                    255,
                ),
                Material::Pumpkin => color(
                    [215, 130, 36],
                    shade / 2 - if x % 5 == 0 { 40 } else { 0 },
                    255,
                ),
                Material::Cocoa => color(
                    [126, 71, 36],
                    shade / 2 - if x % 4 == 0 || y % 7 == 0 { 20 } else { 0 },
                    255,
                ),
                Material::RedMushroom | Material::BrownMushroom => {
                    let dx = (x as i32 - 8).abs();
                    if (4..=8).contains(&y) && dx <= (y as i32 - 2).min(6) {
                        let rgb = if material == Material::RedMushroom {
                            if (x + y * 3) % 7 < 2 {
                                [237, 219, 190]
                            } else {
                                [181, 43, 35]
                            }
                        } else {
                            [139, 101, 69]
                        };
                        color(rgb, shade / 2, 255)
                    } else if (9..=14).contains(&y) && (7..=9).contains(&x) {
                        color([211, 188, 151], shade / 3, 255)
                    } else {
                        [0; 4]
                    }
                }
                Material::Sunflower => {
                    let dx = (x as i32 - 8).abs();
                    let dy = (y as i32 - 5).abs();
                    if dx * dx + dy * dy <= 20 {
                        color(
                            if dx * dx + dy * dy <= 5 {
                                [100, 67, 32]
                            } else {
                                [243, 192, 43]
                            },
                            shade / 3,
                            255,
                        )
                    } else if x == 8 && y >= 9 || (y == 12 && (4..=8).contains(&x)) {
                        color([64, 123, 37], shade / 2, 255)
                    } else {
                        [0; 4]
                    }
                }
                Material::SugarCane => {
                    let stalk = x % 5 == 2 || (y % 6 == 2 && (x + y / 3) % 5 < 3);
                    color(
                        [211; 3],
                        shade / 2 - if y.is_multiple_of(6) { 27 } else { 0 },
                        if stalk { 255 } else { 0 },
                    )
                }
                Material::OakPlanks | Material::SprucePlanks => {
                    let rgb = if material == Material::OakPlanks {
                        [174, 137, 83]
                    } else {
                        [107, 80, 47]
                    };
                    let seam = y.is_multiple_of(4) || (x + (y / 4 % 2) * 7).is_multiple_of(15);
                    color(rgb, shade / 3 - if seam { 37 } else { 0 }, 255)
                }
                Material::Cobblestone => {
                    let stagger = y / 5 % 2 * 3;
                    let seam = y % 5 == 0 || (x + stagger) % 6 == 0;
                    color([119, 122, 116], shade / 2 - if seam { 42 } else { 0 }, 255)
                }
                Material::RoofTiles => {
                    let seam = y.is_multiple_of(4) || (x + y / 4 % 2 * 4).is_multiple_of(8);
                    color([151, 75, 51], shade / 3 - if seam { 40 } else { 0 }, 255)
                }
                Material::Glass => {
                    let frame = x == 0 || y == 0 || x == 15 || y == 15;
                    let glint = (x + y == 13 || x + y == 19) && x > 2 && x < 12 && y > 2 && y < 12;
                    color(
                        [189, 221, 228],
                        shade / 4,
                        if frame || glint { 255 } else { 0 },
                    )
                }
                Material::Farmland => color(
                    [96, 63, 36],
                    shade / 2 - if x % 4 == 0 { 29 } else { 0 },
                    255,
                ),
                Material::Wheat => {
                    let stalk = x % 4 == 1 && y >= 5;
                    let head = y > 1 && y < 8 && (x + y % 2) % 4 < 2;
                    color(
                        if y < 8 {
                            [208, 175, 65]
                        } else {
                            [119, 139, 46]
                        },
                        shade / 2,
                        if stalk || head { 255 } else { 0 },
                    )
                }
                Material::HayBale => {
                    let strap = y == 3 || y == 4 || y == 11 || y == 12;
                    color(
                        if strap { [150, 88, 37] } else { [188, 161, 53] },
                        shade / 2 - if x % 3 == 0 { 17 } else { 0 },
                        255,
                    )
                }
                Material::Path => color(
                    [149, 123, 79],
                    shade / 2 - if grain.is_multiple_of(11) { 19 } else { 0 },
                    255,
                ),
                Material::Sandstone => color(
                    [207, 192, 143],
                    shade / 3
                        - if y.is_multiple_of(8) || (x + y / 8 * 5).is_multiple_of(13) {
                            18
                        } else {
                            0
                        },
                    255,
                ),
                Material::Snow => color([239, 245, 249], shade / 3, 255),
                Material::CoalOre | Material::IronOre => {
                    let ore = hash(x / 3, y / 3, 17) % 9 < 2 && (x + y) % 3 != 0;
                    if ore {
                        let rgb = if material == Material::CoalOre {
                            [48, 51, 49]
                        } else {
                            [188, 145, 109]
                        };
                        color(rgb, shade / 2, 255)
                    } else {
                        color([125, 129, 127], shade / 2, 255)
                    }
                }
                Material::Gravel => color(
                    [131, 123, 116],
                    shade + (hash(x / 2, y / 2, 23) % 35) as i32 - 17,
                    255,
                ),
                Material::Clay => color(
                    [157, 169, 177],
                    shade / 4 + if y.is_multiple_of(5) { -7 } else { 0 },
                    255,
                ),
            };
            pixels.extend_from_slice(&rgba);
        }
    }
    pixels
}
