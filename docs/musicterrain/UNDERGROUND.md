# Modern-style caves and abandoned mines

MusicTerrain now generates original modern-style underground detail in its isolated
preview: broad cave chambers, winding connectors, four underground palettes,
restricted aquifers, ores, and abandoned mines. It still uses Horizon's production
section meshes, material arrays, renderer and editor loop. No Minecraft assets or
structure templates are included.

## Run and inspect

```sh
cargo run -p musicterrain -- demo --mines showcase --view cutaway
cargo run -p musicterrain -- demo --mines showcase --view underground
cargo run -p musicterrain -- preview --audio song.wav --mines auto
```

**08 UNDERGROUND** in the left navigation has working Caves, Mines and View
controls. Mines cycles Auto / Showcase / Off. View cycles Surface / Cutaway /
Underground. These controls invalidate cache identity and generation revisions.
Caves OFF also suspends mines. Mines require a 64×64 height-field preview; larger
LOD/density/island previews retain caves but suspend the detailed mine structures.

Cutaway removes the positive X/Z quadrant, exposing vertical sections while
keeping the rest of the surface and vegetation. Underground hides cells above
Y10 and suppresses surface vegetation, exposing a horizontal mine/cave slice.
These are display masks applied after generation, not destructive world edits;
they do not count as newly carved cave voxels. Free camera controls still work.

## Cave field and geology

`underground.rs::UndergroundField::column` reuses Horizon's seeded Simplex noise
and computes fixed-capacity, merged analytic vertical intervals. Broad overlapping
ellipsoids form chamber shelves/openings; world-space winding X/Z tube families
create smaller connectors. It is evaluated per column, not with a fresh allocation
or noise planner per voxel. Native filling unions it with the earlier entrance
and descending-branch network in `Ecology::cave_column`.

The new intervals retain Y0–2 as solid base and at least three blocks of roof.
Stone, Lush, Dripstone and Deep palettes are selected from seeded world noise.
Exposed cave surfaces receive moss, short voxel stalactites/stalagmites, occasional
amethyst outcrops and cutout lichen. Copper/gold/diamond seams and deepslate add
subsurface material variation. This preview chooses deepslate by depth below the
sampled surface, not Minecraft's full-height Y thresholds.

Low aquifers fill only carved cells through Y6, under a bounded world-noise mask.
Water OFF removes aquifer water as well as surface water. These are static visual
cells, not Minecraft fluid pressure, lava selection or flowing-water simulation.
Amethyst is a decorative outcrop here, not a complete hollow geode generator.

## Abandoned mine layouts

`mineshaft.rs::MineScene::generate` builds complete seeded world-cell plans before
region clipping. Auto uses 128-block cells, ±12 centre jitter and a 56-block
candidate margin; Showcase is fixed at world origin. The floor is Y6–8, selected
by Seed. A main gallery, central crossing and two side galleries form connected
rail routes where the sampled actual surface provides sufficient roof cover.
Pond-shaped ground is sampled over the whole plan, including outside the clipped
preview, so a surface pond cannot expose a supposedly buried support frame.

The galleries contain plank floors, wooden posts and beams, directional rail
planes, overhead webs and small lantern models. Frames avoid intersections so
posts do not block the rail routes. Rails are deduplicated and oriented along
their corridor. Lighting remains the existing preview shading; lanterns and
lichen are not simulated light emitters. Inspection views brighten cave shading
for readability without changing normal Surface shading.

Mine plans check cancellation per candidate and corridor segment; requests are
limited to 256 blocks, 100,000 planned air cells and 60,000 material cells.
Bedrock/roof safety, invalid inputs and clipping are tested. Detailed mines are
not collapsed into coarse LOD cells. One pending mesh request, revision rejection
and shutdown checks remain unchanged.

## Compatibility and actual limits

This is an original preview adapter, not vanilla noise-router/aquifer/mineshaft
parity. The preview volume still spans Y0..128; the full Minecraft -64..320 world,
ancient cities, trial chambers, minecarts, loot, spawners, entities, collisions,
block lighting, world saves and region export are not implemented by this update.
Normal Horizon gameplay/generation/saves remain untouched. `analyze` continues
to export the core height field rather than a decorated Minecraft world.

All previous material IDs remain stable; new IDs54..64 add deepslate, copper/gold/
diamond ore, moss, dripstone, amethyst, rail, cobweb, lantern and lichen. The atlas
now has 65 original layers, with optional matching local PNG replacements. Scene
modes remain runtime/CLI options rather than saved preset JSON fields.

## Evidence

[Actual cutaway](images/underground/cutaway.png) ·
[Actual horizontal slice](images/underground/slice.png).
Tests cover cave intervals/biomes/aquifers/safety, connected unobstructed rails,
directional rail geometry, both Auto and Showcase clipped plans, material output,
view/switch behaviour and section-local vertices. Actual native UI interactions
and renderer readback are recorded in [TEST_REPORT](TEST_REPORT.md) and
[PERFORMANCE](PERFORMANCE.md).
