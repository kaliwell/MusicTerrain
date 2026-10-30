# Textured scene generation

The native MusicTerrain preview now builds a textured, decorated voxel scene through Horizon's existing `SectionMesh` and `TerrainVertex` formats and production renderer. It remains isolated from vanilla save files and the integrated server. These are real visible voxel faces, alpha cutout plants and translucent water; this preview does not provide gameplay block states, collision, fluid simulation or world persistence.

## Data flow

`TerrainSnapshot + SceneOptions → TerrainField heights/density → Ecology surface/cave columns → bounded material volume → village material overlays → exposed terrain faces → seeded decorations → Horizon section meshes`.

`visual::SceneOptions` has three independent, enabled-by-default switches: `vegetation`, `water`, and `caves`. `mesh_snapshot` and `MeshWorker::submit` use these defaults. `mesh_snapshot_with_options` and `submit_with_options` accept explicit options. Options participate in the completed-snapshot cache identity; changing a switch cannot reuse the preceding geometry. Biome selection and VillageMode are also part of SceneOptions and cache identity.
Preview generation changes are separate from the deterministic core heightmap export.

## Materials and terrain

The original base material layers include the following in `src/materials.rs`: grass top/side, dirt, stone, sand, water, oak bark/end grain, oak leaves, short grass, poppy, dandelion, snow, coal ore, iron ore, gravel and clay. The default atlas consists entirely of original pixel art. An optional local resource texture overlay is handled by the material loader and renderer, not copied into the repository.

Grass tops and sides use separate layers. Three dirt layers lie beneath ordinary grass, followed by stone. Ecology selects shore sand and alpine snow. Seeded three-dimensional ore patches, gravel and clay appear on exposed deep faces. Vertex colours use the material's tint, directional face shading and a small music colour response, instead of tinting every material with the same landscape colour.

Adjacent opaque voxels hide their shared faces. Side faces merge only within one material and one 16-block section; emitted rectangles are then divided into spans of at most two blocks per texture axis, matching the production vertex format's fixed-point UV range. Repeating UVs retain block texture scale rather than stretching one stone image across an entire cliff. Leaf layers retain alpha cutout rather than setting Horizon's force-opaque layer flag.

Section keys use `horizon_client_render::chunk::MIN_Y` when converting world Y to section index. All uploaded positions remain within their section. Both negative world coordinates and section boundaries use Euclidean division.

## Water

Low terrain fills to level 8. Higher landscapes additionally contain deterministic local ponds anchored to a 48-block world grid. Each pond samples the underlying music-driven field at its fixed centre, carves a shallow bowl and fills it to a capped local level. Sand marks the pond shore. This makes water available in the default mountainous preset without moving the entire sea above mountain terrain.

Water quads go to `SectionMesh::translucent`; they are never mixed into the opaque terrain batch. This is a visual fill, not a simulation of Minecraft water flow. Disabling water also disables these preview-only pond depressions.

## Trees and plants

`ecology::Ecology::decorations` supplies seed-derived world-grid anchors and stable event IDs. Oak trees contain individual trunk and crown voxels. End faces use the log-end layer, bark uses the side layer, and leaf voxels use the leaf layer. Shared faces inside trunks and crowns are culled. Canopies are clipped to the preview region, with outside anchors included by Ecology's halo query.

Grass and flowers consist of two crossing planes, each emitted with both windings, so they remain visible from either side with the existing culled terrain pipeline. Their PNG alpha creates the silhouette. Vegetation needs genuine solid support; it is not placed over a pond or a carved cave entrance.

## Caves and islands

The default height-field path now has actual carved air voxels. Ecology computes allocation-free cave intervals once per X/Z column, then the material fill removes those voxels and exposes walls, floors and ceilings. Cave faces receive depth shading so entrances read as openings. The cave switch controls these preview networks and the core cave threshold in the density path.

The three-dimensional density path is selected only by floating-island strength.
Lowering the cave threshold never changes the sampling algorithm. In the ordinary
preview it adjusts cached cave intervals around the original 0.78 baseline, with
bounded expansion preserving the bottom three layers and existing roof/entrances.
The raw core density field used by Noise Lab remains a separate mathematical field.

`cave_w` selects a slice of seeded, quintic-interpolated 4D value noise. Its W
period is 256, so clock wrapping has no discontinuity. Each preview column caches
up to 59 transformed cave intervals in fixed storage before voxel filling. The
field changes interval center/thickness, with the same floor/roof caps, and keeps
the original cave layout at W=0. No vertices move between grid points. The raw
core density field modulates its cave signal with the same 4D slice difference.
Default mappings drive W from RMS; playback adds `audio_time * cave_speed`.
Camera orbit is independent from audio time and does not request new meshes.
`MeshResult::voxel_step` records actual detail; full detail uses step one even for islands.

## Budgets and diagnostics

Full detail uses up to 256×256 horizontal samples and 128 vertical cells (8,388,608
cells); adaptive detail retains the previous coarser sampling. Each voxel stores
one byte of material identity plus a carved-air flag. Footprints clip the material
volume and decorations, including structure overlays, before meshing. No material
volume is retained after its mesh is produced. The worker still has one active job,
one pending slot and one ready result; cancellation and shutdown remain bounded.

`MeshResult::scene` reports actual emitted content: tree and plant counts, carved voxel count, cave face count, water quad count, solid voxel count and quad counts for every material. Counts refer to the generated preview, not an estimate of persistent Minecraft world content.

## Verification

`tests/scene_mesh.rs` checks default preset content, real switch effects, determinism, material-layer identity, alpha/cutout flags, section bounds, texture UV bounds, option-sensitive cache identity and density topology. `tests/visual_pipeline.rs` retains revision, queue, invalid input and fixed-time geometry checks. The flat terrain test disables scene enrichments and checks the complete exterior area and outward winding even though material boundaries now split its faces.

Actual command results and measured performance are recorded in `TEST_REPORT.md` and `PERFORMANCE.md`; these design budgets are not frame-rate claims.

## Rich ecology / village increment

There are now 54 stable original material layers. Jungle includes variable ordinary
and giant trees, rounded crowns, cocoa, bamboo, bushes and melons. Other biomes
have distinct understory. Tree/plant halos are independently clipped, preserving
whole/tiled ecology. Villages are full deterministic world-cell plans before
clipping; their material cells are installed into the temporary volume before
face culling, and their reserved footprint suppresses vegetation. Crops use
existing cutout plant rendering. Details and limits: [WORLD_DETAILS](WORLD_DETAILS.md).

## Modern-style underground increment

The material volume now unions original entrance carving with cached
UndergroundField chamber/connector intervals, applies cave surface palettes and
bounded static aquifers, then original mine plans and village overlays. Optional
inspection masks run after generation. Rail planes, web/lantern detail meshes
and lichen overlays use existing Horizon vertices/sections. Mines and View are
part of SceneOptions/revision/cache identity. See [UNDERGROUND](UNDERGROUND.md)
for actual geometry, safety, budgets and remaining gameplay/save limitations.
