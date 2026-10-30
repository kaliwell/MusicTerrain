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
