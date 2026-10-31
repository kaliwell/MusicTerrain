//! Shared bounded 4D cave slices for native and web previews.
/// Sample a continuous 4D field once per interval, keeping the voxel loop cheap.
/// W=0 reproduces the authored cave columns; other slices move and reshape them.
pub fn cave_slice(
    range: [f32; 2],
    x: i32,
    z: i32,
    surface: i32,
    margin: f32,
    seed: i64,
    w: f32,
) -> [f32; 2] {
    let delta = if w == 0.0 {
        0.0
    } else {
        let p = [
            f64::from(x) * 0.045,
            f64::from((range[0] + range[1]) * 0.5) * 0.06,
            f64::from(z) * 0.045,
            f64::from(w),
        ];
        crate::noise4::value(seed, p) - crate::noise4::value(seed, [p[0], p[1], p[2], 0.0])
    };
    let shift = delta * 3.0;
    let expansion = delta.abs() * 1.5;
    let roof = range[1].max(surface as f32 - 4.0);
    [
        (range[0] - margin + shift - expansion).max(3.0),
        (range[1] + margin + shift + expansion).min(roof),
    ]
}
