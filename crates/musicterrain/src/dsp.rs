//! Deterministic, fixed-hop FFT analysis. The FFT plan and scratch memory are reused.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};

use crate::audio::DecodedAudio;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioFeatures {
    pub time: f64,
    pub rms: f32,
    /// Window-energy calibrated RMS in each frequency band (not FFT bin indices).
    pub bands: [f32; 7],
    pub centroid: f32,
    pub flux: f32,
    pub onset: bool,
    pub beat_strength: f32,
    /// Only present after at least four regular, well-separated onset events.
    pub beat_phase: Option<f32>,
    pub silence: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioFrame {
    pub time: f64,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrequencyBand {
    pub low_hz: f32,
    pub high_hz: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub fft_size: usize,
    pub hop_size: usize,
    pub bands: [FrequencyBand; 7],
    pub silence_threshold: f32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            fft_size: 4096,
            hop_size: 512,
            bands: [
                (20.0, 80.0),
                (80.0, 250.0),
                (250.0, 500.0),
                (500.0, 2000.0),
                (2000.0, 6000.0),
                (6000.0, 12000.0),
                (12000.0, 20000.0),
            ]
            .map(|(low_hz, high_hz)| FrequencyBand { low_hz, high_hz }),
            silence_threshold: 1e-5,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DspError {
    #[error("FFT size must be a power of two in 256..=32768; hop must be 64..=FFT size")]
    InvalidFft,
    #[error("frequency bands and silence threshold must be finite, positive and ordered")]
    InvalidBand,
    #[error("audio PCM is empty, nonfinite or has an invalid sample rate")]
    InvalidAudio,
    #[error("analysis would exceed the one-million-frame limit")]
    TooManyFrames,
    #[error("audio analysis cancelled")]
    Cancelled,
}

impl AnalysisConfig {
    pub fn validate(&self) -> Result<(), DspError> {
        if !self.fft_size.is_power_of_two()
            || !(256..=32768).contains(&self.fft_size)
            || !(64..=self.fft_size).contains(&self.hop_size)
        {
            return Err(DspError::InvalidFft);
        }
        if !self.silence_threshold.is_finite()
            || !(0.0..=0.1).contains(&self.silence_threshold)
            || self.bands.iter().any(|band| {
                !band.low_hz.is_finite()
                    || !band.high_hz.is_finite()
                    || band.low_hz < 0.0
                    || band.high_hz <= band.low_hz
                    || band.high_hz > 192000.0
            })
        {
            return Err(DspError::InvalidBand);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioTimeline {
    pub frames: Vec<AudioFeatures>,
    pub duration: f64,
    pub sample_rate: u32,
    pub hop_size: usize,
}

impl AudioTimeline {
    pub fn analyze(audio: &DecodedAudio, config: &AnalysisConfig) -> Result<Self, DspError> {
        Self::analyze_cancellable(audio, config, &AtomicBool::new(false))
    }

    pub fn analyze_cancellable(
        audio: &DecodedAudio,
        config: &AnalysisConfig,
        cancelled: &AtomicBool,
    ) -> Result<Self, DspError> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(DspError::Cancelled);
        }
        config.validate()?;
        if audio.samples.is_empty()
            || !(8000..=384000).contains(&audio.sample_rate)
            || audio.samples.iter().any(|v| !v.is_finite())
        {
            return Err(DspError::InvalidAudio);
        }
        let count = audio.samples.len().div_ceil(config.hop_size);
        if count > 1_000_000 {
            return Err(DspError::TooManyFrames);
        }
        let mut analyzer = Analyzer::new(audio.sample_rate, config.clone())?;
        let mut frames = Vec::with_capacity(count);
        for start in (0..audio.samples.len()).step_by(config.hop_size) {
            if cancelled.load(Ordering::Relaxed) {
                return Err(DspError::Cancelled);
            }
            // A frame is centred on its timestamp. Zero padding handles both boundaries,
            // so the preview does not look a full FFT window ahead of the playback clock.
            frames.push(analyzer.analyze_centered(&audio.samples, start));
        }
        Ok(Self {
            frames,
            duration: audio.duration(),
            sample_rate: audio.sample_rate,
            hop_size: config.hop_size,
        })
    }

    /// Fixed-hop nearest-past lookup. Seeking never advances smoothing or onset history.
    pub fn at(&self, time: f64) -> AudioFeatures {
        if self.frames.is_empty() || self.hop_size == 0 {
            return AudioFeatures {
                silence: true,
                ..Default::default()
            };
        }
        let time = if time.is_nan() { 0.0 } else { time.max(0.0) };
        // Searching stored sample-derived timestamps also avoids a floating-point
        // round-trip putting an exact frame time one frame before itself.
        let after = self.frames.partition_point(|frame| frame.time <= time);
        self.frames[after.saturating_sub(1).min(self.frames.len() - 1)]
    }
}

pub struct Analyzer {
    sample_rate: u32,
    config: AnalysisConfig,
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    window: Vec<f32>,
    energy_normalization: f32,
    previous: Vec<f32>,
    flux_average: f32,
    last_onset: Option<f64>,
    intervals: [f64; 4],
    interval_count: usize,
}

impl Analyzer {
    pub fn new(sample_rate: u32, config: AnalysisConfig) -> Result<Self, DspError> {
        config.validate()?;
        if !(8000..=384000).contains(&sample_rate) {
            return Err(DspError::InvalidAudio);
        }
        let n = config.fft_size;
        let window: Vec<_> = (0..n)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
            .collect();
        let energy_normalization = 1.0 / (n as f32 * window.iter().map(|v| v * v).sum::<f32>());
        let fft = FftPlanner::new().plan_fft_forward(n);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Ok(Self {
            sample_rate,
            config,
            fft,
            scratch,
            window,
            energy_normalization,
            buffer: vec![Complex::default(); n],
            previous: vec![0.0; n / 2 + 1],
            flux_average: 0.0,
            last_onset: None,
            intervals: [0.0; 4],
            interval_count: 0,
        })
    }

    fn analyze_centered(&mut self, samples: &[f32], center: usize) -> AudioFeatures {
        let half = self.config.fft_size / 2;
        for i in 0..self.config.fft_size {
            let source = center.checked_add(i).and_then(|v| v.checked_sub(half));
            let value = source.and_then(|v| samples.get(v)).copied().unwrap_or(0.0);
            self.buffer[i] = Complex::new(value.clamp(-1.0, 1.0) * self.window[i], 0.0);
        }
        self.finish(center as f64 / self.sample_rate as f64)
    }

    /// Streaming analysis of exactly one window. Caller supplies a monotonic sample timestamp.
    /// Do not call this from a device callback: enqueue PCM to a bounded worker queue instead.
    pub fn analyze_window(
        &mut self,
        samples: &[f32],
        time: f64,
    ) -> Result<AudioFeatures, DspError> {
        if samples.len() != self.buffer.len()
            || !time.is_finite()
            || samples.iter().any(|v| !v.is_finite())
        {
            return Err(DspError::InvalidAudio);
        }
        for (i, &sample) in samples.iter().enumerate() {
            self.buffer[i] = Complex::new(sample.clamp(-1.0, 1.0) * self.window[i], 0.0);
        }
        Ok(self.finish(time))
    }

    fn finish(&mut self, time: f64) -> AudioFeatures {
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let mut energy = 0.0;
        let mut bands = [0.0_f32; 7];
        let mut weighted_frequency = 0.0;
        let mut flux_power = 0.0;
        let n = self.config.fft_size;
        let nyquist = self.sample_rate as f32 * 0.5;
        for index in 0..=n / 2 {
            let one_sided = if index == 0 || index == n / 2 {
                1.0
            } else {
                2.0
            };
            let power = self.buffer[index].norm_sqr() * one_sided * self.energy_normalization;
            let magnitude = power.sqrt();
            let frequency = index as f32 * self.sample_rate as f32 / n as f32;
            energy += power;
            weighted_frequency += frequency * power;
            flux_power += (magnitude - self.previous[index]).max(0.0).powi(2);
            self.previous[index] = magnitude;
            for (band, total) in self.config.bands.iter().zip(&mut bands) {
                // Upper edges are exclusive; Nyquist itself is included in the final clipped band.
                if frequency >= band.low_hz
                    && (frequency < band.high_hz.min(nyquist)
                        || (frequency == nyquist
                            && band.high_hz >= nyquist
                            && band.low_hz < nyquist))
                {
                    *total += power;
                }
            }
        }
        let rms = energy.sqrt();
        let silence = rms <= self.config.silence_threshold;
        let flux = if silence { 0.0 } else { flux_power.sqrt() };
        let onset = !silence
            && flux > self.flux_average * 1.8 + 0.003
            && self.last_onset.is_none_or(|last| time - last >= 0.15);
        self.flux_average += 0.08 * (flux - self.flux_average);
        if onset {
            if let Some(last) = self.last_onset {
                let interval = time - last;
                if (0.2..=2.0).contains(&interval) {
                    self.intervals[self.interval_count % 4] = interval;
                    self.interval_count += 1;
                } else {
                    self.interval_count = 0;
                }
            }
            self.last_onset = Some(time);
        }
        let beat_phase = if self.interval_count >= 3 && !silence {
            let count = self.interval_count.min(4);
            let mean = self.intervals[..count].iter().sum::<f64>() / count as f64;
            let regular = self.intervals[..count]
                .iter()
                .all(|v| (v - mean).abs() <= mean * 0.15);
            regular
                .then(|| ((time - self.last_onset.unwrap_or(time)) / mean).rem_euclid(1.0) as f32)
        } else {
            None
        };
        AudioFeatures {
            time,
            rms: if silence { 0.0 } else { rms },
            bands: if silence {
                [0.0; 7]
            } else {
                bands.map(f32::sqrt)
            },
            centroid: if silence {
                0.0
            } else {
                weighted_frequency / energy.max(f32::MIN_POSITIVE)
            },
            flux,
            onset,
            beat_strength: if silence {
                0.0
            } else {
                (flux / (rms + 0.03)).clamp(0.0, 1.0)
            },
            beat_phase,
            silence,
        }
    }
}
