//! Schema documentation tool — returns field-level docs for ProjectionLab entity types.
//!
//! An LLM caller can use `schema_help` before creating/updating events to understand
//! required fields, valid values, and type-specific variations. Field names are the
//! camelCase names ProjectionLab uses in its V5 data model.

use rmcp::model::*;
use serde_json::json;

/// All topics the schema_help tool can explain.
const TOPICS: &[&str] = &[
    "topics",
    "v5_changes",
    "current_finances",
    "account",
    "debt",
    "real_asset",
    "date_or_milestone",
    "yearly_change",
    "expense",
    "income",
    "priority",
    "plan_account",
    "asset",
    "withdrawal_strategy",
    "milestone",
    "tax_strategy",
    "what_if",
];

pub fn lookup(topic: &str) -> CallToolResult {
    let doc = match topic {
        "topics" => topics_index(),
        "v5_changes" => v5_changes(),
        "current_finances" => current_finances(),
        "account" => account(),
        "debt" => debt(),
        "real_asset" | "starting_asset" => real_asset(),
        "date_or_milestone" => date_or_milestone(),
        "yearly_change" => yearly_change(),
        "expense" => expense(),
        "income" => income(),
        "priority" => priority(),
        "plan_account" => plan_account(),
        "asset" | "plan_asset" => asset(),
        "withdrawal_strategy" => withdrawal_strategy(),
        "milestone" => milestone(),
        "tax_strategy" | "optimize" => tax_strategy(),
        "what_if" => what_if(),
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
            "current_finances — how accounts, debts, and real assets are stored",
            "date_or_milestone — used by every event's start/end timing",
            "yearly_change — used by expenses, income, assets for growth modeling",
            "Then read the specific entity type you want to create/update"
        ],
        "best_practice": "The most reliable way to create a new plan event is to list existing events of the same type (e.g. expenses_list / plans_get) and copy one as a template, changing only what you need. Unknown fields are passed through to ProjectionLab untouched."
    })
}

fn v5_changes() -> serde_json::Value {
    json!({
        "description": "What changed in ProjectionLab V5 (late 2026) relative to older exports and older versions of these tools.",
        "export_format": "exportData now returns {schema: 1, exportedAt, documents: {currentFinances, plans, settings, accountHistory}} instead of {meta, today, plans, progress, settings}.",
        "current_finances": "savingsAccounts / investmentAccounts / debts / assets arrays were replaced by one `accounts` map keyed by id; each item has `kind` ('account' | 'debt' | 'real-asset'). Household members moved to household.people (map keyed by 'me' / 'spouse').",
        "deleting": "Accounts cannot be deleted through the plugin API (restoreCurrentFinances merges accounts). They are archived instead via `archivedOn` (ISO date) — use accounts_archive / accounts_unarchive; archived items drop out of net worth but keep their history.",
        "history": "The `progress` document and restoreProgress were removed. History is recorded automatically whenever a current value changes (updateAccountValues / updateAccount). Per-account history ships in documents.accountHistory; aggregated net worth is in the app's net-worth series (net_worth_history).",
        "plugin_api": "Plugin API version 2 adds getAccounts and updateAccountValues({accountId, values}). validateApiKey is now synchronous.",
        "plans": "Plans no longer have `active`, `initialized`, `simKey`, `hasNotes`, or `computedMilestones`. settings.plan.activeId is the plan selected in the UI and settings.plan.baselineId is the comparison baseline. Plan routes no longer include the plan id.",
        "events": "Every plan event now carries `kind` (expense, income, priority, real-asset, account). Legacy fields `title`, `key`, `planPath`, `frequencyChoices`, `color`, `subtitle`, `persistent` are gone from new events. Plan accounts use `liquidity` instead of `liquid`/`withdraw`. Real-asset events use value/purchasePrice/assessedValue instead of initialValue/amount/balance.",
        "new_event_types": "Expenses of type 'debt' (with debtId, interestRate, monthlyPayment, paymentMethod, forgiveness settings) model Current Finances debts inside a plan. Priorities gained 'roth-401k', employerMatchIsRoth, catchUpContributionsAreRoth, and mbdrDestination (mega backdoor Roth)."
    })
}

