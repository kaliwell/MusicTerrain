#![cfg(feature = "native")]

use musicterrain::materials::{Material, TextureAtlas};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "musicterrain-materials-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, width: u32, height: u32, pixels: &[u8]) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, png(width, height, pixels)).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Small original RGBA fixtures: PNG chunks + a stored (uncompressed) zlib block.
// No production dependency or external image encoder is required by these tests.
fn png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    let mut raw = Vec::new();
    for row in pixels.chunks_exact((width * 4) as usize) {
        raw.push(0); // PNG filter: none.
        raw.extend_from_slice(row);
    }
    assert!(raw.len() < u16::MAX as usize);
    let mut zlib = vec![0x78, 0x01, 0x01];
    let length = raw.len() as u16;
    zlib.extend_from_slice(&length.to_le_bytes());
    zlib.extend_from_slice(&(!length).to_le_bytes());
    zlib.extend_from_slice(&raw);
    let (mut a, mut b) = (1_u32, 0_u32);
    for value in raw {
        a = (a + value as u32) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(bytes);
    let mut crc = u32::MAX;
    for &byte in kind.iter().chain(bytes) {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

fn layer(atlas: &TextureAtlas, material: Material) -> &[u8] {
    let size = (atlas.size * atlas.size * 4) as usize;
    let start = material.layer() as usize * size;
    &atlas.pixels[start..start + size]
}

#[test]
fn original_pixel_art_has_stable_distinct_layers() {
    let atlas = TextureAtlas::load(None).unwrap();
    assert_eq!(
        (atlas.size, atlas.layers, atlas.loaded, atlas.fallback),
        (32, Material::ALL.len() as u32, 0, Material::ALL.len())
    );
    assert_eq!(atlas.pixels.len(), 32 * 32 * 4 * Material::ALL.len());
    assert_eq!(atlas.missing, Material::ALL);
    let repeated = TextureAtlas::load(None).unwrap();
    assert_eq!(atlas.pixels, repeated.pixels);
    for (index, material) in Material::ALL.into_iter().enumerate() {
        assert_eq!(material.layer() as usize, index);
        let pixels = layer(&atlas, material);
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[..3] != [255; 3]));
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p != &pixels[..4]));
        for other in Material::ALL.into_iter().skip(index + 1) {
            assert_ne!(pixels, layer(&atlas, other), "{material:?} and {other:?}");
        }
    }
}

#[test]
fn leaves_plants_and_water_keep_their_rendering_alpha() {
    let atlas = TextureAtlas::load(None).unwrap();
    for material in [
        Material::OakLeaves,
        Material::BirchLeaves,
        Material::SpruceLeaves,
        Material::AcaciaLeaves,
        Material::JungleLeaves,
        Material::Vine,
        Material::Fern,
        Material::DeadBush,
        Material::TallGrass,
        Material::Poppy,
        Material::Dandelion,
        Material::Bamboo,
        Material::RedMushroom,
        Material::BrownMushroom,
        Material::Sunflower,
        Material::SugarCane,
        Material::Wheat,
        Material::Glass,
    ] {
        let pixels = layer(&atlas, material);
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        assert!(!material.translucent());
    }
    assert!(
        layer(&atlas, Material::Water)
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] > 0 && p[3] < 255)
    );
    for material in Material::ALL {
        assert_eq!(material.translucent(), material == Material::Water);
    }
    assert_eq!(Material::GrassSide.tint(), [255; 3]);
    assert!(Material::GrassTop.tint()[1] > Material::GrassTop.tint()[0]);
}

#[test]
fn real_png_loads_from_pack_prepared_textures_and_direct_roots() {
    let fixture = Fixture::new();
    fixture.write(
        "assets/minecraft/textures/block/stone.png",
        4,
        4,
        &[41, 82, 123, 255].repeat(16),
    );
    for root in [
        fixture.0.clone(),
        fixture.0.join("assets"),
        fixture.0.join("assets/minecraft"),
        fixture.0.join("assets/minecraft/textures"),
        fixture.0.join("assets/minecraft/textures/block"),
    ] {
        let atlas = TextureAtlas::load(Some(&root)).unwrap();
        assert_eq!(
            (atlas.loaded, atlas.fallback, atlas.missing.len()),
            (1, Material::ALL.len() - 1, Material::ALL.len() - 1)
        );
        assert!(
            layer(&atlas, Material::Stone)
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [41, 82, 123, 255])
        );
    }
}

