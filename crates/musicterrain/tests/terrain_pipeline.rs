use musicterrain::{
    dsp::AudioFeatures,
    mapping::{AudioSource, CombineMode, MappingGraph, MappingRule, MappingState, ResponseCurve},
    preset::{VisualizationPreset, builtins},
    terrain::{
        GenerationRevision, NoiseType, TargetParameter, TerrainField, TerrainGenerationRequest,
        TerrainParameters, TerrainSnapshot,
    },
};

#[test]
fn seeded_horizon_noise_is_reproducible_and_seed_sensitive() {
    let p = TerrainParameters::default();
    let a = TerrainField::new(42);
    let b = TerrainField::new(42);
    let c = TerrainField::new(43);
    let values = |f: &TerrainField| f.heightmap([-32, -32], [64, 64], &p).unwrap();
    assert_eq!(values(&a), values(&b));
    assert_ne!(values(&a), values(&c));
}
#[test]
fn adjacent_tiles_sample_identical_world_edges_in_any_order() {
    let f = TerrainField::new(-891);
    let p = TerrainParameters::default();
    let right = f.heightmap([0, -16], [17, 17], &p).unwrap();
    let left = f.heightmap([-16, -16], [17, 17], &p).unwrap();
    for z in 0..17 {
        assert_eq!(left[z * 17 + 16].to_bits(), right[z * 17].to_bits());
    }
    assert_eq!(left, f.heightmap([-16, -16], [17, 17], &p).unwrap());
}
#[test]
fn volume_generation_is_order_independent_and_bounded() {
    let field = TerrainField::new(7);
    let snapshot = TerrainSnapshot {
        revision: GenerationRevision(1),
        seed: 7,
        parameters: TerrainParameters::default(),
        audio_time: 0.0,
    };
    let a = TerrainGenerationRequest {
        origin: [0, 0, 0],
        size: [16, 32, 16],
        snapshot: snapshot.clone(),
    };
    let b = TerrainGenerationRequest {
        origin: [16, 0, 0],
        size: [16, 32, 16],
        snapshot,
    };
    let first = field.generate(&a).unwrap();
    let next = field.generate(&b).unwrap();
    assert_eq!(next, field.generate(&b).unwrap());
    assert_eq!(first, field.generate(&a).unwrap());
    let mut invalid = a;
    invalid.size = [u32::MAX; 3];
    assert!(field.generate(&invalid).is_err());
}
#[test]
fn all_noise_modes_and_extreme_parameters_remain_finite() {
    let f = TerrainField::new(i64::MIN);
    let mut p = TerrainParameters::default();
    for target in TargetParameter::ALL {
        target.set(&mut p, target.bounds()[1]);
    }
    p.octaves = 8;
    for mode in NoiseType::ALL {
        p.noise_type = mode;
        p.validate().unwrap();
        for x in [
            -30_000_000.0,
            -16.0,
            0.0,
            16.0,
            30_000_000.0,
            f64::MAX,
            f64::NAN,
        ] {
            assert!(f.height(x, x, &p).is_finite());
            assert!(f.density(x, x, x, &p).is_finite());
        }
    }
    p.frequency = f32::NAN;
    assert!(p.validate().is_err());
    assert!(f.height(1.0, 1.0, &p).is_finite());
}
#[test]
fn noise_modes_produce_distinct_seeded_height_fields() {
    let field = TerrainField::new(42);
    let mut outputs = Vec::new();
    for noise_type in NoiseType::ALL {
        let p = TerrainParameters {
            noise_type,
            detail: 0.0,
            ..Default::default()
        };
        let heights = field.heightmap([-32, -32], [16, 16], &p).unwrap();
        assert_eq!(heights, field.heightmap([-32, -32], [16, 16], &p).unwrap());
        assert!(
            outputs
                .iter()
                .all(|previous: &Vec<f32>| previous != &heights),
            "{noise_type:?}"
        );
        outputs.push(heights);
    }
    let preset = builtins().remove(0);
    let mut quiet = MappingState::default();
    let mut loud = MappingState::default();
    let a = preset
        .mappings
        .evaluate(
            &AudioFeatures::default(),
            &preset.parameters,
            1.0,
            &mut quiet,
        )
        .unwrap();
    let b = preset
        .mappings
        .evaluate(
            &AudioFeatures {
                rms: 0.8,
                ..Default::default()
            },
            &preset.parameters,
            1.0,
            &mut loud,
        )
        .unwrap();
    assert!(b.cave_w > a.cave_w, "default music mapping must drive W");
}
#[test]
fn parameters_have_observable_effects_and_three_dimensional_topology() {
    let f = TerrainField::new(42);
    let p = TerrainParameters::default();
    let mut raised = p.clone();
    raised.base_height += 10.0;
    assert!((f.height(10.0, 20.0, &raised) - f.height(10.0, 20.0, &p) - 10.0).abs() < 1e-5);
    let mut caverns = p.clone();
    caverns.cave_threshold = 0.0;
    let solid = f.density(0.0, -30.0, 0.0, &p);
    let carved = f.density(0.0, -30.0, 0.0, &caverns);
    assert!(solid > carved);
    let mut islands = p.clone();
    islands.island_strength = 1.0;
    let mut changed = false;
    for y in 20..80 {
        for x in 0..32 {
            if f.density(f64::from(x), f64::from(y), 0.0, &islands) > 0.0
                && f.density(f64::from(x), f64::from(y), 0.0, &p) <= 0.0
            {
                changed = true;
            }
        }
    }
    assert!(changed);
}
#[test]
fn mapping_combines_in_declared_order_and_pauses_stably() {
    let mut a = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 10.0]);
    a.attack = 0.0;
    a.release = 0.0;
    a.curve = ResponseCurve::Linear;
    a.input_range = [0.0, 1.0];
    a.combine = CombineMode::Replace;
    let mut b = a.clone();
    b.output_range = [1.0, 2.0];
    b.combine = CombineMode::Multiply;
    let graph = MappingGraph {
        rules: vec![a.clone(), b.clone()],
    };
    let features = AudioFeatures {
        bands: [1.0; 7],
        ..AudioFeatures::default()
    };
    let mut state = MappingState::default();
    let p = TerrainParameters::default();
    let value = graph.evaluate(&features, &p, 0.01, &mut state).unwrap();
    assert_eq!(value.amplitude, 20.0);
    let paused = graph
        .evaluate(&AudioFeatures::default(), &p, 0.0, &mut state)
        .unwrap();
    assert_eq!(paused, value);
    let reverse = MappingGraph { rules: vec![b, a] }
        .evaluate(&features, &p, 0.01, &mut MappingState::default())
        .unwrap();
    assert_eq!(reverse.amplitude, 10.0);
}
#[test]
fn mapping_rejects_bad_ranges_and_resets_after_rule_edit() {
    let mut rule = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 20.0]);
    rule.attack = 0.1;
    let mut graph = MappingGraph { rules: vec![rule] };
    let mut state = MappingState::default();
    let base = TerrainParameters::default();
    let f = AudioFeatures {
        bands: [1.0; 7],
        ..AudioFeatures::default()
    };
    graph.evaluate(&f, &base, 1.0, &mut state).unwrap();
    graph.rules[0].source = AudioSource::Band(1);
    let result = graph
        .evaluate(&AudioFeatures::default(), &base, 0.0, &mut state)
        .unwrap();
    assert_eq!(result.amplitude, base.amplitude);
    graph.rules[0].input_range = [0.0, 0.0];
    assert!(graph.validate().is_err());
}
#[test]
fn silent_and_nonfinite_features_cannot_explode_mapping() {
    let preset = &builtins()[0];
    let mut state = MappingState::default();
    let mut f = AudioFeatures::default();
    f.bands[0] = f32::NAN;
    f.rms = f32::INFINITY;
    let first = preset
        .mappings
        .evaluate(&f, &preset.parameters, 0.01, &mut state)
        .unwrap();
    first.validate().unwrap();
    let mut last = first;
    for _ in 0..1000 {
        last = preset
            .mappings
            .evaluate(&f, &preset.parameters, 0.01, &mut state)
            .unwrap();
    }
    let settled = preset
        .mappings
        .evaluate(&f, &preset.parameters, 0.01, &mut state)
        .unwrap();
    assert_eq!(settled, last);
}
#[test]
fn presets_are_valid_distinct_and_versioned() {
    let presets = builtins();
    let f = TerrainField::new(42);
    let mut outputs = Vec::new();
    for preset in &presets {
        preset.validate().unwrap();
        let json = preset.to_json().unwrap();
        assert_eq!(*preset, VisualizationPreset::from_json(&json).unwrap());
        assert_eq!(
            preset.hash().unwrap(),
            VisualizationPreset::from_json(&json)
                .unwrap()
                .hash()
                .unwrap()
        );
        outputs.push(f.heightmap([0, 0], [16, 16], &preset.parameters).unwrap());
    }
    for i in 0..outputs.len() {
        for j in i + 1..outputs.len() {
            assert_ne!(outputs[i], outputs[j]);
        }
    }
    let json = presets[0].to_json().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value["version"] = 0.into();
    assert_eq!(
        VisualizationPreset::from_json(&value.to_string()).unwrap(),
        presets[0]
    );
    value["version"] = 99.into();
    assert!(VisualizationPreset::from_json(&value.to_string()).is_err());
}

