//! Schema documentation tool — returns field-level docs for ProjectionLab entity types.
//!
//! An LLM caller can use `schema_help` before creating/updating events to understand
//! required fields, valid values, and type-specific variations.

use rmcp::model::*;
use serde_json::json;

/// All topics the schema_help tool can explain.
const TOPICS: &[&str] = &[
    "topics",
    "date_or_milestone",
    "yearly_change",
    "expense",
    "income",
    "priority",
    "account",
    "plan_account",
    "asset",
    "starting_asset",
    "debt",
    "withdrawal_strategy",
    "milestone",
];

pub fn lookup(topic: &str) -> CallToolResult {
    let doc = match topic {
        "topics" => topics_index(),
        "date_or_milestone" => date_or_milestone(),
        "yearly_change" => yearly_change(),
        "expense" => expense(),
        "income" => income(),
        "priority" => priority(),
        "account" => account(),
        "plan_account" => plan_account(),
        "asset" => asset(),
        "starting_asset" => starting_asset(),
        "debt" => debt(),
        "withdrawal_strategy" => withdrawal_strategy(),
        "milestone" => milestone(),
        _ => {
            return CallToolResult::success(vec![Content::text(format!(
                "Unknown topic: '{}'. Call schema_help with topic=\"topics\" to see all available topics.",
                topic
            ))]);
        }
    };

    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&doc).unwrap(),
    )])
}

fn topics_index() -> serde_json::Value {
    json!({
        "available_topics": TOPICS,
        "description": "Call schema_help with any topic to get field documentation, valid values, and examples. Start here to understand what fields are needed before creating or updating entities.",
        "recommended_reading_order": [
            "date_or_milestone — used by every event's start/end timing",
            "yearly_change — used by expenses, income, assets for growth modeling",
            "Then read the specific entity type you want to create/update"
        ]
    })
}

fn date_or_milestone() -> serde_json::Value {
    json!({
        "description": "DateOrMilestone specifies when an event starts or ends. Used by every event's `start` and `end` fields.",
        "fields": {
            "type": "required — 'keyword' | 'date' | 'milestone'",
            "value": "required — depends on type (see below)",
            "modifier": "optional — 'include' | 'exclude' | numeric offset (years)"
        },
        "variants": {
            "keyword": {
                "description": "Relative/symbolic dates",
                "valid_values": [
                    "beforeCurrentYear — before the simulation starts",
                    "endOfPlan — end of the simulation horizon",
                    "retirement — when the user retires (computed milestone)",
                    "death — end of life expectancy"
                ],
                "example": { "type": "keyword", "value": "beforeCurrentYear" }
            },
            "date": {
                "description": "Exact calendar date (ISO format)",
                "example": { "type": "date", "value": "2027-01-01" }
            },
            "milestone": {
                "description": "Reference to a user-defined milestone by UUID",
                "modifier_meaning": "'include' = on or after milestone year, 'exclude' = year after milestone",
                "example": { "type": "milestone", "value": "abc-123-uuid", "modifier": "include" }
            }
        },
        "common_patterns": {
            "starts_now_ends_at_retirement": {
                "start": { "type": "keyword", "value": "beforeCurrentYear" },
                "end": { "type": "keyword", "value": "retirement" }
            },
            "starts_on_specific_date": {
                "start": { "type": "date", "value": "2030-06-01" },
                "end": { "type": "keyword", "value": "endOfPlan" }
            }
        }
    })
}

