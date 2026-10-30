#![cfg(feature = "native")]
use musicterrain::{
    materials::Material,
    preset::builtins,
    terrain::{GenerationRevision, TerrainSnapshot},
    visual::{MeshWorker, SceneOptions, mesh_snapshot, mesh_snapshot_with_options},
};
use std::time::{Duration, Instant};
fn scene() -> TerrainSnapshot {
    let preset = builtins().remove(0);
    TerrainSnapshot {
        revision: GenerationRevision(1),
        seed: 42,
        parameters: preset.parameters,
        audio_time: 0.0,
    }
}
#[test]
fn default_bass_scene_contains_real_materials_trees_flowers_caves_and_water() {
    let mesh = mesh_snapshot(&scene(), 64).unwrap();
    println!(
        "scene64 {:.2}ms vertices={} sections={} stats={:?}",
        mesh.generation_ms,
        mesh.vertices,
        mesh.sections.len(),
        mesh.scene
    );
    assert!(
        mesh.scene.trees > 0,
        "default scene must contain actual tree trunks"
    );
    assert!(
        mesh.scene.plants > 0,
        "default scene must contain actual cutout plant quads"
    );
    assert!(
        mesh.scene.carved_voxels > 0 && mesh.scene.cave_faces > 0,
        "caves must remove solids and expose interior faces"
    );
    assert!(
        mesh.scene.water_quads > 0,
        "default scene must contain visible pond water"
    );
    for material in [
        Material::GrassTop,
        Material::GrassSide,
        Material::Dirt,
        Material::Stone,
        Material::Sand,
        Material::OakLogSide,
        Material::OakLogTop,
        Material::OakLeaves,
    ] {
        assert!(
            mesh.scene.material_quads[usize::from(material.layer())] > 0,
            "missing actual mesh layer {material:?}"
        );
    }
    assert!(
        mesh.scene.material_quads[usize::from(Material::Poppy.layer())]
            + mesh.scene.material_quads[usize::from(Material::Dandelion.layer())]
            > 0,
        "flowers must have their own texture layer"
    );
    let solid: usize = mesh.sections.iter().map(|(_, m)| m.solid.len()).sum();
    let water: usize = mesh.sections.iter().map(|(_, m)| m.translucent.len()).sum();
    assert_eq!(water, mesh.scene.water_quads as usize * 4);
    assert_eq!(solid + water, mesh.vertices);
    assert_eq!(
        mesh.scene.material_quads.iter().sum::<u32>() as usize * 4,
        mesh.vertices
    );
    assert!(mesh.sections.iter().all(|(_, m)| {
        m.translucent
            .iter()
            .all(|v| v.layer == Material::Water.layer())
    }));
}
#[test]
fn independent_scene_switches_remove_their_actual_geometry() {
    let snapshot = scene();
    let bare = SceneOptions {
        vegetation: false,
        water: false,
        caves: false,
        ..SceneOptions::default()
    };
    let mesh = mesh_snapshot_with_options(&snapshot, 64, &bare).unwrap();
    assert_eq!(mesh.scene.trees, 0);
    assert_eq!(mesh.scene.plants, 0);
    assert_eq!(mesh.scene.carved_voxels, 0);
    assert_eq!(mesh.scene.cave_faces, 0);
    assert_eq!(mesh.scene.water_quads, 0);
    for material in [
        Material::Water,
        Material::OakLeaves,
        Material::OakLogSide,
        Material::OakLogTop,
        Material::TallGrass,
        Material::Poppy,
        Material::Dandelion,
    ] {
        assert_eq!(mesh.scene.material_quads[usize::from(material.layer())], 0);
    }
    let only_caves = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            caves: true,
            ..bare
        },
    )
    .unwrap();
    assert!(only_caves.scene.carved_voxels > 0 && only_caves.scene.cave_faces > 0);
    assert!(only_caves.scene.solid_voxels < mesh.scene.solid_voxels);
    let only_water = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            water: true,
            ..bare
        },
    )
    .unwrap();
    assert!(only_water.scene.water_quads > 0);
    assert_eq!(only_water.scene.trees, 0);
    let only_plants = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            vegetation: true,
            ..bare
        },
    )
    .unwrap();
    assert!(only_plants.scene.trees > 0 && only_plants.scene.plants > 0);
    assert_eq!(only_plants.scene.cave_faces, 0);
}
#[test]
fn textured_scene_is_deterministic_and_all_quads_stay_section_local() {
    let snapshot = scene();
    let a = mesh_snapshot(&snapshot, 64).unwrap();
    let b = mesh_snapshot(&snapshot, 64).unwrap();
    assert_eq!(a.checksum, b.checksum);
    assert_eq!(a.scene, b.scene);
    assert_eq!(a.vertices, b.vertices);
    for ((cx, sy, cz), mesh) in &a.sections {
        assert_eq!(mesh.solid.len() % 4, 0);
        assert_eq!(mesh.translucent.len() % 4, 0);
        for vertex in mesh.solid.iter().chain(&mesh.translucent) {
            assert!(
                usize::from(vertex.layer) < Material::ALL.len(),
                "leaf alpha must use cutout, without force-opaque bit"
            );
            assert!(
                vertex
                    .pos
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=16.0).contains(v))
            );
            assert!(
                vertex.uv().iter().all(|v| (0.0..=2.0).contains(v)),
                "fixed point texture UVs must never wrap from integer overflow"
            );
            assert_eq!(
                (vertex.color >> 24) & 15,
                15,
                "ordinary materials must not emit HDR light"
            );
            let world = [
                vertex.pos[0] + (cx * 16) as f32,
                vertex.pos[1] + (-64 + sy * 16) as f32,
                vertex.pos[2] + (cz * 16) as f32,
            ];
            assert!((-32.0..=32.0).contains(&world[0]) && (-32.0..=32.0).contains(&world[2]));
        }
    }
}
#[test]
fn scene_options_participate_in_snapshot_cache_identity() {
    let snapshot = scene();
    let bare = SceneOptions {
        vegetation: false,
        water: false,
        caves: false,
        ..SceneOptions::default()
    };
    let mut worker = MeshWorker::new();
    assert!(worker.submit_with_options(snapshot.clone(), 16, bare));
    let deadline = Instant::now() + Duration::from_secs(15);
    while worker.poll(snapshot.revision).is_none() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(!worker.submit_with_options(snapshot.clone(), 16, bare));
    assert_eq!(worker.cache_hits, 1);
    assert!(
        worker.submit_with_options(
            snapshot.clone(),
            16,
            SceneOptions {
                vegetation: true,
                ..bare
            }
        ),
        "a changed geometry option must invalidate the completed cache"
    );
    while worker.poll(snapshot.revision).is_none() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
#[test]
fn density_islands_keep_real_topology_and_materials() {
    let mut snapshot = scene();
    snapshot.parameters = builtins().remove(3).parameters;
    let mesh = mesh_snapshot(&snapshot, 64).unwrap();
    assert_eq!(mesh.voxel_step, 2);
    assert!(mesh.scene.solid_voxels > 0 && mesh.vertices > 0);
    assert!(mesh.scene.material_quads[usize::from(Material::Stone.layer())] > 0);
    assert!(
        mesh.sections
            .iter()
            .flat_map(|(_, m)| m.solid.iter())
            .all(|v| v
                .pos
                .iter()
                .all(|p| p.is_finite() && (0.0..=16.0).contains(p)))
    );
}

#[test]
fn biome_selection_changes_real_mesh_species_and_ground_without_changing_audio() {
    use musicterrain::ecology::SceneBiome;
    let mut snapshot = scene();
    snapshot.parameters.amplitude = 0.;
    snapshot.parameters.detail = 0.;
    snapshot.parameters.base_height = 25.;
    let mut hashes = std::collections::HashSet::new();
    for (biome, layers) in [
        (
            SceneBiome::BirchForest,
            vec![
                Material::BirchLogSide,
                Material::BirchLogTop,
                Material::BirchLeaves,
            ],
        ),
        (
            SceneBiome::Taiga,
            vec![
                Material::Podzol,
                Material::SpruceLogSide,
                Material::SpruceLeaves,
            ],
        ),
        (
            SceneBiome::Desert,
            vec![Material::Sand, Material::Cactus, Material::DeadBush],
        ),
        (
            SceneBiome::Badlands,
            vec![Material::RedSand, Material::Terracotta, Material::Cactus],
        ),
        (
            SceneBiome::Savanna,
            vec![
                Material::GrassTop,
                Material::AcaciaLogSide,
                Material::AcaciaLeaves,
            ],
        ),
        (
            SceneBiome::Jungle,
            vec![
                Material::JungleLogSide,
                Material::JungleLogTop,
                Material::JungleLeaves,
                Material::Vine,
                Material::Fern,
            ],
        ),
        (
            SceneBiome::Swamp,
            vec![Material::GrassTop, Material::Clay, Material::OakLeaves],
        ),
    ] {
        let options = SceneOptions {
            biome: Some(biome),
            water: false,
            caves: false,
            ..SceneOptions::default()
        };
        let mesh = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
        for material in layers {
            assert!(
                mesh.scene.material_quads[material.layer() as usize] > 0,
                "missing {material:?} in {biome:?}"
            );
        }
        assert!(
            hashes.insert(mesh.checksum),
            "identical geometry for {biome:?}"
        );
        assert_eq!(
            mesh.checksum,
            mesh_snapshot_with_options(&snapshot, 64, &options)
                .unwrap()
                .checksum
        );
        println!(
            "{biome:?}: {} vertices, trees={}, plants={}",
            mesh.vertices, mesh.scene.trees, mesh.scene.plants
        );
    }
}

#[test]
fn jungle_vegetation_switch_removes_trunks_canopies_ferns_and_vines() {
    use musicterrain::ecology::SceneBiome;
    let mut snapshot = scene();
    snapshot.parameters.amplitude = 0.;
    snapshot.parameters.detail = 0.;
    snapshot.parameters.base_height = 25.;
    let options = SceneOptions {
        biome: Some(SceneBiome::Jungle),
        ..SceneOptions::default()
    };
    let grown = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
    let bare = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            vegetation: false,
            ..options
        },
    )
    .unwrap();
    assert!(grown.scene.trees > 0 && grown.scene.plants > 0);
    assert_eq!((bare.scene.trees, bare.scene.plants), (0, 0));
    for material in [
        Material::JungleLogSide,
        Material::JungleLogTop,
        Material::JungleLeaves,
        Material::Vine,
        Material::Fern,
    ] {
        assert!(grown.scene.material_quads[material.layer() as usize] > 0);
        assert_eq!(bare.scene.material_quads[material.layer() as usize], 0);
    }
    assert!(
        grown
            .sections
            .iter()
            .flat_map(|(_, section)| &section.solid)
            .all(|v| v
                .pos
                .iter()
                .all(|c| c.is_finite() && (0.0..=16.0).contains(c)))
    );
}

#[test]
fn rich_jungle_mesh_contains_bamboo_melons_cocoa_and_fern_layers() {
    use musicterrain::{ecology::SceneBiome, village::VillageMode};
    let mut snapshot = scene();
    snapshot.parameters.amplitude = 0.;
    snapshot.parameters.detail = 0.;
    snapshot.parameters.base_height = 25.;
    let options = SceneOptions {
        biome: Some(SceneBiome::Jungle),
        villages: VillageMode::Off,
        water: false,
        ..SceneOptions::default()
    };
    let mesh = mesh_snapshot_with_options(&snapshot, 128, &options).unwrap();
    for material in [
        Material::Bamboo,
        Material::Melon,
        Material::Cocoa,
        Material::Fern,
        Material::JungleLeaves,
    ] {
        assert!(
            mesh.scene.material_quads[material.layer() as usize] > 0,
            "missing actual {material:?}"
        );
    }
    assert!(mesh.scene.trees > 15 && mesh.scene.plants > 50);
    assert_eq!(
        mesh.checksum,
        mesh_snapshot_with_options(&snapshot, 128, &options)
            .unwrap()
            .checksum
    );
}
