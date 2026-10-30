# Known limitations

- S2 is a standalone executable using Horizon engine crates. It is not yet a
  selectable mode in the ordinary Horizon game window.
- Minecraft saves, collisions, light/heightmap adaptation and beat structures are
  not implemented. Analysis exports are explicitly heightmaps/JSON, not worlds.
- Live means file playback and audio-timed terrain animation. Microphone input,
  Windows system loopback and device switching are not implemented.
- Playback uses mono analysis PCM and supports default-device f32/i16/u16 formats.
  The sample cursor follows output callbacks; hardware buffering/timestamp latency
  calibration is still needed. Offset is editable. Seeking cannot retract sound
  already queued in the hardware device.
- The explicit silent diagnostic preview uses a software clock and outputs no sound.
  Without a device, normal mode shows an error and leaves the preview paused.
- Preview meshes update asynchronously, targeting up to 60 Hz with adaptive delay;
  rendering is independent. No continuous WGSL vertex deformation is implemented.
- Full detail preserves one-block voxels at 64/128/256; adaptive detail optionally
  reduces resolution. Large full-detail scenes cost more CPU/memory and can update
  much slower than the display. These are bounded previews, not persistent chunks.
- OpenSimplex is not implemented; the menu accurately names Horizon Simplex.
- Preview W animation reshapes analytic cave intervals with a continuous 4D field;
  it is not Minecraft's vanilla cave noise router. Voxel occupancy still changes
  in whole blocks. Larger/density scenes can update more slowly while W advances.
- Orthographic preview uses a flat sky background without perspective clouds.
  Orbit/projection/zoom are session controls, not part of saved visualization presets.
- Structure density and particle intensity are reserved parameters; their controls
  must not imply that event structures or dynamic particles already exist.
- Audio decoding is capped at 20 minutes of PCM. Import is bounded and shutdown
  cancels hashing, decoding, resampling and FFT at fixed checkpoints. Replacing a
  currently loading file queues at most one next import.
- RustFFT may select CPU-specific SIMD. Cross-architecture bit-identical feature
  extraction has not been verified. Fixed-input chunk order tests target this build.
- Windows audio/display, RTX 3060 frame rate and actual audible playback on physical
  hardware require target-machine validation; Linux software Vulkan is separate.

- Measured Linux software-Vulkan 64² windows ranged from about 15–33 FPS in short
  repeated runs. The 256² experiment had a 3.06 s stall. Keep 64² as the demo default;
  the larger extents need further upload/render work and target-GPU measurement.

- The textured scene uses original pixel art by default. Optional local PNGs do
  not include Minecraft resources with the repository. Texture animation uses
  the first square frame; ZIP packs, custom block models and mcmeta frame grids
  are not supported yet.
- Trees, ponds and carved preview tunnels are real mesh geometry, but their
  material volume is temporary. They have no collision, fluid simulation or
  persistence. The deterministic heightmap CLI remains the core-field export.
- Preview scene switches currently belong to the session/CLI, not the existing
  visualization-preset JSON schema. Full snapshot export is future work.

- Rich ecology and villages reference Minecraft's visual vocabulary using original
  templates/materials; exact vanilla feature/jigsaw parity is not implemented.
- Villages are suspended in the floating-island density path and in coarse adaptive previews, with a Settings notice;
  use Full Detail to retain one-block walls and interiors. Steep musical terrain may reject plots or expose
  tall foundations. Showcase is world-origin anchored, not camera-relative.
- Village houses/crops/wells are visible geometry. Villager/animal entities,
  crop growth, climbing, collision and persistent-world integration are absent.
- Sugar cane currently uses nominal level-8 banks; elevated preview ponds do not
  all receive it. Glass uses cutout frames, not a new blended-glass renderer.

- Underground preview remains Y0..128, with deepslate based on sampled depth.
  Its cave/low-aquifer/mine algorithms are original, not vanilla noise-router parity.
- Detailed mines require a one-block height-field path; coarse/density previews suspend
  them. Underground view is a fixed Y10 horizontal slice, not a movable slice slider.
- Amethyst is a decorative outcrop, not a full geode structure. Lanterns/lichen
  are visual details without emitted block light. No minecarts, loot, spawners,
  entities, collisions or saved Minecraft regions are introduced.
