// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// JSON-family sidecar store (V-L2-F3, #146).
//
// An append-only document store that mirrors the `verisimdb_*` overlay
// tables, with the same runtime octad operations the SQLite path
// implements today: provenance hash-chains (incl. first-class forks),
// temporal versioning (monotonic, exactly-one-current), temporal drift,
// and age-based gc.
//
// One internal [`SidecarData`] model holds every collection; the on-disk
// [`JsonFormat`] (plain JSON / JSON-LD / NDJSON) is *purely a codec* over
// it, so the operations are written once and are format-independent.
//
// Concurrency model: load → mutate → atomic rewrite (temp file + rename).
// History is append-only at the *logical* level (rows are never mutated
// except `gc`); the physical file is rewritten atomically. Unlike the
// SQLite path — which serialises concurrent writers through the database
// write lock — this store assumes a single writer at a time. Cross-process
// write serialisation is a hardening follow-up.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::JsonFormat;
use crate::abi::ProvenanceEntry;
use crate::manifest::{OctadConfig, RetentionConfig};
use crate::tier1::drift::{DriftCategory, DriftReport, temporal_drift_score};
use crate::tier1::provenance::ForkPoint;

/// JSON-LD vocabulary IRI; bare `@type`/field terms expand against it.
const LD_VOCAB: &str = "https://verisimdb.org/ns#";
/// Reserved pseudo-table for scaffold metadata; ignored on read.
const META_TABLE: &str = "_meta";

// ---------------------------------------------------------------------------
// Row types — one per verisimdb_* table the runtime path touches
// ---------------------------------------------------------------------------

/// A row of `verisimdb_provenance_log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRow {
    pub hash: String,
    pub previous_hash: String,
    pub entity_id: String,
    pub table_name: String,
    pub operation: String,
    pub actor: String,
    /// ISO 8601 / RFC 3339.
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformation: Option<String>,
}

/// A branch tip in `verisimdb_provenance_chain_heads`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainHead {
    pub entity_id: String,
    pub head_hash: String,
}

/// A row of `verisimdb_temporal_versions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalRow {
    pub entity_id: String,
    pub table_name: String,
    pub version: u64,
    pub valid_from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    pub snapshot: String,
    pub operation: String,
}

/// A row of `verisimdb_lineage_graph`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRow {
    pub edge_id: String,
    pub source_entity: String,
    pub source_table: String,
    pub target_entity: String,
    pub target_table: String,
    pub derivation_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
}

/// A row of `verisimdb_access_policies`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPolicyRow {
    pub policy_id: String,
    pub target_table: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_column: Option<String>,
    pub principal: String,
    pub access_level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub created_at: String,
    pub active: bool,
}

/// The full in-memory sidecar model. The plain-JSON encoding is exactly
/// this struct (field renames are the table names); the other formats are
/// alternate codecs over the same data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarData {
    #[serde(default, rename = "verisimdb_provenance_log")]
    pub provenance_log: Vec<ProvenanceRow>,
    #[serde(default, rename = "verisimdb_provenance_chain_heads")]
    pub provenance_chain_heads: Vec<ChainHead>,
    #[serde(default, rename = "verisimdb_temporal_versions")]
    pub temporal_versions: Vec<TemporalRow>,
    #[serde(default, rename = "verisimdb_lineage_graph")]
    pub lineage_graph: Vec<LineageRow>,
    #[serde(default, rename = "verisimdb_access_policies")]
    pub access_policies: Vec<AccessPolicyRow>,
}

/// Rows purged per dimension by [`JsonStore::gc_purge`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcCounts {
    pub provenance: usize,
    pub temporal: usize,
    pub lineage: usize,
}

// ---------------------------------------------------------------------------
// Codec: SidecarData <-> {plain, ld, ndjson}
// ---------------------------------------------------------------------------

/// Map a `verisimdb_*` table name to its JSON-LD `@type` term.
fn ld_type_for(table: &str) -> &'static str {
    match table {
        "verisimdb_provenance_log" => "ProvenanceEntry",
        "verisimdb_provenance_chain_heads" => "ProvenanceChainHead",
        "verisimdb_temporal_versions" => "TemporalVersion",
        "verisimdb_lineage_graph" => "LineageEdge",
        "verisimdb_access_policies" => "AccessPolicy",
        _ => "Thing",
    }
}

/// Inverse of [`ld_type_for`], tolerant of a `verisimdb:`/vocab IRI prefix.
/// Returns `None` for the reserved `Meta` type (skipped on read).
fn table_for_ld_type(ld_type: &str) -> Result<Option<&'static str>> {
    let term = ld_type.rsplit(['#', ':']).next().unwrap_or(ld_type);
    match term {
        "ProvenanceEntry" => Ok(Some("verisimdb_provenance_log")),
        "ProvenanceChainHead" => Ok(Some("verisimdb_provenance_chain_heads")),
        "TemporalVersion" => Ok(Some("verisimdb_temporal_versions")),
        "LineageEdge" => Ok(Some("verisimdb_lineage_graph")),
        "AccessPolicy" => Ok(Some("verisimdb_access_policies")),
        "Meta" => Ok(None),
        other => anyhow::bail!("unknown JSON-LD @type {other:?} in sidecar @graph"),
    }
}

/// Encode `data` to a string in the requested `format`.
pub fn encode(data: &SidecarData, format: JsonFormat) -> Result<String> {
    match format {
        JsonFormat::Plain => Ok(serde_json::to_string_pretty(data)?),
        JsonFormat::Ndjson => encode_ndjson(data),
        JsonFormat::Ld => encode_ld(data),
    }
}

