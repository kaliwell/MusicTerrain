//! MusicTerrain's opt-in audio, mapping and terrain kernel.
pub mod audio;
pub mod caves;
pub mod diagnostics;
pub mod dsp;
pub mod ecology;
#[cfg(feature = "native")]
pub mod localization;
pub mod mapping;
#[cfg(feature = "scene")]
pub mod materials;
#[cfg(feature = "scene")]
pub mod mineshaft;
pub mod noise4;
pub mod preset;
#[cfg(feature = "native")]
pub mod preview;
#[cfg(feature = "native")]
pub mod renderer;
pub mod shape;
pub mod terrain;
pub mod transport;
#[cfg(feature = "native")]
pub mod ui;
pub mod underground;
#[cfg(feature = "native")]
mod unicode_font;
#[cfg(feature = "scene")]
pub mod village;
#[cfg(feature = "scene")]
pub mod visual;

#[cfg(feature = "native")]
pub mod capture;

#[cfg(all(feature = "scene", not(feature = "native")))]
mod scene_mesh;
