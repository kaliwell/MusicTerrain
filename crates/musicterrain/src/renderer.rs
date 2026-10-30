//! The MusicTerrain preview uses Horizon's production section renderer.
//! Textures are original procedural pixels unless a local resource pack is selected.
use crate::materials::TextureAtlas;
use glam::{DVec3, Mat4};
use horizon_client_render::{
    environment::Sky,
    gfx::{Frame, Renderer, pack_uniforms::PackInputs, panorama::Faces},
    graphics::GraphicsApi,
    hud::HudBatch,
    weather,
};
use std::{error::Error, path::Path, sync::Arc};
use winit::window::Window;

pub struct PreviewRenderer {
    pub renderer: Renderer,
    pub texture_info: String,
    cloud_texture: (u32, u32, Vec<bool>),
}

impl PreviewRenderer {
    pub fn new(window: Arc<Window>, offscreen: bool) -> Result<Self, Box<dyn Error>> {
        Self::new_with_resources(window, offscreen, None)
    }

    pub fn new_with_resources(
        window: Arc<Window>,
        offscreen: bool,
        resources: Option<&Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let atlas = TextureAtlas::load(resources)?;
        let texture_info = if atlas.loaded == 0 {
            "ORIGINAL MATERIALS: 17".to_owned()
        } else {
            format!("LOCAL PACK {}/17 + FALLBACK", atlas.loaded)
        };
        let layer_bytes = (atlas.size * atlas.size * 4) as usize;
        let mips: Vec<_> = atlas
            .pixels
            .chunks_exact(layer_bytes)
            .map(|pixels| horizon_client_render::mipmap::Settings {
                has_transparent: pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 0),
                ..Default::default()
            })
            .collect();
        tracing::info!(
            loaded_materials = atlas.loaded,
            material_layers = atlas.layers,
            "preview material atlas prepared"
        );
        let white = [255u8; 4];
        let panorama = Faces {
            size: 1,
            pixels: [20, 25, 38, 255].repeat(6),
        };
        let mut renderer = pollster::block_on(Renderer::new(
            window,
            offscreen,
            &atlas.pixels,
            atlas.size,
            atlas.size,
            atlas.layers,
            &mips,
            &[],
            (&white, 1, 1),
            (&white, 1, 1),
            (&white, 1, 1),
            (&white, 1),
            &panorama,
            None,
            None,
            GraphicsApi::Auto,
        ))?;
        tracing::info!(adapter=%renderer.info.name, backend=?renderer.info.backend, device_type=?renderer.info.device_type, "Horizon preview renderer ready");
        renderer.fullbright = true;
        renderer.animated_textures = false;
        let cloud_texture = (
            32,
            32,
            (0..32)
                .flat_map(|z| {
                    (0..32).map(move |x| {
                        (x / 3 + z / 2) % 5 == 0 || ((x / 4) * 7 + (z / 3) * 3) % 11 < 3
                    })
                })
                .collect(),
        );
        Ok(Self {
            renderer,
            texture_info,
            cloud_texture,
        })
    }

    pub fn draw(
        &mut self,
        camera: DVec3,
        yaw: f32,
        pitch: f32,
        hud: &HudBatch,
        time: f64,
    ) -> Result<bool, Box<dyn Error>> {
        let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let ticks = (time * 20.0) as u64;
        let frame = Frame {
            camera,
            yaw,
            pitch,
            fov_degrees: 65.0,
            bob: Mat4::IDENTITY,
            spin: Mat4::IDENTITY,
            sky: Sky {
                sky_color: [0.34, 0.60, 0.93],
                fog_color: [0.65, 0.80, 0.96],
                sky_light_factor: 1.0,
                sky_light_color: [1.0; 3],
                block_light_tint: [1.0; 3],
                ambient_light_color: [0.6; 3],
                fog_start: 192.0,
                fog_end: 384.0,
                cloud_fog_end: 384.0,
                ..Sky::default()
            },
            distance: 384.0,
            detail_distance: 384.0,
            depth_far: 1024.0,
            underwater: false,
            cave_culling: false,
            title: false,
            sky_blocked: false,
            skybox: true,
            end_sky: false,
            end_sky_sprite: None,
            environment_fog: None,
            water_fog: [0.08, 0.22, 0.38],
            fog: None,
            dark_disc: false,
            outline: Vec::new(),
            outline_after_terrain: false,
            outline_high_contrast: false,
            debug_lines: &[],
            debug_lines_on_top: &[],
            debug_crosshair: &[],
            hud,
            glint_alpha: 0.0,
            glint_ticks: ticks as f32,
            nether_lighting: false,
            entities: &[],
            glowing: &[],
            entity_spans: &[],
            weather: &[],
            weather_time: weather::Time::new(ticks as i64, 0.0),
            dynamic_lights: &[],
            clouds: Some(horizon_client_render::clouds::Input {
                texture: &self.cloud_texture,
                uv: [0.0; 2],
                camera,
                time: ticks as i64,
                partial_tick: 0.0,
                range: 8,
                fast: true,
                color: [1.0, 1.0, 1.0, 0.8],
            }),
            lightning: Default::default(),
            additive: &[],
            see_through: &[],
            celestial: &[],
            stars: None,
            hand: &[],
            hand_fov_degrees: 65.0,
            screen: &[],
            pack: PackInputs::default(),
            border: None,
            game_time: ticks as f32,
            shader_game_time: ((time * 20.0) % 24000.0 / 24000.0) as f32,
            end_portal_sprites: [[0.0; 4]; 2],
            crumbling_sprite: [0.0; 4],
            ticks,
            darkness: (0.0, 0.0),
            night_vision: 0.0,
            block_flicker: 0.0,
            boss_darken: 0.0,
            menu_blur: None,
        };
        self.renderer.render(&frame)
    }
}
