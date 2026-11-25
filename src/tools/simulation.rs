//! Simulation, analysis, and what-if scripts run inside the ProjectionLab app.
//!
//! ProjectionLab computes projections in the browser. These scripts drive the
//! app's own Pinia stores:
//! - the plan to analyse is chosen with the plan store's `selectPlan` (routes
//!   no longer carry plan ids) and the previous selection is restored after;
//! - the latest projection for the selected plan is `planStore.simulationRun`;
//! - what-if scenarios use the app's *plan exploration* mode
//!   (`startPlanExploration` / `endPlanExploration`): an unsaved working copy
//!   of the plan and Current Finances that the app simulates instead of the
//!   saved plan. Nothing in an exploration is persisted.
//!
//! Scripts are assembled from fixed parts by concatenation (no format!
//! escaping); inputs are passed as a JSON string parsed with the page's own
//! `window.JSON` so all objects belong to the page realm.

use serde_json::{json, Value};

/// Shared helpers available to every script.
const HARNESS: &str = r#"
const __sleep = ms => new Promise(r => setTimeout(r, ms));
const __KIND = { expenses: 'expense', income: 'income', priorities: 'priority', assets: 'real-asset', accounts: 'account' };
const __round = v => (typeof v === 'number' && Number.isFinite(v)) ? Math.round(v) : (v ?? undefined);
const __round2 = v => (typeof v === 'number' && Number.isFinite(v)) ? Math.round(v * 100) / 100 : (v ?? undefined);
// Copy a value into the page's realm. Objects created in WebDriver's sandbox
// have a foreign Object.prototype and fail ProjectionLab's plain-object checks.
const __page = v => (v && typeof v === 'object') ? window.JSON.parse(JSON.stringify(v)) : v;

async function __app() {
    for (const t0 = Date.now(); ; ) {
        const app = document.querySelector('#app');
        const gp = app && app.__vue_app__ && app.__vue_app__.config.globalProperties;
        const ps = gp && gp.$pinia && gp.$pinia._s.get('plan');
        if (ps && (ps.plans || []).length) {
            return { gp, pinia: gp.$pinia, router: gp.$router, ps, worker: gp.$pinia._s.get('worker') };
        }
        if (Date.now() - t0 > 20000) throw new Error('ProjectionLab app did not finish loading (plan store unavailable; the app may have changed).');
        await __sleep(200);
    }
}

// Wait for a finished simulation run that differs from `previous` and has
// been stable for a moment (plan edits can trigger several runs in a row).
async function __settledRun(ctx, previous) {
    const t0 = Date.now();
    let candidate = null, since = 0;
    for (;;) {
        const run = ctx.ps.simulationRun;
        const running = !!(ctx.worker && ctx.worker.simulationState && ctx.worker.simulationState.running);
        const ok = run && Array.isArray(run.data) && run.data.length > 0 && run !== previous && !running;
        if (ok) {
            if (run !== candidate) { candidate = run; since = Date.now(); }
            else if (Date.now() - since >= 400) return run;
        } else {
            candidate = null;
        }
        if (Date.now() - t0 > 90000) throw new Error('Timed out waiting for the simulation to finish');
        await __sleep(100);
    }
}

// Select `planId`, wait for its projection, run `body(ctx)`, then restore the
// previous plan selection and route and discard any exploration.
async function __withPlan(planId, body) {
    const ctx = await __app();
    const plan = ctx.ps.plans.find(p => p.id === planId);
    if (!plan) throw new Error('Plan not found: ' + planId);
    if (ctx.ps.exploration) ctx.ps.endPlanExploration();
    const prevPlanId = ctx.ps.planId;
    const prevRoute = ctx.router.currentRoute.value.fullPath;
    const prevRun = ctx.ps.simulationRun;
    const switched = prevPlanId !== planId;
    try {
        if (switched) ctx.ps.selectPlan(planId);
        if (switched || !(prevRun && prevRun.data && prevRun.data.length)) {
            await ctx.router.push('/plan/overview').catch(() => {});
        }
        ctx.plan = plan;
        ctx.baseRun = await __settledRun(ctx, switched ? prevRun : null);
        return await body(ctx);
    } finally {
        if (ctx.ps.exploration) ctx.ps.endPlanExploration();
        if (switched && prevPlanId) ctx.ps.selectPlan(prevPlanId);
        if (ctx.router.currentRoute.value.fullPath !== prevRoute) ctx.router.push(prevRoute).catch(() => {});
    }
}

function __years(run) { return run.data.filter(y => y.isSimulatedYear); }
function __age(run, yearIndex) { const m = run._meta || {}; return (m.startAge != null && yearIndex != null) ? m.startAge + yearIndex : undefined; }