fn yearly_change() -> serde_json::Value {
    json!({
        "description": "YearlyChange models how an amount grows or shrinks over time. Used by expenses, income, and assets.",
        "fields": {
            "type": "required — 'none' | 'match-inflation' | 'inflation+' | 'custom' | 'depreciate' | 'appreciate'",
            "amount": "rate value (e.g., 3.0 for 3%)",
            "amountType": "optional — 'today$' | '$' | '%' (default '%')",
            "limit": "cap on growth (0 if unused)",
            "limitEnabled": "boolean — whether the limit is active",
            "limitType": "'today$' | '$' | '%'",
            "custom": "optional — only when type='custom'"
        },
        "variants": {
            "none": {
                "description": "No yearly change — amount stays flat",
                "example": { "type": "none", "amount": 0, "limit": 0, "limitEnabled": false, "limitType": "%" }
            },
            "match-inflation": {
                "description": "Grows with the plan's inflation rate",
                "example": { "type": "match-inflation", "amount": 0, "limit": 0, "limitEnabled": false, "limitType": "%" }
            },
            "inflation+": {
                "description": "Grows at inflation + X%",
                "example": { "type": "inflation+", "amount": 2.0, "limit": 0, "limitEnabled": false, "limitType": "%" }
            },
            "depreciate": {
                "description": "Decreases by X% per year (common for cars)",
                "example": { "type": "depreciate", "amount": 8.0, "limit": 0, "limitEnabled": false, "limitType": "%" }
            },
            "appreciate": {
                "description": "Increases by X% per year (real estate, collectibles)",
                "example": { "type": "appreciate", "amount": 3.0, "limit": 0, "limitEnabled": false, "limitType": "%" }
            },
            "custom": {
                "description": "Year-by-year custom values",
                "example": {
                    "type": "custom",
                    "amount": 0,
                    "limit": 0,
                    "limitEnabled": false,
                    "limitType": "%",
                    "custom": {
                        "type": "inflation+%",
                        "data": [
                            { "x": 2025, "y": 5.0 },
                            { "x": 2030, "y": 3.0 },
                            { "x": 2040, "y": 2.0 }
                        ]
                    }
                }
            }
        }
    })
}

fn expense() -> serde_json::Value {
    json!({
        "description": "ExpenseEvent — a spending item in a plan. Types: living-expenses, rent, travel, vacation, wedding, healthcare, education, childcare, custom, etc.",
        "required_fields": {
            "name": "display name",
            "type": "expense category — 'living-expenses', 'rent', 'travel', 'vacation', 'wedding', 'healthcare', 'education', 'childcare', 'custom', etc.",
            "title": "short title (usually same as name)",
            "icon": "Material Design icon string (e.g., 'mdi-home', 'mdi-airplane')",
            "amount": "numeric amount",
            "amountType": "'today$' (inflation-adjusted) | '$' (nominal) | '%'",
            "owner": "'me' | 'spouse' | 'joint'",
            "start": "DateOrMilestone — when this expense begins (see date_or_milestone topic)",
            "end": "DateOrMilestone — when this expense ends",
            "frequency": "'monthly' | 'yearly' | 'once' | 'biweekly' | 'semi-monthly' | 'quarterly' | 'semi-annually'",
            "frequencyChoices": "boolean (usually true)",
            "yearlyChange": "YearlyChange — how the amount grows (see yearly_change topic)",
            "spendingType": "'essential' | 'discretionary'",
            "planPath": "'expenses' (always this value)",
            "key": "numeric key for ordering (use Date.now() timestamp)"
        },
        "optional_fields": {
            "repeat": "boolean — whether the expense repeats at intervals",
            "repeatInterval": "number of intervals between repeats",
            "repeatIntervalType": "'yearly' — unit for repeat",
            "repeatScaler": "multiplier for repeat interval",
            "repeatEnd": "DateOrMilestone — when repeating stops",
            "fundWithAccount": "boolean — fund from a specific account"
        },
        "auto_generated_fields": {
            "id": "auto-generated if not provided (format: exp_<timestamp>)"
        },
        "example": {
            "name": "Vacation",
            "type": "vacation",
            "title": "Vacation",
            "icon": "mdi-airplane",
            "amount": 5000,
            "amountType": "today$",
            "owner": "joint",
            "start": { "type": "keyword", "value": "beforeCurrentYear" },
            "end": { "type": "keyword", "value": "endOfPlan" },
            "frequency": "yearly",
            "frequencyChoices": true,
            "yearlyChange": { "type": "match-inflation", "amount": 0, "limit": 0, "limitEnabled": false, "limitType": "%" },
            "spendingType": "discretionary",
            "planPath": "expenses",
            "key": 1700000000000.0
        }
    })
}

