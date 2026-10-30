//! Original, bounded abandoned-mine preview layouts. No vanilla templates or entities.
use crate::materials::Material;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MineMode {
    Off,
    #[default]
    Auto,
    Showcase,
}
impl MineMode {
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UndergroundView {
    #[default]
    Surface,
    Cutaway,
    Underground,
}
impl UndergroundView {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Surface => "SURFACE",
            Self::Cutaway => "CUTAWAY",
            Self::Underground => "UNDERGROUND",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Surface => Self::Cutaway,
            Self::Cutaway => Self::Underground,
            Self::Underground => Self::Surface,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "surface" => Some(Self::Surface),
            "cutaway" => Some(Self::Cutaway),
            "underground" => Some(Self::Underground),
            _ => None,
        }
    }
    /// Applied only after generation; it cannot change the underlying cave/mine plan.
    pub fn visible(self, p: [i32; 3]) -> bool {
        match self {
            Self::Surface => true,
            Self::Cutaway => !(p[0] >= 0 && p[2] >= 0),
            Self::Underground => p[1] <= 10,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct MineScene {
    pub air: BTreeSet<[i32; 3]>,
    pub blocks: BTreeMap<[i32; 3], Material>,
    pub rails: Vec<([i32; 3], u8)>,
    pub webs: Vec<[i32; 3]>,
    pub lamps: Vec<[i32; 3]>,
    pub mines: u32,
    pub supports: u32,
    frames: Vec<[i32; 3]>,
}
impl MineScene {
    pub fn generate(
        seed: i64,
        origin: [i32; 2],
        size: u32,
        mode: MineMode,
        surface: impl Fn(i32, i32) -> f32,
        cancelled: impl Fn() -> bool,
    ) -> Option<Self> {
        if size == 0 || size > 256 || origin.iter().any(|v| v.unsigned_abs() > 29_999_000) {
            return None;
        }
        let mut out = Self::default();
        if mode == MineMode::Off {
            return Some(out);
        }
        let end = [origin[0] + size as i32, origin[1] + size as i32];
        let mut centres = Vec::new();
        if mode == MineMode::Showcase {
            centres.push((0, 0, mix(seed, 0, 0)));
        } else {
            for cz in (origin[1] - 56).div_euclid(128)..=(end[1] + 56).div_euclid(128) {
                for cx in (origin[0] - 56).div_euclid(128)..=(end[0] + 56).div_euclid(128) {
                    let id = mix(seed, cx, cz);
                    if (id >> 16) % 100 < 60 {
                        centres.push((
                            cx * 128 + (id % 25) as i32 - 12,
                            cz * 128 + ((id >> 8) % 25) as i32 - 12,
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
            if x + 42 < origin[0] || z + 42 < origin[1] || x - 42 >= end[0] || z - 42 >= end[1] {
                continue;
            }
            let floor = 6 + ((id >> 24) % 3) as i32;
            let mut mine = Self::default();
            for (axis, ox, oz, length) in [
                (0, 0, 0, 40),
                (1, 0, 0, 30),
                (1, -18, 0, 18),
                (1, 18, 0, 18),
            ] {
                for t in -length..=length {
                    if cancelled() {
                        return None;
                    }
                    let centre = [
                        x + ox + if axis == 0 { t } else { 0 },
                        z + oz + if axis == 1 { t } else { 0 },
                    ];
                    let mut dry = true;
                    for across in -2..=2 {
                        let wx = centre[0] + if axis == 1 { across } else { 0 };
                        let wz = centre[1] + if axis == 0 { across } else { 0 };
                        let h = surface(wx, wz);
                        if !h.is_finite() || h < (floor + 8) as f32 {
                            dry = false;
                            break;
                        }
                    }
                    if !dry {
                        continue;
                    }
                    for across in -2..=2 {
                        let wx = centre[0] + if axis == 1 { across } else { 0 };
                        let wz = centre[1] + if axis == 0 { across } else { 0 };
                        for dy in 1..=4 {
                            mine.air.insert([wx, floor + dy, wz]);
                        }
                        mine.blocks.insert([wx, floor, wz], Material::OakPlanks);
                    }
                    mine.rails.push(([centre[0], floor + 1, centre[1]], axis));
                    let junction = if axis == 0 {
                        [0_i32, -18, 18].iter().any(|v| (t - *v).abs() <= 4)
                    } else {
                        t.abs() <= 4
                    };
                    if t.rem_euclid(7) == 0 && !junction {
                        for across in -2..=2 {
                            let wx = centre[0] + if axis == 1 { across } else { 0 };
                            let wz = centre[1] + if axis == 0 { across } else { 0 };
                            mine.blocks.insert([wx, floor + 4, wz], Material::OakPlanks);
                            if across.abs() == 2 {
                                for dy in 1..=3 {
                                    mine.blocks
                                        .insert([wx, floor + dy, wz], Material::OakLogSide);
                                }
                            }
                        }
                        mine.frames.push([centre[0], floor + 2, centre[1]]);
                        if t.rem_euclid(14) == 0 {
                            mine.lamps.push([
                                centre[0] + if axis == 1 { 1 } else { 0 },
                                floor + 3,
                                centre[1] + if axis == 0 { 1 } else { 0 },
                            ]);
                        }
                    } else if (t + ((id >> 32) % 13) as i32).rem_euclid(13) == 0 {
                        mine.webs.push([
                            centre[0] + if axis == 1 { 1 } else { 0 },
                            floor + 3,
                            centre[1] + if axis == 0 { 1 } else { 0 },
                        ]);
                    }
                }
            }
            if mine.air.is_empty() {
                continue;
            }
            out.mines += 1;
            out.frames.extend(mine.frames);
            out.air.extend(mine.air);
            out.blocks.extend(mine.blocks);
            out.rails.extend(mine.rails);
            out.webs.extend(mine.webs);
            out.lamps.extend(mine.lamps);
            if out.air.len() > 100_000 || out.blocks.len() > 60_000 {
                return None;
            }
        }
        let inside =
            |p: &[i32; 3]| p[0] >= origin[0] && p[2] >= origin[1] && p[0] < end[0] && p[2] < end[1];
        out.air.retain(inside);
        out.blocks.retain(|p, _| inside(p));
        out.rails
            .retain(|(p, _)| inside(p) && !out.blocks.contains_key(p));
        out.webs.retain(inside);
        out.lamps.retain(inside);
        out.rails.sort();
        out.rails.dedup_by_key(|r| r.0);
        out.frames.retain(inside);
        out.supports = out.frames.len() as u32;
        if out.air.is_empty() {
            out.mines = 0;
        }
        out.webs.sort();
        out.webs.dedup();
        out.lamps.sort();
        out.lamps.dedup();
        Some(out)
    }
}
fn mix(seed: i64, x: i32, z: i32) -> u64 {
    let mut v = seed as u64
        ^ (x as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as u32 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb)
}
