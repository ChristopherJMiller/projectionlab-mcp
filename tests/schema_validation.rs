use projectionlab_mcp::models::{Plan, FullExport};
use schemars::schema_for;
use std::fs;

#[test]
fn generate_and_compare_schema() {
    // Generate schema from our Rust types
    let our_schema = schema_for!(Plan);
    let our_schema_json = serde_json::to_string_pretty(&our_schema)
        .expect("Failed to serialize our schema");

    // Write our generated schema to a file for comparison
    fs::write("schema_from_rust.json", &our_schema_json)
        .expect("Failed to write schema_from_rust.json");

    println!("✓ Generated schema_from_rust.json from Rust types");

    // Read the schema generated from example.json by quicktype
    let example_schema_json = fs::read_to_string("schema.json")
        .expect("Failed to read schema.json - run: quicktype --lang schema example.json -o schema.json");

    println!("✓ Loaded schema.json from quicktype");

    // Parse both schemas
    let our_schema_value: serde_json::Value = serde_json::from_str(&our_schema_json)
        .expect("Failed to parse our schema");
    let example_schema_value: serde_json::Value = serde_json::from_str(&example_schema_json)
        .expect("Failed to parse example schema");

    // Print summary
    println!("\n=== Schema Comparison ===");
    println!("Our schema definitions: {}", count_definitions(&our_schema_value));
    println!("Example schema definitions: {}", count_definitions(&example_schema_value));

    // The schemas won't match exactly (different formats, naming, etc.)
    // but this test documents the difference and generates the comparison file
    println!("\nSchemas generated successfully!");
    println!("Compare schema.json (from example.json) vs schema_from_rust.json (from Rust types)");
}

fn count_definitions(schema: &serde_json::Value) -> usize {
    schema
        .get("definitions")
        .and_then(|d| d.as_object())
        .map(|o| o.len())
        .unwrap_or(0)
}

#[test]
fn parse_example_json() {
    // Read and parse example.json with our Rust types
    let example_json = fs::read_to_string("example.json")
        .expect("Failed to read example.json");

    let data: FullExport = serde_json::from_str(&example_json)
        .expect("Failed to parse example.json with our Rust types");

    println!("\n=== Successfully Parsed example.json ===");
    println!("Version: {}", data.meta.version);
    println!("Last updated: {}", data.meta.last_updated);
    println!("Number of plans: {}", data.plans.len());
    println!("Number of savings accounts: {}", data.today.savings_accounts.len());
    println!("Number of investment accounts: {}", data.today.investment_accounts.len());

    // Verify we have at least one plan
    assert!(!data.plans.is_empty(), "Should have at least one plan");

    // Verify the first plan has expected fields
    let first_plan = &data.plans[0];
    println!("\nFirst plan:");
    println!("  ID: {}", first_plan.id);
    println!("  Name: {}", first_plan.name);
    println!("  Active: {}", first_plan.active);
    println!("  Number of expenses: {}", first_plan.expenses.events.len());
    println!("  Number of income events: {}", first_plan.income.events.len());

    println!("\n✓ All data structures parsed successfully!");
}
