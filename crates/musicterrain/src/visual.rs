//! Bounded preview meshing, using Horizon's production vertex and section formats.
//! Preview voxels never enter the persistent world or its server thread.
#[cfg(not(feature = "native"))]
use crate::scene_mesh::{MIN_Y, SectionKey, SectionMesh, TerrainVertex};
use crate::{
    caves::cave_slice,
    ecology::{CaveColumn, DecorationKind, Ecology, SceneBiome, SurfaceKind},
    materials::Material,
    mineshaft::{MineMode, MineScene, UndergroundView},
    terrain::{GenerationRevision, TerrainField, TerrainSnapshot},
    underground::{UndergroundBiome, UndergroundColumn, UndergroundField},
    village::{VillageMode, VillageScene},
};
#[cfg(feature = "native")]
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
};

pub const SCENE_MIN_Y: i32 = MIN_Y;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneOptions {
    pub shape: crate::shape::TerrainShape,
    pub full_detail: bool,
    pub vegetation: bool,
    pub water: bool,
    pub caves: bool,
    pub biome: Option<SceneBiome>,
    pub villages: VillageMode,
    pub mines: MineMode,
    pub underground_view: UndergroundView,
}
impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            shape: Default::default(),
            full_detail: true,
            vegetation: true,
            water: true,
            caves: true,
            biome: None,
            villages: VillageMode::Auto,
            mines: MineMode::Auto,
            underground_view: UndergroundView::Surface,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStats {
    pub trees: u32,
    pub plants: u32,
    pub cave_faces: u32,
    pub water_quads: u32,
    pub material_quads: [u32; Material::ALL.len()],
    pub solid_voxels: u32,
    pub carved_voxels: u32,
    pub villages: u32,
    pub houses: u32,
    pub farms: u32,
    pub mines: u32,
    pub supports: u32,
    pub rails: u32,
}

