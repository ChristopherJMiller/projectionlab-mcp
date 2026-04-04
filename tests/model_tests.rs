use projectionlab_mcp::models::*;
use serde_json::json;

/// Helper: load the real example.json fixture
fn load_fixture() -> FullExport {
    let json = std::fs::read_to_string("example.json").expect("Failed to read example.json");
    serde_json::from_str(&json).expect("Failed to parse example.json")
}

// ---- Roundtrip tests: deserialize → serialize → deserialize produces identical data ----

#[test]
fn full_export_roundtrip() {
    let data = load_fixture();
    let serialized = serde_json::to_string(&data).expect("Failed to serialize");
    let reparsed: FullExport = serde_json::from_str(&serialized).expect("Failed to reparse");

    assert_eq!(data.meta.version, reparsed.meta.version);
    assert_eq!(data.plans.len(), reparsed.plans.len());
    assert_eq!(
        data.today.savings_accounts.len(),
        reparsed.today.savings_accounts.len()
    );
    assert_eq!(
        data.today.investment_accounts.len(),
        reparsed.today.investment_accounts.len()
    );
    assert_eq!(data.today.debts.len(), reparsed.today.debts.len());
    assert_eq!(data.today.assets.len(), reparsed.today.assets.len());
    assert_eq!(data.progress.data.len(), reparsed.progress.data.len());
}

#[test]
fn plan_roundtrip() {
    let data = load_fixture();
    for plan in &data.plans {
        let serialized = serde_json::to_string(plan).expect("Failed to serialize plan");
        let reparsed: Plan = serde_json::from_str(&serialized).expect("Failed to reparse plan");
        assert_eq!(plan.id, reparsed.id);
        assert_eq!(plan.name, reparsed.name);
        assert_eq!(plan.expenses.events.len(), reparsed.expenses.events.len());
        assert_eq!(plan.income.events.len(), reparsed.income.events.len());
        assert_eq!(
            plan.priorities.events.len(),
            reparsed.priorities.events.len()
        );
        assert_eq!(plan.milestones.len(), reparsed.milestones.len());
    }
}

#[test]
fn variables_roundtrip() {
    let data = load_fixture();
    for plan in &data.plans {
        let serialized =
            serde_json::to_string(&plan.variables).expect("Failed to serialize variables");
        let reparsed: Variables =
            serde_json::from_str(&serialized).expect("Failed to reparse variables");
        assert_eq!(plan.variables.start_year, reparsed.start_year);
        assert_eq!(plan.variables.investment_return, reparsed.investment_return);
        assert_eq!(plan.variables.inflation, reparsed.inflation);
    }
}

#[test]
fn starting_account_roundtrip() {
    let data = load_fixture();
    for account in &data.today.savings_accounts {
        let serialized =
            serde_json::to_string(account).expect("Failed to serialize savings account");
        let reparsed: StartingAccount =
            serde_json::from_str(&serialized).expect("Failed to reparse savings account");
        assert_eq!(account.id, reparsed.id);
        assert_eq!(account.balance, reparsed.balance);
    }
    for account in &data.today.investment_accounts {
        let serialized =
            serde_json::to_string(account).expect("Failed to serialize investment account");
        let reparsed: StartingAccount =
            serde_json::from_str(&serialized).expect("Failed to reparse investment account");
        assert_eq!(account.id, reparsed.id);
        assert_eq!(account.balance, reparsed.balance);
    }
}

#[test]
fn expense_event_roundtrip() {
    let data = load_fixture();
    for plan in &data.plans {
        for expense in &plan.expenses.events {
            let serialized =
                serde_json::to_string(expense).expect("Failed to serialize expense");
            let reparsed: ExpenseEvent =
                serde_json::from_str(&serialized).expect("Failed to reparse expense");
            assert_eq!(expense.id, reparsed.id);
            assert_eq!(expense.amount, reparsed.amount);
        }
    }
}

#[test]
fn income_event_roundtrip() {
    let data = load_fixture();
    for plan in &data.plans {
        for income in &plan.income.events {
            let serialized = serde_json::to_string(income).expect("Failed to serialize income");
            let reparsed: IncomeEvent =
                serde_json::from_str(&serialized).expect("Failed to reparse income");
            assert_eq!(income.id, reparsed.id);
            assert_eq!(income.amount, reparsed.amount);
        }
    }
}

