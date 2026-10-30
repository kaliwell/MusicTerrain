//! Voxel-aligned horizontal footprints, extruded through the preview's vertical layers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainShape {
    #[default]
    Square,
    Cylinder,
    Hexagon,
    Diamond,
}
impl TerrainShape {
    pub const ALL: [Self; 4] = [Self::Square, Self::Cylinder, Self::Hexagon, Self::Diamond];
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "square" => Some(Self::Square),
            "cylinder" => Some(Self::Cylinder),
            "hexagon" => Some(Self::Hexagon),
            "diamond" => Some(Self::Diamond),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Square => "SQUARE",
            Self::Cylinder => "CYLINDER",
            Self::Hexagon => "HEXAGON",
            Self::Diamond => "DIAMOND",
        }
    }
    pub fn next(self) -> Self {
        Self::ALL[(Self::ALL.iter().position(|s| *s == self).unwrap() + 1) % Self::ALL.len()]
    }
    pub fn contains(self, x: f32, z: f32, size: u32) -> bool {
        let r = size as f32 * 0.5;
        if !x.is_finite() || !z.is_finite() || x.abs() > r || z.abs() > r {
            return false;
        }
        match self {
            Self::Square => true,
            Self::Cylinder => x * x + z * z <= r * r,
            Self::Hexagon => {
                z.abs() <= r * 3.0_f32.sqrt() * 0.5
                    && x.abs() * 3.0_f32.sqrt() + z.abs() <= r * 3.0_f32.sqrt()
            }
            Self::Diamond => x.abs() + z.abs() <= r,
        }
    }
}