fn income() -> serde_json::Value {
    json!({
        "description": "IncomeEvent — an income stream in a plan. The `type` field determines which optional fields are relevant.",
        "types": {
            "salary": "W-2 employment income. Supports pension and part-time fields.",
            "rsu": "Restricted stock units. Supports routeToAccounts.",
            "other": "Catch-all: rental income, side income, etc. Supports isDividendIncome, isPassiveIncome, selfEmployment, wage flags."
        },
        "required_fields": {
            "name": "display name",
            "type": "'salary' | 'rsu' | 'other'",
            "title": "short title",
            "icon": "Material Design icon (e.g., 'mdi-briefcase', 'mdi-chart-line')",
            "amount": "numeric amount per frequency period",
            "amountType": "'today$' | '$' | '%'",
            "owner": "'me' | 'spouse'",
            "start": "DateOrMilestone",
            "end": "DateOrMilestone",
            "frequency": "'monthly' | 'yearly' | 'biweekly' | 'semi-monthly' | 'quarterly' | 'semi-annually'",
            "frequencyChoices": "boolean (usually true)",
            "yearlyChange": "YearlyChange — raise/growth pattern",
            "taxExempt": "boolean — is this income tax-exempt?",
            "taxWithholding": "boolean — enable tax withholding?",
            "withhold": "number — withholding percentage (0-100)",
            "planPath": "'income' (always this value)",
            "key": "numeric ordering key"
        },
        "salary_specific_fields": {
            "hasPension": "boolean — does this job have a pension?",
            "pensionContribution": "number — employee contribution amount",
            "pensionContributionType": "'$' | '%'",
            "contribsReduceTaxableIncome": "boolean",
            "pensionPayoutsStart": "DateOrMilestone — when pension payments begin",
            "pensionPayoutsEnd": "DateOrMilestone — when pension payments end",
            "pensionPayoutAmount": "number — annual payout",
            "pensionPayoutRate": "number — payout as % of salary",
            "pensionPayoutType": "'amount' | 'rate'",
            "pensionPayoutsAreTaxFree": "boolean",
            "goPartTime": "boolean — transition to part-time?",
            "partTimeStart": "DateOrMilestone",
            "partTimeEnd": "DateOrMilestone",
            "partTimeRate": "number — fraction of full-time (0.0-1.0)"
        },
        "rsu_specific_fields": {
            "routeToAccounts": "string[] — account IDs to route vested RSUs into"
        },
        "other_specific_fields": {
            "isDividendIncome": "boolean — treated as dividend income for tax",
            "isPassiveIncome": "boolean — passive income flag",
            "selfEmployment": "boolean — subject to self-employment tax",
            "wage": "boolean — treated as wage income"
        },
        "example_salary": {
            "name": "Software Engineer",
            "type": "salary",
            "title": "Software Engineer",
            "icon": "mdi-briefcase",
            "amount": 150000,
            "amountType": "today$",
            "owner": "me",
            "start": { "type": "keyword", "value": "beforeCurrentYear" },
            "end": { "type": "keyword", "value": "retirement" },
            "frequency": "yearly",
            "frequencyChoices": true,
            "yearlyChange": { "type": "inflation+", "amount": 2.0, "limit": 0, "limitEnabled": false, "limitType": "%" },
            "taxExempt": false,
            "taxWithholding": true,
            "withhold": 25.0,
            "planPath": "income",
            "key": 1700000000000.0,
            "hasPension": false
        }
    })
}

