// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// VeriSimiser — augment any database with VeriSimDB octad capabilities.
// #3 priority in the -iser family (after TypedQLiser and Chapeliser).

use anyhow::Result;
use clap::{Parser, Subcommand};

mod abi;
mod intercept;
mod manifest;
mod tier1;
mod tier2;

/// VeriSimiser — augment any database with VeriSimDB octad capabilities.
#[derive(Parser)]
#[command(name = "verisimiser", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise a verisimiser.toml manifest.
    Init {
        #[arg(short, long, default_value = "postgresql")]
        database: String,
    },
    /// Start the VeriSimiser augmentation daemon.
    Start {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
    },
    /// Check drift status across all monitored entities.
    Drift {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Show only entities with drift above this threshold (0.0 - 1.0).
        #[arg(long, default_value = "0.1")]
        threshold: f64,
    },
    /// Query provenance chain for an entity.
    Provenance {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Entity ID to trace.
        entity: String,
    },
    /// Query temporal version history for an entity.
    History {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Entity ID.
        entity: String,
        /// Point-in-time (ISO 8601). If omitted, shows full history.
        #[arg(long)]
        at: Option<String>,
    },
    /// Show augmentation status and health.
    Status {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
    },
    /// Show the octad modalities and which tiers they belong to.
    Octad,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { database } => manifest::init_manifest(&database),
        Commands::Start { manifest } => {
            let m = manifest::load_manifest(&manifest)?;
            println!("Starting VeriSimiser augmentation for {} ({})", m.verisimiser.name, m.database.target_db);
            println!("  Tier 1: drift={}, provenance={}, temporal={}",
                m.tier1.drift_detection, m.tier1.provenance, m.tier1.temporal_versioning);
            // TODO: start interception daemon
            Ok(())
        }
        Commands::Drift { manifest, threshold } => {
            let _m = manifest::load_manifest(&manifest)?;
            println!("Checking cross-modal drift (threshold: {})...", threshold);
            // TODO: query drift index
            Ok(())
        }
        Commands::Provenance { manifest, entity } => {
            let _m = manifest::load_manifest(&manifest)?;
            println!("Provenance chain for entity: {}", entity);
            // TODO: query provenance sidecar
            Ok(())
        }
        Commands::History { manifest, entity, at } => {
            let _m = manifest::load_manifest(&manifest)?;
            match at {
                Some(t) => println!("Entity {} at {}", entity, t),
                None => println!("Full history for entity {}", entity),
            }
            // TODO: query temporal sidecar
            Ok(())
        }
        Commands::Status { manifest } => {
            let m = manifest::load_manifest(&manifest)?;
            manifest::print_status(&m);
            Ok(())
        }
        Commands::Octad => {
            print_octad();
            Ok(())
        }
    }
}

fn print_octad() {
    println!("=== VeriSimDB Octad: Eight Modalities ===");
    println!();
    println!("  TIER 1 — True piggybacks (no storage in your database):");
    println!("    Temporal     Version history and time-series (sidecar)");
    println!("    Provenance   SHA-256 hash-chain origin tracking (sidecar)");
    println!("    [Drift]      Cross-modal consistency monitoring (read-path observer)");
    println!();
    println!("  TIER 2 — Augmentation layer (additional storage alongside):");
    println!("    Graph        RDF triples and property graph edges");
    println!("    Vector       Embeddings for similarity search (HNSW)");
    println!("    Tensor       Multi-dimensional numeric data (ndarray/Burn)");
    println!("    Semantic     Type annotations and CBOR proof blobs");
    println!("    Document     Full-text searchable content (Tantivy)");
    println!("    Spatial      Geospatial coordinates (R-tree)");
}
