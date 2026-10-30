//! Seeded preview ecology, independent of native materials and vanilla generation.
//!
//! Heights are the first air block above a column, matching the preview mesher.
use horizon_core::{noise::SimplexNoise, random::LegacyRandom};
use serde::{Deserialize, Serialize};

pub const WATER_LEVEL: f32 = 8.0;
pub const MAX_SCENE_SIZE: u32 = 256;
const TREE_CELL: i32 = 14;
const PLANT_CELL: i32 = 6;
const CAVE_CELL: i32 = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneBiome {
    Meadow,
    Forest,
    Alpine,
    Shore,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    Grass,
    Dirt,
    Stone,
    Sand,
    Snow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceProfile {
    pub biome: SceneBiome,
    pub top: SurfaceKind,
    pub subsoil: SurfaceKind,
    pub stone_depth: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationKind {
    OakTree,
    TallGrass,
    Poppy,
    Dandelion,
    Rock,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldDecoration {
    pub anchor: [i32; 3],
    pub kind: DecorationKind,
    pub event_id: u64,
    /// Trunk height for trees, block height for rocks/plants.
    pub scale: u8,
}
pub type Decoration = WorldDecoration;

/// Allocation-free column intersection of the preview cave network.
#[derive(Clone, Debug)]
pub struct CaveColumn {
    ranges: [[f32; 2]; 27],
    count: usize,
}
impl Default for CaveColumn {
    fn default() -> Self {
        Self {
            ranges: [[0.0; 2]; 27],
            count: 0,
        }
    }
}
impl CaveColumn {
    pub fn contains(&self, y: i32) -> bool {
        let y = y as f32;
        self.ranges[..self.count]
            .iter()
            .any(|r| y >= r[0] && y <= r[1])
    }
    pub fn intervals(&self) -> &[[f32; 2]] {
        &self.ranges[..self.count]
    }
    fn add(&mut self, bottom: f32, top: f32) {
        if self.count < self.ranges.len() && bottom <= top {
            self.ranges[self.count] = [bottom, top];
            self.count += 1;
        }
    }
}
#[derive(Clone, Debug)]
pub struct Ecology {
    seed: i64,
    climate: SimplexNoise,
}
impl Ecology {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            climate: SimplexNoise::new(&mut LegacyRandom::new(seed ^ 0x4543_4f4c_4f47_5901)),
        }
    }
    fn hash(&self, x: i32, z: i32, salt: u64) -> u64 {
        // World bounds keep cell coordinates inside signed 24 bits. Preserve both
        // axes: vanilla position_seed has collisions for mirrored X/Z positions.
        let key = ((x as u32 as u64 & 0x00ff_ffff) << 24) | (z as u32 as u64 & 0x00ff_ffff);
        let mut v = key ^ ((salt & 0xffff) << 48) ^ (self.seed as u64).rotate_left(23);
        v = v.wrapping_add(0x9e37_79b9_7f4a_7c15);
        v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        v = (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        v ^ (v >> 31)
    }
    pub fn biome(&self, x: i32, z: i32, height: f32) -> SceneBiome {
        if !height.is_finite() || height <= WATER_LEVEL + 1.0 {
            return SceneBiome::Shore;
        }
        let climate = self
            .climate
            .get2(f64::from(x) * 0.011, f64::from(z) * 0.011);
        if height >= 62.0 + climate * 8.0 {
            SceneBiome::Alpine
        } else if climate > -0.12 {
            SceneBiome::Forest
        } else {
            SceneBiome::Meadow
        }
    }
    pub fn surface_profile(&self, x: i32, z: i32, height: f32) -> SurfaceProfile {
        let biome = self.biome(x, z, height);
        let (top, subsoil, stone_depth) = match biome {
            SceneBiome::Shore => (SurfaceKind::Sand, SurfaceKind::Sand, 3),
            SceneBiome::Alpine => (SurfaceKind::Snow, SurfaceKind::Stone, 1),
            SceneBiome::Meadow | SceneBiome::Forest => (SurfaceKind::Grass, SurfaceKind::Dirt, 3),
        };
        SurfaceProfile {
            biome,
            top,
            subsoil,
            stone_depth,
        }
    }
    fn tree(
        &self,
        cx: i32,
        cz: i32,
        surface: &impl Fn(i32, i32) -> f32,
    ) -> Option<WorldDecoration> {
        let id = self.hash(cx, cz, 0x5452_4545);
        let x = cx * TREE_CELL + 4 + (id % 6) as i32;
        let z = cz * TREE_CELL + 4 + ((id >> 8) % 6) as i32;
        let height = surface(x, z);
        if !height.is_finite() || !(WATER_LEVEL + 3.0..=120.0).contains(&height) {
            return None;
        }
        let threshold = match self.biome(x, z, height) {
            SceneBiome::Forest => 78,
            SceneBiome::Meadow => 32,
            _ => 0,
        };
        if (id >> 16) % 100 >= threshold {
            return None;
        }
        let h = height.round() as i32;
        if self.cave_column(x, z, height).contains(h - 1) {
            return None;
        }
        for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
            let nearby = surface(x + dx, z + dz);
            if !nearby.is_finite() || (nearby - height).abs() > 2.75 {
                return None;
            }
        }
        Some(WorldDecoration {
            anchor: [x, h, z],
            kind: DecorationKind::OakTree,
            event_id: id,
            scale: 4 + ((id >> 32) % 3) as u8,
        })
    }
    /// Returns all anchor plants plus trees whose radius-3 crown intersects the region.
    /// Shared edge trees have identical event IDs; callers clip geometry to their own region.
    /// Fixed world grids bound requests to fewer than 2,500 objects at 256².
    pub fn decorations(
        &self,
        origin: [i32; 2],
        size: u32,
        surface: impl Fn(i32, i32) -> f32,
    ) -> Vec<WorldDecoration> {
        if size == 0
            || size > MAX_SCENE_SIZE
            || origin.iter().any(|v| v.unsigned_abs() > 29_999_000)
        {
            return Vec::new();
        }
        let [ox, oz] = origin;
        let end_x = ox + size as i32;
        let end_z = oz + size as i32;
        let mut out = Vec::new();
        for cz in (oz - 3).div_euclid(TREE_CELL)..=(end_z + 2).div_euclid(TREE_CELL) {
            for cx in (ox - 3).div_euclid(TREE_CELL)..=(end_x + 2).div_euclid(TREE_CELL) {
                if let Some(tree) = self.tree(cx, cz, &surface) {
                    let [x, _, z] = tree.anchor;
                    if x + 3 >= ox && x - 3 < end_x && z + 3 >= oz && z - 3 < end_z {
                        out.push(tree);
                    }
                }
            }
        }
        let tree_count = out.len();
        for cz in oz.div_euclid(PLANT_CELL)..=(end_z - 1).div_euclid(PLANT_CELL) {
            for cx in ox.div_euclid(PLANT_CELL)..=(end_x - 1).div_euclid(PLANT_CELL) {
                let id = self.hash(cx, cz, 0x504c_414e_5453);
                let x = cx * PLANT_CELL + (id % 6) as i32;
                let z = cz * PLANT_CELL + ((id >> 8) % 6) as i32;
                if x < ox || x >= end_x || z < oz || z >= end_z || (id >> 16) % 100 > 86 {
                    continue;
                }
                let h = surface(x, z);
                if !h.is_finite() || !(WATER_LEVEL + 2.0..=126.0).contains(&h) {
                    continue;
                }
                let y = h.round() as i32;
                if self.cave_column(x, z, h).contains(y - 1) {
                    continue;
                }
                if out[..tree_count]
                    .iter()
                    .any(|tree| (tree.anchor[0] - x).abs() <= 2 && (tree.anchor[2] - z).abs() <= 2)
                {
                    continue;
                }
                let neighbor = surface(x + 1, z);
                if !neighbor.is_finite() || (neighbor - h).abs() > 2.0 {
                    continue;
                }
                let kind = match self.biome(x, z, h) {
                    SceneBiome::Shore => continue,
                    SceneBiome::Alpine => {
                        if !(id >> 24).is_multiple_of(4) {
                            continue;
                        }
                        DecorationKind::Rock
                    }
                    _ => match (id >> 24) % 10 {
                        0 | 1 => DecorationKind::Poppy,
                        2 | 3 => DecorationKind::Dandelion,
                        _ => DecorationKind::TallGrass,
                    },
                };
                out.push(WorldDecoration {
                    anchor: [x, y, z],
                    kind,
                    event_id: id,
                    scale: 1,
                });
            }
        }
        out
    }
    /// Compute once per X/Z column, then use `contains` in the voxel loop.
    pub fn cave_column(&self, x: i32, z: i32, surface: f32) -> CaveColumn {
        self.cave_column_at(f64::from(x), f64::from(z), surface)
    }
    fn cave_column_at(&self, x: f64, z: f64, surface: f32) -> CaveColumn {
        let mut out = CaveColumn::default();
        if !surface.is_finite()
            || surface <= WATER_LEVEL + 2.0
            || x.abs() > 30_000_000.0
            || z.abs() > 30_000_000.0
        {
            return out;
        }
        let cell_x = (x / f64::from(CAVE_CELL)).floor() as i32;
        let cell_z = (z / f64::from(CAVE_CELL)).floor() as i32;
        for cz in cell_z - 1..=cell_z + 1 {
            for cx in cell_x - 1..=cell_x + 1 {
                let id = self.hash(cx, cz, 0x4341_5645);
                let px = cx * CAVE_CELL + 19 + (id % 11) as i32;
                let pz = cz * CAVE_CELL + 19 + ((id >> 8) % 11) as i32;
                let dx = (x - f64::from(px)) as f32;
                let dz = (z - f64::from(pz)) as f32;
                // Entrance and shaft meet the two descending tunnel branches.
                let shaft_radius = 2.3;
                if dx * dx + dz * dz <= shaft_radius * shaft_radius {
                    out.add(surface - 9.0, surface + 0.5);
                }
                let sign = if (id >> 16) & 1 == 0 { 1.0 } else { -1.0 };
                for (vx, vz, depth, radius) in [
                    (18.0 * sign, 7.0, 8.0, 2.6),
                    (-11.0 * sign, 15.0, 10.0, 2.2),
                ] {
                    let projection = ((dx * vx + dz * vz) / (vx * vx + vz * vz)).clamp(0.0, 1.0);
                    let rx = dx - vx * projection;
                    let rz = dz - vz * projection;
                    let distance = rx * rx + rz * rz;
                    if distance < radius * radius {
                        let half = (radius * radius - distance).sqrt();
                        let center = surface - 6.0 - projection * depth;
                        out.add(center - half, center + half);
                    }
                }
            }
        }
        out
    }
    pub fn is_cave(&self, x: i32, y: i32, z: i32, surface: f32) -> bool {
        self.cave_column(x, z, surface).contains(y)
    }
    /// Continuous signed cave field (not an exact Euclidean SDF), negative inside.
    /// The cached column and this field share the same shaft/tunnel intersections.
    pub fn cave_density(&self, x: f64, y: f64, z: f64, surface: f32) -> f32 {
        if !x.is_finite()
            || !y.is_finite()
            || !z.is_finite()
            || !surface.is_finite()
            || surface <= WATER_LEVEL + 2.0
            || x.abs() > 30_000_000.0
            || z.abs() > 30_000_000.0
            || y.abs() > 30_000_000.0
        {
            return 64.0;
        }
        let cell_x = (x / f64::from(CAVE_CELL)).floor() as i32;
        let cell_z = (z / f64::from(CAVE_CELL)).floor() as i32;
        let depth = surface - y as f32;
        let mut density = 64.0_f32;
        for cz in cell_z - 1..=cell_z + 1 {
            for cx in cell_x - 1..=cell_x + 1 {
                let id = self.hash(cx, cz, 0x4341_5645);
                let px = cx * CAVE_CELL + 19 + (id % 11) as i32;
                let pz = cz * CAVE_CELL + 19 + ((id >> 8) % 11) as i32;
                let dx = (x - f64::from(px)) as f32;
                let dz = (z - f64::from(pz)) as f32;
                let shaft = ((dx * dx + dz * dz).sqrt() - 2.3)
                    .max(depth - 9.0)
                    .max(-depth - 0.5);
                density = density.min(shaft);
                let sign = if (id >> 16) & 1 == 0 { 1.0 } else { -1.0 };
                for (vx, vz, drop, radius) in [
                    (18.0 * sign, 7.0, 8.0, 2.6),
                    (-11.0 * sign, 15.0, 10.0, 2.2),
                ] {
                    let projection = ((dx * vx + dz * vz) / (vx * vx + vz * vz)).clamp(0.0, 1.0);
                    let rx = dx - vx * projection;
                    let rz = dz - vz * projection;
                    let squared = rx * rx + rz * rz;
                    let half = (radius * radius - squared).max(0.0).sqrt();
                    let tunnel = (squared.sqrt() - radius)
                        .max((depth - 6.0 - projection * drop).abs() - half);
                    density = density.min(tunnel);
                }
            }
        }
        density
    }
}
