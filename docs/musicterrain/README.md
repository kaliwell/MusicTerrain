# MusicTerrain × Horizon

Opt-in Rust music terrain editor built in the actual Horizon Cargo workspace.
It reuses Horizon's seeded Perlin/Simplex noise, section vertex format, production
wgpu renderer, GPU arenas, camera math and HUD. Vanilla gameplay and saves are untouched.

[Textured world screenshot](images/scene.png) · [Noise Lab](images/scene-noise-lab.png) ·
[Floating Islands](images/scene-islands.png)

## Run the demo

The manifests declare Rust **1.90+**; the tested toolchain is stable **1.99.1**. Linux also needs ALSA development headers and a
working Vulkan/OpenGL display driver (`libasound2-dev` on Debian). Windows uses
cpal's native audio backend and Horizon's existing graphics backends.

From the repository root:

```sh
cargo run -p musicterrain -- demo
cargo run -p musicterrain -- preview --audio /path/to/song.wav
```

Without a file it synthesizes a labeled 12-second beat/melody sample, analyzes it
with the real FFT and plays that PCM. Drop WAV, MP3 or FLAC onto the window to
import. Import and analysis run on a bounded worker, never on the audio callback.

- Space: play/pause; Left/Right: seek five seconds; click/drag the timeline to seek.
- Hold right mouse and move to look; W/A/S/D move, Q/E descend/ascend.
- Left navigation opens Live, Music World status, Noise Lab, mappings, presets,
  settings and diagnostics. Music World export/capture show their true unavailable status.
- Inspector sliders edit the live parameters; mouse wheel scrolls inspector/rules.
  Noise type, octave count and Seed buttons cycle/increment values.
- Noise Lab displays the same field's heightmap, density slice and cross section;
  its Music Overlay toggle combines manual parameters with song mappings.
- Mapping editor changes source, target, ranges, curve, gain, bias, attack/release,
  combination, enable, copy and delete. Rules run in their displayed order.
- Presets provides seven different built-ins, Reset, Undo and Redo. Save/Load use
  `musicterrain-preset.json` or `--preset-file FILE`. Save creates a new file and
  reports an error if it exists; choose a new path to preserve an earlier preset.
- Settings controls spectrum, audio/visual offset and preview extent. Larger
  extents reduce voxel detail to retain a finite mesh budget.
- Diagnostics reports actual worker, renderer and upload metrics. GPU copy
  submission time is distinguished from GPU execution time.

File logs are optional and bounded (four files, 2 MiB each):

```sh
cargo run -p musicterrain -- demo --log-dir /path/to/musicterrain-logs
```

No audio hardware: normal mode reports an error and pauses. To explicitly test
animation without sound, use `--silent-preview` (labeled software clock):

```sh
cargo run -p musicterrain -- demo --silent-preview
```


## Textured world preview

The default preview now includes grass/dirt/stone layers, oak trees, cutout grass
and flowers, sand banks, translucent pond water, coal/iron ore, gravel/clay and
actual carved cave entrances/walls. Alpine terrain uses snow. Daylight sky and
block clouds use Horizon's existing renderer. All 54 built-in texture layers are
original generated pixel art; no Minecraft texture is bundled or downloaded.

Eleven seeded biomes are available: meadow, forest, alpine, shore, birch forest,
taiga, desert, badlands, savanna, swamp and tropical jungle. Jungle adds
ordinary and giant trees, thick trunks, rounded crowns, hanging vines, bamboo,
bushes, melons, cocoa and ferns. Settings cycles Auto / each biome;
`--biome desert` selects one at launch. [Biome comparison and details](BIOMES.md).

Village Auto / Showcase / Off is available in Settings or `--villages`.
`--biome meadow --villages showcase` opens the origin village preview.
See [rich ecology and village details](WORLD_DETAILS.md).

Trees/Plants, Water and Caves are independent controls on Settings. They rebuild
the actual geometry and invalidate old-generation results. Equivalent CLI flags:

```sh
cargo run -p musicterrain -- demo --no-vegetation --no-water --no-caves
cargo run -p musicterrain -- demo --resources /path/to/resource-pack
```

Resource directories can be a pack/prepared root containing
`assets/minecraft/textures/block`, a `textures` root or the `block` directory.
Known missing PNGs retain original fallback textures; corrupt files produce an
error. The UI shows how many layers came from the local pack. Only the first
square animation frame is currently used. ZIP import/custom models are not
implemented. See [MATERIALS](MATERIALS.md), [ECOLOGY](ECOLOGY.md) and
[SCENE_GENERATION](SCENE_GENERATION.md) for the actual data path and limits.

