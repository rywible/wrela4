#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Trap,
    Io,
    Volatile,
    Time,
    Entropy,
    Arena,
    Mutate,
    Dma,
    Block,
    AddressArithmetic,
    Assembly,
}

impl Effect {
    pub const ALL: [Effect; 11] = [
        Effect::Trap,
        Effect::Io,
        Effect::Volatile,
        Effect::Time,
        Effect::Entropy,
        Effect::Arena,
        Effect::Mutate,
        Effect::Dma,
        Effect::Block,
        Effect::AddressArithmetic,
        Effect::Assembly,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Trap" => Some(Effect::Trap),
            "Io" => Some(Effect::Io),
            "Volatile" => Some(Effect::Volatile),
            "Time" => Some(Effect::Time),
            "Entropy" => Some(Effect::Entropy),
            "Arena" => Some(Effect::Arena),
            "Mutate" => Some(Effect::Mutate),
            "Dma" => Some(Effect::Dma),
            "Block" => Some(Effect::Block),
            "AddressArithmetic" => Some(Effect::AddressArithmetic),
            "Assembly" => Some(Effect::Assembly),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Effect::Trap => "Trap",
            Effect::Io => "Io",
            Effect::Volatile => "Volatile",
            Effect::Time => "Time",
            Effect::Entropy => "Entropy",
            Effect::Arena => "Arena",
            Effect::Mutate => "Mutate",
            Effect::Dma => "Dma",
            Effect::Block => "Block",
            Effect::AddressArithmetic => "AddressArithmetic",
            Effect::Assembly => "Assembly",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet(u16);

impl EffectSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn single(effect: Effect) -> Self {
        Self(1 << effect as u16)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, effect: Effect) -> bool {
        (self.0 & (1 << effect as u16)) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.bits() == 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        (self.0 & !other.0) == 0
    }

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub fn names_in_order(self) -> Vec<&'static str> {
        Effect::ALL
            .into_iter()
            .filter(|effect| self.contains(*effect))
            .map(Effect::name)
            .collect()
    }
}

pub fn render_effect_set(effects: EffectSet) -> String {
    let names = effects.names_in_order();
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(",")
    }
}

pub fn parse_effect_set(text: &str) -> Result<EffectSet, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "none" {
        return Ok(EffectSet::empty());
    }
    let mut bits = 0u16;
    for part in trimmed.split(',') {
        let effect =
            Effect::from_name(part.trim()).ok_or_else(|| format!("unknown effect `{part}`"))?;
        bits |= 1 << effect as u16;
    }
    Ok(EffectSet::from_bits(bits))
}

#[cfg(test)]
mod tests {
    use super::{Effect, EffectSet};

    #[test]
    fn effect_sets_union_and_subset() {
        let trap = EffectSet::single(Effect::Trap);
        let io = EffectSet::single(Effect::Io);
        let both = trap.union(io);

        assert!(trap.is_subset_of(both));
        assert!(io.is_subset_of(both));
        assert!(both.contains(Effect::Trap));
        assert!(both.contains(Effect::Io));
        assert_eq!(EffectSet::empty().bits(), 0);
    }
}