function __yearAt(run, age) {
    const ys = __years(run);
    if (age == null) return ys[0];
    const y = ys.find(y => y.age === age);
    if (!y) throw new Error('No simulated year at age ' + age + '. Available ages: ' + ys[0].age + '-' + ys[ys.length - 1].age);
    return y;
}

// A summary category: total and (optionally) its line items and sections.
function __cat(y, key, withItems) {
    const c = y.summary && y.summary[key];
    if (!c || typeof c !== 'object') return undefined;
    const out = { total: __round(c.total) };
    if (withItems) {
        const items = (c.events || []).filter(e => e && e.amount).map(e => ({
            name: e.displayLabel || e.name, type: e.type || undefined, subtype: e.subtype || undefined, amount: __round(e.amount),
        }));
        if (items.length) out.items = items;
        if (c.sections && typeof c.sections === 'object') {
            const sections = {};
            for (const [k, s] of Object.entries(c.sections)) {
                if (s && typeof s === 'object' && typeof s.total === 'number') sections[s.name || k] = __round(s.total);
            }
            if (Object.keys(sections).length) out.sections = sections;
        }
    }
    return out;
}

function __filer(y, key) { return __round(y.filerVariables && y.filerVariables.total && y.filerVariables.total[key]); }

function __milestoneNames(ctx) {
    const names = {};
    const plan = ctx.ps.plan;
    for (const m of plan.milestones || []) names[m.id] = m.name;
    for (const e of (plan.priorities && plan.priorities.events) || []) names['goal:' + e.id] = 'Goal: ' + e.name;
    return names;
}

function __milestones(run, ctx) {
    const names = __milestoneNames(ctx);
    const cache = (run._meta && run._meta.milestoneCompletionCache) || {};
    return Object.entries(cache).map(([id, c]) => {
        const e = c && (c.effective || c.resolved);
        if (!e || e.yearIndex == null) return { id, name: names[id] || id, reached: false };
        return { id, name: names[id] || id, year: e.date ? new Date(e.date).getUTCFullYear() : undefined, age: __age(run, e.yearIndex) };
    });
}

// Headline metrics used by what-if, optimize, and plan comparison.
function __metrics(run, ctx) {
    const ys = __years(run);
    const meta = run._meta || {};
    const sum = k => ys.reduce((a, y) => a + ((y.summary && y.summary[k] && y.summary[k].total) || 0), 0);
    const fsum = k => ys.reduce((a, y) => a + ((y.filerVariables && y.filerVariables.total && y.filerVariables.total[k]) || 0), 0);
    const income = sum('income'), taxes = sum('taxes');
    const flex = ys.map(y => y.flexSpendingMultiplier).filter(v => typeof v === 'number');
    const netWorthByAge = {};
    ys.forEach((y, i) => { if (i % 5 === 0 || i === ys.length - 1) netWorthByAge[y.age] = __round(y.summary && y.summary.netWorth && y.summary.netWorth.total); });
    return {
        outcome: run.outcome && run.outcome.status,
        succeeded: run.outcome && run.outcome.succeeded,
        retirementAge: __age(run, meta.retirementYearXVal),
        netWorthAtRetirement: __round(meta.netWorthAtRetirement),
        finalNetWorth: __round(meta.finalNetWorth),
        finalNetLegacy: __round(meta.finalNetLegacy),
        lifetimeIncome: __round(income),
        lifetimeTaxes: __round(taxes),
        lifetimeExpenses: __round(sum('expenses')),
        averageEffectiveTaxRate: income ? Math.round(taxes / income * 10000) / 100 : null,
        lifetimeRothConversions: __round(fsum('rothConversions')),
        lifetimeGainHarvesting: __round(fsum('gainHarvesting')),
        lifetimeEarlyWithdrawalPenalties: __round(fsum('ewPenalties')),
        minFlexSpendingMultiplier: flex.length ? __round2(Math.min(...flex)) : undefined,
        milestones: __milestones(run, ctx),
        netWorthByAge,
    };
}

// Numeric differences (scenario − baseline) for the top-level metrics.
function __delta(base, other) {
    const d = {};
    for (const [k, v] of Object.entries(other)) {
        if (typeof v === 'number' && typeof base[k] === 'number') d[k] = __round2(v - base[k]);
    }
    const bm = Object.fromEntries((base.milestones || []).map(m => [m.id, m.age]));
    const changed = (other.milestones || []).filter(m => bm[m.id] !== m.age).map(m => ({ id: m.id, name: m.name, baselineAge: bm[m.id], age: m.age }));
    if (changed.length) d.milestoneAgeChanges = changed;
    return d;
}

