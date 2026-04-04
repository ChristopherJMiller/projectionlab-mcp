//! Debug tool: export ProjectionLab data to debug-data/ for type validation and test fixtures.
//!
//! Usage: cargo run --bin export-data
//!
//! This launches Firefox, waits for you to log in, exports all data,
//! and writes several files to debug-data/:
//!   - full_export.json       — the complete export (raw JSON)
//!   - debts.json             — just the debts array from starting conditions
//!   - assets.json            — just the assets array from starting conditions
//!   - parse_result.txt       — whether our Rust types can parse the export
//!
//! All output is gitignored. Never commit debug-data/.

use anyhow::{Context, Result};
use projectionlab_mcp::browser::BrowserSession;
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

    println!("Starting browser session...");
    println!("Please log in to ProjectionLab when the browser opens.");
    println!();

    let mut browser = BrowserSession::new().await?;

    println!("Logged in! Exporting data...");

    let raw_value = browser
        .call_plugin_api("exportData", vec![])
        .await
        .context("Failed to call exportData")?;

    // Write raw JSON
    let raw_json = serde_json::to_string_pretty(&raw_value)?;
    let raw_path = output_dir.join("full_export.json");
    fs::write(&raw_path, &raw_json)?;
    println!("Wrote {} ({} bytes)", raw_path.display(), raw_json.len());

    // Extract and write debts
    if let Some(today) = raw_value.get("today") {
        if let Some(debts) = today.get("debts") {
            let debts_json = serde_json::to_string_pretty(debts)?;
            let debts_path = output_dir.join("debts.json");
            fs::write(&debts_path, &debts_json)?;
            println!(
                "Wrote {} ({} entries)",
                debts_path.display(),
                debts.as_array().map(|a| a.len()).unwrap_or(0)
            );
        }

        if let Some(assets) = today.get("assets") {
            let assets_json = serde_json::to_string_pretty(assets)?;
            let assets_path = output_dir.join("assets.json");
            fs::write(&assets_path, &assets_json)?;
            println!(
                "Wrote {} ({} entries)",
                assets_path.display(),
                assets.as_array().map(|a| a.len()).unwrap_or(0)
            );
        }
    }

    // Try parsing with our Rust types
    println!("\nAttempting to parse with Rust types...");
    let parse_result =
        match serde_json::from_value::<projectionlab_mcp::models::FullExport>(raw_value.clone()) {
            Ok(data) => {
                let mut report = String::new();
                report.push_str("PARSE: OK\n\n");
                report.push_str(&format!("Plans: {}\n", data.plans.len()));
                report.push_str(&format!(
                    "Savings accounts: {}\n",
                    data.today.savings_accounts.len()
                ));
                report.push_str(&format!(
                    "Investment accounts: {}\n",
                    data.today.investment_accounts.len()
                ));
                report.push_str(&format!("Debts: {}\n", data.today.debts.len()));
                report.push_str(&format!("Assets: {}\n", data.today.assets.len()));
                report.push_str(&format!(
                    "Progress data points: {}\n",
                    data.progress.data.len()
                ));

                // Report any extra fields captured by flatten on debts
                for (i, debt) in data.today.debts.iter().enumerate() {
                    if !debt.extra.is_empty() {
                        report.push_str(&format!(
                            "\nDebt[{}] '{}' has {} extra fields: {:?}\n",
                            i,
                            debt.name,
                            debt.extra.len(),
                            debt.extra.keys().collect::<Vec<_>>()
                        ));
                    }
                }

                // Report any extra fields captured by flatten on assets
                for (i, asset) in data.today.assets.iter().enumerate() {
                    if !asset.extra.is_empty() {
                        report.push_str(&format!(
                            "\nAsset[{}] '{}' has {} extra fields: {:?}\n",
                            i,
                            asset.name,
                            asset.extra.len(),
                            asset.extra.keys().collect::<Vec<_>>()
                        ));
                    }
                }

                report
            }
            Err(e) => {
                format!("PARSE: FAILED\n\nError: {}\n", e)
            }
        };

    let result_path = output_dir.join("parse_result.txt");
    fs::write(&result_path, &parse_result)?;
    println!("{}", parse_result);
    println!("Wrote {}", result_path.display());

    // Clean up browser — closes Firefox and GeckoDriver
    browser.shutdown().await;

    println!("\nDone! Check debug-data/ for output.");
    Ok(())
}
