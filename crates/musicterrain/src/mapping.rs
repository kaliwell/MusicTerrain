//! Ordered feature mapping. Smoothing state belongs to an audio timeline, never a render thread.
use crate::{
    dsp::AudioFeatures,
    terrain::{TargetParameter, TerrainError, TerrainParameters},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioSource {
    Band(u8),
    Rms,
    Centroid,
    Flux,
    Onset,
    BeatStrength,
    BeatPhase,
}
impl AudioSource {
    pub fn sample(self, f: &AudioFeatures) -> f32 {
        let value = match self {
            Self::Band(i) => f.bands.get(usize::from(i)).copied().unwrap_or(0.0),
            Self::Rms => f.rms,
            Self::Centroid => f.centroid,
            Self::Flux => f.flux,
            Self::Onset => {
                if f.onset {
                    1.0
                } else {
                    0.0
                }
            }
            Self::BeatStrength => f.beat_strength,
            Self::BeatPhase => f.beat_phase.unwrap_or(0.0),
        };
        if value.is_finite() { value } else { 0.0 }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResponseCurve {
    Linear,
    Smoothstep,
    Power(f32),
    Logarithmic,
}
impl ResponseCurve {
    fn apply(self, x: f32) -> f32 {
        match self {
            Self::Linear => x,
            Self::Smoothstep => x * x * (3.0 - 2.0 * x),
            Self::Power(p) => x.signum() * x.abs().powf(p),
            Self::Logarithmic => x.signum() * (1.0 + 9.0 * x.abs()).ln() / 10.0_f32.ln(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombineMode {
    Replace,
    Add,
    Multiply,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MappingRule {
    pub source: AudioSource,
    pub target: TargetParameter,
    pub input_range: [f32; 2],
    pub output_range: [f32; 2],
    pub gain: f32,
    pub bias: f32,
    pub curve: ResponseCurve,
    /// Seconds. Zero is an instantaneous response.
    pub attack: f32,
    pub release: f32,
    pub clamp: bool,
    pub enabled: bool,
    pub combine: CombineMode,
}
impl MappingRule {
    pub fn band(band: u8, target: TargetParameter, output_range: [f32; 2]) -> Self {
        Self {
            source: AudioSource::Band(band),
            target,
            input_range: [0.0, 0.5],
            output_range,
            gain: 1.0,
            bias: 0.0,
            curve: ResponseCurve::Smoothstep,
            attack: 0.035,
            release: 0.25,
            clamp: true,
            enabled: true,
            combine: CombineMode::Add,
        }
    }
    pub fn validate(&self) -> Result<(), TerrainError> {
        if matches!(self.source,AudioSource::Band(i)if i>=7) {
            return Err(TerrainError::InvalidParameter("mapping band must be 0..6"));
        }
        if !self
            .input_range
            .iter()
            .chain(self.output_range.iter())
            .chain([&self.gain, &self.bias, &self.attack, &self.release])
            .all(|v| v.is_finite())
            || self.input_range[0] >= self.input_range[1]
            || self.attack < 0.0
            || self.release < 0.0
            || self.attack > 60.0
            || self.release > 60.0
            || self.gain.abs() > 1000.0
            || self.bias.abs() > 1000.0
            || self
                .input_range
                .iter()
                .chain(self.output_range.iter())
                .any(|v| v.abs() > 1.0e6)
        {
            return Err(TerrainError::InvalidParameter(
                "invalid mapping range or response",
            ));
        }
        if let ResponseCurve::Power(p) = self.curve
            && (!p.is_finite() || !(0.1..=8.0).contains(&p))
        {
            return Err(TerrainError::InvalidParameter(
                "curve exponent must be 0.1..8",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MappingGraph {
    pub rules: Vec<MappingRule>,
}
#[derive(Clone, Debug, Default)]
pub struct MappingState {
    rules: Vec<MappingRule>,
    values: Vec<f32>,
}
impl MappingState {
    pub fn reset(&mut self) {
        self.rules.clear();
        self.values.clear();
    }
}
impl MappingGraph {
    pub fn validate(&self) -> Result<(), TerrainError> {
        if self.rules.len() > 128 {
            return Err(TerrainError::InvalidParameter("at most 128 mappings"));
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        Ok(())
    }
    /// Rule order is vector order. A caller must reset/replay fixed audio hops after seek.
    /// A zero dt leaves smoothing unchanged, making paused previews stable.
    pub fn evaluate(
        &self,
        features: &AudioFeatures,
        base: &TerrainParameters,
        dt: f32,
        state: &mut MappingState,
    ) -> Result<TerrainParameters, TerrainError> {
        self.validate()?;
        base.validate()?;
        if !dt.is_finite() || !(0.0..=10.0).contains(&dt) {
            return Err(TerrainError::InvalidParameter(
                "mapping timestep must be 0..10 seconds",
            ));
        }
        if state.rules != self.rules {
            state.rules = self.rules.clone();
            state.values = vec![0.0; self.rules.len()];
        }
        let mut out = base.clone();
        for (i, rule) in self.rules.iter().enumerate() {
            if !rule.enabled {
                continue;
            }
            let input = rule.source.sample(features);
            let mut normalized = ((input - rule.input_range[0])
                / (rule.input_range[1] - rule.input_range[0]))
                * rule.gain
                + rule.bias;
            if rule.clamp {
                normalized = normalized.clamp(0.0, 1.0);
            }
            // Bound intermediates even for unclamped mappings and reject nonfinite inputs above.
            let curved = rule
                .curve
                .apply(normalized.clamp(-8.0, 8.0))
                .clamp(-1.0e6, 1.0e6);
            let tau = if curved > state.values[i] {
                rule.attack
            } else {
                rule.release
            };
            let alpha = if dt == 0.0 {
                0.0
            } else if tau == 0.0 {
                1.0
            } else {
                1.0 - (-dt / tau).exp()
            };
            state.values[i] += (curved - state.values[i]) * alpha;
            let mapped = rule.output_range[0]
                + state.values[i] * (rule.output_range[1] - rule.output_range[0]);
            let old = rule.target.get(&out);
            let combined = match rule.combine {
                CombineMode::Replace => mapped,
                CombineMode::Add => old + mapped,
                CombineMode::Multiply => old * mapped,
            };
            let [min, max] = rule.target.bounds();
            rule.target.set(&mut out, combined.clamp(min, max));
        }
        Ok(out)
    }
}
