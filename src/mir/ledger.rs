use std::{collections::BTreeMap, fs, path::PathBuf};

use super::{OptimizerKey, StableHash, TargetProfile};

pub const LEDGER_MAX_ENTRIES: usize = 4096;
pub const LEDGER_MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerStatus {
    KnownWinner,
    KnownLoser,
    NeedsRemeasure,
}

impl LedgerStatus {
    fn as_str(self) -> &'static str {
        match self {
            LedgerStatus::KnownWinner => "known-winner",
            LedgerStatus::KnownLoser => "known-loser",
            LedgerStatus::NeedsRemeasure => "needs-remeasure",
        }
    }

    fn from_str(text: &str) -> Option<Self> {
        match text {
            "known-winner" => Some(LedgerStatus::KnownWinner),
            "known-loser" => Some(LedgerStatus::KnownLoser),
            "needs-remeasure" => Some(LedgerStatus::NeedsRemeasure),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerEntry {
    key_line: String,
    key_hash: StableHash,
    status: LedgerStatus,
    samples: u32,
    runtime_delta_percent: i32,
    code_size_delta_bytes: i32,
    sequence: u64,
}

impl LedgerEntry {
    pub fn known_winner(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self::new(
            key,
            LedgerStatus::KnownWinner,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
        )
    }

    pub fn known_loser(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self::new(
            key,
            LedgerStatus::KnownLoser,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
        )
    }

    pub fn needs_remeasure(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self::new(
            key,
            LedgerStatus::NeedsRemeasure,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
        )
    }

    fn new(
        key: OptimizerKey,
        status: LedgerStatus,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self {
            key_line: key.as_line(),
            key_hash: key.stable_hash(),
            status,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
            sequence: 0,
        }
    }

    pub fn status(&self) -> LedgerStatus {
        self.status
    }

    pub fn runtime_delta_percent(&self) -> i32 {
        self.runtime_delta_percent
    }

    fn to_line(&self) -> String {
        let escaped_key_line = self.key_line.replacen("key=", "key_text=", 1);
        format!(
            "v1\tseq={}\tkey={}\tstatus={}\tsamples={}\truntime_delta_percent={}\tcode_size_delta_bytes={}\t{}",
            self.sequence,
            self.key_hash,
            self.status.as_str(),
            self.samples,
            self.runtime_delta_percent,
            self.code_size_delta_bytes,
            escaped_key_line
        )
    }
}

pub struct OptimizationLedger {
    dir: PathBuf,
    target_profile: TargetProfile,
    entries: BTreeMap<StableHash, LedgerEntry>,
    corrupt_lines: usize,
    next_sequence: u64,
}

impl OptimizationLedger {
    pub fn open(dir: PathBuf, target_profile: TargetProfile) -> Result<Self, String> {
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let file = dir.join(format!("{}.ledger", target_profile.name()));
        let mut entries = BTreeMap::new();
        let mut corrupt_lines = 0usize;
        let mut next_sequence = 1u64;
        if let Ok(text) = fs::read_to_string(&file) {
            for line in text.lines() {
                match parse_line(line) {
                    Some(entry) => {
                        next_sequence = next_sequence.max(entry.sequence + 1);
                        entries.insert(entry.key_hash, entry);
                    }
                    None => corrupt_lines += 1,
                }
            }
        }
        Ok(Self {
            dir,
            target_profile,
            entries,
            corrupt_lines,
            next_sequence,
        })
    }

    pub fn record(&mut self, mut entry: LedgerEntry) -> Result<(), String> {
        entry.sequence = self.next_sequence;
        self.next_sequence += 1;
        self.entries.insert(entry.key_hash, entry);
        while self.entries.len() > LEDGER_MAX_ENTRIES {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.sequence)
                .map(|(key, _)| *key)
                .expect("non-empty ledger");
            self.entries.remove(&oldest);
        }
        Ok(())
    }

    pub fn lookup(&self, key: &OptimizerKey) -> Option<&LedgerEntry> {
        self.entries.get(&key.stable_hash())
    }

    pub fn corrupt_lines(&self) -> usize {
        self.corrupt_lines
    }

    pub fn flush(&self) -> Result<(), String> {
        let file = self
            .dir
            .join(format!("{}.ledger", self.target_profile.name()));
        let mut text = String::new();
        for entry in self.entries.values() {
            text.push_str(&entry.to_line());
            text.push('\n');
        }
        if text.len() as u64 > LEDGER_MAX_BYTES {
            return Err("ledger exceeds byte budget".to_string());
        }
        fs::write(file, text).map_err(|err| err.to_string())
    }
}

fn parse_line(line: &str) -> Option<LedgerEntry> {
    if !line.starts_with("v1\t") {
        return None;
    }
    let mut sequence = None;
    let mut key_hash = None;
    let mut status = None;
    let mut samples = None;
    let mut runtime_delta_percent = None;
    let mut code_size_delta_bytes = None;
    let mut key_line = None;
    for part in line.split('\t').skip(1) {
        let (name, value) = part.split_once('=')?;
        match name {
            "seq" => sequence = value.parse::<u64>().ok(),
            "key" => key_hash = u64::from_str_radix(value, 16).ok().map(StableHash::new),
            "key_text" => key_line = Some(format!("key={value}")),
            "status" => status = LedgerStatus::from_str(value),
            "samples" => samples = value.parse::<u32>().ok(),
            "runtime_delta_percent" => runtime_delta_percent = value.parse::<i32>().ok(),
            "code_size_delta_bytes" => code_size_delta_bytes = value.parse::<i32>().ok(),
            _ => {}
        }
    }
    Some(LedgerEntry {
        key_line: key_line?,
        key_hash: key_hash?,
        status: status?,
        samples: samples?,
        runtime_delta_percent: runtime_delta_percent?,
        code_size_delta_bytes: code_size_delta_bytes?,
        sequence: sequence?,
    })
}
