//! Isolated MusicTerrain fields built with Horizon's seeded noise primitives.
use horizon_core::{
    noise::{PerlinNoise, SimplexNoise},
    random::LegacyRandom,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum TerrainError {
    #[error("invalid terrain parameter: {0}")]
    InvalidParameter(&'static str),
    #[error("terrain request is outside the bounded preview domain")]
    InvalidRequest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseType {
    Perlin,
    Simplex,
    #[default]
    Fbm,
    Ridged,
    Billow,
    Cellular,
    Value,
    Terraced,
}

impl NoiseType {
    pub const ALL: [Self; 8] = [
        Self::Perlin,
        Self::Simplex,
        Self::Fbm,
        Self::Ridged,
        Self::Billow,
        Self::Cellular,
        Self::Value,
        Self::Terraced,
    ];
    pub fn next(self) -> Self {
        Self::ALL[(Self::ALL.iter().position(|n| *n == self).unwrap() + 1) % Self::ALL.len()]
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|n| format!("{n:?}").eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerrainParameters {
    pub noise_type: NoiseType,
    pub frequency: f32,
    pub scale: f32,
    pub base_height: f32,
    pub amplitude: f32,
    pub octaves: u32,
    pub persistence: f32,
    pub lacunarity: f32,
    pub gain: f32,
    pub domain_warp: f32,
    pub density_threshold: f32,
    pub cave_threshold: f32,
    pub cave_w: f32,
    pub cave_speed: f32,
    pub ridge_strength: f32,
    pub erosion_influence: f32,
    pub structure_density: f32,
    pub detail: f32,
    pub island_strength: f32,
    pub color_shift: f32,
    pub particle_intensity: f32,
    pub shockwave: f32,
    pub effect_intensity: f32,
}
impl Default for TerrainParameters {
    fn default() -> Self {
        Self {
            noise_type: NoiseType::Fbm,
            frequency: 0.025,
            scale: 1.0,
            base_height: 12.0,
            amplitude: 20.0,
            octaves: 4,
            persistence: 0.5,
            lacunarity: 2.0,
            gain: 1.0,
            domain_warp: 2.0,
            density_threshold: 0.0,
            cave_threshold: 0.78,
            cave_w: 0.0,
            cave_speed: 0.12,
            ridge_strength: 0.0,
            erosion_influence: 0.0,
            structure_density: 0.0,
            detail: 1.0,
            island_strength: 0.0,
            color_shift: 0.0,
            particle_intensity: 0.0,
            shockwave: 0.0,
            effect_intensity: 1.0,
        }
    }
}
/// Numeric bounds shared by validation, mapping and the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetParameter {
    Frequency,
    Scale,
    BaseHeight,
    Amplitude,
    Persistence,
    Lacunarity,
    Gain,
    DomainWarp,
    DensityThreshold,
    CaveThreshold,
    CaveW,
    CaveSpeed,
    RidgeStrength,
    ErosionInfluence,
    StructureDensity,
    Detail,
    IslandStrength,
    ColorShift,
    ParticleIntensity,
    Shockwave,
    EffectIntensity,
}
impl TargetParameter {
    pub const ALL: [Self; 21] = [
        Self::Frequency,
        Self::Scale,
        Self::BaseHeight,
        Self::Amplitude,
        Self::Persistence,
        Self::Lacunarity,
        Self::Gain,
        Self::DomainWarp,
        Self::DensityThreshold,
        Self::CaveThreshold,
        Self::CaveW,
        Self::CaveSpeed,
        Self::RidgeStrength,
        Self::ErosionInfluence,
        Self::StructureDensity,
        Self::Detail,
        Self::IslandStrength,
        Self::ColorShift,
        Self::ParticleIntensity,
        Self::Shockwave,
        Self::EffectIntensity,
    ];
    pub fn bounds(self) -> [f32; 2] {
        match self {
            Self::Frequency => [0.0001, 1.0],
            Self::Scale => [0.01, 100.0],
            Self::BaseHeight => [-128.0, 256.0],
            Self::Amplitude => [0.0, 256.0],
            Self::Persistence => [0.0, 1.0],
            Self::Lacunarity => [1.0, 4.0],
            Self::Gain => [0.0, 8.0],
            Self::DomainWarp => [0.0, 128.0],
            Self::DensityThreshold => [-64.0, 64.0],
            Self::CaveThreshold => [0.0, 1.0],
            Self::CaveW => [-128.0, 128.0],
            Self::CaveSpeed => [0.0, 2.0],
            Self::RidgeStrength
            | Self::ErosionInfluence
            | Self::StructureDensity
            | Self::IslandStrength
            | Self::ParticleIntensity
            | Self::EffectIntensity => [0.0, 1.0],
            Self::Detail => [0.0, 8.0],
            Self::ColorShift => [-1.0, 1.0],
            Self::Shockwave => [0.0, 32.0],
        }
    }
    pub fn get(self, p: &TerrainParameters) -> f32 {
        match self {
            Self::Frequency => p.frequency,
            Self::Scale => p.scale,
            Self::BaseHeight => p.base_height,
            Self::Amplitude => p.amplitude,
            Self::Persistence => p.persistence,
            Self::Lacunarity => p.lacunarity,
            Self::Gain => p.gain,
            Self::DomainWarp => p.domain_warp,
            Self::DensityThreshold => p.density_threshold,
            Self::CaveThreshold => p.cave_threshold,
            Self::CaveW => p.cave_w,
            Self::CaveSpeed => p.cave_speed,
            Self::RidgeStrength => p.ridge_strength,
            Self::ErosionInfluence => p.erosion_influence,
            Self::StructureDensity => p.structure_density,
            Self::Detail => p.detail,
            Self::IslandStrength => p.island_strength,
            Self::ColorShift => p.color_shift,
            Self::ParticleIntensity => p.particle_intensity,
            Self::Shockwave => p.shockwave,
            Self::EffectIntensity => p.effect_intensity,
        }
    }
    pub fn set(self, p: &mut TerrainParameters, v: f32) {
        match self {
            Self::Frequency => p.frequency = v,
            Self::Scale => p.scale = v,
            Self::BaseHeight => p.base_height = v,
            Self::Amplitude => p.amplitude = v,
            Self::Persistence => p.persistence = v,
            Self::Lacunarity => p.lacunarity = v,
            Self::Gain => p.gain = v,
            Self::DomainWarp => p.domain_warp = v,
            Self::DensityThreshold => p.density_threshold = v,
            Self::CaveThreshold => p.cave_threshold = v,
            Self::CaveW => p.cave_w = v,
            Self::CaveSpeed => p.cave_speed = v,
            Self::RidgeStrength => p.ridge_strength = v,
            Self::ErosionInfluence => p.erosion_influence = v,
            Self::StructureDensity => p.structure_density = v,
            Self::Detail => p.detail = v,
            Self::IslandStrength => p.island_strength = v,
            Self::ColorShift => p.color_shift = v,
            Self::ParticleIntensity => p.particle_intensity = v,
            Self::Shockwave => p.shockwave = v,
            Self::EffectIntensity => p.effect_intensity = v,
        }
    }
}
impl TerrainParameters {
    pub fn validate(&self) -> Result<(), TerrainError> {
        if !(1..=8).contains(&self.octaves) {
            return Err(TerrainError::InvalidParameter("octaves must be 1..8"));
        }
        for t in TargetParameter::ALL {
            let v = t.get(self);
            let [min, max] = t.bounds();
            if !v.is_finite() || v < min || v > max {
                return Err(TerrainError::InvalidParameter(
                    "nonfinite or out-of-range field",
                ));
            }
        }
        Ok(())
    }
    pub fn clamped(&self) -> Self {
        let mut p = self.clone();
        let defaults = Self::default();
        p.octaves = p.octaves.clamp(1, 8);
        for t in TargetParameter::ALL {
            let [min, max] = t.bounds();
            let v = t.get(&p);
            t.set(
                &mut p,
                if v.is_finite() {
                    v.clamp(min, max)
                } else {
                    t.get(&defaults)
                },
            );
        }
        p
    }
}

#[derive(Clone, Debug)]
pub struct TerrainField {
    seed: i64,
    perlin: PerlinNoise,
    simplex: SimplexNoise,
    warp: PerlinNoise,
}
impl TerrainField {
    pub fn new(seed: i64) -> Self {
        let mut rng = LegacyRandom::new(seed);
        Self {
            seed,
            perlin: PerlinNoise::new(&mut rng),
            simplex: SimplexNoise::new(&mut rng),
            warp: PerlinNoise::new(&mut rng),
        }
    }
    pub fn seed(&self) -> i64 {
        self.seed
    }
    fn noise(&self, x: f64, y: f64, z: f64, kind: NoiseType) -> f32 {
        if kind == NoiseType::Value {
            return crate::noise4::value(self.seed, [x, y, z, 0.0]);
        }
        if kind == NoiseType::Cellular {
            return crate::noise4::cellular(self.seed, x, y, z);
        }
        if kind == NoiseType::Simplex {
            self.simplex.get3(
                horizon_core::noise::wrap(x),
                horizon_core::noise::wrap(y),
                horizon_core::noise::wrap(z),
            )
        } else {
            self.perlin.get3(x, y, z)
        }
    }
    fn fractal(&self, x: f64, y: f64, z: f64, p: &TerrainParameters) -> f32 {
        let mut f = 1.0;
        let mut a = 1.0;
        let mut total = 0.0;
        let mut weight = 0.0;
        let octaves = if matches!(
            p.noise_type,
            NoiseType::Perlin | NoiseType::Simplex | NoiseType::Cellular | NoiseType::Value
        ) {
            1
        } else {
            p.octaves
        };
        for _ in 0..octaves {
            let raw = self.noise(x * f, y * f, z * f, p.noise_type);
            let n = match p.noise_type {
                NoiseType::Billow => raw.abs() * 2.0 - 1.0,
                NoiseType::Terraced => (raw * 6.0).floor() / 6.0,
                _ => raw,
            };
            let ridge = 1.0 - 2.0 * n.abs();
            let mix = if p.noise_type == NoiseType::Ridged {
                1.0
            } else {
                p.ridge_strength
            };
            total += (n * (1.0 - mix) + ridge * mix) * a;
            weight += a;
            a *= p.persistence;
            f *= f64::from(p.lacunarity);
        }
        if weight > 0.0 { total / weight } else { 0.0 }
    }
    /// World coordinates only: callers can tile requests without local-coordinate seams.
    /// Invalid parameters are sanitized; validated constructors/editor report the original error.
    pub fn height(&self, x: f64, z: f64, p: &TerrainParameters) -> f32 {
        if !x.is_finite() || !z.is_finite() || x.abs() > 30_000_000.0 || z.abs() > 30_000_000.0 {
            return 0.0;
        }
        let p = p.clamped();
        let frequency = f64::from(p.frequency / p.scale);
        let wx = self.warp.get3(x * frequency, 7.3, z * frequency) * p.domain_warp;
        let wz = self.warp.get3(x * frequency, 49.1, z * frequency) * p.domain_warp;
        let u = (x + f64::from(wx)) * frequency;
        let v = (z + f64::from(wz)) * frequency;
        let n = self.fractal(u, 0.0, v, &p);
        let detail = self.perlin.get3(u * 7.0, 23.0, v * 7.0) * p.detail;
        let erosion = 1.0 - p.erosion_influence * (n.abs().min(1.0));
        (p.base_height + (n * p.amplitude * erosion + detail) * p.gain * p.effect_intensity)
            .clamp(-256.0, 512.0)
    }
    /// Positive density is solid; caves and islands are independent 3D fields.
    pub fn density(&self, x: f64, y: f64, z: f64, p: &TerrainParameters) -> f32 {
        if !x.is_finite()
            || !y.is_finite()
            || !z.is_finite()
            || x.abs() > 30_000_000.0
            || y.abs() > 30_000_000.0
            || z.abs() > 30_000_000.0
        {
            return -1.0;
        }
        let p = p.clamped();
        let surface = self.height(x, z, &p);
        let f = f64::from(p.frequency / p.scale) * 2.5;
        let mut cave = self
            .simplex
            .get3(
                horizon_core::noise::wrap(x * f),
                horizon_core::noise::wrap(y * f),
                horizon_core::noise::wrap(z * f),
            )
            .abs()
            .min(1.0);
        if p.cave_w != 0.0 {
            cave = (cave
                + (crate::noise4::value(self.seed, [x * f, y * f, z * f, f64::from(p.cave_w)])
                    - crate::noise4::value(self.seed, [x * f, y * f, z * f, 0.0]))
                    * 0.5)
                .clamp(0.0, 1.0);
        }
        let ground = surface - y as f32 - p.density_threshold;
        let carved = if y < (f64::from(surface) - 3.0) && cave > p.cave_threshold {
            ground.min((p.cave_threshold - cave) * 32.0)
        } else {
            ground
        };
        let island_noise = self.perlin.get3(x * f, y * f, z * f);
        let island = 12.0 * (island_noise - 0.15) - (y as f32 - p.base_height - 25.0).abs() * 0.32;
        let result = if p.island_strength > 0.0 {
            carved.max(island - (1.0 - p.island_strength) * 32.0)
        } else {
            carved
        };
        result.clamp(-1.0e6, 1.0e6)
    }
    pub fn heightmap(
        &self,
        origin: [i32; 2],
        size: [u32; 2],
        p: &TerrainParameters,
    ) -> Result<Vec<f32>, TerrainError> {
        p.validate()?;
        if size.contains(&0) || size[0] > 1024 || size[1] > 1024 {
            return Err(TerrainError::InvalidRequest);
        }
        let mut out = Vec::with_capacity((size[0] * size[1]) as usize);
        for z in 0..size[1] {
            for x in 0..size[0] {
                out.push(self.height(
                    f64::from(origin[0]) + f64::from(x),
                    f64::from(origin[1]) + f64::from(z),
                    p,
                ));
            }
        }
        Ok(out)
    }
    /// Bounded, independently generated solid mask, X-fast, then Z, then Y.
    /// Adapter code must resolve its material IDs through Horizon's BlockStates.
    pub fn generate(&self, request: &TerrainGenerationRequest) -> Result<Vec<bool>, TerrainError> {
        request.snapshot.parameters.validate()?;
        let [sx, sy, sz] = request.size;
        if sx == 0
            || sy == 0
            || sz == 0
            || sx > 256
            || sy > 512
            || sz > 256
            || u64::from(sx) * u64::from(sy) * u64::from(sz) > 2_097_152
            || request.snapshot.seed != self.seed
        {
            return Err(TerrainError::InvalidRequest);
        }
        let mut out = Vec::with_capacity((sx * sy * sz) as usize);
        for y in 0..sy {
            for z in 0..sz {
                for x in 0..sx {
                    out.push(
                        self.density(
                            f64::from(request.origin[0]) + f64::from(x),
                            f64::from(request.origin[1]) + f64::from(y),
                            f64::from(request.origin[2]) + f64::from(z),
                            &request.snapshot.parameters,
                        ) > 0.0,
                    );
                }
            }
        }
        Ok(out)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GenerationRevision(pub u64);
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainSnapshot {
    pub revision: GenerationRevision,
    pub seed: i64,
    pub parameters: TerrainParameters,
    pub audio_time: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainGenerationRequest {
    pub origin: [i32; 3],
    pub size: [u32; 3],
    pub snapshot: TerrainSnapshot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeMappingMode {
    #[default]
    Clamped,
    Loop,
    Finite,
}
/// Maps X to a selected audio interval without consulting playback or wall time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTimeMapping {
    pub origin_x: f64,
    pub blocks_per_second: f64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub mode: TimeMappingMode,
}
impl AudioTimeMapping {
    pub fn validate(&self) -> Result<(), TerrainError> {
        if !self.origin_x.is_finite()
            || !self.blocks_per_second.is_finite()
            || !(0.001..=1.0e6).contains(&self.blocks_per_second)
            || !self.start_seconds.is_finite()
            || !self.end_seconds.is_finite()
            || self.start_seconds < 0.0
            || self.end_seconds <= self.start_seconds
            || self.end_seconds > 86_400.0
        {
            return Err(TerrainError::InvalidParameter(
                "invalid audio spatial mapping",
            ));
        }
        Ok(())
    }
    pub fn time_at(&self, world_x: f64) -> Result<Option<f64>, TerrainError> {
        self.validate()?;
        if !world_x.is_finite() {
            return Err(TerrainError::InvalidRequest);
        }
        let relative = (world_x - self.origin_x) / self.blocks_per_second;
        if !relative.is_finite() {
            return Err(TerrainError::InvalidRequest);
        }
        let duration = self.end_seconds - self.start_seconds;
        Ok(match self.mode {
            TimeMappingMode::Clamped => Some(self.start_seconds + relative.clamp(0.0, duration)),
            TimeMappingMode::Loop => Some(self.start_seconds + relative.rem_euclid(duration)),
            TimeMappingMode::Finite => {
                if (0.0..=duration).contains(&relative) {
                    Some(self.start_seconds + relative)
                } else {
                    None
                }
            }
        })
    }
}