// Apply a scenario to a fresh exploration copy of the selected plan.
function __applyScenario(ctx, sc) {
    const ex = ctx.ps.startPlanExploration();
    if (!ex || !ex.plan) throw new Error('ProjectionLab plan exploration is unavailable (the app may have changed).');
    const plan = ex.plan;
    const merge = (target, patch) => { for (const [k, v] of Object.entries(patch || {})) target[k] = __page(v); };
    if (sc.variables) merge(plan.variables, sc.variables);
    if (sc.withdrawalStrategy) merge(plan.withdrawalStrategy, sc.withdrawalStrategy);
    if (sc.montecarlo) merge(plan.montecarlo, sc.montecarlo);
    for (const ch of sc.events || []) {
        const container = plan[ch.section];
        if (!container || !Array.isArray(container.events)) throw new Error('Unknown plan section: ' + ch.section);
        const list = container.events;
        if (ch.add) {
            const e = __page(ch.add);
            if (!e.id) e.id = window.crypto.randomUUID();
            if (!e.kind && __KIND[ch.section]) e.kind = __KIND[ch.section];
            list.push(e);
            continue;
        }
        const i = list.findIndex(e => e.id === ch.id);
        if (i < 0) throw new Error('Event not found in ' + ch.section + ': ' + ch.id);
        if (ch.remove) list.splice(i, 1); else merge(list[i], ch.patch);
    }
    for (const ch of sc.milestones || []) {
        const m = (plan.milestones || []).find(m => m.id === ch.id);
        if (!m) throw new Error('Milestone not found: ' + ch.id);
        merge(m, ch.patch);
    }
    for (const ch of sc.accounts || []) {
        let found = false;
        for (const accounts of [ex.currentFinances && ex.currentFinances.accounts, plan.startingConditions && plan.startingConditions.accounts]) {
            if (accounts && accounts[ch.id]) { merge(accounts[ch.id], ch.patch); found = true; }
        }
        if (!found) throw new Error('Account not found: ' + ch.id);
    }
}

// Baseline (saved plan) vs each scenario, all extracted with `extract`.
async function __runScenarios(ctx, scenarios, extract) {
    const baseline = extract(ctx.baseRun, ctx);
    const results = [];
    for (const sc of scenarios) {
        const marker = ctx.ps.simulationRun;
        __applyScenario(ctx, sc);
        const run = await __settledRun(ctx, marker);
        const m = extract(run, ctx);
        results.push({ name: sc.name, description: sc.description, metrics: m, vsBaseline: __delta(baseline, m) });
        const before = ctx.ps.simulationRun;
        ctx.ps.endPlanExploration();
        await __settledRun(ctx, before);
    }
    return { baseline, scenarios: results };
}
"#;

const PRELUDE: &str = "const callback = arguments[arguments.length - 1];\nconst input = window.JSON.parse(";

/// Assemble a script: `input` is available as a page-realm object, the
/// harness and `extractor` definitions are in scope, and `main` is an async
/// function expression whose result is returned.
fn build(input: &Value, extractor: &str, main: &str) -> String {
    let input_literal =
        serde_json::to_string(&input.to_string()).expect("string literal serializes");
    [
        PRELUDE,
        &input_literal,
        ");\n",
        HARNESS,
        "\n",
        extractor,
        "\n(",
        main,
        ")().then(r => callback(r === undefined ? null : r), e => callback({\"__error\": (e && e.message) || String(e)}));\n",
    ]
    .concat()
}

/// Run `extractor` (defines `function extract(run, ctx)`) on a fresh projection of `plan_id`.
pub fn for_plan(plan_id: &str, extractor: &str) -> String {
    build(
        &json!({ "planId": plan_id }),
        extractor,
        "async () => __withPlan(input.planId, async ctx => Object.assign({ planId: ctx.plan.id, planName: ctx.plan.name }, extract(ctx.baseRun, ctx)))",
    )
}

/// Compare the saved plan with what-if scenarios (each applied to an unsaved exploration copy).
pub fn what_if(plan_id: &str, scenarios: &Value) -> String {
    build(
        &json!({ "planId": plan_id, "scenarios": scenarios }),
        METRICS,
        "async () => __withPlan(input.planId, async ctx => Object.assign({ planId: ctx.plan.id, planName: ctx.plan.name }, await __runScenarios(ctx, input.scenarios, extract)))",
    )
}

/// Headline metrics for several saved plans.
pub fn compare_plans(plan_ids: &[String]) -> String {
    build(
        &json!({ "planIds": plan_ids }),
        METRICS,
        r#"async () => {
            const plans = [];
            for (const id of input.planIds) {
                plans.push(await __withPlan(id, async ctx => ({ planId: ctx.plan.id, planName: ctx.plan.name, metrics: extract(ctx.baseRun, ctx) })));
            }
            const base = plans[0];
            return { plans: plans.map((p, i) => i === 0 ? p : Object.assign({}, p, { vsFirst: __delta(base.metrics, p.metrics) })) };
        }"#,
    )
}

