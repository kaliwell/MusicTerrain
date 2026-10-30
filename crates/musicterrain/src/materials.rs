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
}

impl Material {
    pub const ALL: [Self; 17] = [
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
            Self::Water => [78, 145, 225],
            _ => [255; 3],
        }
    }

    pub const fn translucent(self) -> bool {
        matches!(self, Self::Water)
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
                Material::OakLeaves => color(
                    [190; 3],
                    shade + (hash(x / 3, y / 3, 9) % 43) as i32 - 21,
                    if grain.is_multiple_of(7) { 0 } else { 255 },
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