#[test]
fn seek_replay_and_sampling_order_do_not_change_fixed_hop_snapshots() {
    let preset = builtins().remove(0);
    let hop = 512.0 / 48_000.0;
    let frames: Vec<_> = (0..240)
        .map(|i| AudioFeatures {
            bands: [((i as f32 * 0.13).sin() * 0.25 + 0.25); 7],
            rms: 0.3,
            onset: i % 40 == 0,
            ..AudioFeatures::default()
        })
        .collect();
    let replay = |through: usize| {
        let mut state = MappingState::default();
        let mut p = preset.parameters.clone();
        for f in &frames[..=through] {
            p = preset
                .mappings
                .evaluate(f, &preset.parameters, hop, &mut state)
                .unwrap();
        }
        p
    };
    let mut state = MappingState::default();
    let mut forward = Vec::new();
    for f in &frames {
        forward.push(
            preset
                .mappings
                .evaluate(f, &preset.parameters, hop, &mut state)
                .unwrap(),
        );
    }
    for i in [239, 4, 81, 0, 183, 4] {
        assert_eq!(replay(i), forward[i]);
    }
}

#[test]
fn audio_coordinate_mapping_has_explicit_clamp_loop_and_finite_semantics() {
    use musicterrain::terrain::{AudioTimeMapping, TimeMappingMode};
    let mut map = AudioTimeMapping {
        origin_x: 100.0,
        blocks_per_second: 10.0,
        start_seconds: 2.0,
        end_seconds: 12.0,
        mode: TimeMappingMode::Clamped,
    };
    assert_eq!(map.time_at(120.0).unwrap(), Some(4.0));
    assert_eq!(map.time_at(-100.0).unwrap(), Some(2.0));
    assert_eq!(map.time_at(900.0).unwrap(), Some(12.0));
    map.mode = TimeMappingMode::Loop;
    assert_eq!(map.time_at(220.0).unwrap(), Some(4.0));
    assert_eq!(map.time_at(80.0).unwrap(), Some(10.0));
    map.mode = TimeMappingMode::Finite;
    assert_eq!(map.time_at(80.0).unwrap(), None);
    assert_eq!(map.time_at(200.0).unwrap(), Some(12.0));
    map.blocks_per_second = 0.0;
    assert!(map.validate().is_err());
}

