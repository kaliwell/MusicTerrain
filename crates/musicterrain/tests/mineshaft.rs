#![cfg(feature = "native")]
use musicterrain::{
    materials::Material,
    mineshaft::{MineMode, MineScene, UndergroundView},
    preset::builtins,
    terrain::{GenerationRevision, TerrainSnapshot},
    village::VillageMode,
    visual::{SceneOptions, mesh_snapshot_with_options},
};
use std::collections::{BTreeMap, BTreeSet};
fn mine(origin: [i32; 2], size: u32) -> MineScene {
    MineScene::generate(42, origin, size, MineMode::Showcase, |_, _| 48., || false).unwrap()
}
#[test]
fn mine_has_unblocked_connected_rails_supports_webs_and_lanterns() {
    let m = mine([-48, -48], 96);
    assert_eq!(m.mines, 1);
    assert!(m.supports > 8 && m.rails.len() > 100 && !m.webs.is_empty() && !m.lamps.is_empty());
    for (p, axis) in &m.rails {
        assert!(*axis <= 1);
        assert!(!m.blocks.contains_key(p));
        assert!(m.air.contains(p));
        assert_eq!(
            m.blocks.get(&[p[0], p[1] - 1, p[2]]),
            Some(&Material::OakPlanks)
        );
    }
    let y = m.rails[0].0[1];
    for x in [-18, 0, 18] {
        assert!(
            m.rails.iter().any(|(p, _)| *p == [x, y, 0]),
            "blocked rail junction"
        );
    }
    let walkable: BTreeSet<_> = m
        .air
        .iter()
        .filter(|p| p[1] == y && !m.blocks.contains_key(*p))
        .copied()
        .collect();
    let mut visited = BTreeSet::new();
    let mut todo = vec![[0, y, 0]];
    while let Some(p) = todo.pop() {
        if !walkable.contains(&p) || !visited.insert(p) {
            continue;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            todo.push([p[0] + dx, y, p[2] + dz]);
        }
    }
    assert!(m.rails.iter().all(|(p, _)| visited.contains(p)));
}
#[test]
fn full_and_clipped_mine_plans_match_in_arbitrary_region_order() {
    let whole = mine([-32, -32], 64);
    let mut blocks = BTreeMap::new();
    let mut air = BTreeSet::new();
    let mut rails = Vec::new();
    for origin in [[0, 0], [-32, 0], [0, -32], [-32, -32]] {
        let part = mine(origin, 32);
        blocks.extend(part.blocks);
        air.extend(part.air);
        rails.extend(part.rails);
    }
    rails.sort();
    assert_eq!(whole.blocks, blocks);
    assert_eq!(whole.air, air);
    assert_eq!(whole.rails, rails);
    assert_eq!(whole.blocks, mine([-32, -32], 64).blocks);
}
#[test]
fn mines_respect_bounds_roof_clearance_cancellation_and_seed() {
    assert!(
        MineScene::generate(42, [i32::MIN, 0], 64, MineMode::Auto, |_, _| 48., || false).is_none()
    );
    assert!(
        MineScene::generate(42, [0, 0], u32::MAX, MineMode::Auto, |_, _| 48., || false).is_none()
    );
    assert!(
        MineScene::generate(42, [-32, -32], 64, MineMode::Showcase, |_, _| 48., || true).is_none()
    );
    for surface in [f32::NAN, 8., 12.] {
        assert!(
            MineScene::generate(
                42,
                [-32, -32],
                64,
                MineMode::Showcase,
                |_, _| surface,
                || false
            )
            .unwrap()
            .air
            .is_empty()
        );
    }
    let a =
        MineScene::generate(42, [-128, -128], 256, MineMode::Auto, |_, _| 48., || false).unwrap();
    let b =
        MineScene::generate(43, [-128, -128], 256, MineMode::Auto, |_, _| 48., || false).unwrap();
    assert_ne!(a.blocks, b.blocks);
    assert!(a.air.len() < 100000 && a.blocks.len() < 60000);
}
#[test]
fn rendered_mine_views_and_switches_emit_real_geometry_and_remain_section_local() {
    let mut parameters = builtins().remove(0).parameters;
    parameters.amplitude = 0.;
    parameters.detail = 0.;
    parameters.base_height = 48.;
    let snapshot = TerrainSnapshot {
        seed: 42,
        revision: GenerationRevision(1),
        parameters,
        audio_time: 0.,
    };
    let options = SceneOptions {
        mines: MineMode::Showcase,
        villages: VillageMode::Off,
        underground_view: UndergroundView::Underground,
        ..SceneOptions::default()
    };
    let visible = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
    assert!(visible.scene.rails > 40);
    for material in [
        Material::Rail,
        Material::OakPlanks,
        Material::Deepslate,
        Material::CopperOre,
        Material::GoldOre,
        Material::DiamondOre,
    ] {
        assert!(
            visible.scene.material_quads[material.layer() as usize] > 0,
            "missing {material:?}"
        );
    }
    let cutaway = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            underground_view: UndergroundView::Cutaway,
            ..options
        },
    )
    .unwrap();
    for material in [Material::Rail, Material::Cobweb, Material::Lantern] {
        assert!(
            cutaway.scene.material_quads[material.layer() as usize] > 0,
            "missing {material:?}"
        );
    }
    assert_ne!(visible.checksum, cutaway.checksum);
    for (_, mesh) in &visible.sections {
        assert!(mesh.solid.iter().chain(&mesh.translucent).all(|v| {
            v.pos
                .iter()
                .all(|p| p.is_finite() && (0.0..=16.0).contains(p))
        }));
    }
    let off = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            mines: MineMode::Off,
            ..options
        },
    )
    .unwrap();
    assert_eq!(off.scene.rails, 0);
    assert_eq!(off.scene.material_quads[Material::Rail.layer() as usize], 0);
    let closed = mesh_snapshot_with_options(
        &snapshot,
        64,
        &SceneOptions {
            caves: false,
            ..options
        },
    )
    .unwrap();
    assert_eq!(closed.scene.mines, 0);
    assert_eq!(closed.scene.carved_voxels, 0); // clipping does not masquerade as real cave carving
}

