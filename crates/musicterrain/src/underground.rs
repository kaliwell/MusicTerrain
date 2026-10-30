//! Original modern-style preview caves: cached analytic column intervals.
//!
//! This is not vanilla noise-router parity. All queries use seeded world coordinates;
//! their result is independent of region boundaries, request order and playback clocks.
use horizon_core::{noise::SimplexNoise, random::LegacyRandom};
use serde::{Deserialize, Serialize};

const CELL: i32 = 64;
const CAPACITY: usize = 32;
const LIMIT: i32 = 30_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndergroundBiome {
    #[default]
    Stone,
    Lush,
    Dripstone,
    Deep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndergroundColumn {
    ranges: [[f32; 2]; CAPACITY],
    count: usize,
    biome: UndergroundBiome,
    water: Option<i32>,
}
impl Default for UndergroundColumn {
    fn default() -> Self {
        Self {
            ranges: [[0.; 2]; CAPACITY],
            count: 0,
            biome: UndergroundBiome::Stone,
            water: None,
        }
    }
}
impl UndergroundColumn {
    pub fn contains(&self, y: i32) -> bool {
        self.intervals()
            .iter()
            .any(|r| y as f32 >= r[0] && y as f32 <= r[1])
    }
    pub fn biome(&self) -> UndergroundBiome {
        self.biome
    }
    /// Inclusive maximum water-cell Y. Callers fill only carved cells, not solid rock.
    pub fn water_level(&self) -> Option<i32> {
        self.water
    }
    pub fn intervals(&self) -> &[[f32; 2]] {
        &self.ranges[..self.count]
    }
    fn add(&mut self, bottom: f32, top: f32, roof: f32) {
        let bottom = bottom.max(3.);
        let top = top.min(roof);
        if bottom <= top && self.count < CAPACITY {
            self.ranges[self.count] = [bottom, top];
            self.count += 1;
        }
    }
    fn merge(&mut self) {
        self.ranges[..self.count].sort_unstable_by(|a, b| a[0].total_cmp(&b[0]));
        let mut merged = 0;
        for index in 0..self.count {
            let range = self.ranges[index];
            if merged > 0 && range[0] <= self.ranges[merged - 1][1] {
                self.ranges[merged - 1][1] = self.ranges[merged - 1][1].max(range[1]);
            } else {
                self.ranges[merged] = range;
                merged += 1;
            }
        }
        self.count = merged;
    }
}

#[derive(Clone, Debug)]
pub struct UndergroundField {
    seed: i64,
    geology: SimplexNoise,
}
impl UndergroundField {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            geology: SimplexNoise::new(&mut LegacyRandom::new(seed ^ 0x554e_4445_5247_5244)),
        }
    }
    pub fn column(&self, x: i32, z: i32, surface: f32) -> UndergroundColumn {
        let mut out = UndergroundColumn::default();
        if x.unsigned_abs() > LIMIT as u32
            || z.unsigned_abs() > LIMIT as u32
            || !surface.is_finite()
            || surface < 11.
        {
            return out;
        }
        let roof = (surface - 4.).min(124.);
        let xf = f64::from(x);
        let zf = f64::from(z);
        let climate = self.geology.get2(xf * 0.012 + 311., zf * 0.012 - 173.);
        out.biome = if climate > 0.38 {
            UndergroundBiome::Lush
        } else if climate < -0.4 {
            UndergroundBiome::Deep
        } else if self.geology.get2(xf * 0.016 - 73., zf * 0.016 + 127.) > 0.12 {
            UndergroundBiome::Dripstone
        } else {
            UndergroundBiome::Stone
        };
        // Closed-form ellipsoid intersections create broad chambers. Two overlapping
        // layers per cell make shelves, pillars and connecting openings instead of a
        // single repeated spherical room. The largest horizontal radius is < CELL/2.
        for cz in z.div_euclid(CELL) - 1..=z.div_euclid(CELL) + 1 {
            for cx in x.div_euclid(CELL) - 1..=x.div_euclid(CELL) + 1 {
                let id = hash(self.seed, cx, cz);
                let px = cx * CELL + 16 + (id % 33) as i32;
                let pz = cz * CELL + 16 + ((id >> 8) % 33) as i32;
                let rx = 15. + ((id >> 16) % 9) as f32;
                let rz = 11. + ((id >> 24) % 9) as f32;
                let cy = 12. + ((id >> 32) % 9) as f32;
                let ry = 5. + ((id >> 40) % 5) as f32;
                ellipsoid(
                    &mut out,
                    (x - px) as f32,
                    (z - pz) as f32,
                    rx,
                    rz,
                    cy,
                    ry,
                    roof,
                );
                ellipsoid(
                    &mut out,
                    (x - px - 7) as f32,
                    (z - pz + 4) as f32,
                    rx * 0.65,
                    rz * 0.7,
                    7. + ((id >> 48) % 5) as f32,
                    4.,
                    roof,
                );
            }
        }
        // Winding X/Z connector families are evaluated per column, never per voxel.
        // Neighbouring regions share the same sinusoid phase and global Y centre.
        let y = 11. + self.geology.get2(xf * 0.02, zf * 0.02) * 4.;
        for band in z.div_euclid(32) - 1..=z.div_euclid(32) + 1 {
            let id = hash(self.seed ^ 0x584e_4f4f, band, 0);
            let phase = (id % 6283) as f64 * 0.001;
            let centre = f64::from(band * 32 + 16) + (xf * 0.055 + phase).sin() * 7.;
            tube(
                &mut out,
                (zf - centre) as f32,
                y,
                1.7 + ((id >> 16) % 3) as f32 * 0.3,
                roof,
            );
        }
        for band in x.div_euclid(32) - 1..=x.div_euclid(32) + 1 {
            let id = hash(self.seed ^ 0x5a4e_4f4f, band, 0);
            let phase = (id % 6283) as f64 * 0.001;
            let centre = f64::from(band * 32 + 16) + (zf * 0.061 + phase).sin() * 6.;
            tube(&mut out, (xf - centre) as f32, y + 3., 1.8, roof);
        }
        out.merge();
        // A bounded low aquifer. Its mask is world-space continuous noise, not a
        // per-query random draw; filling remains confined to the carved intervals.
        if roof >= 6.
            && self.geology.get2(xf * 0.018 + 991., zf * 0.018 + 421.) > 0.28
            && out.intervals().iter().any(|r| r[0] <= 6.)
        {
            out.water = Some(6);
        }
        out
    }
}
fn ellipsoid(
    out: &mut UndergroundColumn,
    dx: f32,
    dz: f32,
    rx: f32,
    rz: f32,
    cy: f32,
    ry: f32,
    roof: f32,
) {
    let distance = (dx / rx).powi(2) + (dz / rz).powi(2);
    if distance < 1. {
        let half = ry * (1. - distance).sqrt();
        out.add(cy - half, cy + half, roof);
    }
}
fn tube(out: &mut UndergroundColumn, distance: f32, y: f32, radius: f32, roof: f32) {
    if distance.abs() < radius {
        let half = (radius * radius - distance * distance).sqrt();
        out.add(y - half, y + half, roof);
    }
}
fn hash(seed: i64, x: i32, z: i32) -> u64 {
    let packed = ((x as u32 as u64) << 32) | z as u32 as u64;
    let mut v = packed ^ (seed as u64).rotate_left(17);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    v ^ (v >> 31)
}
