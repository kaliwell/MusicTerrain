//! Bounded preview meshing, using Horizon's production vertex and section formats.
//! Preview voxels never enter the persistent world or its server thread.
use crate::terrain::{GenerationRevision, TerrainField, TerrainSnapshot};
use horizon_client_render::{
    chunk::MIN_Y,
    gfx::SectionKey,
    mesher::{SectionMesh, TerrainVertex},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::JoinHandle,
    time::Instant,
};

pub struct MeshResult {
    pub revision: GenerationRevision,
    /// Section keys use Horizon's MIN_Y-relative section index, not a world Y index.
    pub sections: Vec<(SectionKey, SectionMesh)>,
    pub generation_ms: f32,
    pub checksum: u64,
    pub audio_time: f64,
    pub vertices: usize,
    /// Adaptive horizontal step. Density preview also uses this step vertically.
    pub voxel_step: u32,
    cache_key: CacheKey,
}
#[derive(Clone)]
struct CacheKey {
    snapshot: TerrainSnapshot,
    size: u32,
}
impl CacheKey {
    fn matches(&self, snapshot: &TerrainSnapshot, size: u32) -> bool {
        self.size == size
            && self.snapshot.revision == snapshot.revision
            && self.snapshot.seed == snapshot.seed
            && self.snapshot.parameters == snapshot.parameters
            && (snapshot.parameters.shockwave == 0.0
                || self.snapshot.audio_time == snapshot.audio_time)
    }
}
struct Request {
    snapshot: TerrainSnapshot,
    size: u32,
}
/// One active job, one pending job and one ready result, regardless of UI rate.
pub struct MeshWorker {
    requests: Option<SyncSender<Request>>,
    results: Receiver<MeshResult>,
    revision: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    pending: Arc<AtomicUsize>,
    handle: Option<JoinHandle<()>>,
    pub submitted: u64,
    pub completed: u64,
    pub dropped: u64,
    pub cache_hits: u64,
    last_completed: Option<CacheKey>,
    pub error: Option<String>,
}
impl Default for MeshWorker {
    fn default() -> Self {
        Self::new()
    }
}
impl MeshWorker {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let revision = Arc::new(AtomicU64::new(0));
        let stopping = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(AtomicUsize::new(0));
        let worker_pending = pending.clone();
        let worker_revision = revision.clone();
        let worker_stopping = stopping.clone();
        let worker_busy = busy.clone();
        let spawned = std::thread::Builder::new()
            .name("MusicTerrain mesh".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    worker_pending.fetch_sub(1, Ordering::AcqRel);
                    if worker_stopping.load(Ordering::Acquire) {
                        break;
                    }
                    worker_busy.store(true, Ordering::Release);
                    let result = generate_mesh(&request.snapshot, request.size, || {
                        worker_stopping.load(Ordering::Relaxed)
                            || worker_revision.load(Ordering::Acquire)
                                != request.snapshot.revision.0
                    });
                    worker_busy.store(false, Ordering::Release);
                    if let Some(result) = result {
                        // The renderer owns the other end. A slow frame can never stall shutdown.
                        let _ = results.try_send(result);
                    }
                }
            });
        let (handle, error) = match spawned {
            Ok(handle) => (Some(handle), None),
            Err(error) => (None, Some(format!("Could not start mesh worker: {error}"))),
        };
        Self {
            requests: Some(tx),
            results: rx,
            revision,
            stopping,
            busy,
            pending,
            handle,
            submitted: 0,
            completed: 0,
            dropped: 0,
            cache_hits: 0,
            last_completed: None,
            error,
        }
    }
    /// Returns false if the single pending slot is occupied or the input is invalid.
    /// A revision change invalidates old work even while that pending slot is full.
    pub fn submit(&mut self, snapshot: TerrainSnapshot, size: u32) -> bool {
        self.revision.store(snapshot.revision.0, Ordering::Release);
        if !valid_size(size)
            || snapshot.parameters.validate().is_err()
            || !snapshot.audio_time.is_finite()
        {
            self.dropped += 1;
            return false;
        }
        if self
            .last_completed
            .as_ref()
            .is_some_and(|key| key.matches(&snapshot, size))
        {
            self.cache_hits += 1;
            return false;
        }
        let Some(tx) = &self.requests else {
            return false;
        };
        self.pending.fetch_add(1, Ordering::AcqRel);
        match tx.try_send(Request { snapshot, size }) {
            Ok(()) => {
                self.submitted += 1;
                true
            }
            Err(TrySendError::Full(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                self.dropped += 1;
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                self.error = Some("Mesh worker stopped".into());
                self.dropped += 1;
                false
            }
        }
    }
    pub fn poll(&mut self, current: GenerationRevision) -> Option<MeshResult> {
        while let Ok(result) = self.results.try_recv() {
            if result.revision == current {
                self.completed += 1;
                self.last_completed = Some(result.cache_key.clone());
                return Some(result);
            }
            self.dropped += 1;
        }
        None
    }
    /// Waiting jobs (0 or 1), excluding the currently running job.
    pub fn queued_jobs(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
}
impl Drop for MeshWorker {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.requests.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
fn valid_size(size: u32) -> bool {
    (16..=256).contains(&size) && size.is_multiple_of(16)
}

/// Synchronous entry for deterministic checks and fixed-timestamp captures.
/// At most 64² height samples or 49,152 density cells are retained.
pub fn mesh_snapshot(snapshot: &TerrainSnapshot, size: u32) -> Option<MeshResult> {
    generate_mesh(snapshot, size, || false)
}
fn generate_mesh(
    snapshot: &TerrainSnapshot,
    size: u32,
    cancelled: impl Fn() -> bool,
) -> Option<MeshResult> {
    if !valid_size(size)
        || snapshot.parameters.validate().is_err()
        || !snapshot.audio_time.is_finite()
    {
        return None;
    }
    let start = Instant::now();
    let field = TerrainField::new(snapshot.seed);
    let mut builder = MeshBuilder::new(snapshot);
    let density =
        snapshot.parameters.island_strength > 0.1 || snapshot.parameters.cave_threshold < 0.5;
    let step = if density {
        size.div_ceil(32).next_power_of_two().max(2)
    } else {
        size.div_ceil(64).next_power_of_two()
    };
    if density {
        density_mesh(&mut builder, &field, size, step, &cancelled)?;
    } else {
        height_mesh(&mut builder, &field, size, step, &cancelled)?;
    }
    if cancelled() {
        return None;
    }
    let vertices = builder
        .sections
        .values()
        .map(|section| section.solid.len())
        .sum();
    let checksum = builder.checksum;
    let sections = builder
        .sections
        .into_iter()
        .map(|(key, mut section)| {
            section.sort_solid();
            (key, section)
        })
        .collect();
    Some(MeshResult {
        revision: snapshot.revision,
        sections,
        generation_ms: start.elapsed().as_secs_f32() * 1000.0,
        checksum,
        audio_time: snapshot.audio_time,
        vertices,
        voxel_step: step,
        cache_key: CacheKey {
            snapshot: snapshot.clone(),
            size,
        },
    })
}
fn wave(snapshot: &TerrainSnapshot, x: f64, z: f64) -> f32 {
    if snapshot.parameters.shockwave == 0.0 {
        return 0.0;
    }
    let radius = x.hypot(z) as f32;
    let cycle = snapshot.audio_time.rem_euclid(std::f64::consts::TAU / 5.0) as f32;
    let phase = radius * 0.27 - cycle * 5.0;
    snapshot.parameters.shockwave * phase.sin() * (-radius * 0.015).exp()
}
fn height_mesh(
    builder: &mut MeshBuilder<'_>,
    field: &TerrainField,
    size: u32,
    step: u32,
    cancelled: &impl Fn() -> bool,
) -> Option<()> {
    let n = (size / step) as usize;
    let origin = -(size as i32) / 2;
    let mut heights = vec![0i32; n * n];
    for z in 0..n {
        if cancelled() {
            return None;
        }
        for x in 0..n {
            let wx = f64::from(origin) + x as f64 * f64::from(step);
            let wz = f64::from(origin) + z as f64 * f64::from(step);
            heights[z * n + x] = (field.height(wx, wz, &builder.snapshot.parameters)
                - builder.snapshot.parameters.density_threshold
                + wave(builder.snapshot, wx, wz))
            .round()
            .clamp(1.0, 128.0) as i32;
        }
    }
    let get = |x: i32, z: i32| -> i32 {
        if x < 0 || z < 0 || x >= n as i32 || z >= n as i32 {
            0
        } else {
            heights[z as usize * n + x as usize]
        }
    };
    for z in 0..n as i32 {
        if cancelled() {
            return None;
        }
        for x in 0..n as i32 {
            let h = get(x, z);
            let wx = origin + x * step as i32;
            let wz = origin + z * step as i32;
            builder.face_extent([wx, h - 1, wz], [step as i32, 1, step as i32], Face::Up);
            builder.face_extent([wx, 0, wz], [step as i32, 1, step as i32], Face::Down);
            for (dx, dz, face) in [
                (1, 0, Face::East),
                (-1, 0, Face::West),
                (0, 1, Face::South),
                (0, -1, Face::North),
            ] {
                let mut y = get(x + dx, z + dz);
                while y < h {
                    // Greedily merge vertical faces within one Horizon section.
                    let end = h.min((y.div_euclid(16) + 1) * 16);
                    builder.face_extent([wx, y, wz], [step as i32, end - y, step as i32], face);
                    y = end;
                }
            }
        }
    }
    Some(())
}
fn density_mesh(
    builder: &mut MeshBuilder<'_>,
    field: &TerrainField,
    size: u32,
    step: u32,
    cancelled: &impl Fn() -> bool,
) -> Option<()> {
    let n = (size / step) as usize;
    let vertical = (96 / step) as usize;
    let origin = -(size as i32) / 2;
    let mut cells = vec![false; n * n * vertical];
    for y in 0..vertical {
        if cancelled() {
            return None;
        }
        for z in 0..n {
            for x in 0..n {
                let wx = f64::from(origin) + (x as f64 + 0.5) * f64::from(step);
                let wz = f64::from(origin) + (z as f64 + 0.5) * f64::from(step);
                let wy =
                    (y as f64 + 0.5) * f64::from(step) - f64::from(wave(builder.snapshot, wx, wz));
                cells[(y * n + z) * n + x] =
                    field.density(wx, wy, wz, &builder.snapshot.parameters) > 0.0;
            }
        }
    }
    let solid = |x: i32, y: i32, z: i32| -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && x < n as i32
            && z < n as i32
            && y < vertical as i32
            && cells[(y as usize * n + z as usize) * n + x as usize]
    };
    for y in 0..vertical as i32 {
        if cancelled() {
            return None;
        }
        for z in 0..n as i32 {
            for x in 0..n as i32 {
                if !solid(x, y, z) {
                    continue;
                }
                for (dx, dy, dz, face) in [
                    (1, 0, 0, Face::East),
                    (-1, 0, 0, Face::West),
                    (0, 1, 0, Face::Up),
                    (0, -1, 0, Face::Down),
                    (0, 0, 1, Face::South),
                    (0, 0, -1, Face::North),
                ] {
                    if !solid(x + dx, y + dy, z + dz) {
                        builder.face(
                            [
                                origin + x * step as i32,
                                y * step as i32,
                                origin + z * step as i32,
                            ],
                            step as i32,
                            face,
                        );
                    }
                }
            }
        }
    }
    Some(())
}
#[derive(Clone, Copy)]
enum Face {
    East,
    West,
    Up,
    Down,
    South,
    North,
}
impl Face {
    fn corners(self) -> [[i32; 3]; 4] {
        match self {
            Self::East => [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
            Self::West => [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]],
            Self::Up => [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
            Self::Down => [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]],
            Self::South => [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]],
            Self::North => [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]],
        }
    }
    fn shade(self) -> f32 {
        match self {
            Self::Up => 1.0,
            Self::Down => 0.45,
            Self::East | Self::West => 0.78,
            Self::South | Self::North => 0.62,
        }
    }
}
struct MeshBuilder<'a> {
    snapshot: &'a TerrainSnapshot,
    sections: BTreeMap<SectionKey, SectionMesh>,
    checksum: u64,
}
impl<'a> MeshBuilder<'a> {
    fn new(snapshot: &'a TerrainSnapshot) -> Self {
        Self {
            snapshot,
            sections: BTreeMap::new(),
            checksum: 0xcbf2_9ce4_8422_2325,
        }
    }
    fn face(&mut self, anchor: [i32; 3], step: i32, face: Face) {
        self.face_extent(anchor, [step; 3], face);
    }
    fn face_extent(&mut self, anchor: [i32; 3], extent: [i32; 3], face: Face) {
        let key = (
            anchor[0].div_euclid(16),
            (anchor[1] - MIN_Y).div_euclid(16),
            anchor[2].div_euclid(16),
        );
        let origin = [key.0 * 16, MIN_Y + key.1 * 16, key.2 * 16];
        let shade = face.shade();
        let altitude = (anchor[1] as f32 / 96.0).clamp(0., 1.);
        let shift = (self.snapshot.parameters.color_shift * 0.5 + 0.5).clamp(0., 1.);
        let tint = [
            65.0 + altitude * 95.0 + shift * 30.0,
            165.0 - altitude * 20.0 - shift * 45.0,
            140.0 + altitude * 45.0 + shift * 35.0,
        ];
        let color = u32::from_le_bytes([
            (tint[0] * shade) as u8,
            (tint[1] * shade) as u8,
            (tint[2] * shade) as u8,
            255,
        ]);
        let vertices = face.corners().map(|p| {
            std::array::from_fn::<_, 3, _>(|axis| {
                (anchor[axis] + p[axis] * extent[axis] - origin[axis]) as f32
            })
        });
        for coordinate in [key.0, key.1, key.2] {
            self.checksum = (self.checksum ^ coordinate as u64).wrapping_mul(0x100_0000_01b3);
        }
        let section = self.sections.entry(key).or_default();
        for (pos, uv) in vertices
            .into_iter()
            .zip([[0., 0.], [0., 1.], [1., 1.], [1., 0.]])
        {
            for bits in pos.map(f32::to_bits).into_iter().chain([color]) {
                self.checksum = (self.checksum ^ u64::from(bits)).wrapping_mul(0x100_0000_01b3);
            }
            section
                .solid
                .push(TerrainVertex::new(pos, uv, 0, color, 0xf0f0));
        }
    }
}
