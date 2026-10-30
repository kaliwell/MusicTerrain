# Audio engine

Implemented in `crates/musicterrain/src/audio.rs` and `dsp.rs`. File analysis is
independent of device callbacks, rendering and world generation.

## Files and PCM

`DecodedAudio::decode` uses Symphonia 0.5 with its codec/format features enabled,
including WAV/PCM, MP3 and FLAC. It reads the original file for its SHA-256 content
identity, decodes the default audio track, averages interleaved channels in f64,
rejects nonfinite samples and changing/invalid sample rates, and prepares mono
48,000 Hz f32 PCM. Gapless demuxing is enabled to trim codec encoder delay and
padding when the container supplies that information. Channel counts are limited
to 32 and sample rates to
8,000–384,000 Hz. Corrupt packets produce errors rather than silently shifting
the timeline. Empty audio also produces an error. End-of-stream is represented
by Symphonia's `UnexpectedEof`; a truncated final packet that the demuxer treats
as normal end-of-stream cannot always be distinguished from a complete file.

Resampling uses a deterministic 32-tap windowed-sinc low-pass converter with a
cutoff below the lower sample-rate Nyquist. This is a bounded offline converter,
not a claimed studio-quality or bit-identical-to-Minecraft resampler. Same-rate
input avoids filtering. PCM is limited to 20 minutes (about 220 MiB for the final
mono buffer); transient decode/resample memory can exceed that buffer size.
`DecodedAudio::from_mono` supplies the same preparation path for tests or generated
audio and hashes the source sample rate plus finite, amplitude-clamped PCM.

## FFT and features

`AnalysisConfig` defaults to FFT size 4096, hop 512 and a periodic Hann window.
FFT size 8192 is supported for low-frequency precision. The FFT plan, transform
buffer, scratch space, Hann coefficients and previous magnitudes are allocated
once per `Analyzer`. FFT sizes must be powers of two in 256–32768, and hops must
be 64–FFT size. Timeline allocation is capped at one million feature frames.

`AudioTimeline::analyze` computes windows centred on fixed sample positions and
zero-pads the boundaries. A frame at time `t` therefore represents a window
centred on `t`, with a half-window of offline lookahead. `AudioTimeline::at(t)`
selects the previous fixed-hop frame, clamps out-of-range positions and does not
advance mutable DSP state. Repeated playback seeks produce the same features.

Each FFT bin uses `frequency = bin * sample_rate / fft_size`, clipped at the
actual Nyquist. One-sided power doubles bins except DC and Nyquist and divides
by `fft_size * sum(Hann²)`. Band values are square roots of accumulated power:
they are calibrated RMS amplitude, so a full-scale sine in one band is about
0.707, regardless of the FFT size. No audio frequency is directly used as a
terrain noise frequency. Bands have configurable finite lower/upper Hz bounds;
overlapping bands are allowed deliberately. The default bands are 20–80,
80–250, 250–500, 500–2000, 2000–6000, 6000–12000 and 12000–20000 Hz.

Features include full-spectrum weighted RMS, seven band RMS values, a
power-weighted centroid in Hz, positive spectral flux, onset, beat strength,
optional beat phase, silence and sample-derived time. Onsets use an adaptive
flux baseline, minimum threshold and 150 ms refractory period. Beat phase is
only exposed after three intervals from four onsets agree within 15%; this is
a basic onset-based estimator and does not claim general-purpose beat tracking.
The default silence gate is RMS ≤ 1e-5. Silence emits exact zero energies,
centroid and flux, suppresses onset and removes beat phase; no normalization
division can amplify silence. Mapping attack/release and response curves belong
to `mapping.rs`, keeping the spectral extraction independent of terrain settings.

`Analyzer::analyze_window` provides reused-plan streaming analysis of an exact
FFT window with a caller-supplied timestamp. It is a worker API: device callbacks
must only copy PCM into bounded storage and must not invoke the FFT.

`DecodedAudio::decode_cancellable` and `AudioTimeline::analyze_cancellable` take
a shared `AtomicBool` cancellation flag. Hashing checks at each 64 KiB read,
decoding at each packet, sample-rate conversion every 1024 output samples, and
analysis before each FFT. Cancellation returns a typed error and no partial
timeline, allowing the preview loader to stop its worker when the window closes.

## Verification

`tests/audio_pipeline.rs` exercises real WAV decoding and stereo downmixing,
encoded-content hashing, malformed input, 44.1 kHz resampling, ultrasound alias
suppression, 40/60/120/440/1000/4000 Hz band identity and calibrated RMS, silence,
white noise, a frequency sweep, transient/regular onset events, optional beat
phase, 8192 FFT mode, invalid input, Nyquist clipping and seek independence.

Run the tests without device or GPU requirements:

```sh
cargo test -p musicterrain --no-default-features --test audio_pipeline
```

Executed on Linux with Rust 1.99.1: 9 passed, 0 failed (2026-10-29). The first
run exposed an onset threshold that missed a single-sample impulse; reducing the
absolute flux threshold while retaining the adaptive baseline and refractory
period fixed it, and the complete suite then passed. The ninth test verifies
cooperative import/analysis cancellation. `rustfmt --check` also
passed for the decoder, DSP and audio integration test files.

An additional development smoke check used ffmpeg to encode a synthetic 120 Hz,
44.1 kHz, one-second tone as MP3 and FLAC, then called this Rust decoder and FFT.
Both decoded to exactly 1.0000 s at 48 kHz, with centroid 120.000 Hz and B2 as the
dominant band. Enabling gapless demuxing removed the MP3 encoder padding observed
in the first check. Generated audio and the temporary Rust probe reside outside
the repository in `/workspace/scratch/musicterrain-codecs`.

Device playback and clock behavior are owned by `transport.rs`; capture-device
and Windows WASAPI loopback acceptance require actual hardware/platform testing.
See `TEST_REPORT.md` for executed checks and current limitations.
