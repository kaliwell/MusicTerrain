use musicterrain::{
    ecology::{DecorationKind, Ecology, SceneBiome, SurfaceKind, WorldDecoration},
    preset::builtins,
    terrain::TerrainField,
};
use std::collections::BTreeMap;

fn by_event(decorations: Vec<WorldDecoration>) -> BTreeMap<u64, WorldDecoration> {
    decorations.into_iter().map(|d| (d.event_id, d)).collect()
}
#[test]
fn seeded_layout_is_repeatable_and_different_seeds_change_it() {
    let ecology = Ecology::new(42);
    let a = ecology.decorations([-32, -32], 64, |_, _| 25.0);
    assert_eq!(a, Ecology::new(42).decorations([-32, -32], 64, |_, _| 25.0));
    assert_ne!(a, Ecology::new(43).decorations([-32, -32], 64, |_, _| 25.0));
    assert!(a.iter().all(|d| d.anchor[1] == 25));
    assert!(a.iter().any(|d| d.kind == DecorationKind::OakTree));
    let trees: Vec<_> = a
        .iter()
        .filter(|d| d.kind == DecorationKind::OakTree)
        .collect();
    for (i, first) in trees.iter().enumerate() {
        for second in &trees[i + 1..] {
            let dx = first.anchor[0] - second.anchor[0];
            let dz = first.anchor[2] - second.anchor[2];
            assert!(dx * dx + dz * dz >= 81, "tree crowns overlap");
        }
    }
}
#[test]
fn tiled_generation_and_edge_halos_match_one_whole_request_in_any_order() {
    let ecology = Ecology::new(-152);
    let whole = by_event(ecology.decorations([-32, -32], 64, |_, _| 25.0));
    let mut tiles = BTreeMap::new();
    for origin in [[0, 0], [-32, -32], [0, -32], [-32, 0]] {
        for (id, d) in by_event(ecology.decorations(origin, 32, |_, _| 25.0)) {
            if let Some(previous) = tiles.insert(id, d) {
                assert_eq!(previous, d);
            }
        }
    }
    assert_eq!(whole, tiles);
}
#[test]
fn decorations_follow_the_sampled_surface_and_avoid_water_steep_slopes_and_holes() {
    let ecology = Ecology::new(42);
    let low = ecology.decorations([-32, -32], 64, |_, _| 25.0);
    let high = by_event(ecology.decorations([-32, -32], 64, |_, _| 30.0));
    for d in low {
        if let Some(raised) = high.get(&d.event_id) {
            assert_eq!(raised.anchor[1], d.anchor[1] + 5);
        }
    }
    assert!(ecology.decorations([-32, -32], 64, |_, _| 8.0).is_empty());
    assert!(
        ecology
            .decorations([-32, -32], 64, |x, _| 25.0 + x as f32 * 4.0)
            .is_empty()
    );
    assert!(
        ecology
            .decorations([-32, -32], 64, |_, _| f32::NAN)
            .is_empty()
    );
    for d in ecology.decorations([-32, -32], 64, |_, _| 25.0) {
        assert!(!ecology.is_cave(d.anchor[0], d.anchor[1] - 1, d.anchor[2], 25.0));
    }
}
#[test]
fn default_bass_mountains_has_trees_grass_flowers_and_visible_cave_entrances() {
    let preset = builtins().remove(0);
    let terrain = TerrainField::new(42);
    let ecology = Ecology::new(42);
    let height = |x: i32, z: i32| {
        terrain
            .height(f64::from(x), f64::from(z), &preset.parameters)
            .round()
            .clamp(1.0, 128.0)
    };
    let decorations = ecology.decorations([-32, -32], 64, height);
    for kind in [
        DecorationKind::OakTree,
        DecorationKind::TallGrass,
        DecorationKind::Poppy,
        DecorationKind::Dandelion,
    ] {
        assert!(
            decorations.iter().any(|d| d.kind == kind),
            "default scene missing {kind:?}"
        );
    }
    let mut openings = 0;
    for z in -32..32 {
        for x in -32..32 {
            let h = height(x, z);
            if ecology.is_cave(x, h as i32 - 1, z, h) {
                openings += 1;
            }
        }
    }
    assert!(
        (8..200).contains(&openings),
        "unreasonable entrance coverage: {openings}"
    );
}
#[test]
fn cave_columns_match_signed_density_and_continue_across_world_cells() {
    let ecology = Ecology::new(42);
    let mut found_underground = false;
    for z in -32..32 {
        for x in -32..32 {
            let column = ecology.cave_column(x, z, 25.0);
            for y in 3..27 {
                let density = ecology.cave_density(f64::from(x), f64::from(y), f64::from(z), 25.0);
                if density.abs() > 1e-4 {
                    assert_eq!(column.contains(y), density < 0.0);
                }
                if y < 20 && column.contains(y) {
                    found_underground = true;
                }
            }
        }
    }
    assert!(found_underground);
    for x in [-48.0, 0.0, 48.0] {
        for z in -64..64 {
            let left = ecology.cave_density(x - 0.0001, 12.0, f64::from(z), 25.0);
            let right = ecology.cave_density(x + 0.0001, 12.0, f64::from(z), 25.0);
            assert!(
                (left - right).abs() < 0.05,
                "cave discontinuity at {x}, {z}"
            );
        }
    }
}
#[test]
fn ecology_requests_and_extreme_coordinates_are_bounded_and_stable() {
    let ecology = Ecology::new(42);
    assert!(ecology.decorations([0, 0], 0, |_, _| 25.0).is_empty());
    assert!(
        ecology
            .decorations([0, 0], u32::MAX, |_, _| 25.0)
            .is_empty()
    );
    assert!(
        ecology
            .decorations([i32::MIN, 0], 64, |_, _| 25.0)
            .is_empty()
    );
    let large = ecology.decorations([-128, -128], 256, |_, _| 25.0);
    assert!(large.len() < 2500);
    assert_eq!(large, ecology.decorations([-128, -128], 256, |_, _| 25.0));
    assert!(!ecology.is_cave(i32::MIN, 5, i32::MAX, 25.0));
    assert!(ecology.cave_density(f64::NAN, 0.0, 0.0, 25.0).is_finite());
    assert!(ecology.cave_column(0, 0, f32::NAN).intervals().is_empty());
}
#[test]
fn surface_profiles_distinguish_shores_meadows_forests_and_alpine() {
    let ecology = Ecology::new(42);
    let shore = ecology.surface_profile(0, 0, 8.0);
    assert_eq!(shore.biome, SceneBiome::Shore);
    assert_eq!(shore.top, SurfaceKind::Sand);
    let alpine = ecology.surface_profile(0, 0, 100.0);
    assert_eq!(alpine.biome, SceneBiome::Alpine);
    assert_eq!(alpine.top, SurfaceKind::Snow);
    let mut meadow = false;
    let mut forest = false;
    for z in (-512..512).step_by(32) {
        for x in (-512..512).step_by(32) {
            let profile = ecology.surface_profile(x, z, 25.0);
            meadow |= profile.biome == SceneBiome::Meadow;
            forest |= profile.biome == SceneBiome::Forest;
            assert_eq!(profile.top, SurfaceKind::Grass);
            assert_eq!(profile.subsoil, SurfaceKind::Dirt);
        }
    }
    assert!(meadow && forest);
}
