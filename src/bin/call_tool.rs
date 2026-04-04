//! Debug tool: call any ProjectionLab Plugin API method directly.
//!
//! Usage:
//!   cargo run --bin call-tool                     # exports all data (default)
//!   cargo run --bin call-tool exportData           # same as above
//!   cargo run --bin call-tool plans                # list plans (parsed)
//!   cargo run --bin call-tool accounts             # list accounts (parsed)
//!   cargo run --bin call-tool raw <method> [args]  # raw plugin API call
//!
//! Reuses the persistent Firefox profile so login is automatic after first use.

use anyhow::Result;
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::models::FullExport;

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

    match command {
        "exportData" | "export" => {
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            match serde_json::from_value::<FullExport>(raw.clone()) {
                Ok(data) => {
                    eprintln!("Parse: OK");
                    eprintln!("Plans: {}", data.plans.len());
                    eprintln!("Savings accounts: {}", data.today.savings_accounts.len());
                    eprintln!("Investment accounts: {}", data.today.investment_accounts.len());
                    eprintln!("Debts: {}", data.today.debts.len());
                    eprintln!("Assets: {}", data.today.assets.len());
                    eprintln!("Progress points: {}", data.progress.data.len());
                    eprintln!();
                    println!("{}", serde_json::to_string_pretty(&raw)?);
                }
                Err(e) => {
                    eprintln!("Parse FAILED: {}", e);
                    println!("{}", serde_json::to_string_pretty(&raw)?);
                }
            }
        }
        "plans" => {
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            let data: FullExport = serde_json::from_value(raw)?;
            for plan in &data.plans {
                println!(
                    "{} | {} | active={} | expenses={} income={} priorities={} milestones={}",
                    plan.id,
                    plan.name,
                    plan.active,
                    plan.expenses.events.len(),
                    plan.income.events.len(),
                    plan.priorities.events.len(),
                    plan.milestones.len(),
                );
            }
        }
        "accounts" => {
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            let data: FullExport = serde_json::from_value(raw)?;
            for a in &data.today.savings_accounts {
                println!("{} | {} | savings | balance={}", a.id, a.name, a.balance);
            }
            for a in &data.today.investment_accounts {
                println!(
                    "{} | {} | {} | balance={}",
                    a.id, a.name, a.account_type, a.balance
                );
            }
            for d in &data.today.debts {
                println!(
                    "{} | {} | {} | balance={}",
                    d.id, d.name, d.debt_type, d.balance
                );
            }
        }
        "raw" => {
            let method = args.get(1).expect("Usage: call-tool raw <method> [json_args]");
            let api_args: Vec<serde_json::Value> = if let Some(json_str) = args.get(2) {
                serde_json::from_str(json_str)?
            } else {
                vec![]
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

    browser.shutdown().await;
    Ok(())
}
