# Changelog

## 0.1.0 development — 2026-10-29

- Audited Horizon baseline `62a23ba0258a89e7a69b513f333c6c19d05e6594` and created
  `feature/music-terrain` without modifying vanilla gameplay or save modules.
- Added one functional workspace crate, `musicterrain`: file decode/resample,
  fixed-hop FFT/features, validated ordered mappings, Horizon-based fields and
  seven versioned presets.
- Added native audio playback, explicit silent diagnostic clock, bounded async
  preview meshes, production Horizon renderer adapter and functional HUD editor.
- Added analysis CLI exporting features and deterministic heightmaps, with checked
  output creation and completion markers; no Minecraft save claim.
- Added bounded diagnostic file rotation and real core/CLI/meshing tests.
- Integrated compilation/runtime checks are recorded in TEST_REPORT, including
  failures and remaining target-machine checks.

### Verification completed 2026-10-30 UTC

- Final native demo: build, all-target Clippy and 42 tests passed. Headless mode:
  33 tests passed. Upstream core: 107 passed/2 original microbenchmarks ignored;
  upstream Python: 416 passed.
- Real native renderer readback and X11 editor actions passed on software Vulkan.
  Saved screenshots and raw evidence under images/ and verification/.
- Documented measured 64² software-GPU limits and severe 256² stalls without
  claiming hardware 60 FPS, physical audio or complete Music World export.

### Textured scene update — 2026-10-30 UTC

- Replaced the shared pale stone layer with 17 original pixel-art materials and
  optional local resource-directory PNG loading, including grass-side overlay.
- Added seeded ecology, actual oak/leaf blocks, cutout flowers/grass, ponds,
  sand/snow surfaces, deep ore materials and carved preview cave networks.
- Added sky/clouds, material-source indicators, real scene counts and independent
  geometry switches. No persistent-world or physical-fluid claim.
- Verified 60 native tests, 40 headless tests, fmt/clippy/build, actual renderer
  readback, local-PNG rendering and real GUI switches. Evidence: verification/scene/.

### Biome update — 2026-10-30 UTC

- Expanded four preview biomes to ten, adding birch forest, taiga, desert,
  badlands, savanna and swamp with seeded climate distribution.
- Added birch/spruce/acacia species and crown shapes, cactus, dead bush,
  red sand, terracotta and podzol; appended 14 original atlas layers (31 total).
- Added real Settings biome cycling and validated `--biome` selection, included
  selection in mesh cache identity and reused revision-based stale-result rejection.
- Added automatic-distribution, tiled-ecology and real-mesh tests plus an X11
  biome-switching harness. No vanilla generator or persistent-world changes.

### Tropical jungle — 2026-10-30 UTC

- Added Jungle to automatic humid/hot climate selection, Settings and `--biome jungle`.
- Added denser jungle trees with 12–14-block 2×2 trunks and broad radius-3 crowns,
  deterministic hanging vines and fern understory. Five new original material layers;
  36 layers total, previous material IDs preserved.
- Extended tile/species/material/alpha and all-biome UI checks; added jungle
  density/height and vegetation-switch tests. No climbing or world-save claim.

### Rich biome details and villages — 2026-10-30 UTC

- Added mixed-size jungle trees, irregular rounded crowns, understory bushes,
  bamboo patches, melons, attached cocoa and two-block sunflowers; differentiated
  forest/taiga/swamp/meadow plants with mushrooms, pumpkins and bank-only cane.
- Expanded tree halo to five, shrub/bamboo halos to two/one; retained seeded
  layout, clipping, cancellation and bounded requests.
- Appended 18 original plant/building/farm material layers (54 total), preserving
  old IDs and cutout policies.
- Added original biome-styled village plans with hollow houses, open doors,
  windows, roofs, foundations, roads, a well and irrigated farms/hay details.
  Auto / Showcase / Off is real UI/CLI state and part of cache identity.
- Added real village/tile/switch/structure tests and native UI/render evidence.
  No ordinary Horizon gameplay/save modifications or vanilla-template claim.

### Modern-style caves and abandoned mines — 2026-10-30 UTC

- Added bounded analytic cave chambers, winding connectors, four underground
  palettes, limited low aquifers, depth-relative deepslate and extra ore seams.
- Added moss, short dripstone spikes, amethyst outcrops and cutout cave lichen;
  appended eleven original material layers, preserving old IDs (65 total).
- Added deterministic abandoned mine galleries, floors/supports, connected
  directional rails, webs and lantern models. Roof safety samples full-plan
  actual pond heights; support frames avoid crossings.
- Added native Underground page, CLI modes, nonpersistent quadrant cutaway and
  Y10 slice, cache/revision invalidation and actual geometry diagnostics.
- Added real cave, clipping, connectivity, material, view and switch tests;
  verified native renderer readback and complete UI interactions. No exact
  vanilla/noise-router/persistence or negative-Y-world claim.
