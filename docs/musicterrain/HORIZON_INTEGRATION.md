# Horizon integration

Baseline: `62a23ba0258a89e7a69b513f333c6c19d05e6594`.

## Preview adapter implemented in this iteration

`crates/musicterrain/src/renderer.rs` wraps production
`horizon_client_render::gfx::Renderer`; it does not contain a replacement wgpu
renderer. `PreviewRenderer::new(Arc<Window>, offscreen)` creates procedural
16×16 stone pixels, a white HUD texel and a six-face 1×1 panorama. No Minecraft
resource bundle is needed. `PreviewRenderer::draw` constructs a real `gfx::Frame`
and routes it through `Renderer::render`. The mesh adapter uploads the existing
`SectionMesh` format through the public `renderer` field. Fullbright is deliberate
for the isolated preview; it is not a vanilla world lighting implementation.

The wrapper uses one caller-owned winit event loop. Resize uses
`preview.renderer.resize(width,height)`. A screenshot sets
`preview.renderer.screenshot = Some(path)` before draw and checks
`screenshot_result` afterward; a file's existence alone is insufficient proof.
Rendering statistics come from `last_encode`, `last_wait`, `gpu_times`,
`uploaded_bytes`, `resident_sections()` and `section_vertex_bytes()`.

The separate executable is an engine integration and preview deliverable. It is
**not** an implemented Horizon title-menu mode, game-world streaming integration,
prepared block-material integration or persistent Music World. Those acceptance
criteria require their own code and verification.

## Later built-in mode

The lowest-risk game entry is an explicit CLI/screen route in
`crates/horizon-client/src/main.rs`/`game.rs` before opening a normal world.
Keep MusicTerrain state in its own owner and return to the existing title screen
on exit. Use the existing renderer/HUD/camera/input rather than nesting a second
window loop. With prepared resources available, replace procedural sprites with
the current pack and emit meshes using real block material IDs/models.

Do not inject transient meshes into the normal `World` or its dirty/save log.
An isolated renderer draw group or isolated section set needs lifecycle clear/reset
when switching modes. Existing `Renderer::clear_sections` removes resident detail
meshes; preserve/rebuild normal game meshes when returning. Pause/stop preview
audio, cancel/join preview workers and clear immutable snapshots before vanilla
world loading. This transition is not implemented simply by launching the preview.

## Music World on the authoritative server

The default single-player path uses `game/integrated.rs::open_integrated`, which
spawns a second `Game` and joins through memory as a remote client. A generator
selected only in the window cannot affect authoritative chunks. Add serialized,
validated world metadata to the creation/loading options and install a dedicated
`ChunkGenerator` in the server's world-loading path. Its input must own an immutable
audio-derived parameter timeline and seed, independent of playback state.

`VanillaTerrain` currently implements `streaming::ChunkGenerator` in
`crates/horizon-client/src/vanillagen.rs`; `save.rs::Persistent<G>` wraps it with
loading/writing. The new generator should reuse these interfaces without changing
the default implementation. It must provide real block/biome palettes, heightmaps,
lighting and bounds; let existing chunk packets/meshing synchronize clients.
Both default server-thread and `--in-frame` paths need tests when implemented.

## Save compatibility and recovery

Reuse `horizon-world::{chunk,nbt,region}` and `horizon-client::save` codecs.
`RegionWriter::create/write/finish` stages a new region; `SaveDir::flush` exposes
write failures. Create a new world in a fresh sibling staging directory and
refuse an existing destination. Write versioned MusicTerrain metadata separately;
audio is not embedded by default. Check cancellation between finite work units,
flush before promotion and retain/report recoverable failed staging data.
Reopen through `Level::read`/`SaveDir::load` and compare chunk output before calling
this feature complete. Do not fabricate a private format with an `.mca` extension.

## Tests required when game code changes

Follow `AGENTS.md` and `tools/pr_ready.py`; run touched-crate fmt/clippy/tests.
Run `horizon-client` lib/bin/integration tests rather than all its unrelated
examples. `tools/parity.py --changed origin/main --list` identifies vanilla
harnesses, which need the user's private prepared game data and JDK 25.
Run server-thread and in-frame world open/close, vanilla baseline, mode switch,
save reopen, error/cancel, and stale revision scenarios separately. Software
offscreen rendering cannot establish RTX 3060/Windows 60 FPS acceptance.

## Rich world details and village adapter

`village.rs` creates original bounded preview structures; `Volume::apply_village`
applies their materials before meshing through the existing Horizon renderer.
It does not create server-side block states, structure starts, POIs or villagers.
The original resource-backed jigsaw/template path remains untouched. See
[WORLD_DETAILS](WORLD_DETAILS.md) and the source audit follow-up for actual
upstream interfaces and why this preview uses an isolated adapter.
