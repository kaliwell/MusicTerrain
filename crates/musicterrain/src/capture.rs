//! WASAPI loopback: bounded PCM delivery, FFT on a worker, no audio retransmission.
use crate::dsp::AudioFeatures;
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub struct SystemCapture {
    latest: Arc<Mutex<(AudioFeatures, Instant)>>,
    failed: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    #[cfg(feature = "audio-output")]
    stream: Option<cpal::Stream>,
}
impl SystemCapture {
    pub fn start() -> Result<Self, Box<dyn Error>> {
        #[cfg(all(windows, feature = "audio-output"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("No default output device")?;
            let supported = device.default_output_config()?;
            let rate = supported.sample_rate().0;
            let channels = usize::from(supported.channels());
            let config = supported.config();
            let (tx, rx) = std::sync::mpsc::sync_channel(8);
            let latest = Arc::new(Mutex::new((
                AudioFeatures {
                    silence: true,
                    ..Default::default()
                },
                Instant::now(),
            )));
            let failed = Arc::new(AtomicBool::new(false));
            let errors = failed.clone();
            let error = move |_| {
                errors.store(true, Ordering::Release);
            };
            let mut cursor = 0_u64;
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &config,
                    move |data: &[f32], _| enqueue(data, channels, &tx, &mut cursor, |x| x),
                    error,
                    None,
                )?,
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        enqueue(data, channels, &tx, &mut cursor, |x| f32::from(x) / 32768.0)
                    },
                    error,
                    None,
                )?,
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        enqueue(data, channels, &tx, &mut cursor, |x| {
                            f32::from(x) / 32768.0 - 1.0
                        })
                    },
                    error,
                    None,
                )?,
                _ => return Err("Unsupported system capture format".into()),
            };
            let output = latest.clone();
            let worker = std::thread::Builder::new()
                .name("musicterrain-loopback-fft".into())
                .spawn(move || analyze(rx, rate, output))?;
            let capture = Self {
                latest,
                failed,
                worker: Some(worker),
                stream: Some(stream),
            };
            capture.stream.as_ref().unwrap().play()?;
            tracing::info!(
                sample_rate = rate,
                channels,
                "system loopback capture started"
            );
            Ok(capture)
        }
        #[cfg(not(all(windows, feature = "audio-output")))]
        {
            Err("System capture requires Windows and audio-output support".into())
        }
    }
    pub fn features(&self) -> AudioFeatures {
        let latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        if latest.1.elapsed() > Duration::from_millis(250) || self.failed() {
            AudioFeatures {
                time: latest.0.time,
                silence: true,
                ..Default::default()
            }
        } else {
            latest.0
        }
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
impl Drop for SystemCapture {
    fn drop(&mut self) {
        #[cfg(feature = "audio-output")]
        self.stream.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(any(test, all(windows, feature = "audio-output")))]
struct Block {
    samples: [f32; 512],
    len: usize,
    start: u64,
}
#[cfg(any(test, all(windows, feature = "audio-output")))]
fn enqueue<T: Copy>(
    data: &[T],
    channels: usize,
    tx: &std::sync::mpsc::SyncSender<Block>,
    cursor: &mut u64,
    convert: impl Fn(T) -> f32,
) {
    for chunk in data.chunks(512 * channels) {
        let mut block = Block {
            samples: [0.0; 512],
            len: chunk.len() / channels,
            start: *cursor,
        };
        for (i, frame) in chunk.chunks_exact(channels).enumerate() {
            block.samples[i] = (frame
                .iter()
                .map(|v| {
                    let s = convert(*v);
                    if s.is_finite() {
                        s.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
                / channels as f32)
                .clamp(-1.0, 1.0);
        }
        *cursor += block.len as u64;
        let _ = tx.try_send(block);
    }
}
#[cfg(any(test, all(windows, feature = "audio-output")))]
fn analyze(
    rx: std::sync::mpsc::Receiver<Block>,
    rate: u32,
    latest: Arc<Mutex<(AudioFeatures, Instant)>>,
) {
    use crate::dsp::{AnalysisConfig, Analyzer};
    let Ok(mut analyzer) = Analyzer::new(rate, AnalysisConfig::default()) else {
        return;
    };
    let mut samples = std::collections::VecDeque::with_capacity(8192);
    let mut expected = 0;
    while let Ok(block) = rx.recv() {
        if block.start != expected {
            samples.clear();
            analyzer = Analyzer::new(rate, AnalysisConfig::default())
                .expect("validated capture sample rate");
        }
        expected = block.start + block.len as u64;
        samples.extend(&block.samples[..block.len]);
        while samples.len() >= 4096 {
            let window = samples.make_contiguous();
            if let Ok(features) = analyzer.analyze_window(
                &window[..4096],
                (expected - (window.len() - 4096) as u64) as f64 / f64::from(rate),
            ) {
                *latest.lock().unwrap_or_else(|e| e.into_inner()) = (features, Instant::now());
            }
            samples.drain(..512);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_disconnected_capture_returns_silence() {
        let latest = Arc::new(Mutex::new((
            AudioFeatures {
                rms: 0.7,
                ..Default::default()
            },
            Instant::now() - Duration::from_secs(1),
        )));
        let capture = SystemCapture {
            latest: latest.clone(),
            failed: Arc::new(AtomicBool::new(false)),
            worker: None,
            #[cfg(feature = "audio-output")]
            stream: None,
        };
        assert!(capture.features().silence);
        latest.lock().unwrap().1 = Instant::now();
        assert_eq!(capture.features().rms, 0.7);
        capture.failed.store(true, Ordering::Release);
        assert_eq!(capture.features().rms, 0.0);
    }
    #[test]
    fn stereo_capture_drives_real_fft_and_queue_is_bounded() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let mut cursor = 0;
        let pcm: Vec<_> = (0..4096)
            .flat_map(|i| {
                let s = (i as f32 * std::f32::consts::TAU * 1000.0 / 48000.0).sin() * 0.4;
                [s, s]
            })
            .collect();
        enqueue(&pcm, 2, &tx, &mut cursor, |x| x);
        drop(tx);
        let latest = Arc::new(Mutex::new((AudioFeatures::default(), Instant::now())));
        analyze(rx, 48000, latest.clone());
        let f = latest.lock().unwrap().0;
        assert!(f.rms > 0.25 && f.bands[3] > f.bands[0] * 10.0);
        assert!((f.time - 4096.0 / 48000.0).abs() < 1e-6);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        enqueue(&pcm, 2, &tx, &mut cursor, |x| x);
        assert_eq!(rx.try_iter().count(), 1);
    }
}
