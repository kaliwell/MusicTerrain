# MusicTerrain noise and mapping engine

Implementation: `crates/musicterrain/src/{terrain,mapping,preset}.rs`.
This is an isolated field adapter, not a replacement for vanilla's density router.
It uses `horizon_core::noise::{PerlinNoise,SimplexNoise}` initialized by Horizon's
`LegacyRandom`. No proprietary resources or third-party terrain implementations
are included.

## Implemented fields

`TerrainField::new(seed)` constructs immutable samplers once. `height(x,z,p)`
samples world coordinates, domain warps X/Z, sums bounded octaves, blends ridges,
adds a smaller detail field and applies gain/erosion. Available noise modes are
Perlin, Simplex (Horizon Simplex, not OpenSimplex), fBm and Ridged. Frequency is a
spatial sampling rate; audio Hz never becomes a noise frequency directly.
Scale divides spatial frequency. Ridge blending is independent of the selected
mode except Ridged, which selects the full ridge response.

`density(x,y,z,p)` combines the height surface with a 3D Simplex cave field and an
optional Perlin floating-island field. Positive density is solid. This provides
actual different solid masks, rather than shader-displaced collision geometry.
`generate(request)` emits a bounded deterministic solid mask in X/Z/Y order;
this is an in-memory adapter result, **not a Minecraft save format**. Horizon's
block registry, heightmaps, light and region writer must be connected before this
mask can become a playable saved world. No save integration is claimed here.

All field parameters have defaults and bounds. `validate` reports invalid input;
`clamped` sanitizes a snapshot before sampling. Octaves are limited to 1–8,
heightmap dimensions to 1024 each, solid-mask jobs to 2,097,152 voxels. Coordinates
outside ±30,000,000 or nonfinite coordinates return a stable empty result. World
coordinates avoid tile seams, and independent samplers never advance RNG during
generation. Revision/snapshot/request types tag asynchronous results, but worker
cancellation and stale-result rejection belong to the preview integration.

The `structure_density`, color, particle and shockwave fields are transport
parameters reserved for rendering/structure adapters; the scalar height/density
field does not itself generate beat structures or particles. Deterministic event
placement and Horizon feature/structure integration remain later-stage work.

## Mapping

`MappingGraph` is an ordered list of at most 128 `MappingRule`s. Each rule samples
a band (0–6), RMS, centroid, flux, onset, beat strength or a reliable optional beat
phase, applies input range / gain / bias, optional clamp, response curve,
attack/release smoothing and an output range. Replace/Add/Multiply then combine
with the current target value in declared vector order. Each intermediate target
is clamped to the same bounds the editor uses. Curves are Linear, Smoothstep,
Power and Logarithmic. Nonfinite audio features become zero. There is no hidden
rolling normalization or random state.

`MappingState` belongs to the audio timeline. Use fixed DSP hops for deterministic
offline snapshots. Pass `dt=0` when paused, and reset/replay fixed hops after
seeking. A changed rule list automatically resets state so values from the old
graph cannot bleed into the new graph. Frame-rate independence requires callers
to sample audio hops, not to drive smoothing with render time.

`preset::builtins()` supplies seven different configurations with the requested
names. Presets are validated version-1 JSON; version 0 with the same prototype
schema upgrades to 1, and unknown versions fail explicitly. SHA-256 hashes use
canonical struct serialization. `save` creates a new file and refuses overwrite;
`load` and `from_json` validate parameters and mappings before use.

## Validation

`tests/terrain_pipeline.rs` covers seed identity/sensitivity, shared tile edges,
request-order independence, bounded volume rejection, finite extreme input,
parameter effects, cave/island topology, ordered combining, pause stability,
graph-edit state reset, invalid ranges, silent/nonfinite features, all seven
preset differences and versioned serialization. See `TEST_REPORT.md` for actual
commands and results; the existence of these tests is not a passing result.

## Modern-style cave preview

`UndergroundField` adds analytic ellipsoid chambers and globally winding tube
families, sampled as fixed-capacity merged vertical intervals. Horizon seeded
Simplex selects cave palettes and low aquifers. These are preview-specific fields,
not modifications to the core exported height/density function or a vanilla
noise-router port. Mines use separate deterministic world-cell structure plans.
The display cutaway/slice does not change the cave/mine generation inputs.