The preview is a finite visual scene; it does not yet add gameplay collisions,
fluid simulation or a Minecraft save. The analysis CLI exports the original
core height field; its files do not contain these preview decorations.

## Deterministic analysis and heightmaps

This is an analysis export, **not a Minecraft world save**. Destinations must be new.

```sh
cargo run -p musicterrain --no-default-features -- synthesize /tmp/test.wav
cargo run -p musicterrain --no-default-features -- analyze /tmp/test.wav --output /tmp/terrain-new --seed 42 --preset 0 --time 1.5 --size 64
```

Output: raw 16-bit PGM (`[-256,512]` height range mapped to `[0,65535]`), world-coordinate
CSV heights, feature timeline JSON, validated preset JSON and metadata containing
content/preset SHA-256, seed, generator version and timing. An interrupted export
retains an `INCOMPLETE` marker. Existing directories/files are never overwritten.
Fixed-hop replay makes mapping state independent of rendering frame rate or seek order.

## Verification and current scope

```sh
cargo fmt --check -p musicterrain
cargo clippy -p musicterrain --all-targets -- -D warnings
cargo test -p musicterrain
cargo build -p musicterrain
cargo test -p musicterrain --no-default-features
```

For automated real-renderer screenshots on a machine with a display (Xvfb works):

```sh
cargo run -p musicterrain -- demo --silent-preview --offscreen --frames 30 --screenshot /tmp/musicterrain.png
```

The smoke exits successfully only after multiple distinct meshes reached the
renderer. See [TEST_REPORT](TEST_REPORT.md) for actual execution and evidence,
[KNOWN_ISSUES](KNOWN_ISSUES.md) for limitations and [TASKS](TASKS.md) for stage status.
Hardware audio, Windows capture, ordinary Horizon title integration and Minecraft
world generation/export remain separate acceptance gates.

## This cloud workspace

This container initially lacked Rust, ALSA headers, a display and a GPU. Rust is
installed under the normal rustup home. Build headers, Xvfb, xdotool and Mesa
were extracted without root into `/workspace/scratch/sysroot`; this is local
validation setup, not a shipped runtime dependency.

```sh
. "$HOME/.cargo/env"
export PKG_CONFIG_PATH=/workspace/scratch/pkgconfig
export LIBRARY_PATH=/workspace/scratch/sysroot/usr/lib/x86_64-linux-gnu
export LD_LIBRARY_PATH=/workspace/scratch/sysroot/usr/lib/x86_64-linux-gnu
cargo build -p musicterrain
```

For this session's Xvfb display `:100`:

```sh
DISPLAY=:100 LP_NUM_THREADS=2 python3 tools/musicterrain_smoke.py --sysroot /workspace/scratch/sysroot --output /workspace/scratch/new-evidence --frames 120
DISPLAY=:100 LP_NUM_THREADS=2 python3 tools/musicterrain_interaction.py --sysroot /workspace/scratch/sysroot --output /workspace/scratch/new-interaction-evidence
```

These developer harnesses create fresh data homes and set `HORIZON_NO_AUDIO=1`,
as required by Horizon's `AGENTS.md`. Their explicit silent clock does not test a
physical sound device. Evidence directories must be new. If the environment has
restarted, first start the extracted display server in another terminal:

```sh
LD_LIBRARY_PATH=/workspace/scratch/sysroot/usr/lib/x86_64-linux-gnu /workspace/scratch/sysroot/usr/bin/Xvfb :100 -screen 0 1280x800x24 -nolisten tcp -ac
```

To leave the interactive native demo open in this provisioned cloud display,
using the compiled executable (no Cargo invocation):

```sh
DISPLAY=:100 XDG_RUNTIME_DIR=/workspace/scratch/runtime LP_NUM_THREADS=2 LD_LIBRARY_PATH=/workspace/scratch/sysroot/usr/lib/x86_64-linux-gnu VK_DRIVER_FILES=/workspace/scratch/sysroot/usr/share/vulkan/icd.d/lvp_icd.json ./target/debug/musicterrain demo --silent-preview
```

The window is on Xvfb, so it is not a physical desktop display. On a local desktop,
use the normal `cargo run -p musicterrain -- demo` command to request real audio.
