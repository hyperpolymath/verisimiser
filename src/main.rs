#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::manual_strip,
    clippy::if_same_then_else,
    clippy::vec_init_then_push,
    clippy::upper_case_acronyms,
    clippy::format_in_format_args,
    clippy::enum_variant_names,
    clippy::module_inception,
    clippy::doc_lazy_continuation,
    clippy::manual_clamp,
    clippy::type_complexity
)]
#![forbid(unsafe_code)]
// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// VeriSimiser — augment any database with VeriSimDB octad capabilities.
// #3 priority in the -iser family (after TypedQLiser and Chapeliser).
//
// This is the CLI entry point. Subcommands:
//   init       — Generate a verisimiser.toml manifest
//   generate   — Parse schema and generate sidecar overlay + query interceptors
//   start      — Start the augmentation daemon (placeholder)
//   drift      — Check cross-modal drift status
//   provenance — Query provenance chain for an entity
//   history    — Query temporal version history for an entity
//   status     — Show augmentation status and health
//   octad      — Show the 8 octad dimensions

use anyhow::Result;
use clap::{Parser, Subcommand};

mod abi;
mod codegen;
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
        /// Database backend: postgresql, sqlite, or mongodb.
        #[arg(short, long, default_value = "postgresql")]
        database: String,
    },
    /// Parse the target database schema and generate sidecar overlay + interceptors.
    Generate {
        /// Path to the verisimiser.toml manifest.
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Output directory for generated SQL files.
        #[arg(short, long, default_value = ".verisim")]
        output: String,
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

        Commands::Generate { manifest, output } => {
            let m = manifest::load_manifest(&manifest)?;

            // Determine schema source: from manifest or auto-detect.
            let schema = if let Some(ref schema_path) = m.database.schema_source {
                println!("Parsing schema from: {}", schema_path);
                codegen::parser::parse_schema_file(schema_path)?
            } else {
                println!("No schema-source specified; generating empty overlay.");
                codegen::parser::ParsedSchema {
                    tables: Vec::new(),
                    source: None,
                }
            };

            // Determine the backend for SQL dialect selection.
            let backend_name = m.database.effective_backend();
            let backend = abi::DatabaseBackend::from_str(backend_name)
                .unwrap_or(abi::DatabaseBackend::PostgreSQL);

            // Create output directory.
            std::fs::create_dir_all(&output)?;

            // Generate sidecar overlay schema.
            let overlay_ddl = codegen::overlay::generate_sidecar_schema(&schema, &m.octad);
            let overlay_path = format!("{}/sidecar_schema.sql", output);
            std::fs::write(&overlay_path, &overlay_ddl)?;
            println!("Generated sidecar schema: {}", overlay_path);

            // Generate query interceptors.
            let interceptors = codegen::query::generate_interceptors(&schema, &m.octad, backend);
            let interceptor_sql = codegen::query::render_interceptors(&interceptors);
            let interceptor_path = format!("{}/interceptors.sql", output);
            std::fs::write(&interceptor_path, &interceptor_sql)?;
            println!("Generated query interceptors: {}", interceptor_path);

            println!(
                "\nGeneration complete. {} table(s) processed, {}/8 octad dimensions enabled.",
                schema.tables.len(),
                m.octad.enabled_count()
            );
            Ok(())
        }

        Commands::Start { manifest } => {
            let m = manifest::load_manifest(&manifest)?;
            let name = if !m.project.name.is_empty() {
                &m.project.name
            } else {
                &m.verisimiser.name
            };
            let backend = m.database.effective_backend();
            println!(
                "Starting VeriSimiser augmentation for {} ({})",
                name, backend
            );
            println!("  Octad: {}/8 dimensions enabled", m.octad.enabled_count());
            println!("  Sidecar: {} ({})", m.sidecar.path, m.sidecar.storage);
            // TODO: start interception daemon
            Ok(())
        }

        Commands::Drift {
            manifest,
            threshold,
        } => {
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

        Commands::History {
            manifest,
            entity,
            at,
        } => {
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

/// Print the 8 octad dimensions with descriptions.
fn print_octad() {
    println!("=== VeriSimDB Octad: Eight Dimensions ===");
    println!();
    for dim in abi::OctadDimension::all() {
        let inherent = if dim.is_inherent() {
            " (always on)"
        } else {
            ""
        };
        println!(
            "  {:15} {}{}",
            dim.label(),
            dimension_description(&dim),
            inherent
        );
    }
}

/// Returns a short description for each octad dimension.
fn dimension_description(dim: &abi::OctadDimension) -> &'static str {
    match dim {
        abi::OctadDimension::Data => "The original data in your database",
        abi::OctadDimension::Metadata => "Schema and type information",
        abi::OctadDimension::Provenance => "SHA-256 hash-chain origin tracking",
        abi::OctadDimension::Lineage => "Data derivation DAG",
        abi::OctadDimension::Constraints => "Cross-dimensional invariant enforcement",
        abi::OctadDimension::AccessControl => "Row/column-level access policies",
        abi::OctadDimension::Temporal => "Version history with point-in-time queries",
        abi::OctadDimension::Simulation => "What-if branching and sandbox queries",
    }
}
