use musicterrain::underground::{UndergroundBiome, UndergroundField};

#[test]
fn cave_columns_are_seeded_repeatable_and_region_order_independent() {
    let field = UndergroundField::new(42);
    let other = UndergroundField::new(43);
    let mut different = 0;
    let mut forward = Vec::new();
    for z in -48..48 {
        for x in -48..48 {
            let column = field.column(x, z, 32.);
            assert_eq!(column, UndergroundField::new(42).column(x, z, 32.));
            different += usize::from(column != other.column(x, z, 32.));
            forward.push((x, z, column));
        }
    }
    assert!(different > 100);
    for (x, z, column) in forward.into_iter().rev() {
        assert_eq!(column, field.column(x, z, 32.));
    }
}

#[test]
fn chambers_and_tunnels_preserve_bedrock_roof_and_finite_bounded_intervals() {
    let field = UndergroundField::new(42);
    let mut carved = 0;
    let mut spacious = 0;
    for z in -96..96 {
        for x in -96..96 {
            let column = field.column(x, z, 32.);
            assert!(column.intervals().len() <= 32);
            for r in column.intervals() {
                assert!(r.iter().all(|v| v.is_finite()));
                assert!(r[0] >= 3. && r[1] <= 28. && r[0] <= r[1]);
                spacious += usize::from(r[1] - r[0] > 8.);
            }
            for y in 0..3 {
                assert!(!column.contains(y));
            }
            for y in 29..34 {
                assert!(!column.contains(y));
            }
            carved += usize::from((3..28).any(|y| column.contains(y)));
        }
    }
    assert!(
        carved > 2000 && spacious > 100,
        "chambers/tunnels should occupy meaningful volume"
    );
}

#[test]
fn cave_biomes_and_limited_aquifers_are_reachable() {
    let field = UndergroundField::new(42);
    let mut found = [false; 4];
    let mut water = 0;
    let mut dry = 0;
    for z in (-2048..2048).step_by(8) {
        for x in (-2048..2048).step_by(8) {
            let column = field.column(x, z, 32.);
            found[match column.biome() {
                UndergroundBiome::Stone => 0,
                UndergroundBiome::Lush => 1,
                UndergroundBiome::Dripstone => 2,
                UndergroundBiome::Deep => 3,
            }] = true;
            if let Some(level) = column.water_level() {
                assert_eq!(level, 6);
                assert!(column.intervals().iter().any(|r| r[0] <= level as f32));
                water += 1;
            } else {
                dry += 1;
            }
        }
    }
    assert!(found.into_iter().all(|v| v));
    assert!(water > 0 && dry > water);
}

#[test]
fn extreme_inputs_and_low_surfaces_remain_safe() {
    let field = UndergroundField::new(42);
    for (x, z, height) in [
        (0, 0, f32::NAN),
        (0, 0, f32::INFINITY),
        (0, 0, 8.),
        (i32::MIN, 0, 32.),
        (0, i32::MAX, 32.),
    ] {
        let column = field.column(x, z, height);
        assert!(column.intervals().is_empty());
        assert_eq!(column.water_level(), None);
    }
    for (x, z) in [(-30_000_000, -30_000_000), (30_000_000, 30_000_000)] {
        let column = field.column(x, z, 128.);
        assert!(column.intervals().iter().flatten().all(|v| v.is_finite()));
    }
}

#[test]
fn cave_columns_cross_global_cell_edges_without_special_case_seams() {
    let field = UndergroundField::new(42);
    // Full request and reversed tile evaluation sample the same global integer columns.
    for edge in [-128, -64, 0, 64, 128] {
        let whole: Vec<_> = (-64..64)
            .map(|z| (field.column(edge - 1, z, 32.), field.column(edge, z, 32.)))
            .collect();
        for z in (-64..64).rev() {
            assert_eq!(
                whole[(z + 64) as usize],
                (field.column(edge - 1, z, 32.), field.column(edge, z, 32.))
            );
        }
    }
}