/// Decode a string in `format` back to [`SidecarData`]. An empty input is
/// an empty store. The reserved `_meta` record/key/`@type` is ignored.
pub fn decode(text: &str, format: JsonFormat) -> Result<SidecarData> {
    match format {
        JsonFormat::Plain => {
            if text.trim().is_empty() {
                Ok(SidecarData::default())
            } else {
                serde_json::from_str(text).context("parsing plain-JSON sidecar")
            }
        }
        JsonFormat::Ndjson => decode_ndjson(text),
        JsonFormat::Ld => decode_ld(text),
    }
}

/// Apply `f` to every (table, serialised-row) pair in deterministic order.
/// Centralises the per-collection walk shared by the ndjson/ld encoders.
fn for_each_row(data: &SidecarData, mut f: impl FnMut(&str, Value) -> Result<()>) -> Result<()> {
    for r in &data.provenance_log {
        f("verisimdb_provenance_log", serde_json::to_value(r)?)?;
    }
    for r in &data.provenance_chain_heads {
        f("verisimdb_provenance_chain_heads", serde_json::to_value(r)?)?;
    }
    for r in &data.temporal_versions {
        f("verisimdb_temporal_versions", serde_json::to_value(r)?)?;
    }
    for r in &data.lineage_graph {
        f("verisimdb_lineage_graph", serde_json::to_value(r)?)?;
    }
    for r in &data.access_policies {
        f("verisimdb_access_policies", serde_json::to_value(r)?)?;
    }
    Ok(())
}

/// Extract the object map from a serialised row, or fail loudly (every row
/// type serialises to a JSON object).
fn into_object(value: Value) -> Result<Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        _ => anyhow::bail!("internal: sidecar row did not serialise to a JSON object"),
    }
}

fn encode_ndjson(data: &SidecarData) -> Result<String> {
    let mut out = String::new();
    for_each_row(data, |table, value| {
        let mut map = into_object(value)?;
        map.insert("_table".to_string(), Value::String(table.to_string()));
        out.push_str(&serde_json::to_string(&Value::Object(map))?);
        out.push('\n');
        Ok(())
    })?;
    Ok(out)
}

fn decode_ndjson(text: &str) -> Result<SidecarData> {
    let mut data = SidecarData::default();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut value: Value =
            serde_json::from_str(line).with_context(|| format!("ndjson line {}", i + 1))?;
        let obj = value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("ndjson line {} is not a JSON object", i + 1))?;
        let table = obj
            .remove("_table")
            .and_then(|t| t.as_str().map(String::from))
            .ok_or_else(|| anyhow::anyhow!("ndjson line {} missing \"_table\"", i + 1))?;
        if table == META_TABLE {
            continue;
        }
        push_row(&mut data, &table, value).with_context(|| format!("ndjson line {}", i + 1))?;
    }
    Ok(data)
}

fn encode_ld(data: &SidecarData) -> Result<String> {
    let mut graph: Vec<Value> = Vec::new();
    for_each_row(data, |table, value| {
        let mut map = into_object(value)?;
        map.insert(
            "@type".to_string(),
            Value::String(ld_type_for(table).to_string()),
        );
        map.insert("@id".to_string(), Value::String(ld_id(table, &map)));
        graph.push(Value::Object(map));
        Ok(())
    })?;

    let doc = serde_json::json!({
        "@context": { "@vocab": LD_VOCAB, "verisimdb": LD_VOCAB },
        "@graph": graph,
    });
    Ok(serde_json::to_string_pretty(&doc)?)
}

fn decode_ld(text: &str) -> Result<SidecarData> {
    if text.trim().is_empty() {
        return Ok(SidecarData::default());
    }
    let doc: Value = serde_json::from_str(text).context("parsing JSON-LD sidecar")?;
    let graph = doc
        .get("@graph")
        .and_then(|g| g.as_array())
        .ok_or_else(|| anyhow::anyhow!("JSON-LD sidecar has no \"@graph\" array"))?;

    let mut data = SidecarData::default();
    for node in graph {
        let mut map = node
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("JSON-LD @graph node is not an object"))?;
        let ld_type = map
            .get("@type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("JSON-LD @graph node missing \"@type\""))?
            .to_string();
        let Some(table) = table_for_ld_type(&ld_type)? else {
            continue; // Meta node
        };
        map.remove("@type");
        map.remove("@id");
        push_row(&mut data, table, Value::Object(map))?;
    }
    Ok(data)
}

/// Compute a stable `@id` IRI for a row, from its already-serialised map.
fn ld_id(table: &str, map: &Map<String, Value>) -> String {
    let get = |k: &str| map.get(k).and_then(|v| v.as_str()).unwrap_or("");
    match table {
        "verisimdb_provenance_log" => format!("urn:verisimdb:provenance:{}", get("hash")),
        "verisimdb_provenance_chain_heads" => format!(
            "urn:verisimdb:chain-head:{}:{}",
            get("entity_id"),
            get("head_hash")
        ),
        "verisimdb_temporal_versions" => format!(
            "urn:verisimdb:temporal:{}:{}:{}",
            get("entity_id"),
            get("table_name"),
            map.get("version")
                .map(|v| v.to_string())
                .unwrap_or_default()
        ),
        "verisimdb_lineage_graph" => format!("urn:verisimdb:lineage:{}", get("edge_id")),
        "verisimdb_access_policies" => format!("urn:verisimdb:access:{}", get("policy_id")),
        _ => format!("urn:verisimdb:row:{table}"),
    }
}

