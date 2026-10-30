# MusicTerrain UI scope

Horizon uses winit and its own `HudBatch`/quad widgets; its native UI is not egui.
MusicTerrain follows that ownership and does not start a second GUI main loop.
The first preview uses original pixels/type and the existing HUD renderer without
Minecraft resources. See `TASKS.md`/`README.md` for actually working controls;
the full product pages below are a target specification until implemented.

## First runnable preview

Keep the terrain central, playback/seek at the bottom, editable parameter rows
and current values at the right, and visible preset/audio/error state near the
top. Drawing and hit tests must share the `HudBatch::scale` coordinate transform.
Keyboard controls and pointer controls should call the same state transitions.
Pause must freeze audio-time-derived state. Seek must use the file player's
sample cursor and deterministic feature timeline, then invalidate stale work.
Expose FPS/frame time, generation time, pending jobs and actual output-device
status. Never show a successful device state when playback initialization failed.

Every implemented button needs an action and error result; unavailable capture,
loopback, world saving or density modes must be marked unavailable, not represented
as active controls. Audio import should report decode errors without printing
private full paths into default logs. Settings/preset JSON must validate before
application and old state should survive a failed load.

## Editor progression

| Page | Functional requirements | Acceptance evidence |
|---|---|---|
| Live Visualizer | File import, play/pause, seek, offset, spectrum, camera, bounded dynamic preview | Open the real renderer and compare visible terrain at different song times. |
| Music World | Seed/time mapping, deterministic preview, progress/cancel, create-new-only export/reopen | Reopen a real generated save; compare chunk blocks in different request orders. |
| Noise Lab | All supported noise parameters with ranges/defaults/reset, 2D map, 3D terrain, section/density slice | Changing each enabled parameter measurably changes the corresponding output. |
| Mapping Editor | Source/target, ranges, gain/bias, curve, attack/release, clamp, enabled, combine; ordered copy/delete | Serialized graph round-trip and interactive edits affect output. |
| Preset Library | Seven distinguishable presets; save/load/version validation | Repeatable visual differences and configuration round-trip. |
| Settings | Device enumeration/switching, spectrum, offset, budgets, log location | Real device lifecycle and recovery tests; no fake entries. |
| Diagnostics | Frame/mesh/upload/queue/cache/latency/memory statistics, error/log export | Measured values with units and platform context. |

Use existing design tokens (`horizon-theme`) for cool neutrals and the violet
primary action; dense rows are preferable to decorative cards. Undo/redo should
store bounded validated edit snapshots, coalesce slider drags, and increment a
configuration revision. Preset load/reset are edits and must be undoable. Error
notices should persist long enough to read and identify the failed operation.

Windows system loopback is a capability with platform-specific tests. Until the
capture path is implemented, a label explaining unavailability is truthful;
selecting an input device without consuming samples is not capture support.

## Biome selector

Settings includes a **BIOME AUTO** button below the independent scene switches.
It cycles through all ten named biomes and returns to Auto. This changes real
surface/decoration meshes and invalidates generation revisions. Scene selections
are runtime options, not saved terrain presets or undoable parameter edits.
`--biome NAME` initializes the same control. See [BIOMES](BIOMES.md).
