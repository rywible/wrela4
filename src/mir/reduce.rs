use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{MirModule, StableHash};

#[derive(Clone, Debug)]
pub struct ReductionRequest {
    failure: String,
    hash: StableHash,
    module: MirModule,
}

impl ReductionRequest {
    pub fn new(failure: impl Into<String>, hash: StableHash, module: MirModule) -> Self {
        Self {
            failure: failure.into(),
            hash,
            module,
        }
    }

    pub fn write_to(&self, dir: &Path) -> Result<PathBuf, String> {
        fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        let path = dir.join(format!("{}-{}.wmir", self.failure, self.hash));
        let mut text = String::new();
        text.push_str(&format!("; failure={}\n", self.failure));
        text.push_str(&format!("; hash={}\n", self.hash));
        text.push_str(&crate::mir::text::render_module(&self.module));
        fs::write(&path, text).map_err(|err| err.to_string())?;
        Ok(path)
    }
}

pub fn preserve_failure_artifact(
    failure: &str,
    domain: &str,
    module: &MirModule,
) -> Result<PathBuf, String> {
    let hash = crate::mir::stable_hash_text(domain, &crate::mir::text::render_module(module));
    let request = ReductionRequest::new(failure, hash, module.clone());
    let reduction_dir = PathBuf::from("target").join("wrela").join("reductions");
    request.write_to(&reduction_dir)
}

#[cfg(test)]
mod tests {
    use super::ReductionRequest;

    #[test]
    fn reducer_writes_artifact_for_failed_rewrite() {
        let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
        let dir = std::env::temp_dir().join(format!("wrela-reductions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let artifact = ReductionRequest::new(
            "rewrite-verifier-failure",
            crate::mir::StableHash::new(42),
            module,
        )
        .write_to(&dir)
        .unwrap();

        let text = std::fs::read_to_string(artifact).unwrap();
        assert!(text.contains("failure=rewrite-verifier-failure"));
        assert!(text.contains("hash=000000000000002a"));
    }
}