/// The app's Optimize comparison for one strategy module: no strategy,
/// current settings, and (for bracket targets) each alternative bracket.
pub fn optimize(plan_id: &str, module: &str) -> String {
    build(
        &json!({ "planId": plan_id, "module": module }),
        METRICS,
        r#"async () => __withPlan(input.planId, async ctx => {
            const v = ctx.ps.plan.variables;
            const keys = { tax: 'taxStrategy', roth: 'rothConversionStrategy', withdrawal_shielding: 'drawdownBlendingStrategy', gain_harvesting: 'gainHarvestingStrategy' };
            const key = keys[input.module];
            if (!key) throw new Error('Unknown module ' + input.module + ' (expected tax, roth, withdrawal_shielding, gain_harvesting)');
            const current = v[key] || null;
            const disabled = obj => Object.assign({}, obj || {}, { enabled: false });
            const scenarios = [];
            if (input.module === 'tax') {
                const vars = {};
                for (const k of Object.values(keys)) if (v[k]) vars[k] = disabled(v[k]);
                scenarios.push({ name: 'No strategy', description: 'Tax strategy and all optimization modules disabled', variables: vars });
            } else {
                scenarios.push({ name: 'Without ' + input.module.replace('_', ' '), description: key + ' disabled', variables: { [key]: disabled(current) } });
                if (!current || current.enabled === false) {
                    scenarios.push({ name: 'With ' + input.module.replace('_', ' ') + ' (defaults)', description: key + ' enabled with default settings', variables: { [key]: Object.assign({}, current || {}, { enabled: true }) } });
                }
            }
            const ts = v.taxStrategy;
            const choices = (ctx.ps.nationalBracketRateChoiceValues || []).filter(r => typeof r === 'number');
            if (ts && ts.type === 'bracket') {
                for (const rate of choices) {
                    if (rate === ts.bracketRate) continue;
                    scenarios.push({ name: 'Fill ' + rate + '% bracket', description: 'taxStrategy.bracketRate = ' + rate, variables: { taxStrategy: Object.assign({}, ts, { bracketRate: rate, enabled: true }) } });
                }
            }
            const result = await __runScenarios(ctx, scenarios, extract);
            const notes = [];
            if (v.incomeTaxMode === 'fixed') notes.push('This plan uses a fixed effective income tax rate (' + v.effectiveIncomeTaxRate + '%), so bracket-based strategies have limited effect. For a meaningful comparison, try a plans_what_if scenario with variables {incomeTaxMode: \'auto\', estimateTaxes: true, taxStrategy: {enabled: true, type: \'bracket\', bracketRate: 22}} plus the strategy to test.');
            if (v.estimateTaxes === false) notes.push('Tax estimation is turned off for this plan (variables.estimateTaxes = false).');
            if (!ts || ts.enabled === false) notes.push('No tax strategy target is enabled (variables.taxStrategy); module strategies use it to decide how much to convert/harvest.');
            return Object.assign({ planId: ctx.plan.id, planName: ctx.plan.name, module: input.module, baselineLabel: 'Current plan (saved)', currentSettings: { [key]: current, taxStrategy: ts || null }, bracketRateChoices: choices, notes }, result);
        })"#,
    )
}