/// Deserialise `value` into the row type named by `table` and append it.
fn push_row(data: &mut SidecarData, table: &str, value: Value) -> Result<()> {
    match table {
        "verisimdb_provenance_log" => data.provenance_log.push(serde_json::from_value(value)?),
        "verisimdb_provenance_chain_heads" => data
            .provenance_chain_heads
            .push(serde_json::from_value(value)?),
        "verisimdb_temporal_versions" => {
            data.temporal_versions.push(serde_json::from_value(value)?)
        }
        "verisimdb_lineage_graph" => data.lineage_graph.push(serde_json::from_value(value)?),
        "verisimdb_access_policies" => data.access_policies.push(serde_json::from_value(value)?),
        other => anyhow::bail!("unknown sidecar table {other:?}"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Store: load / save + octad operations
// ---------------------------------------------------------------------------

/// A JSON-family sidecar store bound to a file path and on-disk format.
pub struct JsonStore {
    path: PathBuf,
    format: JsonFormat,
    data: SidecarData,
}

impl JsonStore {
    /// Open the store at `path`, or start an empty one if it doesn't exist.
    pub fn open(path: impl AsRef<Path>, format: JsonFormat) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading sidecar {}", path.display()))?;
            decode(&text, format).with_context(|| {
                format!("decoding {} sidecar {}", format.as_str(), path.display())
            })?
        } else {
            SidecarData::default()
        };
        Ok(Self { path, format, data })
    }

    /// Borrow the underlying data (read-only).
    pub fn data(&self) -> &SidecarData {
        &self.data
    }

    /// Persist the store atomically: write a sibling temp file, then rename
    /// over the target so a crash mid-write can't truncate the sidecar.
    /// `rename(2)` within a directory is atomic, so a concurrent reader sees
    /// either the old or new complete file, never a partial one.
    ///
    /// For *write* flows, call this via [`with_locked`] so the load→mutate→
    /// save cycle is serialised against other writers; calling it bare is
    /// fine for a freshly-built store no other process can see yet.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating sidecar dir {}", parent.display()))?;
            }
        }
        let text = encode(&self.data, self.format)?;
        let tmp = self
            .path
            .with_extension(format!("{}.tmp", self.format.extension()));
        std::fs::write(&tmp, text.as_bytes())
            .with_context(|| format!("writing sidecar temp {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }

    // --- Provenance (mirrors tier1::provenance) ---------------------------

    /// Current branch-tip hashes for `entity_id`.
    fn head_set(&self, entity_id: &str) -> Vec<String> {
        self.data
            .provenance_chain_heads
            .iter()
            .filter(|h| h.entity_id == entity_id)
            .map(|h| h.head_hash.clone())
            .collect()
    }

    fn add_head(&mut self, entity_id: &str, hash: &str) {
        if !self
            .data
            .provenance_chain_heads
            .iter()
            .any(|h| h.entity_id == entity_id && h.head_hash == hash)
        {
            self.data.provenance_chain_heads.push(ChainHead {
                entity_id: entity_id.to_string(),
                head_hash: hash.to_string(),
            });
        }
    }

    fn remove_head(&mut self, entity_id: &str, hash: &str) {
        self.data
            .provenance_chain_heads
            .retain(|h| !(h.entity_id == entity_id && h.head_hash == hash));
    }

    /// Append a provenance entry on the entity's single current tip
    /// (genesis if none). A forked entity (≥2 heads) is ambiguous — use
    /// [`JsonStore::append_provenance_fork`]. Returns the new entry hash.
    #[allow(clippy::too_many_arguments)]
    pub fn append_provenance(
        &mut self,
        entity_id: &str,
        table_name: &str,
        operation: &str,
        actor: &str,
        before_snapshot: Option<&str>,
        transformation: Option<&str>,
    ) -> Result<String> {
        let heads = self.head_set(entity_id);
        let prev_hash = match heads.len() {
            0 => String::new(),
            1 => heads[0].clone(),
            n => anyhow::bail!(
                "entity {entity_id:?} has {n} chain heads (forked); linear append \
                 is ambiguous — use append_provenance_fork(from_hash) (ADR-0010)"
            ),
        };
        let hash = self.insert_provenance(
            &prev_hash,
            entity_id,
            table_name,
            operation,
            actor,
            before_snapshot,
            transformation,
        )?;
        if !prev_hash.is_empty() {
            self.remove_head(entity_id, &prev_hash);
        }
        self.add_head(entity_id, &hash);
        Ok(hash)
    }

    /// Deliberately fork: extend `entity_id` from a *specific ancestor*
    /// `from_hash` rather than the current tip (ADR-0010 §2). Adds a head
    /// without removing one, so the entity gains a branch.
    #[allow(clippy::too_many_arguments)]
    pub fn append_provenance_fork(
        &mut self,
        entity_id: &str,
        table_name: &str,
        operation: &str,
        actor: &str,
        before_snapshot: Option<&str>,
        transformation: Option<&str>,
        from_hash: &str,
    ) -> Result<String> {
        let ancestor_exists = self
            .data
            .provenance_log
            .iter()
            .any(|r| r.entity_id == entity_id && r.hash == from_hash);
        if !ancestor_exists {
            anyhow::bail!(
                "from_hash {from_hash:?} is not an entry in entity {entity_id:?}'s chain; \
                 cannot fork from a non-existent ancestor"
            );
        }
        let hash = self.insert_provenance(
            from_hash,
            entity_id,
            table_name,
            operation,
            actor,
            before_snapshot,
            transformation,
        )?;
        self.add_head(entity_id, &hash);
        Ok(hash)
    }

    /// Compute the hash, reject an exact-duplicate (the `hash` primary-key
    /// guard in the SQLite path), and push the log row.
    #[allow(clippy::too_many_arguments)]
    fn insert_provenance(
        &mut self,
        prev_hash: &str,
        entity_id: &str,
        table_name: &str,
        operation: &str,
        actor: &str,
        before_snapshot: Option<&str>,
        transformation: Option<&str>,
    ) -> Result<String> {
        let timestamp = Utc::now();
        let hash = ProvenanceEntry::compute_hash(
            prev_hash,
            entity_id,
            operation,
            actor,
            &timestamp,
            before_snapshot,
            transformation,
        );
        if self.data.provenance_log.iter().any(|r| r.hash == hash) {
            anyhow::bail!(
                "duplicate provenance entry: an entry with hash {hash} already exists \
                 (identical preimage)"
            );
        }
        self.data.provenance_log.push(ProvenanceRow {
            hash: hash.clone(),
            previous_hash: prev_hash.to_string(),
            entity_id: entity_id.to_string(),
            table_name: table_name.to_string(),
            operation: operation.to_string(),
            actor: actor.to_string(),
            timestamp: timestamp.to_rfc3339(),
            before_snapshot: before_snapshot.map(str::to_string),
            transformation: transformation.map(str::to_string),
        });
        Ok(hash)
    }

    /// Verify every branch of `entity_id`'s chain is hash-consistent
    /// (ADR-0010 §3). A forked entity is not a tampered one: each branch
    /// tip is walked back to a genesis and every node must recompute to its
    /// stored hash and chain to a present predecessor.
    pub fn verify_chain(&self, entity_id: &str) -> bool {
        let nodes: HashMap<&str, &ProvenanceRow> = self
            .data
            .provenance_log
            .iter()
            .filter(|r| r.entity_id == entity_id)
            .map(|r| (r.hash.as_str(), r))
            .collect();
        if nodes.is_empty() {
            return true; // vacuous
        }

        let mut has_child: HashSet<&str> = HashSet::new();
        for r in nodes.values() {
            if !r.previous_hash.is_empty() {
                has_child.insert(r.previous_hash.as_str());
            }
        }

        let mut tips: HashSet<String> = self.head_set(entity_id).into_iter().collect();
        for hash in nodes.keys() {
            if !has_child.contains(hash) {
                tips.insert((*hash).to_string());
            }
        }

        for tip in tips {
            let mut cursor = tip;
            loop {
                let Some(node) = nodes.get(cursor.as_str()) else {
                    return false; // dangling tip or broken link
                };
                let Ok(ts) = DateTime::parse_from_rfc3339(&node.timestamp) else {
                    return false;
                };
                let recomputed = ProvenanceEntry::compute_hash(
                    &node.previous_hash,
                    entity_id,
                    &node.operation,
                    &node.actor,
                    &ts.with_timezone(&Utc),
                    node.before_snapshot.as_deref(),
                    node.transformation.as_deref(),
                );
                if recomputed != cursor {
                    return false;
                }
                if node.previous_hash.is_empty() {
                    break;
                }
                cursor = node.previous_hash.clone();
            }
        }
        true
    }

    /// Every fork point in `entity_id`'s history (predecessors with >1
    /// child). Empty ⇒ the chain is linear.
    pub fn fork_points(&self, entity_id: &str) -> Vec<ForkPoint> {
        let mut counts: HashMap<&str, u64> = HashMap::new();
        for r in self
            .data
            .provenance_log
            .iter()
            .filter(|r| r.entity_id == entity_id)
        {
            *counts.entry(r.previous_hash.as_str()).or_insert(0) += 1;
        }
        let mut points: Vec<ForkPoint> = counts
            .into_iter()
            .filter(|&(_, c)| c > 1)
            .map(|(predecessor, children)| ForkPoint {
                predecessor: predecessor.to_string(),
                children,
            })
            .collect();
        points.sort_by(|a, b| a.predecessor.cmp(&b.predecessor));
        points
    }

    // --- Temporal (mirrors tier1::temporal) -------------------------------

    /// Append a new version of `(entity_id, table_name)`. Closes out the
    /// previous current row (sets its `valid_to`) before inserting the new
    /// one, preserving "exactly one current version" by construction.
    /// Returns the assigned (monotonic) version number.
    pub fn append_temporal_version(
        &mut self,
        entity_id: &str,
        table_name: &str,
        snapshot: &str,
        operation: &str,
    ) -> u64 {
        let prev_version = self
            .data
            .temporal_versions
            .iter()
            .filter(|r| r.entity_id == entity_id && r.table_name == table_name)
            .map(|r| r.version)
            .max()
            .unwrap_or(0);
        let next_version = prev_version + 1;
        let now = Utc::now().to_rfc3339();

        for row in self.data.temporal_versions.iter_mut().filter(|r| {
            r.entity_id == entity_id && r.table_name == table_name && r.valid_to.is_none()
        }) {
            row.valid_to = Some(now.clone());
        }

        self.data.temporal_versions.push(TemporalRow {
            entity_id: entity_id.to_string(),
            table_name: table_name.to_string(),
            version: next_version,
            valid_from: now,
            valid_to: None,
            snapshot: snapshot.to_string(),
            operation: operation.to_string(),
        });
        next_version
    }

    /// Current snapshot of `(entity_id, table_name)`, if any.
    pub fn read_current(&self, entity_id: &str, table_name: &str) -> Option<String> {
        self.data
            .temporal_versions
            .iter()
            .find(|r| {
                r.entity_id == entity_id && r.table_name == table_name && r.valid_to.is_none()
            })
            .map(|r| r.snapshot.clone())
    }

    /// Snapshot of `(entity_id, table_name)` as it existed at time `t`:
    /// `valid_from <= t` and (`valid_to` is NULL or `> t`), highest version
    /// wins. `None` if the entity didn't exist then.
    pub fn read_at(&self, entity_id: &str, table_name: &str, t: &DateTime<Utc>) -> Option<String> {
        self.data
            .temporal_versions
            .iter()
            .filter(|r| r.entity_id == entity_id && r.table_name == table_name)
            .filter(|r| {
                let from_ok = parse_ts(&r.valid_from).map(|f| f <= *t).unwrap_or(false);
                let to_ok = match &r.valid_to {
                    None => true,
                    Some(s) => parse_ts(s).map(|to| to > *t).unwrap_or(false),
                };
                from_ok && to_ok
            })
            .max_by_key(|r| r.version)
            .map(|r| r.snapshot.clone())
    }

    /// Roll `(entity_id, table_name)` back to `target_version` by appending
    /// that snapshot as a new `rollback` version (audit-preserving). Errors
    /// if the target version doesn't exist.
    pub fn rollback_to(
        &mut self,
        entity_id: &str,
        table_name: &str,
        target_version: u64,
    ) -> Result<u64> {
        let snapshot = self
            .data
            .temporal_versions
            .iter()
            .find(|r| {
                r.entity_id == entity_id
                    && r.table_name == table_name
                    && r.version == target_version
            })
            .map(|r| r.snapshot.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("no version {target_version} for ({entity_id:?}, {table_name:?})")
            })?;
        Ok(self.append_temporal_version(entity_id, table_name, &snapshot, "rollback"))
    }

    // --- Drift (reuses the storage-agnostic kernel) -----------------------

    /// Entities that have at least one temporal version, de-duplicated and
    /// sorted (drive for `verisimiser drift`).
    pub fn distinct_temporal_entities(&self) -> Vec<String> {
        let mut seen: Vec<String> = self
            .data
            .temporal_versions
            .iter()
            .map(|r| r.entity_id.clone())
            .collect();
        seen.sort();
        seen.dedup();
        seen
    }

    /// Temporal drift for one entity (ADR-0003 §3.1): max pairwise drift of
    /// the latest version per `table_name`. `None` if the entity is
    /// recorded under fewer than two modalities.
    pub fn detect_temporal_drift(&self, entity_id: &str) -> Option<DriftReport> {
        let mut latest: HashMap<&str, i64> = HashMap::new();
        for r in self
            .data
            .temporal_versions
            .iter()
            .filter(|r| r.entity_id == entity_id)
        {
            let e = latest.entry(r.table_name.as_str()).or_insert(0);
            *e = (*e).max(r.version as i64);
        }
        if latest.len() < 2 {
            return None;
        }
        let versions: Vec<i64> = latest.into_values().collect();
        let score = temporal_drift_score(&versions);
        Some(DriftReport {
            entity_id: entity_id.to_string(),
            overall_score: score,
            categories: vec![(DriftCategory::Temporal, score)],
            measured_at: Utc::now(),
        })
    }

    // --- GC (mirrors gc::run_gc semantics) --------------------------------

    /// Purge rows older than the retention bounds. A field of `0` days
    /// means "keep forever". Only *superseded* temporal versions
    /// (`valid_to` set) are eligible — the current version is always kept.
    /// `dry_run` counts without mutating; otherwise rows are removed in
    /// place (the caller persists via [`JsonStore::save`]).
    pub fn gc_purge(&mut self, retention: &RetentionConfig, dry_run: bool) -> GcCounts {
        let now = Utc::now();
        let mut counts = GcCounts::default();

        if retention.provenance_days > 0 {
            let cutoff = now - Duration::days(retention.provenance_days as i64);
            counts.provenance = purge_vec(&mut self.data.provenance_log, dry_run, |r| {
                older_than(&r.timestamp, &cutoff)
            });
        }
        if retention.temporal_days > 0 {
            let cutoff = now - Duration::days(retention.temporal_days as i64);
            counts.temporal = purge_vec(&mut self.data.temporal_versions, dry_run, |r| {
                r.valid_to.is_some() && older_than(&r.valid_from, &cutoff)
            });
        }
        if retention.lineage_days > 0 {
            let cutoff = now - Duration::days(retention.lineage_days as i64);
            counts.lineage = purge_vec(&mut self.data.lineage_graph, dry_run, |r| {
                older_than(&r.created_at, &cutoff)
            });
        }
        counts
    }
}

