//! Debug tool: export ProjectionLab data to debug-data/ for type validation and test fixtures.
//!
//! Usage: cargo run --bin export-data
//!
//! This launches Firefox (headless if the saved session is still valid),
//! exports all data, and writes to debug-data/:
//!   - full_export.json       — the complete V5 export envelope (raw JSON)
//!   - current_finances.json  — just documents.currentFinances
//!   - parse_result.txt       — whether our Rust types can parse the export
//!
//! All output is gitignored. Never commit debug-data/.

use anyhow::{Context, Result};
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::models::parse_export;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let output_dir = Path::new("debug-data");
    fs::create_dir_all(output_dir)?;

    println!("Starting browser session (log in if a browser window opens)...");
    let mut browser = BrowserSession::new().await?;

    println!("Exporting data...");
    let raw = browser
        .call_plugin_api("exportData", vec![])
        .await
        .context("Failed to call exportData")?;
    browser.shutdown().await;

    let raw_json = serde_json::to_string_pretty(&raw)?;
    let raw_path = output_dir.join("full_export.json");
    fs::write(&raw_path, &raw_json)?;
    println!("Wrote {} ({} bytes)", raw_path.display(), raw_json.len());

    if let Some(cf) = raw.pointer("/documents/currentFinances") {
        let path = output_dir.join("current_finances.json");
        fs::write(&path, serde_json::to_string_pretty(cf)?)?;
        println!("Wrote {}", path.display());
    }

    println!("\nAttempting to parse with Rust types...");
    let report = match parse_export(&raw) {
        Ok(data) => {
            let mut r = String::from("PARSE: OK\n\n");
            let cf = data.current_finances();
            let mut by_kind: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
            for a in cf.accounts.values() {
                let e = by_kind.entry(a.kind.as_str()).or_default();
                if a.is_archived() {
                    e.1 += 1;
                } else {
                    e.0 += 1;
                }
            }
            writeln!(r, "Export schema: {}", data.schema)?;
            writeln!(r, "Plans: {}", data.plans().len())?;
            for (kind, (active, archived)) in by_kind {
                writeln!(r, "Current Finances {}: {} active, {} archived", kind, active, archived)?;
            }
            writeln!(r, "Net worth (active items): {:.2}", cf.net_worth())?;
            writeln!(r, "Accounts with history: {}", data.documents.account_history.len())?;
            writeln!(r, "Active plan: {:?}", data.active_plan_id())?;
            writeln!(r, "Baseline plan: {:?}", data.baseline_plan_id())?;
            if !data.documents.extra.is_empty() {
                writeln!(
                    r,
                    "Unrecognized documents: {:?}",
                    data.documents.extra.keys().collect::<Vec<_>>()
                )?;
            }
            r
        }
        Err(e) => format!("PARSE: FAILED\n\n{:#}\n", e),
    };

    let result_path = output_dir.join("parse_result.txt");
    fs::write(&result_path, &report)?;
    println!("{}", report);
    println!("Wrote {}", result_path.display());
    Ok(())
}