impl Default for SceneStats {
    fn default() -> Self {
        Self {
            trees: 0,
            plants: 0,
            cave_faces: 0,
            water_quads: 0,
            material_quads: [0; Material::ALL.len()],
            solid_voxels: 0,
            carved_voxels: 0,
            villages: 0,
            houses: 0,
            farms: 0,
            mines: 0,
            supports: 0,
            rails: 0,
        }
    }
}

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
    pub scene: SceneStats,
    pub section_hashes: BTreeMap<SectionKey, u64>,
    cache_key: CacheKey,
}
#[derive(Clone)]
struct CacheKey {
    snapshot: TerrainSnapshot,
    size: u32,
    options: SceneOptions,
}
impl CacheKey {
    fn matches(&self, snapshot: &TerrainSnapshot, size: u32, options: SceneOptions) -> bool {
        self.size == size
            && self.options == options
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
    options: SceneOptions,
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
                    let result =
                        generate_mesh(&request.snapshot, request.size, request.options, || {
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
        self.submit_with_options(snapshot, size, SceneOptions::default())
    }
    pub fn submit_with_options(
        &mut self,
        snapshot: TerrainSnapshot,
        size: u32,
        options: SceneOptions,
    ) -> bool {
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
            .is_some_and(|key| key.matches(&snapshot, size, options))
        {
            self.cache_hits += 1;
            return false;
        }
        let Some(tx) = &self.requests else {
            return false;
        };
        self.pending.fetch_add(1, Ordering::AcqRel);
        match tx.try_send(Request {
            snapshot,
            size,
            options,
        }) {
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

/// Deterministic, isolated preview scene at an exact audio timestamp.
pub fn mesh_snapshot(snapshot: &TerrainSnapshot, size: u32) -> Option<MeshResult> {
    mesh_snapshot_with_options(snapshot, size, &SceneOptions::default())
}
pub fn mesh_snapshot_with_options(
    snapshot: &TerrainSnapshot,
    size: u32,
    options: &SceneOptions,
) -> Option<MeshResult> {
    generate_mesh(snapshot, size, *options, || false)
}
fn generate_mesh(
    snapshot: &TerrainSnapshot,
    size: u32,
    options: SceneOptions,
    cancelled: impl Fn() -> bool + Sync,
) -> Option<MeshResult> {
    if !valid_size(size)
        || snapshot.parameters.validate().is_err()
        || !snapshot.audio_time.is_finite()
    {
        return None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();
    let field = TerrainField::new(snapshot.seed);
    let ecology = Ecology::new(snapshot.seed).with_biome(options.biome);
    let mut builder = MeshBuilder::new(snapshot);
    let density = snapshot.parameters.island_strength > 0.1;
    let step = if options.full_detail {
        1
    } else if density {
        size.div_ceil(32).next_power_of_two().max(2)
    } else {
        size.div_ceil(64).next_power_of_two()
    };
    let mut volume = Volume::generate(
        snapshot, &field, &ecology, size, step, density, options, &cancelled,
    )?;
    // A clipped query may intersect a village whose full plan extends another
    // 84 blocks beyond it (mines). Sample pond shape over full structure plans.
    let structure_ponds = if options.water
        && (options.villages != VillageMode::Off
            || (options.caves && options.mines != MineMode::Off))
        && !density
        && step == 1
    {
        ponds(snapshot, &field, volume.origin - 96, size + 192)
    } else {
        Vec::new()
    };
    let villages = if density || step > 1 {
        VillageScene::default()
    } else {
        VillageScene::generate(
            snapshot.seed,
            [volume.origin; 2],
            size,
            options.villages,
            &ecology,
            |x, z| {
                let h = base_height(&field, snapshot, x, z);
                if options.water {
                    let (ground, water) = pond_height(h, x, z, &structure_ponds);
                    if ground < water {
                        f32::NAN
                    } else {
                        ground as f32
                    }
                } else {
                    h as f32
                }
            },
            &cancelled,
        )?
    };
    let mines = if options.caves && !density && step == 1 {
        MineScene::generate(
            snapshot.seed,
            [volume.origin; 2],
            size,
            options.mines,
            |x, z| {
                let h = base_height(&field, snapshot, x, z);
                if options.water {
                    pond_height(h, x, z, &structure_ponds).0 as f32
                } else {
                    h as f32
                }
            },
            &cancelled,
        )?
    } else {
        MineScene::default()
    };
    volume.apply_mines(&mines);
    volume.apply_village(&villages, options.water);
    volume.apply_view(options.underground_view);
    volume.clip_shape(options.shape);
    builder.scene.mines = mines.mines;
    builder.scene.supports = mines.supports;

    builder.scene.villages = villages.villages;
    builder.scene.houses = villages.houses;
    builder.scene.farms = villages.farms;
    builder.scene.solid_voxels = volume
        .cells
        .iter()
        .filter(|m| **m != 0 && **m != encode(Material::Water))
        .count() as u32;
    builder.scene.carved_voxels = volume.carved.iter().filter(|v| **v).count() as u32;
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4);
    if size >= 128 && threads > 1 {
        let stripes = volume.n.div_ceil(threads);
        let parts = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..volume.n)
                .step_by(stripes)
                .map(|start| {
                    let volume = &volume;
                    let cancelled = &cancelled;
                    scope.spawn(move || {
                        let mut part = MeshBuilder::new(snapshot);
                        volume.mesh_rows(
                            &mut part,
                            cancelled,
                            start..(start + stripes).min(volume.n),
                        )?;
                        Some(part)
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|job| job.join().ok().flatten())
                .collect::<Option<Vec<_>>>()
        })?;
        for part in parts {
            builder.scene.cave_faces += part.scene.cave_faces;
            builder.scene.water_quads += part.scene.water_quads;
            for (a, b) in builder
                .scene
                .material_quads
                .iter_mut()
                .zip(part.scene.material_quads)
            {
                *a += b;
            }
            for (key, mesh) in part.sections {
                match builder.sections.entry(key) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(mesh);
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        entry.get_mut().solid.extend(mesh.solid);
                        entry.get_mut().translucent.extend(mesh.translucent);
                    }
                }
            }
        }
    } else {
        volume.mesh(&mut builder, &cancelled)?;
    }
    if options.vegetation && options.underground_view != UndergroundView::Underground {
        decorations(
            &mut builder,
            &volume,
            &field,
            &ecology,
            options,
            &villages,
            &cancelled,
        )?;
        for crop in &villages.crops {
            if options.underground_view.visible(*crop)
                && options
                    .shape
                    .contains(crop[0] as f32 + 0.5, crop[2] as f32 + 0.5, size)
            {
                builder.plant(*crop, Material::Wheat);
                builder.scene.plants += 1;
            }
        }
    }
    for &(p, axis) in &mines.rails {
        if options.underground_view.visible(p)
            && volume.world_get(p) == 0
            && volume.world_get([p[0], p[1] - 1, p[2]]) != 0
        {
            builder.horizontal_detail(p, Material::Rail, axis == 0);
            builder.scene.rails += 1;
        }
    }
    for p in &mines.webs {
        if options.underground_view.visible(*p)
            && volume.world_get(*p) == 0
            && options
                .shape
                .contains(p[0] as f32 + 0.5, p[2] as f32 + 0.5, size)
        {
            builder.plant(*p, Material::Cobweb);
        }
    }
    for p in &mines.lamps {
        if options.underground_view.visible(*p)
            && volume.world_get(*p) == 0
            && options
                .shape
                .contains(p[0] as f32 + 0.5, p[2] as f32 + 0.5, size)
        {
            builder.detail_box(*p, [0.25, 0.22, 0.25], [0.5, 0.65, 0.5], Material::Lantern);
        }
    }
    if cancelled() {
        return None;
    }
    let vertices = builder
        .sections
        .values()
        .map(|s| s.solid.len() + s.translucent.len())
        .sum();
    let scene = builder.scene;
    let sections: Vec<_> = builder
        .sections
        .into_iter()
        .map(|(key, mut mesh)| {
            mesh.sort_solid();
            (key, mesh)
        })
        .collect();
    use std::hash::{Hash, Hasher};
    let section_hashes: BTreeMap<_, _> = sections
        .iter()
        .map(|(key, mesh)| (*key, mesh.fingerprint()))
        .collect();
    let mut identity = std::collections::hash_map::DefaultHasher::new();
    section_hashes.hash(&mut identity);
    let checksum = identity.finish();
    Some(MeshResult {
        section_hashes,
        revision: snapshot.revision,
        sections,
        generation_ms: {
            #[cfg(not(target_arch = "wasm32"))]
            {
                start.elapsed().as_secs_f32() * 1000.0
            }
            #[cfg(target_arch = "wasm32")]
            {
                0.0
            }
        },
        checksum,
        audio_time: snapshot.audio_time,
        vertices,
        voxel_step: step,
        scene,
        cache_key: CacheKey {
            snapshot: snapshot.clone(),
            size,
            options,
        },
    })
}

#[cfg(test)]
mod preview_regressions {
    use super::*;
    use crate::{shape::TerrainShape, terrain::TerrainParameters};

    fn flat(height: f32) -> TerrainSnapshot {
        TerrainSnapshot {
            revision: GenerationRevision(1),
            seed: 42,
            audio_time: 0.0,
            parameters: TerrainParameters {
                base_height: height,
                amplitude: 0.0,
                detail: 0.0,
                ..Default::default()
            },
        }
    }
    fn bare() -> SceneOptions {
        SceneOptions {
            vegetation: false,
            water: false,
            villages: VillageMode::Off,
            mines: MineMode::Off,
            ..Default::default()
        }
    }
    #[test]
    fn parallel_fill_preserves_every_material_cave_and_surface_sample() {
        let mut request = flat(40.0);
        request.parameters.cave_w = 1.25;
        request.parameters.cave_threshold = 0.197;
        let field = TerrainField::new(request.seed);
        let ecology = Ecology::new(request.seed);
        let options = SceneOptions {
            shape: TerrainShape::Cylinder,
            ..bare()
        };
        for (density, step) in [(false, 1), (true, 4)] {
            let serial = Volume::generate_with_workers(
                &request,
                &field,
                &ecology,
                128,
                step,
                density,
                options,
                &|| false,
                1,
            )
            .unwrap();
            let parallel = Volume::generate_with_workers(
                &request,
                &field,
                &ecology,
                128,
                step,
                density,
                options,
                &|| false,
                4,
            )
            .unwrap();
            assert_eq!(parallel.cells, serial.cells);
            assert_eq!(parallel.carved, serial.carved);
            assert_eq!(parallel.surfaces, serial.surfaces);
        }
    }
    #[test]
    fn parallel_full_detail_mesh_matches_serial_vertices_uvs_and_cave_shading() {
        let mut request = flat(40.0);
        request.parameters.cave_w = 1.25;
        for shape in [TerrainShape::Square, TerrainShape::Cylinder] {
            let options = SceneOptions { shape, ..bare() };
            let field = TerrainField::new(request.seed);
            let ecology = Ecology::new(request.seed);
            let mut volume =
                Volume::generate(&request, &field, &ecology, 128, 1, false, options, &|| {
                    false
                })
                .unwrap();
            volume.clip_shape(shape);
            let mut serial = MeshBuilder::new(&request);
            volume.mesh(&mut serial, &|| false).unwrap();
            let expected: BTreeMap<_, _> = serial
                .sections
                .iter_mut()
                .map(|(key, mesh)| {
                    mesh.sort_solid();
                    (*key, mesh.fingerprint())
                })
                .collect();
            let parallel = mesh_snapshot_with_options(&request, 128, &options).unwrap();
            assert_eq!(parallel.voxel_step, 1);
            assert_eq!(parallel.section_hashes, expected);
            assert_eq!(parallel.scene.material_quads, serial.scene.material_quads);
            assert_eq!(parallel.scene.cave_faces, serial.scene.cave_faces);
        }
    }
    #[test]
    fn w_evolves_caves_without_translating_the_surface_or_breaking_floor_and_roof() {
        let mut request = flat(60.0);
        request.parameters.cave_threshold = 0.0;
        let field = TerrainField::new(42);
        let ecology = Ecology::new(42);
        let original =
            Volume::generate(&request, &field, &ecology, 32, 1, false, bare(), &|| false).unwrap();
        request.parameters.cave_w = 1.25;
        let moved =
            Volume::generate(&request, &field, &ecology, 32, 1, false, bare(), &|| false).unwrap();
        let repeat =
            Volume::generate(&request, &field, &ecology, 32, 1, false, bare(), &|| false).unwrap();
        assert_eq!(moved.cells, repeat.cells);
        assert_eq!(original.surfaces, moved.surfaces);
        assert!(
            original
                .carved
                .iter()
                .zip(&moved.carved)
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        for z in 0..32 {
            for x in 0..32 {
                for y in 0..3 {
                    assert_ne!(moved.get(x, y, z), 0);
                }
                for y in 57..60 {
                    if original.get(x, y, z) != 0 {
                        assert_ne!(moved.get(x, y, z), 0);
                    }
                }
            }
        }
        let a = cave_slice([10.0, 20.0], 5, -8, 60, 0.5, 42, 0.99999);
        let b = cave_slice([10.0, 20.0], 5, -8, 60, 0.5, 42, 1.00001);
        assert!((a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001);
    }
    #[test]
    fn low_cave_threshold_preserves_full_height_floor_and_existing_roof() {
        let mut request = flat(100.0);
        let field = TerrainField::new(42);
        let ecology = Ecology::new(42);
        let mut previous_carved = 0;
        for threshold in [1.0, 0.78, 0.5001, 0.4999, 0.197, 0.0] {
            request.parameters.cave_threshold = threshold;
            let v = Volume::generate(&request, &field, &ecology, 32, 1, false, bare(), &|| false)
                .unwrap();
            let carved = v.carved.iter().filter(|c| **c).count();
            assert!(carved >= previous_carved);
            previous_carved = carved;
            for z in 0..32 {
                for x in 0..32 {
                    for y in 0..3 {
                        assert_ne!(v.get(x, y, z), 0, "floor at {threshold}");
                    }
                    let original = ecology.cave_column(x - 16, z - 16, 100.0);
                    for y in 97..100 {
                        if !original.contains(y) {
                            assert_ne!(v.get(x, y, z), 0, "roof at {threshold}");
                        }
                    }
                }
            }
            assert!(v.cells.iter().filter(|c| **c != 0).count() > 32 * 32 * 50);
            let mesh = mesh_snapshot_with_options(&request, 32, &bare()).unwrap();
            assert_eq!(mesh.voxel_step, 1);
            assert!(
                mesh.sections.iter().any(|((_, sy, _), _)| *sy >= 10),
                "upper terrain must not be cut at Y96"
            );
        }
        assert!(previous_carved > 0);
    }
    #[test]
    fn shaped_volumes_are_clipped_at_every_height_and_keep_caves() {
        let request = flat(40.0);
        let field = TerrainField::new(42);
        let ecology = Ecology::new(42);
        let mut counts = Vec::new();
        for shape in TerrainShape::ALL {
            let options = SceneOptions { shape, ..bare() };
            let mut v =
                Volume::generate(&request, &field, &ecology, 32, 1, false, options, &|| false)
                    .unwrap();
            // Structure writes outside the footprint must also be removed by final clipping.
            let i = v.index(0, 10, 0);
            v.cells[i] = encode(Material::Stone);
            v.clip_shape(shape);
            for z in 0..32 {
                for x in 0..32 {
                    let inside = shape.contains(x as f32 - 15.5, z as f32 - 15.5, 32);
                    for y in 0..v.vertical as i32 {
                        if !inside {
                            assert_eq!(v.get(x, y, z), 0);
                        }
                    }
                    if inside {
                        assert_ne!(v.get(x, 0, z), 0);
                    }
                }
            }
            let mesh = mesh_snapshot_with_options(&request, 32, &options).unwrap();
            assert!(mesh.scene.carved_voxels > 0);
            counts.push(mesh.scene.solid_voxels);
        }
        counts.sort();
        counts.dedup();
        assert_eq!(counts.len(), 4);
    }
    #[test]
    fn full_detail_stays_one_block_at_all_extents_and_adaptive_remains_available() {
        let request = flat(10.0);
        let full = SceneOptions {
            caves: false,
            ..bare()
        };
        for size in [64, 128, 256] {
            let mesh = mesh_snapshot_with_options(&request, size, &full).unwrap();
            assert_eq!(mesh.voxel_step, 1);
            assert_eq!(mesh.scene.solid_voxels, size * size * 10);
            let coarse = mesh_snapshot_with_options(
                &request,
                size,
                &SceneOptions {
                    full_detail: false,
                    ..full
                },
            )
            .unwrap();
            assert_eq!(coarse.voxel_step, size / 64);
        }
    }
}

fn wave(snapshot: &TerrainSnapshot, x: f64, z: f64) -> f32 {
    if snapshot.parameters.shockwave == 0. {
        return 0.;
    }
    let radius = x.hypot(z) as f32;
    let cycle = snapshot.audio_time.rem_euclid(std::f64::consts::TAU / 5.) as f32;
    snapshot.parameters.shockwave * (radius * 0.27 - cycle * 5.).sin() * (-radius * 0.015).exp()
}
fn base_height(field: &TerrainField, snapshot: &TerrainSnapshot, x: i32, z: i32) -> i32 {
    (field.height(f64::from(x), f64::from(z), &snapshot.parameters)
        - snapshot.parameters.density_threshold
        + wave(snapshot, f64::from(x), f64::from(z)))
    .round()
    .clamp(1., 128.) as i32
}
fn hash(seed: i64, x: i32, y: i32, z: i32) -> u64 {
    let mut v = (seed as u64)
        ^ ((x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
        ^ ((y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9))
        ^ ((z as u64).wrapping_mul(0x94d0_49bb_1331_11eb));
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    v ^ (v >> 31)
}
#[derive(Clone, Copy)]
struct Pond {
    x: i32,
    z: i32,
    radius: f32,
    level: i32,
}
fn ponds(snapshot: &TerrainSnapshot, field: &TerrainField, origin: i32, size: u32) -> Vec<Pond> {
    let mut out = Vec::new();
    for cz in (origin - 9).div_euclid(48)..=(origin + size as i32 + 9).div_euclid(48) {
        for cx in (origin - 9).div_euclid(48)..=(origin + size as i32 + 9).div_euclid(48) {
            let id = hash(snapshot.seed, cx, 0x504f, cz);
            let x = cx * 48 + 11 + (id % 5) as i32;
            let z = cz * 48 + 31 + ((id >> 8) % 7) as i32;
            out.push(Pond {
                x,
                z,
                radius: 6. + ((id >> 16) % 3) as f32,
                level: (base_height(field, snapshot, x, z) - 2).clamp(4, 112),
            });
        }
    }
    out
}
fn pond_height(mut height: i32, x: i32, z: i32, ponds: &[Pond]) -> (i32, i32) {
    let mut water = 8;
    for pond in ponds {
        let distance =
            ((x - pond.x).pow(2) + (z - pond.z).pow(2)) as f32 / (pond.radius * pond.radius);
        if distance < 1. {
            let basin = pond.level - 3 + (distance * distance * 4.).floor() as i32;
            height = height.min(basin.max(1));
            water = water.max(pond.level);
        }
    }
    (height, water)
}
fn encode(material: Material) -> u8 {
    material.layer() as u8 + 1
}
fn decode(value: u8) -> Option<Material> {
    value
        .checked_sub(1)
        .and_then(|i| Material::ALL.get(usize::from(i)))
        .copied()
}
fn surface_material(kind: SurfaceKind) -> Material {
    match kind {
        SurfaceKind::Grass => Material::GrassTop,
        SurfaceKind::Dirt => Material::Dirt,
        SurfaceKind::Stone => Material::Stone,
        SurfaceKind::Sand => Material::Sand,
        SurfaceKind::Snow => Material::Snow,
        SurfaceKind::RedSand => Material::RedSand,
        SurfaceKind::Terracotta => Material::Terracotta,
        SurfaceKind::Podzol => Material::Podzol,
        SurfaceKind::Clay => Material::Clay,
    }
}
struct Volume {
    origin: i32,
    size: u32,
    n: usize,
    vertical: usize,
    step: i32,
    ystep: i32,
    cells: Vec<u8>,
    carved: Vec<bool>,
    surfaces: Vec<i32>,
    ponds: Vec<Pond>,
    view: UndergroundView,
    caves: bool,
}
impl Volume {
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        // Generation, cave palettes and wall merging all walk vertical columns.
        // Keep a column contiguous instead of jumping n*n bytes for every Y.
        (z * self.n + x) * self.vertical + y
    }
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if x < 0
            || y < 0
            || z < 0
            || x >= self.n as i32
            || z >= self.n as i32
            || y >= self.vertical as i32
        {
            0
        } else {
            self.cells[self.index(x as usize, y as usize, z as usize)]
        }
    }
    fn is_carved(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && x < self.n as i32
            && z < self.n as i32
            && y < self.vertical as i32
            && self.carved[self.index(x as usize, y as usize, z as usize)]
    }
    fn world_get(&self, p: [i32; 3]) -> u8 {
        let x = p[0] - self.origin;
        let z = p[2] - self.origin;
        if x < 0 || z < 0 || x >= self.size as i32 || z >= self.size as i32 {
            0
        } else {
            self.get(x / self.step, p[1].div_euclid(self.ystep), z / self.step)
        }
    }
    fn world_height(&self, x: i32, z: i32) -> Option<i32> {
        let x = x - self.origin;
        let z = z - self.origin;
        if x < 0 || z < 0 || x >= self.size as i32 || z >= self.size as i32 {
            None
        } else {
            Some(self.surfaces[(z / self.step) as usize * self.n + (x / self.step) as usize])
        }
    }
    fn apply_mines(&mut self, scene: &MineScene) {
        for p in &scene.air {
            let index = self.index(
                ((p[0] - self.origin) / self.step) as usize,
                (p[1] / self.ystep) as usize,
                ((p[2] - self.origin) / self.step) as usize,
            );
            self.cells[index] = 0;
            self.carved[index] = true;
        }
        for (p, material) in &scene.blocks {
            let index = self.index(
                ((p[0] - self.origin) / self.step) as usize,
                (p[1] / self.ystep) as usize,
                ((p[2] - self.origin) / self.step) as usize,
            );
            self.cells[index] = encode(*material);
            self.carved[index] = false;
        }
    }
    fn clip_shape(&mut self, shape: crate::shape::TerrainShape) {
        if shape == crate::shape::TerrainShape::Square {
            return;
        }
        for z in 0..self.n {
            for x in 0..self.n {
                if !shape.contains(
                    self.origin as f32 + (x as f32 + 0.5) * self.step as f32,
                    self.origin as f32 + (z as f32 + 0.5) * self.step as f32,
                    self.size,
                ) {
                    for y in 0..self.vertical {
                        let i = self.index(x, y, z);
                        self.cells[i] = 0;
                        self.carved[i] = false;
                    }
                }
            }
        }
    }
    fn apply_view(&mut self, view: UndergroundView) {
        if view == UndergroundView::Surface {
            return;
        }
        for z in 0..self.n {
            for x in 0..self.n {
                for y in 0..self.vertical {
                    let p = [
                        self.origin + x as i32 * self.step,
                        y as i32 * self.ystep,
                        self.origin + z as i32 * self.step,
                    ];
                    if !view.visible(p) {
                        let index = self.index(x, y, z);
                        self.cells[index] = 0;
                    }
                }
            }
        }
    }
    fn apply_village(&mut self, scene: &VillageScene, water: bool) {
        for (p, material) in &scene.blocks {
            let x = (p[0] - self.origin) / self.step;
            let z = (p[2] - self.origin) / self.step;
            let y = p[1] / self.ystep;
            if x >= 0
                && z >= 0
                && y >= 0
                && x < self.n as i32
                && z < self.n as i32
                && y < self.vertical as i32
            {
                let index = self.index(x as usize, y as usize, z as usize);
                self.cells[index] = encode(if !water && *material == Material::Water {
                    Material::Clay
                } else {
                    *material
                });
                self.carved[index] = false;
            }
        }
    }
    fn generate(
        snapshot: &TerrainSnapshot,
        field: &TerrainField,
        ecology: &Ecology,
        size: u32,
        step: u32,
        density: bool,
        options: SceneOptions,
        cancelled: &(impl Fn() -> bool + Sync),
    ) -> Option<Self> {
        let workers = if size >= 128 {
            std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min(4)
        } else {
            1
        };
        Self::generate_with_workers(
            snapshot, field, ecology, size, step, density, options, cancelled, workers,
        )
    }
    fn generate_with_workers(
        snapshot: &TerrainSnapshot,
        field: &TerrainField,
        ecology: &Ecology,
        size: u32,
        step: u32,
        density: bool,
        options: SceneOptions,
        cancelled: &(impl Fn() -> bool + Sync),
        workers: usize,
    ) -> Option<Self> {
        let n = (size / step) as usize;
        let ystep = if density { step as i32 } else { 1 };
        let vertical = if density { 96 / ystep as usize } else { 128 };
        let origin = -(size as i32) / 2;
        let local_ponds = if options.water {
            ponds(snapshot, field, origin, size)
        } else {
            Vec::new()
        };
        let mut volume = Self {
            origin,
            size,
            n,
            vertical,
            step: step as i32,
            ystep,
            cells: vec![0; n * n * vertical],
            carved: vec![false; n * n * vertical],
            surfaces: vec![0; n * n],
            ponds: local_ponds,
            view: options.underground_view,
            caves: options.caves,
        };
        let mut parameters = snapshot.parameters.clone();
        if !options.caves {
            parameters.cave_threshold = 1.;
        }
        // Threshold adjusts bounded cave intervals, never the preview algorithm or voxel size.
        let cave_margin = if density {
            0.0
        } else {
            (0.78 - parameters.cave_threshold) * 4.0
        };
        let underground_field = UndergroundField::new(snapshot.seed);
        let ponds = &volume.ponds;
        let fill = |start: usize,
                    cells: &mut [u8],
                    carved: &mut [bool],
                    heights: &mut [i32]|
         -> Option<()> {
            for local_z in 0..heights.len() / n {
                let z = start + local_z;
                if cancelled() {
                    return None;
                }
                for x in 0..n {
                    let wx = origin + x as i32 * step as i32;
                    let wz = origin + z as i32 * step as i32;
                    if !options.shape.contains(
                        wx as f32 + step as f32 * 0.5,
                        wz as f32 + step as f32 * 0.5,
                        size,
                    ) {
                        continue;
                    }
                    let base = base_height(field, snapshot, wx, wz);
                    let (height, water) = if options.water {
                        pond_height(base, wx, wz, ponds)
                    } else {
                        (base, 0)
                    };
                    heights[local_z * n + x] = height;
                    let profile = ecology.surface_profile(wx, wz, height as f32);
                    let top = if options.water && height <= water + 1 {
                        Material::Sand
                    } else {
                        surface_material(profile.top)
                    };
                    let subsoil = if top == Material::Sand {
                        Material::Sand
                    } else {
                        surface_material(profile.subsoil)
                    };
                    let caves = if options.caves {
                        ecology.cave_column(wx, wz, height as f32)
                    } else {
                        CaveColumn::default()
                    };
                    let underground = if options.caves {
                        underground_field.column(wx, wz, height as f32)
                    } else {
                        UndergroundColumn::default()
                    };
                    let mut slices = [[0.0; 2]; 64];
                    let mut slice_count = 0;
                    for range in caves.intervals().iter().chain(underground.intervals()) {
                        slices[slice_count] = cave_slice(
                            *range,
                            wx,
                            wz,
                            height,
                            cave_margin,
                            snapshot.seed,
                            parameters.cave_w,
                        );
                        slice_count += 1;
                    }
                    let column_limit = if density {
                        vertical
                    } else {
                        height.max(water).clamp(0, 128) as usize
                    };
                    for y in 0..column_limit {
                        let wy = y as i32 * ystep;
                        let density_solid = if density {
                            field.density(
                                f64::from(wx) + f64::from(step) * 0.5,
                                f64::from(wy) + f64::from(ystep) * 0.5
                                    - f64::from(wave(snapshot, f64::from(wx), f64::from(wz))),
                                f64::from(wz) + f64::from(step) * 0.5,
                                &parameters,
                            ) > 0.
                        } else {
                            wy < height
                        };
                        let solid =
                            density_solid && !(options.water && wy >= height && height < base);
                        let index = (local_z * n + x) * vertical + y;
                        if solid
                            && options.caves
                            && wy >= 3
                            && slices[..slice_count]
                                .iter()
                                .any(|r| wy as f32 >= r[0] && wy as f32 <= r[1])
                        {
                            carved[index] = true;
                            if options.water
                                && underground.water_level().is_some_and(|level| wy <= level)
                            {
                                cells[index] = encode(Material::Water);
                            }
                            continue;
                        }
                        if solid {
                            let depth = height - 1 - wy;
                            let material = if depth == 0 {
                                top
                            } else if (1..=i32::from(profile.stone_depth)).contains(&depth) {
                                subsoil
                            } else {
                                let vein = hash(
                                    snapshot.seed,
                                    wx.div_euclid(3),
                                    wy.div_euclid(3),
                                    wz.div_euclid(3),
                                );
                                if depth > 22 && vein % 113 == 11 {
                                    Material::DiamondOre
                                } else if depth > 16 && vein % 79 == 5 {
                                    Material::GoldOre
                                } else if depth > 8 && vein % 53 == 3 {
                                    Material::CopperOre
                                } else if depth > 5 && vein.is_multiple_of(41) {
                                    Material::CoalOre
                                } else if depth > 9 && vein % 67 == 1 {
                                    Material::IronOre
                                } else if wy < 8 && vein % 29 == 3 {
                                    Material::Gravel
                                } else if wy < 5 && vein % 37 == 2 {
                                    Material::Clay
                                } else if depth > 20 {
                                    Material::Deepslate
                                } else {
                                    Material::Stone
                                }
                            };
                            cells[index] = encode(material);
                        } else if options.water && wy >= height && wy < water {
                            cells[index] = encode(Material::Water);
                        }
                    }
                }
            }
            Some(())
        };
        let workers = workers.clamp(1, n);
        if workers > 1 {
            let rows = n.div_ceil(workers);
            std::thread::scope(|scope| {
                let jobs: Vec<_> = volume
                    .cells
                    .chunks_mut(rows * n * vertical)
                    .zip(volume.carved.chunks_mut(rows * n * vertical))
                    .zip(volume.surfaces.chunks_mut(rows * n))
                    .enumerate()
                    .map(|(i, ((cells, carved), heights))| {
                        let fill = &fill;
                        scope.spawn(move || fill(i * rows, cells, carved, heights))
                    })
                    .collect();
                jobs.into_iter()
                    .map(|j| j.join().ok().flatten())
                    .collect::<Option<Vec<_>>>()
            })?;
        } else {
            fill(
                0,
                &mut volume.cells,
                &mut volume.carved,
                &mut volume.surfaces,
            )?;
        }
        if options.caves {
            volume.decorate_geology(&underground_field, snapshot.seed, cancelled)?;
        }
        Some(volume)
    }
    fn decorate_geology(
        &mut self,
        field: &UndergroundField,
        seed: i64,
        cancelled: &impl Fn() -> bool,
    ) -> Option<()> {
        let mut edits = Vec::new();
        for z in 0..self.n as i32 {
            if cancelled() {
                return None;
            }
            for x in 0..self.n as i32 {
                let wx = self.origin + x * self.step;
                let wz = self.origin + z * self.step;
                let column = field.column(
                    wx,
                    wz,
                    self.surfaces[z as usize * self.n + x as usize] as f32,
                );
                for y in 3..self.vertical as i32 - 1 {
                    let material = decode(self.get(x, y, z));
                    if material.is_none() || material == Some(Material::Water) {
                        continue;
                    }
                    let id = hash(seed, wx, y * self.ystep, wz);
                    let floor = self.get(x, y + 1, z) == 0 && self.is_carved(x, y + 1, z);
                    let ceiling = self.get(x, y - 1, z) == 0 && self.is_carved(x, y - 1, z);
                    if !floor && !ceiling {
                        continue;
                    }
                    match column.biome() {
                        UndergroundBiome::Lush if !id.is_multiple_of(3) => {
                            edits.push((x, y, z, Material::Moss))
                        }
                        UndergroundBiome::Dripstone if id.is_multiple_of(5) => {
                            for d in 1..=1 + (id % 3) as i32 {
                                let qy = if floor { y + d } else { y - d };
                                if self.get(x, qy, z) != 0 || qy < 3 {
                                    break;
                                }
                                edits.push((x, qy, z, Material::Dripstone));
                            }
                        }
                        UndergroundBiome::Deep if id.is_multiple_of(17) => {
                            edits.push((x, y, z, Material::Amethyst))
                        }
                        _ => {}
                    }
                }
            }
        }
        for (x, y, z, m) in edits {
            let i = self.index(x as usize, y as usize, z as usize);
            self.cells[i] = encode(m);
            self.carved[i] = false;
        }
        Some(())
    }
    fn mesh(&self, builder: &mut MeshBuilder<'_>, cancelled: &impl Fn() -> bool) -> Option<()> {
        self.mesh_rows(builder, cancelled, 0..self.n)
    }
    fn mesh_rows(
        &self,
        builder: &mut MeshBuilder<'_>,
        cancelled: &impl Fn() -> bool,
        rows: std::ops::Range<usize>,
    ) -> Option<()> {
        for z in rows.map(|z| z as i32) {
            if cancelled() {
                return None;
            }
            for x in 0..self.n as i32 {
                let column_top = (0..self.vertical as i32)
                    .rev()
                    .find(|y| self.get(x, *y, z) != 0)
                    .map_or(0, |y| y + 1);
                for face in Face::ALL {
                    let [dx, dy, dz] = face.normal();
                    let mut y = 0;
                    while y < column_top {
                        let value = self.get(x, y, z);
                        let other = self.get(x + dx, y + dy, z + dz);
                        let Some(material) = decode(value) else {
                            y += 1;
                            continue;
                        };
                        if !visible(material, decode(other)) {
                            y += 1;
                            continue;
                        }
                        let cave = self.is_carved(x + dx, y + dy, z + dz);
                        let mut end = y + 1;
                        // Only vertical side walls can merge, and they stop at section boundaries.
                        if dy == 0 {
                            while end < column_top
                                && (end * self.ystep).div_euclid(16)
                                    == (y * self.ystep).div_euclid(16)
                                && self.get(x, end, z) == value
                                && visible(material, decode(self.get(x + dx, end, z + dz)))
                                && self.is_carved(x + dx, end, z + dz) == cave
                            {
                                end += 1;
                            }
                        }
                        let anchor = [
                            self.origin + x * self.step,
                            y * self.ystep,
                            self.origin + z * self.step,
                        ];
                        let depth =
                            (self.surfaces[z as usize * self.n + x as usize] - anchor[1]).max(0);
                        let shade = if cave {
                            if self.view == UndergroundView::Surface {
                                (1. - depth as f32 * 0.045).clamp(0.22, 0.72)
                            } else {
                                0.78
                            }
                        } else {
                            1.
                        };
                        builder.face_extent(
                            anchor,
                            [self.step, (end - y) * self.ystep, self.step],
                            face,
                            face.material(material),
                            shade,
                            cave,
                        );
                        if self.caves
                            && cave
                            && depth > 4
                            && hash(builder.snapshot.seed, anchor[0], anchor[1], anchor[2])
                                .is_multiple_of(37)
                        {
                            builder.face_overlay(anchor, face, Material::GlowLichen);
                        }
                        y = end;
                    }
                }
            }
        }
        Some(())
    }
}
fn visible(material: Material, other: Option<Material>) -> bool {
    if material == Material::Water {
        other.is_none()
    } else {
        other.is_none() || other == Some(Material::Water) || other.is_some_and(Material::leaves)
    }
}
fn decorations(
    builder: &mut MeshBuilder<'_>,
    volume: &Volume,
    field: &TerrainField,
    ecology: &Ecology,
    options: SceneOptions,
    villages: &VillageScene,
    cancelled: &impl Fn() -> bool,
) -> Option<()> {
    let height = |x, z| {
        volume.world_height(x, z).unwrap_or_else(|| {
            let h = base_height(field, builder.snapshot, x, z);
            if options.water {
                pond_height(h, x, z, &volume.ponds).0
            } else {
                h
            }
        }) as f32
    };
    let layout = ecology.decorations([volume.origin; 2], volume.size, height);
    let mut blocks = BTreeMap::<[i32; 3], Material>::new();
    let mut plants = Vec::new();
    let mut pods = Vec::new();
    let inside = |p: [i32; 3]| {
        p[0] >= volume.origin
            && p[2] >= volume.origin
            && p[0] < volume.origin + volume.size as i32
            && p[2] < volume.origin + volume.size as i32
            && (0..152).contains(&p[1])
            && options
                .shape
                .contains(p[0] as f32 + 0.5, p[2] as f32 + 0.5, volume.size)
            && !villages.reserved(p[0], p[2])
            && options.underground_view.visible(p)
    };
    for decoration in layout {
        if cancelled() {
            return None;
        }
        let [x, y, z] = decoration.anchor;
        if villages.reserved(x, z) {
            continue;
        }
        // A genuine solid top supports vegetation; water and cave openings cannot.
        let support = decode(volume.world_get([x, y - 1, z]));
        if inside([x, y, z])
            && (!matches!(
                support,
                Some(Material::GrassTop)
                    | Some(Material::Dirt)
                    | Some(Material::Snow)
                    | Some(Material::Stone)
                    | Some(Material::Podzol)
                    | Some(Material::Sand)
                    | Some(Material::RedSand)
                    | Some(Material::Terracotta)
                    | Some(Material::Clay)
            ) || volume.world_get([x, y, z]) != 0)
        {
            continue;
        }
        match decoration.kind {
            DecorationKind::OakTree
            | DecorationKind::BirchTree
            | DecorationKind::SpruceTree
            | DecorationKind::AcaciaTree
            | DecorationKind::JungleTree => {
                let (log, leaves) = match decoration.kind {
                    DecorationKind::BirchTree => (Material::BirchLogSide, Material::BirchLeaves),
                    DecorationKind::SpruceTree => (Material::SpruceLogSide, Material::SpruceLeaves),
                    DecorationKind::AcaciaTree => (Material::AcaciaLogSide, Material::AcaciaLeaves),
                    DecorationKind::JungleTree => (Material::JungleLogSide, Material::JungleLeaves),
                    _ => (Material::OakLogSide, Material::OakLeaves),
                };
                let trunk = i32::from(decoration.scale);
                let mut emitted = false;
                for dy in 0..trunk {
                    let offsets: &[(i32, i32)] = if decoration.kind == DecorationKind::JungleTree
                        && decoration.scale >= 12
                    {
                        &[(0, 0), (1, 0), (0, 1), (1, 1)]
                    } else {
                        &[(0, 0)]
                    };
                    // Clip every trunk cell independently: a shared anchor can lie
                    // just outside this region while its thick trunk intersects it.
                    for &(dx, dz) in offsets {
                        let p = [x + dx, y + dy, z + dz];
                        if inside(p) && volume.world_get(p) == 0 {
                            blocks.insert(p, log);
                            emitted = true;
                        }
                    }
                }
                let (bottom, top) = match decoration.kind {
                    DecorationKind::JungleTree => {
                        (if trunk >= 12 { trunk - 5 } else { trunk - 2 }, trunk + 1)
                    }
                    DecorationKind::SpruceTree => (2, trunk + 1),
                    DecorationKind::AcaciaTree => (trunk - 1, trunk),
                    _ => (trunk - 2, trunk + 1),
                };
                for dy in bottom..=top {
                    let radius: i32 = match decoration.kind {
                        DecorationKind::JungleTree => {
                            if trunk < 12 {
                                if dy == trunk + 1 {
                                    1
                                } else {
                                    2 + (decoration.event_id % 2) as i32
                                }
                            } else if dy >= trunk {
                                3
                            } else {
                                4 + ((decoration.event_id >> 9) % 2) as i32
                            }
                        }
                        DecorationKind::SpruceTree => ((trunk + 1 - dy) / 3 + 1).min(3),
                        DecorationKind::AcaciaTree => 3 - (dy - bottom),
                        _ => {
                            if dy == trunk + 1 {
                                1
                            } else {
                                2
                            }
                        }
                    };
                    for dz in -radius..=radius {
                        for dx in -radius..=radius {
                            if radius == 2
                                && dx.abs() == 2
                                && dz.abs() == 2
                                && (decoration.event_id.rotate_left(dy as u32) & 1) == 0
                            {
                                continue;
                            }
                            // Rounded, seeded irregular crowns replace identical cubes.
                            let crown = dx * dx + dz * dz;
                            if crown > radius * radius + 1
                                || (crown > (radius - 1).pow(2)
                                    && (decoration
                                        .event_id
                                        .rotate_left((dx + dz + dy).rem_euclid(64) as u32)
                                        & 7)
                                        == 0)
                            {
                                continue;
                            }
                            let p = [x + dx, y + dy, z + dz];
                            if inside(p) && volume.world_get(p) == 0 {
                                blocks.entry(p).or_insert(leaves);
                            }
                        }
                    }
                }
                if decoration.kind == DecorationKind::JungleTree {
                    // Cocoa pods attach to trunk sides, not random floating positions.
                    for (dx, dz) in [(-1, 0), (0, -1)] {
                        let p = [x + dx, y + 2 + (decoration.event_id % 3) as i32, z + dz];
                        if inside(p) && volume.world_get(p) == 0 {
                            pods.push((
                                p,
                                if dx < 0 {
                                    [0.4, 0.12, 0.15]
                                } else {
                                    [0.15, 0.12, 0.4]
                                },
                                if dx < 0 {
                                    [0.6, 0.65, 0.7]
                                } else {
                                    [0.7, 0.65, 0.6]
                                },
                            ));
                        }
                    }
                    let r = if trunk >= 12 { 4 } else { 2 };
                    for (side, (dx, dz)) in
                        [(r, 0), (-r, 0), (0, r), (0, -r)].into_iter().enumerate()
                    {
                        let length =
                            3 + (decoration.event_id.rotate_right(side as u32 * 8) % 4) as i32;
                        for offset in 1..=length {
                            let p = [x + dx, y + bottom - offset, z + dz];
                            if inside(p) && volume.world_get(p) == 0 {
                                plants.push((p, Material::Vine));
                            }
                        }
                    }
                }
                if emitted {
                    builder.scene.trees += 1;
                }
            }
            DecorationKind::TallGrass
            | DecorationKind::Poppy
            | DecorationKind::Dandelion
            | DecorationKind::DeadBush
            | DecorationKind::Fern
            | DecorationKind::RedMushroom
            | DecorationKind::BrownMushroom
            | DecorationKind::Sunflower => {
                if inside([x, y, z]) {
                    let material = match decoration.kind {
                        DecorationKind::RedMushroom => Material::RedMushroom,
                        DecorationKind::BrownMushroom => Material::BrownMushroom,
                        DecorationKind::Sunflower => Material::Sunflower,
                        DecorationKind::Fern => Material::Fern,
                        DecorationKind::DeadBush => Material::DeadBush,
                        DecorationKind::Poppy => Material::Poppy,
                        DecorationKind::Dandelion => Material::Dandelion,
                        _ => Material::TallGrass,
                    };
                    if decoration.kind == DecorationKind::Sunflower
                        && inside([x, y + 1, z])
                        && volume.world_get([x, y + 1, z]) == 0
                    {
                        plants.push(([x, y, z], Material::TallGrass));
                        plants.push(([x, y + 1, z], material));
                    } else {
                        plants.push(([x, y, z], material));
                    }
                }
            }
            DecorationKind::Bush => {
                let radius = i32::from(decoration.scale.min(2));
                let leaves = if ecology.biome(x, z, y as f32) == SceneBiome::Jungle {
                    Material::JungleLeaves
                } else {
                    Material::OakLeaves
                };
                for dy in 0..=1 {
                    for dz in -radius..=radius {
                        for dx in -radius..=radius {
                            let p = [x + dx, y + dy, z + dz];
                            if dx * dx + dz * dz <= (radius + 1 - dy).pow(2)
                                && inside(p)
                                && volume.world_get(p) == 0
                            {
                                blocks.entry(p).or_insert(leaves);
                            }
                        }
                    }
                }
                if inside([x, y, z]) {
                    builder.scene.plants += 1;
                }
            }
            DecorationKind::Bamboo | DecorationKind::SugarCane => {
                let material = if decoration.kind == DecorationKind::Bamboo {
                    Material::Bamboo
                } else {
                    Material::SugarCane
                };
                let offsets: &[(i32, i32)] = if decoration.kind == DecorationKind::Bamboo {
                    &[(0, 0), (-1, 0), (0, 1)]
                } else {
                    &[(0, 0)]
                };
                for &(dx, dz) in offsets {
                    let ground = height(x + dx, z + dz).round() as i32;
                    if (ground - y).abs() > 2 {
                        continue;
                    }
                    let foot = [x + dx, ground, z + dz];
                    if inside(foot) {
                        let support = decode(volume.world_get([foot[0], ground - 1, foot[2]]));
                        if support.is_none()
                            || support == Some(Material::Water)
                            || volume.world_get(foot) != 0
                        {
                            continue;
                        }
                    }
                    for dy in 0..i32::from(decoration.scale) {
                        let p = [x + dx, ground + dy, z + dz];
                        if inside(p) && volume.world_get(p) == 0 {
                            plants.push((p, material));
                        }
                    }
                }
            }
            DecorationKind::Melon | DecorationKind::Pumpkin => {
                if inside([x, y, z]) {
                    blocks.entry([x, y, z]).or_insert(
                        if decoration.kind == DecorationKind::Melon {
                            Material::Melon
                        } else {
                            Material::Pumpkin
                        },
                    );
                    builder.scene.plants += 1;
                }
            }
            DecorationKind::Cactus => {
                for dy in 0..i32::from(decoration.scale) {
                    let p = [x, y + dy, z];
                    if inside(p) && volume.world_get(p) == 0 {
                        blocks.entry(p).or_insert(Material::Cactus);
                    }
                }
                if inside([x, y, z]) {
                    builder.scene.plants += 1;
                }
            }
            DecorationKind::Rock => {
                if inside([x, y, z]) {
                    blocks.entry([x, y, z]).or_insert(Material::Stone);
                }
            }
        }
    }
    for (position, material) in &blocks {
        if cancelled() {
            return None;
        }
        for face in Face::ALL {
            let d = face.normal();
            let neighbor = std::array::from_fn(|i| position[i] + d[i]);
            let other = blocks
                .get(&neighbor)
                .copied()
                .or_else(|| decode(volume.world_get(neighbor)));
            let exposed = if material.leaves() {
                other.is_none() || other == Some(Material::Water)
            } else {
                visible(*material, other)
            };
            if exposed {
                builder.face_extent(*position, [1; 3], face, face.material(*material), 1., false);
            }
        }
    }
    builder.scene.solid_voxels += blocks.len() as u32;
    for (p, offset, extent) in pods {
        if !blocks.contains_key(&p) {
            builder.detail_box(p, offset, extent, Material::Cocoa);
        }
    }
    for (p, material) in plants {
        if blocks.contains_key(&p) {
            continue;
        }
        builder.scene.plants += 1;
        builder.plant(p, material);
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
    const ALL: [Self; 6] = [
        Self::East,
        Self::West,
        Self::Up,
        Self::Down,
        Self::South,
        Self::North,
    ];
    fn normal(self) -> [i32; 3] {
        match self {
            Self::East => [1, 0, 0],
            Self::West => [-1, 0, 0],
            Self::Up => [0, 1, 0],
            Self::Down => [0, -1, 0],
            Self::South => [0, 0, 1],
            Self::North => [0, 0, -1],
        }
    }
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
            Self::Up => 1.,
            Self::Down => 0.48,
            Self::East | Self::West => 0.84,
            Self::South | Self::North => 0.72,
        }
    }
    fn material(self, material: Material) -> Material {
        match (material, self) {
            (Material::GrassTop, Self::Up) => Material::GrassTop,
            (Material::GrassTop, Self::Down) => Material::Dirt,
            (Material::GrassTop, _) => Material::GrassSide,
            (Material::OakLogSide, Self::Up | Self::Down) => Material::OakLogTop,
            (Material::BirchLogSide, Self::Up | Self::Down) => Material::BirchLogTop,
            (Material::SpruceLogSide, Self::Up | Self::Down) => Material::SpruceLogTop,
            (Material::AcaciaLogSide, Self::Up | Self::Down) => Material::AcaciaLogTop,
            (Material::JungleLogSide, Self::Up | Self::Down) => Material::JungleLogTop,
            _ => material,
        }
    }
}
struct MeshBuilder<'a> {
    snapshot: &'a TerrainSnapshot,
    sections: BTreeMap<SectionKey, SectionMesh>,
    scene: SceneStats,
}
impl<'a> MeshBuilder<'a> {
    fn new(snapshot: &'a TerrainSnapshot) -> Self {
        Self {
            snapshot,
            sections: BTreeMap::new(),
            scene: SceneStats::default(),
        }
    }
    fn face_extent(
        &mut self,
        anchor: [i32; 3],
        extent: [i32; 3],
        face: Face,
        material: Material,
        ambient: f32,
        cave: bool,
    ) {
        // UVs in production TerrainVertex hold [0,2), so split every planar
        // extent into <=2-block quads. Stone and bark retain one texel scale.
        let normal = face.normal();
        let [a, b] = match face {
            Face::East | Face::West => [1, 2],
            Face::Up | Face::Down => [0, 2],
            Face::South | Face::North => [0, 1],
        };
        for vb in (0..extent[b]).step_by(2) {
            for va in (0..extent[a]).step_by(2) {
                let mut at = anchor;
                at[a] += va;
                at[b] += vb;
                let mut shape = extent;
                shape[a] = (extent[a] - va).min(2);
                shape[b] = (extent[b] - vb).min(2);
                let corners = face
                    .corners()
                    .map(|p| std::array::from_fn(|i| (at[i] + p[i] * shape[i]) as f32));
                let uv = if matches!(face, Face::Up | Face::Down) {
                    [
                        [0., 0.],
                        [0., shape[2] as f32],
                        [shape[0] as f32, shape[2] as f32],
                        [shape[0] as f32, 0.],
                    ]
                } else {
                    let width = if normal[0] != 0 { shape[2] } else { shape[0] } as f32;
                    [
                        [0., shape[1] as f32],
                        [0., 0.],
                        [width, 0.],
                        [width, shape[1] as f32],
                    ]
                };
                self.quad(at, corners, uv, material, face.shade() * ambient, cave);
            }
        }
    }
    fn face_overlay(&mut self, anchor: [i32; 3], face: Face, material: Material) {
        let n = face.normal();
        let air_anchor = std::array::from_fn(|i| anchor[i] + n[i]);
        let corners = face
            .corners()
            .map(|p| std::array::from_fn(|i| anchor[i] as f32 + p[i] as f32 + n[i] as f32 * 0.003));
        self.quad(
            air_anchor,
            corners,
            [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
            material,
            1.,
            false,
        );
    }
    fn horizontal_detail(&mut self, anchor: [i32; 3], material: Material, rotate: bool) {
        let [x, y, z] = anchor.map(|v| v as f32);
        let corners = [
            [x, y + 0.015, z],
            [x, y + 0.015, z + 1.],
            [x + 1., y + 0.015, z + 1.],
            [x + 1., y + 0.015, z],
        ];
        let uv = if rotate {
            [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
        } else {
            [[0., 1.], [0., 0.], [1., 0.], [1., 1.]]
        };
        self.quad(anchor, corners, uv, material, 1., false);
    }
    fn detail_box(
        &mut self,
        anchor: [i32; 3],
        offset: [f32; 3],
        extent: [f32; 3],
        material: Material,
    ) {
        for face in Face::ALL {
            let corners = face.corners().map(|p| {
                std::array::from_fn(|axis| {
                    anchor[axis] as f32 + offset[axis] + p[axis] as f32 * extent[axis]
                })
            });
            self.quad(
                anchor,
                corners,
                [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
                material,
                face.shade(),
                false,
            );
        }
    }
    fn plant(&mut self, anchor: [i32; 3], material: Material) {
        let [x, y, z] = anchor.map(|v| v as f32);
        let planes = [
            [
                [x + 0.08, y, z + 0.08],
                [x + 0.08, y + 0.95, z + 0.08],
                [x + 0.92, y + 0.95, z + 0.92],
                [x + 0.92, y, z + 0.92],
            ],
            [
                [x + 0.08, y, z + 0.92],
                [x + 0.08, y + 0.95, z + 0.92],
                [x + 0.92, y + 0.95, z + 0.08],
                [x + 0.92, y, z + 0.08],
            ],
        ];
        for corners in planes {
            self.quad(
                anchor,
                corners,
                [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
                material,
                1.,
                false,
            );
            self.quad(
                anchor,
                [corners[3], corners[2], corners[1], corners[0]],
                [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
                material,
                1.,
                false,
            );
        }
    }
    fn quad(
        &mut self,
        anchor: [i32; 3],
        corners: [[f32; 3]; 4],
        uv: [[f32; 2]; 4],
        material: Material,
        shade: f32,
        cave: bool,
    ) {
        let key = (
            anchor[0].div_euclid(16),
            (anchor[1] - MIN_Y).div_euclid(16),
            anchor[2].div_euclid(16),
        );
        let origin = [key.0 * 16, MIN_Y + key.1 * 16, key.2 * 16];
        let shift = self.snapshot.parameters.color_shift * 0.12;
        let tint = material.tint();
        let factors = [1. + shift, 1. - shift * 0.3, 1. - shift * 0.5];
        let rgb: [u8; 3] = std::array::from_fn(|i| {
            (f32::from(tint[i]) * shade * factors[i]).clamp(0., 255.) as u8
        });
        let color = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 255]);
        self.scene.material_quads[usize::from(material.layer())] += 1;
        if material.translucent() {
            self.scene.water_quads += 1;
        }
        if cave {
            self.scene.cave_faces += 1;
        }
        let mesh = self.sections.entry(key).or_default();
        let vertices = if material.translucent() {
            &mut mesh.translucent
        } else {
            &mut mesh.solid
        };
        for (position, uv) in corners.into_iter().zip(uv) {
            let local = std::array::from_fn(|i| position[i] - origin[i] as f32);
            vertices.push(TerrainVertex::new(
                local,
                uv,
                material.layer(),
                color,
                0xf0f0,
            ));
        }
    }
}