/// Run Monte Carlo for `plan_id` by pressing Run on the Chance of Success
/// page (the runner is private to that page), optionally with unsaved
/// `settings` overrides for `plan.montecarlo`.
pub fn monte_carlo(plan_id: &str, settings: Option<&Value>) -> String {
    build(
        &json!({ "planId": plan_id, "settings": settings }),
        "",
        r#"async () => __withPlan(input.planId, async ctx => {
            const mcStore = ctx.pinia._s.get('monte-carlo');
            if (!mcStore) throw new Error('Monte Carlo store not found (the app may have changed).');
            if (input.settings) {
                const marker = ctx.ps.simulationRun;
                __applyScenario(ctx, { montecarlo: input.settings });
                await __settledRun(ctx, marker);
            }
            const startAge = (ctx.baseRun._meta || {}).startAge;
            await ctx.router.push('/chance-of-success').catch(() => {});
            let button = null;
            for (const t0 = Date.now(); !button; await __sleep(250)) {
                button = [...document.querySelectorAll('button')].find(b => /^(re-?)?run\b/i.test((b.innerText || '').trim()) && !b.disabled);
                if (!button && Date.now() - t0 > 20000) throw new Error('Could not find the Monte Carlo Run button on the Chance of Success page (the app may have changed).');
            }
            const rev0 = mcStore.montecarlo.runRevision;
            button.click();
            const state = () => (ctx.worker && ctx.worker.monteCarloState) || {};
            for (const t0 = Date.now(); ; await __sleep(500)) {
                const mc = mcStore.montecarlo;
                if (mc.runRevision !== rev0 && !state().running && (mc.trialResults || []).length) break;
                if (Date.now() - t0 > 480000) throw new Error('Timed out waiting for Monte Carlo (' + (state().completedTrialCount || 0) + '/' + (state().totalTrialCount || '?') + ' trials)');
            }
            const mc = mcStore.montecarlo;
            const plain = v => JSON.parse(JSON.stringify(v == null ? null : v));
            const stats = plain(state().stats) || {};
            const total = mc.trialResults.length;
            const pctKeys = ['%-99', '%-95', '%-90', '%-75', '%-50', '%-25', '%-10', '%-5', '%-1'];
            const table = plain(mc.milestoneTable) || { items: [] };
            const milestones = (table.items || []).map(it => {
                const ages = {};
                for (const k of pctKeys) if (typeof it[k] === 'number') ages[k.replace('%-', 'p')] = startAge != null ? startAge + it[k] : it[k];
                return { id: it.id, name: it.name, occurrenceRate: it.completionRate, agesByPercentile: ages };
            });
            const bands = {};
            for (const [metric, plot] of Object.entries(plain(mc.percentilePlots) || {})) {
                const out = {};
                for (const ds of (plot && plot.datasets) || []) {
                    const pts = (ds.data || []).filter(p => p && typeof p.y === 'number');
                    const sampled = {};
                    pts.forEach((p, i) => { if (i % 5 === 0 || i === pts.length - 1) sampled[startAge != null ? startAge + Math.round(p.x) : Math.round(p.x)] = Math.round(p.y * 100) / 100; });
                    out[ds.label] = sampled;
                }
                bands[metric] = out;
            }
            return {
                planId: ctx.plan.id, planName: ctx.plan.name,
                trials: total,
                successRate: total ? Math.round((stats.succeeded || 0) / total * 1000) / 10 : null,
                outcomes: stats,
                milestones,
                percentileBandsByAge: bands,
                config: plain(mc.lastRunConfig),
                note: 'Ages in agesByPercentile are when the milestone is reached at that percentile (p99 = top 1% of trials). Bands are keyed by age.',
            };
        })"#,
    )
}

/// Set the comparison baseline plan (persisted in settings).
pub fn set_baseline(plan_id: &str) -> String {
    build(
        &json!({ "planId": plan_id }),
        "",
        r#"async () => {
            const ctx = await __app();
            if (!ctx.ps.plans.some(p => p.id === input.planId)) throw new Error('Plan not found: ' + input.planId);
            if (typeof ctx.ps.setBaselinePlanId !== 'function') throw new Error('setBaselinePlanId not found (the app may have changed).');
            const previous = ctx.ps.baselinePlanId;
            ctx.ps.setBaselinePlanId(input.planId);
            return { previous, baselinePlanId: ctx.ps.baselinePlanId };
        }"#,
    )
}

// ---- Extractors (each defines `function extract(run, ctx)`) ----

/// Headline metrics (see `__metrics`).
pub const METRICS: &str = "function extract(run, ctx) { return __metrics(run, ctx); }";

/// Year-by-year summary, milestone completions, notable events, and outcome.
pub const SIMULATION_SUMMARY: &str = r#"
function extract(run, ctx) {
    const years = __years(run).map(y => {
        const s = y.summary || {};
        const t = k => __round(s[k] && s[k].total);
        return {
            age: y.age, calendarYear: y.calendarYear, yearIndex: y.yearIndex,
            netWorth: t('netWorth'), netWorthNominal: __round(s.netWorth && s.netWorth.actualTotal),
            liquidNetWorth: t('liquidNetWorth'), delta: t('delta'), income: t('income'), taxableIncome: t('taxableIncome'),
            expenses: t('expenses'), taxes: t('taxes'), contributions: t('contributions'), drawdown: t('drawdown'),
            debtPayments: t('debtPayments'), netLegacy: t('netLegacy'),
            withdrawalRate: s.wr ? s.wr.total : null, savingsRate: s.savingsRate ? s.savingsRate.total : null,
            endingAccounts: __round(y.endingValues && y.endingValues.totalAccounts), endingDebt: __round(y.endingValues && y.endingValues.totalDebt),
        };
    });
    const meta = run._meta || {};
    return {
        outcome: run.outcome,
        meta: {
            startAge: meta.startAge, startYear: meta.startYear, lifeExpectancy: meta.lifeExpectancy, endOfPlanAge: meta.endOfPlanAge,
            retirementAge: __age(run, meta.retirementYearXVal), spouseRetirementAge: __age(run, meta.spouseRetirementYearXVal),
            netWorthAtRetirement: __round(meta.netWorthAtRetirement), finalNetWorth: __round(meta.finalNetWorth), finalNetLegacy: __round(meta.finalNetLegacy),
            warnings: meta.warnings,
        },
        milestones: __milestones(run, ctx),
        yearCount: years.length,
        years,
        notableEvents: (run.notableEvents || []).map(e => ({ name: e.name, goalName: e.goalName, yearIndex: e.yearIndex, age: __age(run, e.yearIndex), critical: e.critical })),
    };
}
"#;

