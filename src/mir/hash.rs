#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableHash(u64);

impl StableHash {
    pub const ZERO: StableHash = StableHash(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for StableHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

pub fn stable_hash_bytes(domain: &str, bytes: &[u8]) -> StableHash {
    let mut hash = FNV_OFFSET;
    for byte in domain
        .as_bytes()
        .iter()
        .chain([0u8].iter())
        .chain(bytes.iter())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    StableHash::new(hash)
}

pub fn stable_hash_text(domain: &str, text: &str) -> StableHash {
    stable_hash_bytes(domain, text.as_bytes())
}

use crate::mir::{PassSet, TargetProfile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizerKey {
    region_hash: StableHash,
    target_profile: TargetProfile,
    pass_config_hash: StableHash,
    compiler_version: String,
    certificate_version: u16,
    stable_hash: StableHash,
}

impl OptimizerKey {
    pub fn try_new(
        region_hash: StableHash,
        target_profile: TargetProfile,
        pass_set: &PassSet,
        compiler_version: &str,
        certificate_version: u16,
    ) -> Result<Self, String> {
        if compiler_version.is_empty() {
            return Err("compiler version must not be empty".to_string());
        }
        let pass_config_hash = pass_set.stable_hash();
        let line = format!(
            "v1|region={}|target={}|passes={}|compiler={}|cert={}",
            region_hash,
            target_profile.name(),
            pass_config_hash,
            compiler_version,
            certificate_version
        );
        let stable_hash = stable_hash_text("wmir.optimizer-key.v1", &line);
        Ok(Self {
            region_hash,
            target_profile,
            pass_config_hash,
            compiler_version: compiler_version.to_string(),
            certificate_version,
            stable_hash,
        })
    }

    pub fn new(
        region_hash: StableHash,
        target_profile: TargetProfile,
        pass_set: &PassSet,
        compiler_version: &str,
        certificate_version: u16,
    ) -> Self {
        Self::try_new(
            region_hash,
            target_profile,
            pass_set,
            compiler_version,
            certificate_version,
        )
        .expect("valid optimizer key")
    }

    pub fn stable_hash(&self) -> StableHash {
        self.stable_hash
    }

    pub fn as_line(&self) -> String {
        format!(
            "key={} region={} target={} passes={} compiler={} cert={}",
            self.stable_hash,
            self.region_hash,
            self.target_profile.name(),
            self.pass_config_hash,
            self.compiler_version,
            self.certificate_version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{StableHash, stable_hash_text};
    use crate::mir::{Pass, PassSet};

    #[test]
    fn stable_hash_text_is_deterministic_and_domain_separated() {
        let a = stable_hash_text("wmir.module.v0", "op binary +");
        let b = stable_hash_text("wmir.module.v0", "op binary +");
        let c = stable_hash_text("wmir.region.v0", "op binary +");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, StableHash::ZERO);
    }

    #[test]
    fn pass_set_hash_depends_on_enabled_passes() {
        let scalar = PassSet::only(Pass::ScalarPeephole).stable_hash();
        let fusion = PassSet::only(Pass::TableLoopFusion).stable_hash();

        assert_ne!(scalar, fusion);
    }
}
