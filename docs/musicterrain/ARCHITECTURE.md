# MusicTerrain architecture

This is an incremental implementation inside the real Horizon Cargo workspace.
See [SOURCE_AUDIT.md](SOURCE_AUDIT.md) for the baseline and inspected symbols;
see [TASKS.md](TASKS.md) for implementation status. This file describes ownership
and integration constraints, not a claim that every listed mode exists.

## Selected boundaries

A single `musicterrain` crate keeps small, testable modules without creating
empty packages. Core audio, DSP, mapping and terrain data must be usable without
opening a window. The preview adapter depends on `horizon-client-render` and
uses `Renderer`, `SectionMesh`, `TerrainVertex`, `HudBatch` and the existing camera
math. Terrain sampling depends on `horizon-core` noise and Java-compatible seeded
random. MusicTerrain must not implement another Minecraft renderer or replace
the vanilla world generator.

```text
file → decode/downmix/resample → immutable PCM → offline FFT timeline
                                      │                    │
                                      ↓                    ↓
                            cpal callback cursor → feature lookup
                                                           ↓
                    editable ordered mapping graph → parameter snapshot
                                                           ↓
                 Horizon seeded noise ← TerrainField → preview mesh worker
                                                           ↓
                                           revision filter → Renderer::upload
                                                           ↓
                                          Horizon Renderer + HudBatch
```

The FFT planner/window/scratch buffers are retained. Audio callbacks copy/convert
PCM and advance an atomic sample cursor; no FFT, file I/O, locking waits, terrain
generation or large allocations belong there. Decode/analyze is a loading task;
live capture later adds a bounded PCM ring and DSP consumer. Initial file-based
analysis can prepare a timeline once, then playback/seek samples that timeline.
This yields repeatable feature lookup and keeps render cadence independent of FFT.

## Time and deterministic mapping

Audio sample position is the file player's timebase. Pausing freezes its state;
seeking invalidates any history-dependent visual state and stale pending work.
An explicit silent/no-device clock can be useful for headless verification, but
must be labeled and must not be described as verified hardware playback.

World mode maps X to song time by `(world_x - origin_x) / blocks_per_second`.
Clamp, loop and finite-region handling must be explicit. To preserve chunk-order
independence, smooth parameters in chronological timeline order first or derive
them as a pure fixed-time query; never advance one global smoother as chunks arrive.
All rule combination follows serialized vector order. Each result is validated
and clamped before it becomes an immutable terrain snapshot.

Seeds derive noise tables once. Terrain queries use world coordinates, never
chunk-local coordinates, and never draw from shared mutable RNG state. Structure
events must be identified by stable event ID and evaluated with a finite influence
radius for each intersecting chunk; no neighboring chunk writes may depend on task
order. Audio content hash, preset/config hash, generator version, seed and time
mapping form future world metadata. OS clocks and task order are not world inputs.

## Preview lifetime and budgets

The preview owns no normal world save and never calls vanilla persistence.
Its mesh path uses a fixed extent and bounded pending/result slots. Coalesce
requests; generation revisions distinguish edits/seeks from merely newer audio
frames. A completed old configuration may be discarded, but continuously advancing
playback must not starve every result by cancelling faster than a mesh can finish.
Budget GPU uploads and keep the last complete mesh until a replacement is ready.
Drop must disconnect queues, request cancellation and join workers.

`renderer.rs::PreviewRenderer` supplies original procedural sprite pixels to the
real Horizon `gfx::Renderer::new`. It draws section meshes, camera-relative world
coordinates, depth, culling and HUD through production pipelines. Original pixels
avoid shipping Minecraft data. This adapter does not add a second event loop.

GPU deformation is a separate future path: an explicitly enabled uniform and
shader branch can displace vertices/change tint while retaining mesh buffers.
Topology, collision and persistent blocks do not follow from displacement.
Dynamic density meshes are required for caves/islands and are budgeted separately.

## Persistent-world boundary

The first deliverable is deterministic terrain output, followed by preview.
Actual Minecraft export is a later adapter to `TerrainChunk`, registry block IDs,
heightmaps, `light_chunk`, existing NBT/Anvil writing and the Horizon server-side
`ChunkGenerator` lifecycle. Export must create a new destination only, stage writes,
propagate errors/cancellation and reopen through the normal loader. Exporting an
image, JSON timeline or preview mesh is never labeled as saving a world.

## Schema and compatibility

Configuration JSON must have a version and reject unsupported future versions,
nonfinite numbers, invalid ranges and oversized inputs. Presets, mapping rules,
audio band limits and parameters are validated before use. Adding fields should
use explicit defaults/migrations; future version numbers must not silently load
as old configurations. UI edit history stores bounded snapshots. The mode remains
off unless the user explicitly launches/enables it.

## Underground preview adapter

The height-field preview unions Ecology entrance intervals with
`UndergroundField`'s cached chamber/connector columns. `MineScene` produces bounded
world-space air/material/detail plans before clipping. The native volume applies
caves/geology, mines, village overlays and finally nonpersistent inspection masks.
Rails/webs/lanterns and cave lichen reuse existing terrain/HUD rendering primitives.
SceneOptions includes mine mode and underground view in cache identity. Existing
bounded worker/revision propagation stays unchanged. See [UNDERGROUND](UNDERGROUND.md).
