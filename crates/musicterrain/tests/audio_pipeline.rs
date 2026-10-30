use std::{
    f32::consts::TAU,
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

use musicterrain::{
    audio::{AudioError, DecodedAudio},
    dsp::{AnalysisConfig, Analyzer, AudioTimeline},
};

fn sine(hz: f32, rate: u32, seconds: f32) -> Vec<f32> {
    (0..(rate as f32 * seconds) as usize)
        .map(|n| 0.8 * (TAU * hz * n as f32 / rate as f32).sin())
        .collect()
}

#[test]
fn synthetic_tones_land_in_hz_bands_and_have_calibrated_rms() {
    let config = AnalysisConfig::default();
    for (frequency, band) in [
        (40.0, 0),
        (60.0, 0),
        (120.0, 1),
        (440.0, 2),
        (1000.0, 3),
        (4000.0, 4),
    ] {
        let audio = DecodedAudio::from_mono(sine(frequency, 48000, 0.3), 48000).unwrap();
        let timeline = AudioTimeline::analyze(&audio, &config).unwrap();
        let feature = timeline.at(0.15);
        let dominant = feature
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(dominant, band, "{frequency}Hz: {:?}", feature.bands);
        assert!(
            (feature.rms - 0.8 / 2_f32.sqrt()).abs() < 0.01,
            "{frequency}Hz: {}",
            feature.rms
        );
        assert!(
            (feature.centroid - frequency).abs() < 12.0,
            "{frequency}Hz: {}",
            feature.centroid
        );
    }
}

#[test]
fn silence_is_exactly_stable_even_after_a_transient() {
    let mut samples = vec![0.0; 48000];
    samples[12000] = 1.0;
    let audio = DecodedAudio::from_mono(samples, 48000).unwrap();
    let timeline = AudioTimeline::analyze(&audio, &AnalysisConfig::default()).unwrap();
    assert!(timeline.frames.iter().any(|v| v.onset));
    for frame in timeline.frames.iter().filter(|v| v.time > 0.5) {
        assert!(frame.silence);
        assert_eq!(frame.rms, 0.0);
        assert_eq!(frame.bands, [0.0; 7]);
        assert_eq!(frame.centroid, 0.0);
        assert_eq!(frame.flux, 0.0);
        assert_eq!(frame.beat_strength, 0.0);
        assert_eq!(frame.beat_phase, None);
        assert!(!frame.onset);
    }
}

#[test]
fn white_noise_sweep_and_regular_pulses_produce_finite_meaningful_features() {
    let mut random = 7_u32;
    let noise: Vec<_> = (0..24000)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            (random as f64 / u32::MAX as f64 * 0.4 - 0.2) as f32
        })
        .collect();
    let audio = DecodedAudio::from_mono(noise, 48000).unwrap();
    let timeline = AudioTimeline::analyze(&audio, &AnalysisConfig::default()).unwrap();
    let feature = timeline.at(0.2);
    assert!(feature.bands.iter().all(|v| v.is_finite() && *v > 0.0));
    assert!((9000.0..15000.0).contains(&feature.centroid));
    let mut phase = 0.0;
    let sweep: Vec<_> = (0..48000)
        .map(|i| {
            phase += TAU * (100.0 + 7900.0 * i as f32 / 48000.0) / 48000.0;
            phase.sin() * 0.5
        })
        .collect();
    let audio = DecodedAudio::from_mono(sweep, 48000).unwrap();
    let timeline = AudioTimeline::analyze(&audio, &AnalysisConfig::default()).unwrap();
    assert!(timeline.at(0.8).centroid > timeline.at(0.2).centroid + 3000.0);
    let mut pulse = vec![0.0; 4 * 48000];
    for at in (12000..pulse.len()).step_by(24000) {
        for j in 0..800 {
            pulse[at + j] = (TAU * 1000.0 * j as f32 / 48000.0).sin() * (1.0 - j as f32 / 800.0);
        }
    }
    let audio = DecodedAudio::from_mono(pulse, 48000).unwrap();
    let timeline = AudioTimeline::analyze(&audio, &AnalysisConfig::default()).unwrap();
    let onsets: Vec<_> = timeline.frames.iter().filter(|f| f.onset).collect();
    assert_eq!(onsets.len(), 8);
    assert!(timeline.frames.iter().any(|f| f.beat_phase.is_some()));
    for feature in &timeline.frames {
        assert!(feature.bands.iter().all(|v| v.is_finite()));
        assert!(
            feature.rms.is_finite() && feature.centroid.is_finite() && feature.flux.is_finite()
        );
    }
}

#[test]
fn seeking_and_analysis_are_independent_of_lookup_order() {
    let audio = DecodedAudio::from_mono(sine(440.0, 48000, 1.0), 48000).unwrap();
    let config = AnalysisConfig::default();
    let first = AudioTimeline::analyze(&audio, &config).unwrap();
    let second = AudioTimeline::analyze(&audio, &config).unwrap();
    assert_eq!(first.frames, second.frames);
    for frame in &first.frames {
        assert_eq!(*frame, first.at(frame.time));
    }
    let at = first.at(0.43);
    assert_eq!(at, first.at(0.43));
    let _ = first.at(0.9);
    let _ = first.at(0.1);
    assert_eq!(at, first.at(0.43));
    assert_eq!(first.at(-1.0), first.frames[0]);
    assert_eq!(first.at(f64::NAN), first.frames[0]);
    assert_eq!(first.at(f64::INFINITY), *first.frames.last().unwrap());
}

