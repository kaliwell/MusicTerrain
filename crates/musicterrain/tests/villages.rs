#![cfg(feature = "native")]
use musicterrain::{
    ecology::{Ecology, SceneBiome},
    materials::Material,
    preset::builtins,
    terrain::{GenerationRevision, TerrainSnapshot},
    village::{VillageMode, VillageScene},
    visual::{SceneOptions, mesh_snapshot_with_options},
};
use std::collections::BTreeMap;
fn village(origin: [i32; 2], size: u32) -> VillageScene {
    VillageScene::generate(
        42,
        origin,
        size,
        VillageMode::Showcase,
        &Ecology::new(42).with_biome(Some(SceneBiome::Meadow)),
        |_, _| 25.,
        || false,
    )
    .unwrap()
}
#[test]
fn showcase_has_hollow_houses_windows_doors_roads_well_and_irrigated_crops() {
    let scene = village([-32, -32], 64);
    assert_eq!((scene.villages, scene.houses, scene.farms), (1, 4, 1));
    for material in [
        Material::OakPlanks,
        Material::Cobblestone,
        Material::Glass,
        Material::RoofTiles,
        Material::Path,
        Material::Farmland,
        Material::Water,
        Material::HayBale,
    ] {
        assert!(
            scene.blocks.values().any(|m| *m == material),
            "missing {material:?}"
        );
    }
    // First house: a genuinely open doorway and empty room, backed by a floor.
    assert!(!scene.blocks.contains_key(&[-13, 26, -9]));
    assert!(!scene.blocks.contains_key(&[-13, 27, -12]));
    assert_eq!(
        scene.blocks.get(&[-13, 25, -12]),
        Some(&Material::Cobblestone)
    );
    assert!(!scene.crops.is_empty());
    for p in &scene.crops {
        assert_eq!(
            scene.blocks.get(&[p[0], p[1] - 1, p[2]]),
            Some(&Material::Farmland)
        );
    }
    assert_eq!(scene.blocks, village([-32, -32], 64).blocks);
}
#[test]
fn clipped_village_blocks_and_crops_match_whole_generation_in_any_tile_order() {
    let whole = village([-32, -32], 64);
    let mut blocks = BTreeMap::new();
    let mut crops = Vec::new();
    for origin in [[0, 0], [-32, -32], [0, -32], [-32, 0]] {
        let tile = village(origin, 32);
        blocks.extend(tile.blocks);
        crops.extend(tile.crops);
    }
    let mut expected = whole.crops;
    expected.sort();
    crops.sort();
    assert_eq!(whole.blocks, blocks);
    assert_eq!(expected, crops);
}
#[test]
fn automatic_villages_respect_biomes_styles_ground_bounds_and_cancellation() {
    for biome in [
        SceneBiome::Jungle,
        SceneBiome::Swamp,
        SceneBiome::Forest,
        SceneBiome::BirchForest,
    ] {
        let scene = VillageScene::generate(
            42,
            [-128, -128],
            256,
            VillageMode::Auto,
            &Ecology::new(42).with_biome(Some(biome)),
            |_, _| 25.,
            || false,
        )
        .unwrap();
        assert!(
            scene.blocks.is_empty(),
            "no vanilla natural village in {biome:?}"
        );
    }
    for (biome, material) in [
        (SceneBiome::Desert, Material::Sandstone),
        (SceneBiome::Taiga, Material::SprucePlanks),
        (SceneBiome::Savanna, Material::AcaciaLogSide),
    ] {
        let scene = VillageScene::generate(
            42,
            [-32, -32],
            64,
            VillageMode::Showcase,
            &Ecology::new(42).with_biome(Some(biome)),
            |_, _| 25.,
            || false,
        )
        .unwrap();
        assert!(scene.blocks.values().any(|m| *m == material));
    }
    let ecology = Ecology::new(42).with_biome(Some(SceneBiome::Meadow));
    assert!(
        VillageScene::generate(
            42,
            [-32, -32],
            64,
            VillageMode::Showcase,
            &ecology,
            |_, _| f32::NAN,
            || false
        )
        .unwrap()
        .blocks
        .is_empty()
    );
    assert!(
        VillageScene::generate(
            42,
            [-32, -32],
            64,
            VillageMode::Showcase,
            &ecology,
            |_, _| 25.,
            || true
        )
        .is_none()
    );
    assert!(
        VillageScene::generate(
            42,
            [i32::MIN, 0],
            64,
            VillageMode::Showcase,
            &ecology,
            |_, _| 25.,
            || false
        )
        .is_none()
    );
    assert!(
        VillageScene::generate(
            42,
            [0, 0],
            u32::MAX,
            VillageMode::Showcase,
            &ecology,
            |_, _| 25.,
            || false
        )
        .is_none()
    );
    let bounded = VillageScene::generate(
        42,
        [-128, -128],
        256,
        VillageMode::Auto,
        &ecology,
        |_, _| 25.,
        || false,
    )
    .unwrap();
    assert!(bounded.villages > 0 && bounded.blocks.len() < 150_000);
}
#[test]
fn village_toggle_changes_real_mesh_and_water_switch_removes_all_village_water() {
    let mut parameters = builtins().remove(0).parameters;
    parameters.amplitude = 0.;
    parameters.detail = 0.;
    parameters.base_height = 25.;
    let snapshot = TerrainSnapshot {
        seed: 42,
        revision: GenerationRevision(1),
        parameters,
        audio_time: 0.,
    };
    let options = SceneOptions {
        biome: Some(SceneBiome::Meadow),
        villages: VillageMode::Showcase,
        vegetation: false,
        ..SceneOptions::default()
    };
    let grown = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
    assert_eq!(grown.scene.villages, 1);
    assert!(grown.scene.houses >= 2);
    assert_eq!(grown.scene.farms, 1);
    assert!(grown.scene.material_quads[Material::Glass.layer() as usize] > 0);
    let bare = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            villages: VillageMode::Off,
            ..options
        },
    )
    .unwrap();
    assert_eq!(bare.scene.houses, 0);
    assert_eq!(
        bare.scene.material_quads[Material::Glass.layer() as usize],
        0
    );
    assert_ne!(grown.checksum, bare.checksum);
    let dry = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            water: false,
            ..options
        },
    )
    .unwrap();
    assert_eq!(dry.scene.water_quads, 0);
    assert!(grown.sections.iter().flat_map(|(_, s)| &s.solid).all(|v| {
        v.pos
            .iter()
            .all(|p| p.is_finite() && (0.0..=16.0).contains(p))
    }));
}

