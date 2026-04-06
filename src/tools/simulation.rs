//! Handler logic for browser JS execution and simulation result extraction.

/// JavaScript for extracting simulation results from the plan Pinia store.
pub const SIMULATION_RESULTS_JS: &str = r#"
    const callback = arguments[arguments.length - 1];
    try {
        const app = document.querySelector('#app');
        if (!app || !app.__vue_app__) {
            callback({"__error": "Vue app not found. Page may not be fully loaded."});
            return;
        }
        const pinia = app.__vue_app__.config.globalProperties.$pinia;
        const store = pinia._s.get('plan');
        const results = store.plan._runtime?.results;
        if (!results || !results.data) {
            callback({"__error": "Simulation results not found. The plan may not have finished computing."});
            return;
        }

        // Extract concise yearly summaries
        const years = results.data.filter(y => y.isSimulatedYear).map(y => {
            const s = y.summary || {};
            return {
                age: y.age,
                calendarYear: y.calendarYear,
                yearIndex: y.yearIndex,
                netWorth: Math.round(s.netWorth?.total || 0),
                netWorthNominal: Math.round(s.netWorth?.actualTotal || 0),
                delta: Math.round(s.delta?.total || 0),
                income: Math.round(s.income?.total || 0),
                taxableIncome: Math.round(s.taxableIncome?.total || 0),
                expenses: Math.round(s.expenses?.total || 0),
                taxes: Math.round(s.taxes?.total || 0),
                contributions: Math.round(s.contributions?.total || 0),
                drawdown: Math.round(s.drawdown?.total || 0),
                debtPayments: Math.round(s.debtPayments?.total || 0),
                netLegacy: Math.round(s.netLegacy?.total || 0),
                withdrawalRate: s.wr?.total || null,
                endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                endingDebt: Math.round(y.endingValues?.totalDebt || 0),
            };
        });

        // Meta info
        const meta = results._meta || {};

        // Notable events (milestones, purchases, job changes, etc.)
        const events = (results.notableEvents || []).map(e => ({
            name: e.name,
            icon: e.icon,
            yearIndex: e.yearIndex,
            color: e.color,
            critical: e.critical,
        }));

        callback({
            outcome: results.outcome,
            meta: {
                startAge: meta.startAge,
                startYear: meta.startYear,
                lifeExpectancy: meta.lifeExpectancy,
                endOfPlanAge: meta.endOfPlanAge,
                retirementYearIndex: meta.retirementYearXVal,
                netWorthAtRetirement: Math.round(meta.netWorthAtRetirement || 0),
                finalNetWorth: Math.round(meta.finalNetWorth || 0),
                finalNetLegacy: Math.round(meta.finalNetLegacy || 0),
            },
            milestoneCompletions: meta.milestoneCompletionCache || {},
            yearCount: years.length,
            years: years,
            notableEvents: events,
        });
    } catch(e) {
        callback({"__error": e.message || String(e)});
    }
"#;

/// JavaScript for extracting Monte Carlo results.
pub const MONTECARLO_RESULTS_JS: &str = r#"
    const callback = arguments[arguments.length - 1];
    try {
        const app = document.querySelector('#app');
        if (!app || !app.__vue_app__) {
            callback({"__error": "Vue app not found."});
            return;
        }
        const pinia = app.__vue_app__.config.globalProperties.$pinia;
        const mcStore = pinia._s.get('monte-carlo');
        const mc = mcStore?.montecarlo;
        if (!mc) {
            callback({"__error": "Monte Carlo store not found."});
            return;
        }

        const workerStore = pinia._s.get('worker');
        const mcWorker = workerStore?.workers?.montecarlo;

        // Safe serialize helper
        const seen = new WeakSet();
        function safe(obj) {
            return JSON.parse(JSON.stringify(obj, (key, val) => {
                if (val && typeof val === 'object') {
                    if (seen.has(val)) return '[circular]';
                    seen.add(val);
                }
                return val;
            }));
        }

        callback({
            resultCount: mc.resultCount,
            dirty: mc.dirty,
            rerunRequired: mc.rerunRequired,
            workerRunning: mcWorker?.running || false,
            workerTrials: mcWorker?.trials || 0,
            workerStats: mcWorker?.stats || null,
            percentilePlots: safe(mc.percentilePlots),
            milestoneTable: safe(mc.milestoneTable),
            config: safe(mc.config),
        });
    } catch(e) {
        callback({"__error": e.message || String(e)});
    }
"#;

