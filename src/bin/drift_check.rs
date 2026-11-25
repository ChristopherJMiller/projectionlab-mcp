//! Detect ProjectionLab changes relative to the committed baseline.
//!
//! Usage:
//!   cargo run --bin drift-check                       # report; exit 1 if something required broke
//!   cargo run --bin drift-check -- --update-baseline  # accept the live app as the new baseline
//!
//! The report lists:
//!   - errors:       surfaces the server needs that are missing/incompatible (tools will fail)
//!   - warnings:     newer ProjectionLab versions, removed routes/fields
//!   - new_features: new app routes, Plugin API methods, or export documents worth exposing
//!   - export_shape_diff: export field paths added/removed/retyped since the baseline
//!
//! Writes the raw surface to debug-data/surface.json (gitignored) for inspection.
//! The baseline (compat/projectionlab.json) contains only structure — field
//! names, routes, method names, versions — never values, ids, or dates.

use anyhow::{Context, Result};
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::compat::{self, Baseline};
use std::fs;

const BASELINE_PATH: &str = "compat/projectionlab.json";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let update = std::env::args().any(|a| a == "--update-baseline");

    eprintln!("Starting browser session...");
    let mut browser = BrowserSession::new().await?;
    let gathered = async {
        let surface = browser
            .execute_js_async(&compat::app_surface_script())
            .await
            .context("capturing app surface")?;
        let export = browser
            .call_plugin_api("exportData", vec![])
            .await
            .context("exporting data")?;
        anyhow::Ok((surface, export))
    }
    .await;
    browser.shutdown().await;
    let (surface, export) = gathered?;

    fs::create_dir_all("debug-data")?;
    fs::write("debug-data/surface.json", serde_json::to_string_pretty(&surface)?)?;

    if update {
        let baseline = compat::baseline_from(&surface, &export)?;
        fs::write(BASELINE_PATH, serde_json::to_string_pretty(&baseline)? + "\n")?;
        eprintln!(
            "Wrote {} for ProjectionLab {} ({} routes, {} export paths). Review the diff before committing.",
            BASELINE_PATH,
            baseline.app_version,
            baseline.routes.len(),
            baseline.export_shape.len()
        );
        return Ok(());
    }

    let baseline: Baseline = serde_json::from_str(&fs::read_to_string(BASELINE_PATH)?)?;
    let report = compat::check(&baseline, &surface, Some(&export));
    println!("{}", serde_json::to_string_pretty(&report)?);
    eprintln!("\nstatus: {}", report.status);
    if report.status == "broken" {
        std::process::exit(1);
    }
    Ok(())
}
