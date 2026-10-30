//! Playback clock driven by output frames; the device callback only copies PCM.
use crate::audio::DecodedAudio;
#[cfg(feature = "audio-output")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

// The upper bits are a command epoch: even seek-to-the-same-position invalidates
// a callback already copying samples. The low 40 bits hold output frame position.
const CURSOR_MASK: u64 = (1u64 << 40) - 1;
const EPOCH_STEP: u64 = 1u64 << 40;
struct State {
    cursor: AtomicU64,
    playing: AtomicBool,
    failed: AtomicBool,
}
impl State {
    fn command(&self, position: Option<u64>) {
        let mut old = self.cursor.load(Ordering::Acquire);
        loop {
            let next = (old & !CURSOR_MASK).wrapping_add(EPOCH_STEP)
                | position.unwrap_or(old & CURSOR_MASK).min(CURSOR_MASK);
            match self
                .cursor
                .compare_exchange_weak(old, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(actual) => old = actual,
            }
        }
    }
}
pub struct Transport {
    audio: Arc<DecodedAudio>,
    state: Arc<State>,
    rate: u32,
    silent_started: Option<Mutex<Option<Instant>>>,
    #[cfg(feature = "audio-output")]
    _stream: Option<cpal::Stream>,
}
impl Transport {
    pub fn new(audio: Arc<DecodedAudio>) -> Result<Self, Box<dyn Error>> {
        if std::env::var_os("HORIZON_NO_AUDIO").is_some() {
            return Err("Audio disabled by HORIZON_NO_AUDIO; use --silent-preview for a labeled software clock".into());
        }
        #[cfg(feature = "audio-output")]
        {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("No output device available")?;
            let supported = device.default_output_config()?;
            let rate = supported.sample_rate().0;
            let channels = usize::from(supported.channels());
            let config = supported.config();
            let state = Arc::new(State {
                cursor: AtomicU64::new(0),
                playing: AtomicBool::new(false),
                failed: AtomicBool::new(false),
            });
            let build = |format| -> Result<cpal::Stream, Box<dyn Error>> {
                let state = state.clone();
                let errors = state.clone();
                let samples = audio.clone();
                let callback_state = state.clone();
                let callback =
                    move |data: &mut [f32]| fill(data, channels, rate, &samples, &callback_state);
                let error = move |_| {
                    errors.failed.store(true, Ordering::Relaxed);
                    errors.playing.store(false, Ordering::Release);
                };
                Ok(match format {
                    cpal::SampleFormat::F32 => device.build_output_stream(
                        &config,
                        move |data: &mut [f32], _| callback(data),
                        error,
                        None,
                    )?,
                    cpal::SampleFormat::I16 => {
                        let state = state.clone();
                        let samples = audio.clone();
                        device.build_output_stream(
                            &config,
                            move |data: &mut [i16], _| {
                                fill_typed(data, channels, rate, &samples, &state, |x| {
                                    (x.clamp(-1., 1.) * i16::MAX as f32) as i16
                                })
                            },
                            error,
                            None,
                        )?
                    }
                    cpal::SampleFormat::U16 => {
                        let state = state.clone();
                        let samples = audio.clone();
                        device.build_output_stream(
                            &config,
                            move |data: &mut [u16], _| {
                                fill_typed(data, channels, rate, &samples, &state, |x| {
                                    ((x.clamp(-1., 1.) * 0.5 + 0.5) * u16::MAX as f32).round()
                                        as u16
                                })
                            },
                            error,
                            None,
                        )?
                    }
                    _ => return Err("Unsupported output format (requires f32/i16/u16)".into()),
                })
            };
            let stream = build(supported.sample_format())?;
            stream.play()?;
            Ok(Self {
                audio,
                state,
                rate,
                silent_started: None,
                _stream: Some(stream),
            })
        }
        #[cfg(not(feature = "audio-output"))]
        {
            let _ = audio;
            Err("Built without audio-output; use --silent-preview explicitly".into())
        }
    }
    /// Explicit diagnostic clock. This does not claim to play audible samples.
    pub fn silent(audio: Arc<DecodedAudio>) -> Self {
        Self {
            rate: audio.sample_rate,
            audio,
            state: Arc::new(State {
                cursor: AtomicU64::new(0),
                playing: AtomicBool::new(false),
                failed: AtomicBool::new(false),
            }),
            silent_started: Some(Mutex::new(None)),
            #[cfg(feature = "audio-output")]
            _stream: None,
        }
    }
    pub fn play(&self) {
        if self.failed() {
            return;
        }
        if self.position() >= self.duration() {
            self.seek(0.);
        }
        if let Some(started) = &self.silent_started {
            let mut s = started.lock().unwrap_or_else(|e| e.into_inner());
            if s.is_none() {
                *s = Some(Instant::now());
            }
        }
        self.state.command(None);
        self.state.playing.store(true, Ordering::Release);
    }
    pub fn pause(&self) {
        if let Some(started) = &self.silent_started {
            let mut s = started.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = s.take() {
                self.state.cursor.fetch_add(
                    (t.elapsed().as_secs_f64() * self.rate as f64) as u64,
                    Ordering::Relaxed,
                );
            }
        }
        self.state.playing.store(false, Ordering::Release);
        self.state.command(None);
    }
    pub fn seek(&self, time: f64) {
        let time = if time.is_finite() {
            time.clamp(0., self.duration())
        } else {
            0.
        };
        if let Some(started) = &self.silent_started {
            let mut s = started.lock().unwrap_or_else(|e| e.into_inner());
            if s.is_some() {
                *s = Some(Instant::now());
            }
        }
        self.state.command(Some((time * self.rate as f64) as u64));
    }
    pub fn position(&self) -> f64 {
        let mut time =
            (self.state.cursor.load(Ordering::Acquire) & CURSOR_MASK) as f64 / self.rate as f64;
        if let Some(started) = &self.silent_started
            && let Some(t) = *started.lock().unwrap_or_else(|e| e.into_inner())
        {
            time += t.elapsed().as_secs_f64();
        }
        time.min(self.duration())
    }
    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Acquire) && self.position() < self.duration()
    }
    pub fn duration(&self) -> f64 {
        self.audio.duration()
    }
    pub fn is_silent(&self) -> bool {
        self.silent_started.is_some()
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
#[cfg(feature = "audio-output")]
fn fill(data: &mut [f32], channels: usize, rate: u32, audio: &DecodedAudio, state: &State) {
    fill_typed(data, channels, rate, audio, state, |x| x);
}
#[cfg(feature = "audio-output")]
fn fill_typed<T: Copy>(
    data: &mut [T],
    channels: usize,
    rate: u32,
    audio: &DecodedAudio,
    state: &State,
    convert: impl Fn(f32) -> T,
) {
    let begin = state.cursor.load(Ordering::Acquire);
    let playing = state.playing.load(Ordering::Acquire);
    for (i, frame) in data.chunks_mut(channels).enumerate() {
        let at = ((begin & CURSOR_MASK) + i as u64) as f64 * audio.sample_rate as f64 / rate as f64;
        let index = at as usize;
        let value = if playing {
            let a = audio.samples.get(index).copied().unwrap_or(0.);
            let b = audio.samples.get(index + 1).copied().unwrap_or(a);
            a + (b - a) * (at.fract() as f32)
        } else {
            0.
        };
        frame.fill(convert(value));
    }
    if playing {
        let _ = state.cursor.compare_exchange(
            begin,
            begin + (data.len() / channels) as u64,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "audio-output")]
    #[test]
    fn commands_invalidate_a_callback_in_progress() {
        let audio = DecodedAudio::from_mono(vec![0.25; 48000], 48000).unwrap();
        for pause in [false, true] {
            let state = State {
                cursor: AtomicU64::new(0),
                playing: AtomicBool::new(true),
                failed: AtomicBool::new(false),
            };
            let command_sent = std::cell::Cell::new(false);
            let mut output = [0.; 1024];
            fill_typed(&mut output, 2, 48000, &audio, &state, |sample| {
                if !command_sent.replace(true) {
                    if pause {
                        state.playing.store(false, Ordering::Release);
                    }
                    state.command(Some(0));
                }
                sample
            });
            assert_eq!(
                state.cursor.load(Ordering::Acquire) & CURSOR_MASK,
                0,
                "stale callback must not undo same-position seek or pause"
            );
        }
    }
    #[cfg(feature = "audio-output")]
    #[test]
    fn callback_resamples_and_fills_all_output_channels() {
        let audio = DecodedAudio::from_mono(vec![0.25; 48000], 48000).unwrap();
        let state = State {
            cursor: AtomicU64::new(0),
            playing: AtomicBool::new(true),
            failed: AtomicBool::new(false),
        };
        let mut output = [0.; 960];
        fill(&mut output, 2, 24000, &audio, &state);
        assert!(output.iter().all(|&s| s == 0.25));
        assert_eq!(state.cursor.load(Ordering::Acquire), 480);
        state.playing.store(false, Ordering::Release);
        fill(&mut output, 2, 24000, &audio, &state);
        assert!(output.iter().all(|&s| s == 0.));
        assert_eq!(state.cursor.load(Ordering::Acquire), 480);
    }
    #[test]
    fn seek_pause_and_end_are_stable() {
        let a = Arc::new(DecodedAudio::from_mono(vec![0.; 48000], 48000).unwrap());
        let t = Transport::silent(a);
        t.seek(0.75);
        assert_eq!(t.position(), 0.75);
        t.play();
        t.pause();
        let at = t.position();
        assert_eq!(t.position(), at);
        t.seek(f64::NAN);
        assert_eq!(t.position(), 0.);
        t.seek(3.);
        assert_eq!(t.position(), 1.);
        assert!(!t.is_playing());
    }
}