fn priority() -> serde_json::Value {
    json!({
        "description": "PriorityEvent — a savings goal or contribution strategy. The `type` field determines which fields are relevant and which account it targets.",
        "types": {
            "401k": "401(k) contribution — targets a retirement account, has employer match and contribution limits",
            "roth-401k": "Roth 401(k) contribution — same fields as 401k",
            "403b": "403(b) contribution — same fields as 401k",
            "roth-ira": "Roth IRA contribution — has amount, frequency",
            "ira": "Traditional IRA contribution — has amount, frequency",
            "taxable": "Taxable brokerage contribution — has amount, frequency, desiredContribution",
            "savings": "Savings goal — has mode ('target'/'contribute'), tapFund, tapRate",
            "hsa": "HSA contribution — has amount, frequency",
            "529": "529 plan contribution — has amount, frequency"
        },
        "required_fields": {
            "name": "display name",
            "type": "one of the types listed above",
            "title": "short title",
            "subtitle": "description text",
            "icon": "Material Design icon",
            "color": "hex color (e.g., '#4CAF50')",
            "goalIntent": "'goal' | 'contribute'",
            "owner": "'me' | 'spouse'",
            "accountId": "UUID of the target account this priority contributes to",
            "start": "DateOrMilestone",
            "end": "DateOrMilestone",
            "persistent": "boolean — persists after goal is met?",
            "planPath": "'priorities'",
            "key": "numeric ordering key"
        },
        "401k_specific_fields": {
            "contribution": "number — contribution amount per period",
            "contributionType": "'$' | '%' — of salary",
            "contributionLimit": "number — annual contribution cap",
            "contributionsAreFixed": "boolean — fixed vs % of salary",
            "employerMatch": "number — employer match percentage",
            "employerMatchType": "'$' | '%'",
            "employerMatchLimit": "number — cap on employer match",
            "reduceEmployerMatch": "boolean — reduce match in part-time",
            "incomeStreamId": "UUID — which income stream the % is based on",
            "yearlyLimit": "number — IRS annual limit",
            "yearlyLimit$Type": "'today$' | '$'",
            "yearlyLimitType": "'individual' | 'combined'",
            "country": "'US' | 'CA' | 'UK' — affects contribution rules"
        },
        "taxable_specific_fields": {
            "amount": "number — contribution amount",
            "amountType": "'today$' | '$' | '%'",
            "frequency": "'monthly' | 'yearly' | 'once' | etc.",
            "desiredContribution": "'fixed' | 'max'"
        },
        "savings_specific_fields": {
            "mode": "'target' | 'contribute'",
            "amount": "number — target amount or contribution",
            "amountType": "'today$' | '$' | '%'",
            "frequency": "'monthly' | 'yearly'",
            "tapFund": "boolean — allow withdrawals from this fund",
            "tapRate": "number — max withdrawal rate %"
        },
        "example_401k": {
            "name": "401k Contribution",
            "type": "401k",
            "title": "401k",
            "subtitle": "Max out 401k",
            "icon": "mdi-piggy-bank",
            "color": "#4CAF50",
            "goalIntent": "contribute",
            "owner": "me",
            "accountId": "some-401k-account-uuid",
            "start": { "type": "keyword", "value": "beforeCurrentYear" },
            "end": { "type": "keyword", "value": "retirement" },
            "persistent": true,
            "contribution": 23500,
            "contributionType": "$",
            "contributionsAreFixed": true,
            "employerMatch": 50.0,
            "employerMatchType": "%",
            "employerMatchLimit": 6.0,
            "incomeStreamId": "salary-income-uuid",
            "yearlyLimit": 23500,
            "yearlyLimit$Type": "today$",
            "yearlyLimitType": "individual",
            "country": "US",
            "planPath": "priorities",
            "key": 1700000000000.0
        }
    })
}

fn account() -> serde_json::Value {
    json!({
        "description": "StartingAccount — a financial account in Current Finances. The `type` field determines which optional fields apply. These are the user's current real accounts.",
        "note": "For plan-level account overrides, see the 'plan_account' topic instead.",
        "types": {
            "savings": "Savings/checking account — simplest type, just balance + growth",
            "taxable": "Taxable brokerage — adds costBasis, yearlyFee, dividendReinvestment",
            "401k": "401(k) — adds EWAge, EWPenaltyRate, hasEWPenalty, rmdType, country",
            "roth-401k": "Roth 401(k) — same retirement fields as 401k",
            "roth-ira": "Roth IRA — same retirement fields plus withdrawContribsFree",
            "ira": "Traditional IRA — same retirement fields as 401k",
            "403b": "403(b) — same retirement fields as 401k",
            "457": "457 plan — same retirement fields as 401k",
            "hsa": "HSA — same retirement fields as 401k",
            "529": "529 plan — education savings",
            "pension": "Pension account",
            "crypto": "Cryptocurrency account",
            "other": "Other account type"
        },
        "required_fields": {
            "name": "display name",
            "type": "account type (see above)",
            "title": "short title",
            "icon": "Material Design icon",
            "color": "hex color",
            "owner": "'me' | 'spouse' | 'joint'",
            "balance": "current balance (number)",
            "liquid": "boolean — can this be used for expenses?",
            "withdraw": "boolean — include in withdrawal strategy?",
            "withdrawAge": "DateOrMilestone — when withdrawals can begin",
            "investmentGrowthRate": "number — annual growth rate %",
            "investmentGrowthType": "'fixed' | 'plan' — use fixed rate or plan assumptions",
            "dividendRate": "number — annual dividend yield %",
            "dividendType": "'plan' | 'none'",
            "isPassiveIncome": "boolean — are dividends treated as passive income?"
        },
        "taxable_specific_fields": {
            "costBasis": "number — original cost basis for capital gains",
            "yearlyFee": "number — annual expense ratio/fee",
            "yearlyFeeType": "'$' | '%'",
            "dividendReinvestment": "boolean — auto-reinvest dividends?",
            "dividendTaxType": "'fixed' | 'plan'",
            "dividendsArePassiveIncome": "boolean"
        },
        "retirement_specific_fields": {
            "country": "'US' | 'CA' | 'UK'",
            "EWAge": "number — early withdrawal penalty ends at this age",
            "hasEWPenalty": "boolean — subject to early withdrawal penalty?",
            "EWPenaltyRate": "number — penalty rate % (e.g., 10)",
            "rmdType": "'standard' | 'inherited' — required minimum distribution rules",
            "withdrawContribsFree": "boolean — Roth: can withdraw contributions penalty-free (Roth IRA only)"
        },
        "auto_generated_fields": {
            "id": "auto-generated if not provided"
        }
    })
}