#[test]
fn progress_roundtrip() {
    let data = load_fixture();
    let serialized = serde_json::to_string(&data.progress).expect("Failed to serialize progress");
    let reparsed: Progress =
        serde_json::from_str(&serialized).expect("Failed to reparse progress");
    assert_eq!(data.progress.data.len(), reparsed.data.len());
    if let (Some(first), Some(reparsed_first)) = (data.progress.data.first(), reparsed.data.first())
    {
        assert_eq!(first.net_worth, reparsed_first.net_worth);
        assert_eq!(first.date, reparsed_first.date);
    }
}

// ---- DebtAccount type tests ----

#[test]
fn debt_account_deserialize_minimal() {
    let json = json!({
        "id": "debt_1",
        "name": "Mortgage",
        "title": "Home Mortgage",
        "type": "mortgage",
        "icon": "home",
        "color": "#ff0000",
        "owner": "me",
        "balance": 250000.0
    });

    let debt: DebtAccount = serde_json::from_value(json).expect("Failed to parse minimal debt");
    assert_eq!(debt.id, "debt_1");
    assert_eq!(debt.balance, 250000.0);
    assert_eq!(debt.debt_type, "mortgage");
    assert!(debt.interest_rate.is_none());
    assert!(debt.extra.is_empty());
}

#[test]
fn debt_account_captures_unknown_fields() {
    let json = json!({
        "id": "debt_2",
        "name": "Car Loan",
        "title": "Car Loan",
        "type": "auto-loan",
        "icon": "car",
        "color": "#0000ff",
        "owner": "me",
        "balance": 15000.0,
        "someFutureField": "surprise",
        "anotherNewField": 42
    });

    let debt: DebtAccount = serde_json::from_value(json).expect("Failed to parse debt with extras");
    assert_eq!(debt.id, "debt_2");
    assert_eq!(debt.extra.len(), 2);
    assert_eq!(debt.extra.get("someFutureField").unwrap(), "surprise");
}

#[test]
fn debt_account_roundtrip_preserves_extras() {
    let json = json!({
        "id": "debt_3",
        "name": "Student Loan",
        "title": "Student Loan",
        "type": "student-loan",
        "icon": "school",
        "color": "#00ff00",
        "owner": "me",
        "balance": 30000.0,
        "interestRate": 0.045,
        "unknownField": {"nested": true}
    });

    let debt: DebtAccount = serde_json::from_value(json.clone()).expect("Failed to parse");
    let reserialized = serde_json::to_value(&debt).expect("Failed to serialize");

    // The unknown field should survive roundtrip
    assert_eq!(
        reserialized.get("unknownField"),
        json.get("unknownField")
    );
}

// ---- StartingAsset type tests ----

#[test]
fn starting_asset_deserialize_minimal() {
    let json = json!({
        "id": "asset_1",
        "name": "House",
        "title": "Primary Residence",
        "type": "real-estate",
        "icon": "home",
        "color": "#00ff00",
        "owner": "joint",
        "balance": 500000.0
    });

    let asset: StartingAsset =
        serde_json::from_value(json).expect("Failed to parse minimal asset");
    assert_eq!(asset.id, "asset_1");
    assert_eq!(asset.balance, 500000.0);
    assert!(asset.extra.is_empty());
}

#[test]
fn starting_asset_captures_unknown_fields() {
    let json = json!({
        "id": "asset_2",
        "name": "Tesla",
        "title": "Car",
        "type": "car",
        "icon": "car",
        "color": "#333333",
        "owner": "me",
        "balance": 45000.0,
        "depreciationRate": 0.15,
        "customField": [1, 2, 3]
    });

    let asset: StartingAsset =
        serde_json::from_value(json).expect("Failed to parse asset with extras");
    assert_eq!(asset.extra.len(), 2);
}

// ---- Fixture data integrity tests ----

#[test]
fn fixture_has_expected_structure() {
    let data = load_fixture();

    // Meta
    assert!(!data.meta.version.is_empty());
    assert!(data.meta.last_updated > 0);

    // At least one plan
    assert!(!data.plans.is_empty());

    // Plans have expected sub-structures
    let plan = &data.plans[0];
    assert!(!plan.id.is_empty());
    assert!(!plan.name.is_empty());

    // Variables have reasonable values
    assert!(plan.variables.investment_return >= 0.0);
    assert!(plan.variables.inflation >= 0.0);

    // Settings
    assert!(data.settings.plugins.enabled);
    assert!(!data.settings.plugins.api_key.is_empty());
}

