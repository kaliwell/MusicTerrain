//! File decoding and deterministic PCM preparation. No work here runs in an audio callback.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use sha2::{Digest, Sha256};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as DecodeError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const ANALYSIS_SAMPLE_RATE: u32 = 48_000;
/// Decoded mono PCM is capped at 20 minutes (about 220 MiB at 48 kHz).
pub const MAX_AUDIO_SECONDS: usize = 20 * 60;
const MAX_SAMPLES: usize = ANALYSIS_SAMPLE_RATE as usize * MAX_AUDIO_SECONDS;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("could not read audio: {0}")]
    Io(#[from] std::io::Error),
    #[error("audio decoder: {0}")]
    Decode(#[from] DecodeError),
    #[error("audio contains no decodable samples")]
    Empty,
    #[error("audio sample rate or channel layout is invalid or changed mid-stream")]
    InvalidFormat,
    #[error("audio exceeds the 20-minute decoded PCM limit")]
    TooLong,
    #[error("audio contains nonfinite PCM samples")]
    Nonfinite,
    #[error("audio import cancelled")]
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// SHA-256 of the original encoded bytes; synthetic inputs hash source PCM and rate.
    pub content_hash: String,
}

impl DecodedAudio {
    pub fn decode(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        Self::decode_cancellable(path, &AtomicBool::new(false))
    }

    /// Cooperatively interrupt file hashing, packet decoding and sample-rate conversion.
    pub fn decode_cancellable(
        path: impl AsRef<Path>,
        cancelled: &AtomicBool,
    ) -> Result<Self, AudioError> {
        check_cancelled(cancelled)?;
        let path = path.as_ref();
        let mut hash = Sha256::new();
        let mut file = File::open(path)?;
        let mut bytes = [0_u8; 64 * 1024];
        loop {
            check_cancelled(cancelled)?;
            let count = file.read(&mut bytes)?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        let content_hash = format!("{:x}", hash.finalize());
        file.seek(SeekFrom::Start(0))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(extension);
        }
        let probed = symphonia::default::get_probe().format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )?;
        let mut format = probed.format;
        let track = format.default_track().ok_or(AudioError::InvalidFormat)?;
        let track_id = track.id;
        let rate = track
            .codec_params
            .sample_rate
            .ok_or(AudioError::InvalidFormat)?;
        validate_rate(rate)?;
        let max_input_samples = rate as usize * MAX_AUDIO_SECONDS;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        let mut mono = Vec::new();
        loop {
            check_cancelled(cancelled)?;
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(DecodeError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            };
            if packet.track_id() != track_id {
                continue;
            }
            // A corrupt packet is surfaced; silently skipping it would change the audio timeline.
            let decoded = decoder.decode(&packet)?;
            let spec = *decoded.spec();
            let channels = spec.channels.count();
            if spec.rate != rate || channels == 0 || channels > 32 {
                return Err(AudioError::InvalidFormat);
            }
            let mut interleaved = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            interleaved.copy_interleaved_ref(decoded);
            if mono.len().saturating_add(interleaved.len() / channels) > max_input_samples {
                return Err(AudioError::TooLong);
            }
            for frame in interleaved.samples().chunks_exact(channels) {
                let value = frame.iter().map(|&v| v as f64).sum::<f64>() / channels as f64;
                if !value.is_finite() {
                    return Err(AudioError::Nonfinite);
                }
                mono.push(value.clamp(-1.0, 1.0) as f32);
            }
        }
        if mono.is_empty() {
            return Err(AudioError::Empty);
        }
        Ok(Self {
            samples: resample(&mono, rate, ANALYSIS_SAMPLE_RATE, cancelled)?,
            sample_rate: ANALYSIS_SAMPLE_RATE,
            content_hash,
        })
    }

    pub fn from_mono(mut samples: Vec<f32>, sample_rate: u32) -> Result<Self, AudioError> {
        validate_rate(sample_rate)?;
        if samples.is_empty() {
            return Err(AudioError::Empty);
        }
        if samples.len() > sample_rate as usize * MAX_AUDIO_SECONDS {
            return Err(AudioError::TooLong);
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AudioError::Nonfinite);
        }
        for sample in &mut samples {
            *sample = sample.clamp(-1.0, 1.0);
        }
        let mut hash = Sha256::new();
        hash.update(sample_rate.to_le_bytes());
        for sample in &samples {
            hash.update(sample.to_le_bytes());
        }
        Ok(Self {
            samples: resample(
                &samples,
                sample_rate,
                ANALYSIS_SAMPLE_RATE,
                &AtomicBool::new(false),
            )?,
            sample_rate: ANALYSIS_SAMPLE_RATE,
            content_hash: format!("{:x}", hash.finalize()),
        })
    }

    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), AudioError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(AudioError::Cancelled);
    }
    Ok(())
}

fn validate_rate(rate: u32) -> Result<(), AudioError> {
    if !(8_000..=384_000).contains(&rate) {
        return Err(AudioError::InvalidFormat);
    }
    Ok(())
}

/// Windowed-sinc sample-rate conversion with a low-pass cutoff at the lower Nyquist.
/// 32 taps bound work per output sample and suppress aliasing when downsampling.
fn resample(
    input: &[f32],
    source_rate: u32,
    target_rate: u32,
    cancelled: &AtomicBool,
) -> Result<Vec<f32>, AudioError> {
    check_cancelled(cancelled)?;
    if source_rate == target_rate {
        return Ok(input.to_vec());
    }
    let output_len = ((input.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    if output_len == 0 {
        return Err(AudioError::Empty);
    }
    if output_len > MAX_SAMPLES {
        return Err(AudioError::TooLong);
    }
    let ratio = source_rate as f64 / target_rate as f64;
    let cutoff = (target_rate as f64 / source_rate as f64).min(1.0) * 0.94;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        if index.is_multiple_of(1024) {
            check_cancelled(cancelled)?;
        }
        let position = index as f64 * ratio;
        let center = position.floor() as isize;
        let mut sum = 0.0;
        let mut weight_sum = 0.0;
        for offset in -15..=16 {
            let at = center + offset;
            let distance = position - at as f64;
            let phase = std::f64::consts::PI * distance * cutoff;
            let sinc = if phase.abs() < 1e-12 {
                1.0
            } else {
                phase.sin() / phase
            };
            let window = 0.5 + 0.5 * (std::f64::consts::PI * distance / 16.0).cos();
            let weight = sinc * window * cutoff;
            let sample = input[at.clamp(0, input.len() as isize - 1) as usize] as f64;
            sum += sample * weight;
            weight_sum += weight;
        }
        output.push((sum / weight_sum).clamp(-1.0, 1.0) as f32);
    }
    Ok(output)
}