/// Every summary category (with line items) at one age.
pub fn year_snapshot_extractor(age: i64) -> String {
    let mut s = String::from("const __targetAge = ");
    s.push_str(&age.to_string());
    s.push_str(
        r#";
function extract(run, ctx) {
    const y = __yearAt(run, __targetAge);
    const summary = {};
    for (const k of Object.keys(y.summary || {})) { const c = __cat(y, k, true); if (c && c.total !== undefined) summary[k] = c; }
    return {
        age: y.age, spouseAge: y.spouseAge, calendarYear: y.calendarYear, yearIndex: y.yearIndex, filingStatus: y.filingStatus,
        inflation: y.inflation, location: y.location, flexSpendingMultiplier: y.flexSpendingMultiplier,
        endingAccounts: __round(y.endingValues && y.endingValues.totalAccounts),
        endingInvestableAccounts: __round(y.endingValues && y.endingValues.totalInvestableAccounts),
        endingDebt: __round(y.endingValues && y.endingValues.totalDebt),
        summary,
    };
}
"#,
    );
    s
}

/// The Reports page: chosen summary categories by year (optionally with line items).
pub fn report_extractor(
    metrics: &[&str],
    start_age: Option<i64>,
    end_age: Option<i64>,
    line_items: bool,
) -> String {
    let cfg = json!({ "metrics": metrics, "start": start_age, "end": end_age, "lineItems": line_items });
    let mut s = String::from("const __report = ");
    s.push_str(&cfg.to_string());
    s.push_str(
        r#";
function extract(run, ctx) {
    const all = __years(run);
    const available = Object.keys((all[0] && all[0].summary) || {}).filter(k => all[0].summary[k] && typeof all[0].summary[k].total === 'number').sort();
    const unknown = __report.metrics.filter(m => !available.includes(m));
    if (unknown.length) throw new Error('Unknown metric(s): ' + unknown.join(', ') + '. Available: ' + available.join(', '));
    const ys = all.filter(y => (__report.start == null || y.age >= __report.start) && (__report.end == null || y.age <= __report.end));
    if (!ys.length) throw new Error('No simulated years in the requested age range');
    const years = ys.map((y, i) => {
        const row = { age: y.age, calendarYear: y.calendarYear };
        for (const m of __report.metrics) row[m] = __report.lineItems ? __cat(y, m, true) : __round(y.summary[m].total);
        if (i > 0 && y.summary.netWorth && ys[i - 1].summary.netWorth) row.netWorthChange = __round(y.summary.netWorth.total - ys[i - 1].summary.netWorth.total);
        return row;
    });
    const totals = {};
    for (const m of __report.metrics) totals[m] = __round(ys.reduce((a, y) => a + (y.summary[m].total || 0), 0));
    return { range: { startAge: ys[0].age, endAge: ys[ys.length - 1].age, years: ys.length }, metrics: __report.metrics, totals, years, availableMetrics: available };
}
"#,
    );
    s
}

