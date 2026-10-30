//! Rasterize locally installed Unicode fonts once per glyph. No system font is redistributed.
use ab_glyph::{Font, FontArc, ScaleFont};
use horizon_client_render::hud::HudBatch;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

fn font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        let windows = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:/Windows".into());
        let mut paths = Vec::new();
        if let Some(path) = std::env::var_os("MUSICTERRAIN_CJK_FONT") {
            paths.push(path.into());
        }
        paths.extend([
            windows.join("Fonts/msyh.ttc"),
            windows.join("Fonts/simhei.ttf"),
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
            "/System/Library/Fonts/PingFang.ttc".into(),
        ]);
        paths.into_iter().find_map(|path| {
            let font = FontArc::try_from_vec(std::fs::read(path).ok()?).ok()?;
            (font.glyph_id('中').0 != 0).then_some(font)
        })
    })
    .as_ref()
}

pub fn available() -> bool {
    font().is_some()
}

#[derive(Default)]
struct Raster {
    spans: Vec<(f32, f32, f32, u8)>,
}

fn raster(ch: char) -> Raster {
    let mut result = Raster::default();
    let Some(font) = font() else { return result };
    let scaled = font.as_scaled(17.0);
    let mut glyph = scaled.scaled_glyph(ch);
    glyph.position = ab_glyph::point(0.0, 12.0);
    if let Some(outline) = font.outline_glyph(glyph) {
        let bounds = outline.px_bounds();
        let width = bounds.width() as usize;
        let height = bounds.height() as usize;
        let mut pixels = vec![0u8; width * height];
        outline.draw(|x, y, coverage| {
            pixels[y as usize * width + x as usize] = (coverage * 15.0).round() as u8 * 17
        });
        for y in 0..height {
            let mut x = 0;
            while x < width {
                let alpha = pixels[y * width + x];
                let start = x;
                x += 1;
                while x < width && pixels[y * width + x] == alpha {
                    x += 1;
                }
                if alpha > 0 {
                    result.spans.push((
                        bounds.min.x + start as f32,
                        bounds.min.y + y as f32,
                        (x - start) as f32,
                        alpha,
                    ));
                }
            }
        }
    }
    result
}

pub fn draw(h: &mut HudBatch, ch: char, x: f32, y: f32, color: u32, scale: f32) {
    static CACHE: OnceLock<Mutex<HashMap<char, Raster>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let glyph = cache.entry(ch).or_insert_with(|| raster(ch));
    for &(dx, dy, width, alpha) in &glyph.spans {
        let opacity = ((color >> 24) * u32::from(alpha)) / 255;
        crate::ui::rect(
            h,
            [x + dx * scale, y + dy * scale, width * scale, scale],
            (color & 0xffffff) | (opacity << 24),
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn installed_font_renders_distinct_chinese_glyphs() {
        if !super::available() {
            return;
        }
        let a = super::raster('中');
        let b = super::raster('文');
        assert!(!a.spans.is_empty());
        assert_ne!(a.spans, b.spans);
    }
}