fn current_finances() -> serde_json::Value {
    json!({
        "description": "Current Finances is the household's present-day balance sheet. Every item lives in one `accounts` map keyed by id, distinguished by `kind`.",
        "kinds": {
            "account": "Savings and investment accounts — see topic 'account'",
            "debt": "Loans and credit — see topic 'debt'",
            "real-asset": "Property and valuables — see topic 'real_asset'"
        },
        "common_fields": {
            "id": "string — unique id (auto-generated on create)",
            "kind": "'account' | 'debt' | 'real-asset' (inferred from type on create)",
            "type": "type within the kind (see each kind's topic)",
            "subtype": "optional refinement, e.g. 'inherited' or 'sep' for IRAs",
            "name": "display name",
            "icon": "optional Material Design icon",
            "order": "display order within its section (auto-assigned on create)",
            "archivedOn": "ISO date when archived (absent when active)"
        },
        "value_fields": "balance (accounts, debts), costBasis (taxable-style accounts), value and purchasePrice (real assets). Changing these records history; use accounts_update_balance, accounts_update, or sync_account_balances.",
        "plan_relationship": "Plans with startingConditionsType 'default' simulate from Current Finances directly. 'custom' plans carry their own copy in plan.startingConditions. Each plan also has per-account settings in plan.accounts.events (see 'plan_account') linked by accountId."
    })
}

fn account() -> serde_json::Value {
    json!({
        "description": "A Current Finances account (kind 'account'). These are lean: balances and identity only. Growth, dividends, liquidity, and withdrawal settings are per plan (see 'plan_account').",
        "types": {
            "savings": "Cash / checking / savings (bucket: cash)",
            "taxable": "Taxable brokerage (bucket: taxable; tracks costBasis)",
            "crypto": "Cryptocurrency (bucket: crypto)",
            "401k / 403b / 457b / ira / pension": "Tax-deferred retirement (ira supports subtype 'inherited' or 'sep')",
            "roth-401k / roth-403b / roth-457b / roth-ira / hsa / 529": "Tax-free (roth-ira supports subtype 'inherited'; roth-ira tracks costBasis = contributions)",
            "rrsp / tfsa": "Canada",
            "sipp / uk-pension / stock-isa / lifetime-isa": "United Kingdom",
            "superannuation / super-nonconcessional": "Australia",
            "kh": "Israel (keren hishtalmut)"
        },
        "fields": {
            "name": "display name",
            "type": "one of the types above",
            "owner": "'me' | 'spouse' | 'joint' (joint is allowed for taxable)",
            "balance": "current balance",
            "costBasis": "taxable / roth-ira: total cost basis or contributions"
        },
        "example_create": {
            "tool": "accounts_create",
            "account_type": "taxable",
            "data": { "name": "Brokerage", "owner": "me", "balance": 25000, "costBasis": 18000 }
        }
    })
}

fn debt() -> serde_json::Value {
    json!({
        "description": "A Current Finances debt (kind 'debt').",
        "types": ["mortgage", "home-equity-loan", "auto-loan", "secured-loan", "student-loans", "credit-card", "personal-loan", "medical-debt", "other"],
        "fields": {
            "name": "display name",
            "type": "one of the types above",
            "owner": "'me' | 'spouse' | 'joint'",
            "balance": "current outstanding balance",
            "interestRate": "annual interest rate %",
            "monthlyPayment": "monthly payment",
            "securedByAssetId": "for mortgage / home-equity-loan / auto-loan / secured-loan: id of the real asset securing the loan (mortgage & HELOC: real-estate or land; auto-loan: vehicles)"
        },
        "plan_relationship": "Inside a plan, a debt's repayment is modelled by an expense of type 'debt' whose debtId points at the debt (see 'expense').",
        "example_create": {
            "tool": "debts_create",
            "data": { "type": "auto-loan", "name": "Car loan", "owner": "me", "balance": 18000, "interestRate": 4.5, "monthlyPayment": 400 }
        }
    })
}

fn real_asset() -> serde_json::Value {
    json!({
        "description": "A Current Finances real asset (kind 'real-asset'): property or valuables you own today. For future purchases inside a plan see 'asset'.",
        "types": ["real-estate", "car", "land", "motorcycle", "boat", "jewelry", "precious-metals", "furniture", "instrument", "machinery", "other"],
        "fields": {
            "name": "display name",
            "type": "one of the types above (real-estate supports subtypes 'building', 'rental', 'rental-building')",
            "value": "current market value",
            "purchasePrice": "original purchase price"
        },
        "example_create": {
            "tool": "real_assets_create",
            "data": { "type": "car", "name": "Family car", "value": 22000, "purchasePrice": 35000 }
        }
    })
}