fn plan_account() -> serde_json::Value {
    json!({
        "description": "AccountEvent — a plan-level override for an account. These appear inside a plan's `accounts.events` array and let each plan customize growth rates, balances, or withdrawal settings for the same underlying account.",
        "fields_same_as": "Same field set as StartingAccount (see 'account' topic) — the plan-level version can override any field from the Current Finances account.",
        "key_fields_to_override": {
            "balance": "override the account's starting balance for this plan",
            "investmentGrowthRate": "override growth rate for this plan's scenario",
            "investmentGrowthType": "'fixed' | 'plan'",
            "withdraw": "whether to include in this plan's withdrawal strategy",
            "withdrawAge": "DateOrMilestone — override withdrawal start age"
        },
        "note": "Use plan_accounts_update to change these. The accountId field links back to the Current Finances account."
    })
}

fn asset() -> serde_json::Value {
    let mut doc = serde_json::Map::new();
    doc.insert("description".into(), json!("AssetEvent — a planned asset purchase/ownership in a plan (future home, car, etc.). These are FUTURE planned purchases, not current assets (see 'starting_asset' for current)."));
    doc.insert("types".into(), json!({
        "real-estate": "Home/property — has rental income, HOA, improvement, management fields",
        "car": "Vehicle — typically depreciates",
        "furniture": "Furniture/appliances",
        "other": "Custom asset type"
    }));
    doc.insert("required_fields".into(), json!({
        "name": "display name",
        "type": "'real-estate' | 'car' | 'furniture' | 'other'",
        "title": "short title",
        "icon": "Material Design icon",
        "owner": "'me' | 'spouse' | 'joint'",
        "start": "DateOrMilestone — when you acquire the asset",
        "end": "DateOrMilestone — when you sell/dispose",
        "initialValue": "number — purchase price",
        "initialValueType": "'today$' | '$'",
        "amount": "number — same as initialValue typically",
        "amountType": "'today$' | '$'",
        "yearlyChange": "YearlyChange — appreciation/depreciation",
        "planPath": "'assets'",
        "key": "numeric ordering key"
    }));
    doc.insert("payment_method_fields".into(), json!({
        "paymentMethod": "'pay-in-full' | 'financed' — determines which loan fields matter",
        "note": "When 'financed', all the loan fields below are used. When 'pay-in-full', loan fields are ignored."
    }));
    doc.insert("loan_fields".into(), json!({
        "balance": "number — loan balance (usually initialValue - downPayment)",
        "balanceType": "'today$' | '$'",
        "downPayment": "number — down payment amount",
        "downPaymentType": "'today$' | '$'",
        "monthlyPayment": "number — monthly loan payment",
        "monthlyPaymentType": "'today$' | '$'",
        "interestRate": "number — annual interest rate %",
        "interestType": "'fixed' | 'variable'",
        "compounding": "'monthly' | 'yearly'",
        "excludeLoanFromLNW": "boolean — exclude loan from liquid net worth?"
    }));
    doc.insert("cost_fields".into(), json!({
        "brokersFee": "number — % fee on sale",
        "taxRate": "number — property tax rate",
        "taxRateType": "'$' | '%'",
        "insuranceRate": "number — insurance cost",
        "insuranceRateType": "'$' | '%'",
        "maintenanceRate": "number — maintenance cost",
        "maintenanceRateType": "'$' | '%'"
    }));
    doc.insert("real_estate_specific_fields".into(), json!({
        "initialBuildingValue": "number — building portion (for depreciation)",
        "initialBuildingValueType": "'today$' | '$'",
        "classification": "'residential' | 'commercial'",
        "generateIncome": "boolean — rental property?",
        "percentRented": "number — 0-100",
        "incomeRate": "number — rental yield",
        "incomeRateType": "'$' | '%'",
        "isPassiveIncome": "boolean",
        "cancelRent": "boolean — buying cancels a rent expense?",
        "improvementRate": "number — capital improvements %",
        "improvementRateType": "'$' | '%'",
        "managementRate": "number — property management fee %",
        "managementRateType": "'$' | '%'",
        "monthlyHOA": "number — monthly HOA fee",
        "estimateQBI": "boolean — estimate qualified business income?",
        "selfEmployment": "boolean — self-employment tax?",
        "estimateRentalDeductions": "boolean"
    }));
    doc.insert("repeat_fields".into(), json!({
        "repeat": "boolean — buy this asset repeatedly?",
        "repeatInterval": "number — years between purchases",
        "repeatIntervalType": "'yearly'",
        "repeatEnd": "DateOrMilestone — stop repeating",
        "repeatKeepLast": "boolean — keep the last purchase"
    }));
    doc.insert("example_home_purchase".into(), asset_example());
    serde_json::Value::Object(doc)
}

