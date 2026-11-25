//! Debug tool: call any ProjectionLab Plugin API method directly.
//!
//! Usage:
//!   cargo run --bin call-tool                     # exports all data (default)
//!   cargo run --bin call-tool exportData           # same as above
//!   cargo run --bin call-tool plans                # list plans (parsed)
//!   cargo run --bin call-tool accounts             # list accounts (parsed)
//!   cargo run --bin call-tool raw <method> [args]  # raw plugin API call (args: JSON array)
//!
//! Reuses the persistent Firefox profile so login is automatic after first use.

use anyhow::Result;
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::models::parse_export;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(|s| s.as_str()).unwrap_or("exportData");

    eprintln!("Starting browser session...");
    let mut browser = BrowserSession::new().await?;
    eprintln!("Connected!\n");

    let result = run(&browser, command, &args).await;
    browser.shutdown().await;
    result
}

async fn run(browser: &BrowserSession, command: &str, args: &[String]) -> Result<()> {
    match command {
        "exportData" | "export" => {
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            match parse_export(&raw) {
                Ok(data) => {
                    eprintln!("Parse: OK");
                    eprintln!("Plans: {}", data.plans().len());
                    eprintln!("Current Finances items: {}", data.current_finances().accounts.len());
                    eprintln!("Accounts with history: {}", data.documents.account_history.len());
                }
                Err(e) => eprintln!("Parse FAILED: {:#}", e),
            }
            println!("{}", serde_json::to_string_pretty(&raw)?);
        }
        "plans" => {
            let data = parse_export(&browser.call_plugin_api("exportData", vec![]).await?)?;
            for plan in data.plans() {
                println!(
                    "{} | {} | active={} baseline={} | expenses={} income={} priorities={} assets={} milestones={}",
                    plan.id,
                    plan.name,
                    data.active_plan_id() == Some(plan.id.as_str()),
                    data.baseline_plan_id() == Some(plan.id.as_str()),
                    plan.expenses.events.len(),
                    plan.income.events.len(),
                    plan.priorities.events.len(),
                    plan.assets.events.len(),
                    plan.milestones.len(),
                );
            }
        }
        "accounts" => {
            let data = parse_export(&browser.call_plugin_api("exportData", vec![]).await?)?;
            for a in data.current_finances().sorted_accounts() {
                println!(
                    "{} | {} | {}/{} | value={} {}",
                    a.id,
                    a.name,
                    a.kind,
                    a.account_type,
                    a.current_value(),
                    a.archived_on.as_deref().map(|d| format!("(archived {})", d)).unwrap_or_default()
                );
            }
        }
        "raw" => {
            let method = args.get(1).expect("Usage: call-tool raw <method> [json_args]");
            let api_args: Vec<serde_json::Value> = match args.get(2) {
                Some(json_str) => serde_json::from_str(json_str)?,
                None => vec![],
            };
            let result = browser.call_plugin_api(method, api_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        other => {
            eprintln!("Unknown command: {}", other);
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  call-tool                      # export all data");
            eprintln!("  call-tool plans                 # list plans");
            eprintln!("  call-tool accounts              # list accounts");
            eprintln!("  call-tool raw <method> [args]   # raw plugin API call");
            std::process::exit(1);
        }
    }
    Ok(())
}