/// Build the `generate` scaffold for the enabled octad dimensions in the
/// given format. Emits an empty store annotated with a `_meta` record so a
/// freshly-generated file is self-describing; the runtime ignores `_meta`.
pub fn scaffold(octad: &OctadConfig, format: JsonFormat) -> Result<String> {
    let mut dims: Vec<&str> = vec!["data", "metadata"];
    if octad.enable_provenance {
        dims.push("provenance");
    }
    if octad.enable_lineage {
        dims.push("lineage");
    }
    if octad.enable_temporal {
        dims.push("temporal");
    }
    if octad.enable_access_control {
        dims.push("access_control");
    }
    if octad.enable_constraints {
        dims.push("constraints");
    }
    if octad.enable_simulation {
        dims.push("simulation");
    }

    let meta = serde_json::json!({
        "generator": "verisimiser generate (V-L2-F3)",
        "storage": "json",
        "format": format.as_str(),
        "dimensions": dims,
        "note": "append-only sidecar; mirrors the verisimdb_* overlay tables",
    });

    let enabled_tables = |octad: &OctadConfig| -> Vec<&'static str> {
        let mut t = Vec::new();
        if octad.enable_provenance {
            t.push("verisimdb_provenance_log");
            t.push("verisimdb_provenance_chain_heads");
        }
        if octad.enable_temporal {
            t.push("verisimdb_temporal_versions");
        }
        if octad.enable_lineage {
            t.push("verisimdb_lineage_graph");
        }
        if octad.enable_access_control {
            t.push("verisimdb_access_policies");
        }
        t
    };

    match format {
        JsonFormat::Plain => {
            let mut obj = Map::new();
            obj.insert(META_TABLE.to_string(), meta);
            for t in enabled_tables(octad) {
                obj.insert(t.to_string(), Value::Array(Vec::new()));
            }
            Ok(serde_json::to_string_pretty(&Value::Object(obj))?)
        }
        JsonFormat::Ndjson => {
            let mut meta_obj = meta.as_object().cloned().unwrap_or_default();
            meta_obj.insert("_table".to_string(), Value::String(META_TABLE.to_string()));
            Ok(format!(
                "{}\n",
                serde_json::to_string(&Value::Object(meta_obj))?
            ))
        }
        JsonFormat::Ld => {
            let mut meta_node = meta.as_object().cloned().unwrap_or_default();
            meta_node.insert("@type".to_string(), Value::String("Meta".to_string()));
            meta_node.insert(
                "@id".to_string(),
                Value::String("urn:verisimdb:meta".to_string()),
            );
            let doc = serde_json::json!({
                "@context": { "@vocab": LD_VOCAB, "verisimdb": LD_VOCAB },
                "@graph": [Value::Object(meta_node)],
            });
            Ok(serde_json::to_string_pretty(&doc)?)
        }
    }
}

