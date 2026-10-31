//! CPU mesh containers for hosts without Horizon's native GPU dependencies.
use std::hash::{Hash, Hasher};

pub const MIN_Y: i32 = -64;
pub type SectionKey = (i32, i32, i32);
#[derive(Clone, Copy, Debug)]
pub struct TerrainVertex {
    pub pos: [f32; 3],
    uv: [u16; 2],
    pub layer: u16,
    pub light: u16,
    pub color: u32,
}
impl TerrainVertex {
    pub fn new(pos: [f32; 3], uv: [f32; 2], layer: u16, color: u32, light: u16) -> Self {
        Self {
            pos,
            uv: uv.map(|v| (v * 32768.0).round().clamp(0.0, 65535.0) as u16),
            layer,
            color,
            light,
        }
    }
    pub fn uv(&self) -> [f32; 2] {
        self.uv.map(|v| f32::from(v) / 32768.0)
    }
}
#[derive(Default)]
pub struct SectionMesh {
    pub solid: Vec<TerrainVertex>,
    pub translucent: Vec<TerrainVertex>,
}
impl SectionMesh {
    // WebGL culls triangles directly; the native side buckets aren't needed.
    pub fn sort_solid(&mut self) {}
    pub fn fingerprint(&self) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for vertices in [&self.solid, &self.translucent] {
            vertices.len().hash(&mut hash);
            for v in vertices {
                v.pos.map(f32::to_bits).hash(&mut hash);
                v.uv.hash(&mut hash);
                (v.layer, v.light, v.color).hash(&mut hash);
            }
        }
        hash.finish()
    }
}
