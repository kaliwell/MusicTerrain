//! Executes the shipped Rust binary against fresh, test-owned data directories.
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use musicterrain::{
    dsp::AudioTimeline, mapping::MappingState, preset::builtins, terrain::TerrainField,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    root: PathBuf,
}
impl Scratch {
    fn new() -> Self {
        loop {
            let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "musicterrain-cli-test-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create isolated test directory: {e}"),
            }
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_musicterrain"));
        command
            .env("HORIZON_HOME", self.path("game-home"))
            .env("HORIZON_LAUNCHER_HOME", self.path("launcher-home"))
            .env("XDG_DATA_HOME", self.path("data-home"))
            .env("XDG_CONFIG_HOME", self.path("config-home"))
            .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
            .env("HORIZON_NO_AUDIO", "1")
            .env("RUST_LOG", "off");
        command
    }
    fn synthesize(&self, path: &Path) -> Output {
        self.command()
            .arg("synthesize")
            .arg(path)
            .args(["--seconds", "1.2"])
            .output()
            .expect("run synthesize")
    }
    fn analyze(&self, audio: &Path, output: &Path, seed: &str, time: &str) -> Output {
        self.command()
            .arg("analyze")
            .arg(audio)
            .arg("--output")
            .arg(output)
            .args(["--seed", seed, "--time", time, "--size", "16"])
            .output()
            .expect("run analyze")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        // This path was created exclusively by this fixture, never a user-supplied path.
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn failure(output: &Output) -> String {
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        error.contains("MusicTerrain:"),
        "missing error context: {error}"
    );
    assert!(!error.contains("panicked"), "CLI panicked: {error}");
    error
}

#[test]
fn audio_to_heightmap_exports_are_reproducible_with_correct_world_coordinates() {
    let scratch = Scratch::new();
    let audio = scratch.path("music.wav");
    success(&scratch.synthesize(&audio));
    let first = scratch.path("first");
    let second = scratch.path("second");
    success(&scratch.analyze(&audio, &first, "42", "0.4"));
    success(&scratch.analyze(&audio, &second, "42", "0.4"));
    for name in [
        "heightmap.pgm",
        "heightmap.csv",
        "features.json",
        "preset.json",
    ] {
        assert_eq!(
            fs::read(first.join(name)).unwrap(),
            fs::read(second.join(name)).unwrap(),
            "nondeterministic {name}"
        );
    }
    assert!(!first.join("INCOMPLETE").exists());
    let timeline: AudioTimeline =
        serde_json::from_slice(&fs::read(first.join("features.json")).unwrap()).unwrap();
    let preset = builtins().remove(0);
    let mut state = MappingState::default();
    let mut parameters = preset.parameters.clone();
    let target = timeline.at(0.4).time;
    for frame in &timeline.frames {
        if frame.time > target {
            break;
        }
        parameters = preset
            .mappings
            .evaluate(
                frame,
                &preset.parameters,
                timeline.hop_size as f32 / timeline.sample_rate as f32,
                &mut state,
            )
            .unwrap();
    }
    let field = TerrainField::new(42);
    let csv = fs::read_to_string(first.join("heightmap.csv")).unwrap();
    assert_eq!(csv.lines().next(), Some("x,z,height"));
    let rows: Vec<_> = csv.lines().skip(1).collect();
    assert_eq!(rows.len(), 16 * 16);
    for (index, row) in rows.iter().enumerate() {
        let cells: Vec<_> = row.split(',').collect();
        assert_eq!(cells.len(), 3);
        let x = cells[0].parse::<i32>().unwrap();
        let z = cells[1].parse::<i32>().unwrap();
        assert_eq!(x, index as i32 % 16 - 8);
        assert_eq!(z, index as i32 / 16 - 8);
        let height = cells[2].parse::<f32>().unwrap();
        assert!((height - field.height(f64::from(x), f64::from(z), &parameters)).abs() <= 1e-6);
    }
}

