//! One native winit loop, using Horizon's section renderer and HUD.
use crate::{
    audio::DecodedAudio,
    dsp::{AnalysisConfig, AudioTimeline},
    mapping::{AudioSource, CombineMode, MappingState, ResponseCurve},
    preset::{VisualizationPreset, builtins},
    renderer::PreviewRenderer,
    terrain::{GenerationRevision, TerrainParameters, TerrainSnapshot},
    transport::Transport,
    ui::{Action, Diagnostics, Page, RuleField, Ui, View},
    visual::{MeshWorker, SceneOptions, SceneStats},
};
use glam::DVec3;
use std::{
    collections::HashSet,
    error::Error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub struct PreviewOptions {
    pub language: Option<crate::localization::Language>,
    pub resources: Option<PathBuf>,
    pub scene_options: SceneOptions,
    pub audio_path: Option<PathBuf>,
    pub preset_path: PathBuf,
    pub offscreen: bool,
    pub smoke_frames: Option<u64>,
    pub screenshot: Option<PathBuf>,
    pub silent_preview: bool,
    pub size: u32,
    pub system_audio: bool,
    pub orbit: bool,
    pub orthographic: bool,
    pub noise: Option<crate::terrain::NoiseType>,
    pub cave_w: Option<f32>,
}
impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            language: None,
            resources: None,
            scene_options: SceneOptions::default(),
            audio_path: None,
            preset_path: PathBuf::from("musicterrain-preset.json"),
            offscreen: false,
            smoke_frames: None,
            screenshot: None,
            silent_preview: false,
            size: 64,
            system_audio: false,
            orbit: false,
            orthographic: false,
            noise: None,
            cave_w: None,
        }
    }
}
struct Loaded {
    audio: Arc<DecodedAudio>,
    timeline: AudioTimeline,
    name: String,
}
struct Loader {
    cancelled: Arc<AtomicBool>,
    send: Option<SyncSender<Option<PathBuf>>>,
    receive: Option<Receiver<Result<Loaded, String>>>,
    thread: Option<JoinHandle<()>>,
}
impl Loader {
    fn new() -> Result<Self, std::io::Error> {
        let (send, jobs) = mpsc::sync_channel::<Option<PathBuf>>(1);
        let (results, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let thread = std::thread::Builder::new()
            .name("musicterrain-decode".into())
            .spawn(move || {
                while let Ok(path) = jobs.recv() {
                    let result = (|| -> Result<Loaded, String> {
                        let (audio, name) = if let Some(path) = path {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "AUDIO".into());
                            (
                                DecodedAudio::decode_cancellable(path, &worker_cancelled)
                                    .map_err(|e| e.to_string())?,
                                name,
                            )
                        } else {
                            let samples = (0..48_000 * 12)
                                .map(|i| {
                                    let t = i as f32 / 48_000.0;
                                    let beat = (-((t * 2.0).fract()) * 12.0).exp();
                                    let melody = if t % 4.0 < 2.0 { 440.0 } else { 1000.0 };
                                    beat * 0.48 * (std::f32::consts::TAU * 60.0 * t).sin()
                                        + 0.15 * (std::f32::consts::TAU * 120.0 * t).sin()
                                        + 0.12 * (std::f32::consts::TAU * melody * t).sin()
                                        + beat * 0.04 * (std::f32::consts::TAU * 4000.0 * t).sin()
                                })
                                .collect();
                            (
                                DecodedAudio::from_mono(samples, 48_000)
                                    .map_err(|e| e.to_string())?,
                                "SYNTHESIZED 12 SECOND DEMO".into(),
                            )
                        };
                        let timeline = AudioTimeline::analyze_cancellable(
                            &audio,
                            &AnalysisConfig::default(),
                            &worker_cancelled,
                        )
                        .map_err(|e| e.to_string())?;
                        Ok(Loaded {
                            audio: Arc::new(audio),
                            timeline,
                            name,
                        })
                    })();
                    if results.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            cancelled,
            send: Some(send),
            receive: Some(receive),
            thread: Some(thread),
        })
    }
    fn submit(&self, path: Option<PathBuf>) -> bool {
        self.send.as_ref().is_some_and(|s| s.try_send(path).is_ok())
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.send.take();
        self.receive.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
struct App {
    options: PreviewOptions,
    window: Option<Arc<Window>>,
    renderer: Option<PreviewRenderer>,
    ui: Ui,
    loader: Loader,
    worker: MeshWorker,
    loaded: Option<Loaded>,
    transport: Option<Transport>,
    presets: Vec<VisualizationPreset>,
    preset: VisualizationPreset,
    mapped: Vec<TerrainParameters>,
    page: Page,
    undo: Vec<VisualizationPreset>,
    redo: Vec<VisualizationPreset>,
    status: String,
    error: Option<String>,
    cursor: [f32; 2],
    right_mouse: bool,
    dragging: bool,
    keys: HashSet<KeyCode>,
    camera: DVec3,
    yaw: f32,
    pitch: f32,
    orbit: bool,
    orthographic: bool,
    orbit_radius: f64,
    orthographic_span: f32,
    spectrum: bool,
    offset: f32,
    revision: GenerationRevision,
    last_submit: Instant,
    last_audio: f64,
    last_parameters: Option<TerrainParameters>,
    dirty: bool,
    previous: Instant,
    started: Instant,
    fps: f32,
    mesh_hz: f32,
    generation_ms: f32,
    frames: u64,
    meshes: u64,
    mesh_hashes: Vec<u64>,
    uploaded_columns: HashSet<(i32, i32)>,
    last_mesh_audio: f64,
    upload_ms: f32,
    gpu_ms: Option<f32>,
    music_overlay: bool,
    report_since: Instant,
    report_frames: u64,
    report_meshes: u64,
    report_sum_ms: f64,
    report_max_ms: f64,
    report_intervals: Vec<f64>,
    capture: Option<crate::capture::SystemCapture>,
    live_mapping: MappingState,
    live_parameters: Option<TerrainParameters>,
    live_paused: bool,
    live_time: f64,
    scene: SceneStats,
}
impl App {
    fn new(options: PreviewOptions) -> Result<Self, Box<dyn Error>> {
        let presets = builtins();
        let mut preset = presets[0].clone();
        if let Some(noise) = options.noise {
            preset.parameters.noise_type = noise;
        }
        if let Some(w) = options.cave_w {
            preset.parameters.cave_w = w;
        }
        preset.parameters.validate()?;
        let loader = Loader::new()?;
        if !options.system_audio {
            loader.submit(options.audio_path.clone());
        }
        let now = Instant::now();
        let mut ui = Ui::default();
        if let Some(language) = options.language {
            ui.language = language;
        }
        let camera_scale = f64::from(options.size) / 64.0;
        let orbit = options.orbit;
        let orthographic = options.orthographic;
        let mut app = Self {
            options,
            window: None,
            renderer: None,
            ui,
            loader,
            worker: MeshWorker::new(),
            loaded: None,
            transport: None,
            presets,
            preset,
            mapped: Vec::new(),
            page: Page::Live,
            undo: Vec::new(),
            redo: Vec::new(),
            status: "ANALYZING AUDIO...".into(),
            error: None,
            cursor: [0.; 2],
            right_mouse: false,
            dragging: false,
            keys: HashSet::new(),
            camera: DVec3::new(62., 48., 75.) * camera_scale,
            yaw: 140.,
            pitch: 26.,
            orbit,
            orthographic,
            orbit_radius: 100.0 * camera_scale,
            orthographic_span: 110.0 * camera_scale as f32,
            spectrum: true,
            offset: 0.,
            revision: GenerationRevision(1),
            last_submit: now - Duration::from_secs(1),
            last_audio: -1.,
            last_parameters: None,
            dirty: true,
            previous: now,
            started: now,
            fps: 60.,
            mesh_hz: 0.0,
            generation_ms: 0.,
            frames: 0,
            meshes: 0,
            mesh_hashes: Vec::new(),
            uploaded_columns: HashSet::new(),
            last_mesh_audio: 0.,
            upload_ms: 0.,
            gpu_ms: None,
            music_overlay: false,
            report_since: now,
            report_frames: 0,
            report_meshes: 0,
            report_sum_ms: 0.,
            report_max_ms: 0.,
            report_intervals: Vec::with_capacity(240),
            capture: None,
            live_mapping: MappingState::default(),
            live_parameters: None,
            live_paused: false,
            live_time: 0.0,
            scene: SceneStats::default(),
        };
        if orbit {
            app.aim_at_center();
        }
        if app.options.system_audio {
            app.action(Action::SystemAudio, false);
            if app.capture.is_none() {
                return Err(app.status.clone().into());
            }
        }
        Ok(app)
    }
    fn center(&self) -> DVec3 {
        DVec3::new(0.0, 16.0, 0.0)
    }
    fn aim_at_center(&mut self) {
        let delta = self.camera - self.center();
        self.orbit_radius = delta.length().clamp(8.0, 2048.0);
        self.yaw = delta.x.atan2(-delta.z).to_degrees() as f32;
        self.pitch = (delta.y / delta.length().max(0.001))
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees() as f32;
    }
    fn zoom(&mut self, lines: f32) {
        let factor = (-f64::from(lines.clamp(-10.0, 10.0)) * 0.12).exp();
        if self.orthographic {
            self.orthographic_span =
                (f64::from(self.orthographic_span) * factor).clamp(8.0, 4096.0) as f32;
        } else {
            let delta = self.camera - self.center();
            let radius = (delta.length() * factor).clamp(8.0, 2048.0);
            self.camera = self.center() + delta.normalize_or_zero() * radius;
            self.orbit_radius = radius;
        }
    }
    fn position(&self) -> f64 {
        if self.capture.is_some() {
            self.live_time
        } else {
            self.transport.as_ref().map_or(0., Transport::position)
        }
    }
    fn history(&mut self) {
        self.undo.push(self.preset.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn changed(&mut self) {
        self.revision.0 = self.revision.0.wrapping_add(1);
        self.dirty = true;
        tracing::info!(
            revision = self.revision.0,
            preset_version = self.preset.version,
            seed = self.preset.seed,
            "parameter snapshot changed"
        );
        self.live_mapping = MappingState::default();
        self.mapped.clear();
        if let Some(loaded) = &self.loaded {
            let mut state = MappingState::default();
            let dt = loaded.timeline.hop_size as f32 / loaded.timeline.sample_rate as f32;
            for f in &loaded.timeline.frames {
                match self
                    .preset
                    .mappings
                    .evaluate(f, &self.preset.parameters, dt, &mut state)
                {
                    Ok(p) => self.mapped.push(p),
                    Err(e) => {
                        self.status = e.to_string();
                        break;
                    }
                }
            }
        }
    }
    fn action(&mut self, a: Action, remember: bool) {
        tracing::info!(action=?a,audio_time=self.position(),revision=self.revision.0,"editor action");
        let edit = matches!(
            a,
            Action::Parameter(..)
                | Action::Preset(_)
                | Action::Reset
                | Action::NoiseType
                | Action::Octaves
                | Action::Seed
                | Action::MappingToggle(_)
                | Action::MappingCopy(_)
                | Action::MappingDelete(_)
                | Action::MappingGain(..)
                | Action::MappingTarget(_)
                | Action::MappingSource(_)
                | Action::MappingMode(_)
                | Action::MappingField(..)
                | Action::MappingCurve(_)
                | Action::MappingClamp(_)
        );
        if edit && remember {
            self.history();
        }
        match a {
            Action::SystemAudio => {
                if self.capture.is_some() {
                    self.capture = None;
                    self.live_parameters = None;
                    self.status = "FILE AUDIO MODE".into();
                    if let Some(t) = &self.transport {
                        t.play();
                    } else {
                        self.loader.submit(self.options.audio_path.clone());
                    }
                } else {
                    match crate::capture::SystemCapture::start() {
                        Ok(capture) => {
                            if let Some(t) = &self.transport {
                                t.pause();
                            }
                            self.capture = Some(capture);
                            self.live_paused = false;
                            self.live_time = 0.0;
                            self.live_mapping = MappingState::default();
                            self.status = "SYSTEM AUDIO LIVE".into();
                        }
                        Err(e) => self.status = format!("SYSTEM AUDIO ERROR: {e}"),
                    }
                }
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Orbit => {
                self.orbit = !self.orbit;
                if self.orbit {
                    self.aim_at_center();
                }
            }
            Action::Projection => {
                self.orthographic = !self.orthographic;
                if self.orthographic {
                    self.orthographic_span = ((self.camera - self.center()).length()
                        * 2.0
                        * (32.5_f64.to_radians()).tan())
                    .clamp(8.0, 4096.0) as f32;
                } else {
                    let delta = self.camera - self.center();
                    self.orbit_radius = (f64::from(self.orthographic_span)
                        / (2.0 * 32.5_f64.to_radians().tan()))
                    .clamp(8.0, 2048.0);
                    self.camera = self.center() + delta.normalize_or_zero() * self.orbit_radius;
                }
            }
            Action::Shape => {
                self.options.scene_options.shape = self.options.scene_options.shape.next();
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Detail => {
                self.options.scene_options.full_detail = !self.options.scene_options.full_detail;
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Language => {
                self.ui.language = self.ui.language.toggle();
                if let Err(e) = self.ui.language.save() {
                    self.status = format!("Could not save language preference: {e}");
                }
            }
            Action::Page(p) => {
                self.page = p;
                self.ui.scroll = 0;
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Play if self.capture.is_some() => {
                self.live_paused = !self.live_paused;
            }
            Action::Seek(_) if self.capture.is_some() => {}
            Action::Play => {
                if let Some(t) = &self.transport {
                    if t.is_silent() && !self.options.silent_preview {
                        self.status =
                            "NO AUDIO OUTPUT - USE --SILENT-PREVIEW FOR A SOFTWARE CLOCK".into();
                    } else if t.is_playing() {
                        t.pause()
                    } else {
                        t.play()
                    }
                }
            }
            Action::Seek(t) => {
                if let Some(a) = &self.transport {
                    a.seek(t);
                }
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Preset(i) => {
                if let Some(p) = self.presets.get(i) {
                    self.preset = p.clone();
                    self.changed();
                }
            }
            Action::Reset => {
                self.preset.parameters = TerrainParameters::default();
                self.changed();
            }
            Action::Undo => {
                if let Some(p) = self.undo.pop() {
                    self.redo.push(self.preset.clone());
                    self.preset = p;
                    self.changed();
                }
            }
            Action::Redo => {
                if let Some(p) = self.redo.pop() {
                    self.undo.push(self.preset.clone());
                    self.preset = p;
                    self.changed();
                }
            }
            Action::Save => {
                self.status = match self.preset.save(&self.options.preset_path) {
                    Ok(()) => "PRESET SAVED (EXISTING FILES ARE PRESERVED)".into(),
                    Err(e) => e.to_string(),
                };
            }
            Action::Load => match VisualizationPreset::load(&self.options.preset_path) {
                Ok(p) => {
                    self.history();
                    self.preset = p;
                    self.changed();
                    self.status = "PRESET LOADED".into();
                }
                Err(e) => self.status = e.to_string(),
            },
            Action::Spectrum => self.spectrum = !self.spectrum,
            Action::Offset(v) => {
                self.offset = v;
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Resolution(s) => {
                let ratio = f64::from(s) / f64::from(self.options.size);
                self.camera = self.center() + (self.camera - self.center()) * ratio;
                self.orbit_radius *= ratio;
                self.orthographic_span *= ratio as f32;
                self.options.size = s;
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Parameter(t, v) => {
                t.set(&mut self.preset.parameters, v);
                self.changed();
            }
            Action::NoiseType => {
                self.preset.parameters.noise_type = self.preset.parameters.noise_type.next();
                self.changed();
            }
            Action::Octaves => {
                self.preset.parameters.octaves = self.preset.parameters.octaves % 8 + 1;
                self.changed();
            }
            Action::Seed => {
                self.preset.seed = self.preset.seed.wrapping_add(1);
                self.changed();
            }
            Action::MappingToggle(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.enabled = !r.enabled;
                }
                self.changed();
            }
            Action::MappingCopy(i) => {
                if self.preset.mappings.rules.len() < 128
                    && let Some(r) = self.preset.mappings.rules.get(i).cloned()
                {
                    self.preset.mappings.rules.insert(i + 1, r);
                }
                self.changed();
            }
            Action::MappingDelete(i) => {
                if i < self.preset.mappings.rules.len() {
                    self.preset.mappings.rules.remove(i);
                }
                self.changed();
            }
            Action::MappingGain(i, g) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.gain = g;
                }
                self.changed();
            }
            Action::MappingTarget(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    use crate::terrain::TargetParameter;
                    let all: Vec<_> = TargetParameter::ALL
                        .into_iter()
                        .filter(|t| {
                            !matches!(
                                t,
                                TargetParameter::StructureDensity
                                    | TargetParameter::ParticleIntensity
                            )
                        })
                        .collect();
                    let next = all
                        .iter()
                        .position(|t| *t == r.target)
                        .map_or(0, |n| (n + 1) % all.len());
                    r.target = all[next];
                }
                self.changed();
            }
            Action::MappingSource(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.source = match r.source {
                        AudioSource::Band(n) if n < 6 => AudioSource::Band(n + 1),
                        AudioSource::Band(_) => AudioSource::Rms,
                        AudioSource::Rms => AudioSource::Centroid,
                        AudioSource::Centroid => AudioSource::Flux,
                        AudioSource::Flux => AudioSource::Onset,
                        AudioSource::Onset => AudioSource::BeatStrength,
                        AudioSource::BeatStrength => AudioSource::BeatPhase,
                        AudioSource::BeatPhase => AudioSource::Band(0),
                    };
                }
                self.changed();
            }
            Action::MappingMode(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.combine = match r.combine {
                        CombineMode::Replace => CombineMode::Add,
                        CombineMode::Add => CombineMode::Multiply,
                        CombineMode::Multiply => CombineMode::Replace,
                    };
                }
                self.changed();
            }
            Action::MappingField(i, field, v) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    match field {
                        RuleField::InputMin => r.input_range[0] = v.min(r.input_range[1] - 0.00001),
                        RuleField::InputMax => r.input_range[1] = v.max(r.input_range[0] + 0.00001),
                        RuleField::OutputMin => r.output_range[0] = v,
                        RuleField::OutputMax => r.output_range[1] = v,
                        RuleField::Bias => r.bias = v,
                        RuleField::Attack => r.attack = v,
                        RuleField::Release => r.release = v,
                    }
                }
                self.changed();
            }
            Action::MappingCurve(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.curve = match r.curve {
                        ResponseCurve::Linear => ResponseCurve::Smoothstep,
                        ResponseCurve::Smoothstep => ResponseCurve::Power(2.0),
                        ResponseCurve::Power(_) => ResponseCurve::Logarithmic,
                        ResponseCurve::Logarithmic => ResponseCurve::Linear,
                    };
                }
                self.changed();
            }
            Action::MappingClamp(i) => {
                if let Some(r) = self.preset.mappings.rules.get_mut(i) {
                    r.clamp = !r.clamp;
                }
                self.changed();
            }
            Action::MusicOverlay => {
                self.music_overlay = !self.music_overlay;
                self.revision.0 += 1;
                self.dirty = true;
            }
            Action::Vegetation => {
                self.options.scene_options.vegetation = !self.options.scene_options.vegetation;
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::Water => {
                self.options.scene_options.water = !self.options.scene_options.water;
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::Mines => {
                self.options.scene_options.mines = self.options.scene_options.mines.next();
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::UndergroundView => {
                self.options.scene_options.underground_view =
                    self.options.scene_options.underground_view.next();
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::Villages => {
                self.options.scene_options.villages = self.options.scene_options.villages.next();
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::Biome => {
                use crate::ecology::SceneBiome;
                self.options.scene_options.biome = match self.options.scene_options.biome {
                    None => Some(SceneBiome::ALL[0]),
                    Some(b) => SceneBiome::ALL
                        .iter()
                        .position(|v| *v == b)
                        .and_then(|i| SceneBiome::ALL.get(i + 1))
                        .copied(),
                };
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
            Action::Caves => {
                self.options.scene_options.caves = !self.options.scene_options.caves;
                self.revision.0 = self.revision.0.wrapping_add(1);
                self.dirty = true;
            }
        }
    }
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let raw_interval = now.duration_since(self.previous);
        let dt = raw_interval.as_secs_f32().min(0.1);
        self.previous = now;
        // Frame zero includes device creation; its first draw compiles pipelines.
        // Begin steady interval collection after both have completed.
        if self.frames <= 1 {
            self.report_since = now;
        } else {
            let ms = raw_interval.as_secs_f64() * 1000.;
            self.report_frames += 1;
            self.report_sum_ms += ms;
            self.report_max_ms = self.report_max_ms.max(ms);
            self.report_intervals.push(ms);
        }
        if self.capture.is_none()
            && let Some(result) = self.loader.receive.as_ref().and_then(|r| r.try_recv().ok())
        {
            match result {
                Ok(loaded) => {
                    self.capture = None;
                    self.live_parameters = None;
                    let transport = if self.options.silent_preview {
                        Ok(Transport::silent(loaded.audio.clone()))
                    } else {
                        Transport::new(loaded.audio.clone())
                    };
                    match transport {
                        Ok(t) => {
                            t.play();
                            self.transport = Some(t);
                            self.status = if self.options.silent_preview {
                                "SILENT PREVIEW - SOFTWARE CLOCK (NO AUDIO OUTPUT)".into()
                            } else {
                                "AUDIO PLAYING - DROP ANOTHER FILE TO IMPORT".into()
                            };
                        }
                        Err(e) => {
                            self.status = format!("AUDIO DEVICE ERROR: {e}");
                            self.transport = Some(Transport::silent(loaded.audio.clone()));
                        }
                    }
                    self.loaded = Some(loaded);
                    // Smoke acceptance must observe music-driven changes after import,
                    // not count the initial unmodulated loading preview.
                    self.mesh_hashes.clear();
                    self.meshes = 0;
                    self.changed();
                }
                Err(e) => {
                    self.status = format!("IMPORT ERROR: {e}");
                    if self.options.smoke_frames.is_some() {
                        self.error = Some(self.status.clone());
                        event_loop.exit();
                        return;
                    }
                }
            }
        }
        if self.transport.as_ref().is_some_and(Transport::failed) {
            self.status = "AUDIO DEVICE FAILED - PLAYBACK STOPPED".into();
        }
        let (forward, right) = movement_axes(self.yaw);
        let mut movement = DVec3::ZERO;
        for key in &self.keys {
            movement += match key {
                KeyCode::KeyW => forward,
                KeyCode::KeyS => -forward,
                KeyCode::KeyD => right,
                KeyCode::KeyA => -right,
                KeyCode::KeyE => DVec3::Y,
                KeyCode::KeyQ => -DVec3::Y,
                _ => DVec3::ZERO,
            };
        }
        if movement.length_squared() > 0.0 {
            self.orbit = false;
            self.camera += movement.normalize_or_zero() * f64::from(dt) * 45.;
        }
        if self.orbit {
            self.yaw = (self.yaw + dt * 10.0).rem_euclid(360.0);
            self.camera = self.center() + orbit_offset(self.yaw, self.pitch, self.orbit_radius);
        }
        let position = self.position();
        let audio_time = (position + f64::from(self.offset)).max(0.);
        if self.capture.is_some() && !self.live_paused {
            self.live_time += f64::from(dt);
        }
        let features = self
            .loaded
            .as_ref()
            .map(|l| l.timeline.at(audio_time))
            .unwrap_or_default();
        let features = self
            .capture
            .as_ref()
            .map_or(features, |capture| capture.features());
        if let Some(capture) = &self.capture {
            if capture.failed() {
                self.status = "SYSTEM AUDIO DISCONNECTED - RESELECT SOURCE".into();
            }
            if !self.live_paused {
                self.live_parameters = self
                    .preset
                    .mappings
                    .evaluate(
                        &features,
                        &self.preset.parameters,
                        dt,
                        &mut self.live_mapping,
                    )
                    .ok();
            }
        }
        let mut parameters = if self.page == Page::Noise && !self.music_overlay {
            self.preset.parameters.clone()
        } else {
            let index = self.loaded.as_ref().map_or(0, |l| {
                l.timeline
                    .frames
                    .partition_point(|frame| frame.time <= audio_time)
                    .saturating_sub(1)
                    .min(self.mapped.len().saturating_sub(1))
            });
            self.mapped
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.preset.parameters.clone())
        };
        if self.capture.is_some() && (self.page != Page::Noise || self.music_overlay) {
            parameters = self
                .live_parameters
                .clone()
                .unwrap_or_else(|| self.preset.parameters.clone());
        }
        if self.page != Page::Noise || self.music_overlay {
            parameters.cave_w = ((f64::from(parameters.cave_w)
                + audio_time * f64::from(parameters.cave_speed)
                + 128.0)
                .rem_euclid(256.0)
                - 128.0) as f32;
        }
        let mesh_parameters = parameters.clone();
        let interval =
            Duration::from_secs_f32((self.generation_ms * 0.00125).clamp(1.0 / 60.0, 0.5));
        if self.last_submit.elapsed() >= interval
            && (self.dirty
                || self.last_parameters.as_ref() != Some(&mesh_parameters)
                || (mesh_parameters.shockwave > 0.0 && (audio_time - self.last_audio).abs() > 0.03))
        {
            let snapshot = TerrainSnapshot {
                revision: self.revision,
                seed: self.preset.seed,
                parameters: mesh_parameters.clone(),
                audio_time,
            };
            if self.worker.submit_with_options(
                snapshot,
                self.options.size,
                self.options.scene_options,
            ) {
                self.last_submit = now;
                self.last_audio = audio_time;
                self.last_parameters = Some(mesh_parameters.clone());
                self.dirty = false;
            }
        }
        if let Some(result) = self.worker.poll(self.revision) {
            self.generation_ms = result.generation_ms;
            self.meshes += 1;
            self.last_mesh_audio = result.audio_time;
            self.scene = result.scene;
            let hash = result.checksum;
            if (self.loaded.is_some() || self.capture.is_some())
                && self.mesh_hashes.last() != Some(&hash)
                && self.mesh_hashes.len() < 32
            {
                self.mesh_hashes.push(hash);
            }
            if let Some(renderer) = &mut self.renderer {
                let upload_started = Instant::now();
                renderer
                    .renderer
                    .remove_columns(self.uploaded_columns.drain());
                for (key, mesh) in &result.sections {
                    renderer.renderer.upload(*key, mesh);
                    self.uploaded_columns.insert((key.0, key.2));
                }
                self.upload_ms = upload_started.elapsed().as_secs_f32() * 1000.0;
            }
            tracing::debug!(
                mesh_hash = hash,
                audio_time = self.last_mesh_audio,
                generation_ms = self.generation_ms,
                revision = self.revision.0,
                mines = self.scene.mines,
                rails = self.scene.rails,
                supports = self.scene.supports,
                villages = self.scene.villages,
                houses = self.scene.houses,
                farms = self.scene.farms,
                trees = self.scene.trees,
                plants = self.scene.plants,
                cave_faces = self.scene.cave_faces,
                water_quads = self.scene.water_quads,
                material_kinds = self.scene.material_quads.iter().filter(|n| **n > 0).count(),
                "preview mesh uploaded"
            );
        }
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        if let Some(error) = &self.worker.error {
            self.status = error.clone();
        }
        let diagnostics = if let Some(r) = &mut self.renderer {
            if let Some(ms) = r.renderer.gpu_times.pop() {
                self.gpu_ms = Some(ms);
            }
            r.renderer.gpu_times.clear();
            Diagnostics {
                mesh_hz: self.mesh_hz,
                submitted: self.worker.submitted,
                completed: self.worker.completed,
                dropped: self.worker.dropped,
                busy: self.worker.busy(),
                upload_ms: self.upload_ms,
                encode_ms: r.renderer.last_encode.as_secs_f32() * 1000.0,
                wait_ms: r.renderer.last_wait.as_secs_f32() * 1000.0,
                gpu_ms: self.gpu_ms,
                uploaded_bytes: r.renderer.uploaded_bytes,
                resident: r.renderer.resident_sections(),
                latency_ms: ((audio_time - self.last_mesh_audio).abs() * 1000.0) as f32,
            }
        } else {
            Diagnostics::default()
        };
        let report_elapsed = now.duration_since(self.report_since).as_secs_f64();
        if report_elapsed >= 1.0 && self.report_frames > 0 {
            // Match the visible FPS to the measured one-second frame count.
            self.report_intervals.sort_unstable_by(f64::total_cmp);
            let p95 = self.report_intervals
                [(self.report_intervals.len() * 95 / 100).min(self.report_intervals.len() - 1)];
            let p99 = self.report_intervals
                [(self.report_intervals.len() * 99 / 100).min(self.report_intervals.len() - 1)];
            self.fps = self.report_frames as f32 / report_elapsed as f32;
            self.mesh_hz =
                self.meshes.saturating_sub(self.report_meshes) as f32 / report_elapsed as f32;
            tracing::info!(
                frame_ms_mean = self.report_sum_ms / self.report_frames as f64,
                frame_ms_max = self.report_max_ms,
                frame_ms_p95 = p95,
                frame_ms_p99 = p99,
                fps = self.report_frames as f64 / report_elapsed,
                mesh_hz = self.meshes.saturating_sub(self.report_meshes) as f64 / report_elapsed,
                frames = self.report_frames,
                queued_jobs = self.worker.queued_jobs(),
                busy = self.worker.busy(),
                generation_ms = self.generation_ms,
                upload_ms = self.upload_ms,
                audio_time,
                revision = self.revision.0,
                startup_frames_skipped = 2,
                "preview performance"
            );
            self.report_since = now;
            self.report_meshes = self.meshes;
            self.report_frames = 0;
            self.report_sum_ms = 0.;
            self.report_max_ms = 0.;
            self.report_intervals.clear();
        }
        let hud = self.ui.draw(
            [size.width, size.height],
            View {
                preset: &self.preset,
                presets: &self.presets,
                page: self.page,
                features: &features,
                position,
                system_audio: self.capture.is_some(),
                duration: self.loaded.as_ref().map_or(0., |l| l.audio.duration()),
                playing: if self.capture.is_some() {
                    !self.live_paused
                } else {
                    self.transport.as_ref().is_some_and(Transport::is_playing)
                },
                spectrum: self.spectrum,
                fps: self.fps,
                generation_ms: self.generation_ms,
                revision: self.revision.0,
                status: &self.status,
                audio_name: if self.capture.is_some() {
                    "SYSTEM AUDIO LIVE"
                } else {
                    self.loaded
                        .as_ref()
                        .map_or("LOADING...", |l| l.name.as_str())
                },
                offset: self.offset,
                size: self.options.size,
                diagnostics,
                clock_label: if self.capture.is_some() {
                    "SYSTEM AUDIO LIVE"
                } else if self.transport.as_ref().is_some_and(Transport::is_silent) {
                    "SILENT SOFTWARE CLOCK - NO SOUND"
                } else {
                    "AUDIO OUTPUT CLOCK"
                },
                music_overlay: self.music_overlay,
                orbit: self.orbit,
                orthographic: self.orthographic,
                effective_parameters: &parameters,
                scene_options: self.options.scene_options,
                scene: &self.scene,
                texture_info: self
                    .renderer
                    .as_ref()
                    .map_or("TEXTURES INITIALIZING", |r| r.texture_info.as_str()),
            },
        );
        let ready = if self.capture.is_some() {
            self.meshes > 0
        } else {
            self.meshes >= 2 && self.mesh_hashes.len() >= 2
        };
        let last = self.options.smoke_frames.is_some_and(|n| self.frames >= n) && ready;
        if last && let Some(r) = &mut self.renderer {
            r.renderer.screenshot = self.options.screenshot.clone();
        }
        if let Some(r) = &mut self.renderer
            && let Err(e) = r.draw(
                self.camera,
                self.yaw,
                self.pitch,
                &hud,
                audio_time,
                self.orthographic.then_some(self.orthographic_span),
            )
        {
            self.error = Some(e.to_string());
            event_loop.exit();
            return;
        }
        self.frames += 1;
        if last {
            if let Some(r) = &self.renderer
                && let Some(Err(e)) = &r.renderer.screenshot_result
            {
                self.error = Some(e.clone());
            }
            tracing::info!(
                frames = self.frames,
                meshes = self.meshes,
                distinct_meshes = self.mesh_hashes.len(),
                elapsed = self.started.elapsed().as_secs_f64(),
                "preview smoke completed"
            );
            event_loop.exit();
        } else if self.options.smoke_frames.is_some()
            && self.started.elapsed() > Duration::from_secs(90)
        {
            self.error = Some(format!(
                "Preview smoke timed out: {} meshes, {} distinct meshes",
                self.meshes,
                self.mesh_hashes.len()
            ));
            event_loop.exit();
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn Error>> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("MusicTerrain | Horizon")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(1000., 650.)),
                )?,
            );
            self.renderer = Some(PreviewRenderer::new_with_resources(
                window.clone(),
                self.options.offscreen,
                self.options.resources.as_deref(),
            )?);
            self.window = Some(window);
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(e.to_string());
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => {
                if let Some(r) = &mut self.renderer {
                    r.renderer.resize(s.width, s.height);
                }
            }
            WindowEvent::DroppedFile(path) => {
                if self.loader.submit(Some(path)) {
                    self.capture = None;
                    self.live_parameters = None;
                    self.status = "ANALYZING AUDIO...".into();
                } else {
                    self.status = "AUDIO IMPORT QUEUE FULL - WAIT FOR CURRENT IMPORT".into();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let next = [position.x as f32, position.y as f32];
                if self.right_mouse {
                    self.yaw += (next[0] - self.cursor[0]) * 0.2;
                    self.pitch = (self.pitch + (next[1] - self.cursor[1]) * 0.2).clamp(-89., 89.);
                }
                if self.dragging
                    && let Some(a) = self.ui.action(next[0], next[1])
                    && matches!(
                        a,
                        Action::Parameter(..)
                            | Action::Offset(_)
                            | Action::Seek(_)
                            | Action::MappingGain(..)
                            | Action::MappingField(..)
                    )
                {
                    self.action(a, false);
                }
                self.cursor = next;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Right {
                    self.right_mouse = state == ElementState::Pressed;
                }
                if button == MouseButton::Left {
                    if state == ElementState::Pressed {
                        self.dragging = self.ui.is_slider(self.cursor[0], self.cursor[1]);
                        if let Some(a) = self.ui.action(self.cursor[0], self.cursor[1]) {
                            self.action(a, true);
                        }
                    } else {
                        self.dragging = false;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                let size = self.window.as_ref().map_or([1280.0, 800.0], |w| {
                    [w.inner_size().width as f32, w.inner_size().height as f32]
                });
                if self.cursor[0] > size[0] - 278.0 {
                    if y < 0. {
                        self.ui.scroll = (self.ui.scroll + 1).min(127);
                    } else {
                        self.ui.scroll = self.ui.scroll.saturating_sub(1);
                    }
                } else if self.cursor[0] > 196.0
                    && self.cursor[1] > 66.0
                    && self.cursor[1] < size[1] - 123.0
                {
                    self.zoom(y);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        self.keys.insert(key);
                        if !event.repeat {
                            let ctrl = self.keys.contains(&KeyCode::ControlLeft)
                                || self.keys.contains(&KeyCode::ControlRight);
                            match key {
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::Space => self.action(Action::Play, false),
                                KeyCode::ArrowLeft => {
                                    self.action(Action::Seek((self.position() - 5.).max(0.)), false)
                                }
                                KeyCode::ArrowRight => {
                                    self.action(Action::Seek(self.position() + 5.), false)
                                }
                                KeyCode::KeyZ if ctrl => self.action(Action::Undo, false),
                                KeyCode::KeyY if ctrl => self.action(Action::Redo, false),
                                KeyCode::KeyS if ctrl => self.action(Action::Save, false),
                                KeyCode::KeyO if ctrl => self.action(Action::Load, false),
                                _ => {}
                            }
                        }
                    } else {
                        self.keys.remove(&key);
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.right_mouse = false;
                self.dragging = false;
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
pub fn run(options: PreviewOptions) -> Result<(), Box<dyn Error>> {
    if ![64, 128, 256].contains(&options.size) {
        return Err("Preview size must be 64, 128 or 256".into());
    }
    let event_loop = EventLoop::new()?;
    let mut app = App::new(options)?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        Err(error.into())
    } else {
        Ok(())
    }
}

fn movement_axes(yaw_degrees: f32) -> (DVec3, DVec3) {
    let yaw = f64::from(yaw_degrees).to_radians();
    let forward = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    (forward, forward.cross(DVec3::Y))
}
fn orbit_offset(yaw: f32, pitch: f32, radius: f64) -> DVec3 {
    let yaw = f64::from(yaw).to_radians();
    let pitch = f64::from(pitch).to_radians();
    DVec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ) * radius
}
#[cfg(test)]
mod movement_tests {
    #[test]
    fn orbit_keeps_radius_and_looks_at_center_from_all_angles() {
        for yaw in [0.0, 45.0, 140.0, 270.0, 360.0] {
            for pitch in [-45.0, 0.0, 26.0, 80.0] {
                let delta = super::orbit_offset(yaw, pitch, 120.0);
                assert!((delta.length() - 120.0).abs() < 1e-6);
                let center = horizon_client_render::camera::view_matrix(yaw, pitch)
                    .transform_vector3((-delta).as_vec3());
                assert!(center.x.abs() < 1e-4 && center.y.abs() < 1e-4 && center.z < -119.99);
            }
        }
    }
    #[test]
    fn horizontal_right_agrees_with_rendered_camera_at_every_heading() {
        for yaw in [-180.0, -90.0, -15.0, 0.0, 45.0, 140.0, 270.0] {
            let (forward, right) = super::movement_axes(yaw);
            for pitch in [-45.0, 0.0, 45.0] {
                let view = horizon_client_render::camera::view_matrix(yaw, pitch);
                let screen_right = view.transform_vector3(right.as_vec3());
                assert!(screen_right.x > 0.999, "heading={yaw} pitch={pitch}");
                assert!(view.transform_vector3((-right).as_vec3()).x < -0.999);
                assert!(view.transform_vector3(forward.as_vec3()).z < 0.0);
            }
        }
    }
}
