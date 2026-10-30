//! Bounded original village templates for the isolated preview, not vanilla jigsaw data.
use crate::{
    ecology::{Ecology, SceneBiome},
    materials::Material,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VillageMode {
    Off,
    #[default]
    Auto,
    Showcase,
}
impl VillageMode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Auto => "AUTO",
            Self::Showcase => "SHOWCASE",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Showcase,
            Self::Showcase => Self::Off,
            Self::Off => Self::Auto,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "auto" => Some(Self::Auto),
            "showcase" => Some(Self::Showcase),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct VillageScene {
    pub blocks: BTreeMap<[i32; 3], Material>,
    pub crops: Vec<[i32; 3]>,
    pub villages: u32,
    pub houses: u32,
    pub farms: u32,
    reserved: Vec<[i32; 4]>,
}
impl VillageScene {
    pub fn reserved(&self, x: i32, z: i32) -> bool {
        self.reserved
            .iter()
            .any(|r| x >= r[0] && x <= r[2] && z >= r[1] && z <= r[3])
    }
    /// Complete world-cell plans are evaluated before clipping; request order cannot
    /// change the plots, heights or blocks on a shared boundary. Showcase is anchored
    /// at world origin, not the current camera or request centre.
    pub fn generate(
        seed: i64,
        origin: [i32; 2],
        size: u32,
        mode: VillageMode,
        ecology: &Ecology,
        surface: impl Fn(i32, i32) -> f32,
        cancelled: impl Fn() -> bool,
    ) -> Option<Self> {
        let mut out = Self::default();
        if size == 0 || size > 256 || origin.iter().any(|v| v.unsigned_abs() > 29_999_000) {
            return None;
        }
        if mode == VillageMode::Off {
            return Some(out);
        }
        let end = [origin[0] + size as i32, origin[1] + size as i32];
        let intersects = |x: i32, z: i32, radius: i32| {
            x + radius >= origin[0]
                && z + radius >= origin[1]
                && x - radius < end[0]
                && z - radius < end[1]
        };
        let mut centres = Vec::new();
        if mode == VillageMode::Showcase {
            centres.push((0, 0, mix(seed, 0, 0)));
        } else {
            // Structure radius27 + centre jitter16 needs a >=43-block query halo.
            for cz in (origin[1] - 44).div_euclid(192)..=(end[1] + 44).div_euclid(192) {
                for cx in (origin[0] - 44).div_euclid(192)..=(end[0] + 44).div_euclid(192) {
                    let id = mix(seed, cx, cz);
                    if (id >> 16) % 100 < 70 {
                        centres.push((
                            cx * 192 + (id % 33) as i32 - 16,
                            cz * 192 + ((id >> 8) % 33) as i32 - 16,
                            id,
                        ));
                    }
                }
            }
        }
        for (x, z, id) in centres {
            if cancelled() {
                return None;
            }
            if !intersects(x, z, 27) {
                continue;
            }
            let h = surface(x, z);
            if !h.is_finite() || !(11.0..=104.0).contains(&h) {
                continue;
            }
            let biome = ecology.biome(x, z, h);
            if mode == VillageMode::Auto
                && !matches!(
                    biome,
                    SceneBiome::Meadow
                        | SceneBiome::Desert
                        | SceneBiome::Savanna
                        | SceneBiome::Taiga
                        | SceneBiome::Alpine
                )
            {
                continue;
            }
            let mut village = Self::default();
            let (walls, posts, roof) = match biome {
                SceneBiome::Desert | SceneBiome::Badlands => (
                    Material::Sandstone,
                    Material::Sandstone,
                    Material::Sandstone,
                ),
                SceneBiome::Taiga | SceneBiome::Alpine => (
                    Material::SprucePlanks,
                    Material::SpruceLogSide,
                    Material::SprucePlanks,
                ),
                SceneBiome::Savanna => (
                    Material::OakPlanks,
                    Material::AcaciaLogSide,
                    Material::RoofTiles,
                ),
                SceneBiome::Jungle => (
                    Material::OakPlanks,
                    Material::JungleLogSide,
                    Material::RoofTiles,
                ),
                _ => (
                    Material::OakPlanks,
                    Material::OakLogSide,
                    Material::RoofTiles,
                ),
            };
            let mut visible_houses = 0;
            for (plot, (dx, dz)) in [(-13, -12), (13, -12), (-13, 12), (13, 12)]
                .into_iter()
                .enumerate()
            {
                if cancelled() {
                    return None;
                }
                let px = x + dx;
                let pz = z + dz;
                let Some(floor) = pad(&mut village, px, pz, 4, 4, &surface, mode) else {
                    continue;
                };
                let front = if dz < 0 { 1 } else { -1 };
                for ly in 1..=4 {
                    for lz in -3_i32..=3 {
                        for lx in -3_i32..=3 {
                            if lx.abs() != 3 && lz.abs() != 3 {
                                continue;
                            }
                            if lz == front * 3 && lx == 0 && ly <= 2 {
                                continue;
                            }
                            let material = if lx.abs() == 3 && lz.abs() == 3 {
                                posts
                            } else if ly == 2 && (lx == 0 || lz == 0) {
                                Material::Glass
                            } else {
                                walls
                            };
                            village
                                .blocks
                                .insert([px + lx, floor + ly, pz + lz], material);
                        }
                    }
                }
                // Hollow gable roof, or flat sandstone roof in desert style.
                for lx in -4_i32..=4 {
                    let rise = if walls == Material::Sandstone {
                        0
                    } else {
                        4 - lx.abs()
                    };
                    for lz in -4..=4 {
                        village
                            .blocks
                            .insert([px + lx, floor + 5 + rise, pz + lz], roof);
                    }
                    if rise > 0 {
                        for dy in 0..rise {
                            for lz in [-3, 3] {
                                village
                                    .blocks
                                    .insert([px + lx, floor + 5 + dy, pz + lz], walls);
                            }
                        }
                    }
                }
                if (id >> plot).is_multiple_of(2) && walls != Material::Sandstone {
                    for dy in 4..=10 {
                        village
                            .blocks
                            .insert([px + 2, floor + dy, pz + 1], Material::Cobblestone);
                    }
                }
                // Interior furnishing and harvest details.
                village
                    .blocks
                    .insert([px - 2, floor + 1, pz + 2], Material::HayBale);
                // Three-wide porch and a terrain-following route to the main street.
                for distance in 4..=12 {
                    for width in -1..=1 {
                        let qx = px + width;
                        let qz = pz + front * distance;
                        if let Some(ground) = ground(&surface, qx, qz) {
                            let top = if distance == 4 { floor } else { ground - 1 };
                            village.blocks.insert([qx, top, qz], Material::Path);
                        }
                    }
                }
                village.houses += 1;
                if intersects(px, pz, 4) {
                    visible_houses += 1;
                }
            }
            if village.houses < 2 {
                continue;
            }
            // Main crossroads follow the current surface instead of floating above it.
            for distance in -24..=24 {
                for width in -1..=1 {
                    for (qx, qz) in [(x + distance, z + width), (x + width, z + distance)] {
                        if let Some(ground) = ground(&surface, qx, qz) {
                            village.blocks.insert([qx, ground - 1, qz], Material::Path);
                        }
                    }
                }
            }
            if let Some(floor) = pad(&mut village, x + 23, z + 12, 4, 3, &surface, mode) {
                for dz in -3..=3 {
                    for dx in -4..=4 {
                        let p = [x + 23 + dx, floor, z + 12 + dz];
                        let mat = if dx == 0 {
                            Material::Water
                        } else if dz.abs() == 3 || dx.abs() == 4 {
                            posts
                        } else {
                            Material::Farmland
                        };
                        village.blocks.insert(p, mat);
                        if mat == Material::Farmland {
                            village.crops.push([p[0], p[1] + 1, p[2]]);
                        }
                    }
                }
                village.farms = u32::from(intersects(x + 23, z + 12, 4));
            }
            if let Some(floor) = pad(&mut village, x, z, 2, 2, &surface, mode) {
                for dz in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        village.blocks.insert(
                            [x + dx, floor + 1, z + dz],
                            if dx == 0 && dz == 0 {
                                Material::Water
                            } else {
                                Material::Cobblestone
                            },
                        );
                    }
                }
                for dx in [-2, 2] {
                    for dz in [-2, 2] {
                        for dy in 1..=4 {
                            village.blocks.insert([x + dx, floor + dy, z + dz], posts);
                        }
                    }
                }
                for dx in -2..=2 {
                    for dz in -2..=2 {
                        village.blocks.insert([x + dx, floor + 5, z + dz], roof);
                    }
                }
            }
            if let Some(ground) = ground(&surface, x + 20, z + 2) {
                village
                    .blocks
                    .insert([x + 20, ground, z + 2], Material::HayBale);
                village
                    .blocks
                    .insert([x + 20, ground + 1, z + 2], Material::HayBale);
            }
            out.villages += 1;
            out.houses += visible_houses;
            out.farms += village.farms;
            out.reserved.push([x - 27, z - 27, x + 27, z + 27]);
            out.blocks.extend(village.blocks);
            out.crops.extend(village.crops);
            if out.blocks.len() > 150_000 {
                return None;
            }
        }
        let inside = |p: &[i32; 3]| {
            p[0] >= origin[0]
                && p[2] >= origin[1]
                && p[0] < end[0]
                && p[2] < end[1]
                && (0..128).contains(&p[1])
        };
        out.blocks.retain(|p, _| inside(p));
        out.crops.retain(inside);
        Some(out)
    }
}
fn ground(surface: &impl Fn(i32, i32) -> f32, x: i32, z: i32) -> Option<i32> {
    let y = surface(x, z);
    (y.is_finite() && (10.0..=104.0).contains(&y)).then(|| y.round() as i32)
}
fn pad(
    scene: &mut VillageScene,
    x: i32,
    z: i32,
    rx: i32,
    rz: i32,
    surface: &impl Fn(i32, i32) -> f32,
    mode: VillageMode,
) -> Option<i32> {
    let mut samples = Vec::new();
    let mut low = 128;
    let mut high = 0;
    for dz in -rz..=rz {
        for dx in -rx..=rx {
            let h = ground(surface, x + dx, z + dz)?;
            low = low.min(h);
            high = high.max(h);
            samples.push((x + dx, z + dz, h));
        }
    }
    if high - low > if mode == VillageMode::Showcase { 16 } else { 8 } {
        return None;
    }
    for (px, pz, h) in samples {
        for y in h - 1..=high {
            scene.blocks.insert([px, y, pz], Material::Cobblestone);
        }
    }
    Some(high)
}
fn mix(seed: i64, x: i32, z: i32) -> u64 {
    let mut v = seed as u64
        ^ (x as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as u32 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb)
}