#[test]
fn existing_audio_and_export_directories_are_preserved() {
    let scratch = Scratch::new();
    let audio = scratch.path("existing.wav");
    success(&scratch.synthesize(&audio));
    let original = fs::read(&audio).unwrap();
    failure(&scratch.synthesize(&audio));
    assert_eq!(fs::read(&audio).unwrap(), original);
    let output = scratch.path("existing-output");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("user-data"), b"preserve this").unwrap();
    failure(&scratch.analyze(&audio, &output, "42", "0.2"));
    assert_eq!(
        fs::read(output.join("user-data")).unwrap(),
        b"preserve this"
    );
    assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
    let generated = scratch.path("generated");
    success(&scratch.analyze(&audio, &generated, "42", "0.2"));
    let heights = fs::read(generated.join("heightmap.pgm")).unwrap();
    failure(&scratch.analyze(&audio, &generated, "777", "0.7"));
    assert_eq!(fs::read(generated.join("heightmap.pgm")).unwrap(), heights);
    assert!(!generated.join("INCOMPLETE").exists());
}

#[test]
fn corrupt_audio_fails_with_context_before_creating_an_export() {
    let scratch = Scratch::new();
    let audio = scratch.path("broken.wav");
    fs::write(&audio, b"this is not a playable audio file").unwrap();
    let output = scratch.path("never-created");
    let error = failure(&scratch.analyze(&audio, &output, "42", "0"));
    assert!(error.to_lowercase().contains("audio"), "{error}");
    assert!(!output.exists());
}

#[test]
fn malformed_cli_options_fail_without_creating_outputs() {
    let scratch = Scratch::new();
    for arguments in [
        vec!["unknown-command"],
        vec!["synthesize"],
        vec!["synthesize", "ignored.wav", "--seconds", "NaN"],
        vec!["synthesize", "ignored.wav", "--unknown"],
        vec!["analyze"],
        vec!["analyze", "ignored.wav", "--output"],
        vec![
            "analyze",
            "ignored.wav",
            "--output",
            "ignored-output",
            "--time",
            "NaN",
        ],
        vec![
            "analyze",
            "ignored.wav",
            "--output",
            "ignored-output",
            "--size",
            "0",
        ],
        vec![
            "analyze",
            "ignored.wav",
            "--output",
            "ignored-output",
            "--seed",
            "abc",
        ],
        vec![
            "analyze",
            "ignored.wav",
            "--output",
            "ignored-output",
            "--preset",
            "99",
        ],
        vec!["analyze", "ignored.wav", "--unknown"],
    ] {
        let result = scratch
            .command()
            .current_dir(&scratch.root)
            .args(arguments.iter().map(OsStr::new))
            .output()
            .unwrap();
        failure(&result);
    }
    assert_eq!(fs::read_dir(&scratch.root).unwrap().count(), 0);
}

#[test]
fn seed_and_audio_time_change_terrain_but_not_extracted_audio_features() {
    let scratch = Scratch::new();
    let audio = scratch.path("music.wav");
    success(&scratch.synthesize(&audio));
    let base = scratch.path("base");
    let seed = scratch.path("new-seed");
    let time = scratch.path("new-time");
    success(&scratch.analyze(&audio, &base, "42", "0"));
    success(&scratch.analyze(&audio, &seed, "43", "0"));
    success(&scratch.analyze(&audio, &time, "42", "0.7"));
    let initial = fs::read(base.join("heightmap.csv")).unwrap();
    assert_ne!(initial, fs::read(seed.join("heightmap.csv")).unwrap());
    assert_ne!(initial, fs::read(time.join("heightmap.csv")).unwrap());
    let features = fs::read(base.join("features.json")).unwrap();
    assert_eq!(features, fs::read(seed.join("features.json")).unwrap());
    assert_eq!(features, fs::read(time.join("features.json")).unwrap());
}
