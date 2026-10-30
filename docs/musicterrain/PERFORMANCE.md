# MusicTerrain performance measurements

## Environment and limits

Initial inspection on 2026-10-29 found a Linux container with no `/dev/dri` GPU
device, display service or preinstalled Rust toolchain. The toolchain/build and
display dependencies are being provisioned during this task. A software GPU or
offscreen run must be identified as such. No Windows/RTX 3060 hardware result,
1080p 60 FPS guarantee or live device latency measurement is established by this
environment inspection.

Record the actual adapter, render resolution, preview extent, preset, audio
time range, number of frames, warmup policy and build profile beside every
benchmark. `64×64` is the initial terrain extent target; `128×128` and `256×256`
are expansion targets, not verified performance claims.

## What to measure

| Metric | Source / interpretation |
|---|---|
| Frame time | End-to-end frame CPU duration and frame interval; FPS alone hides stalls. |
| Renderer CPU time | `Renderer::last_encode`, excluding wait time. |
| GPU/display wait | `Renderer::last_wait`; offscreen pacing differs from presentation. |
| GPU time | `Renderer::gpu_times` where timestamp-query support exists; unavailable is not zero. |
| Mesh generation | Worker wall time per completed revision, with extent and vertex counts. |
| Upload time/bytes | CPU time around `Renderer::upload`, plus `uploaded_bytes`; CPU submission duration is not GPU copy completion. |
| Queued work | Bounded pending/result slot occupancy, coalesced requests and discarded stale revisions. |
| Memory | Peak process RSS and resident section vertex arena bytes; one short run cannot establish absence of leaks. |
| Audio-to-visual latency | Capture/output device clock to presented visual, including DSP hop, smoothing and queued mesh; software-clock timing is a separate diagnostic. |
| Cache hit rate | Only report if actual cache hits/misses are instrumented; no fabricated percentage. |

Use the existing Horizon shared GPU arena and fixed preview extent to bound
resident geometry. Keep queues bounded, retain FFT plans, coalesce music frames
and reject old configuration revisions. If mesh work misses its budget, retain
the last complete geometry and lower extent/update rate. Real GPU deformation
is a separate future optimization and cannot be claimed by feeding time into
an unchanged shader.

## Diagnostic logs

`diagnostics::init(None)` uses stderr only. An explicit log directory enables
file output as well. Files use `musicterrain-v1-<time>-<pid>-<sequence>.log`
with checked creation; existing unrelated files and symlinks are not overwritten.
The writer retains at most four matching module logs, each bounded to 2 MiB,
across repeated starts in that directory. Archive cleanup operates only on that
exact generated filename pattern. Use a dedicated MusicTerrain log directory;
simultaneous processes sharing it also share that retention budget.

The tracing filter defaults to `musicterrain=info` and respects `RUST_LOG`.
The startup record contains a diagnostic session ID; that ID never seeds world
generation. Call sites must record preset/revision/coordinates/counts/times,
not full user file paths, credentials or music contents. File formatting and
rotation are synchronous and must never occur inside the audio callback.
Live file I/O failures are surfaced by the tracing writer; initialization and
invalid destinations return errors to the caller. Size limits do not establish
frame-time or real-time audio safety; keep events at diagnostic cadence.

## Actual results

The measured results below come from actual renderer runs. See TEST_REPORT.md
for the executed checks and their limits.

## Measured native demo (2026-10-30 UTC)

Real adapter reported by the application: `llvmpipe (LLVM 19.1.7, 256 bits)`,
Vulkan, CPU device. Xvfb render/readback resolution 1280×800; cgroup CPU limit 2;
`LP_NUM_THREADS=2`. Preset Bass Mountains, synthesized 12-second audio, explicit
silent software clock. Native dev build with opt-level 2 for MusicTerrain and
Horizon core, ordinary dev renderer. No Cargo compilation ran alongside these
measurements. They are short runs, not a sustained GPU benchmark.

| Extent / run | Frames | Distinct uploaded meshes | Mesh mean / max ms | Sampled peak RSS MiB | Reported one-second FPS windows |
| --- | ---: | ---: | ---: | ---: | --- |
| 64² | 121 | 42 | 19.30 / 32.01 | 196.0 | 23.8–33.2 |
| 64² repeat | 121 | 58 | 45.56 / 133.98 | 196.1 | 15.3–23.0 |
| 128² | 41 | 20 | 38.49 / 95.72 | 189.4 | 12.1–17.2 |
| 256² | 41 | 22 | 110.62 / 529.27 | 190.1 | 2.4–16.6 |

Raw summaries: [64²](verification/preview-64.json),
[64² repeat](verification/preview-64-repeat.json),
[128²](verification/preview-128.json),
[256²](verification/preview-256.json). The application logs its measured frame
intervals (not a capped movement delta); the visible FPS uses the same one-second
frame count. The harness samples Linux `VmHWM` every 50 ms, so its last observation
may precede the final allocation at process exit. Whole-run wall time includes
startup/readback/shutdown and must not be used as a steady FPS measurement.

64² frame means were 30.2–42.0 ms in the first run and 43.5–65.2 ms in the repeat;
maximum interval among those windows was 194 ms. The 256² run exhibited a 3.06 s
stall and CPU upload enqueue time up to 381 ms. Larger extents are functional
experiments, **not stable performance targets on this software adapter**. Keep
64² as the default. CPU scheduling/software rendering variance is visible between
repeated runs; these measurements cannot predict RTX 3060 behavior.

Queues were 0 in sampled performance records and are structurally limited to one
pending request and one result, with one active worker. Generation interval adapts
to the measured worker time (67–500 ms). Height previews cap sample count at 64²;
density preview caps the sampled volume, and larger extents reduce voxel detail.
Generation results carry revision IDs; stale revisions cannot replace current
geometry. The renderer's existing section arena reuses allocations. Cache hits
are tracked as actual repeated completed-snapshot hits, not an estimated rate.

The first fully unoptimized 64² smoke recorded mesh mean 221.6 ms and sampled
peak RSS 201.2 MiB. Targeted dev optimization reduced CPU work; the baseline and
later runs used different load/thread conditions, so no controlled speedup ratio
is claimed. These short runs do not establish absence of leaks, 60 FPS, hardware
latency or Windows behavior. The 256² stall remains an optimization issue.