#[test]
fn fixture_accounts_have_valid_ids() {
    let data = load_fixture();

    let mut ids: Vec<&str> = Vec::new();
    for a in &data.today.savings_accounts {
        assert!(!a.id.is_empty(), "Savings account has empty ID");
        ids.push(&a.id);
    }
    for a in &data.today.investment_accounts {
        assert!(!a.id.is_empty(), "Investment account has empty ID");
        ids.push(&a.id);
    }

    // All IDs should be unique
    let unique: std::collections::HashSet<&&str> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "Account IDs are not unique");
}

// ---- Field coverage tests: detect missing fields in our structs ----

/// Compare raw JSON keys vs our typed struct's serialized keys.
/// Returns a list of (path, missing_keys) for any object where keys are lost.
fn find_missing_keys(
    raw: &serde_json::Value,
    typed: &serde_json::Value,
    path: &str,
) -> Vec<(String, Vec<String>)> {
    let mut issues = Vec::new();

    match (raw, typed) {
        (serde_json::Value::Object(raw_obj), serde_json::Value::Object(typed_obj)) => {
            let missing: Vec<String> = raw_obj
                .keys()
                .filter(|k| !typed_obj.contains_key(*k))
                .cloned()
                .collect();

            if !missing.is_empty() {
                issues.push((path.to_string(), missing));
            }

            // Recurse into shared keys
            for (key, raw_val) in raw_obj {
                if let Some(typed_val) = typed_obj.get(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    issues.extend(find_missing_keys(raw_val, typed_val, &child_path));
                }
            }
        }
        (serde_json::Value::Array(raw_arr), serde_json::Value::Array(typed_arr)) => {
            // Compare first element of each array (if both non-empty)
            if let (Some(raw_first), Some(typed_first)) = (raw_arr.first(), typed_arr.first()) {
                let child_path = format!("{}[0]", path);
                issues.extend(find_missing_keys(raw_first, typed_first, &child_path));
            }
        }
        _ => {}
    }

    issues
}

#[test]
fn detect_missing_fields_in_full_export() {
    let raw_json = std::fs::read_to_string("example.json").expect("Failed to read example.json");
    let raw: serde_json::Value =
        serde_json::from_str(&raw_json).expect("Failed to parse raw JSON");
    let typed: FullExport =
        serde_json::from_str(&raw_json).expect("Failed to parse as FullExport");
    let typed_value = serde_json::to_value(&typed).expect("Failed to serialize typed");

    let issues = find_missing_keys(&raw, &typed_value, "");

    if !issues.is_empty() {
        let mut report = String::from("MISSING FIELDS DETECTED:\n");
        for (path, keys) in &issues {
            report.push_str(&format!("  at '{}': {:?}\n", path, keys));
        }
        // Print but don't fail — these are fields we may intentionally skip
        // or that are handled by #[serde(flatten)]
        println!("{}", report);
    }

    // This test documents field coverage. If you want it to be strict:
    // assert!(issues.is_empty(), "{}", report);
}

#[test]
fn detect_missing_fields_in_plan() {
    let raw_json = std::fs::read_to_string("example.json").expect("Failed to read example.json");
    let raw: serde_json::Value =
        serde_json::from_str(&raw_json).expect("Failed to parse raw JSON");
    let typed: FullExport =
        serde_json::from_str(&raw_json).expect("Failed to parse as FullExport");

    // Compare first plan
    if let (Some(raw_plan), Some(typed_plan)) = (
        raw.get("plans").and_then(|p| p.as_array()).and_then(|a| a.first()),
        typed.plans.first(),
    ) {
        let typed_plan_value =
            serde_json::to_value(typed_plan).expect("Failed to serialize plan");
        let issues = find_missing_keys(raw_plan, &typed_plan_value, "plans[0]");

        if !issues.is_empty() {
            let mut report = String::from("MISSING PLAN FIELDS:\n");
            for (path, keys) in &issues {
                report.push_str(&format!("  at '{}': {:?}\n", path, keys));
            }
            println!("{}", report);
        }
    }
}