fn asset_example() -> serde_json::Value {
    json!({
        "name": "Dream Home",
        "type": "real-estate",
        "title": "Dream Home",
        "icon": "mdi-home",
        "owner": "joint",
        "start": { "type": "date", "value": "2028-01-01" },
        "end": { "type": "keyword", "value": "endOfPlan" },
        "initialValue": 600000,
        "initialValueType": "today$",
        "amount": 600000,
        "amountType": "today$",
        "yearlyChange": { "type": "appreciate", "amount": 3.0, "limit": 0, "limitEnabled": false, "limitType": "%" },
        "paymentMethod": "financed",
        "balance": 480000,
        "balanceType": "today$",
        "downPayment": 120000,
        "downPaymentType": "today$",
        "monthlyPayment": 3200,
        "monthlyPaymentType": "$",
        "interestRate": 6.5,
        "interestType": "fixed",
        "compounding": "monthly",
        "excludeLoanFromLNW": false,
        "brokersFee": 6.0,
        "taxRate": 1.2,
        "taxRateType": "%",
        "insuranceRate": 0.5,
        "insuranceRateType": "%",
        "maintenanceRate": 1.0,
        "maintenanceRateType": "%",
        "cancelRent": true,
        "repeat": false,
        "planPath": "assets",
        "key": 1700000000000.0
    })
}

fn starting_asset() -> serde_json::Value {
    json!({
        "description": "StartingAsset — a current asset in Current Finances (car you own now, home you own now, etc.). These are CURRENT real assets, not future planned purchases (see 'asset' for planned).",
        "types": "'car' | 'real-estate' | 'furniture' | 'other'",
        "required_fields": {
            "name": "display name",
            "type": "asset type string",
            "title": "short title",
            "icon": "Material Design icon",
            "color": "hex color",
            "owner": "'me' | 'spouse' | 'joint'",
            "balance": "current market value (number)",
            "initialValue": "original purchase price (number)"
        },
        "auto_generated_fields": {
            "id": "auto-generated if not provided"
        },
        "example": {
            "name": "Tesla Model 3",
            "type": "car",
            "title": "Tesla Model 3",
            "icon": "mdi-car",
            "color": "#FF5722",
            "owner": "me",
            "balance": 25000.0,
            "initialValue": 45000.0
        }
    })
}

fn debt() -> serde_json::Value {
    json!({
        "description": "DebtAccount — a debt/liability in Current Finances (mortgage, auto loan, student loan, credit card).",
        "types": "'mortgage' | 'auto-loan' | 'student-loan' | 'personal-loan' | 'credit-card' | 'other'",
        "required_fields": {
            "name": "display name",
            "type": "debt type string",
            "title": "short title",
            "icon": "Material Design icon",
            "color": "hex color",
            "owner": "'me' | 'spouse' | 'joint'",
            "balance": "current outstanding balance (number)"
        },
        "optional_fields": {
            "interestRate": "number — annual interest rate %",
            "monthlyPayment": "number — monthly payment amount"
        },
        "auto_generated_fields": {
            "id": "auto-generated if not provided"
        },
        "example": {
            "name": "Auto Loan",
            "type": "auto-loan",
            "title": "Auto Loan",
            "icon": "mdi-car",
            "color": "#F44336",
            "owner": "me",
            "balance": 18000.0,
            "interestRate": 4.5,
            "monthlyPayment": 400.0
        }
    })
}

