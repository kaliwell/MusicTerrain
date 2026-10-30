# Preview ecology and caves

`crates/musicterrain/src/ecology.rs` adds original, seeded scene decoration for
MusicTerrain's isolated preview. It does not alter Horizon's vanilla generator,
load Mojang structure templates or claim vanilla biome/feature parity. Materials
and meshes are supplied by the native scene adapter, not this core module.

## Surface and plants

`Ecology::biome(x,z,height)` combines height with Horizon's seeded `SimplexNoise`
to select ten biomes, including birch forest, taiga, desert, badlands, savanna
and swamp; see [BIOMES](BIOMES.md) for the climate and material contracts. `surface_profile` returns material-free
Grass/Dirt, Snow/Stone or Sand layer suggestions. The shared nominal water level
is 8 blocks. Height means the first air block above terrain, so a decoration's
anchor Y is exactly the rounded surface height, with no extra floating block.

`decorations(origin,size,surface)` samples the caller's current, world-coordinate
surface closure. Trees, flowers and grass therefore follow the same musical
terrain snapshot as the mesh. Oak trunks are 4–6 blocks high; crowns are owned by
the native mesher. Alpine regions can receive small rocks. Ground features avoid
water, nonfinite heights, steep sampled slopes and cave-mouth blocks.

Trees use a 14-block world grid with bounded jitter. Candidate anchors are at
least 9 blocks apart, preventing overlap for radius-3 crowns. Plants use a
6-block grid, with grass, poppies and dandelions. Trees near query edges are
returned with a three-block halo so neighbouring meshes can independently clip
the same crown. Plant suppression near trunks uses that same halo. Returned
world anchors, type, scale and event IDs are independent of request order.

Event IDs mix the seed with a lossless packing of two signed 24-bit cell axes and
a category tag. The bounded world domain fits those axes. This deliberately
avoids using vanilla's `position_seed` alone: a tiled-generation test exposed
mirrored-coordinate collisions in that positional hash. Climate sampling still
reuses Horizon's real noise implementation.

Requests are limited to 256×256 and world origins inside ±29,999,000. Larger or
invalid requests return an empty list. The fixed grids bound a maximum request
to fewer than 2,500 decoration candidates; there is no growing global cache or
order-dependent local truncation. Typical 64×64 scenes have fewer than 35 trees
and 180 small plants, with density reduced naturally by biome and slope checks.

## Real preview cave volume

Every 48-block world cell has a seed-jittered entrance. A narrow shaft descends
into two branching tunnels whose depths follow the supplied surface height.
The entrance and tunnels are actual air classifications in the scene's voxel
volume. The default 64-block scene can therefore show cave mouths without an
island preset. Entrance footprint is small, and cave carving is disabled at
water-level surfaces. Adjacent cells are inspected for shapes crossing borders;
all geometry uses world coordinates, including negative coordinates.

`cave_column(x,z,height)` computes shaft/tunnel intersections once, stores them in
27 fixed-capacity vertical ranges, and allocates nothing. The mesher calls
`CaveColumn::contains(y)` inside its voxel loop. `is_cave` is a convenience wrapper.
`cave_density(x,y,z,height)` exposes a continuous signed field for diagnostics and
other adapters; negative values mean cave air. It is not an exact Euclidean SDF.

These are preview caves. Persistent-world block/light/heightmap updates and
walkable collision integration remain separate work. A preview entrance is not
a claim that Minecraft region export is implemented.

## Tests

`tests/ecology.rs` covers seeded layouts, distinct seeds, tree spacing, matching
whole/tiled queries with shared edge IDs, surface following, water/slope/cave
avoidance, the default Bass Mountains scene's trees/grass/flowers/entrances,
column/signed-field agreement, continuity across world cells, bounded requests,
invalid coordinates and surface profiles. Additional biome tests cover all six
new species/soil contracts, automatic reachability and tiled layout equality. Actual consolidated results
belong in `TEST_REPORT.md`; this document does not substitute for running them.
