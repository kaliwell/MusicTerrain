#![cfg(feature = "native")]
use musicterrain::{
    terrain::{GenerationRevision, TerrainParameters, TerrainSnapshot},
    visual::{MeshWorker, SceneOptions, mesh_snapshot, mesh_snapshot_with_options},
};
use std::time::{Duration, Instant};
fn snapshot(revision: u64) -> TerrainSnapshot {
    TerrainSnapshot {
        revision: GenerationRevision(revision),
        seed: 42,
        parameters: TerrainParameters::default(),
        audio_time: 0.0,
    }
}
#[test]
fn fixed_time_meshes_are_deterministic_and_section_local() {
    let request = snapshot(1);
    let a = mesh_snapshot(&request, 64).unwrap();
    let b = mesh_snapshot(&request, 64).unwrap();
    assert_eq!(a.checksum, b.checksum);
    assert_eq!(a.vertices, b.vertices);
    assert!(a.vertices > 4096 * 4);
    assert_eq!(a.voxel_step, 1);
    println!(
        "height64: {:.3} ms, {} vertices, {} sections, checksum {:016x}",
        a.generation_ms,
        a.vertices,
        a.sections.len(),
        a.checksum
    );
    for ((_, sy, _), section) in a.sections {
        assert!((4..12).contains(&sy)); // Horizon's -64 origin, preview y=0..128.
        assert_eq!(section.solid.len() % 4, 0);
        for vertex in section.solid {
            assert!(
                vertex
                    .pos
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=16.0).contains(v))
            );
        }
    }
}
#[test]
fn terrain_and_audio_time_change_actual_geometry() {
    let mut a = snapshot(0);
    let quiet = mesh_snapshot(&a, 32).unwrap();
    a.parameters.amplitude = 100.0;
    let loud = mesh_snapshot(&a, 32).unwrap();
    assert_ne!(quiet.checksum, loud.checksum);
    a.parameters.shockwave = 12.0;
    let first = mesh_snapshot(&a, 32).unwrap();
    a.audio_time = 0.35;
    let later = mesh_snapshot(&a, 32).unwrap();
    assert_ne!(first.checksum, later.checksum);
}
#[test]
fn density_preview_and_larger_regions_use_bounded_lod() {
    let mut request = snapshot(1);
    request.parameters.island_strength = 1.0;
    request.parameters.cave_threshold = 0.35;
    let density = mesh_snapshot(&request, 64).unwrap();
    assert_eq!(density.voxel_step, 2);
    println!(
        "density64: {:.3} ms, {} vertices, {} sections, checksum {:016x}",
        density.generation_ms,
        density.vertices,
        density.sections.len(),
        density.checksum
    );
    assert!(density.vertices > 0);
    for (_, section) in density.sections {
        assert!(
            section
                .solid
                .iter()
                .all(|v| v.pos.iter().all(|p| (0.0..=16.0).contains(p)))
        );
    }
    request.parameters.island_strength = 0.0;
    request.parameters.cave_threshold = 0.78;
    let larger = mesh_snapshot(&request, 256).unwrap();
    assert_eq!(larger.voxel_step, 4);
    println!(
        "height256: {:.3} ms, {} vertices, {} sections, checksum {:016x}",
        larger.generation_ms,
        larger.vertices,
        larger.sections.len(),
        larger.checksum
    );
    assert!(larger.vertices < 600_000);
}
#[test]
fn invalid_requests_never_allocate_meshes() {
    assert!(mesh_snapshot(&snapshot(0), 0).is_none());
    assert!(mesh_snapshot(&snapshot(0), 257).is_none());
    assert!(mesh_snapshot(&snapshot(0), 63).is_none());
    let mut request = snapshot(0);
    request.audio_time = f64::NAN;
    assert!(mesh_snapshot(&request, 64).is_none());
    request.audio_time = 0.;
    request.parameters.frequency = f32::INFINITY;
    assert!(mesh_snapshot(&request, 64).is_none());
}
#[test]
fn obsolete_jobs_cannot_replace_current_revision_and_queue_is_bounded() {
    let mut worker = MeshWorker::new();
    assert!(worker.error.is_none());
    for _ in 0..100 {
        worker.submit(snapshot(1), 64);
    }
    assert!(worker.dropped > 0);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if worker.submit(snapshot(2), 16) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "mesh queue did not accept the new revision"
        );
        let _ = worker.poll(GenerationRevision(2));
        std::thread::yield_now();
    }
    loop {
        if let Some(result) = worker.poll(GenerationRevision(2)) {
            assert_eq!(result.revision, GenerationRevision(2));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "mesh worker did not return current revision"
        );
        std::thread::yield_now();
    }
}

#[test]
fn flat_surface_has_only_exterior_faces_and_correct_winding() {
    let mut request = snapshot(0);
    request.parameters.amplitude = 0.;
    request.parameters.detail = 0.;
    request.parameters.base_height = 16.;
    let flat = mesh_snapshot_with_options(
        &request,
        16,
        &SceneOptions {
            vegetation: false,
            water: false,
            caves: false,
            ..SceneOptions::default()
        },
    )
    .unwrap();
    let mut exterior_area = 0.0;
    for ((cx, sy, cz), section) in flat.sections {
        let origin = [cx * 16, -64 + sy * 16, cz * 16];
        for quad in section.solid.as_chunks::<4>().0 {
            let p: Vec<[f32; 3]> = quad
                .iter()
                .map(|v| std::array::from_fn(|i| v.pos[i] + origin[i] as f32))
                .collect();
            let a: [f32; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
            let b: [f32; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
            let normal = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            let axis = normal.iter().position(|v| *v != 0.).unwrap();
            exterior_area += normal[axis].abs();
            let expected = if axis == 1 {
                if p[0][axis] == 16. { 1. } else { -1. }
            } else if p[0][axis] == 8. {
                1.
            } else {
                -1.
            };
            assert_eq!(normal[axis].signum(), expected);
            assert!(p.iter().all(|v| (-8.0..=8.0).contains(&v[0])
                && (-8.0..=8.0).contains(&v[2])
                && (0.0..=16.0).contains(&v[1])));
        }
    }
    assert_eq!(exterior_area, 6.0 * 16.0 * 16.0);
}

#[test]
fn completed_snapshot_cache_does_not_rebuild_paused_terrain() {
    let mut worker = MeshWorker::new();
    let mut request = snapshot(7);
    assert!(worker.submit(request.clone(), 16));
    let deadline = Instant::now() + Duration::from_secs(10);
    while worker.poll(request.revision).is_none() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    request.audio_time = 100.; // No shockwave: time alone cannot change this mesh.
    assert!(!worker.submit(request.clone(), 16));
    assert_eq!(worker.cache_hits, 1);
    request.revision = GenerationRevision(8);
    assert!(worker.submit(request, 16)); // Cache from another revision must never win.
}
