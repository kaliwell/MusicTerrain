# MusicTerrain source audit

Audit date: 2026-10-29. Upstream: <https://github.com/horizon-rust/horizon>.
Baseline: `62a23ba0258a89e7a69b513f333c6c19d05e6594`.
Development branch: `feature/music-terrain`. The workspace was empty before cloning;
there were no user edits or earlier MusicTerrain prototypes to preserve.

## Repository contracts

Read `AGENTS.md`, `CONTRIBUTING.md`, `README.md`, `Cargo.toml`,
`docs/README.md`, `docs/architecture.md`, `docs/world-generation.md`,
`docs/native-client.md`, and `docs/design-system.md`. This checkout targets Java
Edition **26.3**, uses Rust edition 2024, winit 0.30, wgpu 30.0.1 and a local
`third_party/wgpu-hal` patch. `horizon-client-render` requires Rust 1.90 even
though the workspace package default is 1.85. Do not remove the wgpu patch or
add another window event loop inside the game.

Player-facing code must be Rust. No Mojang resources, prepared `.local/` data,
music or credentials may be committed. Runs must use fresh scratch data homes.
The project requires its own features to be opt-in when changing vanilla visuals.
The existing workspace has many parity probes; `tools/pr_ready.py` selects checks
and reports unavailable vanilla fixtures. Tests requiring resources cannot be
claimed to pass merely because their upstream test returns early.

An open-PR search (`gh pr list --repo horizon-rust/horizon --state open --search
music --limit 10`) found an audio device settings PR #3355 and biome ambient music
PR #3494. MusicTerrain avoids editing those mixer/settings paths in this iteration.

## Seed to block state: actual call graph

| Stage | Source and concrete entry | Data and dependencies |
|---|---|---|
| Resource registry | `crates/horizon-world/src/worldgen/generator.rs`, `GeneratorData::load` | Reads prepared registries, biome JSON, noise/router resources, surface rules, carvers, features and structures. Requires the user's Minecraft resources. |
| Seed binding | Same file, `OverworldGenerator::for_world` → `build` → `Pipeline::new` | Resolves preset/dimension/settings; chooses `LegacyRandom` or `XoroshiroRandom` from `legacy_random_source`; binds one immutable seeded pipeline. |
| Worker isolation | Same file, `OverworldGenerator::worker`, `GeneratorWorker::generate_proto` | Each worker owns sampler contexts, temporary buffers and statistics. The seed/pipeline are shared, contexts are not. |
| Biomes/structures | `Pipeline::generate_with_context` | Samples a 3×3 biome neighborhood, constructs `BiomeRegion`, obtains structure starts/references and binds beardifier fields. |
| Density fill | Same function → `worldgen/terrain.rs::fill_noise` | `DensityVolume([16,height,16], [chunk_x<<4,min_y,chunk_z<<4], [1;3])`; samples `RouterField::FinalDensity`; aquifers select fluid/air/solid. Writes registry-backed `BlockStateId`s to `TerrainChunk`. |
| Surface/carvers | Same function → `build_surface`, `generate_carving_mask`, `apply_carving_mask` | Uses biome/material rules, positional random, carving masks, aquifers and postprocessing metadata. |
| Heightmaps | Same function → `TerrainChunk::prime_heightmaps` | Primes MotionBlocking, MotionBlockingNoLeaves, OceanFloor and WorldSurface before marking `PreLightChunkStage::Terrain`. |
| Decoration | `GeneratorWorker::decorate` → `Pipeline::decorate` → feature graph and structure pieces | Writes to all 9 proto chunks. Vanilla decoration is explicitly order dependent; Music World cannot inherit its unconstrained neighbor-write semantics if arbitrary chunk order must be deterministic. |
| Client conversion | `crates/horizon-client/src/vanillagen.rs`, `VanillaTerrain`'s `ChunkGenerator::{proto,decorate,finish}` | Type-erased `Proto` wraps `ProtoChunk`. `finish` converts palettes/biomes/ticks/block entities/entities to client `Chunk` and compacts sections. Failures are logged; the vanilla fallback can yield an empty chunk. Music World export must instead propagate failures. |