fn withdrawal_strategy() -> serde_json::Value {
    json!({
        "description": "WithdrawalStrategy — how the plan draws down savings in retirement. Set at the plan level via plans_update_variables or plans_get.",
        "top_level_fields": {
            "enabled": "boolean — is a withdrawal strategy active?",
            "strategy": "'initial-%' | 'fixed-%' | 'fixed-amount' | '1/N' | 'vpw' | 'kitces-ratchet' | 'clyatt-95%' | 'guyton-klinger'",
            "start": "DateOrMilestone — when withdrawals begin (usually retirement)",
            "income": "'net' | 'gross' — withdrawal amounts are net or gross of tax",
            "spendMode": "'fixed' | 'flexible'"
        },
        "strategy_variants": {
            "initial-%": {
                "description": "Fixed initial percentage (classic 4% rule)",
                "fields": { "amount": "initial withdrawal rate %" },
                "shared_fields": "min, minType, minEnabled, max, maxType, maxEnabled — floor/ceiling guardrails"
            },
            "fixed-%": {
                "description": "Fixed percentage of current portfolio each year",
                "fields": { "amount": "withdrawal rate %" }
            },
            "fixed-amount": {
                "description": "Fixed dollar amount per year",
                "fields": {
                    "amount": "annual withdrawal amount",
                    "amountType": "'today$' | '$'",
                    "adjust": "boolean — adjust for inflation?"
                }
            },
            "1/N": {
                "description": "Divide portfolio by remaining years",
                "fields": {}
            },
            "vpw": {
                "description": "Variable Percentage Withdrawal",
                "fields": {}
            },
            "kitces-ratchet": {
                "description": "Ratchet up withdrawals when portfolio grows",
                "fields": {
                    "amount": "initial withdrawal rate %",
                    "threshold": "portfolio growth threshold to trigger ratchet",
                    "ratchet": "% increase when triggered",
                    "cooldown": "years between ratchets"
                }
            },
            "clyatt-95%": {
                "description": "95% of previous year's withdrawal (smoothing)",
                "fields": { "percentOfPrevious": "0.95 typical" }
            },
            "guyton-klinger": {
                "description": "Guardrail-based with adjustments",
                "fields": {
                    "amount": "initial withdrawal rate %",
                    "guardrail": "% deviation that triggers adjustment",
                    "adjustment": "% to adjust by when guardrail hit"
                }
            }
        },
        "shared_guardrail_fields": {
            "min": "number — minimum withdrawal floor",
            "minType": "'today$' | '$' | '%'",
            "minEnabled": "boolean",
            "max": "number — maximum withdrawal ceiling",
            "maxType": "'today$' | '$' | '%'",
            "maxEnabled": "boolean"
        }
    })
}

fn milestone() -> serde_json::Value {
    json!({
        "description": "Milestone — a named point in time that events can reference. Milestones are defined per-plan and can be used in DateOrMilestone fields.",
        "required_fields": {
            "name": "display name (e.g., 'Early Retirement', 'Kids Graduate')",
            "icon": "Material Design icon",
            "color": "hex color",
            "criteria": "array of criterion objects that define when the milestone occurs"
        },
        "criterion_types": {
            "year": {
                "description": "Milestone occurs at a specific calendar year",
                "example": { "type": "year", "value": 2035 }
            },
            "age": {
                "description": "Milestone occurs at a specific age",
                "example": { "type": "age", "value": 55, "owner": "me" }
            },
            "milestone": {
                "description": "Relative to another milestone",
                "example": { "type": "milestone", "value": "other-milestone-uuid", "operator": "+", "offset": 5 }
            },
            "account": {
                "description": "When an account reaches a target value",
                "example": { "type": "account", "value": 1000000, "refId": "account-uuid", "operator": ">=" }
            },
            "loan": {
                "description": "When a loan is paid off",
                "example": { "type": "loan", "refId": "asset-event-uuid" }
            }
        },
        "auto_generated_fields": {
            "id": "auto-generated if not provided (format: ms_<timestamp>)"
        },
        "note": "Computed milestones (like 'Retirement') are auto-generated by ProjectionLab and cannot be created/modified directly.",
        "example": {
            "name": "Financial Independence",
            "icon": "mdi-flag-checkered",
            "color": "#4CAF50",
            "criteria": [
                { "type": "account", "value": 2000000, "refId": "total-investments-uuid", "operator": ">=" }
            ]
        }
    })
}
