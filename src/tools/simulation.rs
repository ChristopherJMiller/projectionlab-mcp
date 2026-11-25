//! Simulation result extraction from ProjectionLab's Pinia stores.
//!
//! ProjectionLab V5 no longer routes plans by id (`/plan/<id>`); the plan shown
//! is chosen with the plan store's `selectPlan`, and the latest projection for
//! the selected plan lives in `planStore.simulationRun`. `for_plan` wraps an
//! extractor so that it selects the requested plan, waits for a fresh
//! simulation, extracts, and then restores the previously selected plan/route.

/// Wrap an extractor (JS source of `function extract(run, pinia) { ... }`) so it
/// runs against a fresh simulation of `plan_id`.
pub fn for_plan(plan_id: &str, extractor: &str) -> String {
    let plan_id_json = serde_json::to_string(plan_id).expect("string serializes");
    format!(
        r#"
        const callback = arguments[arguments.length - 1];
        const targetPlanId = {plan_id_json};
        {extractor}
        (async () => {{
            const sleep = ms => new Promise(r => setTimeout(r, ms));
            // Wait for the app and its plans to finish loading (e.g. right after a page load).
            let gp, planStore;
            for (const loadStart = Date.now(); ; ) {{
                const app = document.querySelector('#app');
                gp = app && app.__vue_app__ && app.__vue_app__.config.globalProperties;
                planStore = gp && gp.$pinia && gp.$pinia._s.get('plan');
                if (planStore && (planStore.plans || []).length) break;
                if (Date.now() - loadStart > 20000) throw new Error('ProjectionLab app did not finish loading (plan store unavailable; the app may have changed).');
                await sleep(200);
            }}
            const pinia = gp.$pinia;
            const router = gp.$router;
            const worker = pinia._s.get('worker');
            const plan = planStore.plans.find(p => p.id === targetPlanId);
            if (!plan) throw new Error('Plan not found: ' + targetPlanId);

            const prevPlanId = planStore.planId;
            const prevRoute = router.currentRoute.value.fullPath;
            const prevRun = planStore.simulationRun;
            const switched = prevPlanId !== targetPlanId;
            try {{
                if (switched) planStore.selectPlan(targetPlanId);
                if (switched || !prevRun) {{
                    await router.push('/plan/overview').catch(() => {{}});
                }}
                const started = Date.now();
                for (;;) {{
                    const run = planStore.simulationRun;
                    const running = !!(worker && worker.simulationState && worker.simulationState.running);
                    const fresh = !switched || run !== prevRun;
                    if (run && Array.isArray(run.data) && run.data.length && fresh && !running) break;
                    if (Date.now() - started > 60000) throw new Error('Timed out waiting for the simulation to finish');
                    await sleep(250);
                }}
                const result = extract(planStore.simulationRun, pinia);
                return Object.assign({{ planId: plan.id, planName: plan.name }}, result);
            }} finally {{
                if (switched && prevPlanId) planStore.selectPlan(prevPlanId);
                if (router.currentRoute.value.fullPath !== prevRoute) router.push(prevRoute).catch(() => {{}});
            }}
        }})().then(r => callback(r), e => callback({{"__error": (e && e.message) || String(e)}}));
        "#
    )
}

/// Year-by-year summary, milestone completions, notable events, and outcome.
pub const SIMULATION_SUMMARY: &str = r#"
    function extract(run) {
        const years = run.data.filter(y => y.isSimulatedYear).map(y => {
            const s = y.summary || {};
            return {
                age: y.age,
                calendarYear: y.calendarYear,
                yearIndex: y.yearIndex,
                netWorth: Math.round(s.netWorth?.total || 0),
                netWorthNominal: Math.round(s.netWorth?.actualTotal || 0),
                liquidNetWorth: Math.round(s.liquidNetWorth?.total || 0),
                delta: Math.round(s.delta?.total || 0),
                income: Math.round(s.income?.total || 0),
                taxableIncome: Math.round(s.taxableIncome?.total || 0),
                expenses: Math.round(s.expenses?.total || 0),
                taxes: Math.round(s.taxes?.total || 0),
                contributions: Math.round(s.contributions?.total || 0),
                drawdown: Math.round(s.drawdown?.total || 0),
                debtPayments: Math.round(s.debtPayments?.total || 0),
                netLegacy: Math.round(s.netLegacy?.total || 0),
                withdrawalRate: s.wr?.total ?? null,
                savingsRate: s.savingsRate?.total ?? null,
                endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                endingDebt: Math.round(y.endingValues?.totalDebt || 0),
            };
        });
        const meta = run._meta || {};
        const notableEvents = (run.notableEvents || []).map(e => ({
            name: e.name,
            goalName: e.goalName,
            icon: e.icon,
            yearIndex: e.yearIndex,
            critical: e.critical,
        }));
        return {
            outcome: run.outcome,
            meta: {
                startAge: meta.startAge,
                startYear: meta.startYear,
                lifeExpectancy: meta.lifeExpectancy,
                endOfPlanAge: meta.endOfPlanAge,
                retirementYearIndex: meta.retirementYearXVal,
                spouseRetirementYearIndex: meta.spouseRetirementYearXVal,
                netWorthAtRetirement: Math.round(meta.netWorthAtRetirement || 0),
                finalNetWorth: Math.round(meta.finalNetWorth || 0),
                finalNetLegacy: Math.round(meta.finalNetLegacy || 0),
                warnings: meta.warnings,
            },
            milestoneCompletions: JSON.parse(JSON.stringify(meta.milestoneCompletionCache || {})),
            yearCount: years.length,
            years,
            notableEvents,
        };
    }