/// Run a mutating transaction against the json sidecar at `path` while
/// holding the cross-process write lock for the whole load→mutate→save
/// cycle, then persist atomically.
///
/// This is the safe entry point for any operation that *writes* the store
/// (gc, provenance/temporal appends): the lock serialises concurrent
/// writers (the json analogue of SQLite's write lock) and the fresh load
/// inside the lock guarantees the closure sees the latest state. Read-only
/// callers can use [`JsonStore::open`] directly — atomic rename means a
/// reader always sees a complete file.
pub fn with_locked<T>(
    path: impl AsRef<Path>,
    format: JsonFormat,
    f: impl FnOnce(&mut JsonStore) -> Result<T>,
) -> Result<T> {
    let path = path.as_ref();
    // The lock file is a sibling of the sidecar, so its parent must exist
    // before we can create it.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating sidecar dir {}", parent.display()))?;
        }
    }
    let _lock = super::lock::FileLock::acquire(path)?;
    let mut store = JsonStore::open(path, format)?;
    let out = f(&mut store)?;
    store.save()?;
    Ok(out)
    // `_lock` is dropped here, releasing the write lock.
}

/// Parse an RFC 3339 timestamp to UTC, discarding the offset.
fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// `true` if `ts` parses and is strictly before `cutoff`. Unparseable
/// timestamps are treated as "not older" (never purged) — fail safe.
fn older_than(ts: &str, cutoff: &DateTime<Utc>) -> bool {
    parse_ts(ts).map(|t| t < *cutoff).unwrap_or(false)
}

