//! Complete native CPU scene hosted by a Web Worker. No OS or browser imports.
use musicterrain::{
    dsp::{AnalysisConfig, Analyzer, AudioFeatures},
    ecology::SceneBiome,
    mapping::{AudioSource, MappingGraph, MappingRule, MappingState},
    materials::{Material, TextureAtlas},
    mineshaft::{MineMode, UndergroundView},
    preset::{VisualizationPreset, builtins},
    shape::TerrainShape,
    terrain::{
        GenerationRevision, NoiseType, TargetParameter, TerrainField, TerrainParameters,
        TerrainSnapshot,
    },
    village::VillageMode,
    visual::{SCENE_MIN_Y, SceneOptions, mesh_snapshot_with_options},
};
use serde::Deserialize;
use serde_json::json;
use std::cell::RefCell;

const STRIDE: usize = 24; // packed position, fixed-point UV, layer, light and RGBA
const MAX_INPUT: usize = 1_048_576;
#[derive(Deserialize)]
#[serde(default)]
struct Config {
    size: u32,
    shape: usize,
    noise: usize,
    seed: i64,
    caves: bool,
    trees: bool,
    water: bool,
    full_detail: bool,
    biome: String,
    villages: String,
    mines: String,
    view: String,
    parameters: TerrainParameters,
    mappings: MappingGraph,
    intensity: f32,
    phase: f32,
    dt: f32,
    clock: f64,
    features: AudioFeatures,
    pcm: Vec<f32>,
    sample_rate: u32,
    lab: bool,
    slice: f32,
    overlay: bool,
}
impl Default for Config {
    fn default() -> Self {
        let preset = default_preset();
        Self {
            size: 64,
            shape: 1,
            noise: 3,
            seed: 42,
            caves: true,
            trees: true,
            water: true,
            full_detail: true,
            biome: "auto".into(),
            villages: "auto".into(),
            mines: "auto".into(),
            view: "surface".into(),
            parameters: preset.parameters,
            mappings: preset.mappings,
            intensity: 1.0,
            phase: 0.0,
            dt: 0.03,
            clock: 0.0,
            features: AudioFeatures::default(),
            pcm: Vec::new(),
            sample_rate: 48000,
            lab: false,
            slice: 8.0,
            overlay: true,
        }
    }
}
fn default_preset() -> VisualizationPreset {
    let mut preset = builtins().remove(0);
    // Keep quiet vocal/piano recordings reactive even without strong low bass.
    let mut overall = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 16.0]);
    overall.source = AudioSource::Rms;
    overall.attack = 0.12;
    overall.release = 0.4;
    preset.mappings.rules.push(overall);
    preset
}
struct Kernel {
    input: Vec<u8>,
    json: Vec<u8>,
    atlas: TextureAtlas,
    solid: Vec<u8>,
    water: Vec<u8>,
    lab: Vec<u8>,
    carved: u32,
    mapping: MappingState,
    analyzer: Option<(u32, Analyzer)>,
    last_time: f64,
}
impl Default for Kernel {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            json: Vec::new(),
            atlas: TextureAtlas::original(),
            solid: Vec::new(),
            water: Vec::new(),
            lab: Vec::new(),
            carved: 0,
            mapping: MappingState::default(),
            analyzer: None,
            last_time: -1.0,
        }
    }
}
impl Kernel {
    fn generate(&mut self, c: Config) -> Result<(), String> {
        if !(32..=256).contains(&c.size)
            || !c.size.is_multiple_of(16)
            || c.shape >= 4
            || c.noise >= 8
            || !c.clock.is_finite()
            || !c.phase.is_finite()
            || !c.dt.is_finite()
            || !(0.0..=10.0).contains(&c.dt)
            || !c.intensity.is_finite()
            || !(0.0..=2.0).contains(&c.intensity)
            || !c.slice.is_finite()
        {
            return Err("Invalid scene input".into());
        }
        c.parameters.validate().map_err(|e| e.to_string())?;
        c.mappings.validate().map_err(|e| e.to_string())?;
        let biome = if c.biome == "auto" {
            None
        } else {
            Some(SceneBiome::parse(&c.biome).ok_or("Invalid biome")?)
        };
        let options = SceneOptions {
            shape: TerrainShape::ALL[c.shape],
            full_detail: c.full_detail,
            vegetation: c.trees,
            water: c.water,
            caves: c.caves,
            biome,
            villages: VillageMode::parse(&c.villages).ok_or("Invalid village mode")?,
            mines: MineMode::parse(&c.mines).ok_or("Invalid mine mode")?,
            underground_view: UndergroundView::parse(&c.view).ok_or("Invalid view")?,
        };
        if c.clock < self.last_time {
            self.mapping.reset();
            self.analyzer = None;
        }
        self.last_time = c.clock;
        let features = if c.pcm.is_empty() {
            c.features
        } else {
            if c.pcm.len() != 4096 {
                return Err("Audio window must contain 4096 samples".into());
            }
            if self
                .analyzer
                .as_ref()
                .is_none_or(|(rate, _)| *rate != c.sample_rate)
            {
                self.analyzer = Some((
                    c.sample_rate,
                    Analyzer::new(c.sample_rate, AnalysisConfig::default())
                        .map_err(|e| e.to_string())?,
                ));
            }
            self.analyzer
                .as_mut()
                .unwrap()
                .1
                .analyze_window(&c.pcm, c.clock)
                .map_err(|e| e.to_string())?
        };
        let mut base = c.parameters;
        base.noise_type = NoiseType::ALL[c.noise];
        let mapped = c
            .mappings
            .evaluate(&features, &base, c.dt, &mut self.mapping)
            .map_err(|e| e.to_string())?;
        let mut parameters = base.clone();
        for target in TargetParameter::ALL {
            target.set(
                &mut parameters,
                target.get(&base) + (target.get(&mapped) - target.get(&base)) * c.intensity,
            );
        }
        parameters.cave_w = (parameters.cave_w + c.phase).rem_euclid(256.0);
        if parameters.cave_w > 128.0 {
            parameters.cave_w -= 256.0;
        }
        let snapshot = TerrainSnapshot {
            revision: GenerationRevision(1),
            seed: c.seed,
            parameters: parameters.clamped(),
            audio_time: c.clock,
        };
        let mesh = mesh_snapshot_with_options(&snapshot, c.size, &options)
            .ok_or("Scene generation failed")?;
        // Bound the triangle expansion before allocating the WebGL staging buffers.
        if mesh.vertices > 8_000_000 {
            return Err(
                "Scene exceeds browser geometry budget; reduce size or use adaptive detail".into(),
            );
        }
        self.solid.clear();
        self.water.clear();
        for (key, section) in &mesh.sections {
            let origin = [key.0 * 16, SCENE_MIN_Y + key.1 * 16, key.2 * 16];
            for (vertices, out) in [
                (&section.solid, &mut self.solid),
                (&section.translucent, &mut self.water),
            ] {
                for v in vertices {
                    for (position, offset) in v.pos.into_iter().zip(origin) {
                        out.extend_from_slice(&(position + offset as f32).to_le_bytes());
                    }
                    for uv in v.uv() {
                        out.extend_from_slice(&((uv * 32768.0).round() as u16).to_le_bytes());
                    }
                    out.extend_from_slice(&(v.layer & 0x7fff).to_le_bytes());
                    out.extend_from_slice(&v.light.to_le_bytes());
                    out.extend_from_slice(&v.color.to_le_bytes());
                }
            }
        }
        self.carved = mesh.scene.carved_voxels;
        let s = mesh.scene;
        self.json=serde_json::to_vec(&json!({"trees":s.trees,"plants":s.plants,"carved":s.carved_voxels,"villages":s.villages,"houses":s.houses,"farms":s.farms,"mines":s.mines,"supports":s.supports,"rails":s.rails,"materials":s.material_quads.to_vec(),"voxelStep":mesh.voxel_step,"features":features,"parameters":snapshot.parameters})).unwrap();
        self.lab.clear();
        if c.lab {
            let field = TerrainField::new(c.seed);
            let p = if c.overlay {
                &snapshot.parameters
            } else {
                &base
            };
            for mode in 0..3 {
                for y in 0..64 {
                    for x in 0..64 {
                        let wx = (x as f64 / 63.0 - 0.5) * c.size as f64;
                        let wz = (y as f64 / 63.0 - 0.5) * c.size as f64;
                        let value = match mode {
                            0 => (field.height(wx, wz, p) + 64.0) / 256.0,
                            1 => 0.5 + field.density(wx, c.slice as f64, wz, p) / 64.0,
                            _ => {
                                0.5 + field.density(wx, (y as f64 / 63.0) * 192.0 - 64.0, 0.0, p)
                                    / 64.0
                            }
                        };
                        let v = (value.clamp(0.0, 1.0) * 255.0) as u8;
                        self.lab
                            .extend_from_slice(&[v, (f32::from(v) * 0.85) as u8, 255 - v, 255]);
                    }
                }
            }
        }
        Ok(())
    }
    fn metadata(&mut self) {
        let mut presets = builtins();
        presets[0] = default_preset();
        self.json=serde_json::to_vec(&json!({"presets":presets,"targets":TargetParameter::ALL.map(|t|json!({"name":t,"bounds":t.bounds()})),"biomes":SceneBiome::ALL.map(|b|b.name()),"textures":Material::ALL.map(|m|m.filenames()).to_vec(),"layers":self.atlas.layers,"stride":STRIDE})).unwrap();
    }
}
thread_local! { static KERNEL: RefCell<Kernel> = RefCell::new(Kernel::default()); }
#[unsafe(no_mangle)]
pub extern "C" fn terrain_input(length: usize) -> *mut u8 {
    KERNEL.with_borrow_mut(|k| {
        if length > MAX_INPUT {
            return std::ptr::null_mut();
        }
        k.input.resize(length, 0);
        k.input.as_mut_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_run() -> u32 {
    KERNEL.with_borrow_mut(|k| {
        let result = serde_json::from_slice::<Config>(&k.input)
            .map_err(|e| e.to_string())
            .and_then(|c| k.generate(c));
        match result {
            Ok(()) => 1,
            Err(e) => {
                k.json = serde_json::to_vec(&json!({"error":e})).unwrap();
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_metadata() {
    KERNEL.with_borrow_mut(Kernel::metadata);
}
macro_rules! exports {
    ($ptr:ident,$len:ident,$field:ident,$ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $ptr() -> *const $ty {
            KERNEL.with_borrow(|k| k.$field.as_ptr())
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $len() -> usize {
            KERNEL.with_borrow(|k| k.$field.len())
        }
    };
}
exports!(terrain_json_ptr, terrain_json_len, json, u8);
exports!(terrain_solid_ptr, terrain_solid_len, solid, u8);
exports!(terrain_water_ptr, terrain_water_len, water, u8);
exports!(terrain_lab_ptr, terrain_lab_len, lab, u8);
#[unsafe(no_mangle)]
pub extern "C" fn terrain_atlas_ptr() -> *const u8 {
    KERNEL.with_borrow(|k| k.atlas.pixels.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_atlas_len() -> usize {
    KERNEL.with_borrow(|k| k.atlas.pixels.len())
}
#[unsafe(no_mangle)]
pub extern "C" fn terrain_carved() -> u32 {
    KERNEL.with_borrow(|k| k.carved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_scene_shares_native_geometry_and_atlas() {
        let mut k = Kernel::default();
        let c = Config {
            size: 32,
            biome: "jungle".into(),
            villages: "showcase".into(),
            mines: "showcase".into(),
            ..Default::default()
        };
        k.generate(c).unwrap();
        assert_eq!(k.atlas.layers, 65);
        assert!(!k.solid.is_empty());
        assert!(k.solid.len().is_multiple_of(STRIDE * 4));
        assert!(k.solid.as_chunks::<STRIDE>().0.iter().all(|v| {
            v[..12]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| f32::from_le_bytes(*p).is_finite())
        }));
        let stats: serde_json::Value = serde_json::from_slice(&k.json).unwrap();
        assert!(stats["houses"].as_u64().unwrap() > 0);
        assert!(stats["mines"].as_u64().unwrap() > 0);
        let quads: u64 = stats["materials"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .sum();
        assert_eq!(
            (k.solid.len() + k.water.len()) as u64,
            quads * 4 * STRIDE as u64
        );
        assert!(
            k.solid
                .as_chunks::<STRIDE>()
                .0
                .iter()
                .any(|v| u16::from_le_bytes([v[16], v[17]]) > 9)
        );
    }
    #[test]
    fn invalid_config_does_not_replace_previous_mesh() {
        let mut k = Kernel::default();
        k.generate(Config {
            size: 32,
            ..Default::default()
        })
        .unwrap();
        let before = k.solid.clone();
        assert!(
            k.generate(Config {
                size: 999,
                ..Default::default()
            })
            .is_err()
        );
        assert_eq!(k.solid, before);
    }
    #[test]
    fn all_native_presets_validate_and_lab_has_three_slices() {
        let mut k = Kernel::default();
        k.metadata();
        let meta: serde_json::Value = serde_json::from_slice(&k.json).unwrap();
        assert_eq!(meta["presets"].as_array().unwrap().len(), 7);
        for value in meta["presets"].as_array().unwrap() {
            let preset: VisualizationPreset = serde_json::from_value(value.clone()).unwrap();
            preset.validate().unwrap();
        }
        k.generate(Config {
            size: 32,
            lab: true,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(k.lab.len(), 64 * 64 * 4 * 3);
    }
    #[test]
    fn biomes_change_the_complete_scene_deterministically() {
        let scene = |biome: &str| {
            let mut k = Kernel::default();
            k.generate(Config {
                size: 32,
                biome: biome.into(),
                villages: "off".into(),
                mines: "off".into(),
                ..Default::default()
            })
            .unwrap();
            (k.solid, k.json)
        };
        let desert = scene("desert");
        let forest = scene("forest");
        assert_ne!(desert.0, forest.0);
        assert_eq!(forest, scene("forest"));
        for biome in SceneBiome::ALL {
            assert!(!scene(biome.name()).0.is_empty());
        }
    }
    #[test]
    fn malformed_mapping_is_rejected_before_replacing_geometry() {
        let mut k = Kernel::default();
        k.generate(Config {
            size: 32,
            ..Default::default()
        })
        .unwrap();
        let before = k.solid.clone();
        let mut invalid = Config {
            size: 32,
            ..Default::default()
        };
        invalid.mappings.rules[0].input_range = [1.0, 0.0];
        assert!(k.generate(invalid).is_err());
        assert_eq!(before, k.solid);
    }
}