#[test]
fn automatic_query_halo_includes_jittered_villages_across_cell_edges() {
    let ecology = Ecology::new(0).with_biome(Some(SceneBiome::Meadow));
    let whole = VillageScene::generate(
        0,
        [128, -16],
        64,
        VillageMode::Auto,
        &ecology,
        |_, _| 25.,
        || false,
    )
    .unwrap();
    let tile = VillageScene::generate(
        0,
        [144, -1],
        16,
        VillageMode::Auto,
        &ecology,
        |_, _| 25.,
        || false,
    )
    .unwrap();
    let expected: BTreeMap<_, _> = whole
        .blocks
        .into_iter()
        .filter(|(p, _)| p[0] >= 144 && p[0] < 160 && p[2] >= -1 && p[2] < 15)
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(tile.blocks, expected);
}

#[test]
fn coarse_and_density_previews_suspend_villages_instead_of_collapsing_rooms() {
    let mut parameters = builtins().remove(0).parameters;
    parameters.amplitude = 0.;
    parameters.detail = 0.;
    parameters.base_height = 25.;
    let mut snapshot = TerrainSnapshot {
        seed: 42,
        revision: GenerationRevision(1),
        parameters,
        audio_time: 0.,
    };
    let options = SceneOptions {
        full_detail: false,
        biome: Some(SceneBiome::Meadow),
        villages: VillageMode::Showcase,
        ..SceneOptions::default()
    };
    let coarse = mesh_snapshot_with_options(&snapshot, 128, &options).unwrap();
    assert_eq!(coarse.scene.houses, 0);
    snapshot.parameters.island_strength = 0.8;
    let density = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
    assert_eq!(density.scene.houses, 0);
}

#[test]
fn wet_house_footprints_are_rejected_without_leaving_partial_foundations() {
    let ecology = Ecology::new(42).with_biome(Some(SceneBiome::Meadow));
    let scene = VillageScene::generate(
        42,
        [-32, -32],
        64,
        VillageMode::Showcase,
        &ecology,
        |x, z| {
            if (9..=17).contains(&x) && (-16..=-8).contains(&z) {
                f32::NAN
            } else {
                25.
            }
        },
        || false,
    )
    .unwrap();
    assert_eq!(scene.houses, 3);
    assert!(!scene.blocks.contains_key(&[13, 25, -12]));
    assert!(!scene.blocks.contains_key(&[13, 26, -9]));
}
