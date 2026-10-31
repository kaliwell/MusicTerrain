//! Browser-independent voxel simulation and greedy meshing. JavaScript only hosts
//! the audio bridge, worker and WebGL presentation. No OS or browser imports.
use musicterrain::{
    caves::cave_slice,
    dsp::AudioFeatures,
    ecology::{DecorationKind, Ecology},
    mapping::{AudioSource, MappingGraph, MappingRule, MappingState},
    shape::TerrainShape,
    terrain::{NoiseType, TargetParameter, TerrainField, TerrainParameters},
    underground::UndergroundField,
};
use std::cell::RefCell;

const HEIGHT: usize = 96;
const WATER: u8 = 5;
const STRIDE: usize = 7; // position xyz, tiled uv, texture-array layer, shade
const MAX_FLOATS: usize = 24_000_000;

#[derive(Clone, Copy)]
struct Input {
    size: u32,
    shape: u32,
    noise: u32,
    seed: u32,
    phase: f32,
    dt: f32,
    rms: f32,
    bass: f32,
    mid: f32,
    treble: f32,
    intensity: f32,
    flags: u32,
}
struct World {
    n: usize,
    cells: Vec<u8>,
    heights: Vec<i32>,
}
impl World {
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.n + x) * HEIGHT + y
    }
    fn get(&self, p: [i32; 3]) -> u8 {
        let [x, y, z] = p;
        if x < 0 || z < 0 || y < 0 || x >= self.n as i32 || z >= self.n as i32 || y >= HEIGHT as i32
        {
            0
        } else {
            self.cells[self.index(x as usize, y as usize, z as usize)]
        }
    }
    fn set_empty(&mut self, p: [i32; 3], material: u8, shape: TerrainShape) {
        if p[0] < 0
            || p[2] < 0
            || p[1] < 0
            || p[0] >= self.n as i32
            || p[2] >= self.n as i32
            || p[1] >= HEIGHT as i32
        {
            return;
        }
        let origin = self.n as f32 * 0.5;
        if !shape.contains(
            p[0] as f32 + 0.5 - origin,
            p[2] as f32 + 0.5 - origin,
            self.n as u32,
        ) {
            return;
        }
        let i = self.index(p[0] as usize, p[1] as usize, p[2] as usize);
        if self.cells[i] == 0 {
            self.cells[i] = material;
        }
    }
}
struct Kernel {
    seed: u32,
    field: TerrainField,
    ecology: Ecology,
    underground: UndergroundField,
    mappings: MappingGraph,
    mapping: MappingState,
    solid: Vec<f32>,
    water: Vec<f32>,
    world: World,
    carved: u32,
}
impl Default for Kernel {
    fn default() -> Self {
        let mut w = MappingRule::band(0, TargetParameter::CaveW, [0.0, 2.0]);
        w.source = AudioSource::Rms;
        w.attack = 0.2;
        w.release = 0.45;
        let mut amplitude = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 20.0]);
        amplitude.attack = 0.12;
        amplitude.release = 0.4;
        let mut warp = MappingRule::band(3, TargetParameter::DomainWarp, [0.0, 6.0]);
        warp.attack = 0.18;
        warp.release = 0.5;
        let mut overall = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 16.0]);
        overall.source = AudioSource::Rms;
        overall.attack = 0.12;
        overall.release = 0.4;
        Self {
            seed: 42,
            field: TerrainField::new(42),
            ecology: Ecology::new(42),
            underground: UndergroundField::new(42),
            mappings: MappingGraph {
                rules: vec![amplitude, overall, warp, w],
            },
            mapping: MappingState::default(),
            solid: Vec::new(),
            water: Vec::new(),
            world: World {
                n: 0,
                cells: Vec::new(),
                heights: Vec::new(),
            },
            carved: 0,
        }
    }
}
impl Kernel {
    fn generate(&mut self, input: Input) -> Result<(), ()> {
        if [
            input.phase,
            input.dt,
            input.rms,
            input.bass,
            input.mid,
            input.treble,
            input.intensity,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(());
        }
        let size = input.size.clamp(32, 128) / 16 * 16;
        let n = size as usize;
        let shape = TerrainShape::ALL
            .get(input.shape as usize)
            .copied()
            .unwrap_or_default();
        if self.seed != input.seed {
            self.seed = input.seed;
            self.field = TerrainField::new(i64::from(input.seed));
            self.ecology = Ecology::new(i64::from(input.seed));
            self.underground = UndergroundField::new(i64::from(input.seed));
            self.mapping = MappingState::default();
        }
        let gain = input.intensity.clamp(0.0, 2.0);
        let features = AudioFeatures {
            rms: input.rms.clamp(0.0, 1.0) * gain,
            bands: [
                input.bass,
                input.bass * 0.8,
                input.mid,
                input.mid,
                input.treble,
                input.treble * 0.8,
                input.treble * 0.5,
            ]
            .map(|v| v.clamp(0.0, 1.0) * gain),
            ..Default::default()
        };
        let base = TerrainParameters {
            noise_type: NoiseType::ALL
                .get(input.noise as usize)
                .copied()
                .unwrap_or_default(),
            base_height: 20.0,
            amplitude: 25.0,
            frequency: 0.035,
            detail: 0.8,
            domain_warp: 2.0,
            ..Default::default()
        };
        let p = self
            .mappings
            .evaluate(
                &features,
                &base,
                input.dt.clamp(0.0, 0.25),
                &mut self.mapping,
            )
            .map_err(|_| ())?;
        let phase = (input.phase + p.cave_w + 128.0).rem_euclid(256.0) - 128.0;
        self.world.n = n;
        self.world.cells.clear();
        self.world.cells.resize(n * n * HEIGHT, 0);
        self.world.heights.clear();
        self.world.heights.resize(n * n, 0);
        self.carved = 0;
        let origin = -(size as i32) / 2;
        let water = input.flags & 4 != 0;
        for z in 0..n {
            for x in 0..n {
                let wx = origin + x as i32;
                let wz = origin + z as i32;
                if !shape.contains(wx as f32 + 0.5, wz as f32 + 0.5, size) {
                    continue;
                }
                let h = self
                    .field
                    .height(f64::from(wx), f64::from(wz), &p)
                    .round()
                    .clamp(5.0, 80.0) as i32;
                self.world.heights[z * n + x] = h;
                let column = self.ecology.cave_column(wx, wz, h as f32);
                let underground = self.underground.column(wx, wz, h as f32);
                let mut slices = [[0.0; 2]; 64];
                let mut count = 0;
                if input.flags & 1 != 0 {
                    for range in column.intervals().iter().chain(underground.intervals()) {
                        slices[count] = cave_slice(
                            *range,
                            wx,
                            wz,
                            h,
                            (0.78 - p.cave_threshold) * 4.0,
                            i64::from(input.seed),
                            phase,
                        );
                        count += 1;
                    }
                }
                for y in 0..h.max(if water { 10 } else { 0 }) as usize {
                    let i = self.world.index(x, y, z);
                    if y as i32 >= h {
                        self.world.cells[i] = WATER;
                        continue;
                    }
                    if y >= 3
                        && slices[..count]
                            .iter()
                            .any(|r| y as f32 >= r[0] && y as f32 <= r[1])
                    {
                        self.carved += 1;
                        continue;
                    }
                    self.world.cells[i] = if y as i32 == h - 1 {
                        if water && h <= 11 {
                            4
                        } else if h > 43 {
                            8
                        } else {
                            1
                        }
                    } else if y as i32 >= h - 4 {
                        2
                    } else {
                        3
                    };
                }
            }
        }
        if input.flags & 2 != 0 {
            let heights = &self.world.heights;
            let layout = self.ecology.decorations([origin; 2], size, |x, z| {
                let lx = x - origin;
                let lz = z - origin;
                if lx >= 0 && lz >= 0 && lx < n as i32 && lz < n as i32 {
                    heights[lz as usize * n + lx as usize] as f32
                } else {
                    self.field
                        .height(f64::from(x), f64::from(z), &p)
                        .round()
                        .clamp(5.0, 80.0)
                }
            });
            for tree in layout {
                if !matches!(
                    tree.kind,
                    DecorationKind::OakTree
                        | DecorationKind::BirchTree
                        | DecorationKind::SpruceTree
                        | DecorationKind::AcaciaTree
                        | DecorationKind::JungleTree
                ) {
                    continue;
                }
                let [x, y, z] = tree.anchor;
                let at = [x - origin, y, z - origin];
                if !matches!(self.world.get([at[0], at[1] - 1, at[2]]), 1 | 8)
                    || self.world.get(at) != 0
                {
                    continue;
                }
                let trunk = i32::from(tree.scale).clamp(3, 6);
                for dy in 0..trunk {
                    self.world.set_empty([at[0], y + dy, at[2]], 6, shape);
                }
                for dz in -2_i32..=2 {
                    for dx in -2_i32..=2 {
                        for dy in -1_i32..=1 {
                            if dx.abs() + dz.abs() + dy.abs() > 4 {
                                continue;
                            }
                            self.world.set_empty(
                                [at[0] + dx, y + trunk + dy, at[2] + dz],
                                7,
                                shape,
                            );
                        }
                    }
                }
            }
        }
        self.solid.clear();
        self.water.clear();
        mesh(&self.world, &mut self.solid, &mut self.water)?;
        Ok(())
    }
}