#[test]
fn resampling_preserves_duration_and_tone_and_suppresses_aliases() {
    let audio = DecodedAudio::from_mono(sine(1000.0, 44100, 0.2), 44100).unwrap();
    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.samples.len(), 9600);
    let features = AudioTimeline::analyze(&audio, &AnalysisConfig::default())
        .unwrap()
        .at(0.1);
    assert!((features.centroid - 1000.0).abs() < 2.0);
    assert!((features.rms - 0.8 / 2_f32.sqrt()).abs() < 0.02);
    let ultrasound = DecodedAudio::from_mono(sine(30000.0, 96000, 0.2), 96000).unwrap();
    let features = AudioTimeline::analyze(&ultrasound, &AnalysisConfig::default())
        .unwrap()
        .at(0.1);
    assert!(features.rms < 0.02, "aliased RMS = {}", features.rms);
}

#[test]
fn validation_rejects_nonfinite_pcm_and_invalid_fft_or_bands() {
    assert!(matches!(
        DecodedAudio::from_mono(vec![f32::NAN], 48000),
        Err(AudioError::Nonfinite)
    ));
    assert!(DecodedAudio::from_mono(vec![1.0], 0).is_err());
    let mut config = AnalysisConfig {
        fft_size: 4000,
        ..Default::default()
    };
    assert!(config.validate().is_err());
    config.fft_size = 4096;
    config.bands[0].high_hz = f32::INFINITY;
    assert!(config.validate().is_err());
    let mut analyzer = Analyzer::new(48000, AnalysisConfig::default()).unwrap();
    assert!(analyzer.analyze_window(&[0.0; 32], 0.0).is_err());
    let mut invalid = vec![0.0; 4096];
    invalid[3] = f32::INFINITY;
    assert!(analyzer.analyze_window(&invalid, 0.0).is_err());
    let mut low_rate = Analyzer::new(8000, AnalysisConfig::default()).unwrap();
    let features = low_rate
        .analyze_window(&sine(1000.0, 8000, 0.512), 0.0)
        .unwrap();
    assert_eq!(&features.bands[5..], &[0.0, 0.0]);
}

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
fn wav_fixture(channels: u16, samples: &[i16]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "musicterrain-audio-{}-{}.wav",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let rate = 44100_u32;
    let bytes = samples.len() as u32 * 2;
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(36 + bytes).to_le_bytes());
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&16_u32.to_le_bytes());
    file.extend_from_slice(&1_u16.to_le_bytes());
    file.extend_from_slice(&channels.to_le_bytes());
    file.extend_from_slice(&rate.to_le_bytes());
    file.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    file.extend_from_slice(&(channels * 2).to_le_bytes());
    file.extend_from_slice(&16_u16.to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&bytes.to_le_bytes());
    for &sample in samples {
        file.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(&path, file).unwrap();
    path
}

#[test]
fn actual_wav_decoder_downmixes_and_hashes_encoded_audio() {
    let stereo: Vec<_> = sine(440.0, 44100, 0.2)
        .into_iter()
        .flat_map(|v| {
            [
                (v * i16::MAX as f32) as i16,
                (v * i16::MAX as f32 * 0.5) as i16,
            ]
        })
        .collect();
    let file = wav_fixture(2, &stereo);
    let decoded = DecodedAudio::decode(&file).unwrap();
    let repeated = DecodedAudio::decode(&file).unwrap();
    assert_eq!(decoded.content_hash, repeated.content_hash);
    assert_eq!(decoded.samples, repeated.samples);
    assert_eq!(decoded.content_hash.len(), 64);
    assert_eq!(decoded.samples.len(), 9600);
    let features = AudioTimeline::analyze(&decoded, &AnalysisConfig::default())
        .unwrap()
        .at(0.1);
    assert!((features.rms - 0.6 / 2_f32.sqrt()).abs() < 0.01);
    assert!((features.centroid - 440.0).abs() < 2.0);
    fs::write(&file, b"not an audio file").unwrap();
    assert!(DecodedAudio::decode(&file).is_err());
    fs::remove_file(file).unwrap();
}

#[test]
fn low_frequency_precision_mode_works() {
    let audio = DecodedAudio::from_mono(sine(40.0, 48000, 0.4), 48000).unwrap();
    let config = AnalysisConfig {
        fft_size: 8192,
        ..Default::default()
    };
    let features = AudioTimeline::analyze(&audio, &config).unwrap().at(0.2);
    assert!(features.bands[0] > 0.55);
    assert!((features.centroid - 40.0).abs() < 2.0);
}

#[test]
fn cancelled_import_and_analysis_return_errors_without_partial_results() {
    use musicterrain::dsp::DspError;
    use std::sync::atomic::AtomicBool;
    let cancelled = AtomicBool::new(true);
    let file = wav_fixture(1, &[0; 128]);
    assert!(matches!(
        DecodedAudio::decode_cancellable(&file, &cancelled),
        Err(AudioError::Cancelled)
    ));
    let audio = DecodedAudio::from_mono(vec![0.0; 4096], 48000).unwrap();
    assert!(matches!(
        AudioTimeline::analyze_cancellable(&audio, &AnalysisConfig::default(), &cancelled),
        Err(DspError::Cancelled)
    ));
    fs::remove_file(file).unwrap();
}
