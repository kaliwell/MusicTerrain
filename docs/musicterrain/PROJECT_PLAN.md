# Development plan

Start with a separate opt-in workspace binary sharing Horizon's existing noise,
section vertex format, camera math, GPU renderer, arenas and HUD. This gives a
resource-free demo without editing vanilla world generation, saves or the game loop.

1. S0: audit real source and isolated development branch (complete).
2. S1: decoded PCM → fixed-hop FFT → configurable mapping → deterministic terrain
   and inspectable heightmap export (implemented; validation in TEST_REPORT).
3. S2: real playback → bounded asynchronous meshes → native Horizon renderer,
   functional editor and diagnostics (portable interactive demo verified; hardware
   audio and Windows acceptance remain).
4. S3: integrate a disabled-by-default game screen and lifecycle after validating
   this renderer path; require prepared resources and integrated-server tests.
5. S4: extend cached density meshing, beat-event structures and GPU deformation.
6. S5: adapt deterministic chunks to registry-backed Horizon storage, light and
   heightmaps, then atomic new-world saves; test reopening before enabling export.
7. S6: finish devices/Windows WASAPI, presets and mapping UX, benchmark Windows
   hardware and strengthen end-to-end automation.

The current demo is not a Minecraft world exporter. Future work cannot be marked
complete merely because a configuration field or navigation page exists.
