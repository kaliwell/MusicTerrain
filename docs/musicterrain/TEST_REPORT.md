# MusicTerrain test report

Implementation began 2026-10-29; final verification continued 2026-10-30 (UTC).
Baseline: `62a23ba0258a89e7a69b513f333c6c19d05e6594`.
Branch: `feature/music-terrain`. Code was subsequently uploaded to the user-owned
`kaliwell/MusicTerrain` repository with authorization; no upstream PR was created.

## Environment

Rust 1.99.1; Debian 13 x86_64; a two-CPU cgroup; Xvfb 1280×800;
Mesa 25.0.7 llvmpipe (LLVM 19.1.7), Vulkan, CPU device. No `/dev/dri` hardware
GPU or physical sound device was used. Native runs explicitly selected the
silent diagnostic clock and fresh data homes, with `HORIZON_NO_AUDIO=1`.
JDK 21 is present; Horizon's vanilla harnesses require JDK 25 and prepared 26.3
Minecraft resources, which are absent.

## Executed checks

| Command | Actual result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy -p musicterrain --all-targets -- -D warnings` | Passed |
| `cargo test -p musicterrain` | 42 passed: 8 unit, 9 audio, 5 CLI, 13 terrain/mapping, 7 mesh; none ignored |
| `cargo build -p musicterrain` | Passed; native executable at `target/debug/musicterrain` |
| `cargo test -p musicterrain --no-default-features` | 33 passed; independent audio/terrain/CLI build verified |
| `cargo test -p horizon-core --lib` | 107 passed; 2 existing microbenchmarks ignored |
| `python3 -m unittest discover -s tests` | 416 passed in 102.202 s |
| `python3 tools/check_public_tree.py` | Passed |
| `git diff --check` | Passed |

Builds used `PKG_CONFIG_PATH=/workspace/scratch/pkgconfig` and
`LIBRARY_PATH=/workspace/scratch/sysroot/usr/lib/x86_64-linux-gnu` for the
locally extracted ALSA development package. Core/MusicTerrain dev packages use
opt-level 2; the renderer remains the normal dev build. No tests were deleted
or disabled to obtain these results. Raw final Cargo evidence is in
[verification/](verification/).

## Coverage

- Real stereo WAV decoding/downmix/content hashing; 44.1→48 kHz resampling,
  alias suppression, invalid PCM/configuration and cancellation.
- Synthetic 40, 60, 120, 440, 1000, 4000 Hz tones; silence, noise, sweep,
  impulse/regular beats, 8192 FFT precision, RMS, band energy and finite features.
- Fixed-hop exact timestamp lookup, seek order independence, deterministic
  attack/release replay, ordered Replace/Add/Multiply and malformed rules.
- Seed repeatability/difference; world-coordinate tile edges and generation order;
  finite extremes, actual cave/island density and valid seven presets/migrations.
- Shipped binary: synthesize WAV → decode/FFT/map → heightmap/features export;
  repeated outputs identical, correct CSV world coordinates, changed seed/time,
  corrupt imports, malformed CLI and preservation of existing files/directories.
- Mesh face winding, hidden face removal, section-local coordinates, deterministic
  vertices, changing real geometry, LOD limits, bounded queue, obsolete revision
  rejection, static cache and clean worker shutdown.
- Audio callback sample copying/resampling/channel fill; command epochs ensure an
  in-flight callback cannot undo pause or even a seek to the same position.
- Diagnostic events, concurrent records, rotation/retention, invalid destinations
  and preservation of unrelated files.

After reducing codec dependencies to MP3/WAV/PCM/FLAC/Ogg/Vorbis, the final binary
also imported real temporary MP3 and FLAC files encoded by ffmpeg from a 120 Hz,
44.1 kHz synthetic tone. Both decoded to 1.000 s / 48 kHz and produced 94 analysis
frames. The independent DSP probe measured centroid 120 Hz and dominant B2.
No personal audio or generated audio fixture is included in the repository.

## Real renderer and editor acceptance

`tools/musicterrain_smoke.py` launches the actual native binary, uses Horizon's
production renderer, requests a real PNG readback, samples process peak RSS and
requires distinct uploaded meshes after audio import. It is not a simulated UI
or image generator. The adapter is logged by the application itself.

64², 128² and 256² extents all rendered and exited successfully. Detailed frame,
mesh, memory and queue data—including slow 256² results—are in
[PERFORMANCE.md](PERFORMANCE.md). The 64² measured runs uploaded 42 and 58 distinct
meshes across 121 frames each; snapshot counts are capped at 32 inside the app, while
the harness counts hashes from all upload records.

`tools/musicterrain_interaction.py` drives real X11 mouse/key events. It verified:

- Pausing stops subsequent mesh rebuilding after queued work drains.
- Seek changes audio time while paused; manual parameter edits, Undo and Redo work.
- Preset Save creates a real file; a second Save preserves it; Load succeeds.
- Wave Terrain and Floating Islands produce visibly different geometry.
- Noise Lab, Mapping Editor and Diagnostics render and receive navigation events.
- Escape exits and resources/threads are released; exit code 0.

The native editor event log records 15 real actions. Screenshots include
[demo](images/demo.png), [Noise Lab](images/noise-lab.png) and
[floating islands](images/floating-islands.png).

## Failures diagnosed and fixed

Initial checks exposed a transient onset threshold that missed a single-sample
impulse, floating-point exact-frame lookup rounding, MP3 encoder padding,
incorrect CSV world-coordinate labels, stale callback cursor commits during
pause/seek, absent shutdown cancellation and Clippy warnings. Each affected
code path was corrected and covered or rechecked. The temporary sysroot's ALSA
linker symlink initially pointed to an absent relative target; local environment
setup was repaired without changing system directories. An initial interaction
harness clicked Play instead of Save; its coordinate was corrected before the
successful acceptance run.

## Not established

- Physical-device audible output, measured end-to-end audio/display latency,
  device capture/switching, Windows WASAPI and Windows/RTX 3060 performance.
- S3 game menu/lifecycle, resource-pack materials and integrated-server wiring.
- Minecraft world saves/reopen, vanilla lighting/heightmaps and deterministic
  event structures. Heightmap JSON/PGM exports are not Minecraft saves.
- Full workspace `pr_ready.py`, Java differential harnesses and all gameplay
  integration checks. Relevant Cargo checks, core regression and Python checks
  above passed; unavailable prepared resources are not counted as passing tests.
- Long-duration leak testing or 1080p 60 FPS. Software Vulkan measurements have
  their own explicit limits in PERFORMANCE.md.

## Textured scene update — 2026-10-30 UTC

The scene update adds 17 original pixel-art material layers, local PNG loading,
seeded ecology, grass/dirt/stone/sand/snow surfaces, real oak trunk/leaf blocks,
cutout grass/flowers, pond water, ore materials and actual carved cave voxels.
The production Horizon terrain, translucency, sky and cloud pipelines remain in use.

Final checks for this revision:

| Check | Result |
| --- | --- |
| `cargo test -p musicterrain` | 60 passed, none ignored |
| `cargo test -p musicterrain --no-default-features` | 40 passed |
| `cargo clippy -p musicterrain --all-targets -- -D warnings` | Passed |
| `cargo fmt --all --check` | Passed |
| `cargo build -p musicterrain` | Passed |
| Real renderer smoke with `--require-scene` | Exit 0; 91 frames, 43 distinct uploaded meshes, PNG readback |
| X11 interaction with `--scene` | Exit 0; 22 events; actual geometry switches, pause/seek/history/save/load verified |
| Local resource directory render | Exit 0; actual one local PNG layer + 16 fallback layers, screenshot generated |

The 18 added tests consist of seven ecology, six material and five scene-mesh
tests. The original flat-mesh test still verifies complete exterior area and
outward winding; material boundaries legitimately subdivide faces. No prior
coverage was removed. At the exact default unmodulated snapshot, the new scene
contains 10 trees, 76 plants, 2675 carved cells, 3215 cave quads and 266 water quads;
counts change with the playing music and are logged from actual generation.

Scene tests assert real material-layer use, translucent water batches, cutout
alpha, bounded section-local positions/UVs, foliage/trunk geometry, valid terrain
support, deterministic world-grid identities, matching tiled decoration queries,
carved voids, scene-switch cache identity and preserved 3D density topology.
An early ecology test caught mirrored-position event-ID collisions; the keyed
coordinate packing was corrected and all seven ecology tests then passed.
Rust 1.99 Clippy slice/checked-division warnings were fixed before final checking.
The first renderer attempt found a stale Xvfb socket; restarting on fresh display
`:100` resolved that environmental failure.

Raw evidence is in [verification/scene](verification/scene/). Actual screenshots:
[textured scene](images/scene.png), [scene settings](images/scene-settings.png),
[disabled geometry](images/scene-disabled.png), [Noise Lab](images/scene-noise-lab.png)
and [islands](images/scene-islands.png).

These checks use the explicit silent diagnostic clock and CPU software Vulkan.
They do not establish audible hardware output or target-GPU/Windows performance.
Material volumes/decorations remain preview-only; the heightmap analysis export
is unchanged and no Minecraft world persistence/collision/fluid simulation is claimed.

## Biome update verification — 2026-10-30 UTC

- `cargo test -p musicterrain`: **63 passed**, none ignored: 8 unit, 9 audio,
  5 CLI, 9 ecology, 6 materials, 6 scene mesh and 7 visual plus 13 terrain.
- `cargo test -p musicterrain --no-default-features`: **42 passed**.
- `cargo clippy -p musicterrain --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo build -p musicterrain`, public-tree scan
  and `git diff --check`: passed.
- Added real tests for all lowland climate types, six species/soil contracts,
  per-biome tile/halo equality, actual mesh material layers and reproducible hashes.
  The local-pack fixture now checks fallback counts against the expanded atlas;
  the original IDs and image/alpha checks are retained.
- Actual X11 editor interaction with `--scene --biomes`: exit 0, 34 editor
  events, 37 distinct uploaded meshes. Clicked all ten biomes and restored Auto,
  captured each, verified scene switches, pause, seek, undo/redo and save/load.
- Actual CLI `--biome desert`: 30-frame offscreen run and PNG succeeded;
  invalid biome rejected before creating a window. Same isolated homes,
  explicit silent diagnostic clock and CPU Vulkan environment as above.
- [Screenshots and biome contracts](BIOMES.md). Raw output:
  [verification/biomes](verification/biomes/). No hardware GPU/audio, Windows,
  collision or persistent-world functionality is claimed by these checks.

## Tropical jungle verification — 2026-10-30 UTC

- `cargo test -p musicterrain`: **65 passed**, none ignored (added jungle density,
  trunk-height and vegetation-switch tests). Existing automatic climate,
  whole/tiled layout, mesh material and alpha tests now cover Jungle as well.
- `cargo test -p musicterrain --no-default-features`: **43 passed**.
- Strict all-target Clippy, workspace format check and native build: passed.
  Public-tree scan and `git diff --check`: passed.
- Actual X11 editor `--scene --biomes`: exit 0, 35 editor events, 37 distinct
  uploaded meshes; all eleven biomes and return to Auto were exercised.
- Actual `--biome jungle --silent-preview --offscreen --frames 30` completed
  and wrote a PNG; invalid biome names still fail before window creation.
- [Actual Jungle screenshot](images/biomes/biome-jungle.png). Captured with fresh
  data homes, silent diagnostic clock and Xvfb/software Vulkan. Hardware and
  Windows performance were not measured; vines have no climbing collision.
- Raw evidence: [verification/jungle](verification/jungle/).
