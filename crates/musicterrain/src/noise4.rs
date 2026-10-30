//! Seeded C2-continuous 4D value noise. W is periodic over 256 slices.
fn hash(seed: i64, p: [i64; 4]) -> u64 {
    let mut h = seed as u64;
    for (i, c) in p.into_iter().enumerate() {
        let c = if i == 3 { c.rem_euclid(256) } else { c };
        h ^= (c as u64).wrapping_mul(
            [
                0x9e3779b97f4a7c15,
                0xbf58476d1ce4e5b9,
                0x94d049bb133111eb,
                0xd6e8feb86659fd93,
            ][i],
        );
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }
    h ^ (h >> 27)
}
pub fn value(seed: i64, p: [f64; 4]) -> f32 {
    if p.iter().any(|v| !v.is_finite()) {
        return 0.0;
    }
    let base = p.map(|v| v.floor() as i64);
    let t: [f64; 4] = std::array::from_fn(|i| {
        let v = p[i] - p[i].floor();
        v * v * v * (v * (v * 6.0 - 15.0) + 10.0)
    });
    let mut sum = 0.0;
    for corner in 0..16 {
        let mut cell = base;
        let mut weight = 1.0;
        for i in 0..4 {
            let high = corner & (1 << i) != 0;
            cell[i] = cell[i].saturating_add(i64::from(high));
            weight *= if high { t[i] } else { 1.0 - t[i] };
        }
        sum += weight * ((hash(seed, cell) >> 40) as f64 / 8388607.5 - 1.0);
    }
    sum as f32
}
pub fn cellular(seed: i64, x: f64, y: f64, z: f64) -> f32 {
    if [x, y, z]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 30_000_000.0)
    {
        return 0.0;
    }
    let base = [x.floor() as i64, y.floor() as i64, z.floor() as i64];
    let mut distance = 3.0_f64;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = [base[0] + dx, base[1] + dy, base[2] + dz, 0];
                let h = hash(seed, cell);
                let point = [
                    cell[0] as f64 + (h & 1023) as f64 / 1024.0,
                    cell[1] as f64 + ((h >> 10) & 1023) as f64 / 1024.0,
                    cell[2] as f64 + ((h >> 20) & 1023) as f64 / 1024.0,
                ];
                distance = distance
                    .min((point[0] - x).powi(2) + (point[1] - y).powi(2) + (point[2] - z).powi(2));
            }
        }
    }
    (1.0 - 2.0 * (distance.sqrt() / 1.7320508075688772).min(1.0)) as f32
}
#[cfg(test)]
mod tests {
    #[test]
    fn w_slices_are_smooth_seeded_and_periodic() {
        let p = [0.31, 1.2, -3.7, 0.99999];
        let mut q = p;
        q[3] += 0.00002;
        assert!((super::value(42, p) - super::value(42, q)).abs() < 0.001);
        q[3] = p[3] + 256.0;
        assert!((super::value(42, p) - super::value(42, q)).abs() < 1e-6);
        q[3] = 2.5;
        assert_ne!(super::value(42, p), super::value(42, q));
        assert_ne!(super::value(42, p), super::value(43, p));
    }
}