#[test]
fn mapping_attack_release_response_is_bounded_and_reproducible() {
    let mut rule = MappingRule::band(0, TargetParameter::Amplitude, [0.0, 100.0]);
    rule.input_range = [0.0, 1.0];
    rule.curve = ResponseCurve::Linear;
    rule.combine = CombineMode::Replace;
    rule.attack = 0.1;
    rule.release = 0.4;
    let graph = MappingGraph { rules: vec![rule] };
    let base = TerrainParameters::default();
    let loud = AudioFeatures {
        bands: [1.0; 7],
        ..AudioFeatures::default()
    };
    let mut state = MappingState::default();
    let rise = graph
        .evaluate(&loud, &base, 0.1, &mut state)
        .unwrap()
        .amplitude;
    assert!((rise - 63.212).abs() < 0.01);
    let decay = graph
        .evaluate(&AudioFeatures::default(), &base, 0.1, &mut state)
        .unwrap()
        .amplitude;
    assert!(decay < rise && decay > rise * 0.7);
    state.reset();
    assert_eq!(
        graph
            .evaluate(&loud, &base, 0.1, &mut state)
            .unwrap()
            .amplitude,
        rise
    );
}

#[test]
fn preset_save_cannot_overwrite_existing_configuration() {
    let directory = std::env::temp_dir().join(format!(
        "musicterrain-preset-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("new-preset.json");
    let preset = builtins().remove(0);
    // Fresh temporary path with create_new; only remove the file this test owns.
    let unique = path.with_file_name(format!("preset-{}.json", preset.hash().unwrap()));
    preset.save(&unique).unwrap();
    let before = std::fs::read(&unique).unwrap();
    assert!(preset.save(&unique).is_err());
    assert_eq!(std::fs::read(&unique).unwrap(), before);
    assert_eq!(VisualizationPreset::load(&unique).unwrap(), preset);
    std::fs::remove_file(unique).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
