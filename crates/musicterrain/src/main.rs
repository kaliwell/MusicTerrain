//! Standalone, opt-in entry point. Horizon's vanilla binary is unchanged.
use musicterrain::{
    audio::DecodedAudio,
    dsp::{AnalysisConfig, AudioTimeline},
    mapping::MappingState,
    preset::{VisualizationPreset, builtins},
    terrain::TerrainField,
};
use std::{
    error::Error,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("MusicTerrain: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut log_dir = None;
    while let Some(i) = args.iter().position(|a| a == "--log-dir") {
        if i + 1 >= args.len() {
            return Err("--log-dir needs a directory".into());
        }
        log_dir = Some(PathBuf::from(args.remove(i + 1)));
        args.remove(i);
    }
    musicterrain::diagnostics::init(log_dir.as_deref())?;
    let command = if args.first().is_some_and(|s| !s.starts_with('-')) {
        args.remove(0)
    } else {
        "demo".into()
    };
    if command == "help" || args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "MusicTerrain\n  demo | preview [--language zh-CN|en] [--audio FILE] [--size 64|128|256] [--shape square|cylinder|hexagon|diamond] [--quality full|adaptive] [--orbit] [--orthographic] [--preset-file JSON]\n                 [--biome auto|meadow|forest|alpine|shore|birch-forest|taiga|desert|badlands|savanna|swamp|jungle]\n                 [--mines off|auto|showcase] [--view surface|cutaway|underground]\n                 [--noise perlin|simplex|fbm|ridged|billow|cellular|value|terraced] [--cave-w NUMBER]\n                 [--villages off|auto|showcase] [--resources RESOURCE_DIRECTORY] [--no-vegetation] [--no-water] [--no-caves]\n                 [--silent-preview] [--offscreen --frames N --screenshot PNG]\n  synthesize NEW.wav [--seconds 12]\n  analyze AUDIO --output NEW_DIRECTORY [--seed 42] [--preset 0..6] [--time SECONDS] [--size 64]\n\nAll commands accept --log-dir DIRECTORY for bounded diagnostic logs.\nNative preview: drop WAV/MP3/FLAC, Space play/pause; arrows seek; RMB look; WASD fly; wheel zoom; Settings orbit/projection.\n--silent-preview is an explicit software clock diagnostic and produces no sound."
        );
        return Ok(());
    }
    match command.as_str() {
        "demo" | "preview" => {
            #[cfg(feature = "native")]
            {
                let mut options = musicterrain::preview::PreviewOptions::default();
                let mut i = 0;
                while i < args.len() {
                    match args[i].as_str() {
                        "--language" => {
                            options.language = Some(
                                musicterrain::localization::Language::parse(value(&args, &mut i)?)
                                    .ok_or("--language must be zh-CN or en")?,
                            )
                        }
                        "--audio" => options.audio_path = Some(value(&args, &mut i)?.into()),
                        "--orbit" => options.orbit = true,
                        "--orthographic" => options.orthographic = true,
                        "--noise" => {
                            options.noise = Some(
                                musicterrain::terrain::NoiseType::parse(value(&args, &mut i)?)
                                    .ok_or("Unknown noise type")?,
                            )
                        }
                        "--cave-w" => options.cave_w = Some(value(&args, &mut i)?.parse()?),
                        "--biome" => {
                            let name = value(&args, &mut i)?;
                            options.scene_options.biome = if name == "auto" {
                                None
                            } else {
                                Some(musicterrain::ecology::SceneBiome::parse(name).ok_or("Unknown biome; use auto, meadow, forest, alpine, shore, birch-forest, taiga, desert, badlands, savanna, swamp, jungle")?)
                            };
                        }
                        "--shape" => {
                            options.scene_options.shape =
                                musicterrain::shape::TerrainShape::parse(value(&args, &mut i)?)
                                    .ok_or("--shape must be square, cylinder, hexagon or diamond")?
                        }
                        "--quality" => {
                            options.scene_options.full_detail = match value(&args, &mut i)? {
                                "full" => true,
                                "adaptive" => false,
                                _ => return Err("--quality must be full or adaptive".into()),
                            }
                        }
                        "--resources" => options.resources = Some(value(&args, &mut i)?.into()),
                        "--mines" => {
                            options.scene_options.mines =
                                musicterrain::mineshaft::MineMode::parse(value(&args, &mut i)?)
                                    .ok_or("--mines must be off, auto or showcase")?;
                        }
                        "--view" => {
                            options.scene_options.underground_view =
                                musicterrain::mineshaft::UndergroundView::parse(value(
                                    &args, &mut i,
                                )?)
                                .ok_or("--view must be surface, cutaway or underground")?;
                        }
                        "--villages" => {
                            options.scene_options.villages =
                                musicterrain::village::VillageMode::parse(value(&args, &mut i)?)
                                    .ok_or("--villages must be off, auto or showcase")?;
                        }
                        "--no-vegetation" => options.scene_options.vegetation = false,
                        "--no-water" => options.scene_options.water = false,
                        "--no-caves" => options.scene_options.caves = false,
                        "--preset-file" => options.preset_path = value(&args, &mut i)?.into(),
                        "--size" => options.size = value(&args, &mut i)?.parse()?,
                        "--frames" => {
                            let n = value(&args, &mut i)?.parse()?;
                            if n < 2 {
                                return Err("--frames must be at least 2".into());
                            }
                            options.smoke_frames = Some(n);
                        }
                        "--screenshot" => options.screenshot = Some(value(&args, &mut i)?.into()),
                        "--silent-preview" => options.silent_preview = true,
                        "--offscreen" => options.offscreen = true,
                        other => return Err(format!("Unknown preview option {other}").into()),
                    }
                    i += 1;
                }
                if ![64, 128, 256].contains(&options.size) {
                    return Err("Preview size must be 64,128,256".into());
                }
                musicterrain::preview::run(options)?;
            }
            #[cfg(not(feature = "native"))]
            return Err("Rebuild with default features for native preview".into());
        }
        "synthesize" => {
            let path = PathBuf::from(args.first().ok_or("synthesize requires a NEW.wav path")?);
            let mut seconds = 12.0;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--seconds" => seconds = value(&args, &mut i)?.parse()?,
                    a => return Err(format!("Unknown option {a}").into()),
                }
                i += 1;
            }
            synthesize(&path, seconds)?;
            println!("Created {} seconds of synthetic test audio", seconds);
        }
        "analyze" => {
            let path = PathBuf::from(args.first().ok_or("analyze requires an audio path")?);
            let mut output = None;
            let mut seed = 42;
            let mut preset_index = 0usize;
            let mut time = 0.0f64;
            let mut size = 64u32;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--output" => output = Some(PathBuf::from(value(&args, &mut i)?)),
                    "--seed" => seed = value(&args, &mut i)?.parse()?,
                    "--preset" => preset_index = value(&args, &mut i)?.parse()?,
                    "--time" => time = value(&args, &mut i)?.parse()?,
                    "--size" => size = value(&args, &mut i)?.parse()?,
                    a => return Err(format!("Unknown option {a}").into()),
                }
                i += 1;
            }
            let output = output.ok_or("--output must name a new directory")?;
            if !time.is_finite() || time < 0. || !(1..=1024).contains(&size) {
                return Err("time must be finite >=0, size must be1..1024".into());
            }
            let mut preset = builtins()
                .get(preset_index)
                .cloned()
                .ok_or("preset index must be 0..6")?;
            preset.seed = seed;
            analyze(&path, &output, &preset, time, size)?;
        }
        other => return Err(format!("Unknown command {other}; use help").into()),
    }
    Ok(())
}
fn value<'a>(args: &'a [String], i: &mut usize) -> Result<&'a str, Box<dyn Error>> {
    *i += 1;
    Ok(args.get(*i).ok_or("Missing option value")?.as_str())
}
fn create(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}
fn synthesize(path: &Path, seconds: f64) -> Result<(), Box<dyn Error>> {
    if !seconds.is_finite() || !(0.1..=120.).contains(&seconds) {
        return Err("seconds must be between0.1 and120".into());
    }
    let frames = (seconds * 48_000.) as u32;
    let bytes = frames * 2;
    let mut out = create(path)?;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + bytes).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&48_000u32.to_le_bytes())?;
    out.write_all(&96_000u32.to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for i in 0..frames {
        let t = f64::from(i) / 48_000.;
        let envelope = (-12. * (t * 2.).fract()).exp();
        let melody = if t % 4. < 2. { 440. } else { 1000. };
        let sample = envelope * 0.48 * (std::f64::consts::TAU * 60. * t).sin()
            + 0.15 * (std::f64::consts::TAU * 120. * t).sin()
            + 0.12 * (std::f64::consts::TAU * melody * t).sin()
            + envelope * 0.04 * (std::f64::consts::TAU * 4000. * t).sin();
        out.write_all(&((sample * i16::MAX as f64) as i16).to_le_bytes())?;
    }
    out.sync_all()?;
    Ok(())
}
fn analyze(
    path: &Path,
    output: &Path,
    preset: &VisualizationPreset,
    time: f64,
    size: u32,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let audio = DecodedAudio::decode(path)?;
    let config = AnalysisConfig::default();
    let timeline = AudioTimeline::analyze(&audio, &config)?;
    let analysis_ms = started.elapsed().as_secs_f64() * 1000.;
    let mut state = MappingState::default();
    let mut parameters = preset.parameters.clone();
    let target = timeline.at(time).time;
    for features in &timeline.frames {
        if features.time > target {
            break;
        }
        parameters = preset.mappings.evaluate(
            features,
            &preset.parameters,
            config.hop_size as f32 / audio.sample_rate as f32,
            &mut state,
        )?;
    }
    let field = TerrainField::new(preset.seed);
    let generation = Instant::now();
    let origin = [-(size as i32) / 2; 2];
    let heights = field.heightmap(origin, [size; 2], &parameters)?;
    let generation_ms = generation.elapsed().as_secs_f64() * 1000.;
    // Exclusive directory creation protects existing exports; marker identifies interrupted exports.
    std::fs::create_dir(output)?;
    create(&output.join("INCOMPLETE"))?;
    let mut pgm = create(&output.join("heightmap.pgm"))?;
    write!(pgm, "P5\n{size} {size}\n65535\n")?;
    for &height in &heights {
        let value = ((height + 256.).clamp(0., 768.) / 768. * 65535.).round() as u16;
        pgm.write_all(&value.to_be_bytes())?;
    }
    pgm.sync_all()?;
    let mut csv = create(&output.join("heightmap.csv"))?;
    writeln!(csv, "x,z,height")?;
    for (i, h) in heights.iter().enumerate() {
        writeln!(
            csv,
            "{},{},{:.9}",
            origin[0] + (i % size as usize) as i32,
            origin[1] + (i / size as usize) as i32,
            h
        )?;
    }
    serde_json::to_writer(create(&output.join("features.json"))?, &timeline)?;
    preset.save(output.join("preset.json"))?;
    let metadata = serde_json::json!({"format":"MusicTerrain analysis export (not a Minecraft save)","generator_version":1,"audio_content_hash":audio.content_hash,"preset_hash":preset.hash()?,"seed":preset.seed,"sample_rate":audio.sample_rate,"duration":audio.duration(),"audio_time":target,"size":size,"origin":origin,"analysis_ms":analysis_ms,"heightmap_ms":generation_ms});
    serde_json::to_writer_pretty(create(&output.join("metadata.json"))?, &metadata)?;
    // Persist every new artifact before marking this analysis export complete.
    drop(csv);
    drop(pgm);
    for entry in std::fs::read_dir(output)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::OpenOptions::new()
                .write(true)
                .open(entry.path())?
                .sync_all()?;
        }
    }
    std::fs::remove_file(output.join("INCOMPLETE"))?;
    println!(
        "Analyzed {:.3}s, {} FFT frames; analysis {:.1}ms, heightmap {:.1}ms; {}x{} deterministic heightmap exported",
        audio.duration(),
        timeline.frames.len(),
        analysis_ms,
        generation_ms,
        size,
        size
    );
    Ok(())
}
