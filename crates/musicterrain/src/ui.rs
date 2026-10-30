//! Immediate-mode editor drawn by Horizon's existing HUD pipeline.
use crate::{
    dsp::AudioFeatures,
    preset::VisualizationPreset,
    terrain::{TargetParameter, TerrainField, TerrainParameters},
    visual::{SceneOptions, SceneStats},
};
use horizon_client_render::hud::{HudBatch, rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Live,
    Noise,
    Mapping,
    Presets,
    Settings,
    Diagnostics,
    World,
}
#[derive(Clone, Copy, Debug)]
pub enum RuleField {
    InputMin,
    InputMax,
    OutputMin,
    OutputMax,
    Bias,
    Attack,
    Release,
}
#[derive(Clone, Debug)]
pub enum Action {
    Page(Page),
    Play,
    Seek(f64),
    Preset(usize),
    Reset,
    Undo,
    Redo,
    Save,
    Load,
    Spectrum,
    Offset(f32),
    Resolution(u32),
    Parameter(TargetParameter, f32),
    NoiseType,
    Octaves,
    Seed,
    MappingToggle(usize),
    MappingCopy(usize),
    MappingDelete(usize),
    MappingGain(usize, f32),
    MappingTarget(usize),
    MappingSource(usize),
    MappingMode(usize),
    MappingField(usize, RuleField, f32),
    MappingCurve(usize),
    MappingClamp(usize),
    MusicOverlay,
    Vegetation,
    Water,
    Caves,
}
#[derive(Clone)]
struct Hit {
    rect: [f32; 4],
    action: Action,
    slider: Option<[f32; 2]>,
}
#[derive(Default)]
pub struct Ui {
    hits: Vec<Hit>,
    pub scroll: usize,
    noise_key: Option<(i64, TerrainParameters)>,
    noise_colors: [Vec<u32>; 3],
}
pub struct View<'a> {
    pub clock_label: &'a str,
    pub preset: &'a VisualizationPreset,
    pub presets: &'a [VisualizationPreset],
    pub page: Page,
    pub features: &'a AudioFeatures,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub spectrum: bool,
    pub fps: f32,
    pub generation_ms: f32,
    pub revision: u64,
    pub status: &'a str,
    pub audio_name: &'a str,
    pub offset: f32,
    pub size: u32,
    pub music_overlay: bool,
    pub effective_parameters: &'a TerrainParameters,
    pub diagnostics: Diagnostics,
    pub scene_options: SceneOptions,
    pub scene: &'a SceneStats,
    pub texture_info: &'a str,
}
#[derive(Default)]
pub struct Diagnostics {
    pub submitted: u64,
    pub completed: u64,
    pub dropped: u64,
    pub busy: bool,
    pub upload_ms: f32,
    pub encode_ms: f32,
    pub wait_ms: f32,
    pub gpu_ms: Option<f32>,
    pub uploaded_bytes: u64,
    pub resident: usize,
    pub latency_ms: f32,
}
const INK: u32 = 0xfff3edec;
const MUTED: u32 = 0xff9c8f8b;
const ACCENT: u32 = 0xffff869a;
const PLATE: u32 = 0xff221a18;
impl Ui {
    pub fn action(&self, x: f32, y: f32) -> Option<Action> {
        self.hits.iter().rev().find_map(|h| {
            let [a, b, w, t] = h.rect;
            if x < a || x > a + w || y < b || y > b + t {
                return None;
            }
            let mut action = h.action.clone();
            if let Some([lo, hi]) = h.slider {
                let v = lo + (hi - lo) * ((x - a) / w).clamp(0.0, 1.0);
                match &mut action {
                    Action::Parameter(_, n)
                    | Action::Offset(n)
                    | Action::MappingGain(_, n)
                    | Action::MappingField(_, _, n) => *n = v,
                    Action::Seek(n) => *n = f64::from(v),
                    _ => {}
                }
            }
            Some(action)
        })
    }
    pub fn is_slider(&self, x: f32, y: f32) -> bool {
        self.hits.iter().any(|h| {
            h.slider.is_some()
                && x >= h.rect[0]
                && x <= h.rect[0] + h.rect[2]
                && y >= h.rect[1]
                && y <= h.rect[1] + h.rect[3]
        })
    }
    fn button(&mut self, h: &mut HudBatch, r: [f32; 4], label: &str, action: Action, active: bool) {
        rect(h, r, if active { ACCENT } else { PLATE });
        text(
            h,
            r[0] + 8.0,
            r[1] + 9.0,
            label,
            if active { 0xff130e0d } else { INK },
            1.0,
        );
        self.hits.push(Hit {
            rect: r,
            action,
            slider: None,
        });
    }
    fn slider(
        &mut self,
        h: &mut HudBatch,
        r: [f32; 4],
        label: &str,
        value: f32,
        bounds: [f32; 2],
        action: Action,
    ) {
        text(h, r[0], r[1], label, INK, 1.0);
        text(
            h,
            r[0] + r[2] - 62.0,
            r[1],
            &format!("{value:.3}"),
            MUTED,
            1.0,
        );
        let track = [r[0], r[1] + 14.0, r[2], 14.0];
        rect(h, [track[0], track[1] + 5.0, track[2], 2.0], 0xff3d322f);
        let t = ((value - bounds[0]) / (bounds[1] - bounds[0])).clamp(0.0, 1.0);
        rect(h, [track[0], track[1] + 5.0, track[2] * t, 2.0], ACCENT);
        rect(
            h,
            [track[0] + track[2] * t - 3.0, track[1] + 2.0, 6.0, 8.0],
            INK,
        );
        self.hits.push(Hit {
            rect: track,
            action,
            slider: Some(bounds),
        });
    }
    pub fn draw(&mut self, size: [u32; 2], v: View<'_>) -> HudBatch {
        self.hits.clear();
        let mut h = HudBatch::new(size);
        h.scale = 1.0;
        h.width = size[0] as f32;
        h.height = size[1] as f32;
        let w = h.width;
        let height = h.height;
        let right = w - 278.0;
        rect(&mut h, [0.0, 0.0, w, 66.0], 0xf5130e0d);
        rect(&mut h, [0.0, 66.0, 195.0, height - 66.0], 0xed130e0d);
        rect(&mut h, [right, 66.0, 278.0, height - 66.0], 0xf5130e0d);
        rect(
            &mut h,
            [195.0, height - 123.0, (right - 195.0).max(10.0), 123.0],
            0xf5130e0d,
        );
        text(&mut h, 20.0, 19.0, "MUSICTERRAIN", INK, 2.0);
        text(&mut h, 220.0, 18.0, v.audio_name, MUTED, 1.0);
        text(
            &mut h,
            220.0,
            37.0,
            "DROP MP3 / WAV / FLAC TO IMPORT",
            MUTED,
            1.0,
        );
        self.button(
            &mut h,
            [w - 278.0, 17.0, 72.0, 30.0],
            if v.playing { "PAUSE" } else { "PLAY" },
            Action::Play,
            true,
        );
        self.button(
            &mut h,
            [w - 198.0, 17.0, 80.0, 30.0],
            "SAVE",
            Action::Save,
            false,
        );
        self.button(
            &mut h,
            [w - 110.0, 17.0, 92.0, 30.0],
            "LOAD",
            Action::Load,
            false,
        );
        text(&mut h, 220.0, 52.0, v.clock_label, MUTED, 1.0);
        let pages = [
            (Page::Live, "01 LIVE"),
            (Page::World, "02 MUSIC WORLD"),
            (Page::Noise, "03 NOISE LAB"),
            (Page::Mapping, "04 MAPPINGS"),
            (Page::Presets, "05 PRESETS"),
            (Page::Settings, "06 SETTINGS"),
            (Page::Diagnostics, "07 DIAGNOSTICS"),
        ];
        for (i, (p, label)) in pages.into_iter().enumerate() {
            self.button(
                &mut h,
                [12.0, 86.0 + i as f32 * 43.0, 170.0, 32.0],
                label,
                Action::Page(p),
                v.page == p,
            );
        }
        text(&mut h, 18.0, height - 157.0, "TEXTURE SOURCE", MUTED, 1.0);
        let texture_label: String = v.texture_info.chars().take(27).collect();
        text(&mut h, 18.0, height - 139.0, &texture_label, INK, 1.0);
        text(&mut h, 18.0, height - 110.0, "RMB + MOUSE LOOK", MUTED, 1.0);
        text(&mut h, 18.0, height - 91.0, "WASD / Q E FLY", MUTED, 1.0);
        text(
            &mut h,
            18.0,
            height - 72.0,
            "SPACE PLAY / PAUSE",
            MUTED,
            1.0,
        );
        text(&mut h, 18.0, height - 53.0, "LEFT RIGHT SEEK", MUTED, 1.0);
        text(
            &mut h,
            18.0,
            height - 34.0,
            "CTRL Z / Y HISTORY",
            MUTED,
            1.0,
        );
        text(&mut h, right + 18.0, 85.0, &v.preset.name, INK, 1.0);
        self.button(
            &mut h,
            [right + 18.0, 109.0, 72.0, 29.0],
            "RESET",
            Action::Reset,
            false,
        );
        self.button(
            &mut h,
            [right + 98.0, 109.0, 70.0, 29.0],
            "UNDO",
            Action::Undo,
            false,
        );
        self.button(
            &mut h,
            [right + 176.0, 109.0, 80.0, 29.0],
            "REDO",
            Action::Redo,
            false,
        );
        let x = right + 18.0;
        let sw = 238.0;
        match v.page {
            Page::Live | Page::Noise => {
                self.button(
                    &mut h,
                    [x, 151.0, sw, 28.0],
                    &format!("NOISE {:?}", v.preset.parameters.noise_type),
                    Action::NoiseType,
                    false,
                );
                self.button(
                    &mut h,
                    [x, 187.0, 112.0, 28.0],
                    &format!("OCTAVES {}", v.preset.parameters.octaves),
                    Action::Octaves,
                    false,
                );
                self.button(
                    &mut h,
                    [x + 120.0, 187.0, 118.0, 28.0],
                    &format!("SEED {}", v.preset.seed),
                    Action::Seed,
                    false,
                );
                let n = ((height - 300.0) / 45.0).max(1.0) as usize;
                let targets: Vec<_> = TargetParameter::ALL
                    .into_iter()
                    .filter(|t| {
                        !matches!(
                            t,
                            TargetParameter::StructureDensity | TargetParameter::ParticleIntensity
                        )
                    })
                    .collect();
                let start = self.scroll.min(targets.len().saturating_sub(n));
                for (i, t) in targets.into_iter().skip(start).take(n).enumerate() {
                    let bounds = match t {
                        TargetParameter::Amplitude => [0.0, 80.0],
                        TargetParameter::BaseHeight => [0.0, 64.0],
                        TargetParameter::Frequency => [0.002, 0.15],
                        TargetParameter::Scale => [0.1, 5.0],
                        TargetParameter::DomainWarp => [0.0, 40.0],
                        _ => t.bounds(),
                    };
                    self.slider(
                        &mut h,
                        [x, 232.0 + i as f32 * 45.0, sw, 28.0],
                        &format!("{t:?}"),
                        t.get(&v.preset.parameters),
                        bounds,
                        Action::Parameter(t, 0.0),
                    );
                }
                text(
                    &mut h,
                    x,
                    height - 40.0,
                    "SCROLL FOR MORE PARAMETERS",
                    MUTED,
                    1.0,
                );
                if v.page == Page::Noise {
                    rect(&mut h, [212.0, 80.0, 80.0, 16.0], 0xee130e0d);
                    text(&mut h, 218.0, 85.0, "NOISE LAB", INK, 1.0);
                    self.button(
                        &mut h,
                        [215.0, 119.0, 210.0, 29.0],
                        if v.music_overlay {
                            "MUSIC OVERLAY ON"
                        } else {
                            "MUSIC OVERLAY OFF"
                        },
                        Action::MusicOverlay,
                        v.music_overlay,
                    );
                    let key = (v.preset.seed, v.effective_parameters.clone());
                    if self.noise_key.as_ref() != Some(&key) {
                        let field = TerrainField::new(key.0);
                        for colors in &mut self.noise_colors {
                            colors.clear();
                        }
                        for z in 0..32 {
                            for x in 0..32 {
                                let wx = f64::from(x * 2 - 32);
                                let wz = f64::from(z * 2 - 32);
                                let p = &key.1;
                                let h = ((field.height(wx, wz, p) + 16.0) / 96.0).clamp(0.0, 1.0);
                                self.noise_colors[0].push(rgba(
                                    (h * 170.0 + 25.0) as u8,
                                    (h * 190.0 + 30.0) as u8,
                                    (h * 120.0 + 35.0) as u8,
                                    255,
                                ));
                                let density = field.density(wx, 20.0, wz, p);
                                let d = ((density + 20.0) / 40.0).clamp(0.0, 1.0);
                                self.noise_colors[1].push(rgba(
                                    (d * 210.0) as u8,
                                    (d * 180.0) as u8,
                                    (50.0 + d * 150.0) as u8,
                                    255,
                                ));
                                let solid = field.density(wx, f64::from(62 - z * 2), 0.0, p) > 0.0;
                                self.noise_colors[2].push(if solid {
                                    rgba(160, 142, 118, 255)
                                } else {
                                    rgba(20, 30, 48, 255)
                                });
                            }
                        }
                        self.noise_key = Some(key);
                    }
                    let available = (right - 230.0).max(240.0);
                    let side = (available / 3.0 - 12.0).clamp(64.0, 160.0);
                    let top = height - 123.0 - side - 45.0;
                    for (i, label) in ["HEIGHT XZ", "DENSITY Y=20", "SECTION Z=0"]
                        .into_iter()
                        .enumerate()
                    {
                        let left = 215.0 + i as f32 * (side + 12.0);
                        rect(
                            &mut h,
                            [left - 4.0, top - 20.0, side + 8.0, side + 26.0],
                            0xed130e0d,
                        );
                        text(&mut h, left, top - 14.0, label, INK, 1.0);
                        let cell = side / 32.0;
                        for (index, color) in self.noise_colors[i].iter().enumerate() {
                            rect(
                                &mut h,
                                [
                                    left + (index % 32) as f32 * cell,
                                    top + (index / 32) as f32 * cell,
                                    cell,
                                    cell,
                                ],
                                *color,
                            );
                        }
                    }
                }
            }
            Page::Mapping => {
                let n = ((height - 220.0) / 310.0).max(1.0) as usize;
                for (i, (index, rule)) in v
                    .preset
                    .mappings
                    .rules
                    .iter()
                    .enumerate()
                    .skip(self.scroll)
                    .take(n)
                    .enumerate()
                {
                    let y = 157.0 + i as f32 * 310.0;
                    if matches!(
                        rule.target,
                        TargetParameter::StructureDensity | TargetParameter::ParticleIntensity
                    ) {
                        text(&mut h, x, y, &format!("{:?}", rule.target), MUTED, 1.0);
                        text(
                            &mut h,
                            x,
                            y + 24.0,
                            "VISUAL TARGET NOT IMPLEMENTED",
                            MUTED,
                            1.0,
                        );
                        self.button(
                            &mut h,
                            [x, y + 53.0, sw, 29.0],
                            "CHOOSE ACTIVE TARGET",
                            Action::MappingTarget(index),
                            false,
                        );
                        self.button(
                            &mut h,
                            [x, y + 93.0, sw, 29.0],
                            "DELETE RULE",
                            Action::MappingDelete(index),
                            false,
                        );
                        continue;
                    }
                    self.button(
                        &mut h,
                        [x, y, 116.0, 25.0],
                        &format!("{:?}", rule.source),
                        Action::MappingSource(index),
                        false,
                    );
                    self.button(
                        &mut h,
                        [x + 120.0, y, 118.0, 25.0],
                        &format!("{:?}", rule.target),
                        Action::MappingTarget(index),
                        false,
                    );
                    self.button(
                        &mut h,
                        [x, y + 30.0, 44.0, 24.0],
                        if rule.enabled { "ON" } else { "OFF" },
                        Action::MappingToggle(index),
                        rule.enabled,
                    );
                    self.button(
                        &mut h,
                        [x + 48.0, y + 30.0, 56.0, 24.0],
                        "COPY",
                        Action::MappingCopy(index),
                        false,
                    );
                    self.button(
                        &mut h,
                        [x + 108.0, y + 30.0, 44.0, 24.0],
                        "DEL",
                        Action::MappingDelete(index),
                        false,
                    );
                    self.button(
                        &mut h,
                        [x + 156.0, y + 30.0, 82.0, 24.0],
                        &format!("{:?}", rule.combine),
                        Action::MappingMode(index),
                        false,
                    );
                    self.slider(
                        &mut h,
                        [x, y + 59.0, sw, 28.0],
                        "GAIN",
                        rule.gain,
                        [0.0, 8.0],
                        Action::MappingGain(index, 0.0),
                    );
                    let input_bounds = if rule.source == crate::mapping::AudioSource::Centroid {
                        [0.0, 24000.0]
                    } else {
                        [0.0, 1.0]
                    };
                    let [target_min, target_max] = rule.target.bounds();
                    let span = (target_max - target_min).max(1.0);
                    let output_bounds = [-span, span];
                    let rows = [
                        [
                            (
                                "INPUT MIN",
                                rule.input_range[0],
                                input_bounds,
                                RuleField::InputMin,
                            ),
                            (
                                "INPUT MAX",
                                rule.input_range[1],
                                input_bounds,
                                RuleField::InputMax,
                            ),
                        ],
                        [
                            (
                                "OUTPUT MIN",
                                rule.output_range[0],
                                output_bounds,
                                RuleField::OutputMin,
                            ),
                            (
                                "OUTPUT MAX",
                                rule.output_range[1],
                                output_bounds,
                                RuleField::OutputMax,
                            ),
                        ],
                        [
                            ("ATTACK S", rule.attack, [0.0, 2.0], RuleField::Attack),
                            ("RELEASE S", rule.release, [0.0, 2.0], RuleField::Release),
                        ],
                    ];
                    for (row, items) in rows.into_iter().enumerate() {
                        for (col, (label, value, bounds, field)) in items.into_iter().enumerate() {
                            self.slider(
                                &mut h,
                                [
                                    x + col as f32 * 122.0,
                                    y + 100.0 + row as f32 * 42.0,
                                    116.0,
                                    28.0,
                                ],
                                label,
                                value,
                                bounds,
                                Action::MappingField(index, field, 0.0),
                            );
                        }
                    }
                    self.slider(
                        &mut h,
                        [x, y + 226.0, sw, 28.0],
                        "BIAS",
                        rule.bias,
                        [-4.0, 4.0],
                        Action::MappingField(index, RuleField::Bias, 0.0),
                    );
                    self.button(
                        &mut h,
                        [x, y + 267.0, 146.0, 25.0],
                        &format!("{:?}", rule.curve),
                        Action::MappingCurve(index),
                        false,
                    );
                    self.button(
                        &mut h,
                        [x + 152.0, y + 267.0, 86.0, 25.0],
                        if rule.clamp { "CLAMP ON" } else { "CLAMP OFF" },
                        Action::MappingClamp(index),
                        rule.clamp,
                    );
                }
                text(
                    &mut h,
                    x,
                    height - 40.0,
                    "SCROLL RULES - ORDER IS STABLE",
                    MUTED,
                    1.0,
                );
            }
            Page::Presets => {
                for (i, p) in v.presets.iter().enumerate() {
                    self.button(
                        &mut h,
                        [x, 158.0 + i as f32 * 44.0, sw, 31.0],
                        &p.name,
                        Action::Preset(i),
                        p.name == v.preset.name,
                    );
                }
            }
            Page::Settings => {
                self.button(
                    &mut h,
                    [x, 158.0, sw, 30.0],
                    if v.spectrum {
                        "SPECTRUM ON"
                    } else {
                        "SPECTRUM OFF"
                    },
                    Action::Spectrum,
                    false,
                );
                self.slider(
                    &mut h,
                    [x, 209.0, sw, 28.0],
                    "AV OFFSET SECONDS",
                    v.offset,
                    [-0.5, 0.5],
                    Action::Offset(0.0),
                );
                for (i, s) in [64, 128, 256].into_iter().enumerate() {
                    self.button(
                        &mut h,
                        [x, 263.0 + i as f32 * 41.0, sw, 30.0],
                        &format!("PREVIEW {s} X {s}"),
                        Action::Resolution(s),
                        v.size == s,
                    );
                }
                text(&mut h, x, 414.0, "AUDIO OUTPUT: DEFAULT DEVICE", MUTED, 1.0);
                text(&mut h, x, 437.0, "CAPTURE: NOT IMPLEMENTED", MUTED, 1.0);
                self.button(
                    &mut h,
                    [x, 470.0, sw, 30.0],
                    if v.scene_options.vegetation {
                        "TREES / PLANTS ON"
                    } else {
                        "TREES / PLANTS OFF"
                    },
                    Action::Vegetation,
                    v.scene_options.vegetation,
                );
                self.button(
                    &mut h,
                    [x, 509.0, sw, 30.0],
                    if v.scene_options.water {
                        "WATER ON"
                    } else {
                        "WATER OFF"
                    },
                    Action::Water,
                    v.scene_options.water,
                );
                self.button(
                    &mut h,
                    [x, 548.0, sw, 30.0],
                    if v.scene_options.caves {
                        "CAVES ON"
                    } else {
                        "CAVES OFF"
                    },
                    Action::Caves,
                    v.scene_options.caves,
                );
                text(&mut h, x, 597.0, "TEXTURES", MUTED, 1.0);
                let source: String = v.texture_info.chars().take(38).collect();
                text(&mut h, x, 616.0, &source, INK, 1.0);
            }
            Page::Diagnostics => {
                let lines = [
                    format!("FRAME {:.2} MS", 1000.0 / v.fps.max(0.1)),
                    format!("FPS {:.1}", v.fps),
                    format!("GENERATION {:.2} MS", v.generation_ms),
                    format!("REVISION {}", v.revision),
                    format!("QUEUE CAPACITY 1 - BUSY {}", v.diagnostics.busy),
                    format!("SUBMITTED {}", v.diagnostics.submitted),
                    format!("COMPLETED {}", v.diagnostics.completed),
                    format!("DROPPED {}", v.diagnostics.dropped),
                    format!("UPLOAD ENQUEUE {:.2} MS", v.diagnostics.upload_ms),
                    format!("RENDER ENCODE {:.2} MS", v.diagnostics.encode_ms),
                    format!("RENDER WAIT {:.2} MS", v.diagnostics.wait_ms),
                    format!(
                        "GPU {} MS",
                        v.diagnostics
                            .gpu_ms
                            .map_or_else(|| "N/A".into(), |m| format!("{m:.2}"))
                    ),
                    format!("UPLOAD BYTES {}", v.diagnostics.uploaded_bytes),
                    format!("RESIDENT SECTIONS {}", v.diagnostics.resident),
                    format!("MESH AGE {:.1} MS", v.diagnostics.latency_ms),
                    format!("RMS {:.4}", v.features.rms),
                    format!("CENTROID {:.0} HZ", v.features.centroid),
                    format!("FLUX {:.4}", v.features.flux),
                    format!("SILENCE {}", v.features.silence),
                    format!("TREES {} - PLANTS {}", v.scene.trees, v.scene.plants),
                    format!("CAVE FACES {}", v.scene.cave_faces),
                    format!("WATER QUADS {}", v.scene.water_quads),
                    format!(
                        "MATERIAL KINDS {}",
                        v.scene.material_quads.iter().filter(|n| **n > 0).count()
                    ),
                    format!("SOLID VOXELS {}", v.scene.solid_voxels),
                    format!("CARVED VOXELS {}", v.scene.carved_voxels),
                ];
                let spacing = ((height - 200.0) / lines.len() as f32).clamp(16.0, 28.0);
                for (i, s) in lines.into_iter().enumerate() {
                    text(&mut h, x, 160.0 + i as f32 * spacing, &s, INK, 1.0);
                }
            }
            Page::World => {
                text(&mut h, x, 162.0, "MUSIC WORLD: UNAVAILABLE", INK, 1.0);
                text(&mut h, x, 189.0, "SAVE ADAPTER NOT IMPLEMENTED", MUTED, 1.0);
                text(&mut h, x, 216.0, "NO WORLD FILES ARE WRITTEN", MUTED, 1.0);
            }
        }
        let cw = (right - 235.0).max(60.0);
        let y = height - 108.0;
        if v.spectrum {
            for (i, e) in v.features.bands.iter().enumerate() {
                let bw = (cw - 25.0) / 7.0;
                let bh = (e * 5.0).sqrt().clamp(0.0, 1.0) * 39.0;
                rect(
                    &mut h,
                    [215.0 + i as f32 * (bw + 3.0), y + 42.0 - bh, bw, bh],
                    ACCENT,
                );
            }
        }
        text(
            &mut h,
            215.0,
            height - 54.0,
            &format!(
                "{:.2} / {:.2} S     RMS {:.3}     {:.0} FPS",
                v.position, v.duration, v.features.rms, v.fps
            ),
            INK,
            1.0,
        );
        self.slider(
            &mut h,
            [215.0, height - 31.0, cw, 24.0],
            "",
            v.position as f32,
            [0.0, v.duration.max(0.001) as f32],
            Action::Seek(0.0),
        );
        let status: String = v
            .status
            .chars()
            .take(((right - 225.0) / 6.0).max(1.0) as usize)
            .collect();
        rect(
            &mut h,
            [209.0, 92.0, status.len() as f32 * 6.0 + 12.0, 16.0],
            0xee130e0d,
        );
        text(&mut h, 215.0, 96.0, &status, rgba(240, 190, 140, 255), 1.0);
        h
    }
}
pub fn rect(h: &mut HudBatch, [x, y, w, t]: [f32; 4], color: u32) {
    h.quad(
        [[x, y], [x + w, y], [x + w, y + t], [x, y + t]],
        [[0.5, 0.5]; 4],
        color,
    );
}
pub fn text(h: &mut HudBatch, mut x: f32, y: f32, s: &str, color: u32, scale: f32) {
    for ch in s.to_ascii_uppercase().chars() {
        let rows = glyph(ch);
        for (row, bits) in rows.into_iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    rect(
                        h,
                        [x + col as f32 * scale, y + row as f32 * scale, scale, scale],
                        color,
                    );
                }
            }
        }
        x += 6.0 * scale;
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        ':' => [0, 6, 6, 0, 6, 6, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '[' => [14, 8, 8, 8, 8, 8, 14],
        ']' => [14, 2, 2, 2, 2, 2, 14],
        _ => [0; 7],
    }
}