"#;

/// Detailed snapshot of every summary category at one age.
pub fn year_snapshot_extractor(age: i64) -> String {
    format!(
        r#"
        function extract(run) {{
            const targetAge = {age};
            const year = run.data.find(y => y.age === targetAge && y.isSimulatedYear);
            if (!year) {{
                const ages = run.data.filter(y => y.isSimulatedYear).map(y => y.age);
                throw new Error('No data for age ' + targetAge + '. Available ages: ' + ages[0] + '-' + ages[ages.length - 1]);
            }}
            const summary = {{}};
            for (const [k, v] of Object.entries(year.summary || {{}})) {{
                if (v && typeof v === 'object' && v.total !== undefined) {{
                    summary[k] = {{
                        total: Math.round(v.total * 100) / 100,
                        actualTotal: v.actualTotal !== undefined ? Math.round(v.actualTotal * 100) / 100 : undefined,
                        name: v.name || k,
                    }};
                }}
            }}
            return {{
                age: year.age,
                spouseAge: year.spouseAge,
                calendarYear: year.calendarYear,
                yearIndex: year.yearIndex,
                filingStatus: year.filingStatus,
                inflation: year.inflation,
                location: year.location,
                endingAccounts: Math.round(year.endingValues?.totalAccounts || 0),
                endingInvestableAccounts: Math.round(year.endingValues?.totalInvestableAccounts || 0),
                endingDebt: Math.round(year.endingValues?.totalDebt || 0),
                summary,
            }};
        }}
        "#
    )
}

/// Year-by-year figures and deltas for an age range.
pub fn year_range_extractor(start_age: i64, end_age: i64) -> String {
    format!(
        r#"
        function extract(run) {{
            const startAge = {start_age};
            const endAge = {end_age};
            const filtered = run.data.filter(y => y.isSimulatedYear && y.age >= startAge && y.age <= endAge);
            if (filtered.length === 0) throw new Error('No simulated years in range ' + startAge + '-' + endAge);
            const years = filtered.map((y, i) => {{
                const s = y.summary || {{}};
                const entry = {{
                    age: y.age,
                    calendarYear: y.calendarYear,
                    netWorth: Math.round(s.netWorth?.total || 0),
                    income: Math.round(s.income?.total || 0),
                    expenses: Math.round(s.expenses?.total || 0),
                    taxes: Math.round(s.taxes?.total || 0),
                    contributions: Math.round(s.contributions?.total || 0),
                    drawdown: Math.round(s.drawdown?.total || 0),
                    endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                    endingDebt: Math.round(y.endingValues?.totalDebt || 0),
                }};
                if (i > 0) entry.deltaNW = entry.netWorth - Math.round(filtered[i - 1].summary?.netWorth?.total || 0);
                return entry;
            }});
            const first = years[0];
            const last = years[years.length - 1];
            return {{
                range: {{ startAge, endAge, yearCount: years.length }},
                summary: {{
                    netWorthChange: last.netWorth - first.netWorth,
                    avgAnnualNWGrowth: Math.round((last.netWorth - first.netWorth) / (years.length - 1 || 1)),
                    startNetWorth: first.netWorth,
                    endNetWorth: last.netWorth,
                }},
                years,
            }};
        }}
        "#
    )
}

/// Monte Carlo results currently held by the app (does not trigger a run).
pub const MONTECARLO_RESULTS_JS: &str = r#"
    const callback = arguments[arguments.length - 1];
    try {
        const app = document.querySelector('#app');
        if (!app || !app.__vue_app__) {
            callback({"__error": "Vue app not found."});
            return;
        }
        const pinia = app.__vue_app__.config.globalProperties.$pinia;
        const mc = pinia._s.get('monte-carlo')?.montecarlo;
        if (!mc) {
            callback({"__error": "Monte Carlo store not found (the app may have changed)."});
            return;
        }
        const state = pinia._s.get('worker')?.monteCarloState || {};
        const planStore = pinia._s.get('plan');
        const plain = v => JSON.parse(JSON.stringify(v ?? null));
        const trials = (mc.trialResults || []).length;
        callback({
            activePlanId: planStore?.planId,
            hasResults: trials > 0,
            note: trials > 0 ? undefined : "No Monte Carlo results in this browser session. ProjectionLab only runs Monte Carlo when started from its Chance of Success page; results are not stored in your data.",
            trialCount: trials,
            running: !!state.running,
            completedTrialCount: state.completedTrialCount,
            totalTrialCount: state.totalTrialCount,
            outcomeStats: plain(state.stats),
            dirty: mc.dirty,
            rerunRequired: mc.rerunRequired,
            runRevision: mc.runRevision,
            percentilePlots: plain(mc.percentilePlots),
            milestoneTable: plain(mc.milestoneTable),
            lastRunConfig: plain(mc.lastRunConfig),
        });
    } catch (e) {
        callback({"__error": e.message || String(e)});
    }
"#;