#[test]
fn automatic_mines_match_tiled_queries_near_jittered_cell_boundaries() {
    let mut witnessed = false;
    for seed in 0..6 {
        let whole =
            MineScene::generate(seed, [80, -32], 64, MineMode::Auto, |_, _| 48., || false).unwrap();
        if whole.air.is_empty() {
            continue;
        }
        witnessed = true;
        let mut air = BTreeSet::new();
        let mut blocks = BTreeMap::new();
        let mut rails = Vec::new();
        for origin in [[112, 0], [80, -32], [112, -32], [80, 0]] {
            let p = MineScene::generate(seed, origin, 32, MineMode::Auto, |_, _| 48., || false)
                .unwrap();
            air.extend(p.air);
            blocks.extend(p.blocks);
            rails.extend(p.rails);
        }
        rails.sort();
        assert_eq!(whole.air, air);
        assert_eq!(whole.blocks, blocks);
        assert_eq!(whole.rails, rails);
    }
    assert!(witnessed, "regression must exercise a real mine");
}

#[test]
fn actual_cave_faces_receive_biome_details_and_new_ore_materials() {
    let mut seen = [false; 4];
    for seed in [0, 1, 2, 3, 42] {
        let mut parameters = builtins().remove(0).parameters;
        parameters.amplitude = 0.;
        parameters.detail = 0.;
        parameters.base_height = 48.;
        let snapshot = TerrainSnapshot {
            seed,
            revision: GenerationRevision(1),
            parameters,
            audio_time: 0.,
        };
        let options = SceneOptions {
            mines: MineMode::Off,
            villages: VillageMode::Off,
            underground_view: UndergroundView::Cutaway,
            ..SceneOptions::default()
        };
        let mesh = mesh_snapshot_with_options(&snapshot, 64, &options).unwrap();
        for (i, mat) in [
            Material::Moss,
            Material::Dripstone,
            Material::Amethyst,
            Material::GlowLichen,
        ]
        .iter()
        .enumerate()
        {
            seen[i] |= mesh.scene.material_quads[mat.layer() as usize] > 0;
        }
    }
    assert!(
        seen.into_iter().all(|v| v),
        "cave biome material was never emitted"
    );
}
