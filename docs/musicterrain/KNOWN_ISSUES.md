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
- Preview meshes update asynchronously, capped near 15 Hz with adaptive delay;
  rendering is independent. No continuous WGSL vertex deformation is implemented.
- 128/256 previews deliberately reduce voxel detail. Caves/islands use a bounded,
  coarser density volume, not full-resolution persistent chunks.
- OpenSimplex is not implemented; the menu accurately names Horizon Simplex.
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