/// Count (dry-run) or remove rows matching `purge`. Returns the match count.
fn purge_vec<T>(rows: &mut Vec<T>, dry_run: bool, purge: impl Fn(&T) -> bool) -> usize {
    if dry_run {
        rows.iter().filter(|r| purge(r)).count()
    } else {
        let before = rows.len();
        rows.retain(|r| !purge(r));
        before - rows.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const FORMATS: [JsonFormat; 3] = [JsonFormat::Plain, JsonFormat::Ld, JsonFormat::Ndjson];

    fn store(format: JsonFormat) -> (tempfile::TempDir, JsonStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("sidecar.{}", format.extension()));
        let s = JsonStore::open(&path, format).unwrap();
        (dir, s)
    }

    // --- Provenance parity ------------------------------------------------

    #[test]
    fn provenance_genesis_and_sequential_chain_verifies() {
        for fmt in FORMATS {
            let (_d, mut s) = store(fmt);
            let h1 = s
                .append_provenance("e1", "users", "insert", "alice", None, None)
                .unwrap();
            let h2 = s
                .append_provenance("e1", "users", "update", "alice", Some("{\"n\":1}"), None)
                .unwrap();
            let h3 = s
                .append_provenance("e1", "users", "delete", "bob", None, None)
                .unwrap();
            assert_ne!(h1, h2);
            assert_ne!(h2, h3);
            // Genesis chains from empty.
            assert_eq!(s.data().provenance_log[0].previous_hash, "");
            // A linear chain advances its single head.
            assert_eq!(s.head_set("e1"), vec![h3]);
            assert!(s.verify_chain("e1"), "fresh chain must verify ({fmt:?})");
        }
    }

    #[test]
    fn provenance_tamper_is_detected() {
        let (_d, mut s) = store(JsonFormat::Plain);
        s.append_provenance("e1", "users", "insert", "alice", None, None)
            .unwrap();
        s.append_provenance("e1", "users", "update", "alice", None, None)
            .unwrap();
        // Tamper with a stored field after the fact.
        s.data.provenance_log[1].operation = "transform".to_string();
        assert!(
            !s.verify_chain("e1"),
            "tampered entry must fail verification"
        );
    }

    #[test]
    fn provenance_fork_keeps_both_branches_and_verifies() {
        let (_d, mut s) = store(JsonFormat::Ndjson);
        let genesis = s
            .append_provenance("e1", "users", "insert", "alice", None, None)
            .unwrap();
        let _linear = s
            .append_provenance("e1", "users", "update", "alice", None, None)
            .unwrap();
        // Fork from genesis: a second divergent branch.
        let fork = s
            .append_provenance_fork("e1", "users", "update", "carol", None, None, &genesis)
            .unwrap();
        // Two heads now.
        let heads = s.head_set("e1");
        assert_eq!(heads.len(), 2);
        assert!(heads.contains(&fork));
        // Fork point detected at genesis (two children).
        let points = s.fork_points("e1");
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].predecessor, genesis);
        assert_eq!(points[0].children, 2);
        assert!(s.verify_chain("e1"), "both branches must verify");
    }

    #[test]
    fn provenance_linear_append_on_forked_entity_is_ambiguous() {
        let (_d, mut s) = store(JsonFormat::Plain);
        let genesis = s
            .append_provenance("e1", "users", "insert", "alice", None, None)
            .unwrap();
        s.append_provenance_fork("e1", "users", "update", "bob", None, None, &genesis)
            .unwrap();
        // Two heads now; a plain linear append can't pick a branch.
        let err = s
            .append_provenance("e1", "users", "update", "carol", None, None)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("forked") && err.contains("append_provenance_fork"),
            "linear append on a forked entity must point at the fork API; got: {err}"
        );
    }

    #[test]
    fn provenance_round_trips_through_disk_in_every_format() {
        for fmt in FORMATS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("s.{}", fmt.extension()));
            let head = {
                let mut s = JsonStore::open(&path, fmt).unwrap();
                s.append_provenance("e1", "users", "insert", "alice", None, None)
                    .unwrap();
                let h = s
                    .append_provenance("e1", "users", "update", "bob", Some("{}"), Some("x"))
                    .unwrap();
                s.save().unwrap();
                h
            };
            // Reopen and confirm state survived the codec.
            let s2 = JsonStore::open(&path, fmt).unwrap();
            assert_eq!(s2.head_set("e1"), vec![head], "head survives {fmt:?}");
            assert_eq!(s2.data().provenance_log.len(), 2, "rows survive {fmt:?}");
            assert!(
                s2.verify_chain("e1"),
                "chain re-verifies after reload {fmt:?}"
            );
        }
    }

    // --- Temporal parity --------------------------------------------------

    #[test]
    fn temporal_versions_are_monotonic_with_one_current() {
        for fmt in FORMATS {
            let (_d, mut s) = store(fmt);
            for i in 1..=50u64 {
                let v =
                    s.append_temporal_version("e1", "users", &format!("{{\"v\":{i}}}"), "update");
                assert_eq!(v, i, "monotonic version ({fmt:?})");
            }
            let current = s
                .data()
                .temporal_versions
                .iter()
                .filter(|r| r.entity_id == "e1" && r.valid_to.is_none())
                .count();
            assert_eq!(current, 1, "exactly one current version ({fmt:?})");
            assert_eq!(s.read_current("e1", "users").as_deref(), Some("{\"v\":50}"));
        }
    }

    #[test]
    fn temporal_read_at_returns_point_in_time_snapshot() {
        let (_d, mut s) = store(JsonFormat::Plain);
        s.append_temporal_version("e1", "users", "{\"v\":1}", "insert");
        std::thread::sleep(std::time::Duration::from_millis(15));
        let t1 = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(15));
        s.append_temporal_version("e1", "users", "{\"v\":2}", "update");
        std::thread::sleep(std::time::Duration::from_millis(15));
        let t2 = Utc::now();

        assert_eq!(s.read_at("e1", "users", &t1).as_deref(), Some("{\"v\":1}"));
        assert_eq!(s.read_at("e1", "users", &t2).as_deref(), Some("{\"v\":2}"));
    }

    #[test]
    fn temporal_rollback_appends_old_snapshot_as_new_version() {
        let (_d, mut s) = store(JsonFormat::Ld);
        s.append_temporal_version("e1", "users", "{\"v\":1}", "insert");
        s.append_temporal_version("e1", "users", "{\"v\":2}", "update");
        s.append_temporal_version("e1", "users", "{\"v\":3}", "update");
        let new_v = s.rollback_to("e1", "users", 1).unwrap();
        assert_eq!(new_v, 4, "rollback creates a new version");
        assert_eq!(s.read_current("e1", "users").as_deref(), Some("{\"v\":1}"));
        assert!(s.rollback_to("e1", "users", 99).is_err());
    }

    // --- Drift parity -----------------------------------------------------

    #[test]
    fn drift_matches_sqlite_worked_example() {
        let (_d, mut s) = store(JsonFormat::Plain);
        // Two modalities at versions 5 and 4 -> score 0.2 (ADR-0003).
        for _ in 0..5 {
            s.append_temporal_version("e1", "posts", "{}", "update");
        }
        for _ in 0..4 {
            s.append_temporal_version("e1", "posts_graph", "{}", "update");
        }
        let report = s.detect_temporal_drift("e1").unwrap();
        assert_eq!(report.entity_id, "e1");
        assert!((report.overall_score - 0.2).abs() < 1e-12);
        assert_eq!(report.categories[0].0, DriftCategory::Temporal);

        // Single-modality entity -> None.
        s.append_temporal_version("e2", "posts", "{}", "insert");
        assert!(s.detect_temporal_drift("e2").is_none());

        assert_eq!(s.distinct_temporal_entities(), vec!["e1", "e2"]);
    }

    // --- GC parity --------------------------------------------------------

    fn seed_aged(s: &mut JsonStore) {
        // provenance: 1 old, 1 fresh
        s.data.provenance_log.push(ProvenanceRow {
            hash: "old".into(),
            previous_hash: "".into(),
            entity_id: "e".into(),
            table_name: "t".into(),
            operation: "insert".into(),
            actor: "a".into(),
            timestamp: "2020-01-01T00:00:00+00:00".into(),
            before_snapshot: None,
            transformation: None,
        });
        s.data.provenance_log.push(ProvenanceRow {
            timestamp: "9999-01-01T00:00:00+00:00".into(),
            hash: "new".into(),
            ..s.data.provenance_log[0].clone()
        });
        // temporal: old superseded, old current, fresh superseded
        let mk = |v: u64, from: &str, to: Option<&str>| TemporalRow {
            entity_id: "e".into(),
            table_name: "t".into(),
            version: v,
            valid_from: from.into(),
            valid_to: to.map(str::to_string),
            snapshot: "{}".into(),
            operation: "update".into(),
        };
        s.data.temporal_versions.push(mk(
            1,
            "2020-01-01T00:00:00+00:00",
            Some("2020-06-01T00:00:00+00:00"),
        ));
        s.data
            .temporal_versions
            .push(mk(2, "2020-01-01T00:00:00+00:00", None));
        s.data.temporal_versions.push(mk(
            3,
            "9999-01-01T00:00:00+00:00",
            Some("9999-06-01T00:00:00+00:00"),
        ));
        // lineage: 1 old, 1 fresh
        s.data.lineage_graph.push(LineageRow {
            edge_id: "old".into(),
            source_entity: "a".into(),
            source_table: "t".into(),
            target_entity: "b".into(),
            target_table: "t".into(),
            derivation_type: "copy".into(),
            description: None,
            created_at: "2020-01-01T00:00:00+00:00".into(),
        });
        s.data.lineage_graph.push(LineageRow {
            edge_id: "new".into(),
            created_at: "9999-01-01T00:00:00+00:00".into(),
            ..s.data.lineage_graph[0].clone()
        });
    }

    #[test]
    fn gc_dry_run_counts_but_keeps_current_temporal() {
        let (_d, mut s) = store(JsonFormat::Plain);
        seed_aged(&mut s);
        let r = RetentionConfig {
            provenance_days: 30,
            temporal_days: 30,
            lineage_days: 30,
        };
        let counts = s.gc_purge(&r, true);
        assert_eq!(counts.provenance, 1);
        assert_eq!(counts.temporal, 1, "only old + superseded; current kept");
        assert_eq!(counts.lineage, 1);
        // dry-run mutates nothing.
        assert_eq!(s.data().provenance_log.len(), 2);
        assert_eq!(s.data().temporal_versions.len(), 3);
    }

    #[test]
    fn gc_apply_removes_old_rows_but_keeps_current_version() {
        let (_d, mut s) = store(JsonFormat::Plain);
        seed_aged(&mut s);
        let r = RetentionConfig {
            provenance_days: 30,
            temporal_days: 30,
            lineage_days: 30,
        };
        let counts = s.gc_purge(&r, false);
        assert_eq!(counts.provenance + counts.temporal + counts.lineage, 3);
        assert_eq!(s.data().provenance_log.len(), 1);
        // The old *current* version (v2) survives; only old superseded v1 is gone.
        assert_eq!(s.data().temporal_versions.len(), 2);
        assert!(s.read_current("e", "t").is_some());
        assert_eq!(s.data().lineage_graph.len(), 1);
    }

    #[test]
    fn gc_retention_zero_is_forever() {
        let (_d, mut s) = store(JsonFormat::Plain);
        seed_aged(&mut s);
        let counts = s.gc_purge(&RetentionConfig::default(), false);
        assert_eq!(counts.provenance + counts.temporal + counts.lineage, 0);
    }

    // --- Codec specifics --------------------------------------------------

    #[test]
    fn ld_output_is_genuine_linked_data() {
        let mut data = SidecarData::default();
        data.provenance_log.push(ProvenanceRow {
            hash: "abc".into(),
            previous_hash: "".into(),
            entity_id: "e1".into(),
            table_name: "users".into(),
            operation: "insert".into(),
            actor: "alice".into(),
            timestamp: "2026-01-01T00:00:00+00:00".into(),
            before_snapshot: None,
            transformation: None,
        });
        let text = encode(&data, JsonFormat::Ld).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert!(v.get("@context").is_some(), "JSON-LD needs @context");
        let graph = v.get("@graph").unwrap().as_array().unwrap();
        assert_eq!(graph[0]["@type"], "ProvenanceEntry");
        assert_eq!(graph[0]["@id"], "urn:verisimdb:provenance:abc");
        // Round-trips back to the same data.
        assert_eq!(decode(&text, JsonFormat::Ld).unwrap(), data);
    }

    #[test]
    fn ndjson_is_one_tagged_record_per_line() {
        let mut data = SidecarData::default();
        data.temporal_versions.push(TemporalRow {
            entity_id: "e1".into(),
            table_name: "users".into(),
            version: 1,
            valid_from: "2026-01-01T00:00:00+00:00".into(),
            valid_to: None,
            snapshot: "{}".into(),
            operation: "insert".into(),
        });
        let text = encode(&data, JsonFormat::Ndjson).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1);
        let v: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["_table"], "verisimdb_temporal_versions");
        assert_eq!(decode(&text, JsonFormat::Ndjson).unwrap(), data);
    }

    #[test]
    fn empty_inputs_decode_to_empty_store() {
        for fmt in FORMATS {
            assert_eq!(decode("", fmt).unwrap(), SidecarData::default());
        }
    }

    #[test]
    fn scaffold_round_trips_to_empty_store_ignoring_meta() {
        let octad = OctadConfig::default();
        for fmt in FORMATS {
            let text = scaffold(&octad, fmt).unwrap();
            assert!(text.contains("provenance") || fmt == JsonFormat::Ndjson);
            // The scaffold's _meta must not deserialise into real rows.
            let decoded = decode(&text, fmt).unwrap();
            assert_eq!(
                decoded,
                SidecarData::default(),
                "scaffold is an empty store ({fmt:?})"
            );
        }
    }
}
