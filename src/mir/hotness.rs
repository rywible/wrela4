#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticHotnessSeed {
    root_distance: u32,
    loop_depth: u32,
    score: u32,
}

impl StaticHotnessSeed {
    pub fn new(root_symbol: impl Into<String>, root_distance: u32, loop_depth: u32) -> Self {
        let _ = root_symbol.into();
        let score = 100u32
            .saturating_sub(root_distance.saturating_mul(8))
            .saturating_add(loop_depth.saturating_mul(28));
        Self {
            root_distance,
            loop_depth,
            score,
        }
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn reason(&self) -> String {
        format!(
            "root-distance={} loop-depth={}",
            self.root_distance, self.loop_depth
        )
    }
}