#[test]
fn nearest_first_animation_frame_and_native_alpha_are_preserved() {
    let fixture = Fixture::new();
    let mut water = [200, 210, 220, 255].repeat(16);
    water.extend([20, 30, 40, 255].repeat(16));
    fixture.write("water_still.png", 4, 8, &water);
    let leaves: Vec<u8> = (0..16)
        .flat_map(|i| [175, 175, 175, if i % 2 == 0 { 0 } else { 255 }])
        .collect();
    fixture.write("oak_leaves.png", 4, 4, &leaves);
    let atlas = TextureAtlas::load(Some(&fixture.0)).unwrap();
    assert_eq!(atlas.loaded, 2);
    assert!(
        layer(&atlas, Material::Water)
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [200, 210, 220, 160])
    );
    let pixels = layer(&atlas, Material::OakLeaves);
    assert!(pixels.as_chunks::<4>().0.contains(&[175, 175, 175, 0]));
    assert!(pixels.as_chunks::<4>().0.contains(&[175, 175, 175, 255]));
    // Nearest sampling turns each native texel into an 8x8 block.
    for row in pixels.as_chunks::<128>().0 {
        assert!(row[..8 * 4].as_chunks::<4>().0.iter().all(|p| p[3] == 0));
        assert!(
            row[8 * 4..16 * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 255)
        );
    }
}

#[test]
fn native_grass_overlay_is_tinted_without_turning_dirt_green() {
    let fixture = Fixture::new();
    fixture.write("grass_block_side.png", 4, 4, &[145, 94, 53, 255].repeat(16));
    let overlay: Vec<_> = (0..16)
        .flat_map(|i| [255, 255, 255, if i < 8 { 255 } else { 0 }])
        .collect();
    fixture.write("grass_block_side_overlay.png", 4, 4, &overlay);
    fixture.write(
        "grass_block_top.png",
        4,
        4,
        &[204, 204, 204, 255].repeat(16),
    );
    let atlas = TextureAtlas::load(Some(&fixture.0)).unwrap();
    assert!(atlas.grass_overlay_loaded);
    let side = layer(&atlas, Material::GrassSide);
    assert_eq!(&side[..4], &[112, 169, 76, 255]);
    assert_eq!(&side[side.len() - 4..], &[145, 94, 53, 255]);
    assert_eq!(
        &layer(&atlas, Material::GrassTop)[..4],
        &[204, 204, 204, 255]
    );
}

#[test]
fn invalid_roots_corrupt_pngs_and_resource_budgets_produce_clear_errors() {
    let fixture = Fixture::new();
    assert!(TextureAtlas::load(Some(&fixture.0)).is_err());
    assert!(TextureAtlas::load(Some(&fixture.0.join("absent"))).is_err());
    let path = fixture.write("stone.png", 4, 4, &[100, 100, 100, 255].repeat(16));
    assert!(TextureAtlas::load(Some(&path)).is_err());
    fs::write(&path, [7; 64]).unwrap();
    let error = TextureAtlas::load(Some(&fixture.0))
        .unwrap_err()
        .to_string();
    assert!(error.contains("stone.png") && error.contains("PNG"));
    assert!(!error.contains(fixture.0.to_str().unwrap()));
    let mut oversized = png(4, 4, &[100, 100, 100, 255].repeat(16));
    oversized[16..20].copy_from_slice(&8192_u32.to_be_bytes());
    fs::write(&path, oversized).unwrap();
    let error = TextureAtlas::load(Some(&fixture.0))
        .unwrap_err()
        .to_string();
    assert!(error.contains("dimensions"));
    fs::File::create(&path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        TextureAtlas::load(Some(&fixture.0))
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
    fixture.write("stone.png", 3, 4, &[100, 100, 100, 255].repeat(12));
    assert!(
        TextureAtlas::load(Some(&fixture.0))
            .unwrap_err()
            .to_string()
            .contains("square")
    );
    // The only filenames the loader constructs are fixed material names under the root.
    assert!(
        TextureAtlas::load(Some(Path::new("/a/nonexistent/musicterrain-texture-root"))).is_err()
    );
}

#[test]
fn biome_and_village_layers_append_ids_and_keep_solid_and_glass_contracts() {
    let atlas = TextureAtlas::load(None).unwrap();
    assert_eq!(Material::Bamboo.layer(), 36);
    assert_eq!(Material::SprucePlanks.layer(), 53);
    assert_eq!(Material::ALL.len(), 54);
    for material in [
        Material::Melon,
        Material::Pumpkin,
        Material::Cocoa,
        Material::OakPlanks,
        Material::Cobblestone,
        Material::RoofTiles,
        Material::Farmland,
        Material::HayBale,
        Material::Path,
        Material::Sandstone,
        Material::SprucePlanks,
    ] {
        assert!(
            layer(&atlas, material)
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 255),
            "solid {material:?} has cutouts"
        );
    }
    let glass = layer(&atlas, Material::Glass);
    assert_eq!(Material::Glass.tint(), [255; 3]);
    assert!(!Material::Glass.translucent());
    assert_eq!(glass[3], 255); // Outer frame is opaque.
    assert_eq!(glass[((8 * 32 + 8) * 4 + 3) as usize], 0); // Clear interior.
}
