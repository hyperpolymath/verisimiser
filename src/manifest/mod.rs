// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub verisimiser: VeriSimiserConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub tier1: Tier1Config,
    #[serde(default)]
    pub tier2: Tier2Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeriSimiserConfig { pub name: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "target-db")]
    pub target_db: String,
    #[serde(rename = "connection-string", default)]
    pub connection_string: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier1Config {
    #[serde(rename = "drift-detection", default = "default_true")]
    pub drift_detection: bool,
    #[serde(default = "default_true")]
    pub provenance: bool,
    #[serde(rename = "temporal-versioning", default = "default_true")]
    pub temporal_versioning: bool,
}

impl Default for Tier1Config {
    fn default() -> Self { Self { drift_detection: true, provenance: true, temporal_versioning: true } }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tier2Config {
    #[serde(default)] pub graph: bool,
    #[serde(default)] pub vector: bool,
    #[serde(default)] pub tensor: bool,
    #[serde(default)] pub semantic: bool,
    #[serde(default)] pub document: bool,
    #[serde(default)] pub spatial: bool,
}

fn default_true() -> bool { true }

pub fn load_manifest(path: &str) -> Result<Manifest> {
    let c = std::fs::read_to_string(path).with_context(|| format!("Read: {}", path))?;
    toml::from_str(&c).with_context(|| format!("Parse: {}", path))
}

pub fn init_manifest(database: &str) -> Result<()> {
    let path = "verisimiser.toml";
    if std::path::Path::new(path).exists() { anyhow::bail!("already exists"); }
    let t = format!(r#"# VeriSimiser manifest — augment {database} with VeriSimDB octad capabilities

[verisimiser]
name = "my-augmented-db"

[database]
target-db = "{database}"
# connection-string = "{database}://localhost/mydb"

# Tier 1: true piggybacks (sidecar storage only, your database untouched)
[tier1]
drift-detection = true
provenance = true
temporal-versioning = true

# Tier 2: augmentation overlays (additional storage alongside your database)
[tier2]
graph = false
vector = false
tensor = false
semantic = false
document = false
spatial = false
"#);
    std::fs::write(path, t)?;
    println!("Created verisimiser.toml for {}", database);
    Ok(())
}

pub fn print_status(m: &Manifest) {
    println!("=== VeriSimiser: {} ===", m.verisimiser.name);
    println!("Target DB: {}", m.database.target_db);
    println!();
    println!("Tier 1 (piggybacks):");
    println!("  Drift detection:    {}", if m.tier1.drift_detection { "ON" } else { "off" });
    println!("  Provenance:         {}", if m.tier1.provenance { "ON" } else { "off" });
    println!("  Temporal versioning:{}", if m.tier1.temporal_versioning { "ON" } else { "off" });
    println!();
    let t2 = [("Graph", m.tier2.graph), ("Vector", m.tier2.vector), ("Tensor", m.tier2.tensor),
              ("Semantic", m.tier2.semantic), ("Document", m.tier2.document), ("Spatial", m.tier2.spatial)];
    println!("Tier 2 (overlays):");
    for (name, on) in t2 { println!("  {:18} {}", format!("{}:", name), if on { "ON" } else { "off" }); }
}
