//! Plan events (expenses, income, priorities, real assets, plan accounts) and milestones.
//!
//! Every plan event shares an identity (`id`, `name`, `kind`, `type`); the
//! remaining fields vary by section and type and are kept as a raw JSON map.
//! Use the `schema_help` tool for field documentation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A plan event. `kind` is one of: expense, income, priority, real-asset, account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanEvent {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub event_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl PlanEvent {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Project selected fields (by their ProjectionLab camelCase names) into a JSON object.
    pub fn summary(&self, keys: &[&str]) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), Value::String(self.id.clone()));
        out.insert("name".into(), Value::String(self.name.clone()));
        out.insert("type".into(), Value::String(self.event_type.clone()));
        for key in keys {
            if let Some(v) = self.fields.get(*key) {
                out.insert((*key).to_string(), v.clone());
            }
        }
        Value::Object(out)
    }
}

/// A user-defined milestone in a plan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Milestone {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub criteria: Vec<Value>,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}
