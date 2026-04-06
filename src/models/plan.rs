/// Plan and top-level structures for ProjectionLab
use super::accounts::AccountContainer;
use super::assets::AssetContainer;
use super::common::{AmountType, AssumptionsMode, BondAllocationType, DateOrMilestone, Owner};
use super::debt::DebtAccount;
use super::expenses::ExpenseContainer;
use super::income::IncomeContainer;
use super::milestone::{ComputedMilestone, Milestone};
use super::priorities::PriorityContainer;
use super::starting_asset::StartingAsset;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A complete ProjectionLab plan
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub active: bool,
    pub initialized: bool,
    pub schema: f64,
    pub last_updated: i64,
    pub sim_key: i64,
    pub has_notes: bool,
    pub starting_conditions_type: String,

    // Event containers
    pub expenses: ExpenseContainer,
    pub income: IncomeContainer,
    pub priorities: PriorityContainer,
    pub assets: AssetContainer,
    pub accounts: AccountContainer,

    // Milestones
    pub milestones: Vec<Milestone>,
    pub computed_milestones: Vec<ComputedMilestone>,

    // Configuration
    pub starting_conditions: StartingConditions,
    pub variables: Variables,
    pub withdrawal_strategy: WithdrawalStrategy,
    pub montecarlo: MonteCarloSettings,
    pub meta: PlanMeta,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Starting conditions for a plan
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartingConditions {
    pub schema: f64,
    pub last_updated: i64,
    pub tab: i64,

    // User info
    pub your_name: String,
    pub your_icon: String,
    pub your_color: String,
    pub birth_year: i64,
    pub birth_month: i64,
    pub age: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_fractional: Option<f64>,

    // Partner info
    pub partner_status: String,
    pub spouse_name: String,
    pub spouse_icon: String,
    pub spouse_color: String,
    pub spouse_birth_year: i64,
    pub spouse_birth_month: i64,
    pub spouse_age: i64,
    pub spouse_age_gap: i64,

    // Location
    pub location: Location,

    // Initial accounts/assets
    pub savings_accounts: Vec<StartingAccount>,
    pub investment_accounts: Vec<StartingAccount>,
    pub assets: Vec<StartingAsset>,
    pub debts: Vec<DebtAccount>,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Location {
    pub country: String,
    pub state: String,
}

/// Account in starting conditions
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartingAccount {
    pub id: String,
    pub name: String,
    pub title: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub icon: String,
    pub color: String,
    pub owner: Owner,
    pub balance: f64,
    pub liquid: bool,
    pub withdraw: bool,
    pub withdraw_age: DateOrMilestone,
    pub investment_growth_rate: f64,
    pub investment_growth_type: AssumptionsMode,
    pub dividend_rate: f64,
    pub dividend_type: BondAllocationType,
    pub is_passive_income: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repurpose: Option<bool>,

    // Investment account specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yearly_fee: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yearly_fee_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividend_reinvestment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividend_tax_type: Option<AssumptionsMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividends_are_passive_income: Option<bool>,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Simulation variables and settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Variables {
    // Core simulation settings
    pub start_year: i64,
    pub start_date: String,
    pub loop_year: i64,
    pub assumptions_mode: AssumptionsMode,
    pub project_from: AssumptionsMode,
    pub cash_flow_default: String,
    pub show_future_dollars: bool,

    // Investment returns
    pub investment_return: f64,
    pub investment_return_modifier: f64,
    pub investment_return_custom: CustomRateData,
    pub bond_investment_return: f64,
    pub bond_investment_return_modifier: f64,
    pub bond_investment_return_custom: CustomRateData,

    // Dividends
    pub dividend_rate: f64,
    pub dividend_rate_modifier: f64,
    pub dividend_rate_custom: CustomRateData,
    pub bond_dividend_rate: f64,
    pub bond_dividend_rate_modifier: f64,
    pub bond_dividend_rate_custom: CustomRateData,

    // Inflation
    pub inflation: f64,
    pub inflation_modifier: f64,
    pub inflation_custom: CustomRateData,

    // Tax settings
    pub estimate_taxes: bool,
    pub filing_status: String,
    pub income_tax_mode: AssumptionsMode,
    pub income_tax_modifier: f64,
    pub effective_income_tax_rate: f64,
    pub local_income_tax_rate: f64,
    pub income_tax_national: IncomeTaxBrackets,
    pub income_tax_extra: Vec<Value>,

    // Capital gains
    pub cap_gains_mode: AssumptionsMode,
    pub cap_gains_modifier: f64,
    pub cap_gains_tax_rate: f64,
    pub cap_gains_tax_as_income: bool,
    pub cap_gains_taxable_percent: f64,
    pub cap_gains: TaxBrackets,

    // Dividend tax
    pub dividend_tax_mode: String,
    pub dividend_tax_rate: f64,
    pub dividend_tax: DividendTaxBrackets,
    pub bond_dividend_tax_mode: String,
    pub bond_dividend_tax_rate: f64,
    pub bond_dividend_tax: TaxBrackets,

    // Wealth tax
    pub wealth_tax_mode: BondAllocationType,
    pub wealth_tax_metric: String,
    pub wealth_tax_rate: f64,
    pub wealth_tax: WealthTaxBrackets,

    // Financial transaction tax
    pub ftt_mode: BondAllocationType,
    pub ftt_rate: f64,
    pub ftt_taxable_event: String,

    // Tax policy reversions
    pub tcja_reversion: bool,
    pub bbb_salt_reversion: bool,
    pub bbb_senior_reversion: bool,

    // Medicare and IRMAA
    pub medicare: bool,
    pub irmaa: bool,

    // Bond allocation
    pub bond_allocation_type: BondAllocationType,
    pub bond_allocation: Vec<BondAllocationPoint>,
    pub bond_location: BondLocation,

    // Other settings
    pub drawdown_order: Vec<String>,
    pub withholding: Withholding,
    pub flex_spending: FlexSpending,
    pub show_roth_conversion_icons: bool,

    // Estate planning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estate: Option<EstateSettings>,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Estate planning settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EstateSettings {
    pub step_up_basis: bool,
    pub admin_rate: f64,
    pub asset_liquidation_fee: f64,
    pub charity_rate: f64,
    pub tax_deferred_rate: f64,
    pub taxable_gains_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomRateData {
    #[serde(rename = "type")]
    pub rate_type: String,
    pub data: Vec<YearlyDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct YearlyDataPoint {
    pub x: i64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IncomeTaxBrackets {
    pub name: String,
    pub icon: String,
    pub standard_deduction: f64,
    pub brackets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaxBrackets {
    pub offset: String,
    pub brackets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DividendTaxBrackets {
    pub offset: String,
    pub allowance: f64,
    pub brackets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WealthTaxBrackets {
    pub allowance: f64,
    pub standard_deduction: f64,
    pub brackets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BondAllocationPoint {
    pub x: i64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BondLocation {
    #[serde(rename = "type")]
    pub location_type: String,
    pub account_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Withholding {
    pub taxable: f64,
    pub conversions: f64,
    pub tax_deferred: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FlexSpending {
    pub enabled: bool,
    pub scope: String,
    pub interpolation: BondAllocationType,
    pub points: Vec<Value>,
}

/// Withdrawal strategy settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct WithdrawalStrategy {
    pub enabled: bool,
    pub strategy: String,
    pub start: DateOrMilestone,
    pub income: String,
    #[serde(rename = "spendMode")]
    pub spend_mode: String,

    // Strategy-specific settings
    #[serde(rename = "initial-%")]
    pub initial_percent: StrategySettings,
    #[serde(rename = "fixed-%")]
    pub fixed_percent: StrategySettings,
    pub fixed_amount: FixedAmountSettings,
    #[serde(rename = "1/N")]
    pub one_over_n: StrategySettings,
    pub vpw: StrategySettings,
    pub kitces_ratchet: KitcesRatchetSettings,
    #[serde(rename = "clyatt-95%")]
    pub clyatt_95: StrategySettings,
    pub guyton_klinger: GuytonKlingerSettings,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StrategySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_of_previous: Option<f64>,
    pub min: f64,
    pub min_type: AmountType,
    pub min_enabled: bool,
    pub max: f64,
    pub max_type: AmountType,
    pub max_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FixedAmountSettings {
    pub amount: f64,
    pub amount_type: AmountType,
    pub adjust: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KitcesRatchetSettings {
    pub amount: f64,
    pub threshold: f64,
    pub ratchet: f64,
    pub cooldown: i64,
    pub min: f64,
    pub min_type: AmountType,
    pub min_enabled: bool,
    pub max: f64,
    pub max_type: AmountType,
    pub max_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuytonKlingerSettings {
    pub amount: f64,
    pub guardrail: f64,
    pub adjustment: f64,
    pub min: f64,
    pub min_type: AmountType,
    pub min_enabled: bool,
    pub max: f64,
    pub max_type: AmountType,
    pub max_enabled: bool,
}

/// Monte Carlo simulation settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonteCarloSettings {
    pub mode: String,
    pub sampling: String,
    pub trials: i64,
    pub block_size: i64,
    pub metric: String,
    pub trial_metric: String,
    pub stats_metric: String,
    pub metrics: Vec<String>,
    pub split_point_modifier: String,

    // Investment returns
    pub investment_return: String,
    pub investment_return_mean: f64,
    pub investment_return_std_dev: f64,

    // Bond returns
    pub bond_return: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_return_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_return_std_dev: Option<f64>,

    // Dividend rates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividend_rate: Option<String>,
    pub dividend_rate_mean: f64,
    pub dividend_rate_std_dev: f64,

    // Inflation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflation: Option<String>,
    pub inflation_mean: f64,
    pub inflation_std_dev: f64,

    // Simulation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iterations: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_point: Option<String>,

    // Crypto
    pub crypto_return: String,
    pub crypto_return_mean: f64,
    pub crypto_return_std_dev: f64,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Plan metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanMeta {
    pub index: i64,
    pub run_key: i64,
    pub wizard_step: i64,
    pub dirty: bool,
    pub has_added_priorities: bool,
    pub yearly_summary: YearlySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearlySummary {
    pub year_x_val: i64,
    pub date: i64,
    pub age: i64,
    pub spouse_age: i64,
}