/// Rectangle merging retains every voxel boundary and repeats original texels.
/// A face key includes layer/shade, so roofs, caves and water cannot merge wrongly.
fn mesh(world: &World, solid: &mut Vec<f32>, water: &mut Vec<f32>) -> Result<(), ()> {
    let dims = [world.n as i32, HEIGHT as i32, world.n as i32];
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let width = dims[u] as usize;
        let height = dims[v] as usize;
        let mut mask = vec![0_u16; width * height];
        for sign in [-1_i32, 1] {
            for plane in 0..dims[axis] {
                for j in 0..height {
                    for i in 0..width {
                        let mut pos = [0; 3];
                        pos[axis] = plane;
                        pos[u] = i as i32;
                        pos[v] = j as i32;
                        let cell = world.get(pos);
                        pos[axis] += sign;
                        let other = world.get(pos);
                        let internal = pos[0] >= 0
                            && pos[2] >= 0
                            && pos[1] >= 3
                            && pos[0] < world.n as i32
                            && pos[2] < world.n as i32
                            && pos[1] < HEIGHT as i32
                            && pos[1]
                                < world.heights[pos[2] as usize * world.n + pos[0] as usize] - 3;
                        let visible = cell != 0
                            && if cell == WATER {
                                other == 0
                            } else {
                                other == 0 || other == WATER
                            };
                        let key = if visible {
                            let layer = match cell {
                                1 => {
                                    if axis == 1 {
                                        if sign > 0 { 0 } else { 2 }
                                    } else {
                                        1
                                    }
                                }
                                2 => 2,
                                3 => 3,
                                4 => 4,
                                5 => 5,
                                6 => {
                                    if axis == 1 {
                                        7
                                    } else {
                                        6
                                    }
                                }
                                7 => 8,
                                8 if axis == 1 && sign > 0 => 9,
                                8 => 3,
                                _ => 3,
                            };
                            pos[axis] -= sign;
                            let depth = world.heights[pos[2] as usize * world.n + pos[0] as usize]
                                - pos[1]
                                - 1;
                            let cave = internal && other == 0 && depth > 4 && pos[1] > 2;
                            let shade = if cave {
                                if depth > 16 { 35 } else { 55 }
                            } else if axis == 1 {
                                if sign > 0 { 100 } else { 50 }
                            } else if axis == 0 {
                                80
                            } else {
                                68
                            };
                            1 + layer + (shade << 4)
                        } else {
                            0
                        };
                        mask[j * width + i] = key;
                    }
                }
                let mut j = 0;
                while j < height {
                    let mut i = 0;
                    while i < width {
                        let key = mask[j * width + i];
                        if key == 0 {
                            i += 1;
                            continue;
                        }
                        let mut w = 1;
                        while i + w < width && mask[j * width + i + w] == key {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: while j + h < height {
                            for k in 0..w {
                                if mask[(j + h) * width + i + k] != key {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        let packed = key - 1;
                        let layer = (packed & 15) as f32;
                        let shade = f32::from(packed >> 4) * 0.01;
                        let target = if layer == 5.0 {
                            &mut *water
                        } else {
                            &mut *solid
                        };
                        let mut p = [0.0; 3];
                        p[axis] = (plane + if sign > 0 { 1 } else { 0 }) as f32;
                        p[u] = i as f32;
                        p[v] = j as f32;
                        let mut a = p;
                        a[u] += w as f32;
                        let mut b = a;
                        b[v] += h as f32;
                        let mut c = p;
                        c[v] += h as f32;
                        let points = if sign > 0 {
                            [p, a, b, p, b, c]
                        } else {
                            [p, c, b, p, b, a]
                        };
                        let uv = if sign > 0 {
                            [
                                [0.0, 0.0],
                                [w as f32, 0.0],
                                [w as f32, h as f32],
                                [0.0, 0.0],
                                [w as f32, h as f32],
                                [0.0, h as f32],
                            ]
                        } else {
                            [
                                [0.0, 0.0],
                                [0.0, h as f32],
                                [w as f32, h as f32],
                                [0.0, 0.0],
                                [w as f32, h as f32],
                                [w as f32, 0.0],
                            ]
                        };
                        if target.len() + 6 * STRIDE > MAX_FLOATS {
                            return Err(());
                        }
                        for (pos, uv) in points.into_iter().zip(uv) {
                            let uv = if axis == 0 { [uv[1], uv[0]] } else { uv };
                            target.extend_from_slice(&[
                                pos[0] - world.n as f32 * 0.5,
                                pos[1],
                                pos[2] - world.n as f32 * 0.5,
                                uv[0],
                                uv[1],
                                layer,
                                shade,
                            ]);
                        }
                        for y in 0..h {
                            mask[(j + y) * width + i..(j + y) * width + i + w].fill(0);
                        }
                        i += w;
                    }
                    j += 1;
                }
            }
        }
    }
    Ok(())
}

thread_local! {static KERNEL:RefCell<Kernel>=RefCell::new(Kernel::default());}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_generate(
    size: u32,
    shape: u32,
    noise: u32,
    seed: u32,
    phase: f32,
    dt: f32,
    rms: f32,
    bass: f32,
    mid: f32,
    treble: f32,
    intensity: f32,
    flags: u32,
) -> u32 {
    KERNEL.with_borrow_mut(|kernel| {
        u32::from(
            kernel
                .generate(Input {
                    size,
                    shape,
                    noise,
                    seed,
                    phase,
                    dt,
                    rms,
                    bass,
                    mid,
                    treble,
                    intensity,
                    flags,
                })
                .is_ok(),
        )
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_solid_ptr() -> *const f32 {
    KERNEL.with_borrow(|k| k.solid.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_solid_len() -> usize {
    KERNEL.with_borrow(|k| k.solid.len())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_water_ptr() -> *const f32 {
    KERNEL.with_borrow(|k| k.water.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_water_len() -> usize {
    KERNEL.with_borrow(|k| k.water.len())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_carved() -> u32 {
    KERNEL.with_borrow(|k| k.carved)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            size: 32,
            shape: 0,
            noise: 2,
            seed: 42,
            phase: 0.0,
            dt: 0.08,
            rms: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            intensity: 1.0,
            flags: 7,
        }
    }
    #[test]
    fn greedy_mesh_covers_cube_with_outward_normals_and_tiled_uvs() {
        let mut world = World {
            n: 32,
            cells: vec![0; 32 * 32 * HEIGHT],
            heights: vec![4; 32 * 32],
        };
        for z in 0..32 {
            for x in 0..32 {
                for y in 0..4 {
                    let i = world.index(x, y, z);
                    world.cells[i] = 3;
                }
            }
        }
        let mut solid = Vec::new();
        let mut water = Vec::new();
        mesh(&world, &mut solid, &mut water).unwrap();
        assert_eq!(solid.len(), 6 * 6 * STRIDE);
        assert!(water.is_empty());
        for q in solid.as_chunks::<{ 6 * STRIDE }>().0 {
            let a = &q[0..3];
            let b = &q[7..10];
            let c = &q[14..17];
            let ab = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
            let ac = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
            let normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            assert!(
                (0..3)
                    .map(|i| normal[i] * (a[i] - [0.0, 2.0, 0.0][i]))
                    .sum::<f32>()
                    > 0.0
            );
        }
    }
    #[test]
    fn clipped_outer_walls_and_terraces_are_not_shaded_as_caves() {
        for shape in 0..4 {
            let mut kernel = Kernel::default();
            kernel
                .generate(Input {
                    shape,
                    flags: 0,
                    ..input()
                })
                .unwrap();
            for vertex in kernel.solid.as_chunks::<STRIDE>().0 {
                let shade = vertex[6];
                assert!(
                    shade >= 0.67 || (shade - 0.5).abs() < 1e-6,
                    "false cave shade {shade} in shape {shape}"
                );
            }
        }
    }
    #[test]
    fn shared_fields_are_deterministic_audio_reactive_and_w_changes_only_caves() {
        let mut a = Kernel::default();
        let mut b = Kernel::default();
        let i = input();
        a.generate(i).unwrap();
        b.generate(i).unwrap();
        assert_eq!(a.solid, b.solid);
        let heights = a.world.heights.clone();
        let original = a.world.cells.clone();
        a.generate(Input { phase: 2.1, ..i }).unwrap();
        assert_eq!(a.world.heights, heights);
        assert_ne!(a.world.cells, original);
        b.generate(Input {
            bass: 0.0,
            rms: 0.8,
            dt: 0.25,
            ..i
        })
        .unwrap();
        assert_ne!(b.world.heights, heights);
        assert!(a.solid.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn shapes_clip_floor_roof_and_invalid_inputs_are_bounded() {
        for shape in 0..4 {
            let mut k = Kernel::default();
            k.generate(Input {
                shape,
                flags: 1,
                phase: 3.0,
                ..input()
            })
            .unwrap();
            for z in 0..32 {
                for x in 0..32 {
                    let inside = TerrainShape::ALL[shape as usize].contains(
                        x as f32 - 15.5,
                        z as f32 - 15.5,
                        32,
                    );
                    for y in 0..3 {
                        assert_eq!(k.world.get([x, y, z]) != 0, inside);
                    }
                    if inside {
                        let h = k.world.heights[z as usize * 32 + x as usize];
                        assert_ne!(k.world.get([x, h - 1, z]), 0);
                    }
                }
            }
            assert!(
                k.generate(Input {
                    phase: f32::NAN,
                    ..input()
                })
                .is_err()
            );
        }
    }
}