/// Tax analytics: yearly income/tax figures, lifetime totals, and an optional detailed year.
pub fn tax_analytics_extractor(start_age: Option<i64>, end_age: Option<i64>, detail_age: Option<i64>) -> String {
    let cfg = json!({ "start": start_age, "end": end_age, "detail": detail_age });
    let mut s = String::from("const __tax = ");
    s.push_str(&cfg.to_string());
    s.push_str(
        r#";
function extract(run, ctx) {
    const v = ctx.ps.plan.variables || {};
    const ys = __years(run).filter(y => (__tax.start == null || y.age >= __tax.start) && (__tax.end == null || y.age <= __tax.end));
    const t = (y, k) => __round(y.summary && y.summary[k] && y.summary[k].total);
    const years = ys.map(y => ({
        age: y.age, calendarYear: y.calendarYear, filingStatus: y.filingStatus,
        grossIncome: t(y, 'income'), taxableIncome: t(y, 'taxableIncome'), stateTaxableIncome: t(y, 'stateTaxableIncome'),
        localTaxableIncome: t(y, 'localTaxableIncome'), totalTax: t(y, 'taxes'),
        effectiveTaxRate: y.summary.effectiveTaxRate ? __round2(y.summary.effectiveTaxRate.total) : undefined,
        netIncome: t(y, 'netIncome'), agi: __filer(y, 'agi'), irmaaMagi: __filer(y, 'irmaaMagi'),
        rothConversions: __filer(y, 'rothConversions'), gainHarvesting: __filer(y, 'gainHarvesting'),
        earlyWithdrawalPenalties: __filer(y, 'ewPenalties'),
        taxes: Object.fromEntries(((__cat(y, 'taxes', true) || {}).items || []).map(i => [i.name, i.amount])),
    }));
    const sum = k => years.reduce((a, y) => a + (y[k] || 0), 0);
    const income = sum('grossIncome'), taxes = sum('totalTax');
    const peak = years.reduce((best, y) => (!best || (y.totalTax || 0) > (best.totalTax || 0)) ? y : best, null);
    let detail;
    if (__tax.detail != null) {
        const y = __yearAt(run, __tax.detail);
        const tracking = (y.filerVariables && y.filerVariables.total && y.filerVariables.total.taxTracking) || [];
        detail = {
            age: y.age, calendarYear: y.calendarYear,
            income: __cat(y, 'income', true), taxableIncome: __cat(y, 'taxableIncome', true),
            stateTaxableIncome: __cat(y, 'stateTaxableIncome', true), taxes: __cat(y, 'taxes', true),
            nonTaxableIncome: __cat(y, 'nonTaxableIncome', true),
            filer: Object.fromEntries(Object.entries((y.filerVariables && y.filerVariables.total) || {}).filter(([k, x]) => typeof x === 'number').map(([k, x]) => [k, __round(x)])),
            taxStacks: tracking.map(tt => ({
                type: tt.type,
                brackets: (tt.brackets || []).map(b => ({ rate: b.rate, upTo: __round(b.end) })),
                stack: (tt.stack || []).map(s => ({ name: s.displayLabel || s.name, type: s.type, owner: s.owner || undefined, amount: __round(s.amount) })),
            })),
        };
    }
    return {
        taxSettings: {
            incomeTaxMode: v.incomeTaxMode, estimateTaxes: v.estimateTaxes, effectiveIncomeTaxRate: v.effectiveIncomeTaxRate,
            capGainsMode: v.capGainsMode, filingStatus: v.filingStatus, medicare: v.medicare, irmaa: v.irmaa,
            taxStrategyEnabled: !!(v.taxStrategy && v.taxStrategy.enabled !== false),
        },
        lifetime: {
            grossIncome: __round(income), totalTax: __round(taxes),
            averageEffectiveTaxRate: income ? Math.round(taxes / income * 10000) / 100 : null,
            rothConversions: __round(sum('rothConversions')), gainHarvesting: __round(sum('gainHarvesting')),
            earlyWithdrawalPenalties: __round(sum('earlyWithdrawalPenalties')),
            peakTaxYear: peak ? { age: peak.age, calendarYear: peak.calendarYear, totalTax: peak.totalTax } : null,
        },
        years,
        detail,
    };
}
"#,
    );
    s
}

/// Cash flow for one year: where money came from and where it went.
pub fn cash_flow_extractor(age: Option<i64>) -> String {
    let mut s = String::from("const __cfAge = ");
    s.push_str(&json!(age).to_string());
    s.push_str(
        r#";
function extract(run, ctx) {
    const y = __yearAt(run, __cfAge);
    const pick = keys => Object.fromEntries(keys.map(k => [k, __cat(y, k, true)]).filter(([k, c]) => c && c.total));
    const inflows = pick(['income', 'nonTaxableIncome', 'drawdown', 'otherWithdrawals', 'assetSales', 'assetsLiquidated']);
    const outflows = pick(['expenses', 'taxes', 'contributions', 'debtPayments', 'transfers']);
    const total = o => __round(Object.values(o).reduce((a, c) => a + (c.total || 0), 0));
    return {
        age: y.age, calendarYear: y.calendarYear,
        inflows, outflows,
        totals: { inflows: total(inflows), outflows: total(outflows) },
        spending: __cat(y, 'spending', true),
        flowFunding: __cat(y, 'flowFunding', true),
        savingsRate: y.summary.savingsRate ? __round2(y.summary.savingsRate.total) : undefined,
        flexSpendingMultiplier: y.flexSpendingMultiplier,
        note: 'Categories mirror ProjectionLab summary categories; income includes non-cash items such as employer contributions, so inflow/outflow totals need not balance exactly.',
    };
}
"#,
    );
    s
}

