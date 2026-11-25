use projectionlab_mcp::models::FullExport;
use schemars::schema_for;
use std::fs;

/// Generate `schema_from_rust.json` (gitignored) documenting the typed parts
/// of the V5 export model. Fields not modelled explicitly are preserved via
/// flattened maps and documented by the `schema_help` tool instead.
#[test]
fn generate_schema() {
    let schema = schema_for!(FullExport);
    let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
    fs::write("schema_from_rust.json", &json).expect("write schema_from_rust.json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("properties").and_then(|p| p.get("documents")).is_some());
}
