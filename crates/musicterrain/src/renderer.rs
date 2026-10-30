//! The MusicTerrain preview uses Horizon's production section renderer.
//! All supplied pixels are procedural; this path needs no proprietary game assets.
use glam::{DVec3, Mat4};
use horizon_client_render::{
    environment::Sky,
    gfx::{Frame, Renderer, pack_uniforms::PackInputs, panorama::Faces},
    graphics::GraphicsApi,
    hud::HudBatch,
    weather,
};
use std::{error::Error, sync::Arc};
use winit::window::Window;

pub struct PreviewRenderer {
    pub renderer: Renderer,
}

impl PreviewRenderer {
    pub fn new(window: Arc<Window>, offscreen: bool) -> Result<Self, Box<dyn Error>> {
        // A pale stone pattern carries vertex tints without requiring resource packs.
        let mut pixels = Vec::with_capacity(16 * 16 * 4);
        for z in 0..16u32 {
            for x in 0..16u32 {
                let shade = 216 + ((x * 13 + z * 7 + (x ^ z) * 3) % 32) as u8;
                pixels.extend_from_slice(&[shade, shade, shade, 255]);
            }
        }
        let white = [255u8; 4];
        let panorama = Faces {
            size: 1,
            pixels: [20, 25, 38, 255].repeat(6),
        };
        let mut renderer = pollster::block_on(Renderer::new(
            window,
            offscreen,
            &pixels,
            16,
            16,
            1,
            &[],
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
        Ok(Self { renderer })
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
                sky_color: [0.035, 0.045, 0.075],
                fog_color: [0.035, 0.045, 0.075],
                sky_light_factor: 1.0,
                sky_light_color: [1.0; 3],
                block_light_tint: [1.0; 3],
                ambient_light_color: [0.1; 3],
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
            sky_blocked: true,
            skybox: false,
            end_sky: false,
            end_sky_sprite: None,
            environment_fog: None,
            water_fog: [0.035, 0.045, 0.075],
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
            clouds: None,
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