`docs/world-generation.md` includes historical descriptions of incomplete stages;
the current `generator.rs` call graph above takes priority over those older claims.

## Noise and density

`crates/horizon-core/src/noise/mod.rs` re-exports `PerlinNoise`, `SimplexNoise`,
`NormalNoise`, `NoiseStack` and `DensityVolume`. Perlin and Simplex accept
`&mut impl RandomSource` at construction and expose `get2`/`get3`. Perlin stores
an immutable gradient table and supports derivatives and volume filling.
`noise::wrap` reduces large coordinates; sample all chunks in world coordinates.
`random::LegacyRandom::new(seed)` supplies a deterministic seed without clocks.

`crates/horizon-core/src/density/graph/{mod,seeded,compiler}.rs` compile immutable
graphs; contexts/caches belong to sampling workers. Density inputs include
shifted noise, arithmetic, splines, selections and world fields. In vanilla,
octaves/amplitudes affect normal-noise composition, while router graphs and
surface rules determine final blocks; audio Hz must never be assigned directly
to noise frequency. MusicTerrain layers a parameterized `TerrainField` over the
existing gradient samplers. It does not modify vanilla arithmetic or random draws.

## Streaming, lighting and meshing

`crates/horizon-client-render/src/streaming.rs` owns `ChunkGenerator`, `Streaming`,
`WorkerContext`, `Job`, `Done` and the priority heap. `Streaming::new` starts
workers and assigns worker-local generator state. Jobs are Generate, Decorate,
Finish, Save, Light and Mesh. `Streaming::update` schedules by player distance,
tickets and decoration dependencies. Mesh jobs borrow a 3×3 region, build
`mesher::Mesher`, call `mesh`, then `SectionMesh::sort_solid`; results carry a
sequence. `Streaming::pending`, `busy`, `stages` expose work counts. The upstream
result channel is an unbounded `mpsc::channel`; it is not an acceptable template
for an unbounded high-rate music producer.

`light.rs` computes section light; `world.rs`/`chunk_map.rs` retain resident chunks;
`chunk.rs::Chunk` stores paletted sections, biomes and light. Section mesh vertices
are section-relative `mesher::TerrainVertex`, 24 bytes each. Full smooth light is
fixed point: sky in the low byte, block in the high byte. Visibility and face-side
buckets live in `SectionMesh`. The generated preview can emit this public format
directly; the original material mesher requires baked model/resource tables.

## GPU boundary

`crates/horizon-client-render/src/gfx.rs::Renderer::{new,upload,render}` are public.
`SectionKey = (chunk_x, section_y, chunk_z)`. `upload` releases the previous section
allocation, updates visibility/detail state and stages the replacement mesh.
`gfx/arena.rs::Arena` retains shared vertex pages and reuses free ranges; uploads
use `gfx/upload.rs` and a staging belt instead of one permanent buffer per frame.
The staging limits are 4 MiB chunks and a 64 MiB submit threshold.

`gfx.rs::Frame` carries camera, sky/fog, HUD, entities, effects and shader pack
inputs. `camera.rs::view_matrix`/`perspective` implement production camera math;
camera position is double precision and section origins are made camera-relative
by the renderer. `Renderer` exposes adapter information, GPU timings,
`last_encode`, `last_wait`, `uploaded_bytes`, `section_draw_calls`,
`section_vertex_bytes`, and screenshot request/result fields.

`Renderer::new` accepts raw sprite/HUD/glint/panorama pixels, not a mandatory
resource bundle. This permits a real Horizon renderer preview using original,
procedurally created stone pixels. `gfx/panorama.rs::Faces` is public and accepts
six RGBA images; `hud::HudBatch` accepts public quad geometry. No asset preparation
is required for that preview. This is the selected first playable visual path.

Continuous GPU deformation would extend the existing terrain shader/uniform
binding behind a disabled-by-default option; it is not provided merely by changing
`Frame.game_time`. A bounded asynchronous mesh path is sufficient for the first
preview, with coalesced requests, revision checks and a fixed visible extent.
Neither path should call `World::set` or save transient visual blocks.

## Main loop, UI and native audio