/// Estate: net legacy (what heirs receive) and its components.
pub fn estate_extractor(age: Option<i64>) -> String {
    let mut s = String::from("const __estateAge = ");
    s.push_str(&json!(age).to_string());
    s.push_str(
        r#";
function extract(run, ctx) {
    const ys = __years(run);
    const y = __estateAge == null ? ys[ys.length - 1] : __yearAt(run, __estateAge);
    const plain = v => JSON.parse(JSON.stringify(v == null ? null : v));
    return {
        age: y.age, calendarYear: y.calendarYear,
        netLegacy: __cat(y, 'netLegacy', true),
        netWorth: __round(y.summary.netWorth && y.summary.netWorth.total),
        estateSettings: plain((ctx.ps.plan.variables || {}).estate),
        lifeExpectancy: (run._meta || {}).lifeExpectancy,
        netLegacyByAge: Object.fromEntries(ys.filter((x, i) => i % 5 === 0 || i === ys.length - 1).map(x => [x.age, __round(x.summary.netLegacy && x.summary.netLegacy.total)])),
    };
}
"#,
    );
    s
}

/// Roth conversions year by year, with the strategy that drives them.
pub const ROTH_CONVERSIONS: &str = r#"
function extract(run, ctx) {
    const v = ctx.ps.plan.variables || {};
    const plain = x => JSON.parse(JSON.stringify(x == null ? null : x));
    const years = __years(run).map(y => ({
        age: y.age, calendarYear: y.calendarYear,
        rothConversions: __filer(y, 'rothConversions'),
        totalTax: __round(y.summary.taxes && y.summary.taxes.total),
        taxableIncome: __round(y.summary.taxableIncome && y.summary.taxableIncome.total),
        irmaaMagi: __filer(y, 'irmaaMagi'),
        transfers: (__cat(y, 'transfers', true) || {}).items,
    })).filter(y => y.rothConversions);
    const total = years.reduce((a, y) => a + (y.rothConversions || 0), 0);
    return {
        strategy: { rothConversionStrategy: plain(v.rothConversionStrategy), taxStrategy: plain(v.taxStrategy) },
        taxSettings: { incomeTaxMode: v.incomeTaxMode, estimateTaxes: v.estimateTaxes, effectiveIncomeTaxRate: v.effectiveIncomeTaxRate },
        conversionYears: years.length,
        totalConverted: __round(total),
        firstAge: years.length ? years[0].age : null,
        lastAge: years.length ? years[years.length - 1].age : null,
        years,
        note: years.length ? undefined : 'This projection makes no Roth conversions. Enable variables.rothConversionStrategy (with a tax strategy target) via plans_update_tax_strategy, or test settings first with plans_what_if / plans_optimize_tax_strategy.',
    };
}
"#;

/// Tax strategy settings plus the options the app derives for this plan.
pub const TAX_STRATEGY_INFO: &str = r#"
function extract(run, ctx) {
    const v = ctx.ps.plan.variables || {};
    const plain = x => JSON.parse(JSON.stringify(x == null ? null : x));
    const ref = a => a && { id: a.id, name: a.name, type: a.type };
    return {
        strategies: {
            taxStrategy: plain(v.taxStrategy),
            rothConversionStrategy: plain(v.rothConversionStrategy),
            drawdownBlendingStrategy: plain(v.drawdownBlendingStrategy),
            gainHarvestingStrategy: plain(v.gainHarvestingStrategy),
            flexSpending: plain(v.flexSpending),
            drawdownOrder: plain(v.drawdownOrder),
        },
        taxSettings: {
            incomeTaxMode: v.incomeTaxMode, estimateTaxes: v.estimateTaxes, effectiveIncomeTaxRate: v.effectiveIncomeTaxRate,
            capGainsMode: v.capGainsMode, filingStatus: v.filingStatus,
        },
        app: {
            hasTaxStrategy: ctx.ps.hasTaxStrategy,
            usTaxMode: ctx.ps.usTaxMode,
            bracketRateChoices: plain(ctx.ps.nationalBracketRateChoiceValues),
            capGainsBracketRateChoices: plain(ctx.ps.capGainsBracketRateChoiceValues),
            gainHarvestingEligibleAccounts: (ctx.ps.gainHarvestingEligibleAccounts || []).map(ref),
            taxDeferredAccounts: (ctx.ps.taxDeferredAccounts || []).map(ref),
            rmdsAreRelevant: ctx.ps.rmdsAreRelevant,
            drawdownModules: plain(ctx.ps.planDrawdownModules),
        },
    };
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_embed_input_as_json_string() {
        let s = for_plan("plan \"x\"", METRICS);
        assert!(s.contains(r#"window.JSON.parse("{\"planId\":\"plan \\\"x\\\"\"}")"#), "{}", &s[..200]);
        assert!(s.contains("function extract(run, ctx)"));
        assert!(s.contains("__withPlan(input.planId"));
    }

    #[test]
    fn report_extractor_embeds_config() {
        let s = report_extractor(&["taxes", "income"], Some(40), None, true);
        assert!(s.contains(r#""metrics":["taxes","income"]"#));
        assert!(s.contains(r#""start":40"#));
        assert!(s.contains(r#""lineItems":true"#));
    }
}