fn date_or_milestone() -> serde_json::Value {
    json!({
        "description": "DateOrMilestone specifies when an event starts or ends. Used by every event's `start` and `end` fields (and some others, e.g. withdrawAge).",
        "fields": {
            "type": "required — 'keyword' | 'date' | 'milestone'",
            "value": "required — depends on type (see below)",
            "modifier": "optional — 'include' | 'exclude' (or a numeric offset in some contexts)"
        },
        "variants": {
            "keyword": {
                "valid_values": [
                    "now — today / plan start",
                    "beforeCurrentYear — already ongoing before the simulation starts",
                    "endOfPlan — end of the simulation horizon",
                    "never — open-ended / does not happen"
                ],
                "example": { "type": "keyword", "value": "now", "modifier": "include" }
            },
            "date": {
                "description": "Exact calendar date (ISO format)",
                "example": { "type": "date", "value": "2027-01-01" }
            },
            "milestone": {
                "description": "Reference to a milestone by id. Built-in ids: 'retirement', 'spouseRetirement', 'lifeExpectancy', 'spouseLifeExpectancy', 'fi'; custom milestones use their generated id.",
                "modifier_meaning": "'include' = starting in the milestone year, 'exclude' = the year after",
                "example": { "type": "milestone", "value": "retirement", "modifier": "include" }
            }
        },
        "common_patterns": {
            "now_until_retirement": {
                "start": { "type": "keyword", "value": "now", "modifier": "include" },
                "end": { "type": "milestone", "value": "retirement", "modifier": "exclude" }
            },
            "from_date_until_end": {
                "start": { "type": "date", "value": "2030-06-01" },
                "end": { "type": "keyword", "value": "endOfPlan", "modifier": "include" }
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
        "description": "An expense event in a plan (plan.expenses.events, kind 'expense').",
        "types": "living-expenses, rent, travel, vacation, wedding, education, healthcare, childcare, custom, debt, medicare (auto-generated), ...",
        "fields": {
            "name": "display name (required)",
            "type": "expense category (required)",
            "kind": "'expense' (filled in automatically)",
            "icon": "Material Design icon, e.g. 'mdi-home', 'mdi-airplane'",
            "amount": "numeric amount per frequency period",
            "amountType": "'today$' (inflation-adjusted) | '$' (nominal) | '%'",
            "owner": "'me' | 'spouse' | 'joint'",
            "start": "DateOrMilestone",
            "end": "DateOrMilestone",
            "frequency": "'monthly' | 'yearly' | 'once' | 'biweekly' | 'semi-monthly' | 'quarterly' | 'semi-annually'",
            "yearlyChange": "YearlyChange — how the amount grows",
            "spendingType": "'essential' | 'discretionary'",
            "hidden": "boolean — hide from charts"
        },
        "optional_fields": {
            "repeat / repeatInterval / repeatIntervalType / repeatScaler / repeatEnd": "repeat the expense every N years",
            "fundWithAccount": "boolean — pay from a specific account",
            "taxDeductible / itemized": "education and similar deductible expenses",
            "switchToMarried": "wedding: switch filing status"
        },
        "debt_type_fields": {
            "description": "type 'debt' models repayment of a Current Finances debt inside the plan",
            "debtId": "id of the Current Finances debt",
            "subtype": "the debt's type, e.g. 'student-loans', 'mortgage'",
            "interestRate / interestType ('simple' | 'compound') / compounding ('monthly' | 'yearly')": "loan terms",
            "monthlyPayment / monthlyPaymentType": "payment",
            "paymentMethod": "'financed' | 'pay-in-full'",
            "downPayment / downPaymentType": "for new debts",
            "effectiveDate": "DateOrMilestone — when these terms apply",
            "hasForgiveness / forgiveAt": "loan forgiveness"
        },
        "example": {
            "name": "Vacation",
            "type": "vacation",
            "icon": "mdi-airplane",
            "amount": 5000,
            "amountType": "today$",
            "owner": "joint",
            "start": { "type": "keyword", "value": "now", "modifier": "include" },
            "end": { "type": "keyword", "value": "endOfPlan", "modifier": "include" },
            "frequency": "yearly",
            "yearlyChange": { "type": "match-inflation", "amount": 0, "amountType": "%", "limit": 0, "limitEnabled": false, "limitType": "%" },
            "spendingType": "discretionary"
        }
    })
}

fn income() -> serde_json::Value {
    json!({
        "description": "An income event in a plan (plan.income.events, kind 'income'). `type` determines which optional fields apply.",
        "types": {
            "salary": "Employment income. Supports pension and part-time fields.",
            "rsu": "Restricted stock units. Supports routeToAccounts and repeat schedules.",
            "other": "Rental, side, or other income. Supports isDividendIncome, isPassiveIncome, selfEmployment, wage flags."
        },
        "fields": {
            "name": "display name (required)",
            "type": "'salary' | 'rsu' | 'other' (required)",
            "icon": "Material Design icon",
            "amount": "amount per frequency period",
            "amountType": "'today$' | '$' | '%'",
            "owner": "'me' | 'spouse'",
            "start": "DateOrMilestone",
            "end": "DateOrMilestone",
            "frequency": "'monthly' | 'yearly' | 'biweekly' | 'semi-monthly' | 'quarterly' | 'semi-annually' | 'once'",
            "yearlyChange": "YearlyChange — raises",
            "taxExempt": "boolean",
            "taxCharacter": "'wage' | 'selfEmployment' | ... — how the income is taxed",
            "withholdingMode": "'auto' | 'fixed' | 'none'",
            "withholdingRate": "number — % withheld when withholdingMode is 'fixed'",
            "routeToAccounts": "string[] — account ids to deposit into",
            "preventOverflow": "boolean"
        },
        "salary_specific_fields": {
            "hasPension / pensionContribution / pensionContributionType / contribsReduceTaxableIncome": "pension contributions",
            "pensionPayoutsStart / pensionPayoutsEnd / pensionPayoutAmount / pensionPayoutRate / pensionPayoutType / pensionPayoutsAreTaxFree": "pension payouts",
            "goPartTime / partTimeStart / partTimeEnd / partTimeRate": "part-time transition (rate 0.0-1.0)"
        },
        "other_specific_fields": {
            "isDividendIncome / isPassiveIncome / selfEmployment / wage": "tax treatment flags"
        },
        "legacy_fields": "Older events may still carry taxWithholding / withhold; ProjectionLab keeps them alongside the newer withholdingMode / withholdingRate.",
        "example_salary": {
            "name": "Software Engineer",
            "type": "salary",
            "icon": "mdi-briefcase",
            "amount": 150000,
            "amountType": "today$",
            "owner": "me",
            "start": { "type": "keyword", "value": "now", "modifier": "include" },
            "end": { "type": "milestone", "value": "retirement", "modifier": "exclude" },
            "frequency": "yearly",
            "yearlyChange": { "type": "inflation+", "amount": 2.0, "amountType": "%", "limit": 0, "limitEnabled": false, "limitType": "%" },
            "taxExempt": false,
            "taxCharacter": "wage",
            "withholdingMode": "auto",
            "hasPension": false
        }
    })
}

fn priority() -> serde_json::Value {
    json!({
        "description": "A priority (savings goal or contribution strategy) in a plan (plan.priorities.events, kind 'priority'). `type` matches the target account type.",
        "types": {
            "401k / 403b / 457b": "Employer plan contributions with employer match and limits",
            "roth-401k / roth-403b / roth-457b": "Roth employer plan; supports mbdrDestination (mega backdoor Roth)",
            "ira / roth-ira / hsa / 529": "Individual contributions (amount, frequency)",
            "taxable": "Taxable investing (amount, frequency, desiredContribution)",
            "savings": "Savings goal (mode, tapFund, tapRate)"
        },
        "fields": {
            "name": "display name (required)",
            "type": "one of the types above (required)",
            "icon": "Material Design icon",
            "goalIntent": "'contribute' | 'invest' | 'maintain' | 'target' ...",
            "owner": "'me' | 'spouse'",
            "accountId": "id of the target Current Finances account",
            "start": "DateOrMilestone",
            "end": "DateOrMilestone",
            "hidden": "boolean"
        },
        "employer_plan_fields": {
            "contribution / contributionType ('$' | '%') / contributionsAreFixed / contributionLimit": "employee contributions",
            "employerMatch / employerMatchType / employerMatchLimit / reduceEmployerMatch": "employer match",
            "employerMatchIsRoth": "boolean — employer match goes to Roth",
            "catchUpContributionsAreRoth": "boolean — catch-up contributions are Roth",
            "mbdrDestination": "roth-401k: where mega backdoor Roth contributions go",
            "incomeStreamId": "id of the salary the % is based on",
            "yearlyLimit / yearlyLimit$Type / yearlyLimitType": "annual limit settings"
        },
        "taxable_and_ira_fields": {
            "amount / amountType / frequency": "contribution",
            "desiredContribution": "'fixed' | 'max' | '%-remaining' ..."
        },
        "savings_fields": {
            "mode": "'target' | 'contribute'",
            "amount / amountType / frequency": "target or contribution",
            "tapFund / tapRate": "allow drawing from this fund"
        },
        "example_401k": {
            "name": "401k",
            "type": "401k",
            "icon": "mdi-piggy-bank",
            "goalIntent": "contribute",
            "owner": "me",
            "accountId": "<401k account id>",
            "start": { "type": "keyword", "value": "now", "modifier": "include" },
            "end": { "type": "milestone", "value": "retirement", "modifier": "exclude" },
            "contribution": 23500,
            "contributionType": "$",
            "contributionsAreFixed": true,
            "employerMatch": 50,
            "employerMatchType": "%",
            "employerMatchLimit": 6,
            "employerMatchIsRoth": false,
            "incomeStreamId": "<salary income id>"
        }
    })
}

fn plan_account() -> serde_json::Value {
    json!({
        "description": "Per-plan settings for a Current Finances account (plan.accounts.events, kind 'account'), linked by accountId. Edit with plan_accounts_update.",
        "fields": {
            "accountId": "id of the linked Current Finances account",
            "balance / costBasis": "starting values; plan_accounts_update writes these through to Current Finances (and the plan's own startingConditions for custom plans)",
            "owner / name / icon": "also written through to Current Finances",
            "investmentGrowthType": "'plan' (use plan assumptions) | 'fixed' | 'none'",
            "investmentGrowthRate": "annual growth % when fixed",
            "dividendType": "'plan' | 'fixed' | 'none'",
            "dividendRate": "dividend yield % when fixed",
            "isPassiveIncome": "boolean — count growth/dividends as passive income",
            "liquidity": "true (liquid now) or a DateOrMilestone when funds become accessible",
            "withdrawAge": "DateOrMilestone — when withdrawals may begin",
            "yearlyFee / yearlyFeeType": "account fees",
            "EWAge / hasEWPenalty / EWPenaltyRate": "early-withdrawal penalty rules (retirement accounts)",
            "rmdType": "required minimum distribution rules",
            "withdrawContribsFree": "roth-ira: contributions can be withdrawn penalty-free",
            "repurpose": "savings: repurpose after goals",
            "hidden": "boolean"
        }
    })
}

fn asset() -> serde_json::Value {
    json!({
        "description": "A planned real-asset purchase in a plan (plan.assets.events, kind 'real-asset'), e.g. a future home or car. For assets you own today see 'real_asset'.",
        "types": "real-estate, car, furniture, land, boat, other, ...",
        "fields": {
            "name": "display name (required)",
            "type": "asset type (required)",
            "icon": "Material Design icon",
            "owner": "'me' | 'spouse' | 'joint'",
            "start": "DateOrMilestone — purchase date",
            "end": "DateOrMilestone — sale/disposal",
            "value / valueType": "market value at purchase",
            "purchasePrice / purchasePriceType": "price paid",
            "assessedValue / assessedValueType / assessedValueGrowthRateLimit / assessedValueGrowthRateLimitType": "for property tax",
            "yearlyChange": "YearlyChange — appreciation/depreciation",
            "sellIfNeeded": "boolean — may be sold to cover shortfalls",
            "fundWithAccounts": "string[] — accounts that fund the purchase"
        },
        "financing_fields": {
            "paymentMethod": "'financed' | 'pay-in-full'",
            "downPayment / downPaymentType": "down payment",
            "monthlyPayment / monthlyPaymentType": "loan payment",
            "interestRate / interestType / compounding": "loan terms"
        },
        "cost_fields": "brokersFee, taxRate/taxRateType, insuranceRate/insuranceRateType, maintenanceRate/maintenanceRateType, monthlyHOA/monthlyHOAType",
        "real_estate_fields": "initialBuildingValue/initialBuildingValueType, classification, generateIncome, percentRented, incomeRate/incomeRateType, isPassiveIncome, cancelRent, improvementRate/improvementRateType, managementRate/managementRateType, estimateQBI, selfEmployment, estimateRentalDeductions",
        "repeat_fields": "repeat, repeatInterval, repeatIntervalType, repeatScaler, repeatEnd, repeatKeepLast",
        "example_home_purchase": {
            "name": "Home",
            "type": "real-estate",
            "icon": "mdi-home",
            "owner": "joint",
            "start": { "type": "date", "value": "2028-06-01" },
            "end": { "type": "keyword", "value": "endOfPlan", "modifier": "include" },
            "value": 600000, "valueType": "today$",
            "purchasePrice": 600000, "purchasePriceType": "today$",
            "yearlyChange": { "type": "appreciate", "amount": 3.0, "amountType": "%", "limit": 0, "limitEnabled": false, "limitType": "%" },
            "paymentMethod": "financed",
            "downPayment": 120000, "downPaymentType": "today$",
            "monthlyPayment": 3200, "monthlyPaymentType": "$",
            "interestRate": 6.5, "interestType": "compound", "compounding": "monthly",
            "taxRate": 1.0, "taxRateType": "%",
            "insuranceRate": 0.5, "insuranceRateType": "%",
            "maintenanceRate": 1.0, "maintenanceRateType": "%",
            "cancelRent": true
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
        "description": "A milestone in a plan (plan.milestones). Milestones can be referenced from DateOrMilestone fields.",
        "built_in": "Every plan has built-in milestones such as 'retirement', 'spouseRetirement', 'lifeExpectancy', 'spouseLifeExpectancy', and 'fi'. They have no `removable` flag: they can be edited (e.g. change retirement criteria) or hidden, but not deleted.",
        "fields": {
            "name": "display name (required)",
            "icon": "Material Design icon",
            "color": "color name or hex",
            "criteria": "array of criteria (required); multiple criteria are combined with each criterion's `logic`",
            "hidden": "boolean",
            "removable": "true for custom milestones (set automatically on create)",
            "createdOn": "ISO date (set automatically on create)"
        },
        "criterion_fields": {
            "type": "'year' | 'age' | 'milestone' | 'account' | 'loan' | 'liquidNetWorth' | 'investableAssets' | ...",
            "value": "number or id depending on type",
            "operator": "comparison, e.g. '>=' for value thresholds",
            "valueType": "'today$' | '$' for money thresholds",
            "refId": "account / loan / milestone id the criterion refers to",
            "modifier": "offset or include/exclude",
            "precision / years / logic / removable": "optional refinements"
        },
        "examples": {
            "year": { "type": "year", "value": 2035 },
            "age": { "type": "age", "value": 55 },
            "net_worth_target": { "type": "liquidNetWorth", "value": 2000000, "operator": ">=", "valueType": "today$" },
            "account_target": { "type": "account", "refId": "<account id>", "value": 1000000, "operator": ">=" },
            "loan_paid_off": { "type": "loan", "refId": "<debt or asset event id>" },
            "after_milestone": { "type": "milestone", "value": "retirement", "modifier": 5 }
        },
        "example": {
            "name": "Financial Independence",
            "icon": "mdi-flag-checkered",
            "color": "green",
            "criteria": [{ "type": "liquidNetWorth", "value": 2000000, "operator": ">=", "valueType": "today$" }]
        }
    })
}

fn tax_strategy() -> serde_json::Value {
    json!({
        "description": "Tax optimization settings (ProjectionLab's Optimize pages), stored in plan.variables. Read with plans_get_tax_strategy, save with plans_update_tax_strategy, compare with plans_optimize_tax_strategy / plans_what_if.",
        "prerequisite": "Strategies act on modeled income taxes. With variables.incomeTaxMode = 'fixed' (a flat effective rate) or estimateTaxes = false they have little or no effect. incomeTaxMode values: 'auto' (location-based brackets), 'custom' (your own brackets), 'fixed'.",
        "taxStrategy": {
            "role": "The shared target the module strategies fill up to each year",
            "enabled": "boolean",
            "type": "'bracket' (fill a federal bracket) | 'irmaa' (stay under an IRMAA tier) | 'income' (fill to an income amount) | 'federalPovertyLevel' (stay near a % of FPL, e.g. for ACA)",
            "bracketRate": "for type 'bracket': the bracket rate to fill, e.g. 22 (must be one of the plan's bracket rates; see plans_get_tax_strategy app.bracketRateChoices)",
            "capGainsBracketRate": "capital gains bracket to fill (shared with gain harvesting)",
            "incomeThreshold / incomeThresholdType": "for type 'income': target amount and '$' (nominal) or 'today$'",
            "niitCeiling / irmaaCeiling / acaCeiling": "booleans: also stay below these thresholds",
            "start / end": "optional DateOrMilestone window"
        },
        "rothConversionStrategy": {
            "role": "Convert pre-tax balances to Roth up to the tax strategy target",
            "enabled": "boolean",
            "start / end": "optional DateOrMilestone window",
            "maxAnnualAmount": "optional annual cap",
            "sourceAccountIds": "optional list of tax-deferred account ids to convert from (default: all eligible)"
        },
        "drawdownBlendingStrategy": {
            "role": "'Withdrawal shielding': blend tax-deferred withdrawals into low-income years up to the target",
            "enabled": "boolean",
            "type": "blending mode",
            "start / end": "optional DateOrMilestone window"
        },
        "gainHarvestingStrategy": {
            "role": "Realize capital gains up to the capital gains bracket target to step up basis",
            "enabled": "boolean",
            "capGainsBracketRate": "bracket to fill (if not shared via taxStrategy)",
            "maxAnnualGain": "optional annual cap",
            "sourceAccountIds": "optional taxable account ids",
            "start / end": "optional DateOrMilestone window"
        },
        "flexSpending": {
            "role": "Flexible spending: cut discretionary spending when the plan is under pressure",
            "enabled": "boolean",
            "scope / interpolation / points": "which spending flexes and the multiplier curve"
        },
        "drawdownOrder": "array: the account withdrawal order",
        "example_update": {
            "tool": "plans_update_tax_strategy",
            "tax_strategy": { "enabled": true, "type": "bracket", "bracketRate": 22 },
            "roth_conversions": { "enabled": true, "maxAnnualAmount": 50000 }
        }
    })
}

fn what_if() -> serde_json::Value {
    json!({
        "description": "plans_what_if runs scenarios against an unsaved copy of a plan (ProjectionLab's plan exploration mode) and compares each with the saved plan. Nothing is saved.",
        "scenario_fields": {
            "name": "label (required)",
            "variables": "plan variables to set (shallow), e.g. {\"inflation\": 3.5}, {\"investmentReturn\": 6}, strategies such as {\"rothConversionStrategy\": {...}}",
            "withdrawal_strategy": "withdrawal strategy fields to set (shallow), e.g. {\"strategy\": \"guyton-klinger\"}",
            "events": "[{section, id, patch}] to change, [{section, add: {...}}] to add, [{section, id, remove: true}] to remove; section = expenses | income | priorities | assets | accounts",
            "milestones": "[{id, patch}], e.g. move retirement: {\"id\": \"retirement\", \"patch\": {\"criteria\": [{\"type\": \"age\", \"value\": 55}]}}",
            "accounts": "[{id, patch}] Current Finances changes, e.g. {\"balance\": 150000}"
        },
        "returns": "baseline metrics and, per scenario, metrics plus vsBaseline differences: outcome, retirementAge, netWorthAtRetirement, finalNetWorth, finalNetLegacy, lifetimeIncome/Taxes/Expenses, averageEffectiveTaxRate, lifetimeRothConversions, lifetimeGainHarvesting, milestone ages, netWorthByAge",
        "examples": [
            { "name": "Retire at 55", "milestones": [{ "id": "retirement", "patch": { "criteria": [{ "type": "age", "value": 55 }] } }] },
            { "name": "Bracket taxes + Roth conversions", "variables": { "incomeTaxMode": "auto", "estimateTaxes": true, "taxStrategy": { "enabled": true, "type": "bracket", "bracketRate": 22 }, "rothConversionStrategy": { "enabled": true } } },
            { "name": "Cut travel", "events": [{ "section": "expenses", "id": "<expense id>", "patch": { "amount": 5000 } }] }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_resolves() {
        for topic in TOPICS {
            let result = lookup(topic);
            let text = &result.content[0].as_text().unwrap().text;
            assert!(!text.starts_with("Unknown topic"), "topic {} not handled", topic);
            serde_json::from_str::<serde_json::Value>(text).expect("topic returns JSON");
        }
    }
}