`crates/horizon-client/src/game.rs::run`, `run_event_loop`, `Game::tick`,
`Game::render`, `Game::update_world`, and `ApplicationHandler::about_to_wait`
own the game loop. Horizon uses winit 0.30; renderer construction takes an
`Arc<Window>`. `Screen` and `game/{startup_menu,screens,options}.rs` route screens.
`horizon-client-render/src/{ui,widgets,screen_draw,options_ui,hud}.rs` provide
existing widgets/quad batches. `HudBatch::new` applies automatic GUI scale;
pointer hit tests must use the same scale. A resource-free MusicTerrain HUD can
draw an original bitmap font with quads, using the same batch/renderer.

`crates/horizon-client-render/src/audio.rs` uses Symphonia 0.5 for Ogg/Vorbis and
optional cpal 0.16 for the mixer. `open_device` selects/converts sample formats,
`build_stream` mixes PCM, and `SoundEngine` owns sound-event/channel state. The
existing callback takes a mixer mutex and can resize scratch memory. Do not add
FFT, decoding, capture or terrain generation there. MusicTerrain's dedicated
file player can consume predecoded PCM and publish a sample cursor atomically;
DSP runs outside the output callback. Capture requires a separate bounded PCM
queue and worker. WASAPI loopback is a Windows-specific future capability until
implemented and exercised on Windows.

## Server thread and persistence

`crates/horizon-client/src/game/integrated.rs::Game::open_integrated` clones
options/resources, starts a second `Game` on the Server thread and joins through
an in-memory transport. `remote` is therefore set for default single player.
`Integrated::call` schedules server work; `game/levels.rs` ticks dimensions and
`game/remote.rs` processes incoming state. Mutating a client-only chunk generator
will not change the authoritative default single-player world. Music World must
install the same immutable generation configuration on the server side, then let
normal chunk packets synchronize blocks/light to the client. A live preview is
entirely client-side and never starts that server.

Actual save code is `crates/horizon-client/src/save.rs`, not in the render crate:
`SessionLock::acquire`, `SaveDir::{load,save,flush}`, `encode_with`/`decode_with`,
`Persistent<G>`, `Level::{read,write}`, `write_world_gen_settings_with`.
`crates/horizon-world/src/region.rs` implements Anvil `RegionFile` and transactional
`RegionWriter::{create,write,finish}`; NBT and palette encoding already exist.
Use these for future export. New Music World creation must use a new staging
directory, checked creation, an explicit metadata sidecar and a final rename;
never silently load/update a destination that already exists. A heightmap image
or deterministic sample dump is not a Minecraft save.

## Integration choices and conflict surface

| Path | Cost and limitations | Decision |
|---|---|---|
| Sandboxed native mod | `horizon-mod-sdk` is WebAssembly/WIT, despite running natively in the host. It offers on-frame/on-tick and constrained host APIs, not direct cpal, file decoding or raw wgpu buffers. Extending WIT/runtime plus audio/device permissions is substantial. | Do not pretend a mod already supports this workload. |
| Built-in source feature | Can route a `--musicterrain` command or screen before normal world loading, reuse resources and return to title. Requires `Game` state/lifecycle and integrated-server handling, plus gameplay harnesses if touched. | Later integrated mode; keep vanilla path opt-in and unchanged. |
| Separate workspace executable using engine crates | Reuses production noise, renderer, camera and GPU arenas while owning one winit loop and isolated visual state. It avoids prepared resources via original pixels. Does not itself constitute entering MusicTerrain from the Horizon title screen. | Selected S1/S2 implementation and low-risk engine adaptation. |
| Renderer/shader extension | Best for continuous low-cost vertex/color effects; cannot create caves/collision or persist blocks. Requires audited bindings and shader validation. | Later GPU path; do not rebuild a second rendering engine. |

Primary risks: assigning mutable song state to vanilla workers; reusing vanilla
order-dependent decoration; applying a stale mesh after seek/edit; unbounded
queued updates; assuming output-device time equals wall time; mutating saves for
live effects; claiming Windows/GPU performance from software-rendered Linux.
The selected adaptation isolates these risks in MusicTerrain-owned state and
uses immutable revisions at the renderer boundary.

## Environment observations