/// Build JavaScript for year snapshot extraction at a given age.
pub fn year_snapshot_js(age: i64) -> String {
    format!(
        r#"
        const callback = arguments[arguments.length - 1];
        const targetAge = {age};
        try {{
            const app = document.querySelector('#app');
            if (!app || !app.__vue_app__) {{
                callback({{"__error": "Vue app not found"}});
                return;
            }}
            const pinia = app.__vue_app__.config.globalProperties.$pinia;
            const store = pinia._s.get('plan');
            const data = store.plan._runtime?.results?.data;
            if (!data) {{
                callback({{"__error": "No simulation data available"}});
                return;
            }}

            const year = data.find(y => y.age === targetAge && y.isSimulatedYear);
            if (!year) {{
                const ages = data.filter(y => y.isSimulatedYear).map(y => y.age);
                callback({{"__error": "No data for age " + targetAge + ". Available ages: " + ages[0] + "-" + ages[ages.length-1]}});
                return;
            }}

            // Extract all summary categories with their totals
            const summaryCategories = {{}};
            if (year.summary) {{
                for (const [k, v] of Object.entries(year.summary)) {{
                    if (v && typeof v === 'object' && v.total !== undefined) {{
                        summaryCategories[k] = {{
                            total: Math.round(v.total * 100) / 100,
                            actualTotal: v.actualTotal !== undefined ? Math.round(v.actualTotal * 100) / 100 : undefined,
                            name: v.name || k,
                            visible: v.visible,
                        }};
                    }}
                }}
            }}

            callback({{
                age: year.age,
                calendarYear: year.calendarYear,
                yearIndex: year.yearIndex,
                inflation: year.inflation,
                cumulativeInflation: year.cumulativeInflation,
                location: year.location,
                endingAccounts: Math.round(year.endingValues?.totalAccounts || 0),
                endingDebt: Math.round(year.endingValues?.totalDebt || 0),
                summary: summaryCategories,
            }});
        }} catch(e) {{
            callback({{"__error": e.message || String(e)}});
        }}
        "#,
        age = age,
    )
}

/// Build JavaScript for year range extraction between two ages.
pub fn year_range_js(start_age: i64, end_age: i64) -> String {
    format!(
        r#"
        const callback = arguments[arguments.length - 1];
        const startAge = {start_age};
        const endAge = {end_age};
        try {{
            const app = document.querySelector('#app');
            if (!app || !app.__vue_app__) {{
                callback({{"__error": "Vue app not found"}});
                return;
            }}
            const pinia = app.__vue_app__.config.globalProperties.$pinia;
            const store = pinia._s.get('plan');
            const data = store.plan._runtime?.results?.data;
            if (!data) {{
                callback({{"__error": "No simulation data available"}});
                return;
            }}

            const filtered = data.filter(y => y.isSimulatedYear && y.age >= startAge && y.age <= endAge);
            if (filtered.length === 0) {{
                callback({{"__error": "No simulated years in range " + startAge + "-" + endAge}});
                return;
            }}

            // Extract concise data with deltas
            const years = filtered.map((y, i) => {{
                const s = y.summary || {{}};
                const nw = Math.round(s.netWorth?.total || 0);
                const inc = Math.round(s.income?.total || 0);
                const exp = Math.round(s.expenses?.total || 0);
                const tax = Math.round(s.taxes?.total || 0);
                const contrib = Math.round(s.contributions?.total || 0);
                const draw = Math.round(s.drawdown?.total || 0);

                const entry = {{
                    age: y.age,
                    calendarYear: y.calendarYear,
                    netWorth: nw,
                    income: inc,
                    expenses: exp,
                    taxes: tax,
                    contributions: contrib,
                    drawdown: draw,
                    endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                    endingDebt: Math.round(y.endingValues?.totalDebt || 0),
                }};

                // Compute deltas from previous year
                if (i > 0) {{
                    const prev = filtered[i - 1].summary || {{}};
                    entry.deltaNW = nw - Math.round(prev.netWorth?.total || 0);
                }}

                return entry;
            }});

            // Summary stats
            const first = years[0];
            const last = years[years.length - 1];
            callback({{
                range: {{ startAge, endAge, yearCount: years.length }},
                summary: {{
                    netWorthChange: last.netWorth - first.netWorth,
                    avgAnnualNWGrowth: Math.round((last.netWorth - first.netWorth) / (years.length - 1 || 1)),
                    startNetWorth: first.netWorth,
                    endNetWorth: last.netWorth,
                }},
                years: years,
            }});
        }} catch(e) {{
            callback({{"__error": e.message || String(e)}});
        }}
        "#,
        start_age = start_age,
        end_age = end_age,
    )
}
