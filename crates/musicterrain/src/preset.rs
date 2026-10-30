//! Versioned, validated presets. Audio and renderer state are deliberately excluded.
use crate::{
    mapping::{AudioSource, CombineMode, MappingGraph, MappingRule},
    terrain::{NoiseType, TargetParameter, TerrainParameters},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PRESET_VERSION: u32 = 1;
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("preset JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("preset IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported preset version {0}")]
    Version(u32),
    #[error("invalid preset: {0}")]
    Invalid(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisualizationPreset {
    pub version: u32,
    pub name: String,
    pub seed: i64,
    pub parameters: TerrainParameters,
    pub mappings: MappingGraph,
}
impl VisualizationPreset {
    pub fn validate(&self) -> Result<(), PresetError> {
        if self.version != PRESET_VERSION {
            return Err(PresetError::Version(self.version));
        }
        if self.name.trim().is_empty() || self.name.len() > 128 {
            return Err(PresetError::Invalid("name must be 1..128 bytes".into()));
        }
        self.parameters
            .validate()
            .map_err(|e| PresetError::Invalid(e.to_string()))?;
        self.mappings
            .validate()
            .map_err(|e| PresetError::Invalid(e.to_string()))?;
        Ok(())
    }
    pub fn from_json(json: &str) -> Result<Self, PresetError> {
        if json.len() > 1_048_576 {
            return Err(PresetError::Invalid("preset exceeds 1 MiB".into()));
        }
        let mut value: serde_json::Value = serde_json::from_str(json)?;
        // Version 0 prototypes used the same schema before explicit versioning.
        if value.get("version").and_then(|v| v.as_u64()) == Some(0) {
            value["version"] = PRESET_VERSION.into();
        }
        let preset: Self = serde_json::from_value(value)?;
        preset.validate()?;
        Ok(preset)
    }
    pub fn to_json(&self) -> Result<String, PresetError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PresetError> {
        use std::io::Read;
        let mut json = String::new();
        std::fs::File::open(path)?
            .take(1_048_577)
            .read_to_string(&mut json)?;
        Self::from_json(&json)
    }
    /// Creates a new preset file, preserving any existing user preset.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PresetError> {
        use std::io::Write;
        let json = self.to_json()?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }
    pub fn hash(&self) -> Result<String, PresetError> {
        self.validate()?;
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}
fn default_mappings() -> MappingGraph {
    let mut rules = vec![
        MappingRule::band(0, TargetParameter::Amplitude, [0.0, 40.0]),
        MappingRule::band(1, TargetParameter::BaseHeight, [0.0, 12.0]),
        MappingRule::band(2, TargetParameter::CaveThreshold, [0.0, -0.35]),
        MappingRule::band(3, TargetParameter::DomainWarp, [0.0, 24.0]),
        MappingRule::band(4, TargetParameter::Detail, [0.0, 5.0]),
        MappingRule::band(5, TargetParameter::ColorShift, [0.0, 0.8]),
        MappingRule::band(5, TargetParameter::ParticleIntensity, [0.0, 1.0]),
    ];
    let mut onset = MappingRule::band(0, TargetParameter::Shockwave, [0.0, 12.0]);
    onset.source = AudioSource::Onset;
    onset.input_range = [0.0, 1.0];
    onset.attack = 0.0;
    onset.release = 0.2;
    rules.push(onset);
    let mut cave_w = MappingRule::band(1, TargetParameter::CaveW, [0.0, 2.0]);
    cave_w.source = AudioSource::Rms;
    cave_w.attack = 0.18;
    cave_w.release = 0.4;
    rules.push(cave_w);
    let mut rms = MappingRule::band(0, TargetParameter::EffectIntensity, [0.25, 1.0]);
    rms.source = AudioSource::Rms;
    rms.combine = CombineMode::Multiply;
    rules.push(rms);
    MappingGraph { rules }
}
pub fn builtins() -> Vec<VisualizationPreset> {
    let names = [
        "Bass Mountains",
        "Wave Terrain",
        "Noise Storm",
        "Floating Islands",
        "Crystal Pulse",
        "Ambient Landscape",
        "Extreme Terrain",
    ];
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut p = TerrainParameters::default();
            match i {
                0 => {
                    p.noise_type = NoiseType::Ridged;
                    p.amplitude = 36.0;
                    p.frequency = 0.018;
                    p.ridge_strength = 0.7;
                }
                1 => {
                    p.noise_type = NoiseType::Simplex;
                    p.frequency = 0.035;
                    p.amplitude = 12.0;
                    p.domain_warp = 16.0;
                }
                2 => {
                    p.frequency = 0.09;
                    p.amplitude = 32.0;
                    p.domain_warp = 38.0;
                    p.detail = 6.0;
                    p.octaves = 6;
                }
                3 => {
                    p.base_height = 3.0;
                    p.amplitude = 10.0;
                    p.island_strength = 1.0;
                    p.cave_threshold = 0.35;
                }
                4 => {
                    p.noise_type = NoiseType::Ridged;
                    p.frequency = 0.08;
                    p.amplitude = 48.0;
                    p.structure_density = 0.8;
                    p.ridge_strength = 1.0;
                }
                5 => {
                    p.frequency = 0.012;
                    p.amplitude = 8.0;
                    p.detail = 0.2;
                    p.erosion_influence = 0.8;
                }
                _ => {
                    p.amplitude = 90.0;
                    p.frequency = 0.065;
                    p.domain_warp = 64.0;
                    p.cave_threshold = 0.35;
                    p.island_strength = 0.8;
                    p.detail = 8.0;
                }
            }
            VisualizationPreset {
                version: PRESET_VERSION,
                name: (*name).to_owned(),
                seed: 42,
                parameters: p,
                mappings: default_mappings(),
            }
        })
        .collect()
}
