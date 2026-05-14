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
use verisimiser::{abi, codegen, manifest};

/// Long version string: `<crate-version> (<git-describe>, built <date>)`.
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("VERISIMISER_GIT_DESCRIBE"),
    ", built ",
    env!("VERISIMISER_BUILD_DATE"),
    ")",
);

/// VeriSimiser — augment any database with VeriSimDB octad capabilities.
#[derive(Parser)]
#[command(name = "verisimiser", version = LONG_VERSION, about, long_about = None)]
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
        /// Project name to set under [project].name. Defaults to the value
        /// from `ProjectConfig::default()` if not provided.
        #[arg(short, long)]
        name: Option<String>,
        /// Overwrite an existing verisimiser.toml instead of erroring.
        #[arg(short, long)]
        force: bool,
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
        /// Emit a structured JSON report instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Show the octad modalities and which tiers they belong to.
    Octad,
    /// Print version, git-sha, and build-date.
    Version {
        /// Emit JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Validate a manifest. Exit code is non-zero if any check fails.
    Validate {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Emit the structured ValidationReport as JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            database,
            name,
            force,
        } => manifest::init_manifest(&database, name.as_deref(), force),

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

            // Generate sidecar overlay schema. Errors here surface invalid
            // table/column identifiers in the parsed schema before they
            // reach disk.
            let overlay_ddl = codegen::overlay::generate_sidecar_schema(&schema, &m.octad)?;
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
            // Load the manifest so config errors still surface, but refuse
            // to claim we started the daemon. The interception daemon is
            // tracked by V-L1-C1 (hyperpolymath/verisimiser#46); until it
            // lands, an explicit refusal is less misleading than a silent
            // print-and-exit that implies the augmentation is running.
            let _m = manifest::load_manifest(&manifest)?;
            anyhow::bail!(
                "verisimiser start: the augmentation daemon is not yet \
                 implemented. Manifest at {} parsed successfully, but no \
                 interception is running. Tracked by V-L1-C1 (issue #46).",
                manifest
            );
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

        Commands::Status { manifest, json } => {
            let m = manifest::load_manifest(&manifest)?;
            if json {
                let report = manifest::status_report(&m);
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                manifest::print_status(&m);
            }
            Ok(())
        }

        Commands::Octad => {
            print_octad();
            Ok(())
        }

        Commands::Validate { manifest, json } => {
            let report = manifest::validate_manifest(&manifest);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Validating {} ...", report.manifest);
                for check in &report.checks {
                    let mark = if check.passed { "ok " } else { "FAIL" };
                    println!("  [{}] {} — {}", mark, check.name, check.description);
                    if let Some(detail) = &check.detail {
                        println!("        {}", detail);
                    }
                }
                if report.passed {
                    println!("All {} checks passed.", report.checks.len());
                } else {
                    println!(
                        "{}/{} checks failed.",
                        report.failed_count(),
                        report.checks.len()
                    );
                }
            }
            if report.passed {
                Ok(())
            } else {
                anyhow::bail!("manifest validation failed");
            }
        }

        Commands::Version { json } => {
            if json {
                let report = serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "git_sha": env!("VERISIMISER_GIT_SHA"),
                    "git_describe": env!("VERISIMISER_GIT_DESCRIBE"),
                    "build_date": env!("VERISIMISER_BUILD_DATE"),
                });
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", LONG_VERSION);
            }
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
