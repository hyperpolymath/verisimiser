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
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;
use verisimiser::{abi, codegen, doctor, gc, manifest, tier1};

/// Diagnostic-log rendering. Data output (reports, version, the octad
/// table) is always written verbatim to stdout regardless of this; this
/// only controls the `tracing` diagnostic stream, which goes to stderr.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum LogFormat {
    /// Human-readable, ANSI-coloured single lines.
    Pretty,
    /// One JSON object per line (see docs/logging.adoc for the schema).
    Json,
}

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
    /// Diagnostic-log format. Diagnostics go to stderr; command data
    /// output always stays on stdout.
    #[arg(long, value_enum, default_value_t = LogFormat::Pretty, global = true)]
    log_format: LogFormat,
    /// Diagnostic-log level: trace|debug|info|warn|error. Overrides
    /// `RUST_LOG`. If neither is set, defaults to `info`.
    #[arg(long, global = true)]
    log_level: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

/// Install the global `tracing` subscriber. Writes to **stderr** so the
/// diagnostic stream never contaminates the stdout data contract
/// (JSON reports, version strings, the octad table). Precedence for the
/// level filter: `--log-level` > `RUST_LOG` > `info`.
fn init_tracing(format: LogFormat, level: Option<&str>) {
    let filter = match level {
        Some(l) => EnvFilter::try_new(l).unwrap_or_else(|_| EnvFilter::new("info")),
        None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
    };
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);
    match format {
        LogFormat::Json => builder.json().init(),
        LogFormat::Pretty => builder.init(),
    }
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
    /// Environment-level diagnostics (toolchain, PATH, cwd). Optionally
    /// also runs the manifest checks from `validate`.
    Doctor {
        /// If supplied, also run `validate` checks against this manifest.
        #[arg(short, long)]
        manifest: Option<String>,
        /// Emit the structured ValidationReport as JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Purge sidecar rows older than the bounds in `[retention]`.
    Gc {
        #[arg(short, long, default_value = "verisimiser.toml")]
        manifest: String,
        /// Report what would be deleted without actually deleting.
        #[arg(long)]
        dry_run: bool,
        /// Emit the structured GcReport as JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.log_format, cli.log_level.as_deref());
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
                tracing::info!(schema = %schema_path, "parsing schema");
                codegen::parser::parse_schema_file(schema_path)?
            } else {
                tracing::warn!("no schema-source specified; generating empty overlay");
                codegen::parser::ParsedSchema {
                    tables: Vec::new(),
                    source: None,
                }
            };

            // Determine the backend for SQL dialect selection.
            let backend_name = m.database.effective_backend()?;
            let backend = abi::DatabaseBackend::from_str(backend_name)
                .unwrap_or(abi::DatabaseBackend::PostgreSQL);

            // Create output directory.
            std::fs::create_dir_all(&output)?;

            // The sidecar DDL dialect follows [sidecar].storage. This
            // rejects `json` (tracked by #112) instead of silently
            // emitting SQLite DDL for a non-SQLite store (V-L2-F1).
            let dialect = codegen::overlay::SqlDialect::from_storage(&m.sidecar.storage)?;

            // Generate sidecar overlay schema. Errors here surface invalid
            // table/column identifiers in the parsed schema before they
            // reach disk.
            let overlay_ddl =
                codegen::overlay::generate_sidecar_schema(&schema, &m.octad, dialect)?;
            let overlay_path = format!("{}/sidecar_schema.sql", output);
            std::fs::write(&overlay_path, &overlay_ddl)?;
            tracing::info!(path = %overlay_path, "generated sidecar schema");

            // Generate query interceptors.
            let interceptors = codegen::query::generate_interceptors(&schema, &m.octad, backend);
            let interceptor_sql = codegen::query::render_interceptors(&interceptors);
            let interceptor_path = format!("{}/interceptors.sql", output);
            std::fs::write(&interceptor_path, &interceptor_sql)?;
            tracing::info!(path = %interceptor_path, "generated query interceptors");

            tracing::info!(
                tables = schema.tables.len(),
                octad_enabled = m.octad.enabled_count(),
                "generation complete"
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
            let m = manifest::load_manifest(&manifest)?;
            if m.sidecar.storage != "sqlite" {
                anyhow::bail!(
                    "verisimiser drift currently only supports the SQLite \
                     sidecar backend; [sidecar].storage is {:?}",
                    m.sidecar.storage
                );
            }
            let conn = rusqlite::Connection::open(&m.sidecar.path)?;
            // Distinct entity_ids that have at least one row in temporal_versions.
            let mut stmt =
                conn.prepare("SELECT DISTINCT entity_id FROM verisimdb_temporal_versions")?;
            let entities: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;

            tracing::info!(threshold, "checking temporal drift");
            let mut reported = 0usize;
            for entity in &entities {
                let Some(report) = tier1::drift::detect_temporal_drift(&conn, entity)? else {
                    continue;
                };
                if report.overall_score >= threshold {
                    println!("  {} drift={:.3}", report.entity_id, report.overall_score);
                    reported += 1;
                }
            }
            println!(
                "Scanned {} entit{}; {} above threshold.",
                entities.len(),
                if entities.len() == 1 { "y" } else { "ies" },
                reported
            );
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
                manifest::print_status(&m)?;
            }
            Ok(())
        }

        Commands::Octad => {
            print_octad();
            Ok(())
        }

        Commands::Validate { manifest, json } => {
            let report = manifest::validate_manifest(&manifest);
            emit_report(&report, json, "manifest validation")
        }

        Commands::Doctor { manifest, json } => {
            let report = doctor::run_doctor(manifest.as_deref());
            emit_report(&report, json, "doctor")
        }

        Commands::Gc {
            manifest,
            dry_run,
            json,
        } => {
            let m = manifest::load_manifest(&manifest)?;
            let report = gc::run_gc(&m, dry_run)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                let action = if report.dry_run {
                    "would delete"
                } else {
                    "deleted"
                };
                println!(
                    "verisimiser gc ({}):",
                    if report.dry_run { "dry-run" } else { "apply" }
                );
                println!("  sidecar:    {}", report.sidecar);
                println!("  provenance: {action} {} rows", report.provenance_deleted);
                println!("  temporal:   {action} {} rows", report.temporal_deleted);
                println!("  lineage:    {action} {} rows", report.lineage_deleted);
                println!("  total:      {} rows", report.total());
            }
            Ok(())
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

/// Render a `ValidationReport` (from `validate` or `doctor`) and exit
/// non-zero if any check failed. Plain-text by default; JSON when
/// `json == true`.
fn emit_report(report: &manifest::ValidationReport, json: bool, kind: &str) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!("Running {} for {} ...", kind, report.manifest);
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
        anyhow::bail!("{} failed", kind);
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
