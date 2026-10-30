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

The existing three-dimensional density path remains available for strong caves and floating-island presets. Its coarse sampling remains explicitly represented by `MeshResult::voxel_step`; trees and plants still use individual block geometry when supported by the sampled surface.

## Budgets and diagnostics

The primary height-field volume has at most 64×64 horizontal samples and 128 vertical cells. Density sampling has at most 49,152 cells. Larger preview regions use a larger horizontal sample step. Each voxel stores one byte of material identity plus a carved-air flag. No voxel material volume is retained after its mesh is produced. Decorations come from bounded world-grid queries. The worker has one running job, one pending slot and one ready result; revision cancellation and shutdown checks remain active during generation.

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