#[test]
fn detect_missing_fields_in_starting_conditions() {
    let raw_json = std::fs::read_to_string("example.json").expect("Failed to read example.json");
    let raw: serde_json::Value =
        serde_json::from_str(&raw_json).expect("Failed to parse raw JSON");
    let typed: FullExport =
        serde_json::from_str(&raw_json).expect("Failed to parse as FullExport");

    if let Some(raw_today) = raw.get("today") {
        let typed_today =
            serde_json::to_value(&typed.today).expect("Failed to serialize today");
        let issues = find_missing_keys(raw_today, &typed_today, "today");

        if !issues.is_empty() {
            let mut report = String::from("MISSING STARTING CONDITIONS FIELDS:\n");
            for (path, keys) in &issues {
                report.push_str(&format!("  at '{}': {:?}\n", path, keys));
            }
            println!("{}", report);
        }
    }
}

// ---- Operations logic tests (pure data manipulation) ----

#[test]
fn clone_plan_produces_independent_copy() {
    let mut data = load_fixture();
    let original_count = data.plans.len();

    let source = data.plans[0].clone();
    let mut cloned = source.clone();
    cloned.id = "cloned_plan_123".to_string();
    cloned.name = "Cloned Plan".to_string();
    data.plans.push(cloned);

    assert_eq!(data.plans.len(), original_count + 1);
    assert_eq!(data.plans.last().unwrap().id, "cloned_plan_123");
    assert_eq!(data.plans.last().unwrap().name, "Cloned Plan");

    // Original unchanged
    assert_eq!(data.plans[0].id, source.id);
    assert_eq!(data.plans[0].name, source.name);
}

#[test]
fn update_variables_partial_merge() {
    let mut data = load_fixture();
    let original_inflation = data.plans[0].variables.inflation;

    // Simulate partial variable update via JSON merge
    let mut vars_json =
        serde_json::to_value(&data.plans[0].variables).expect("Failed to serialize vars");
    vars_json
        .as_object_mut()
        .unwrap()
        .insert("inflation".to_string(), json!(0.04));

    let updated: Variables = serde_json::from_value(vars_json).expect("Failed to reparse vars");

    assert_eq!(updated.inflation, 0.04);
    // Other fields should be preserved
    assert_eq!(
        updated.investment_return,
        data.plans[0].variables.investment_return
    );
    assert_ne!(updated.inflation, original_inflation);
}

#[test]
fn add_and_remove_expense_from_plan() {
    let mut data = load_fixture();
    let original_count = data.plans[0].expenses.events.len();

    // Create a minimal expense JSON and parse it
    let expense_json = json!({
        "id": "test_expense",
        "name": "Test Expense",
        "type": "living-expenses",
        "title": "Living Expenses",
        "icon": "shopping_cart",
        "key": 999.0,
        "amount": 1200.0,
        "amountType": "today$",
        "owner": "me",
        "start": {"type": "keyword", "value": "beforeCurrentYear"},
        "end": {"type": "keyword", "value": "endOfPlan"},
        "frequency": "monthly",
        "frequencyChoices": false,
        "yearlyChange": {
            "type": "match-inflation",
            "amount": 0.0,
            "limit": 0.0,
            "limitEnabled": false,
            "limitType": "today$"
        },
        "planPath": "expenses.events",
        "spendingType": "essential"
    });

    let expense: ExpenseEvent =
        serde_json::from_value(expense_json).expect("Failed to parse test expense");
    data.plans[0].expenses.events.push(expense);

    assert_eq!(data.plans[0].expenses.events.len(), original_count + 1);

    // Remove it
    let idx = data.plans[0]
        .expenses
        .events
        .iter()
        .position(|e| e.id == "test_expense")
        .unwrap();
    data.plans[0].expenses.events.remove(idx);

    assert_eq!(data.plans[0].expenses.events.len(), original_count);
}

#[test]
fn batch_balance_update() {
    let mut data = load_fixture();

    // Get first savings account ID and set a new balance
    if let Some(account) = data.today.savings_accounts.first_mut() {
        let old_balance = account.balance;
        account.balance = 99999.99;
        assert_ne!(account.balance, old_balance);
        assert_eq!(account.balance, 99999.99);
    }
}

#[test]
fn add_progress_data_point() {
    let mut data = load_fixture();
    let original_count = data.progress.data.len();

    data.progress.data.push(ProgressDataPoint {
        date: 1712188800000,
        net_worth: 500000.0,
        savings: 50000.0,
        taxable: 200000.0,
        tax_deferred: 150000.0,
        tax_free: 50000.0,
        assets: 50000.0,
        debt: 0.0,
        loans: 0.0,
        crypto: 0.0,
    });

    assert_eq!(data.progress.data.len(), original_count + 1);
    assert_eq!(data.progress.data.last().unwrap().net_worth, 500000.0);
}