Initially no Rust toolchain, display server, `/dev/dri` GPU device or prepared
Minecraft data existed. The root agent is provisioning build dependencies and
recording actual compile/runtime results separately in `TEST_REPORT.md` and
`PERFORMANCE.md`. Installing a toolchain is not evidence that the preview ran.
JDK/vanilla differential harnesses, hardware-GPU and Windows loopback checks
remain distinct from portable DSP/terrain tests.

## Textured preview adaptation (2026-10-30)

`musicterrain/src/materials.rs` now uses Horizon's `hud::decode_bytes` and its
mipmap/upload path to construct a 17-layer array with optional local resource
PNG overrides. `renderer.rs` feeds that atlas into the same `Renderer::new`;
`Frame` enables the existing daylight sky and cloud pass. No renderer fork or
second event loop was introduced.

`horizon-world/src/worldgen/tree.rs` and `vegetation.rs` were inspected: their
real feature execution binds registry block states, prepared JSON definitions,
world queries and placement contexts. They cannot be instantiated faithfully
without those resources. The resource-free preview therefore uses its own
bounded `ecology.rs` adapter for seeded anchors/cave intervals and Horizon core
Simplex for climate. It emits Horizon section vertices for original simplified
oak/plant geometry. This is not claimed to reproduce vanilla tree/biome parity.
Material volumes remain isolated from the vanilla streamer, blocks and saves.

## Ecology and villages follow-up (2026-10-30)

Rechecked `crates/horizon-world/src/worldgen/tree.rs::TreeFeature`, `tree/trunk.rs`,
`tree/foliage.rs`, `tree/decorator.rs`, `bamboo.rs::BambooFeature::{from_json,place}`,
`feature_graph.rs`'s Bamboo binding and `structure/jigsaw.rs::{assemble,assemble_drawing}`.
These operate through registered block-state bindings, feature/placement contexts
and, for villages, resource-backed template pools and structure NBT. The prepared
Minecraft resources are still unavailable in this environment. Copying their
proprietary templates/artwork into the repository is prohibited.

MusicTerrain therefore references recognisable species/biome building vocabulary
with original preview geometry and templates. `ecology.rs::decorations` owns
seeded feature anchors; `visual.rs::decorations` owns native material geometry;
`village.rs::VillageScene::generate` owns bounded full-cell plans before region
clipping. `Volume::apply_village` installs original building/road/farm material
cells before Horizon face meshing. This avoids changing vanilla feature bindings,
integrated-server state, save data or introducing a second renderer.

It is not vanilla village jigsaw parity: natural eligibility is limited to the
supported meadow/desert/savanna/taiga/alpine preview types; Jungle/forest/swamp
villages require explicit Showcase. Villager entities, block behaviour, lighting,
collision and save integration remain separate work.

## Underground follow-up (2026-10-30)

Inspected `crates/horizon-world/src/worldgen/structure/mineshaft.rs::generate`,
`MineshaftPiece` and its Normal/Mesa wood bindings, plus
`crates/horizon-world/src/worldgen/aquifer/noise_based.rs` and `AquiferBlockStates`.
Upstream has rooms, corridors, crossings/stairs and stateful fluid-pressure
selection. Its public layout generator returns pieces; placing them uses the
legacy PieceContext/block-state pipeline. The aquifer consumes registry-backed
states, density volumes, sampler contexts and positional randomness.

The current preview still lacks that registry/server/heightmap/save adapter, so
this increment adds original bounded preview logic instead of changing or
claiming to port those vanilla paths: `underground.rs` computes cached analytic
cave intervals with Horizon Simplex noise; `mineshaft.rs` builds full world-cell
plans before clipping. `visual.rs` fills/carves the temporary volume, adds cave
palette details, applies mine structures, then applies optional display masks
and uploads existing Horizon section meshes. No second renderer is introduced.

Mines sample actual pond-shaped roof heights over the full plan, including
outside query bounds. Frames avoid rail junctions; rail planes carry direction.
The preview remains Y0..128 and uses depth-relative geology. Exact modern
Minecraft noise-router/fluid pressure, negative-Y terrain, loot/spawners/entities,
block light/collision and persistent chunks are not part of this adapter.
