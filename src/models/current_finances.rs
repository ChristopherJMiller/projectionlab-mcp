//! Current Finances: the household's present-day accounts, debts, and real assets.
//!
//! In ProjectionLab V5 every balance-sheet item lives in a single `accounts`
//! map keyed by id. The `kind` field distinguishes them:
//!
//! - `account`    — savings and investment accounts (`type`: savings, taxable, 401k, roth-ira, hsa, crypto, ...)
//! - `debt`       — loans and credit (`type`: mortgage, auto-loan, student-loans, credit-card, secured-loan, ...)
//! - `real-asset` — property and valuables (`type`: real-estate, car, land, boat, jewelry, other, ...)
//!
//! Plans with `startingConditionsType: "custom"` carry their own copy of this
//! document in `plan.startingConditions`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const KIND_ACCOUNT: &str = "account";
pub const KIND_DEBT: &str = "debt";
pub const KIND_REAL_ASSET: &str = "real-asset";

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentFinances {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub household: Option<Household>,
    #[serde(default)]
    pub accounts: BTreeMap<String, CurrentFinancesAccount>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CurrentFinances {
    /// All accounts sorted the way ProjectionLab displays them
    /// (savings, investments, real assets, debts; then by `order`).
    pub fn sorted_accounts(&self) -> Vec<&CurrentFinancesAccount> {
        let mut accounts: Vec<_> = self.accounts.values().collect();
        accounts.sort_by(|a, b| {
            a.display_section()
                .cmp(&b.display_section())
                .then(a.order.unwrap_or(0).cmp(&b.order.unwrap_or(0)))
                .then(a.id.cmp(&b.id))
        });
        accounts
    }

    /// Non-archived accounts, sorted for display.
    pub fn active_accounts(&self) -> Vec<&CurrentFinancesAccount> {
        self.sorted_accounts()
            .into_iter()
            .filter(|a| !a.is_archived())
            .collect()
    }

    pub fn find(&self, id: &str) -> Option<&CurrentFinancesAccount> {
        self.accounts.get(id)
    }

    /// Net worth of active items: accounts + real assets − debts.
    pub fn net_worth(&self) -> f64 {
        let total: f64 = self.active_accounts().iter().map(|a| a.signed_value()).sum();
        (total * 100.0).round() / 100.0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Household {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner_status: Option<String>,
    /// Keyed by person id ("me", "spouse").
    #[serde(default)]
    pub people: BTreeMap<String, Person>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    /// "me" or "spouse"
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth_year: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth_month: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// A single Current Finances item (account, debt, or real asset).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentFinancesAccount {
    pub id: String,
    /// "account" | "debt" | "real-asset"
    pub kind: String,
    #[serde(rename = "type")]
    pub account_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default)]
    pub name: String,
    /// "me" | "spouse" | "joint" (absent on some kinds)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Current balance (accounts and debts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<f64>,
    /// Current value (real assets)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purchase_price: Option<f64>,
    /// ISO date the item was archived; archived items are excluded from net worth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CurrentFinancesAccount {
    pub fn is_archived(&self) -> bool {
        self.archived_on.as_deref().is_some_and(|d| !d.is_empty())
    }

    pub fn is_debt(&self) -> bool {
        self.kind == KIND_DEBT
    }

    pub fn is_real_asset(&self) -> bool {
        self.kind == KIND_REAL_ASSET
    }

    /// The headline number for this item: `value` for real assets, `balance` otherwise.
    pub fn current_value(&self) -> f64 {
        if self.is_real_asset() {
            self.value.unwrap_or(0.0)
        } else {
            self.balance.unwrap_or(0.0)
        }
    }

    /// Contribution to net worth (debts count negatively).
    pub fn signed_value(&self) -> f64 {
        if self.is_debt() {
            -self.current_value()
        } else {
            self.current_value()
        }
    }

    /// Which `updateAccountValues` field holds this item's headline value.
    pub fn value_field(&self) -> &'static str {
        if self.is_real_asset() {
            "value"
        } else {
            "balance"
        }
    }

    /// Tax/liquidity bucket used by ProjectionLab's net worth breakdown.
    pub fn bucket(&self) -> &'static str {
        match self.kind.as_str() {
            KIND_DEBT => "debt",
            KIND_REAL_ASSET => "realAssets",
            _ => account_type_bucket(&self.account_type),
        }
    }

    fn display_section(&self) -> u8 {
        match self.kind.as_str() {
            KIND_ACCOUNT if self.account_type == "savings" => 0,
            KIND_ACCOUNT => 1,
            KIND_REAL_ASSET => 2,
            _ => 3,
        }
    }
}

/// Supported `kind: "account"` types, from ProjectionLab's account registry.
pub const ACCOUNT_TYPES: &[&str] = &[
    "savings",
    "taxable",
    "crypto",
    "401k",
    "roth-401k",
    "403b",
    "roth-403b",
    "457b",
    "roth-457b",
    "ira",
    "roth-ira",
    "hsa",
    "529",
    "pension",
    "rrsp",
    "tfsa",
    "sipp",
    "stock-isa",
    "lifetime-isa",
    "uk-pension",
    "superannuation",
    "super-nonconcessional",
    "kh",
];

/// Supported `kind: "real-asset"` types.
pub const REAL_ASSET_TYPES: &[&str] = &[
    "real-estate",
    "car",
    "land",
    "motorcycle",
    "boat",
    "jewelry",
    "precious-metals",
    "furniture",
    "instrument",
    "machinery",
    "other",
];

/// Common `kind: "debt"` types (ProjectionLab validates the final list).
pub const DEBT_TYPES: &[&str] = &[
    "mortgage",
    "home-equity-loan",
    "auto-loan",
    "secured-loan",
    "student-loans",
    "credit-card",
    "personal-loan",
    "medical-debt",
    "other",
];

/// Map an account type to ProjectionLab's tax bucket.
pub fn account_type_bucket(account_type: &str) -> &'static str {
    match account_type {
        "savings" => "cash",
        "taxable" => "taxable",
        "crypto" => "crypto",
        "roth-401k" | "roth-403b" | "roth-457b" | "roth-ira" | "hsa" | "529" | "tfsa"
        | "stock-isa" | "lifetime-isa" | "superannuation" | "super-nonconcessional" | "kh" => {
            "taxFree"
        }
        _ => "taxDeferred",
    }
}
